//! Glue between live `Telemetry` and the pure-Rust `adaptive::AdaptationState`
//! state machine. Owns one `AdaptationState` instance and emits `SET_MODE` on
//! the supplied outbound channel whenever the state decides to step.

use std::time::Instant;

use adaptive::{Action, AdaptationState};
use ccp_protocol::{ControlMessage, Mode, SetMode, Telemetry};
use tokio::sync::mpsc;
use tracing::info;

pub struct AdaptationDriver {
    state: AdaptationState,
    outbound: mpsc::Sender<ControlMessage>,
}

impl AdaptationDriver {
    pub fn new(outbound: mpsc::Sender<ControlMessage>, initial_mode: Mode) -> Self {
        Self {
            state: AdaptationState::new(initial_mode),
            outbound,
        }
    }

    /// Test-only constructor: starts in a sensible default streaming mode
    /// (encoded HEVC 1080p30), with `start_at` backdated by 10 s so that both
    /// step-up and step-down cooldowns are immediately satisfied.
    #[cfg(any(test, feature = "test-util"))]
    pub fn new_for_tests(outbound: mpsc::Sender<ControlMessage>) -> Self {
        let backdated = Instant::now()
            .checked_sub(std::time::Duration::from_secs(10))
            .unwrap_or_else(Instant::now);
        Self {
            state: AdaptationState::new_at(Mode::default_streaming_1080p30(), backdated),
            outbound,
        }
    }

    /// Feed one telemetry sample plus the current round-trip-time estimate
    /// (milliseconds, from `PING/PONG`; pass `0.0` if RTT unknown). Emits
    /// `SET_MODE` on the outbound channel whenever the state machine steps.
    pub async fn observe(&mut self, telemetry: &Telemetry, rtt_ms: f64) {
        let action = self.state.observe(telemetry, rtt_ms, Instant::now());
        match action {
            Action::None => {}
            Action::StepDown(new_mode) | Action::StepUp(new_mode) => {
                info!(?new_mode, "adaptation: emitting SET_MODE");
                let _ = self
                    .outbound
                    .send(ControlMessage::SetMode(SetMode { mode: new_mode }))
                    .await;
            }
        }
    }
}
