//! `adaptive` — bitrate tables + mode selection + speedtest + adaptation loop.

#![forbid(unsafe_code)]

pub mod adaptation;
pub mod select_mode;
pub mod tables;
pub mod types;

pub use adaptation::{Action, AdaptationState};
pub use select_mode::select_mode;
pub use tables::{h264_target_kbps, hevc_target_kbps, raw_bitrate_kbps};
pub use types::{AvailableMode, Measurement, TransportClass, UserLimits};
