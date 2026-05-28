//! Reads `[28-byte MediaHeader][payload]` records off the media socket.

use bytes::Bytes;
use ccp_protocol::{MediaHeader, HEADER_LEN};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use transport::MediaStream;

use crate::error::MediaPipelineError;

/// 16 MiB safety cap. 1080p NV12 = ~3 MiB, well under.
const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct MediaFrame {
    pub header: MediaHeader,
    pub payload: Bytes,
}

pub async fn read_media_frame(stream: &mut MediaStream) -> Result<MediaFrame, MediaPipelineError> {
    let mut header_buf = [0u8; HEADER_LEN];
    stream
        .reader
        .read_exact(&mut header_buf)
        .await
        .map_err(transport::TransportError::from)?;
    let header = MediaHeader::decode(&header_buf)?;
    if header.payload_len > MAX_FRAME_BYTES {
        return Err(MediaPipelineError::PayloadTooLarge(header.payload_len));
    }
    let mut payload = vec![0u8; header.payload_len as usize];
    stream
        .reader
        .read_exact(&mut payload)
        .await
        .map_err(transport::TransportError::from)?;
    Ok(MediaFrame {
        header,
        payload: Bytes::from(payload),
    })
}

pub async fn write_media_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    header: &MediaHeader,
    payload: &[u8],
) -> Result<(), MediaPipelineError> {
    let hdr = header.encode();
    writer
        .write_all(&hdr)
        .await
        .map_err(transport::TransportError::from)?;
    writer
        .write_all(payload)
        .await
        .map_err(transport::TransportError::from)?;
    writer
        .flush()
        .await
        .map_err(transport::TransportError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::{Codec, Flags, MediaType};
    use tokio::io::{duplex, AsyncWriteExt};

    #[tokio::test]
    async fn round_trips_a_synthetic_frame() {
        let (mut a, b) = duplex(1024 * 1024);
        let header = MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::FULL_RANGE,
            codec: Codec::Raw,
            width: 16,
            height: 16,
            seq: 1,
            pts_usec: 0,
            payload_len: 384,
        };
        let payload = vec![0xABu8; 384];
        a.write_all(&header.encode()).await.unwrap();
        a.write_all(&payload).await.unwrap();
        a.flush().await.unwrap();

        let peer = transport::PeerInfo {
            addr: "127.0.0.1:0".parse().unwrap(),
        };
        let (r, w) = tokio::io::split(b);
        let mut ms = transport::MediaStream {
            peer,
            reader: Box::pin(r),
            writer: Box::pin(w),
        };
        let frame = read_media_frame(&mut ms).await.unwrap();
        assert_eq!(frame.header, header);
        assert_eq!(frame.payload.len(), 384);
        assert_eq!(frame.payload[0], 0xAB);
    }

    #[tokio::test]
    async fn rejects_too_large_payload() {
        let (mut a, b) = duplex(64);
        let mut buf = vec![0u8; HEADER_LEN];
        let header = MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::empty(),
            codec: Codec::Raw,
            width: 0,
            height: 0,
            seq: 0,
            pts_usec: 0,
            payload_len: MAX_FRAME_BYTES + 1,
        };
        buf.copy_from_slice(&header.encode());
        a.write_all(&buf).await.unwrap();
        a.flush().await.unwrap();
        let (r, w) = tokio::io::split(b);
        let mut ms = transport::MediaStream {
            peer: transport::PeerInfo {
                addr: "127.0.0.1:0".parse().unwrap(),
            },
            reader: Box::pin(r),
            writer: Box::pin(w),
        };
        let err = read_media_frame(&mut ms).await.unwrap_err();
        assert!(matches!(err, MediaPipelineError::PayloadTooLarge(_)));
    }
}
