//! Typed wrappers around the raw TCP halves.
//!
//! - `ControlStream`: sends/receives `ControlEnvelope` (length-prefixed JSON).
//! - `MediaStream`: thin handle over the media socket. Phase 1 only writes one
//!   `MEDIA_HELLO` frame and then keeps the socket open for Phase 2 video.

use std::net::SocketAddr;
use std::pin::Pin;

use ccp_protocol::ControlEnvelope;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::error::TransportError;
use crate::framing::{read_frame, write_frame};

type BoxedReader = Pin<Box<dyn AsyncRead + Send + Unpin>>;
type BoxedWriter = Pin<Box<dyn AsyncWrite + Send + Unpin>>;

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub addr: SocketAddr,
}

pub struct ControlStream {
    pub peer: PeerInfo,
    reader: BoxedReader,
    writer: BoxedWriter,
    rx_buf: Vec<u8>,
}

impl std::fmt::Debug for ControlStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlStream")
            .field("peer", &self.peer)
            .finish()
    }
}

impl ControlStream {
    pub fn from_tcp(peer: PeerInfo, sock: TcpStream) -> Self {
        let (r, w) = tokio::io::split(sock);
        Self {
            peer,
            reader: Box::pin(r),
            writer: Box::pin(w),
            rx_buf: Vec::with_capacity(4096),
        }
    }

    pub fn from_halves<R, W>(peer: PeerInfo, r: R, w: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        Self {
            peer,
            reader: Box::pin(r),
            writer: Box::pin(w),
            rx_buf: Vec::with_capacity(4096),
        }
    }

    pub async fn send(&mut self, env: &ControlEnvelope) -> Result<(), TransportError> {
        let json = serde_json::to_vec(env)?;
        write_frame(&mut self.writer, &json).await
    }

    pub async fn recv(&mut self) -> Result<ControlEnvelope, TransportError> {
        read_frame(&mut self.reader, &mut self.rx_buf).await?;
        let env: ControlEnvelope = serde_json::from_slice(&self.rx_buf)?;
        Ok(env)
    }
}

pub struct MediaStream {
    pub peer: PeerInfo,
    pub reader: BoxedReader,
    pub writer: BoxedWriter,
}

impl std::fmt::Debug for MediaStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaStream")
            .field("peer", &self.peer)
            .finish()
    }
}

impl MediaStream {
    pub fn from_tcp(peer: PeerInfo, sock: TcpStream) -> Self {
        let (r, w) = tokio::io::split(sock);
        Self {
            peer,
            reader: Box::pin(r),
            writer: Box::pin(w),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::{Auth, ControlMessage};
    use tokio::io::duplex;

    fn dummy_peer() -> PeerInfo {
        PeerInfo {
            addr: "127.0.0.1:0".parse().unwrap(),
        }
    }

    #[tokio::test]
    async fn control_round_trip_typed_envelope() {
        let (a, b) = duplex(64 * 1024);
        let (ra, wa) = tokio::io::split(a);
        let (rb, wb) = tokio::io::split(b);
        let mut left = ControlStream::from_halves(dummy_peer(), ra, wa);
        let mut right = ControlStream::from_halves(dummy_peer(), rb, wb);

        let env = ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Auth(Auth {
                token: "abc".into(),
            }),
        };
        left.send(&env).await.unwrap();
        let got = right.recv().await.unwrap();
        assert_eq!(got, env);
    }
}
