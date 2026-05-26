//! Speedtest messages (§6.5).

use serde::{Deserialize, Serialize};

use crate::control::streaming::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestPattern {
    /// Step pattern: short increasing bitrate ramps.
    Ramp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestStart {
    pub id: String,
    pub target_bitrate_kbps: u32,
    pub duration_ms: u32,
    pub pattern: SpeedtestPattern,
}

/// Progress notification for an in-flight speedtest run.
///
/// Note: the inner counter is named `tickIndex` (not `seq`) to avoid clashing with
/// the envelope's `seq` when flattened. `id` matches the `id` from [`SpeedtestStart`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestTick {
    pub id: String,
    pub tick_index: u32,
    pub ts_usec: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestResult {
    pub id: String,
    pub goodput_mbps: f64,
    pub rtt_ms: f64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub recommended_mode: Mode,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::streaming::{CodecName, FormatKind, PixelFormat};

    #[test]
    fn speedtest_start_round_trip() {
        let s = SpeedtestStart {
            id: "st-1".into(),
            target_bitrate_kbps: 800_000,
            duration_ms: 3000,
            pattern: SpeedtestPattern::Ramp,
        };
        let j = serde_json::to_string(&s).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["targetBitrateKbps"], 800_000);
        assert_eq!(v["pattern"], "ramp");
        assert_eq!(serde_json::from_str::<SpeedtestStart>(&j).unwrap(), s);
    }

    #[test]
    fn speedtest_result_round_trip() {
        let r = SpeedtestResult {
            id: "st-1".into(),
            goodput_mbps: 412.5,
            rtt_ms: 8.2,
            jitter_ms: 1.3,
            loss_pct: 0.0,
            recommended_mode: Mode {
                format: FormatKind::Encoded,
                codec: CodecName::Hevc,
                width: 1920,
                height: 1080,
                fps: 30,
                bitrate_kbps: 30_000,
                pixel_format: PixelFormat::Nv12,
                full_range: true,
            },
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: SpeedtestResult = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
    }
}
