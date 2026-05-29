//! MediaPipeline: pumps frames from a media socket into FrameSink consumers.
//!
//! Routes by `Codec`:
//!   - `Raw` → split NV12 → fanout (Phase 2 path).
//!   - `Hevc`/`H264` → `Decoder::decode()` → fanout the resulting NV12 frame.
//!
//! Drop-oldest is delegated to each `FrameSink::submit()` impl.

use std::sync::Arc;

use ccp_protocol::Codec;
use decode::{Decoder, FrameHint};
use sink::{Frame, FrameSink};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};
use transport::MediaStream;

use crate::error::MediaPipelineError;
use crate::frame_buf::split_nv12;
use crate::reader::{read_media_frame, MediaFrame};

pub struct MediaPipeline {
    pub task: JoinHandle<()>,
}

impl MediaPipeline {
    pub fn spawn(
        mut stream: MediaStream,
        sinks: Vec<Arc<dyn FrameSink>>,
        decoder: Arc<dyn Decoder>,
    ) -> Self {
        let task = tokio::spawn(async move {
            info!("media pipeline running");
            loop {
                match read_media_frame(&mut stream).await {
                    Ok(frame) => {
                        if let Err(e) = ingest(&frame, &sinks, &*decoder).await {
                            warn!(error = ?e, "frame ingest failed; continuing");
                        }
                    }
                    Err(e) => {
                        info!(error = ?e, "media pipeline exiting");
                        return;
                    }
                }
            }
        });
        Self { task }
    }
}

async fn ingest(
    raw: &MediaFrame,
    sinks: &[Arc<dyn FrameSink>],
    decoder: &dyn Decoder,
) -> Result<(), MediaPipelineError> {
    let frame = match raw.header.codec {
        Codec::Raw => {
            let (y, uv) = split_nv12(raw.payload.clone(), raw.header.width, raw.header.height)?;
            Frame {
                width: raw.header.width,
                height: raw.header.height,
                pts_usec: raw.header.pts_usec,
                seq: raw.header.seq,
                codec: Codec::Raw,
                plane_y: y,
                plane_uv: uv,
                full_range: raw.header.flags.contains(ccp_protocol::Flags::FULL_RANGE),
            }
        }
        codec @ (Codec::Hevc | Codec::H264) => {
            let hint = FrameHint {
                width: raw.header.width,
                height: raw.header.height,
                pts_usec: raw.header.pts_usec,
                seq: raw.header.seq,
                keyframe: raw.header.flags.contains(ccp_protocol::Flags::KEYFRAME),
                config: raw.header.flags.contains(ccp_protocol::Flags::CONFIG),
                full_range: raw.header.flags.contains(ccp_protocol::Flags::FULL_RANGE),
            };
            match decoder.decode(codec, raw.payload.clone(), hint).await {
                Some(f) => f,
                None => {
                    debug!(
                        seq = raw.header.seq,
                        "decoder consumed config/intermediate frame"
                    );
                    return Ok(());
                }
            }
        }
    };
    debug!(seq = frame.seq, "ingested frame");
    for s in sinks {
        s.submit(frame.clone()).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ccp_protocol::{Flags, MediaHeader, MediaType};
    use decode::PassthroughDecoder;
    use tokio::io::{duplex, AsyncWriteExt};
    use tokio::sync::Mutex;

    struct CapturingSink {
        frames: Arc<Mutex<Vec<Frame>>>,
    }
    #[async_trait]
    impl FrameSink for CapturingSink {
        async fn submit(&self, f: Frame) {
            self.frames.lock().await.push(f);
        }
    }

    fn build_stream(buffer_size: usize) -> (tokio::io::DuplexStream, MediaStream) {
        let (writer, reader_half) = duplex(buffer_size);
        let peer = transport::PeerInfo {
            addr: "127.0.0.1:0".parse().unwrap(),
            source: transport::Source::Wifi,
        };
        let (r, w) = tokio::io::split(reader_half);
        let ms = MediaStream {
            peer,
            reader: Box::pin(r),
            writer: Box::pin(w),
        };
        (writer, ms)
    }

    #[tokio::test]
    async fn pipeline_forwards_three_raw_frames_to_sink() {
        let (mut writer, ms) = build_stream(1024 * 1024);
        let frames = Arc::new(Mutex::new(Vec::<Frame>::new()));
        let sink_arc: Arc<dyn FrameSink> = Arc::new(CapturingSink {
            frames: frames.clone(),
        });
        let pipeline = MediaPipeline::spawn(ms, vec![sink_arc], Arc::new(PassthroughDecoder));

        for seq in 0..3u32 {
            let header = MediaHeader {
                media_type: MediaType::Video,
                flags: Flags::FULL_RANGE,
                codec: Codec::Raw,
                width: 16,
                height: 16,
                seq,
                pts_usec: u64::from(seq) * 33_000,
                payload_len: 384,
            };
            writer.write_all(&header.encode()).await.unwrap();
            writer.write_all(&vec![seq as u8; 384]).await.unwrap();
        }
        writer.flush().await.unwrap();
        drop(writer);
        let _ = pipeline.task.await;

        let got = frames.lock().await;
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].seq, 0);
        assert_eq!(got[2].seq, 2);
    }

    #[tokio::test]
    async fn pipeline_routes_hevc_through_decoder_and_skips_config() {
        let (mut writer, ms) = build_stream(64 * 1024);
        let frames = Arc::new(Mutex::new(Vec::<Frame>::new()));
        let sink_arc: Arc<dyn FrameSink> = Arc::new(CapturingSink {
            frames: frames.clone(),
        });
        let pipeline = MediaPipeline::spawn(ms, vec![sink_arc], Arc::new(PassthroughDecoder));

        // Config frame — should be swallowed.
        let cfg = MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::ENCODED | Flags::CONFIG,
            codec: Codec::Hevc,
            width: 16,
            height: 16,
            seq: 0,
            pts_usec: 0,
            payload_len: 4,
        };
        writer.write_all(&cfg.encode()).await.unwrap();
        writer.write_all(&[0u8; 4]).await.unwrap();
        // Two encoded frames.
        for seq in 1..=2u32 {
            let h = MediaHeader {
                media_type: MediaType::Video,
                flags: Flags::ENCODED | Flags::KEYFRAME,
                codec: Codec::Hevc,
                width: 16,
                height: 16,
                seq,
                pts_usec: u64::from(seq),
                payload_len: 8,
            };
            writer.write_all(&h.encode()).await.unwrap();
            writer.write_all(&[0u8; 8]).await.unwrap();
        }
        writer.flush().await.unwrap();
        drop(writer);
        let _ = pipeline.task.await;

        let got = frames.lock().await;
        assert_eq!(got.len(), 2, "config frame should not produce a Frame");
        assert_eq!(got[0].seq, 1);
        assert_eq!(got[1].seq, 2);
        // PassthroughDecoder writes seq as Y byte.
        assert_eq!(got[0].plane_y[0], 1);
    }
}
