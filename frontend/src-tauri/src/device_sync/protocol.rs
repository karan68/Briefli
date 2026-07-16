use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_CAPTURE_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_CAPTURE_DURATION_MS: u64 = 24 * 60 * 60 * 1000;
pub const MAX_DEVICE_NAME_CHARS: usize = 80;
pub const MAX_MEETING_TITLE_CHARS: usize = 200;

const SUPPORTED_MEDIA_TYPES: &[(&str, &str)] = &[
    ("audio/mp4", "m4a"),
    ("audio/aac", "aac"),
    ("audio/mpeg", "mp3"),
    ("audio/ogg", "ogg"),
    ("audio/wav", "wav"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PairDeviceRequest {
    pub protocol_version: u16,
    pub device_id: String,
    pub device_name: String,
    pub public_key_spki_base64: String,
}

impl PairDeviceRequest {
    pub fn validate(&self) -> Result<(), ProtocolValidationError> {
        validate_protocol_version(self.protocol_version)?;
        validate_uuid("deviceId", &self.device_id)?;
        validate_text("deviceName", &self.device_name, MAX_DEVICE_NAME_CHARS)?;

        if self.public_key_spki_base64.len() < 64 || self.public_key_spki_base64.len() > 1024 {
            return Err(ProtocolValidationError::new(
                "publicKeySpkiBase64",
                "must contain a P-256 public key in SPKI form",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureManifest {
    pub protocol_version: u16,
    pub capture_id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub byte_length: u64,
    pub media_type: String,
    pub file_extension: String,
    pub sha256: String,
}

impl CaptureManifest {
    pub fn validate(&self) -> Result<(), ProtocolValidationError> {
        validate_protocol_version(self.protocol_version)?;
        validate_uuid("captureId", &self.capture_id)?;
        validate_text("title", &self.title, MAX_MEETING_TITLE_CHARS)?;

        chrono::DateTime::parse_from_rfc3339(&self.started_at).map_err(|_| {
            ProtocolValidationError::new("startedAt", "must be an RFC 3339 timestamp")
        })?;

        if self.duration_ms == 0 || self.duration_ms > MAX_CAPTURE_DURATION_MS {
            return Err(ProtocolValidationError::new(
                "durationMs",
                "must be between 1 millisecond and 24 hours",
            ));
        }

        if self.byte_length == 0 || self.byte_length > MAX_CAPTURE_BYTES {
            return Err(ProtocolValidationError::new(
                "byteLength",
                "must be between 1 byte and 1 GiB",
            ));
        }

        let extension = self.file_extension.trim().to_ascii_lowercase();
        let is_supported = SUPPORTED_MEDIA_TYPES
            .iter()
            .any(|(media_type, expected_extension)| {
                self.media_type == *media_type && extension == *expected_extension
            });
        if !is_supported {
            return Err(ProtocolValidationError::new(
                "mediaType",
                "does not match a supported audio file extension",
            ));
        }

        if self.sha256.len() != 64 || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ProtocolValidationError::new(
                "sha256",
                "must be a 64-character hexadecimal SHA-256 digest",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq)]
#[error("{field}: {message}")]
#[serde(rename_all = "camelCase")]
pub struct ProtocolValidationError {
    pub field: String,
    pub message: String,
}

impl ProtocolValidationError {
    fn new(field: &str, message: &str) -> Self {
        Self {
            field: field.to_string(),
            message: message.to_string(),
        }
    }
}

pub fn canonical_request_payload(
    method: &str,
    path: &str,
    device_id: &str,
    sequence: u64,
    body_sha256: &str,
) -> Result<String, ProtocolValidationError> {
    if method.is_empty()
        || !method
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte == b'-')
    {
        return Err(ProtocolValidationError::new(
            "method",
            "must be a non-empty uppercase HTTP method",
        ));
    }
    if !path.starts_with('/') || path.contains('\n') || path.contains('\r') {
        return Err(ProtocolValidationError::new(
            "path",
            "must be an absolute path without line breaks",
        ));
    }
    validate_uuid("deviceId", device_id)?;
    if sequence == 0 {
        return Err(ProtocolValidationError::new(
            "sequence",
            "must be greater than zero",
        ));
    }
    if body_sha256.len() != 64 || !body_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProtocolValidationError::new(
            "bodySha256",
            "must be a 64-character hexadecimal SHA-256 digest",
        ));
    }

    Ok(format!(
        "BRIEFLI-SYNC-V{PROTOCOL_VERSION}\n{method}\n{path}\n{device_id}\n{sequence}\n{}",
        body_sha256.to_ascii_lowercase()
    ))
}

fn validate_protocol_version(version: u16) -> Result<(), ProtocolValidationError> {
    if version != PROTOCOL_VERSION {
        return Err(ProtocolValidationError::new(
            "protocolVersion",
            "is not supported by this version of Briefli",
        ));
    }
    Ok(())
}

fn validate_uuid(field: &str, value: &str) -> Result<(), ProtocolValidationError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| ProtocolValidationError::new(field, "must be a valid UUID"))
}

fn validate_text(
    field: &str,
    value: &str,
    max_chars: usize,
) -> Result<(), ProtocolValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max_chars || trimmed.contains('\0') {
        return Err(ProtocolValidationError::new(
            field,
            &format!("must contain between 1 and {max_chars} characters"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> CaptureManifest {
        CaptureManifest {
            protocol_version: PROTOCOL_VERSION,
            capture_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            title: "Design review".to_string(),
            started_at: "2026-07-15T09:30:00Z".to_string(),
            duration_ms: 3_600_000,
            byte_length: 32_000_000,
            media_type: "audio/mp4".to_string(),
            file_extension: "m4a".to_string(),
            sha256: "a".repeat(64),
        }
    }

    #[test]
    fn accepts_a_valid_capture_manifest() {
        assert_eq!(valid_manifest().validate(), Ok(()));
    }

    #[test]
    fn rejects_media_type_extension_mismatch() {
        let mut manifest = valid_manifest();
        manifest.file_extension = "mp3".to_string();

        let error = manifest.validate().unwrap_err();

        assert_eq!(error.field, "mediaType");
    }

    #[test]
    fn rejects_oversized_capture_before_upload() {
        let mut manifest = valid_manifest();
        manifest.byte_length = MAX_CAPTURE_BYTES + 1;

        let error = manifest.validate().unwrap_err();

        assert_eq!(error.field, "byteLength");
    }

    #[test]
    fn rejects_non_hexadecimal_digest() {
        let mut manifest = valid_manifest();
        manifest.sha256 = "z".repeat(64);

        let error = manifest.validate().unwrap_err();

        assert_eq!(error.field, "sha256");
    }

    #[test]
    fn canonical_payload_is_stable_across_digest_case() {
        let lowercase = canonical_request_payload(
            "PUT",
            "/v1/captures/550e8400-e29b-41d4-a716-446655440000/chunks/0",
            "7d9a2b92-c934-42f4-9d08-744563ebf8be",
            42,
            &"ab".repeat(32),
        )
        .unwrap();
        let uppercase = canonical_request_payload(
            "PUT",
            "/v1/captures/550e8400-e29b-41d4-a716-446655440000/chunks/0",
            "7d9a2b92-c934-42f4-9d08-744563ebf8be",
            42,
            &"AB".repeat(32),
        )
        .unwrap();

        assert_eq!(lowercase, uppercase);
        assert!(lowercase.starts_with("BRIEFLI-SYNC-V1\nPUT\n/v1/captures/"));
    }

    #[test]
    fn canonical_payload_rejects_line_break_injection() {
        let error = canonical_request_payload(
            "PUT",
            "/v1/captures/1\nforged",
            "7d9a2b92-c934-42f4-9d08-744563ebf8be",
            1,
            &"ab".repeat(32),
        )
        .unwrap_err();

        assert_eq!(error.field, "path");
    }
}
