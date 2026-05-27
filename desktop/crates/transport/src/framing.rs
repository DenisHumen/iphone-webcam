//! Length-prefixed JSON framing per `docs/03-protocol.md` §5.1.
//!
//! Wire layout: `[uint32 BE length][UTF-8 JSON payload]`.

use ccp_protocol::DEFAULT_MAX_PAYLOAD;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::TransportError;

const MAX_PAYLOAD_USIZE: usize = DEFAULT_MAX_PAYLOAD as usize;

pub async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    json: &[u8],
) -> Result<(), TransportError> {
    if json.len() > MAX_PAYLOAD_USIZE {
        return Err(TransportError::FrameTooLarge(json.len()));
    }
    let len: u32 = json
        .len()
        .try_into()
        .map_err(|_| TransportError::FrameTooLarge(json.len()))?;
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(json).await?;
    w.flush().await?;
    Ok(())
}

pub async fn read_frame<R: AsyncRead + Unpin>(
    r: &mut R,
    buf: &mut Vec<u8>,
) -> Result<usize, TransportError> {
    let mut prefix = [0u8; 4];
    r.read_exact(&mut prefix).await?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len > MAX_PAYLOAD_USIZE {
        return Err(TransportError::FrameTooLarge(len));
    }
    buf.clear();
    buf.resize(len, 0);
    r.read_exact(buf).await?;
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn round_trip_simple_json() {
        let (mut a, mut b) = duplex(64 * 1024);
        let payload = br#"{"t":"HELLO","seq":1}"#;
        write_frame(&mut a, payload).await.unwrap();
        let mut buf = Vec::new();
        let n = read_frame(&mut b, &mut buf).await.unwrap();
        assert_eq!(n, payload.len());
        assert_eq!(&buf, payload);
    }

    #[tokio::test]
    async fn rejects_oversized_frame() {
        let (mut a, _b) = duplex(1024);
        let payload = vec![b'x'; MAX_PAYLOAD_USIZE + 1];
        let err = write_frame(&mut a, &payload).await.unwrap_err();
        assert!(matches!(err, TransportError::FrameTooLarge(_)));
    }

    #[tokio::test]
    async fn read_eof_propagates() {
        let (a, mut b) = duplex(64);
        drop(a);
        let mut buf = Vec::new();
        let err = read_frame(&mut b, &mut buf).await.unwrap_err();
        assert!(matches!(err, TransportError::Io(_)));
    }
}
