use chrono::Utc;
use serde::Serialize;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use thiserror::Error;

use super::protocol::CaptureManifest;

#[derive(Debug, Clone, FromRow, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    pub id: String,
    pub display_name: String,
    pub public_key_spki_base64: String,
    pub last_sequence: i64,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MobileCapture {
    pub id: String,
    pub device_id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: i64,
    pub byte_length: i64,
    pub bytes_received: i64,
    pub media_type: String,
    pub file_extension: String,
    pub sha256: String,
    pub status: String,
    pub inbox_path: String,
    pub meeting_id: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationOutcome {
    Created(MobileCapture),
    Existing(MobileCapture),
}

#[derive(Debug, Error)]
pub enum DeviceSyncRepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("device ID is already paired with a different public key")]
    DeviceIdentityConflict,
    #[error("paired device is revoked or does not exist")]
    DeviceUnavailable,
    #[error("request sequence was already used")]
    ReplayedSequence,
    #[error("capture ID is already registered with different immutable metadata")]
    CaptureIdentityConflict,
    #[error("numeric value exceeds SQLite's supported range")]
    NumericOverflow,
}

pub struct DeviceSyncRepository;

impl DeviceSyncRepository {
    pub async fn pair_device(
        pool: &SqlitePool,
        device_id: &str,
        display_name: &str,
        public_key_spki_base64: &str,
    ) -> Result<PairedDevice, DeviceSyncRepositoryError> {
        let now = Utc::now().to_rfc3339();
        let mut transaction = pool.begin().await?;

        sqlx::query(
            "INSERT INTO paired_devices
                (id, display_name, public_key_spki_base64, paired_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(device_id)
        .bind(display_name)
        .bind(public_key_spki_base64)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        let existing = sqlx::query_as::<_, PairedDevice>(
            "SELECT id, display_name, public_key_spki_base64, last_sequence,
                    paired_at, last_seen_at, revoked_at
             FROM paired_devices
             WHERE id = ?",
        )
        .bind(device_id)
        .fetch_one(&mut *transaction)
        .await?;

        if existing.public_key_spki_base64 != public_key_spki_base64 {
            transaction.rollback().await?;
            return Err(DeviceSyncRepositoryError::DeviceIdentityConflict);
        }
        if existing.revoked_at.is_some() {
            sqlx::query(
                "UPDATE paired_devices
                 SET display_name = ?, last_sequence = 0, paired_at = ?,
                     last_seen_at = NULL, revoked_at = NULL
                 WHERE id = ? AND revoked_at IS NOT NULL",
            )
            .bind(display_name)
            .bind(&now)
            .bind(device_id)
            .execute(&mut *transaction)
            .await?;
        } else {
            sqlx::query("UPDATE paired_devices SET display_name = ? WHERE id = ?")
                .bind(display_name)
                .bind(device_id)
                .execute(&mut *transaction)
                .await?;
        }

        transaction.commit().await?;
        Self::get_device(pool, device_id)
            .await?
            .ok_or(DeviceSyncRepositoryError::DeviceUnavailable)
    }

    pub async fn get_device(
        pool: &SqlitePool,
        device_id: &str,
    ) -> Result<Option<PairedDevice>, sqlx::Error> {
        sqlx::query_as::<_, PairedDevice>(
            "SELECT id, display_name, public_key_spki_base64, last_sequence,
                    paired_at, last_seen_at, revoked_at
             FROM paired_devices
             WHERE id = ?",
        )
        .bind(device_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn list_devices(pool: &SqlitePool) -> Result<Vec<PairedDevice>, sqlx::Error> {
        sqlx::query_as::<_, PairedDevice>(
            "SELECT id, display_name, public_key_spki_base64, last_sequence,
                    paired_at, last_seen_at, revoked_at
             FROM paired_devices
             WHERE revoked_at IS NULL
             ORDER BY paired_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn revoke_device(
        pool: &SqlitePool,
        device_id: &str,
    ) -> Result<(), DeviceSyncRepositoryError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE paired_devices
             SET revoked_at = ?
             WHERE id = ? AND revoked_at IS NULL",
        )
        .bind(now)
        .bind(device_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(DeviceSyncRepositoryError::DeviceUnavailable)
        }
    }

    pub async fn advance_sequence(
        pool: &SqlitePool,
        device_id: &str,
        sequence: u64,
    ) -> Result<(), DeviceSyncRepositoryError> {
        let sequence = i64::try_from(sequence)
            .map_err(|_| DeviceSyncRepositoryError::NumericOverflow)?;
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE paired_devices
             SET last_sequence = ?, last_seen_at = ?
             WHERE id = ? AND revoked_at IS NULL AND last_sequence < ?",
        )
        .bind(sequence)
        .bind(now)
        .bind(device_id)
        .bind(sequence)
        .execute(pool)
        .await?;

        if result.rows_affected() == 1 {
            return Ok(());
        }

        match Self::get_device(pool, device_id).await? {
            Some(device) if device.revoked_at.is_none() => {
                Err(DeviceSyncRepositoryError::ReplayedSequence)
            }
            _ => Err(DeviceSyncRepositoryError::DeviceUnavailable),
        }
    }

    pub async fn register_capture(
        pool: &SqlitePool,
        device_id: &str,
        manifest: &CaptureManifest,
        inbox_path: &str,
    ) -> Result<RegistrationOutcome, DeviceSyncRepositoryError> {
        let duration_ms = i64::try_from(manifest.duration_ms)
            .map_err(|_| DeviceSyncRepositoryError::NumericOverflow)?;
        let byte_length = i64::try_from(manifest.byte_length)
            .map_err(|_| DeviceSyncRepositoryError::NumericOverflow)?;
        let now = Utc::now().to_rfc3339();
        let mut transaction = pool.begin().await?;

        let result = sqlx::query(
            "INSERT INTO mobile_captures
                (id, device_id, title, started_at, duration_ms, byte_length,
                 media_type, file_extension, sha256, inbox_path, created_at, updated_at)
             SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
             WHERE EXISTS (
                 SELECT 1 FROM paired_devices
                 WHERE id = ? AND revoked_at IS NULL
             )
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&manifest.capture_id)
        .bind(device_id)
        .bind(&manifest.title)
        .bind(&manifest.started_at)
        .bind(duration_ms)
        .bind(byte_length)
        .bind(&manifest.media_type)
        .bind(&manifest.file_extension)
        .bind(manifest.sha256.to_ascii_lowercase())
        .bind(inbox_path)
        .bind(&now)
        .bind(&now)
        .bind(device_id)
        .execute(&mut *transaction)
        .await?;

        let capture = sqlx::query_as::<_, MobileCapture>(
            "SELECT id, device_id, title, started_at, duration_ms, byte_length,
                    bytes_received, media_type, file_extension, sha256, status,
                    inbox_path, meeting_id, error, created_at, updated_at
             FROM mobile_captures
             WHERE id = ?",
        )
        .bind(&manifest.capture_id)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some(capture) = capture else {
            transaction.rollback().await?;
            return Err(DeviceSyncRepositoryError::DeviceUnavailable);
        };

        if !capture_matches_manifest(&capture, device_id, manifest) {
            transaction.rollback().await?;
            return Err(DeviceSyncRepositoryError::CaptureIdentityConflict);
        }

        transaction.commit().await?;
        if result.rows_affected() == 1 {
            Ok(RegistrationOutcome::Created(capture))
        } else {
            Ok(RegistrationOutcome::Existing(capture))
        }
    }

    pub async fn get_capture(
        pool: &SqlitePool,
        capture_id: &str,
    ) -> Result<Option<MobileCapture>, sqlx::Error> {
        sqlx::query_as::<_, MobileCapture>(
            "SELECT id, device_id, title, started_at, duration_ms, byte_length,
                    bytes_received, media_type, file_extension, sha256, status,
                    inbox_path, meeting_id, error, created_at, updated_at
             FROM mobile_captures
             WHERE id = ?",
        )
        .bind(capture_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn record_received_bytes(
        pool: &SqlitePool,
        capture_id: &str,
        device_id: &str,
        bytes_received: u64,
    ) -> Result<MobileCapture, DeviceSyncRepositoryError> {
        let bytes_received = i64::try_from(bytes_received)
            .map_err(|_| DeviceSyncRepositoryError::NumericOverflow)?;
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE mobile_captures
             SET bytes_received = ?, status = 'receiving', error = NULL, updated_at = ?
             WHERE id = ? AND device_id = ?
               AND status IN ('pending', 'receiving')
               AND ? <= byte_length",
        )
        .bind(bytes_received)
        .bind(now)
        .bind(capture_id)
        .bind(device_id)
        .bind(bytes_received)
        .execute(pool)
        .await?;

        if result.rows_affected() != 1 {
            return Err(DeviceSyncRepositoryError::CaptureIdentityConflict);
        }
        Self::get_capture(pool, capture_id)
            .await?
            .ok_or(DeviceSyncRepositoryError::CaptureIdentityConflict)
    }

    pub async fn mark_received(
        pool: &SqlitePool,
        capture_id: &str,
        device_id: &str,
        completed_path: &str,
    ) -> Result<MobileCapture, DeviceSyncRepositoryError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE mobile_captures
             SET status = 'received', inbox_path = ?, error = NULL, updated_at = ?
             WHERE id = ? AND device_id = ?
               AND status IN ('receiving', 'received')
               AND bytes_received = byte_length",
        )
        .bind(completed_path)
        .bind(now)
        .bind(capture_id)
        .bind(device_id)
        .execute(pool)
        .await?;

        if result.rows_affected() != 1 {
            return Err(DeviceSyncRepositoryError::CaptureIdentityConflict);
        }
        Self::get_capture(pool, capture_id)
            .await?
            .ok_or(DeviceSyncRepositoryError::CaptureIdentityConflict)
    }

    pub async fn mark_importing(
        pool: &SqlitePool,
        capture_id: &str,
    ) -> Result<(), DeviceSyncRepositoryError> {
        transition_status(pool, capture_id, "received", "importing", None, None).await
    }

    pub async fn mark_imported(
        pool: &SqlitePool,
        capture_id: &str,
        meeting_id: &str,
    ) -> Result<(), DeviceSyncRepositoryError> {
        transition_status(
            pool,
            capture_id,
            "importing",
            "imported",
            Some(meeting_id),
            None,
        )
        .await
    }

    pub async fn mark_imported_in_transaction(
        transaction: &mut Transaction<'_, Sqlite>,
        capture_id: &str,
        meeting_id: &str,
    ) -> Result<(), DeviceSyncRepositoryError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE mobile_captures
             SET status = 'imported', meeting_id = ?, error = NULL, updated_at = ?
             WHERE id = ? AND status = 'importing'",
        )
        .bind(meeting_id)
        .bind(now)
        .bind(capture_id)
        .execute(&mut **transaction)
        .await?;

        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(DeviceSyncRepositoryError::CaptureIdentityConflict)
        }
    }

    pub async fn recover_interrupted_imports(
        pool: &SqlitePool,
    ) -> Result<u64, DeviceSyncRepositoryError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE mobile_captures
             SET status = 'received', meeting_id = NULL,
                 error = 'Import interrupted; queued for retry', updated_at = ?
             WHERE status = 'importing'",
        )
        .bind(now)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn mark_import_failed(
        pool: &SqlitePool,
        capture_id: &str,
        error: &str,
    ) -> Result<(), DeviceSyncRepositoryError> {
        transition_status(
            pool,
            capture_id,
            "importing",
            "failed",
            None,
            Some(error),
        )
        .await
    }

    pub async fn list_captures(
        pool: &SqlitePool,
    ) -> Result<Vec<MobileCapture>, sqlx::Error> {
        sqlx::query_as::<_, MobileCapture>(
            "SELECT id, device_id, title, started_at, duration_ms, byte_length,
                    bytes_received, media_type, file_extension, sha256, status,
                    inbox_path, meeting_id, error, created_at, updated_at
             FROM mobile_captures
             ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }
}

