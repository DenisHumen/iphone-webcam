use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
}
