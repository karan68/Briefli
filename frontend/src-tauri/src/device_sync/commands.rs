use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use axum_server::{tls_rustls::RustlsConfig, Handle};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

use super::{
    inbox::CaptureInbox,
    repository::{DeviceSyncRepository, MobileCapture, PairedDevice},
    server::{router, DeviceSyncServerContext},
    session::{generate_pairing_session, PairingSessionView},
};

pub struct DeviceSyncRuntimeState {
    running: Arc<Mutex<Option<RunningServer>>>,
}

struct RunningServer {
    handle: Handle,
    session: PairingSessionView,
    cancellation: CancellationToken,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSyncStatus {
    pub running: bool,
    pub session: Option<PairingSessionView>,
    pub paired_devices: Vec<PairedDevice>,
    pub captures: Vec<MobileCapture>,
}

impl DeviceSyncRuntimeState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn shutdown(&self) {
        if let Some(server) = self.running.lock().await.take() {
            server.cancellation.cancel();
            server
                .handle
                .graceful_shutdown(Some(Duration::from_secs(2)));
        }
    }
}

impl Default for DeviceSyncRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn start_device_sync_session(
    app: AppHandle,
    runtime: State<'_, DeviceSyncRuntimeState>,
) -> Result<PairingSessionView, String> {
    runtime.shutdown().await;

    let local_ip = select_lan_ipv4()?;
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
        .map_err(|error| format!("Failed to bind the local sync server: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure the local sync server: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Failed to read the local sync port: {error}"))?
        .port();

    let materials = generate_pairing_session(IpAddr::V4(local_ip), port, chrono::Utc::now())
        .map_err(|error| format!("Failed to create pairing session: {error}"))?;
    let tls_config =
        RustlsConfig::from_der(vec![materials.certificate_der], materials.private_key_der)
            .await
            .map_err(|error| format!("Failed to configure sync encryption: {error}"))?;

    let app_state = app
        .try_state::<AppState>()
        .ok_or_else(|| "Briefli database is not ready".to_string())?;
    let inbox_root = device_sync_inbox_root(&app)?;
    std::fs::create_dir_all(&inbox_root)
        .map_err(|error| format!("Failed to create the phone sync inbox: {error}"))?;
    let context = DeviceSyncServerContext::new(
        app_state.db_manager.pool().clone(),
        CaptureInbox::new(inbox_root),
        materials.pairing,
    );
    let (import_sender, import_receiver) = mpsc::unbounded_channel();
    let cancellation = CancellationToken::new();
    let context = context.with_received_sender(import_sender.clone());
    let service = router(context);
    let handle = Handle::new();
    let server_handle = handle.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(error) = axum_server::from_tcp_rustls(listener, tls_config)
            .handle(server_handle)
            .serve(service.into_make_service())
            .await
        {
            log::error!("Phone sync server stopped with an error: {}", error);
        }
    });

    let import_app = app.clone();
    let import_pool = app_state.db_manager.pool().clone();
    tauri::async_runtime::spawn(run_import_worker(
        import_app,
        import_pool.clone(),
        import_receiver,
        import_sender.clone(),
        cancellation.clone(),
    ));

    let recovered = DeviceSyncRepository::recover_interrupted_imports(&import_pool)
        .await
        .map_err(|error| format!("Failed to recover interrupted phone imports: {error}"))?;
    if recovered > 0 {
        log::info!("Requeued {} interrupted phone import(s)", recovered);
    }

    let pending = DeviceSyncRepository::list_captures(&import_pool)
        .await
        .map_err(|error| format!("Failed to recover queued phone imports: {error}"))?;
    for capture in pending {
        if capture.status == "received" {
            let _ = import_sender.send(capture.id);
        }
    }

    let session = materials.view;
    *runtime.running.lock().await = Some(RunningServer {
        handle,
        session: session.clone(),
        cancellation,
    });
    Ok(session)
}

#[tauri::command]
pub async fn stop_device_sync_session(
    runtime: State<'_, DeviceSyncRuntimeState>,
) -> Result<(), String> {
    runtime.shutdown().await;
    Ok(())
}

#[tauri::command]
pub async fn get_device_sync_status(
    app: AppHandle,
    runtime: State<'_, DeviceSyncRuntimeState>,
) -> Result<DeviceSyncStatus, String> {
    let app_state = app
        .try_state::<AppState>()
        .ok_or_else(|| "Briefli database is not ready".to_string())?;
    let running = runtime.running.lock().await;
    let paired_devices = DeviceSyncRepository::list_devices(app_state.db_manager.pool())
        .await
        .map_err(|error| format!("Failed to list paired phones: {error}"))?;
    let captures = DeviceSyncRepository::list_captures(app_state.db_manager.pool())
        .await
        .map_err(|error| format!("Failed to list phone captures: {error}"))?;

    Ok(DeviceSyncStatus {
        running: running.is_some(),
        session: running.as_ref().map(|server| server.session.clone()),
        paired_devices,
        captures,
    })
}

