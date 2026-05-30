#[allow(dead_code)]
pub const TRANSPORT_CHANGED: &str = "session://transport_changed";
#[allow(dead_code)]
pub const USB_TRUST_REQUEST: &str = "session://usb_trust_request";

use session::ControlPlaneEvent;
use tauri::Emitter;
use tracing::warn;

pub async fn pump(app: tauri::AppHandle, state: super::commands::AppState) {
    loop {
        let events = {
            let guard = state.lock().await;
            guard.as_ref().map(|h| h.events.clone())
        };
        let Some(events) = events else {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            continue;
        };
        let mut rx = events.lock().await;
        match rx.recv().await {
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Some(ev) => match ev {
                ControlPlaneEvent::StateChanged(kind) => {
                    if let Err(e) = app.emit("session://state", &kind) {
                        warn!(error = ?e, "emit session://state failed");
                    }
                }
                ControlPlaneEvent::DeviceInfo(d) => {
                    if let Err(e) = app.emit("session://device", &d) {
                        warn!(error = ?e, "emit session://device failed");
                    }
                }
                ControlPlaneEvent::Telemetry(t) => {
                    if let Err(e) = app.emit("session://telemetry", &t) {
                        warn!(error = ?e, "emit session://telemetry failed");
                    }
                }
                ControlPlaneEvent::Closed(reason) => {
                    if let Err(e) = app.emit("session://closed", &reason) {
                        warn!(error = ?e, "emit session://closed failed");
                    }
                }
                ControlPlaneEvent::UsbTrustRequest { udid } => {
                    if let Err(e) = app.emit(USB_TRUST_REQUEST, &udid) {
                        warn!(error = ?e, "emit session://usb_trust_request failed");
                    }
                }
            },
        }
    }
}
