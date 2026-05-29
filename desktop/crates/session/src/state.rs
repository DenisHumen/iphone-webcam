use std::collections::HashMap;

use ccp_protocol::{CameraEntry, DeviceInfo, Telemetry};
use serde::{Deserialize, Serialize};

/// Transport discriminator — Wi-Fi vs USB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportTag {
    Wifi,
    Usb,
}

/// Coarse-grained UI-facing state for one session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionStateKind {
    Idle,
    Listening {
        control_port: u16,
        media_port: u16,
    },
    /// Wi-Fi inbound handshake (server-accepted; serialized as `kind: "handshaking"`
    /// for backward wire compat).
    #[serde(rename = "handshaking")]
    WifiHandshake,
    /// USB outbound handshake; carries the device UDID we dialed.
    #[serde(rename = "usb_handshake")]
    UsbHandshake {
        udid: String,
    },
    Ready,
    Reconnecting,
    Closed {
        reason: String,
    },
}

impl SessionStateKind {
    pub fn wifi_handshake() -> Self {
        Self::WifiHandshake
    }

    pub fn usb_handshake(udid: impl Into<String>) -> Self {
        Self::UsbHandshake { udid: udid.into() }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Listening { .. } => "listening",
            Self::WifiHandshake => "handshaking",
            Self::UsbHandshake { .. } => "usb_handshake",
            Self::Ready => "ready",
            Self::Reconnecting => "reconnecting",
            Self::Closed { .. } => "closed",
        }
    }

    pub fn udid(&self) -> Option<&str> {
        match self {
            Self::UsbHandshake { udid } => Some(udid.as_str()),
            _ => None,
        }
    }
}

/// Source-of-truth snapshot the UI mirrors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub state: SessionStateKind,
    pub device: Option<DeviceSnapshot>,
    pub last_telemetry: Option<Telemetry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSnapshot {
    pub model: String,
    pub os_ver: String,
    pub usb3_capable: bool,
    pub battery_level: f64,
    pub cameras: Vec<CameraEntry>,
}

impl SessionSnapshot {
    pub fn idle() -> Self {
        Self {
            state: SessionStateKind::Idle,
            device: None,
            last_telemetry: None,
        }
    }
}

impl DeviceSnapshot {
    pub fn from_device_info(info: &DeviceInfo) -> Self {
        Self {
            model: info.model.clone(),
            os_ver: info.os_ver.clone(),
            usb3_capable: info.usb3_capable,
            battery_level: info.battery_level,
            cameras: Vec::new(),
        }
    }
}

/// Pending pairing table — `(sessionId, token)` → media-stream delivery channel.
#[derive(Default)]
pub struct PendingMediaBindings(
    HashMap<(String, String), tokio::sync::oneshot::Sender<transport::MediaStream>>,
);

impl PendingMediaBindings {
    pub fn insert(
        &mut self,
        session_id: String,
        token: String,
        tx: tokio::sync::oneshot::Sender<transport::MediaStream>,
    ) {
        self.0.insert((session_id, token), tx);
    }

    pub fn take(
        &mut self,
        session_id: &str,
        token: &str,
    ) -> Option<tokio::sync::oneshot::Sender<transport::MediaStream>> {
        self.0.remove(&(session_id.to_string(), token.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_serde_round_trip() {
        let snap = SessionSnapshot {
            state: SessionStateKind::Listening {
                control_port: 7000,
                media_port: 7001,
            },
            device: Some(DeviceSnapshot {
                model: "iPhone15,3".into(),
                os_ver: "iOS 18.0".into(),
                usb3_capable: true,
                battery_level: 0.9,
                cameras: vec![],
            }),
            last_telemetry: None,
        };
        let s = serde_json::to_string(&snap).unwrap();
        let back: SessionSnapshot = serde_json::from_str(&s).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn wifi_handshake_state_has_no_udid_and_correct_label() {
        let s = SessionStateKind::wifi_handshake();
        assert_eq!(s.label(), "handshaking");
        assert!(s.udid().is_none());
    }

    #[test]
    fn usb_handshake_state_carries_udid_and_correct_label() {
        let s = SessionStateKind::usb_handshake("UDID-1");
        assert_eq!(s.label(), "usb_handshake");
        assert_eq!(s.udid(), Some("UDID-1"));
    }

    #[test]
    fn wifi_handshake_serializes_with_legacy_handshaking_tag() {
        let s = SessionStateKind::WifiHandshake;
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""kind":"handshaking""#));
    }

    #[test]
    fn legacy_handshaking_tag_still_deserializes_to_wifi_handshake() {
        let s: SessionStateKind = serde_json::from_str(r#"{"kind":"handshaking"}"#).unwrap();
        assert_eq!(s, SessionStateKind::WifiHandshake);
    }

    #[test]
    fn usb_handshake_serializes_with_udid_field() {
        let s = SessionStateKind::usb_handshake("ABC");
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""kind":"usb_handshake""#));
        assert!(json.contains(r#""udid":"ABC""#));
    }
}
