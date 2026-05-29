//! Control plane: length-prefixed JSON messages.
//!
//! Wire format: `[uint32 BE length][UTF-8 JSON payload]` per `docs/03-protocol.md` §5.1.
//!
//! Each JSON object has a `t` (type) discriminator, a `seq` counter, and an optional
//! `ack` referencing the request `seq`.

pub mod device;
pub mod envelope;
pub mod handshake;
pub mod speedtest;
pub mod streaming;
pub mod telemetry;

use serde::{Deserialize, Serialize};

pub use device::{
    BatteryState, CameraEntry, CameraList, CameraPosition, CameraState, DeviceInfo, SetCamera,
    ThermalState,
};
pub use envelope::{frame, try_unframe, FrameError, DEFAULT_MAX_PAYLOAD, PREFIX_LEN};
pub use handshake::{
    Auth, AuthOk, Bye, DeviceIdent, ErrorCode, ErrorMsg, Hello, HelloAck, MediaHello,
};
pub use speedtest::{SpeedtestPattern, SpeedtestResult, SpeedtestStart, SpeedtestTick};
pub use streaming::{CodecName, FormatKind, Mode, ModeApplied, PixelFormat, SetMode, Start, Stop};
pub use telemetry::{Ping, Pong, Telemetry};

/// Wire-level wrapper carrying `seq`, optional `ack`, and a typed body.
///
/// JSON shape (flattened): `{ "seq": .., "ack": .., "t": "...", ... body fields ... }`.
///
/// Note: cannot derive `Eq` because some variants contain `f64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
///
/// Note: cannot derive `Eq` because `Telemetry`/`SpeedtestResult` contain `f64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(rename = "DEVICE_INFO")]
    DeviceInfo(DeviceInfo),
    #[serde(rename = "CAMERA_LIST")]
    CameraList(CameraList),
    #[serde(rename = "SET_CAMERA")]
    SetCamera(SetCamera),
    #[serde(rename = "CAMERA_STATE")]
    CameraState(CameraState),
    #[serde(rename = "START")]
    Start(Start),
    #[serde(rename = "STOP")]
    Stop(Stop),
    #[serde(rename = "SET_MODE")]
    SetMode(SetMode),
    #[serde(rename = "MODE_APPLIED")]
    ModeApplied(ModeApplied),
    #[serde(rename = "TELEMETRY")]
    Telemetry(Telemetry),
    #[serde(rename = "PING")]
    Ping(Ping),
    #[serde(rename = "PONG")]
    Pong(Pong),
    #[serde(rename = "SPEEDTEST_START")]
    SpeedtestStart(SpeedtestStart),
    #[serde(rename = "SPEEDTEST_TICK")]
    SpeedtestTick(SpeedtestTick),
    #[serde(rename = "SPEEDTEST_RESULT")]
    SpeedtestResult(SpeedtestResult),
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
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["ack"], 99);
        assert_eq!(v["t"], "ERROR");
        assert_eq!(v["code"], "bad_request");
    }

    #[test]
    fn envelope_set_camera_round_trip() {
        let env = ControlEnvelope {
            seq: 42,
            ack: None,
            body: ControlMessage::SetCamera(SetCamera {
                camera_id: "wide".into(),
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["t"], "SET_CAMERA");
        assert_eq!(v["cameraId"], "wide");
        let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn envelope_telemetry_round_trip() {
        let env = ControlEnvelope {
            seq: 7,
            ack: None,
            body: ControlMessage::Telemetry(Telemetry {
                ts_usec: 1,
                battery_level: 1.0,
                battery_state: BatteryState::Full,
                thermal_state: ThermalState::Nominal,
                sent_bitrate_kbps: 0,
                enc_fps: 0,
                capture_fps: 0,
                queue_depth: 0,
                drop_count: 0,
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn all_variants_round_trip_through_envelope() {
        let mode = Mode {
            format: FormatKind::Raw,
            codec: CodecName::None,
            width: 1280,
            height: 720,
            fps: 30,
            bitrate_kbps: 0,
            pixel_format: PixelFormat::Nv12,
            full_range: false,
        };

        let cases: Vec<ControlMessage> = vec![
            ControlMessage::Hello(Hello {
                proto_ver: 1,
                app: "x".into(),
                device: DeviceIdent {
                    model: "m".into(),
                    os_ver: "v".into(),
                },
                session_id: "s".into(),
                caps: vec![Capability::Hevc],
            }),
            ControlMessage::HelloAck(HelloAck {
                proto_ver: 1,
                caps: vec![],
            }),
            ControlMessage::Auth(Auth::token("t")),
            ControlMessage::AuthOk(AuthOk {
                session_id: "s".into(),
            }),
            ControlMessage::MediaHello(MediaHello {
                session_id: "s".into(),
                token: "t".into(),
            }),
            ControlMessage::Bye(Bye {
                reason: "done".into(),
            }),
            ControlMessage::Error(ErrorMsg {
                code: ErrorCode::Timeout,
                message: "x".into(),
            }),
            ControlMessage::DeviceInfo(DeviceInfo {
                model: "m".into(),
                os_ver: "v".into(),
                battery_level: 0.5,
                battery_state: BatteryState::Charging,
                thermal_state: ThermalState::Nominal,
                usb3_capable: false,
            }),
            ControlMessage::CameraList(CameraList { cameras: vec![] }),
            ControlMessage::SetCamera(SetCamera {
                camera_id: "wide".into(),
            }),
            ControlMessage::CameraState(CameraState {
                active_camera_id: "wide".into(),
                applied_format: "1080p30 hevc".into(),
            }),
            ControlMessage::Start(Start { mode: mode.clone() }),
            ControlMessage::Stop(Stop {}),
            ControlMessage::SetMode(SetMode { mode: mode.clone() }),
            ControlMessage::ModeApplied(ModeApplied {
                mode: mode.clone(),
                at_seq: 0,
            }),
            ControlMessage::Telemetry(Telemetry {
                ts_usec: 0,
                battery_level: 0.0,
                battery_state: BatteryState::Unknown,
                thermal_state: ThermalState::Nominal,
                sent_bitrate_kbps: 0,
                enc_fps: 0,
                capture_fps: 0,
                queue_depth: 0,
                drop_count: 0,
            }),
            ControlMessage::Ping(Ping { ts_usec: 0 }),
            ControlMessage::Pong(Pong {
                ts_usec: 0,
                echo_usec: 0,
            }),
            ControlMessage::SpeedtestStart(SpeedtestStart {
                id: "x".into(),
                target_bitrate_kbps: 0,
                duration_ms: 0,
                pattern: SpeedtestPattern::Ramp,
            }),
            ControlMessage::SpeedtestTick(SpeedtestTick {
                id: "x".into(),
                tick_index: 0,
                ts_usec: 0,
            }),
            ControlMessage::SpeedtestResult(SpeedtestResult {
                id: "x".into(),
                goodput_mbps: 0.0,
                rtt_ms: 0.0,
                jitter_ms: 0.0,
                loss_pct: 0.0,
                recommended_mode: mode,
            }),
        ];

        for body in cases {
            let env = ControlEnvelope {
                seq: 1,
                ack: None,
                body: body.clone(),
            };
            let s = serde_json::to_string(&env).unwrap();
            let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
            assert_eq!(back, env, "round-trip failed for {body:?}");
        }
    }
}
