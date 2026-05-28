//! Live adaptation loop per docs/06 §5.
//!
//! Input each tick: telemetry sample. Output: `Action` — either nothing, a
//! step-up, or a step-down to a new `Mode`. The state machine asymmetric:
//! step-down is aggressive (a single congestion sample is enough), step-up is
//! cautious (4 consecutive stable samples + 5 s cooldown).

use std::time::{Duration, Instant};

use ccp_protocol::{CodecName, FormatKind, Mode, Telemetry};
use serde::{Deserialize, Serialize};

use crate::tables::{h264_target_kbps, hevc_target_kbps};

const STABLE_SAMPLES_BEFORE_STEPUP: u32 = 4;
const RTT_CONGESTION_FACTOR: f64 = 2.0;
const STEPUP_COOLDOWN: Duration = Duration::from_secs(5);
const STEPDOWN_COOLDOWN: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    None,
    StepUp(Mode),
    StepDown(Mode),
}

#[derive(Debug)]
pub struct AdaptationState {
    pub current: Mode,
    consecutive_stable: u32,
    base_rtt_ms: f64,
    last_drop_count: u64,
    last_action_at: Option<Instant>,
    start_at: Instant,
}

impl AdaptationState {
    pub fn new(initial: Mode) -> Self {
        Self::new_at(initial, Instant::now())
    }

    pub fn new_at(initial: Mode, start_at: Instant) -> Self {
        Self {
            current: initial,
            consecutive_stable: 0,
            base_rtt_ms: 0.0,
            last_drop_count: 0,
            last_action_at: None,
            start_at,
        }
    }

    /// Feed a telemetry sample + the latest measured RTT from PING/PONG.
    pub fn observe(&mut self, telemetry: &Telemetry, rtt_ms: f64, now: Instant) -> Action {
        let congested = self.is_congested(telemetry, rtt_ms);
        if congested {
            if !self.cooldown_elapsed(now, STEPDOWN_COOLDOWN) {
                return Action::None;
            }
            self.consecutive_stable = 0;
            self.last_drop_count = telemetry.drop_count;
            if let Some(next) = self.next_lower() {
                self.last_action_at = Some(now);
                self.current = next.clone();
                return Action::StepDown(next);
            }
            return Action::None;
        }
        let alpha = 0.2;
        if self.base_rtt_ms == 0.0 {
            self.base_rtt_ms = rtt_ms;
        } else {
            self.base_rtt_ms = (1.0 - alpha) * self.base_rtt_ms + alpha * rtt_ms;
        }
        self.last_drop_count = telemetry.drop_count;
        self.consecutive_stable = self.consecutive_stable.saturating_add(1);
        if self.consecutive_stable >= STABLE_SAMPLES_BEFORE_STEPUP
            && self.cooldown_elapsed(now, STEPUP_COOLDOWN)
        {
            if let Some(next) = self.next_higher() {
                self.consecutive_stable = 0;
                self.last_action_at = Some(now);
                self.current = next.clone();
                return Action::StepUp(next);
            }
        }
        Action::None
    }

    fn cooldown_elapsed(&self, now: Instant, dur: Duration) -> bool {
        let reference = self.last_action_at.unwrap_or(self.start_at);
        now.duration_since(reference) >= dur
    }

    fn is_congested(&self, telemetry: &Telemetry, rtt_ms: f64) -> bool {
        if telemetry.queue_depth > 0 {
            return true;
        }
        if telemetry.drop_count > self.last_drop_count {
            return true;
        }
        if self.base_rtt_ms > 0.0 && rtt_ms > self.base_rtt_ms * RTT_CONGESTION_FACTOR {
            return true;
        }
        false
    }

    /// Step-down ladder: 1) drop fps to 30 (if higher); 2) reduce encoded
    /// bitrate by 25%; 3) raw → encoded; 4) drop resolution to next-lower
    /// canonical step.
    fn next_lower(&self) -> Option<Mode> {
        let mut next = self.current.clone();
        if next.fps > 30 {
            next.fps = 30;
            if next.format == FormatKind::Encoded {
                next.bitrate_kbps = encoded_target(&next);
            }
            return Some(next);
        }
        if next.format == FormatKind::Encoded && next.bitrate_kbps > 1_000 {
            next.bitrate_kbps = next.bitrate_kbps * 3 / 4;
            return Some(next);
        }
        if next.format == FormatKind::Raw {
            next.format = FormatKind::Encoded;
            next.codec = CodecName::Hevc;
            next.bitrate_kbps = encoded_target(&next);
            return Some(next);
        }
        let (new_w, new_h) = match (next.width, next.height) {
            (3840, 2160) => (1920, 1080),
            (1920, 1080) => (1280, 720),
            (1280, 720) => return None,
            _ => return None,
        };
        next.width = new_w;
        next.height = new_h;
        if next.format == FormatKind::Encoded {
            next.bitrate_kbps = encoded_target(&next);
        }
        Some(next)
    }

