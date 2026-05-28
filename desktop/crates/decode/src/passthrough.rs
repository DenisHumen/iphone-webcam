//! Passthrough decoder for tests/CI.
//!
//! Synthesizes a `width×height` NV12 frame whose Y plane carries the seq
//! number as a byte pattern. This is enough to exercise the encoded → frame
//! routing path; it is NOT a real decoder.

use async_trait::async_trait;
use bytes::Bytes;
use ccp_protocol::Codec;
use sink::Frame;

use crate::{Decoder, FrameHint};

pub struct PassthroughDecoder;

#[async_trait]
impl Decoder for PassthroughDecoder {
    async fn decode(&self, _codec: Codec, _payload: Bytes, hint: FrameHint) -> Option<Frame> {
        if hint.config {
            return None;
        }
        let w = hint.width as usize;
        let h = hint.height as usize;
        let y_size = w * h;
        let uv_size = w * h / 2;
        let y_byte = u8::try_from(hint.seq % 256).unwrap_or(0);
        let plane_y = Bytes::from(vec![y_byte; y_size]);
        let plane_uv = Bytes::from(vec![128u8; uv_size]);
        Some(Frame {
            width: hint.width,
            height: hint.height,
            pts_usec: hint.pts_usec,
            seq: hint.seq,
            codec: Codec::Raw,
            plane_y,
            plane_uv,
            full_range: hint.full_range,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn passthrough_synthesizes_nv12_frame_from_hint() {
        let d = PassthroughDecoder;
        let hint = FrameHint {
            width: 16,
            height: 16,
            pts_usec: 100,
            seq: 7,
            keyframe: true,
            config: false,
            full_range: true,
        };
        let frame = d
            .decode(Codec::Hevc, Bytes::from(vec![0xAB; 32]), hint)
            .await
            .unwrap();
        assert_eq!(frame.width, 16);
        assert_eq!(frame.plane_y.len(), 256);
        assert_eq!(frame.plane_uv.len(), 128);
        assert_eq!(frame.seq, 7);
        assert_eq!(frame.plane_y[0], 7);
    }

    #[tokio::test]
    async fn config_frame_is_swallowed() {
        let d = PassthroughDecoder;
        let hint = FrameHint {
            width: 16,
            height: 16,
            pts_usec: 0,
            seq: 0,
            keyframe: false,
            config: true,
            full_range: true,
        };
        assert!(d
            .decode(Codec::Hevc, Bytes::from(vec![1, 2, 3]), hint)
            .await
            .is_none());
    }
}