async fn transition_status(
    pool: &SqlitePool,
    capture_id: &str,
    expected_status: &str,
    next_status: &str,
    meeting_id: Option<&str>,
    error: Option<&str>,
) -> Result<(), DeviceSyncRepositoryError> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE mobile_captures
         SET status = ?, meeting_id = COALESCE(?, meeting_id), error = ?, updated_at = ?
         WHERE id = ? AND status = ?",
    )
    .bind(next_status)
    .bind(meeting_id)
    .bind(error)
    .bind(now)
    .bind(capture_id)
    .bind(expected_status)
    .execute(pool)
    .await?;

    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(DeviceSyncRepositoryError::CaptureIdentityConflict)
    }
}

fn capture_matches_manifest(
    capture: &MobileCapture,
    device_id: &str,
    manifest: &CaptureManifest,
) -> bool {
    capture.device_id == device_id
        && capture.title == manifest.title
        && capture.started_at == manifest.started_at
        && capture.duration_ms == manifest.duration_ms as i64
        && capture.byte_length == manifest.byte_length as i64
        && capture.media_type == manifest.media_type
        && capture.file_extension == manifest.file_extension
        && capture.sha256 == manifest.sha256.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;
    use crate::device_sync::protocol::PROTOCOL_VERSION;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE meetings (id TEXT PRIMARY KEY);
             CREATE TABLE paired_devices (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                public_key_spki_base64 TEXT NOT NULL,
                last_sequence INTEGER NOT NULL DEFAULT 0,
                paired_at TEXT NOT NULL,
                last_seen_at TEXT,
                revoked_at TEXT
             );
             CREATE TABLE mobile_captures (
                id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                title TEXT NOT NULL,
                started_at TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                byte_length INTEGER NOT NULL,
                bytes_received INTEGER NOT NULL DEFAULT 0,
                media_type TEXT NOT NULL,
                file_extension TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                inbox_path TEXT NOT NULL,
                meeting_id TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn manifest() -> CaptureManifest {
        CaptureManifest {
            protocol_version: PROTOCOL_VERSION,
            capture_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            title: "Architecture review".to_string(),
            started_at: "2026-07-15T10:00:00Z".to_string(),
            duration_ms: 600_000,
            byte_length: 8_000_000,
            media_type: "audio/mp4".to_string(),
            file_extension: "m4a".to_string(),
            sha256: "ab".repeat(32),
        }
    }

    async fn paired_pool() -> SqlitePool {
        let pool = test_pool().await;
        DeviceSyncRepository::pair_device(
            &pool,
            "7d9a2b92-c934-42f4-9d08-744563ebf8be",
            "Pixel",
            &"A".repeat(96),
        )
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn refuses_device_key_replacement() {
        let pool = paired_pool().await;

        let error = DeviceSyncRepository::pair_device(
            &pool,
            "7d9a2b92-c934-42f4-9d08-744563ebf8be",
            "Impersonator",
            &"B".repeat(96),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            DeviceSyncRepositoryError::DeviceIdentityConflict
        ));
    }

    #[tokio::test]
    async fn sequence_only_moves_forward() {
        let pool = paired_pool().await;
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";

        DeviceSyncRepository::advance_sequence(&pool, device_id, 5)
            .await
            .unwrap();
        let replay = DeviceSyncRepository::advance_sequence(&pool, device_id, 5)
            .await
            .unwrap_err();
        let older = DeviceSyncRepository::advance_sequence(&pool, device_id, 4)
            .await
            .unwrap_err();
        DeviceSyncRepository::advance_sequence(&pool, device_id, 6)
            .await
            .unwrap();

        assert!(matches!(
            replay,
            DeviceSyncRepositoryError::ReplayedSequence
        ));
        assert!(matches!(
            older,
            DeviceSyncRepositoryError::ReplayedSequence
        ));
    }

    #[tokio::test]
    async fn revoked_device_is_hidden_and_can_only_return_through_pairing() {
        let pool = paired_pool().await;
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";
        let public_key = "A".repeat(96);
        DeviceSyncRepository::advance_sequence(&pool, device_id, 5)
            .await
            .unwrap();

        DeviceSyncRepository::revoke_device(&pool, device_id)
            .await
            .unwrap();

        assert!(DeviceSyncRepository::list_devices(&pool).await.unwrap().is_empty());
        assert!(matches!(
            DeviceSyncRepository::advance_sequence(&pool, device_id, 6)
                .await
                .unwrap_err(),
            DeviceSyncRepositoryError::DeviceUnavailable
        ));

        let repaired = DeviceSyncRepository::pair_device(
            &pool,
            device_id,
            "Pixel repaired",
            &public_key,
        )
        .await
        .unwrap();

        assert_eq!(repaired.display_name, "Pixel repaired");
        assert_eq!(repaired.last_sequence, 0);
        assert!(repaired.revoked_at.is_none());
        assert_eq!(DeviceSyncRepository::list_devices(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn capture_registration_is_idempotent_across_inbox_path_changes() {
        let pool = paired_pool().await;
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";
        let manifest = manifest();

        let first = DeviceSyncRepository::register_capture(
            &pool,
            device_id,
            &manifest,
            "C:/inbox/capture.part",
        )
        .await
        .unwrap();
        let retry = DeviceSyncRepository::register_capture(
            &pool,
            device_id,
            &manifest,
            "D:/moved-inbox/capture.part",
        )
        .await
        .unwrap();

        assert!(matches!(first, RegistrationOutcome::Created(_)));
        let RegistrationOutcome::Existing(existing) = retry else {
            panic!("retry should return the existing capture");
        };
        assert_eq!(existing.inbox_path, "C:/inbox/capture.part");
    }

    #[tokio::test]
    async fn changed_manifest_cannot_reuse_capture_id() {
        let pool = paired_pool().await;
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";
        let manifest = manifest();
        DeviceSyncRepository::register_capture(
            &pool,
            device_id,
            &manifest,
            "C:/inbox/capture.part",
        )
        .await
        .unwrap();

        let mut changed = manifest;
        changed.sha256 = "cd".repeat(32);
        let error = DeviceSyncRepository::register_capture(
            &pool,
            device_id,
            &changed,
            "C:/inbox/capture.part",
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            DeviceSyncRepositoryError::CaptureIdentityConflict
        ));
    }

    #[tokio::test]
    async fn received_and_imported_transitions_require_complete_transfer() {
        let pool = paired_pool().await;
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";
        let manifest = manifest();
        DeviceSyncRepository::register_capture(
            &pool,
            device_id,
            &manifest,
            "C:/inbox/audio.part",
        )
        .await
        .unwrap();

        assert!(DeviceSyncRepository::mark_received(
            &pool,
            &manifest.capture_id,
            device_id,
            "C:/inbox/audio.m4a",
        )
        .await
        .is_err());

        DeviceSyncRepository::record_received_bytes(
            &pool,
            &manifest.capture_id,
            device_id,
            manifest.byte_length,
        )
        .await
        .unwrap();
        DeviceSyncRepository::mark_received(
            &pool,
            &manifest.capture_id,
            device_id,
            "C:/inbox/audio.m4a",
        )
        .await
        .unwrap();
        DeviceSyncRepository::mark_importing(&pool, &manifest.capture_id)
            .await
            .unwrap();
        DeviceSyncRepository::mark_imported(
            &pool,
            &manifest.capture_id,
            "meeting-1",
        )
        .await
        .unwrap();

        let capture = DeviceSyncRepository::get_capture(&pool, &manifest.capture_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(capture.status, "imported");
        assert_eq!(capture.meeting_id.as_deref(), Some("meeting-1"));
    }

    #[tokio::test]
    async fn imported_transition_can_commit_with_the_meeting_transaction() {
        let pool = paired_pool().await;
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";
        let manifest = manifest();
        DeviceSyncRepository::register_capture(
            &pool,
            device_id,
            &manifest,
            "C:/inbox/audio.part",
        )
        .await
        .unwrap();
        DeviceSyncRepository::record_received_bytes(
            &pool,
            &manifest.capture_id,
            device_id,
            manifest.byte_length,
        )
        .await
        .unwrap();
        DeviceSyncRepository::mark_received(
            &pool,
            &manifest.capture_id,
            device_id,
            "C:/inbox/audio.m4a",
        )
        .await
        .unwrap();
        DeviceSyncRepository::mark_importing(&pool, &manifest.capture_id)
            .await
            .unwrap();

        let mut transaction = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO meetings (id) VALUES ('meeting-atomic')")
            .execute(&mut *transaction)
            .await
            .unwrap();
        DeviceSyncRepository::mark_imported_in_transaction(
            &mut transaction,
            &manifest.capture_id,
            "meeting-atomic",
        )
        .await
        .unwrap();
        transaction.commit().await.unwrap();

        let capture = DeviceSyncRepository::get_capture(&pool, &manifest.capture_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(capture.status, "imported");
        assert_eq!(capture.meeting_id.as_deref(), Some("meeting-atomic"));
    }

    #[tokio::test]
    async fn interrupted_imports_are_requeued_for_retry() {
        let pool = paired_pool().await;
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";
        let manifest = manifest();
        DeviceSyncRepository::register_capture(
            &pool,
            device_id,
            &manifest,
            "C:/inbox/audio.part",
        )
        .await
        .unwrap();
        DeviceSyncRepository::record_received_bytes(
            &pool,
            &manifest.capture_id,
            device_id,
            manifest.byte_length,
        )
        .await
        .unwrap();
        DeviceSyncRepository::mark_received(
            &pool,
            &manifest.capture_id,
            device_id,
            "C:/inbox/audio.m4a",
        )
        .await
        .unwrap();
        DeviceSyncRepository::mark_importing(&pool, &manifest.capture_id)
            .await
            .unwrap();

        let recovered = DeviceSyncRepository::recover_interrupted_imports(&pool)
            .await
            .unwrap();

        let capture = DeviceSyncRepository::get_capture(&pool, &manifest.capture_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered, 1);
        assert_eq!(capture.status, "received");
        assert!(capture.meeting_id.is_none());
        assert_eq!(
            capture.error.as_deref(),
            Some("Import interrupted; queued for retry")
        );
    }
}