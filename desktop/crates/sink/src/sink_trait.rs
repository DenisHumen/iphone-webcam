use async_trait::async_trait;

use crate::frame::Frame;

/// Frame consumer. Implementations MUST be non-blocking on backpressure —
/// drop the frame if the consumer is behind, per docs/02 §8 (latency over completeness).
#[async_trait]
pub trait FrameSink: Send + Sync + 'static {
    async fn submit(&self, frame: Frame);
    async fn stop(&self) {}
}
