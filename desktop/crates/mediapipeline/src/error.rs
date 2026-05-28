use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaPipelineError {
    #[error("transport: {0}")]
    Transport(#[from] transport::TransportError),
    #[error("media header: {0}")]
    Header(#[from] ccp_protocol::MediaDecodeError),
    #[error("payload too large: {0} bytes")]
    PayloadTooLarge(u32),
    #[error("unsupported codec: {0:?}")]
    UnsupportedCodec(ccp_protocol::Codec),
    #[error("plane size mismatch: got {got}, expected {expected}")]
    PlaneSizeMismatch { got: usize, expected: usize },
}
