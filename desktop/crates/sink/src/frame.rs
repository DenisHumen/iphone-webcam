//! Canonical in-pipeline frame: NV12 planes + presentation timestamp.

use bytes::Bytes;
use ccp_protocol::Codec;

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub pts_usec: u64,
    pub seq: u32,
    pub codec: Codec,
    /// Y plane: `width*height` bytes (tightly packed, stride = width).
    pub plane_y: Bytes,
    /// Interleaved CbCr plane: `width*height/2` bytes (NV12). Empty for non-raw frames.
    pub plane_uv: Bytes,
    pub full_range: bool,
}

impl Frame {
    #[must_use]
    pub fn is_raw_nv12(&self) -> bool {
        matches!(self.codec, Codec::Raw) && !self.plane_y.is_empty() && !self.plane_uv.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_raw_nv12_checks_planes_and_codec() {
        let f = Frame {
            width: 1280,
            height: 720,
            pts_usec: 0,
            seq: 0,
            codec: Codec::Raw,
            plane_y: Bytes::from(vec![0u8; 1280 * 720]),
            plane_uv: Bytes::from(vec![0u8; 1280 * 720 / 2]),
            full_range: true,
        };
        assert!(f.is_raw_nv12());
    }

    #[test]
    fn encoded_frame_is_not_raw_nv12() {
        let f = Frame {
            width: 1280,
            height: 720,
            pts_usec: 0,
            seq: 0,
            codec: Codec::Hevc,
            plane_y: Bytes::new(),
            plane_uv: Bytes::new(),
            full_range: true,
        };
        assert!(!f.is_raw_nv12());
    }
}
