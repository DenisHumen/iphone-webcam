//! ClearCam Protocol (CCP) — pure data types and (de)serialization.
//!
//! Wire format spec: `docs/03-protocol.md`. No I/O, no async, no platform code.

#![forbid(unsafe_code)]

pub mod control;
pub mod media;
pub mod version;

pub use control::{
    Auth, AuthOk, BatteryState, Bye, CameraEntry, CameraList, CameraPosition, CameraState,
    CodecName, ControlEnvelope, ControlMessage, DeviceIdent, DeviceInfo, ErrorCode, ErrorMsg,
    FormatKind, Hello, HelloAck, MediaHello, Mode, ModeApplied, Ping, PixelFormat, Pong, SetCamera,
    SetMode, SpeedtestPattern, SpeedtestResult, SpeedtestStart, SpeedtestTick, Start, Stop,
    Telemetry, ThermalState, DEFAULT_MAX_PAYLOAD,
};
pub use media::{
    Codec, DecodeError as MediaDecodeError, Flags, MediaHeader, MediaType, HEADER_LEN, MAGIC,
};
pub use version::{Capability, PROTO_VER};
