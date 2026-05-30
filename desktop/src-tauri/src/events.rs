pub const TRANSPORT_CHANGED: &str = "session://transport_changed";
pub const USB_TRUST_REQUEST: &str = "session://usb_trust_request";

use session::{ControlPlaneEvent, SessionStateKind, TransportTag};
use tauri::Emitter;
use tracing::warn;

/// Tags inferred from the current `SessionStateKind`. Handshake variants and
/// `Ready { transport }` all carry an explicit hint; `Reconnecting`/`Idle`/
/// `Closed` inherit whatever the last handshake selected.
fn transport_hint(kind: &SessionStateKind) -> Option<&'static str> {
    match kind {
        SessionStateKind::WifiHandshake => Some("wifi"),
        SessionStateKind::UsbHandshake { .. } => Some("usb"),
        SessionStateKind::Ready {
            transport: Some(TransportTag::Wifi),
        } => Some("wifi"),
        SessionStateKind::Ready {
            transport: Some(TransportTag::Usb),
        } => Some("usb"),
        _ => None,
    }
}

pub async fn pump(app: tauri::AppHandle, state: super::commands::AppState) {
    let mut last_transport: Option<&'static str> = None;
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
                    // Emit `session://transport_changed` whenever a new
                    // handshake variant tells us which transport is now
                    // active. Ready/Idle states reuse the prior hint.
                    if let Some(hint) = transport_hint(&kind) {
                        if Some(hint) != last_transport {
                            if let Err(e) = app.emit(TRANSPORT_CHANGED, &hint) {
                                warn!(error = ?e, "emit session://transport_changed failed");
                            }
                            last_transport = Some(hint);
                        }
                    }
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
