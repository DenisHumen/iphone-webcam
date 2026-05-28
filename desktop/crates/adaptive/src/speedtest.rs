//! Speedtest aggregator per docs/06 §3.
//!
//! Pure-Rust accumulator: the runner pushes per-step received-byte counters
//! over the media socket; `evaluate()` collapses them into a `Measurement`.

use std::time::Duration;

use ccp_protocol::SpeedtestPattern;
use serde::{Deserialize, Serialize};

use crate::tables::raw_bitrate_kbps;
use crate::types::Measurement;

/// Default ramp from docs/06 §3.2.
pub const DEFAULT_RAMP_KBPS: &[u64] = &[50_000, 100_000, 200_000, 400_000, 800_000];
pub const DEFAULT_STEP_MS: u64 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepObservation {
    pub target_kbps: u64,
    pub received_bytes: u64,
    pub duration: Duration,
    pub avg_rtt_ms: f64,
}

impl StepObservation {
    #[must_use]
    pub fn measured_kbps(&self) -> u64 {
        let secs = self.duration.as_secs_f64();
        if secs <= 0.0 {
            return 0;
        }
        ((self.received_bytes as f64) * 8.0 / 1000.0 / secs) as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedtestPlan {
    pub pattern: SpeedtestPattern,
    pub steps_kbps: Vec<u64>,
    pub step_duration: Duration,
}

impl SpeedtestPlan {
    #[must_use]
    pub fn default_ramp() -> Self {
        Self {
            pattern: SpeedtestPattern::Ramp,
            steps_kbps: DEFAULT_RAMP_KBPS.to_vec(),
            step_duration: Duration::from_millis(DEFAULT_STEP_MS),
        }
    }

    #[must_use]
    pub fn total_duration(&self) -> Duration {
        self.step_duration * u32::try_from(self.steps_kbps.len()).unwrap_or(u32::MAX)
    }
}

/// Reduce a series of step observations into a single `Measurement`.
///
/// The sustained goodput is the highest step whose measured throughput is at
/// least 85% of the target and whose RTT hasn't doubled from the baseline.
#[must_use]
pub fn evaluate(observations: &[StepObservation], baseline_rtt_ms: f64) -> Measurement {
    let mut sustained_kbps: u64 = 0;
    let mut max_rtt: f64 = baseline_rtt_ms;
    for obs in observations {
        let measured = obs.measured_kbps();
        let fits_throughput = measured as f64 >= obs.target_kbps as f64 * 0.85;
        let rtt_ok = baseline_rtt_ms <= 0.0 || obs.avg_rtt_ms <= baseline_rtt_ms * 2.0;
        if fits_throughput && rtt_ok {
            sustained_kbps = sustained_kbps.max(measured.min(obs.target_kbps));
        }
        max_rtt = max_rtt.max(obs.avg_rtt_ms);
    }
    Measurement {
        goodput_mbps: sustained_kbps as f64 / 1000.0,
        rtt_ms: max_rtt,
        jitter_ms: 0.0,
        loss_pct: 0.0,
    }
}

/// Helper: how many bytes per step would a perfect generator produce given the
/// ramp + the step duration?
#[must_use]
pub fn expected_step_bytes(target_kbps: u64, duration: Duration) -> u64 {
    (target_kbps * 1000 * duration.as_millis() as u64) / 8 / 1000
}

/// Helper: convert a media frame at `width×height×fps NV12` into a per-step
/// frame budget that hits `target_kbps`.
#[must_use]
pub fn frames_per_step_for_target(target_kbps: u64, width: u32, height: u32, step_ms: u64) -> u32 {
    let frame_bits = u64::from(width) * u64::from(height) * 12; // NV12 = 12 bpp
    if frame_bits == 0 {
        return 0;
    }
    // total bits over step = target_kbps * 1000 * step_s
    let total_bits = target_kbps * 1000 * step_ms / 1000;
    let _ = raw_bitrate_kbps; // keep import; explanatory only
    u32::try_from((total_bits / frame_bits).max(1)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_kbps_matches_bytes_over_time() {
        let obs = StepObservation {
            target_kbps: 100_000,
            received_bytes: 1_000_000, // 8 Mbit
            duration: Duration::from_millis(100),
            avg_rtt_ms: 5.0,
        };
        // 8 Mbit / 0.1 s = 80 Mbit/s = 80_000 kbps
        assert_eq!(obs.measured_kbps(), 80_000);
    }

    #[test]
    fn evaluate_returns_highest_sustained_step() {
        let baseline = 5.0;
        let steps = vec![
            StepObservation {
                target_kbps: 50_000,
                received_bytes: 50_000 * 1000 * 500 / 8 / 1000,
                duration: Duration::from_millis(500),
                avg_rtt_ms: 5.0,
            },
            StepObservation {
                target_kbps: 100_000,
                received_bytes: 100_000 * 1000 * 500 / 8 / 1000,
                duration: Duration::from_millis(500),
                avg_rtt_ms: 6.0,
            },
            // Third step: pushed 200 Mbps target, but RTT exploded → ignored.
            StepObservation {
                target_kbps: 200_000,
                received_bytes: 200_000 * 1000 * 500 / 8 / 1000,
                duration: Duration::from_millis(500),
                avg_rtt_ms: 50.0,
            },
        ];
        let m = evaluate(&steps, baseline);
        assert!((m.goodput_mbps - 100.0).abs() < 0.5);
        assert!(m.rtt_ms >= 50.0);
    }

    #[test]
    fn evaluate_with_zero_baseline_accepts_any_rtt() {
        let steps = vec![StepObservation {
            target_kbps: 50_000,
            received_bytes: expected_step_bytes(50_000, Duration::from_millis(500)),
            duration: Duration::from_millis(500),
            avg_rtt_ms: 100.0,
        }];
        let m = evaluate(&steps, 0.0);
        assert!((m.goodput_mbps - 50.0).abs() < 0.5);
    }

    #[test]
    fn expected_step_bytes_for_100mbps_500ms_is_6250000() {
        // 100 Mbit/s × 0.5 s = 50 Mbit = 6_250_000 bytes
        assert_eq!(
            expected_step_bytes(100_000, Duration::from_millis(500)),
            6_250_000
        );
    }

    #[test]
    fn frames_per_step_for_720p_100mbps() {
        // 720p NV12 frame: 1280×720×12 bits = 11_059_200 bits = ~1.38 MB.
        // 100 Mbps × 0.5s = 50 Mb. 50e6 / 11.06e6 ≈ 4.5 → 4 frames.
        let f = frames_per_step_for_target(100_000, 1280, 720, 500);
        assert_eq!(f, 4);
    }
}
