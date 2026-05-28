//! MediaPipeline: pumps frames from a media socket into FrameSink consumers.
//!
//! Phase 2 only handles `Codec::Raw` (NV12). Drop-oldest is achieved naturally
//! by each FrameSink owning its own bounded queue — `submit()` is required to
//! be non-blocking and shed load locally.

use std::sync::Arc;

use ccp_protocol::Codec;
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
    pub fn spawn(mut stream: MediaStream, sinks: Vec<Arc<dyn FrameSink>>) -> Self {
        let task = tokio::spawn(async move {
            info!("media pipeline running");
            loop {
                match read_media_frame(&mut stream).await {
                    Ok(frame) => {
                        if let Err(e) = ingest(&frame, &sinks).await {
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

async fn ingest(raw: &MediaFrame, sinks: &[Arc<dyn FrameSink>]) -> Result<(), MediaPipelineError> {
    if raw.header.codec != Codec::Raw {
        return Err(MediaPipelineError::UnsupportedCodec(raw.header.codec));
    }
    let (y, uv) = split_nv12(raw.payload.clone(), raw.header.width, raw.header.height)?;
    let frame = Frame {
        width: raw.header.width,
        height: raw.header.height,
        pts_usec: raw.header.pts_usec,
        seq: raw.header.seq,
        codec: raw.header.codec,
        plane_y: y,
        plane_uv: uv,
        full_range: raw.header.flags.contains(ccp_protocol::Flags::FULL_RANGE),
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
    use ccp_protocol::{Codec, Flags, MediaHeader, MediaType};
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

    #[tokio::test]
    async fn pipeline_forwards_three_raw_frames_to_sink() {
        let (mut writer, reader_half) = duplex(1024 * 1024);
        let peer = transport::PeerInfo {
            addr: "127.0.0.1:0".parse().unwrap(),
        };
        let (r, w) = tokio::io::split(reader_half);
        let ms = transport::MediaStream {
            peer,
            reader: Box::pin(r),
            writer: Box::pin(w),
        };
        let frames = Arc::new(Mutex::new(Vec::<Frame>::new()));
        let sink_arc: Arc<dyn FrameSink> = Arc::new(CapturingSink {
            frames: frames.clone(),
        });
        let pipeline = MediaPipeline::spawn(ms, vec![sink_arc]);

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
        drop(writer); // close, let the reader's read_exact fail and the loop exit

        // wait for pipeline task to drain + exit
        let _ = pipeline.task.await;

        let got = frames.lock().await;
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].seq, 0);
        assert_eq!(got[2].seq, 2);
        assert_eq!(got[0].plane_y.len(), 256);
        assert_eq!(got[0].plane_uv.len(), 128);
        assert!(got[0].full_range);
    }
}
