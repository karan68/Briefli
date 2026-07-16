use std::{collections::HashMap, sync::Arc};

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, OriginalUri, Path, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use dashmap::DashMap;
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;

use super::{
    inbox::{CaptureInbox, InboxError, MAX_CHUNK_BYTES},
    protocol::{canonical_request_payload, CaptureManifest, PairDeviceRequest},
    repository::{DeviceSyncRepository, DeviceSyncRepositoryError, MobileCapture, PairedDevice},
    security::{sha256_hex, validate_p256_public_key, verify_p256_signature},
    session::{PairingSession, PairingSessionError},
};

const HEADER_DEVICE_ID: &str = "x-briefli-device-id";
const HEADER_SEQUENCE: &str = "x-briefli-sequence";
const HEADER_SIGNATURE: &str = "x-briefli-signature";

#[derive(Clone)]
pub struct DeviceSyncServerContext {
    pool: SqlitePool,
    inbox: CaptureInbox,
    pairing: Arc<Mutex<PairingSession>>,
    capture_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    received_sender: Option<UnboundedSender<String>>,
}

impl DeviceSyncServerContext {
    pub fn new(pool: SqlitePool, inbox: CaptureInbox, pairing: PairingSession) -> Self {
        Self {
            pool,
            inbox,
            pairing: Arc::new(Mutex::new(pairing)),
            capture_locks: Arc::new(DashMap::new()),
            received_sender: None,
        }
    }

    pub fn with_received_sender(mut self, sender: UnboundedSender<String>) -> Self {
        self.received_sender = Some(sender);
        self
    }

