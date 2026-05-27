//! Pair an incoming media socket with the control session that already
//! completed AUTH_OK. The media socket sends MEDIA_HELLO { sessionId, token }
//! as a length-prefixed JSON envelope on the same wire as control (Phase 1
//! reuses the framing).

use std::time::Duration;

use ccp_protocol::{ControlEnvelope, ControlMessage, MediaHello};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;
use tracing::warn;
use transport::{MediaStream, TransportError};

use crate::error::SessionError;

pub const MEDIA_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaBinding {
    pub session_id: String,
    pub token: String,
}

pub async fn read_media_hello(stream: &mut MediaStream) -> Result<MediaBinding, SessionError> {
    let mut prefix = [0u8; 4];
    timeout(
        MEDIA_HANDSHAKE_TIMEOUT,
        stream.reader.read_exact(&mut prefix),
    )
    .await
    .map_err(|_| SessionError::MediaHandshakeTimeout)?
    .map_err(TransportError::from)?;
    let len = u32::from_be_bytes(prefix) as usize;
    let mut buf = vec![0u8; len];
    timeout(MEDIA_HANDSHAKE_TIMEOUT, stream.reader.read_exact(&mut buf))
        .await
        .map_err(|_| SessionError::MediaHandshakeTimeout)?
        .map_err(TransportError::from)?;
    let env: ControlEnvelope =
        serde_json::from_slice(&buf).map_err(|e| SessionError::Transport(e.into()))?;
    match env.body {
        ControlMessage::MediaHello(m) => Ok(MediaBinding {
            session_id: m.session_id,
            token: m.token,
        }),
        other => {
            warn!(?other, "media socket did not present MEDIA_HELLO");
            Err(SessionError::UnexpectedMessage("expected MEDIA_HELLO"))
        }
    }
}

pub async fn write_media_hello(
    stream: &mut MediaStream,
    binding: &MediaBinding,
) -> Result<(), SessionError> {
    let env = ControlEnvelope {
        seq: 0,
        ack: None,
        body: ControlMessage::MediaHello(MediaHello {
            session_id: binding.session_id.clone(),
            token: binding.token.clone(),
        }),
    };
    let json = serde_json::to_vec(&env).map_err(|e| SessionError::Transport(e.into()))?;
    let len: u32 = json.len() as u32;
    stream
        .writer
        .write_all(&len.to_be_bytes())
        .await
        .map_err(TransportError::from)?;
    stream
        .writer
        .write_all(&json)
        .await
        .map_err(TransportError::from)?;
    stream.writer.flush().await.map_err(TransportError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::{TcpListener, TcpStream};
    use transport::PeerInfo;

    #[tokio::test]
    async fn round_trip_media_hello_over_tcp() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (sock, peer) = l.accept().await.unwrap();
            let mut ms = MediaStream::from_tcp(PeerInfo { addr: peer }, sock);
            read_media_hello(&mut ms).await
        });
        let client = tokio::spawn(async move {
            let sock = TcpStream::connect(addr).await.unwrap();
            let peer = sock.peer_addr().unwrap();
            let mut ms = MediaStream::from_tcp(PeerInfo { addr: peer }, sock);
            write_media_hello(
                &mut ms,
                &MediaBinding {
                    session_id: "sess-xyz".into(),
                    token: "tok".into(),
                },
            )
            .await
        });
        client.await.unwrap().unwrap();
        let got = server.await.unwrap().unwrap();
        assert_eq!(
            got,
            MediaBinding {
                session_id: "sess-xyz".into(),
                token: "tok".into(),
            }
        );
    }
}
