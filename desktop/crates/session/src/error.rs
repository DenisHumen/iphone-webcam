use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("transport: {0}")]
    Transport(#[from] transport::TransportError),
    #[error("handshake timeout")]
    HandshakeTimeout,
    #[error("unauthorized")]
    Unauthorized,
    #[error("incompatible protocol version: peer={peer}, local={local}")]
    IncompatibleProtoVer { peer: u32, local: u32 },
    #[error("unexpected message: {0}")]
    UnexpectedMessage(&'static str),
    #[error("media socket presented unknown sessionId/token")]
    UnknownMediaBinding,
    #[error("media handshake timeout")]
    MediaHandshakeTimeout,
    #[error("session closed: {0}")]
    Closed(String),
}
