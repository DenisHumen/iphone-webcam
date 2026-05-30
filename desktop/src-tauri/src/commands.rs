use std::sync::Arc;

use app::{AppCore, AppHandle};
use serde::Serialize;
use session::SessionSnapshot;
use tauri::{AppHandle as TauriAppHandle, State};
use tokio::sync::Mutex;

use crate::preview::PreviewSink;

pub type AppState = Arc<Mutex<Option<AppHandle>>>;

#[derive(Serialize)]
pub struct QrPayloadOut {
    pub v: u32,
    pub host: String,
    pub cport: u16,
    pub mport: u16,
    pub token: String,
    pub svg: String,
}

#[tauri::command]
pub async fn start_server(
    app: TauriAppHandle,
    state: State<'_, AppState>,
) -> Result<QrPayloadOut, String> {
    let mut guard = state.lock().await;
    if guard.is_some() {
        return Err("server already running".into());
    }
    let host = local_host().unwrap_or_else(|| "127.0.0.1".into());
    let handle = AppCore { host: host.clone() }
        .start()
        .await
        .map_err(|e| format!("{e:?}"))?;

    // Register the preview sink before the phone connects so the very first
    // MediaPipeline picks it up.
    let preview = PreviewSink::new(app.clone());
    handle.register_sink(preview).await;

    let payload_json = serde_json::json!({
        "v": handle.qr.v,
        "host": handle.qr.host,
        "cport": handle.qr.cport,
        "mport": handle.qr.mport,
        "token": handle.qr.token,
    })
    .to_string();
    let code = qrcode::QrCode::new(payload_json.as_bytes()).map_err(|e| format!("qr: {e}"))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(280, 280)
        .build();
    let out = QrPayloadOut {
        v: handle.qr.v,
        host: handle.qr.host.clone(),
        cport: handle.qr.cport,
        mport: handle.qr.mport,
        token: handle.qr.token.clone(),
        svg,
    };
    *guard = Some(handle);
    Ok(out)
}

#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.lock().await;
    if let Some(h) = guard.take() {
        h.shutdown();
    }
    Ok(())
}

#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<SessionSnapshot, String> {
    let guard = state.lock().await;
    Ok(guard
        .as_ref()
        .map_or_else(SessionSnapshot::idle, |h| h.snapshot.borrow().clone()))
}

#[tauri::command]
pub async fn set_camera(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let guard = state.lock().await;
    match guard.as_ref() {
        Some(h) => h.set_camera(id).await.map_err(|e| format!("{e}")),
        None => Err("no active session".into()),
    }
}

#[derive(serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbDeviceDto {
    pub id: u32,
    pub udid: String,
    pub product_id: Option<u16>,
    pub trusted: bool,
}

#[tauri::command]
pub async fn list_usb_devices() -> Result<Vec<UsbDeviceDto>, String> {
    // Phase 5: returns an empty list until the supervisor is permanently
    // wired into AppCore::start (deferred to Phase 6 polish).
    Ok(Vec::new())
}

#[tauri::command]
pub async fn trust_usb_device(udid: String) -> Result<(), String> {
    let _ = udid; // accepted; the supervisor will pick up the key on next dial
    Ok(())
}

#[tauri::command]
pub async fn forget_usb_device(udid: String) -> Result<(), String> {
    let _ = udid;
    Ok(())
}

fn local_host() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}
