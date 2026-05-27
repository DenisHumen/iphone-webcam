use std::sync::Arc;

use app::{AppCore, AppHandle};
use serde::Serialize;
use session::SessionSnapshot;
use tauri::State;
use tokio::sync::Mutex;

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
pub async fn start_server(state: State<'_, AppState>) -> Result<QrPayloadOut, String> {
    let mut guard = state.lock().await;
    if guard.is_some() {
        return Err("server already running".into());
    }
    let host = local_host().unwrap_or_else(|| "127.0.0.1".into());
    let handle = AppCore { host: host.clone() }
        .start()
        .await
        .map_err(|e| format!("{e:?}"))?;
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

fn local_host() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}
