//! Handshake & error messages (§6.1).

use serde::{Deserialize, Serialize};

use crate::version::Capability;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdent {
    pub model: String,
    pub os_ver: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub proto_ver: u32,
    pub app: String,
    pub device: DeviceIdent,
    pub session_id: String,
    pub caps: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAck {
    pub proto_ver: u32,
    pub caps: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auth {
    /// Either a fresh QR token (Wi-Fi) or a stored pairing key (USB).
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthOk {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaHello {
    pub session_id: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bye {
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    IncompatibleVersion,
    Unauthorized,
    BadRequest,
    UnsupportedMode,
    CameraUnavailable,
    Internal,
    Timeout,
}

/// Error body. The `seq` of the failing request, when applicable, is carried by the
/// envelope's `ack` field — `ErrorMsg` itself does not duplicate it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMsg {
    pub code: ErrorCode,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_round_trip() {
        let hello = Hello {
            proto_ver: 1,
            app: "ClearCam-iOS/0.1.0".into(),
            device: DeviceIdent {
                model: "iPhone15,3".into(),
                os_ver: "iOS 18.0".into(),
            },
            session_id: "sess-abc".into(),
            caps: vec![Capability::Hevc, Capability::RawNv12],
        };
        let s = serde_json::to_string(&hello).unwrap();
        let back: Hello = serde_json::from_str(&s).unwrap();
        assert_eq!(back, hello);

        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["protoVer"], 1);
        assert_eq!(v["sessionId"], "sess-abc");
        assert_eq!(v["device"]["osVer"], "iOS 18.0");
    }

    #[test]
    fn error_msg_has_no_ack_field() {
        let e = ErrorMsg {
            code: ErrorCode::Unauthorized,
            message: "bad token".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        // ack belongs to the envelope; not in the body.
        assert!(!s.contains("\"ack\""));
    }

    #[test]
    fn error_code_snake_case() {
        let s = serde_json::to_string(&ErrorCode::IncompatibleVersion).unwrap();
        assert_eq!(s, "\"incompatible_version\"");
    }
}
