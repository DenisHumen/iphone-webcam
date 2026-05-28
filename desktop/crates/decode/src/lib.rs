//! `decode` — Decoder trait + a passthrough/stub impl.
//!
//! Production decoders (VideoToolbox on macOS, ffmpeg cross-platform) land in
//! Phase 6 polish behind the same trait. Phase 4 uses `PassthroughDecoder` to
//! exercise the encoded routing path in CI without dragging in native deps.

#![forbid(unsafe_code)]

use async_trait::async_trait;
use bytes::Bytes;
use ccp_protocol::Codec;
use sink::Frame;

pub mod passthrough;

pub use passthrough::PassthroughDecoder;

#[derive(Debug, Clone)]
pub struct FrameHint {
    pub width: u16,
    pub height: u16,
    pub pts_usec: u64,
    pub seq: u32,
    pub keyframe: bool,
    pub config: bool,
    pub full_range: bool,
}

#[async_trait]
pub trait Decoder: Send + Sync + 'static {
    /// Decode an access unit. Returns `None` if the AU was a config-only frame
    /// (VPS/SPS/PPS) or if more input is required.
    async fn decode(&self, codec: Codec, payload: Bytes, hint: FrameHint) -> Option<Frame>;
}