#[tauri::command]
pub async fn unpair_device_sync_phone(app: AppHandle, device_id: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&device_id).map_err(|_| "Invalid paired phone ID".to_string())?;
    let app_state = app
        .try_state::<AppState>()
        .ok_or_else(|| "Briefli database is not ready".to_string())?;

    DeviceSyncRepository::revoke_device(app_state.db_manager.pool(), &device_id)
        .await
        .map_err(|error| format!("Failed to unpair phone: {error}"))
}

fn device_sync_inbox_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("device-sync").join("inbox"))
        .map_err(|error| format!("Failed to resolve Briefli app data: {error}"))
}

fn select_lan_ipv4() -> Result<Ipv4Addr, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|error| format!("Failed to inspect network interfaces: {error}"))?;
    socket
        .connect((Ipv4Addr::new(192, 0, 2, 1), 9))
        .map_err(|error| format!("No active local network route was found: {error}"))?;
    match socket
        .local_addr()
        .map_err(|error| format!("Failed to inspect the local network route: {error}"))?
        .ip()
    {
        IpAddr::V4(address) if !address.is_loopback() && !address.is_unspecified() => Ok(address),
        _ => Err("No usable IPv4 address was found. Connect the phone and PC to the same Wi-Fi or phone hotspot.".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbox_path_is_not_user_controlled() {
        let base = PathBuf::from("C:/BriefliData");
        let result = base.join("device-sync").join("inbox");
        assert_eq!(result, PathBuf::from("C:/BriefliData/device-sync/inbox"));
    }
}

async fn run_import_worker(
    app: AppHandle,
    pool: sqlx::SqlitePool,
    mut receiver: UnboundedReceiver<String>,
    sender: UnboundedSender<String>,
    cancellation: CancellationToken,
) {
    loop {
        let capture_id = tokio::select! {
            _ = cancellation.cancelled() => break,
            capture_id = receiver.recv() => match capture_id {
                Some(capture_id) => capture_id,
                None => break,
            },
        };
        let capture = match DeviceSyncRepository::get_capture(&pool, &capture_id).await {
            Ok(Some(capture)) if capture.status == "received" => capture,
            Ok(_) => continue,
            Err(error) => {
                log::error!(
                    "Failed to load queued phone capture {}: {}",
                    capture_id,
                    error
                );
                continue;
            }
        };

        if crate::audio::import::is_import_in_progress() {
            let retry_sender = sender.clone();
            let retry_cancellation = cancellation.clone();
            tauri::async_runtime::spawn(async move {
                tokio::select! {
                    _ = retry_cancellation.cancelled() => {}
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {
                        let _ = retry_sender.send(capture_id);
                    }
                }
            });
            continue;
        }

        if let Err(error) = DeviceSyncRepository::mark_importing(&pool, &capture.id).await {
            log::error!(
                "Failed to claim phone capture {} for import: {}",
                capture.id,
                error
            );
            continue;
        }

        let (provider, model) = configured_transcription_model(&pool).await;
        let result = crate::audio::import::start_phone_sync_import(
            app.clone(),
            capture.id.clone(),
            capture.inbox_path.clone(),
            capture.title.clone(),
            None,
            model,
            provider,
        )
        .await;

        match result {
            Ok(imported) => {
                log::info!(
                    "Phone capture {} imported as {}",
                    capture.id,
                    imported.meeting_id
                );
                if let Err(error) = app.emit("phone-capture-imported", imported.meeting_id.clone())
                {
                    log::warn!(
                        "Phone capture {} imported but the meeting list refresh event failed: {}",
                        capture.id,
                        error
                    );
                }
            }
            Err(error) => {
                let message = error.to_string();
                if let Err(status_error) =
                    DeviceSyncRepository::mark_import_failed(&pool, &capture.id, &message).await
                {
                    log::error!(
                        "Phone capture {} import failed and status update also failed: {}",
                        capture.id,
                        status_error
                    );
                }
            }
        }
    }
}

async fn configured_transcription_model(
    pool: &sqlx::SqlitePool,
) -> (Option<String>, Option<String>) {
    let configured = sqlx::query_as::<_, (String, String)>(
        "SELECT provider, model FROM transcript_settings WHERE id = '1'",
    )
    .fetch_optional(pool)
    .await;

    match configured {
        Ok(Some((provider, model))) => {
            let import_provider = if provider == "parakeet" {
                "parakeet"
            } else {
                "whisper"
            };
            (Some(import_provider.to_string()), Some(model))
        }
        Ok(None) => (None, None),
        Err(error) => {
            log::warn!(
                "Failed to read transcription config for phone import: {}",
                error
            );
            (None, None)
        }
    }
}
