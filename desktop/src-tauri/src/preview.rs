//! PreviewSink: NV12 → JPEG → `session://preview` event for the React canvas.
//!
//! Throttled to ≤30 emits/sec. Frames arriving faster are silently dropped at
//! the sink, satisfying the drop-oldest invariant described in docs/02 §6.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use sink::{Frame, FrameSink};
use tauri::Emitter;
use tokio::sync::Mutex;
use tracing::warn;

const MAX_PREVIEW_WIDTH: u32 = 720;
const MIN_EMIT_INTERVAL_MS: u128 = 33;

pub struct PreviewSink {
    app: tauri::AppHandle,
    last_emit: Mutex<std::time::Instant>,
}

impl PreviewSink {
    pub fn new(app: tauri::AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            last_emit: Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(1)),
        })
    }
}

#[async_trait]
impl FrameSink for PreviewSink {
    async fn submit(&self, frame: Frame) {
        {
            let mut last = self.last_emit.lock().await;
            if last.elapsed().as_millis() < MIN_EMIT_INTERVAL_MS {
                return;
            }
            *last = std::time::Instant::now();
        }
        let app = self.app.clone();
        // Encode off the runtime thread — JPEG is CPU-bound for a 720p frame
        // (~1 ms) but keeps the media-pipeline task free of head-of-line jitter.
        let result = tokio::task::spawn_blocking(move || nv12_to_jpeg(&frame))
            .await
            .ok()
            .and_then(Result::ok);
        let Some((jpeg, w, h, seq, pts)) = result else {
            warn!("preview encode failed");
            return;
        };
        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
        if let Err(e) = app.emit(
            "session://preview",
            serde_json::json!({
                "seq": seq,
                "ptsUsec": pts,
                "width": w,
                "height": h,
                "jpegBase64": b64,
            }),
        ) {
            warn!(error = ?e, "emit session://preview failed");
        }
    }
}

fn nv12_to_jpeg(f: &Frame) -> Result<(Vec<u8>, u32, u32, u32, u64), image::ImageError> {
    use image::{ImageBuffer, Rgb};
    let w = f.width as usize;
    let h = f.height as usize;
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let y_val = f64::from(f.plane_y[y * w + x]);
            let uv_row = y / 2;
            let uv_col = (x / 2) * 2;
            let uv_idx = uv_row * w + uv_col;
            let cb = f64::from(f.plane_uv[uv_idx]);
            let cr = f64::from(f.plane_uv[uv_idx + 1]);
            let (yv, cbv, crv) = if f.full_range {
                (y_val, cb - 128.0, cr - 128.0)
            } else {
                (1.164 * (y_val - 16.0), cb - 128.0, cr - 128.0)
            };
            let r = (yv + 1.596 * crv).clamp(0.0, 255.0) as u8;
            let g = (yv - 0.392 * cbv - 0.813 * crv).clamp(0.0, 255.0) as u8;
            let b = (yv + 2.017 * cbv).clamp(0.0, 255.0) as u8;
            rgb.extend_from_slice(&[r, g, b]);
        }
    }
    let img: ImageBuffer<Rgb<u8>, _> =
        ImageBuffer::from_raw(u32::from(f.width), u32::from(f.height), rgb)
            .expect("rgb buffer size matches");

    let final_img = if img.width() > MAX_PREVIEW_WIDTH {
        let scale = MAX_PREVIEW_WIDTH as f32 / img.width() as f32;
        let new_w = MAX_PREVIEW_WIDTH;
        let new_h = (img.height() as f32 * scale) as u32;
        image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let mut out = Vec::new();
    {
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 70);
        enc.encode(
            final_img.as_raw(),
            final_img.width(),
            final_img.height(),
            image::ExtendedColorType::Rgb8,
        )?;
    }
    Ok((
        out,
        final_img.width(),
        final_img.height(),
        f.seq,
        f.pts_usec,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use ccp_protocol::Codec;

    #[test]
    fn nv12_to_jpeg_produces_non_empty_output() {
        let f = Frame {
            width: 16,
            height: 16,
            pts_usec: 0,
            seq: 0,
            codec: Codec::Raw,
            plane_y: Bytes::from(vec![128u8; 16 * 16]),
            plane_uv: Bytes::from(vec![128u8; 16 * 16 / 2]),
            full_range: true,
        };
        let (jpeg, w, h, _seq, _pts) = nv12_to_jpeg(&f).unwrap();
        assert!(!jpeg.is_empty());
        assert!(jpeg.starts_with(&[0xFF, 0xD8])); // JPEG SOI marker
        assert_eq!(w, 16);
        assert_eq!(h, 16);
    }
}
