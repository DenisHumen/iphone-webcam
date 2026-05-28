//! `adaptive` — bitrate tables + mode selection + speedtest + adaptation loop.

#![forbid(unsafe_code)]

pub mod tables;
pub mod types;

pub use tables::{h264_target_kbps, hevc_target_kbps, raw_bitrate_kbps};
pub use types::{AvailableMode, Measurement, TransportClass, UserLimits};
