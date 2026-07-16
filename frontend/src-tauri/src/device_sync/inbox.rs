use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CaptureInbox {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturePaths {
    pub directory: PathBuf,
    pub partial: PathBuf,
    pub completed: PathBuf,
}

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("capture ID is not a valid UUID")]
    InvalidCaptureId,
    #[error("file extension is not safe")]
    InvalidExtension,
    #[error("chunk must contain between 1 byte and 1 MiB")]
    InvalidChunkSize,
    #[error("chunk offset mismatch: expected {expected}, current file length is {actual}")]
    OffsetMismatch { expected: u64, actual: u64 },
    #[error("chunk would exceed the declared capture length")]
    ExceedsDeclaredLength,
    #[error("capture length mismatch: expected {expected}, actual {actual}")]
    LengthMismatch { expected: u64, actual: u64 },
    #[error("capture SHA-256 does not match its manifest")]
    HashMismatch,
    #[error("capture is already complete")]
    AlreadyComplete,
    #[error("capture file operation failed: {0}")]
    Io(#[from] std::io::Error),
}

impl CaptureInbox {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn paths_for(
        &self,
        capture_id: &str,
        file_extension: &str,
    ) -> Result<CapturePaths, InboxError> {
        let capture_id = Uuid::parse_str(capture_id).map_err(|_| InboxError::InvalidCaptureId)?;
        let extension = file_extension.trim().to_ascii_lowercase();
        if extension.is_empty()
            || extension.len() > 8
            || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(InboxError::InvalidExtension);
        }

        let directory = self.root.join(capture_id.to_string());
        Ok(CapturePaths {
            partial: directory.join("audio.part"),
            completed: directory.join(format!("audio.{extension}")),
            directory,
        })
    }

    pub fn prepare(
        &self,
        capture_id: &str,
        file_extension: &str,
    ) -> Result<CapturePaths, InboxError> {
        let paths = self.paths_for(capture_id, file_extension)?;
        std::fs::create_dir_all(&paths.directory)?;
        if paths.completed.exists() {
            return Err(InboxError::AlreadyComplete);
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.partial)?;
        Ok(paths)
    }

    pub fn received_bytes(
        &self,
        capture_id: &str,
        file_extension: &str,
    ) -> Result<u64, InboxError> {
        let paths = self.paths_for(capture_id, file_extension)?;
        if paths.completed.exists() {
            return Ok(std::fs::metadata(paths.completed)?.len());
        }
        match std::fs::metadata(paths.partial) {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error.into()),
        }
    }

    pub fn append_chunk(
        &self,
        capture_id: &str,
        file_extension: &str,
        offset: u64,
        declared_length: u64,
        chunk: &[u8],
    ) -> Result<u64, InboxError> {
        if chunk.is_empty() || chunk.len() > MAX_CHUNK_BYTES {
            return Err(InboxError::InvalidChunkSize);
        }
        let paths = self.prepare(capture_id, file_extension)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&paths.partial)?;
        let current_length = file.metadata()?.len();
        if current_length != offset {
            return Err(InboxError::OffsetMismatch {
                expected: offset,
                actual: current_length,
            });
        }

        let new_length = offset
            .checked_add(chunk.len() as u64)
            .ok_or(InboxError::ExceedsDeclaredLength)?;
        if new_length > declared_length {
            return Err(InboxError::ExceedsDeclaredLength);
        }

        file.seek(SeekFrom::Start(offset))?;
        if let Err(error) = file.write_all(chunk).and_then(|_| file.sync_data()) {
            if let Err(rollback_error) = file.set_len(offset).and_then(|_| file.sync_data()) {
                return Err(InboxError::Io(std::io::Error::new(
                    rollback_error.kind(),
                    format!(
                        "chunk write failed ({error}); rollback to offset {offset} also failed ({rollback_error})"
                    ),
                )));
            }
            return Err(InboxError::Io(error));
        }
        Ok(new_length)
    }

    pub fn finalize(
        &self,
        capture_id: &str,
        file_extension: &str,
        expected_length: u64,
        expected_sha256: &str,
    ) -> Result<PathBuf, InboxError> {
        let paths = self.paths_for(capture_id, file_extension)?;
        if paths.completed.exists() {
            verify_file(
                &paths.completed,
                expected_length,
                expected_sha256,
            )?;
            return Ok(paths.completed);
        }

        verify_file(&paths.partial, expected_length, expected_sha256)?;
        std::fs::rename(&paths.partial, &paths.completed)?;
        sync_directory(&paths.directory)?;
        Ok(paths.completed)
    }
}