    fn capture_lock(&self, capture_id: &str) -> Arc<Mutex<()>> {
        self.capture_locks
            .entry(capture_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

pub fn router(context: DeviceSyncServerContext) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/pair", post(pair_device))
        .route("/v1/captures", post(register_capture))
        .route("/v1/captures/:capture_id", get(capture_status))
        .route("/v1/captures/:capture_id/chunks/:offset", put(upload_chunk))
        .route("/v1/captures/:capture_id/finalize", post(finalize_capture))
        .layer(DefaultBodyLimit::max(MAX_CHUNK_BYTES))
        .with_state(context)
}

async fn health() -> Json<HashMap<&'static str, &'static str>> {
    Json(HashMap::from([
        ("status", "ok"),
        ("service", "briefli-sync"),
    ]))
}

async fn pair_device(
    State(context): State<DeviceSyncServerContext>,
    headers: HeaderMap,
    Json(request): Json<PairDeviceRequest>,
) -> Result<(StatusCode, Json<PairedDevice>), ApiError> {
    request
        .validate()
        .map_err(|error| ApiError::bad_request("invalid_pairing_request", error.to_string()))?;
    validate_p256_public_key(&request.public_key_spki_base64)
        .map_err(|error| ApiError::bad_request("invalid_device_key", error.to_string()))?;

    let token = bearer_token(&headers)?;
    context
        .pairing
        .lock()
        .await
        .verify_and_consume(token, chrono::Utc::now())
        .map_err(ApiError::from_pairing)?;

    let device = DeviceSyncRepository::pair_device(
        &context.pool,
        &request.device_id,
        &request.device_name,
        &request.public_key_spki_base64,
    )
    .await
    .map_err(ApiError::from_repository)?;

    Ok((StatusCode::CREATED, Json(device)))
}

async fn register_capture(
    State(context): State<DeviceSyncServerContext>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<MobileCapture>), ApiError> {
    let device = authenticate(&context, &headers, &Method::POST, request_path(&uri), &body).await?;
    let manifest: CaptureManifest = serde_json::from_slice(&body)
        .map_err(|_| ApiError::bad_request("invalid_manifest", "manifest must be valid JSON"))?;
    manifest
        .validate()
        .map_err(|error| ApiError::bad_request("invalid_manifest", error.to_string()))?;

    let inbox = context.inbox.clone();
    let capture_id = manifest.capture_id.clone();
    let extension = manifest.file_extension.clone();
    let paths = tokio::task::spawn_blocking(move || inbox.prepare(&capture_id, &extension))
        .await
        .map_err(|_| ApiError::internal("inbox_task_failed", "inbox task failed"))?
        .map_err(ApiError::from_inbox)?;
    let inbox_path = paths.partial.to_string_lossy().to_string();

    let outcome =
        DeviceSyncRepository::register_capture(&context.pool, &device.id, &manifest, &inbox_path)
            .await
            .map_err(ApiError::from_repository)?;

    match outcome {
        super::repository::RegistrationOutcome::Created(capture) => {
            Ok((StatusCode::CREATED, Json(capture)))
        }
        super::repository::RegistrationOutcome::Existing(capture) => {
            Ok((StatusCode::OK, Json(capture)))
        }
    }
}

async fn capture_status(
    State(context): State<DeviceSyncServerContext>,
    OriginalUri(uri): OriginalUri,
    Path(capture_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<MobileCapture>, ApiError> {
    let device = authenticate(&context, &headers, &Method::GET, request_path(&uri), &[]).await?;
    let capture = owned_capture(&context.pool, &capture_id, &device.id).await?;

    if matches!(capture.status.as_str(), "pending" | "receiving") {
        let inbox = context.inbox.clone();
        let id = capture.id.clone();
        let extension = capture.file_extension.clone();
        let actual_bytes =
            tokio::task::spawn_blocking(move || inbox.received_bytes(&id, &extension))
                .await
                .map_err(|_| ApiError::internal("inbox_task_failed", "inbox task failed"))?
                .map_err(ApiError::from_inbox)?;

        if actual_bytes != capture.bytes_received as u64 {
            let reconciled = DeviceSyncRepository::record_received_bytes(
                &context.pool,
                &capture.id,
                &device.id,
                actual_bytes,
            )
            .await
            .map_err(ApiError::from_repository)?;
            return Ok(Json(reconciled));
        }
    }

    Ok(Json(capture))
}

async fn upload_chunk(
    State(context): State<DeviceSyncServerContext>,
    OriginalUri(uri): OriginalUri,
    Path((capture_id, offset)): Path<(String, u64)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<MobileCapture>, ApiError> {
    let device = authenticate(&context, &headers, &Method::PUT, request_path(&uri), &body).await?;
    let capture = owned_capture(&context.pool, &capture_id, &device.id).await?;
    let capture_lock = context.capture_lock(&capture_id);
    let _guard = capture_lock.lock().await;

    let inbox = context.inbox.clone();
    let id = capture.id.clone();
    let extension = capture.file_extension.clone();
    let declared_length = capture.byte_length as u64;
    let chunk = body.to_vec();
    let new_length = tokio::task::spawn_blocking(move || {
        inbox.append_chunk(&id, &extension, offset, declared_length, &chunk)
    })
    .await
    .map_err(|_| ApiError::internal("inbox_task_failed", "inbox task failed"))?
    .map_err(ApiError::from_inbox)?;

    let updated = DeviceSyncRepository::record_received_bytes(
        &context.pool,
        &capture_id,
        &device.id,
        new_length,
    )
    .await
    .map_err(ApiError::from_repository)?;
    Ok(Json(updated))
}

async fn finalize_capture(
    State(context): State<DeviceSyncServerContext>,
    OriginalUri(uri): OriginalUri,
    Path(capture_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<MobileCapture>, ApiError> {
    let device = authenticate(&context, &headers, &Method::POST, request_path(&uri), &[]).await?;
    let capture = owned_capture(&context.pool, &capture_id, &device.id).await?;

    if matches!(
        capture.status.as_str(),
        "received" | "importing" | "imported" | "failed"
    ) {
        if capture.status == "received" {
            notify_import_worker(&context, &capture.id);
        }
        return Ok(Json(capture));
    }

    let capture_lock = context.capture_lock(&capture_id);
    let _guard = capture_lock.lock().await;
    let inbox = context.inbox.clone();
    let id = capture.id.clone();
    let extension = capture.file_extension.clone();
    let expected_length = capture.byte_length as u64;
    let expected_sha256 = capture.sha256.clone();
    let completed_path = tokio::task::spawn_blocking(move || {
        inbox.finalize(&id, &extension, expected_length, &expected_sha256)
    })
    .await
    .map_err(|_| ApiError::internal("inbox_task_failed", "inbox task failed"))?
    .map_err(ApiError::from_inbox)?;

    DeviceSyncRepository::record_received_bytes(
        &context.pool,
        &capture_id,
        &device.id,
        expected_length,
    )
    .await
    .map_err(ApiError::from_repository)?;
    let completed = DeviceSyncRepository::mark_received(
        &context.pool,
        &capture_id,
        &device.id,
        &completed_path.to_string_lossy(),
    )
    .await
    .map_err(ApiError::from_repository)?;
    notify_import_worker(&context, &capture_id);
    Ok(Json(completed))
}

fn notify_import_worker(context: &DeviceSyncServerContext, capture_id: &str) {
    if let Some(sender) = &context.received_sender {
        if sender.send(capture_id.to_string()).is_err() {
            log::warn!(
                "Phone capture {} is durable but could not be queued; it will be recovered next session",
                capture_id
            );
        }
    }
}

fn request_path(uri: &Uri) -> &str {
    uri.path_and_query()
        .map(|path_and_query| path_and_query.as_str())
        .unwrap_or_else(|| uri.path())
}

async fn authenticate(
    context: &DeviceSyncServerContext,
    headers: &HeaderMap,
    method: &Method,
    path_and_query: &str,
    body: &[u8],
) -> Result<PairedDevice, ApiError> {
    let device_id = required_header(headers, HEADER_DEVICE_ID)?;
    let sequence = required_header(headers, HEADER_SEQUENCE)?
        .parse::<u64>()
        .map_err(|_| {
            ApiError::unauthorized("invalid_sequence", "sequence must be a positive integer")
        })?;
    let signature = required_header(headers, HEADER_SIGNATURE)?;
    let device = DeviceSyncRepository::get_device(&context.pool, device_id)
        .await
        .map_err(|_| ApiError::internal("database_error", "database query failed"))?
        .filter(|device| device.revoked_at.is_none())
        .ok_or_else(|| ApiError::unauthorized("unknown_device", "device is not paired"))?;

    let canonical = canonical_request_payload(
        method.as_str(),
        path_and_query,
        device_id,
        sequence,
        &sha256_hex(body),
    )
    .map_err(|error| ApiError::unauthorized("invalid_signature_input", error.to_string()))?;
    verify_p256_signature(
        &device.public_key_spki_base64,
        signature,
        canonical.as_bytes(),
    )
    .map_err(|_| ApiError::unauthorized("invalid_signature", "request signature is invalid"))?;

    DeviceSyncRepository::advance_sequence(&context.pool, device_id, sequence)
        .await
        .map_err(ApiError::from_repository)?;
    Ok(device)
}

async fn owned_capture(
    pool: &SqlitePool,
    capture_id: &str,
    device_id: &str,
) -> Result<MobileCapture, ApiError> {
    let capture = DeviceSyncRepository::get_capture(pool, capture_id)
        .await
        .map_err(|_| ApiError::internal("database_error", "database query failed"))?
        .ok_or_else(|| ApiError::not_found("capture_not_found", "capture was not found"))?;
    if capture.device_id != device_id {
        return Err(ApiError::not_found(
            "capture_not_found",
            "capture was not found",
        ));
    }
    Ok(capture)
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let authorization = required_header(headers, "authorization")?;
    authorization
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("invalid_pairing_token", "expected a bearer token"))
}

fn required_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, ApiError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::unauthorized("missing_auth_header", format!("missing {name}")))
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl ApiError {
    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message)
    }

    fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    fn internal(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, code, message)
    }

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn from_pairing(error: PairingSessionError) -> Self {
        match error {
            PairingSessionError::Expired | PairingSessionError::AlreadyConsumed => {
                Self::unauthorized("pairing_session_unavailable", error.to_string())
            }
            PairingSessionError::InvalidToken => {
                Self::unauthorized("invalid_pairing_token", error.to_string())
            }
            _ => Self::internal("pairing_session_error", error.to_string()),
        }
    }

    fn from_repository(error: DeviceSyncRepositoryError) -> Self {
        match error {
            DeviceSyncRepositoryError::ReplayedSequence => {
                Self::unauthorized("replayed_sequence", error.to_string())
            }
            DeviceSyncRepositoryError::DeviceUnavailable => {
                Self::unauthorized("unknown_device", error.to_string())
            }
            DeviceSyncRepositoryError::DeviceIdentityConflict
            | DeviceSyncRepositoryError::CaptureIdentityConflict => {
                Self::new(StatusCode::CONFLICT, "identity_conflict", error.to_string())
            }
            _ => Self::internal("database_error", error.to_string()),
        }
    }

    fn from_inbox(error: InboxError) -> Self {
        match error {
            InboxError::OffsetMismatch { .. } => {
                Self::new(StatusCode::CONFLICT, "offset_mismatch", error.to_string())
            }
            InboxError::InvalidCaptureId
            | InboxError::InvalidExtension
            | InboxError::InvalidChunkSize
            | InboxError::ExceedsDeclaredLength
            | InboxError::LengthMismatch { .. }
            | InboxError::HashMismatch => {
                Self::bad_request("invalid_capture_data", error.to_string())
            }
            InboxError::AlreadyComplete => {
                Self::new(StatusCode::CONFLICT, "already_complete", error.to_string())
            }
            InboxError::Io(_) => Self::internal("inbox_error", error.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ApiErrorBody {
            code: self.code,
            message: &self.message,
        });
        (self.status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
    use p256::{
        ecdsa::{signature::Signer, Signature, SigningKey},
        elliptic_curve::rand_core::OsRng,
        pkcs8::EncodePublicKey,
    };
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;
    use tower::ServiceExt;

    use super::*;
    use crate::device_sync::{protocol::PROTOCOL_VERSION, session::generate_pairing_session};

    struct TestClient {
        device_id: String,
        signing_key: SigningKey,
        sequence: u64,
    }

    impl TestClient {
        fn signed_request(&mut self, method: Method, path: &str, body: Vec<u8>) -> Request<Body> {
            self.sequence += 1;
            let canonical = canonical_request_payload(
                method.as_str(),
                path,
                &self.device_id,
                self.sequence,
                &sha256_hex(&body),
            )
            .unwrap();
            let signature: Signature = self.signing_key.sign(canonical.as_bytes());
            Request::builder()
                .method(method)
                .uri(path)
                .header(HEADER_DEVICE_ID, &self.device_id)
                .header(HEADER_SEQUENCE, self.sequence.to_string())
                .header(
                    HEADER_SIGNATURE,
                    BASE64_STANDARD.encode(signature.to_der().as_bytes()),
                )
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap()
        }
    }

    #[test]
    fn canonical_request_path_uses_origin_form_for_absolute_uri() {
        let uri: Uri = "https://192.168.1.69:55420/v1/captures?source=phone"
            .parse()
            .unwrap();

        assert_eq!(request_path(&uri), "/v1/captures?source=phone");
    }

    #[tokio::test]
    async fn pairs_uploads_resumes_and_finalizes_capture() {
        let pool = test_pool().await;
        let temp = tempdir().unwrap();
        let now = chrono::Utc::now();
        let materials = generate_pairing_session("127.0.0.1".parse().unwrap(), 43111, now).unwrap();
        let pairing_token = materials.pairing_token.clone();
        let app = router(DeviceSyncServerContext::new(
            pool.clone(),
            CaptureInbox::new(temp.path()),
            materials.pairing,
        ));

        let signing_key = SigningKey::random(&mut OsRng);
        let public_key = signing_key.verifying_key().to_public_key_der().unwrap();
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be".to_string();
        let pair_body = serde_json::to_vec(&PairDeviceRequest {
            protocol_version: PROTOCOL_VERSION,
            device_id: device_id.clone(),
            device_name: "Test Pixel".to_string(),
            public_key_spki_base64: BASE64_STANDARD.encode(public_key.as_bytes()),
        })
        .unwrap();
        let pair_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/pair")
                    .header("authorization", format!("Bearer {pairing_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(pair_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(pair_response.status(), StatusCode::CREATED);

        let payload = b"phone meeting audio";
        let manifest = CaptureManifest {
            protocol_version: PROTOCOL_VERSION,
            capture_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            title: "Phone meeting".to_string(),
            started_at: "2026-07-15T10:00:00Z".to_string(),
            duration_ms: 1000,
            byte_length: payload.len() as u64,
            media_type: "audio/mp4".to_string(),
            file_extension: "m4a".to_string(),
            sha256: sha256_hex(payload),
        };
        let mut client = TestClient {
            device_id,
            signing_key,
            sequence: 0,
        };

        let register_response = app
            .clone()
            .oneshot(client.signed_request(
                Method::POST,
                "/v1/captures",
                serde_json::to_vec(&manifest).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(register_response.status(), StatusCode::CREATED);

        let first = &payload[..8];
        let first_response = app
            .clone()
            .oneshot(client.signed_request(
                Method::PUT,
                &format!("/v1/captures/{}/chunks/0", manifest.capture_id),
                first.to_vec(),
            ))
            .await
            .unwrap();
        assert_eq!(first_response.status(), StatusCode::OK);

        let rest_response = app
            .clone()
            .oneshot(client.signed_request(
                Method::PUT,
                &format!("/v1/captures/{}/chunks/8", manifest.capture_id),
                payload[8..].to_vec(),
            ))
            .await
            .unwrap();
        assert_eq!(rest_response.status(), StatusCode::OK);

        let finalize_path = format!("/v1/captures/{}/finalize", manifest.capture_id);
        let finalize_response = app
            .clone()
            .oneshot(client.signed_request(Method::POST, &finalize_path, Vec::new()))
            .await
            .unwrap();
        assert_eq!(finalize_response.status(), StatusCode::OK);

        let retry_response = app
            .oneshot(client.signed_request(Method::POST, &finalize_path, Vec::new()))
            .await
            .unwrap();
        assert_eq!(retry_response.status(), StatusCode::OK);

        let capture = DeviceSyncRepository::get_capture(&pool, &manifest.capture_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(capture.status, "received");
        assert_eq!(std::fs::read(capture.inbox_path).unwrap(), payload);
    }

    #[tokio::test]
    async fn rejects_replayed_signed_request() {
        let pool = test_pool().await;
        let temp = tempdir().unwrap();
        let materials =
            generate_pairing_session("127.0.0.1".parse().unwrap(), 43111, chrono::Utc::now())
                .unwrap();
        let signing_key = SigningKey::random(&mut OsRng);
        let public_key = signing_key.verifying_key().to_public_key_der().unwrap();
        let device_id = "7d9a2b92-c934-42f4-9d08-744563ebf8be";
        DeviceSyncRepository::pair_device(
            &pool,
            device_id,
            "Test Pixel",
            &BASE64_STANDARD.encode(public_key.as_bytes()),
        )
        .await
        .unwrap();
        let app = router(DeviceSyncServerContext::new(
            pool,
            CaptureInbox::new(temp.path()),
            materials.pairing,
        ));

        let canonical = canonical_request_payload(
            "GET",
            "/v1/captures/missing",
            device_id,
            1,
            &sha256_hex(&[]),
        )
        .unwrap();
        let signature: Signature = signing_key.sign(canonical.as_bytes());
        let signature = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let make_request = || {
            Request::builder()
                .uri("/v1/captures/missing")
                .header(HEADER_DEVICE_ID, device_id)
                .header(HEADER_SEQUENCE, "1")
                .header(HEADER_SIGNATURE, &signature)
                .body(Body::empty())
                .unwrap()
        };

        let first = app.clone().oneshot(make_request()).await.unwrap();
        assert_eq!(first.status(), StatusCode::NOT_FOUND);
        let replay = app.oneshot(make_request()).await.unwrap();
        assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    }

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE meetings (id TEXT PRIMARY KEY);
             CREATE TABLE paired_devices (
                id TEXT PRIMARY KEY, display_name TEXT NOT NULL,
                public_key_spki_base64 TEXT NOT NULL,
                last_sequence INTEGER NOT NULL DEFAULT 0,
                paired_at TEXT NOT NULL, last_seen_at TEXT, revoked_at TEXT
             );
             CREATE TABLE mobile_captures (
                id TEXT PRIMARY KEY, device_id TEXT NOT NULL, title TEXT NOT NULL,
                started_at TEXT NOT NULL, duration_ms INTEGER NOT NULL,
                byte_length INTEGER NOT NULL, bytes_received INTEGER NOT NULL DEFAULT 0,
                media_type TEXT NOT NULL, file_extension TEXT NOT NULL, sha256 TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending', inbox_path TEXT NOT NULL,
                meeting_id TEXT, error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
             );",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }
}
