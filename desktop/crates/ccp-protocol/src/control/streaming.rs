//! Streaming control & mode (§6.3 + §7).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatKind {
    Raw,
    Encoded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecName {
    None,
    Hevc,
    H264,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    Nv12,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    pub format: FormatKind,
    pub codec: CodecName,
    pub width: u16,
    pub height: u16,
    pub fps: u16,
    /// For `Encoded`; ignored for `Raw`.
    pub bitrate_kbps: u32,
    pub pixel_format: PixelFormat,
    pub full_range: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    pub mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stop {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMode {
    pub mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeApplied {
    pub mode: Mode,
    /// First media `seq` produced under the new mode.
    pub at_seq: u32,
}

impl Mode {
    /// Default streaming mode used as the adaptation starting point:
    /// encoded HEVC 1080p30 @ 30 Mbps, NV12, full-range.
    pub fn default_streaming_1080p30() -> Self {
        Self {
            format: FormatKind::Encoded,
            codec: CodecName::Hevc,
            width: 1920,
            height: 1080,
            fps: 30,
            bitrate_kbps: 30_000,
            pixel_format: PixelFormat::Nv12,
            full_range: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mode() -> Mode {
        Mode {
            format: FormatKind::Encoded,
            codec: CodecName::Hevc,
            width: 1920,
            height: 1080,
            fps: 30,
            bitrate_kbps: 30_000,
            pixel_format: PixelFormat::Nv12,
            full_range: true,
        }
    }

    #[test]
    fn default_streaming_mode_is_hevc_1080p30() {
        let m = Mode::default_streaming_1080p30();
        assert_eq!((m.width, m.height, m.fps), (1920, 1080, 30));
        assert_eq!(m.codec, CodecName::Hevc);
    }

    #[test]
    fn mode_round_trip() {
        let m = sample_mode();
        let s = serde_json::to_string(&m).unwrap();
        let back: Mode = serde_json::from_str(&s).unwrap();
        assert_eq!(back, m);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["format"], "encoded");
        assert_eq!(v["codec"], "hevc");
        assert_eq!(v["bitrateKbps"], 30_000);
        assert_eq!(v["pixelFormat"], "nv12");
        assert_eq!(v["fullRange"], true);
    }

    #[test]
    fn mode_applied_round_trip() {
        let ma = ModeApplied {
            mode: sample_mode(),
            at_seq: 4242,
        };
        let s = serde_json::to_string(&ma).unwrap();
        let back: ModeApplied = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ma);
    }
}
