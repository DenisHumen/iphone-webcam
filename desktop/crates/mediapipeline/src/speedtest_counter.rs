//! Speedtest filler frames arrive on the media socket with `seq` in a reserved
//! high range so the pipeline can divert them away from the video sinks and
//! into a byte/step counter the speedtest runner reads.

use std::sync::Arc;

use tokio::sync::Mutex;

/// Media frames whose `seq` is >= this base are speedtest filler, not video.
/// The lower 16 bits encode the ramp step index.
pub const SPEEDTEST_SEQ_BASE: u32 = 0xFFFF_0000;

#[must_use]
pub fn is_speedtest_marker(seq: u32) -> bool {
    seq >= SPEEDTEST_SEQ_BASE
}

#[must_use]
pub fn speedtest_step_index(seq: u32) -> u32 {
    seq - SPEEDTEST_SEQ_BASE
}

/// One observed speedtest filler frame: which step, how many payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedtestArrival {
    pub step_index: u32,
    pub payload_bytes: u64,
}

/// Shared accumulator the pipeline writes to and the runner reads from.
/// Cheap to clone (`Arc`-backed).
#[derive(Clone, Default)]
pub struct SpeedTestCounter {
    inner: Arc<Mutex<Vec<SpeedtestArrival>>>,
}

impl SpeedTestCounter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn record(&self, arrival: SpeedtestArrival) {
        self.inner.lock().await.push(arrival);
    }

    /// Total payload bytes recorded for `step_index`.
    pub async fn bytes_for_step(&self, step_index: u32) -> u64 {
        self.inner
            .lock()
            .await
            .iter()
            .filter(|a| a.step_index == step_index)
            .map(|a| a.payload_bytes)
            .sum()
    }

    /// Snapshot of all arrivals (for the runner to aggregate).
    pub async fn drain(&self) -> Vec<SpeedtestArrival> {
        std::mem::take(&mut *self.inner.lock().await)
    }
}
