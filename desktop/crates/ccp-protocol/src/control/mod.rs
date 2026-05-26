//! Control plane: length-prefixed JSON messages.
//!
//! Wire format: `[uint32 BE length][UTF-8 JSON payload]` per `docs/03-protocol.md` §5.1.
//!
//! Each JSON object has a `t` (type) discriminator, a `seq` counter, and an optional
//! `ack` referencing the request `seq`.

pub mod envelope;
pub mod handshake;

use serde::{Deserialize, Serialize};

pub use envelope::{frame, try_unframe, FrameError, DEFAULT_MAX_PAYLOAD, PREFIX_LEN};
pub use handshake::{
    Auth, AuthOk, Bye, DeviceIdent, ErrorCode, ErrorMsg, Hello, HelloAck, MediaHello,
};

/// Wire-level wrapper carrying `seq`, optional `ack`, and a typed body.
///
/// JSON shape (flattened): `{ "seq": .., "ack": .., "t": "...", ... body fields ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlEnvelope {
    pub seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<u64>,
    #[serde(flatten)]
    pub body: ControlMessage,
}

/// Internally-tagged discriminated union over all CCP control messages.
///
/// Tag values mirror the wire strings in `docs/03-protocol.md` §6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum ControlMessage {
    #[serde(rename = "HELLO")]
    Hello(Hello),
    #[serde(rename = "HELLO_ACK")]
    HelloAck(HelloAck),
    #[serde(rename = "AUTH")]
    Auth(Auth),
    #[serde(rename = "AUTH_OK")]
    AuthOk(AuthOk),
    #[serde(rename = "MEDIA_HELLO")]
    MediaHello(MediaHello),
    #[serde(rename = "BYE")]
    Bye(Bye),
    #[serde(rename = "ERROR")]
    Error(ErrorMsg),
    // Additional variants added in Tasks 6–7.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Capability;

    #[test]
    fn envelope_hello_round_trip_and_shape() {
        let env = ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: 1,
                app: "ClearCam-iOS/0.1.0".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "iOS 18.0".into(),
                },
                session_id: "sess-abc".into(),
                caps: vec![Capability::Hevc],
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["seq"], 1);
        assert_eq!(v["t"], "HELLO");
        assert_eq!(v["protoVer"], 1);
        assert!(v.get("ack").is_none());
        let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn envelope_error_includes_ack_when_set() {
        let env = ControlEnvelope {
            seq: 100,
            ack: Some(99),
            body: ControlMessage::Error(ErrorMsg {
                code: ErrorCode::BadRequest,
                message: "oops".into(),
                ack: None,
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["ack"], 99);
        assert_eq!(v["t"], "ERROR");
        assert_eq!(v["code"], "bad_request");
    }
}