    /// Step-up ladder: raise fps if 30, else raise res.
    fn next_higher(&self) -> Option<Mode> {
        let mut next = self.current.clone();
        if next.fps == 30 {
            next.fps = 60;
            if next.format == FormatKind::Encoded {
                next.bitrate_kbps = encoded_target(&next);
            }
            return Some(next);
        }
        let (new_w, new_h) = match (next.width, next.height) {
            (1280, 720) => (1920, 1080),
            (1920, 1080) => (3840, 2160),
            _ => return None,
        };
        next.width = new_w;
        next.height = new_h;
        if next.format == FormatKind::Encoded {
            next.bitrate_kbps = encoded_target(&next);
        }
        Some(next)
    }
}

fn encoded_target(m: &Mode) -> u32 {
    let target = match m.codec {
        CodecName::Hevc | CodecName::None => {
            hevc_target_kbps(u32::from(m.width), u32::from(m.height), u32::from(m.fps))
        }
        CodecName::H264 => {
            h264_target_kbps(u32::from(m.width), u32::from(m.height), u32::from(m.fps))
        }
    };
    u32::try_from(target).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::{BatteryState, PixelFormat, ThermalState};

    fn t(queue: u32, drops: u64) -> Telemetry {
        Telemetry {
            ts_usec: 0,
            battery_level: 1.0,
            battery_state: BatteryState::Unplugged,
            thermal_state: ThermalState::Nominal,
            sent_bitrate_kbps: 0,
            enc_fps: 0,
            capture_fps: 0,
            queue_depth: queue,
            drop_count: drops,
        }
    }

    fn mode_1080p60() -> Mode {
        Mode {
            format: FormatKind::Encoded,
            codec: CodecName::Hevc,
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate_kbps: 50_000,
            pixel_format: PixelFormat::Nv12,
            full_range: true,
        }
    }

    #[test]
    fn queue_depth_triggers_step_down_to_30fps() {
        let start = Instant::now();
        let mut s = AdaptationState::new_at(mode_1080p60(), start - Duration::from_secs(10));
        let action = s.observe(&t(2, 0), 10.0, start);
        match action {
            Action::StepDown(m) => {
                assert_eq!(m.fps, 30);
                assert_eq!((m.width, m.height), (1920, 1080));
            }
            other => panic!("expected StepDown, got {other:?}"),
        }
    }

    #[test]
    fn no_action_during_cooldown() {
        let start = Instant::now();
        let mut s = AdaptationState::new_at(mode_1080p60(), start - Duration::from_secs(10));
        let first = s.observe(&t(1, 0), 10.0, start);
        assert!(matches!(first, Action::StepDown(_)));
        // Even with congestion, no further step in cooldown.
        let second = s.observe(&t(5, 5), 50.0, start + Duration::from_millis(100));
        assert_eq!(second, Action::None);
    }

    #[test]
    fn stable_run_then_step_up() {
        let start = Instant::now();
        let mut s = AdaptationState::new_at(
            Mode {
                fps: 30,
                ..mode_1080p60()
            },
            start,
        );
        for i in 0..4u64 {
            let a = s.observe(&t(0, 0), 5.0, start + Duration::from_millis(100 * i));
            assert_eq!(a, Action::None, "tick {i} should not step up yet");
        }
        let action = s.observe(&t(0, 0), 5.0, start + Duration::from_secs(6));
        match action {
            Action::StepUp(m) => {
                assert_eq!(m.fps, 60);
            }
            other => panic!("expected StepUp, got {other:?}"),
        }
    }

    #[test]
    fn raw_steps_down_to_encoded_first() {
        let raw = Mode {
            format: FormatKind::Raw,
            codec: CodecName::None,
            width: 1280,
            height: 720,
            fps: 30,
            bitrate_kbps: 0,
            pixel_format: PixelFormat::Nv12,
            full_range: true,
        };
        let start = Instant::now();
        let mut s = AdaptationState::new_at(raw, start - Duration::from_secs(10));
        let action = s.observe(&t(2, 0), 10.0, start);
        match action {
            Action::StepDown(m) => {
                assert_eq!(m.format, FormatKind::Encoded);
                assert_eq!(m.codec, CodecName::Hevc);
                assert_eq!((m.width, m.height, m.fps), (1280, 720, 30));
            }
            other => panic!("expected StepDown, got {other:?}"),
        }
    }
}