fn verify_file(
    path: &Path,
    expected_length: u64,
    expected_sha256: &str,
) -> Result<(), InboxError> {
    let mut file = File::open(path)?;
    let actual_length = file.metadata()?.len();
    if actual_length != expected_length {
        return Err(InboxError::LengthMismatch {
            expected: expected_length,
            actual: actual_length,
        });
    }

    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        digest.update(&buffer[..bytes_read]);
    }

    if format!("{:x}", digest.finalize()) != expected_sha256.to_ascii_lowercase() {
        return Err(InboxError::HashMismatch);
    }
    Ok(())
}

#[cfg(windows)]
fn sync_directory(_directory: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(directory: &Path) -> Result<(), std::io::Error> {
    File::open(directory)?.sync_all()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::device_sync::security::sha256_hex;

    const CAPTURE_ID: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn resumes_from_exact_file_offset() {
        let temp = tempdir().unwrap();
        let inbox = CaptureInbox::new(temp.path());
        let payload = b"first-second";

        assert_eq!(
            inbox
                .append_chunk(CAPTURE_ID, "m4a", 0, payload.len() as u64, b"first-")
                .unwrap(),
            6
        );
        assert_eq!(inbox.received_bytes(CAPTURE_ID, "m4a").unwrap(), 6);
        assert_eq!(
            inbox
                .append_chunk(CAPTURE_ID, "m4a", 6, payload.len() as u64, b"second")
                .unwrap(),
            payload.len() as u64
        );

        let completed = inbox
            .finalize(
                CAPTURE_ID,
                "m4a",
                payload.len() as u64,
                &sha256_hex(payload),
            )
            .unwrap();
        assert_eq!(std::fs::read(completed).unwrap(), payload);
    }

    #[test]
    fn rejects_stale_or_skipped_offset_without_writing() {
        let temp = tempdir().unwrap();
        let inbox = CaptureInbox::new(temp.path());
        inbox
            .append_chunk(CAPTURE_ID, "m4a", 0, 10, b"12345")
            .unwrap();

        let stale = inbox
            .append_chunk(CAPTURE_ID, "m4a", 0, 10, b"again")
            .unwrap_err();
        let skipped = inbox
            .append_chunk(CAPTURE_ID, "m4a", 8, 10, b"xx")
            .unwrap_err();

        assert!(matches!(stale, InboxError::OffsetMismatch { actual: 5, .. }));
        assert!(matches!(skipped, InboxError::OffsetMismatch { actual: 5, .. }));
        assert_eq!(inbox.received_bytes(CAPTURE_ID, "m4a").unwrap(), 5);
    }

    #[test]
    fn enforces_chunk_and_declared_file_limits() {
        let temp = tempdir().unwrap();
        let inbox = CaptureInbox::new(temp.path());

        assert!(matches!(
            inbox.append_chunk(CAPTURE_ID, "m4a", 0, 1, &[]),
            Err(InboxError::InvalidChunkSize)
        ));
        assert!(matches!(
            inbox.append_chunk(CAPTURE_ID, "m4a", 0, 2, b"three"),
            Err(InboxError::ExceedsDeclaredLength)
        ));
    }

    #[test]
    fn refuses_finalize_on_hash_or_length_mismatch() {
        let temp = tempdir().unwrap();
        let inbox = CaptureInbox::new(temp.path());
        inbox
            .append_chunk(CAPTURE_ID, "m4a", 0, 4, b"data")
            .unwrap();

        assert!(matches!(
            inbox.finalize(CAPTURE_ID, "m4a", 5, &sha256_hex(b"data")),
            Err(InboxError::LengthMismatch { .. })
        ));
        assert!(matches!(
            inbox.finalize(CAPTURE_ID, "m4a", 4, &"00".repeat(32)),
            Err(InboxError::HashMismatch)
        ));
        assert_eq!(inbox.received_bytes(CAPTURE_ID, "m4a").unwrap(), 4);
    }

    #[test]
    fn capture_id_and_extension_cannot_escape_inbox() {
        let temp = tempdir().unwrap();
        let inbox = CaptureInbox::new(temp.path());

        assert!(matches!(
            inbox.paths_for("../../outside", "m4a"),
            Err(InboxError::InvalidCaptureId)
        ));
        assert!(matches!(
            inbox.paths_for(CAPTURE_ID, "../exe"),
            Err(InboxError::InvalidExtension)
        ));
    }
}