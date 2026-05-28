# Phase 2 — RAW Video Path + Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Live RAW NV12 video flows from the iPhone (or mock client) over Wi-Fi to the desktop, where `mediapipeline` normalizes it into NV12 frames, hands them to a `PreviewSink`, and the React UI renders them on a `<canvas>` at ~720p30. Camera switch from the desktop UI changes the active iPhone lens in <1 s.

**Architecture:** The media socket — which Phase 1 left parked after MEDIA_HELLO — is now consumed by a new `mediapipeline` actor running per session. It reads `[28-byte MediaHeader][NV12 payload]` records from the socket, validates them, drops oldest on backpressure (bounded mpsc), and fans frames out to `FrameSink` consumers. `PreviewSink` downscales to ≤720×whatever, encodes JPEG, and pushes byte-arrays through a Tauri event channel into a React `<canvas>`. Camera-switch comes top-down: UI → Tauri command → `ControlPlane` outbound queue → `SET_CAMERA` envelope → iPhone `CaptureEngine.setCamera()` → `CAMERA_STATE` confirmation.

**Tech Stack:** Continues with Rust 1.95 stable, tokio, serde, ccp-protocol media types. New deps: `image` crate (JPEG encode), `fast_image_resize` (downscale). iOS adds AVFoundation (`AVCaptureSession`, `AVCaptureVideoDataOutput`), CoreVideo (`CVPixelBuffer`), and an NV12 packer.

**Acceptance (docs/10 Phase 2):**
- mock-iphone — or a real iPhone — streams RAW NV12 at ≥720p30 to the desktop.
- Tauri UI `<canvas>` shows the live preview at ~25–30 fps with bounded latency (drop-oldest, no buffer growth).
- "Switch camera" picker in the UI flips iPhone's active lens; preview shows the new feed in <1 s.
- `cargo test --workspace --all-targets` covers MediaPipeline (jitter buffer, header parse, drop-oldest), PreviewSink, and an e2e test where mock-iphone sends ≥10 synthetic frames and the UI-side mock sink receives them in order with dropped duplicates.
- `swift test` covers the iOS NV12 packer and CaptureEngine state machine via injectable provider.
- CI green; tag `v0.3.0-phase2`.

**Out of scope (deferred):**
- VideoToolbox encode/decode (Phase 4).
- Virtual camera sink (Phase 3).
- USB transport (Phase 5).
- Adaptive engine / speedtest (Phase 4).

---

## File Structure

### New / modified Rust files

```
desktop/
├── Cargo.toml                                       # MOD: workspace deps image, fast_image_resize, base64
├── crates/
│   ├── sink/
│   │   ├── Cargo.toml                               # MOD: deps + bytes
│   │   └── src/
│   │       ├── lib.rs                               # MOD: exports
│   │       ├── frame.rs                             # NEW: Frame { width, height, pts_usec, plane_y, plane_uv, ... }
│   │       └── trait.rs                             # NEW: FrameSink async trait
│   ├── mediapipeline/
│   │   ├── Cargo.toml                               # MOD: deps tokio, ccp-protocol, transport, sink, bytes, tracing, thiserror
│   │   └── src/
│   │       ├── lib.rs                               # MOD: wiring
│   │       ├── reader.rs                            # NEW: async media-socket reader
│   │       ├── pipeline.rs                          # NEW: MediaPipeline actor + drop-oldest mpsc fanout
│   │       ├── frame_buf.rs                         # NEW: NV12 plane slice helpers
│   │       └── error.rs                             # NEW: MediaPipelineError
│   ├── app/
│   │   ├── Cargo.toml                               # MOD: + mediapipeline, sink
│   │   └── src/lib.rs                               # MOD: spawn MediaPipeline on media-attach, expose preview Frame stream
│   └── session/
│       └── src/controlplane.rs                      # MOD: outbound queue for SET_CAMERA (forwarded by app)
├── src-tauri/
│   ├── Cargo.toml                                   # MOD: + sink + base64 + image
│   └── src/
│       ├── commands.rs                              # MOD: set_camera command
│       ├── events.rs                                # MOD: pump preview frames as session://preview
│       └── preview.rs                               # NEW: PreviewSink impl — downscale + JPEG + Emitter
└── tools/
    └── mock-iphone/
        ├── Cargo.toml                               # MOD: + clap-like args
        └── src/main.rs                              # MOD: synthetic NV12 generator + emit at 30 fps
```

### New / modified iOS files

```
ios/
├── Package.swift                                    # MOD: add Capture target deps if needed
├── Sources/
│   ├── ClearCamCore/
│   │   ├── Capture/
│   │   │   ├── CameraEnumerator.swift               # NEW: AVCaptureDevice.DiscoverySession → [CameraEntry]
│   │   │   ├── CaptureEngine.swift                  # NEW: AVCaptureSession + AVCaptureVideoDataOutput driver
│   │   │   └── CapturedFrame.swift                  # NEW: NV12 plane refs + pts_usec
│   │   ├── Encode/
│   │   │   ├── RawEncoder.swift                     # NEW: CVPixelBuffer → MediaHeader+payload bytes
│   │   │   └── MediaWire.swift                      # NEW: BE write of 28-byte header
│   │   ├── Transport/
│   │   │   └── MediaStream.swift                    # NEW: high-rate NWConnection write helper
│   │   └── Session/
│   │       └── SessionController.swift              # MOD: handleSetCamera, forward to CaptureEngine
│   └── ClearCamApp/
│       ├── PreviewLocalView.swift                   # NEW (optional): AVCaptureVideoPreviewLayer for the iPhone user
│       └── ConnectedView.swift                      # MOD: show capture state, active camera id
└── Tests/
    └── ClearCamCoreTests/
        ├── MediaWireTests.swift                     # NEW: byte-exact header tests (mirrors Rust)
        ├── RawEncoderTests.swift                    # NEW: packs synthetic CVPixelBuffer
        └── CameraEnumeratorTests.swift              # NEW: injectable provider tests
```

### UI

```
desktop/ui/src/
├── lib/types.ts                                     # MOD: PreviewFrame, SetCameraResult
├── lib/tauri.ts                                     # MOD: onPreviewFrame, setCamera
├── components/
│   ├── PreviewCanvas.tsx                            # NEW: <canvas> + ImageBitmap rendering, throttle
│   └── CameraPicker.tsx                             # NEW: dropdown of cameras
└── App.tsx                                          # MOD: wire CameraPicker + PreviewCanvas inside DeviceCard
```

---

## Conventions
- Per-task TDD: failing test → minimal impl → green → commit.
- Per-task gate (rustfmt, clippy, cargo test for Rust; pnpm typecheck/lint/format/build for UI; swift build/test for iOS).
- Working dirs: `desktop/` for cargo, `desktop/ui/` for pnpm, `ios/` (with `. scripts/swift-env.sh`) for swift.

---

### Task 1: `sink::Frame` + `FrameSink` trait (TDD)

**Files:**
- Modify: `desktop/crates/sink/Cargo.toml`
- Create: `desktop/crates/sink/src/frame.rs`
- Create: `desktop/crates/sink/src/r#trait.rs` (renamed to `sink_trait.rs`)
- Modify: `desktop/crates/sink/src/lib.rs`

- [ ] **Step 1: Add deps**

```toml
[dependencies]
async-trait = { workspace = true }
bytes = { workspace = true }
thiserror = { workspace = true }
ccp-protocol = { path = "../ccp-protocol" }
```

- [ ] **Step 2: Frame type**

`frame.rs`:

```rust
//! Canonical in-pipeline frame: NV12 planes + pts.

use bytes::Bytes;
use ccp_protocol::Codec;

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub pts_usec: u64,
    pub seq: u32,
    pub codec: Codec,
    /// Y plane: `width*height` bytes.
    pub plane_y: Bytes,
    /// CbCr (chroma) plane: `width*height/2` bytes (NV12). Empty for non-raw frames.
    pub plane_uv: Bytes,
    pub full_range: bool,
}

impl Frame {
    pub fn is_raw_nv12(&self) -> bool {
        matches!(self.codec, Codec::Raw) && !self.plane_y.is_empty() && !self.plane_uv.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_raw_nv12_checks_planes_and_codec() {
        let f = Frame {
            width: 1280, height: 720, pts_usec: 0, seq: 0,
            codec: Codec::Raw,
            plane_y: Bytes::from(vec![0u8; 1280*720]),
            plane_uv: Bytes::from(vec![0u8; 1280*720/2]),
            full_range: true,
        };
        assert!(f.is_raw_nv12());
    }
}
```

- [ ] **Step 3: Trait**

`sink_trait.rs`:

```rust
use async_trait::async_trait;

use crate::frame::Frame;

#[async_trait]
pub trait FrameSink: Send + Sync + 'static {
    /// Submit a frame. Implementations MUST be non-blocking on backpressure —
    /// drop the frame if the consumer is behind.
    async fn submit(&self, frame: Frame);
    async fn stop(&self) {}
}
```

- [ ] **Step 4: lib.rs**

```rust
#![forbid(unsafe_code)]

pub mod frame;
mod sink_trait;

pub use frame::Frame;
pub use sink_trait::FrameSink;
```

- [ ] **Step 5: gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p sink
git add desktop/crates/sink && git commit -m "feat(sink): Frame + FrameSink async trait"
```

---

### Task 2: MediaPipeline reader + actor (TDD)

**Files:**
- Modify: `desktop/crates/mediapipeline/Cargo.toml`
- Create: `desktop/crates/mediapipeline/src/error.rs`
- Create: `desktop/crates/mediapipeline/src/reader.rs`
- Create: `desktop/crates/mediapipeline/src/frame_buf.rs`
- Create: `desktop/crates/mediapipeline/src/pipeline.rs`
- Modify: `desktop/crates/mediapipeline/src/lib.rs`

- [ ] **Step 1: Cargo.toml**

```toml
[dependencies]
ccp-protocol = { path = "../ccp-protocol" }
transport = { path = "../transport" }
sink = { path = "../sink" }
tokio = { workspace = true }
bytes = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
async-trait = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

- [ ] **Step 2: error.rs**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaPipelineError {
    #[error("transport: {0}")]
    Transport(#[from] transport::TransportError),
    #[error("media header: {0}")]
    Header(#[from] ccp_protocol::MediaDecodeError),
    #[error("payload too large: {0} bytes")]
    PayloadTooLarge(u32),
    #[error("unsupported codec for Phase 2 (raw only): {0:?}")]
    UnsupportedCodec(ccp_protocol::Codec),
    #[error("plane size mismatch: got {got}, expected {expected}")]
    PlaneSizeMismatch { got: usize, expected: usize },
}
```

- [ ] **Step 3: reader.rs — async media frame reader**

```rust
//! Reads `[28-byte MediaHeader][payload]` records off the media socket.

use bytes::Bytes;
use ccp_protocol::{Codec, Flags, MediaHeader, HEADER_LEN};
use tokio::io::{AsyncRead, AsyncReadExt};
use transport::MediaStream;

use crate::error::MediaPipelineError;

const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024; // 16 MiB cap for safety

pub struct MediaFrame {
    pub header: MediaHeader,
    pub payload: Bytes,
}

pub async fn read_media_frame(stream: &mut MediaStream) -> Result<MediaFrame, MediaPipelineError> {
    let mut header_buf = [0u8; HEADER_LEN];
    stream.reader.read_exact(&mut header_buf).await
        .map_err(transport::TransportError::from)?;
    let header = MediaHeader::decode(&header_buf)?;
    if header.payload_len > MAX_FRAME_BYTES {
        return Err(MediaPipelineError::PayloadTooLarge(header.payload_len));
    }
    let mut payload = vec![0u8; header.payload_len as usize];
    stream.reader.read_exact(&mut payload).await
        .map_err(transport::TransportError::from)?;
    Ok(MediaFrame { header, payload: Bytes::from(payload) })
}

pub async fn write_media_frame<W: AsyncRead + tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    header: &MediaHeader,
    payload: &[u8],
) -> Result<(), MediaPipelineError> {
    use tokio::io::AsyncWriteExt;
    let hdr = header.encode();
    writer.write_all(&hdr).await.map_err(transport::TransportError::from)?;
    writer.write_all(payload).await.map_err(transport::TransportError::from)?;
    writer.flush().await.map_err(transport::TransportError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncWriteExt};

    #[tokio::test]
    async fn round_trips_a_synthetic_frame() {
        let (mut a, b) = duplex(1024 * 1024);
        let header = MediaHeader {
            media_type: ccp_protocol::MediaType::Video,
            flags: Flags::FULL_RANGE,
            codec: Codec::Raw,
            width: 16, height: 16, seq: 1, pts_usec: 0,
            payload_len: 384, // 16*16 + 16*16/2
        };
        let payload = vec![0xABu8; 384];
        a.write_all(&header.encode()).await.unwrap();
        a.write_all(&payload).await.unwrap();
        a.flush().await.unwrap();

        let peer = transport::PeerInfo { addr: "127.0.0.1:0".parse().unwrap() };
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
}
```

> **Note:** `MediaStream` fields must be `pub` to allow the test's hand-build. They already are.

- [ ] **Step 4: frame_buf.rs — split NV12 payload into Y / UV**

```rust
use bytes::Bytes;

use crate::error::MediaPipelineError;

pub fn split_nv12(payload: Bytes, width: u16, height: u16) -> Result<(Bytes, Bytes), MediaPipelineError> {
    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = w * h / 2;
    let expected = y_size + uv_size;
    if payload.len() != expected {
        return Err(MediaPipelineError::PlaneSizeMismatch { got: payload.len(), expected });
    }
    let y = payload.slice(0..y_size);
    let uv = payload.slice(y_size..);
    Ok((y, uv))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_evenly() {
        let buf = Bytes::from(vec![0u8; 16*16 + 16*16/2]);
        let (y, uv) = split_nv12(buf, 16, 16).unwrap();
        assert_eq!(y.len(), 256);
        assert_eq!(uv.len(), 128);
    }

    #[test]
    fn rejects_short_payload() {
        let buf = Bytes::from(vec![0u8; 100]);
        let err = split_nv12(buf, 16, 16).unwrap_err();
        assert!(matches!(err, MediaPipelineError::PlaneSizeMismatch { .. }));
    }
}
```

- [ ] **Step 5: pipeline.rs — MediaPipeline actor with drop-oldest fanout**

```rust
//! MediaPipeline: pumps frames from a media socket into FrameSink consumers.

use std::sync::Arc;

use ccp_protocol::Codec;
use sink::{Frame, FrameSink};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};
use transport::MediaStream;

use crate::error::MediaPipelineError;
use crate::frame_buf::split_nv12;
use crate::reader::read_media_frame;

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

async fn ingest(
    raw: &crate::reader::MediaFrame,
    sinks: &[Arc<dyn FrameSink>],
) -> Result<(), MediaPipelineError> {
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
```

- [ ] **Step 6: lib.rs**

```rust
#![forbid(unsafe_code)]

pub mod error;
pub mod frame_buf;
pub mod pipeline;
pub mod reader;

pub use error::MediaPipelineError;
pub use pipeline::MediaPipeline;
pub use reader::{read_media_frame, write_media_frame, MediaFrame};
```

- [ ] **Step 7: gate + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p mediapipeline
git add desktop/crates/mediapipeline && git commit -m "feat(mediapipeline): media-socket reader + NV12 frame_buf + drop-oldest fanout"
```

---

### Task 3: Hook MediaPipeline into AppCore (TDD)

**Files:**
- Modify: `desktop/crates/app/Cargo.toml`
- Modify: `desktop/crates/app/src/lib.rs`

- [ ] **Step 1: Add deps**

```toml
sink = { path = "../sink" }
mediapipeline = { path = "../mediapipeline" }
async-trait = { workspace = true }
```

- [ ] **Step 2: Expose `register_sink()` on AppHandle and feed media stream into MediaPipeline**

Replace the `tokio::spawn(async move { let _ = rx_media.await; ... })` block in `spawn_accept_loop` with:

```rust
let sinks_handle = sinks.clone();
tokio::spawn(async move {
    if let Ok(ms) = rx_media.await {
        info!(%session_id, "media stream attached; starting MediaPipeline");
        let sinks_snapshot: Vec<_> = sinks_handle.read().await.iter().cloned().collect();
        let _pipeline = MediaPipeline::spawn(ms, sinks_snapshot);
        // The pipeline owns the join handle; if a sink is added later we
        // recreate or live-mutate. For Phase 2 a single registration before
        // start_server is sufficient.
    }
});
```

Add to `AppHandle`:

```rust
pub sinks: Arc<tokio::sync::RwLock<Vec<Arc<dyn FrameSink>>>>,
```

…and a helper `pub async fn register_sink(&self, sink: Arc<dyn FrameSink>) { self.sinks.write().await.push(sink); }` that **must be called before** `start_server` or before the iPhone connects (Phase 2 single-session caveat — Phase 6 will dynamic-mutate).

Construct `sinks` in `AppCore::start()` and pass into the accept loop.

- [ ] **Step 3: Unit test — a mock sink receives frames after handshake + raw frame send**

`crates/app/tests/e2e_raw_video.rs`:

```rust
use std::sync::Arc;
use std::time::Duration;

use app::AppCore;
use async_trait::async_trait;
use bytes::Bytes;
use ccp_protocol::{Auth, Capability, ControlEnvelope, ControlMessage, DeviceIdent, Hello, MediaHello, MediaHeader, MediaType, Flags, Codec, PROTO_VER};
use mediapipeline::write_media_frame;
use session::{write_media_hello, MediaBinding};
use sink::{Frame, FrameSink};
use tokio::sync::Mutex;
use tokio::time::sleep;
use transport::wifi_connect;

struct CapturingSink {
    inner: Arc<Mutex<Vec<Frame>>>,
}

#[async_trait]
impl FrameSink for CapturingSink {
    async fn submit(&self, f: Frame) {
        self.inner.lock().await.push(f);
    }
}

#[tokio::test]
async fn mock_iphone_streams_raw_frames_to_sink() {
    let core = AppCore { host: "127.0.0.1".into() };
    let handle = core.start().await.unwrap();
    let captured = Arc::new(Mutex::new(Vec::<Frame>::new()));
    handle.register_sink(Arc::new(CapturingSink { inner: captured.clone() })).await;

    let token = handle.qr.token.clone();
    let (mut control, mut media) = wifi_connect(
        format!("127.0.0.1:{}", handle.qr.cport).parse().unwrap(),
        format!("127.0.0.1:{}", handle.qr.mport).parse().unwrap(),
    ).await.unwrap();

    // handshake (abbreviated — mirrors Phase 1 e2e)
    control.send(&ControlEnvelope { seq: 1, ack: None, body: ControlMessage::Hello(Hello {
        proto_ver: PROTO_VER, app: "t".into(),
        device: DeviceIdent { model: "m".into(), os_ver: "v".into() },
        session_id: "x".into(), caps: vec![Capability::RawNv12],
    })}).await.unwrap();
    let _ = control.recv().await.unwrap();
    control.send(&ControlEnvelope { seq: 2, ack: None, body: ControlMessage::Auth(Auth { token: token.clone() })}).await.unwrap();
    let ok = control.recv().await.unwrap();
    let sid = match ok.body { ControlMessage::AuthOk(o) => o.session_id, _ => panic!() };
    write_media_hello(&mut media, &MediaBinding { session_id: sid, token }).await.unwrap();

    // Send 5 raw NV12 frames (16x16, payload 384 bytes).
    for seq in 0..5u32 {
        let header = MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::FULL_RANGE,
            codec: Codec::Raw,
            width: 16, height: 16, seq, pts_usec: u64::from(seq) * 33_000,
            payload_len: 384,
        };
        let payload = vec![seq as u8; 384];
        write_media_frame(&mut media.writer, &header, &payload).await.unwrap();
    }

    // wait
    for _ in 0..50 {
        sleep(Duration::from_millis(50)).await;
        if captured.lock().await.len() >= 5 { break; }
    }
    let got = captured.lock().await;
    assert_eq!(got.len(), 5);
    assert_eq!(got[0].seq, 0);
    assert_eq!(got[4].seq, 4);
    handle.shutdown();
}
```

- [ ] **Step 4: gate + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/crates/app && git commit -m "feat(app): wire MediaPipeline into AppCore + e2e raw-video test"
```

---

### Task 4: PreviewSink (downscale + JPEG)

**Files:**
- Modify: `desktop/Cargo.toml` (add `image`, `fast_image_resize`)
- Modify: `desktop/src-tauri/Cargo.toml`
- Create: `desktop/src-tauri/src/preview.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Workspace deps**

```toml
image = { version = "0.25", default-features = false, features = ["jpeg"] }
fast_image_resize = "5"
base64 = "0.22"
```

- [ ] **Step 2: PreviewSink — NV12 → RGB → resized JPEG → emit event**

`preview.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use sink::{Frame, FrameSink};
use tauri::Emitter;
use tokio::sync::Mutex;

const MAX_WIDTH: u32 = 720;

pub struct PreviewSink {
    app: tauri::AppHandle,
    last_emit: Mutex<std::time::Instant>,
}

impl PreviewSink {
    pub fn new(app: tauri::AppHandle) -> Arc<Self> {
        Arc::new(Self { app, last_emit: Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(1)) })
    }
}

#[async_trait]
impl FrameSink for PreviewSink {
    async fn submit(&self, frame: Frame) {
        // Throttle to ≤30 fps emit rate.
        {
            let mut last = self.last_emit.lock().await;
            if last.elapsed() < std::time::Duration::from_millis(33) { return; }
            *last = std::time::Instant::now();
        }
        let jpeg = match nv12_to_jpeg(&frame) {
            Ok(j) => j,
            Err(e) => { tracing::warn!(error = ?e, "preview encode failed"); return; }
        };
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
        let _ = self.app.emit("session://preview", serde_json::json!({
            "seq": frame.seq,
            "ptsUsec": frame.pts_usec,
            "width": frame.width,
            "height": frame.height,
            "jpegBase64": b64,
        }));
    }
}

fn nv12_to_jpeg(f: &Frame) -> Result<Vec<u8>, image::ImageError> {
    use image::{ImageBuffer, Rgb};
    // Simple-and-correct: BT.601 limited→full when full_range=false, else passthrough.
    let w = f.width as usize;
    let h = f.height as usize;
    let mut rgb = Vec::with_capacity(w*h*3);
    for y in 0..h {
        for x in 0..w {
            let y_val = f.plane_y[y * w + x] as f32;
            let uv_idx = (y / 2) * w + (x / 2) * 2;
            let cb = f.plane_uv[uv_idx] as f32;
            let cr = f.plane_uv[uv_idx + 1] as f32;
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
    let img: ImageBuffer<Rgb<u8>, _> = ImageBuffer::from_raw(f.width as u32, f.height as u32, rgb)
        .expect("rgb buffer size matches");

    // Downscale if wider than MAX_WIDTH.
    let final_img = if img.width() > MAX_WIDTH {
        let scale = MAX_WIDTH as f32 / img.width() as f32;
        let new_w = MAX_WIDTH;
        let new_h = (img.height() as f32 * scale) as u32;
        image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let mut out = Vec::new();
    {
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 70);
        enc.encode(final_img.as_raw(), final_img.width(), final_img.height(), image::ExtendedColorType::Rgb8)?;
    }
    Ok(out)
}
```

- [ ] **Step 3: Register PreviewSink during Tauri setup**

In `lib.rs` setup closure, before the events::pump spawn, hold the AppHandle and register the sink AFTER the user calls `start_server`. Simpler design: `start_server` returns the QR and then registers the preview sink immediately.

Edit `commands::start_server`:

```rust
let handle = AppCore { host: host.clone() }.start().await.map_err(|e| format!("{e:?}"))?;
let preview = preview::PreviewSink::new(app_handle.clone());
handle.register_sink(preview).await;
```

This requires `start_server` to accept the `tauri::AppHandle` — add it via `tauri::Manager::state` or pass through. Easiest: add a `state.app_handle: tauri::AppHandle` slot set in `setup()` and read in commands.

- [ ] **Step 4: gate + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo build -p clearcam-desktop
git add desktop && git commit -m "feat(tauri): PreviewSink converts NV12 frames to JPEG and emits session://preview"
```

---

### Task 5: Mock-iPhone synthetic NV12 generator

**Files:**
- Modify: `desktop/tools/mock-iphone/src/main.rs`

- [ ] **Step 1: After handshake, spawn a writer task that pumps frames at 30 fps**

Add an `args.send_video: bool` flag (default true) and a `--width/--height/--fps` set. After `MEDIA_HELLO`, spawn:

```rust
let media_writer = Arc::new(tokio::sync::Mutex::new(media));
let mw = media_writer.clone();
tokio::spawn(async move {
    let w = args.width as usize;
    let h = args.height as usize;
    let frame_bytes = w * h * 3 / 2;
    let mut seq: u32 = 0;
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(1000 / u64::from(args.fps)));
    loop {
        tick.tick().await;
        let payload = synth_nv12(args.width, args.height, seq);
        let header = ccp_protocol::MediaHeader {
            media_type: ccp_protocol::MediaType::Video,
            flags: ccp_protocol::Flags::FULL_RANGE,
            codec: ccp_protocol::Codec::Raw,
            width: args.width, height: args.height, seq,
            pts_usec: session::keepalive::now_usec(),
            payload_len: frame_bytes as u32,
        };
        let mut guard = mw.lock().await;
        if mediapipeline::write_media_frame(&mut guard.writer, &header, &payload).await.is_err() {
            return;
        }
        seq = seq.wrapping_add(1);
    }
});

fn synth_nv12(w: u16, h: u16, t: u32) -> Vec<u8> {
    let w = w as usize; let h = h as usize;
    let mut buf = vec![0u8; w * h * 3 / 2];
    // moving horizontal gradient + a checkerboard for visual interest
    for y in 0..h {
        for x in 0..w {
            buf[y * w + x] = ((x + t as usize) & 0xFF) as u8;
        }
    }
    // chroma: a flat U=Cb≈128 and V=Cr varies with t — gives a hue shift over time
    let uv_off = w * h;
    for i in 0..(w * h / 4) {
        buf[uv_off + i * 2] = 128;
        buf[uv_off + i * 2 + 1] = ((128 + (t / 2) as i32) & 0xFF) as u8;
    }
    buf
}
```

> The existing `mock-iphone` Cargo.toml needs `mediapipeline = { path = "../../crates/mediapipeline" }`.

- [ ] **Step 2: Update args parser to accept `--width 1280 --height 720 --fps 30 --no-video`**

Defaults: 1280x720@30, video on.

- [ ] **Step 3: smoke**

```bash
cargo run -p mock-iphone -- --help
```

Should still exit 0.

- [ ] **Step 4: commit**

```bash
git add desktop/tools/mock-iphone && git commit -m "feat(mock-iphone): synthetic 720p NV12 generator at 30 fps"
```

---

### Task 6: Tauri preview event → React canvas

**Files:**
- Modify: `desktop/ui/src/lib/types.ts`
- Modify: `desktop/ui/src/lib/tauri.ts`
- Create: `desktop/ui/src/components/PreviewCanvas.tsx`
- Modify: `desktop/ui/src/App.tsx`

- [ ] **Step 1: Types**

```ts
export type PreviewFrame = {
  seq: number;
  ptsUsec: number;
  width: number;
  height: number;
  jpegBase64: string;
};
```

- [ ] **Step 2: Listener**

```ts
export async function onPreviewFrame(cb: (f: PreviewFrame) => void): Promise<UnlistenFn> {
  return listen<PreviewFrame>("session://preview", (e) => cb(e.payload));
}
```

- [ ] **Step 3: Canvas component (incrementally renders via createImageBitmap)**

```tsx
import { useEffect, useRef } from "react";
import { onPreviewFrame } from "../lib/tauri";

export default function PreviewCanvas() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  useEffect(() => {
    let off: (() => void) | undefined;
    (async () => {
      off = await onPreviewFrame(async (f) => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const bin = atob(f.jpegBase64);
        const buf = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i);
        const blob = new Blob([buf], { type: "image/jpeg" });
        const bitmap = await createImageBitmap(blob);
        canvas.width = bitmap.width;
        canvas.height = bitmap.height;
        const ctx = canvas.getContext("2d");
        ctx?.drawImage(bitmap, 0, 0);
        bitmap.close();
      });
    })();
    return () => { off?.(); };
  }, []);
  return (
    <div className="rounded-2xl border border-neutral-800 bg-neutral-900 p-4">
      <canvas
        ref={canvasRef}
        className="w-full rounded-lg bg-black"
        style={{ aspectRatio: "16/9" }}
      />
    </div>
  );
}
```

- [ ] **Step 4: Wire into App.tsx after DeviceCard**

Render `<PreviewCanvas />` when `showDevice` is true.

- [ ] **Step 5: UI gate + commit**

```bash
cd desktop/ui && pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add desktop/ui && git commit -m "feat(ui): live preview canvas listening to session://preview"
```

---

### Task 7: Camera switch — Tauri command + ControlPlane outbound

**Files:**
- Modify: `desktop/crates/session/src/controlplane.rs` (add `outbound: mpsc::Sender<ControlMessage>`)
- Modify: `desktop/crates/app/src/lib.rs` (expose `set_camera(id)` returning success)
- Modify: `desktop/src-tauri/src/commands.rs` (`set_camera` command)
- Modify: `desktop/ui/src/components/CameraPicker.tsx` (new)
- Modify: `desktop/ui/src/components/DeviceCard.tsx` (render picker)

- [ ] **Step 1: ControlPlane accepts outbound messages**

Add a parallel `outbound_rx: mpsc::Receiver<ControlMessage>` to `ControlPlane::run`, selected together with control.recv() and keepalive. Each outbound message gets a fresh `seq`.

- [ ] **Step 2: AppHandle::set_camera(id)**

```rust
pub async fn set_camera(&self, id: String) -> anyhow::Result<()> {
    let tx = self.outbound_tx.read().await.as_ref().cloned();
    tx.ok_or_else(|| anyhow::anyhow!("no active session"))?
        .send(ControlMessage::SetCamera(SetCamera { camera_id: id }))
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}
```

Track per-session `outbound_tx` in AppHandle. Phase 2 keeps a single Option<Sender> swapped on connect/disconnect.

- [ ] **Step 3: Tauri `set_camera`**

```rust
#[tauri::command]
pub async fn set_camera(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let guard = state.lock().await;
    match guard.as_ref() {
        Some(h) => h.set_camera(id).await.map_err(|e| format!("{e}")),
        None => Err("no session".into()),
    }
}
```

- [ ] **Step 4: UI**

`CameraPicker.tsx`:

```tsx
import type { CameraEntry } from "../lib/types";
import { setCamera } from "../lib/tauri";

export default function CameraPicker({
  cameras,
  activeId,
  onChange,
}: {
  cameras: CameraEntry[];
  activeId: string | null;
  onChange: (id: string) => void;
}) {
  return (
    <div className="flex gap-2 flex-wrap">
      {cameras.map((c) => (
        <button
          key={c.id}
          onClick={async () => { await setCamera(c.id); onChange(c.id); }}
          className={`rounded-full px-3 py-1 text-sm ${
            activeId === c.id
              ? "bg-emerald-700 text-emerald-50"
              : "bg-neutral-800 text-neutral-200 hover:bg-neutral-700"
          }`}
        >
          {c.name}
        </button>
      ))}
    </div>
  );
}
```

Add to `DeviceCard` between header and metrics.

- [ ] **Step 5: tauri.ts**

```ts
export async function setCamera(id: string): Promise<void> {
  return invoke("set_camera", { id });
}
```

- [ ] **Step 6: gate + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
cd desktop/ui && pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
git add desktop && git commit -m "feat: camera-switch from UI → SET_CAMERA on the wire"
```

---

# Section 2B — iOS capture engine

### Task 8: NV12 packer + MediaWire (TDD)

**Files:**
- Create: `ios/Sources/ClearCamCore/Encode/MediaWire.swift`
- Create: `ios/Sources/ClearCamCore/Encode/RawEncoder.swift`
- Create: `ios/Tests/ClearCamCoreTests/MediaWireTests.swift`

- [ ] **Step 1: MediaWire — byte-exact mirror of Rust MediaHeader**

```swift
import ClearCamProtocol
import Foundation

public enum MediaWire {
    public static let magic: UInt16 = 0xCC01
    public static let headerLen: Int = 28

    public static func encodeHeader(_ h: MediaHeader) -> Data {
        var data = Data(count: headerLen)
        data.replaceSubrange(0..<2, with: magic.bigEndian.bytes)
        data[2] = h.mediaType.rawValue
        data[3] = h.flags.rawValue
        data[4] = h.codec.rawValue
        // bytes 5..7 reserved = 0
        data.replaceSubrange(8..<10, with: h.width.bigEndian.bytes)
        data.replaceSubrange(10..<12, with: h.height.bigEndian.bytes)
        data.replaceSubrange(12..<16, with: h.seq.bigEndian.bytes)
        data.replaceSubrange(16..<24, with: h.ptsUsec.bigEndian.bytes)
        data.replaceSubrange(24..<28, with: h.payloadLen.bigEndian.bytes)
        return data
    }
}

private extension FixedWidthInteger {
    var bytes: [UInt8] {
        withUnsafeBytes(of: self, Array.init)
    }
}
```

The exact `MediaHeader` struct mirrors Rust:

```swift
public struct MediaHeader {
    public var mediaType: MediaType  // .video = 1
    public var flags: MediaFlags     // OptionSet matching Rust bitflags
    public var codec: MediaCodec     // .raw = 0
    public var width: UInt16
    public var height: UInt16
    public var seq: UInt32
    public var ptsUsec: UInt64
    public var payloadLen: UInt32
}
```

(MediaHeader is already in `ClearCamProtocol`; this just adds `MediaWire.encodeHeader`.)

- [ ] **Step 2: RawEncoder**

```swift
import AVFoundation
import ClearCamProtocol
import CoreVideo
import Foundation

public enum RawEncoderError: Error { case lockFailed, planeMissing }

public struct PackedFrame: Sendable {
    public let header: MediaHeader
    public let payload: Data
}

public enum RawEncoder {
    /// Pack a CVPixelBuffer (kCVPixelFormatType_420YpCbCr8BiPlanarFullRange) into
    /// the wire layout: MediaHeader (28 BE) + Y plane + interleaved CbCr plane.
    public static func packNV12(_ pixelBuffer: CVPixelBuffer, seq: UInt32, ptsUsec: UInt64) throws -> PackedFrame {
        guard CVPixelBufferLockBaseAddress(pixelBuffer, [.readOnly]) == kCVReturnSuccess else {
            throw RawEncoderError.lockFailed
        }
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, [.readOnly]) }
        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        guard CVPixelBufferGetPlaneCount(pixelBuffer) == 2,
              let yBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0),
              let uvBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 1)
        else { throw RawEncoderError.planeMissing }
        let yStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
        let uvStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 1)

        var payload = Data(count: width * height + width * height / 2)
        payload.withUnsafeMutableBytes { (dst: UnsafeMutableRawBufferPointer) in
            // Copy Y rows (tight pack to width).
            for r in 0..<height {
                let src = yBase.advanced(by: r * yStride)
                memcpy(dst.baseAddress!.advanced(by: r * width), src, width)
            }
            // Copy CbCr rows.
            let uvOff = width * height
            for r in 0..<(height / 2) {
                let src = uvBase.advanced(by: r * uvStride)
                memcpy(dst.baseAddress!.advanced(by: uvOff + r * width), src, width)
            }
        }

        let header = MediaHeader(
            mediaType: .video,
            flags: [.fullRange],
            codec: .raw,
            width: UInt16(width),
            height: UInt16(height),
            seq: seq,
            ptsUsec: ptsUsec,
            payloadLen: UInt32(payload.count)
        )
        return PackedFrame(header: header, payload: payload)
    }
}
```

- [ ] **Step 3: Tests — header byte-exact match Rust golden**

```swift
import XCTest
@testable import ClearCamCore
import ClearCamProtocol

final class MediaWireTests: XCTestCase {
    func testHeaderBytesMatchRustGolden() {
        let h = MediaHeader(
            mediaType: .video,
            flags: [.keyframe, .encoded, .fullRange],
            codec: .hevc,
            width: 1920, height: 1080,
            seq: 42,
            ptsUsec: 1234567890,
            payloadLen: 12345
        )
        let bytes = MediaWire.encodeHeader(h)
        let expected: [UInt8] = [
            0xCC, 0x01, 0x01, 0x07, 0x01,
            0x00, 0x00, 0x00,
            0x07, 0x80, 0x04, 0x38,
            0x00, 0x00, 0x00, 0x2A,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x96, 0x02, 0xD2,
            0x00, 0x00, 0x30, 0x39,
        ]
        XCTAssertEqual(Array(bytes), expected)
    }
}
```

- [ ] **Step 4: swift test + commit**

```bash
. scripts/swift-env.sh && cd ios && swift test --filter MediaWireTests
git add ios && git commit -m "feat(ios): MediaWire encoder + RawEncoder NV12 packer with golden test"
```

---

### Task 9: Camera enumerator + CaptureEngine (TDD)

**Files:**
- Create: `ios/Sources/ClearCamCore/Capture/CameraEnumerator.swift`
- Create: `ios/Sources/ClearCamCore/Capture/CaptureEngine.swift`
- Create: `ios/Sources/ClearCamCore/Capture/CapturedFrame.swift`
- Create: `ios/Tests/ClearCamCoreTests/CameraEnumeratorTests.swift`

- [ ] **Step 1: CameraEnumerator with injectable provider**

```swift
import AVFoundation
import ClearCamProtocol

public protocol CameraDiscovery: Sendable {
    func discover() -> [CameraEntry]
}

public struct AVCameraDiscovery: CameraDiscovery {
    public init() {}
    public func discover() -> [CameraEntry] {
        #if canImport(UIKit)
        let session = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInWideAngleCamera, .builtInUltraWideCamera, .builtInTelephotoCamera, .builtInTrueDepthCamera],
            mediaType: .video,
            position: .unspecified)
        return session.devices.compactMap { device in
            let position: CameraPosition = device.position == .front ? .front : .back
            let f = device.activeFormat.formatDescription
            let dim = CMVideoFormatDescriptionGetDimensions(f)
            let maxFps = device.formats.flatMap { $0.videoSupportedFrameRateRanges }.map { $0.maxFrameRate }.max() ?? 30
            return CameraEntry(
                id: device.uniqueID,
                name: device.localizedName,
                position: position,
                maxWidth: UInt16(dim.width),
                maxHeight: UInt16(dim.height),
                maxFps: UInt16(maxFps),
                supportedFormats: ["nv12"]
            )
        }
        #else
        return []
        #endif
    }
}

public struct StubCameraDiscovery: CameraDiscovery {
    public let entries: [CameraEntry]
    public init(_ entries: [CameraEntry]) { self.entries = entries }
    public func discover() -> [CameraEntry] { entries }
}
```

- [ ] **Step 2: CapturedFrame + CaptureEngine skeleton**

```swift
import AVFoundation
import Foundation

public final class CaptureEngine: NSObject {
    private let session = AVCaptureSession()
    private let videoOutput = AVCaptureVideoDataOutput()
    private let queue = DispatchQueue(label: "clearcam.capture", qos: .userInteractive)
    public typealias FrameHandler = @Sendable (CVPixelBuffer, CMTime) -> Void
    private var onFrame: FrameHandler?

    public override init() { super.init() }

    public func start(cameraId: String, onFrame: @escaping FrameHandler) throws {
        self.onFrame = onFrame
        guard let device = AVCaptureDevice(uniqueID: cameraId) else { throw NSError(domain: "ClearCam", code: 1) }
        session.beginConfiguration()
        session.inputs.forEach(session.removeInput)
        let input = try AVCaptureDeviceInput(device: device)
        if session.canAddInput(input) { session.addInput(input) }
        videoOutput.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange]
        videoOutput.setSampleBufferDelegate(self, queue: queue)
        if !session.outputs.contains(videoOutput), session.canAddOutput(videoOutput) {
            session.addOutput(videoOutput)
        }
        session.commitConfiguration()
        queue.async { [weak self] in self?.session.startRunning() }
    }

    public func setCamera(_ cameraId: String) throws {
        guard let device = AVCaptureDevice(uniqueID: cameraId) else { return }
        session.beginConfiguration()
        session.inputs.forEach(session.removeInput)
        let input = try AVCaptureDeviceInput(device: device)
        if session.canAddInput(input) { session.addInput(input) }
        session.commitConfiguration()
    }

    public func stop() {
        queue.async { [weak self] in self?.session.stopRunning() }
    }
}

extension CaptureEngine: AVCaptureVideoDataOutputSampleBufferDelegate {
    public func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let pb = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        onFrame?(pb, pts)
    }
}
```

- [ ] **Step 3: CameraEnumerator tests**

```swift
final class CameraEnumeratorTests: XCTestCase {
    func testStubProvidesEntries() {
        let entry = CameraEntry(id: "wide", name: "Wide", position: .back, maxWidth: 4032, maxHeight: 3024, maxFps: 60, supportedFormats: ["nv12"])
        let d = StubCameraDiscovery([entry])
        XCTAssertEqual(d.discover().count, 1)
        XCTAssertEqual(d.discover()[0].id, "wide")
    }
}
```

(The AVFoundation path is not unit-testable headless; covered by manual on-device step.)

- [ ] **Step 4: gate + commit**

```bash
. scripts/swift-env.sh && cd ios && swift build && swift test
git add ios && git commit -m "feat(ios): CameraEnumerator + CaptureEngine skeleton"
```

---

### Task 10: SessionController emits camera list & wires SET_CAMERA

**Files:**
- Modify: `ios/Sources/ClearCamCore/Session/SessionController.swift`
- Add: `ios/Tests/ClearCamCoreTests/CameraSwitchTests.swift`

- [ ] **Step 1: After `connect()` succeeds, before starting telemetry, send CAMERA_LIST + start CaptureEngine on the first back camera**

In `connect`:

```swift
let cameras = cameraDiscovery.discover()
self.cameras = cameras
seq += 1
try await controlChannel.send(ControlEnvelope(
    seq: seq, ack: nil,
    body: .cameraList(CameraList(cameras: cameras))))
if let first = cameras.first(where: { $0.position == .back }) ?? cameras.first {
    try captureEngine.start(cameraId: first.id) { [weak self] pb, pts in
        Task { await self?.onCapturedFrame(pb, pts) }
    }
    activeCameraId = first.id
}
```

- [ ] **Step 2: `onCapturedFrame` packs and sends over media channel**

```swift
private var mediaSeq: UInt32 = 0
private func onCapturedFrame(_ pb: CVPixelBuffer, _ pts: CMTime) async {
    mediaSeq &+= 1
    let ptsUsec = UInt64(max(0, CMTimeGetSeconds(pts)) * 1_000_000)
    guard let packed = try? RawEncoder.packNV12(pb, seq: mediaSeq, ptsUsec: ptsUsec) else { return }
    var data = MediaWire.encodeHeader(packed.header)
    data.append(packed.payload)
    _ = try? await mediaChannel.sendRaw(data)
}
```

(`ControlStream.sendRaw(_:)` must be added — a passthrough writer that bypasses framing.)

- [ ] **Step 3: Listen for SET_CAMERA**

Add a `runLoop()` task that drains incoming control envelopes after handshake and dispatches:

```swift
case .setCamera(let sc):
    try? captureEngine.setCamera(sc.cameraId)
    activeCameraId = sc.cameraId
    seq += 1
    try? await controlChannel.send(ControlEnvelope(
        seq: seq, ack: env.seq,
        body: .cameraState(CameraState(activeCameraId: sc.cameraId, appliedFormat: "raw nv12"))))
```

- [ ] **Step 4: Test**

```swift
final class CameraSwitchTests: XCTestCase {
    func testRespondsToSetCamera() async throws {
        // Use MemoryPipe + StubCameraDiscovery + a mock CaptureEngineStub that records setCamera() calls.
        // assert: after server sends SET_CAMERA(id="tele"), controller emits CAMERA_STATE(activeCameraId="tele").
    }
}
```

- [ ] **Step 5: gate + commit**

```bash
. scripts/swift-env.sh && cd ios && swift test
git add ios && git commit -m "feat(ios): SessionController emits CAMERA_LIST + reacts to SET_CAMERA"
```

---

### Task 11: Manual on-device acceptance

- [ ] **Step 1:** Open `ios/ClearCam.xcodeproj`, set team, run on device.
- [ ] **Step 2:** Start desktop (`cargo tauri dev`). Scan QR.
- [ ] **Step 3:** Verify live preview at 720p30. Switch cameras from the UI — preview swaps within 1 s.
- [ ] **Step 4:** Record observations in `plans/phase-2-raw-video-preview.md` acceptance log.

---

### Task 12: CI + tag

- [ ] **Step 1:** No CI changes if the new crates compile clean — the existing matrix covers them.
- [ ] **Step 2:** Push, wait CI, then `git tag v0.3.0-phase2 && git push --tags`.
- [ ] **Step 3:** Write retrospective at the bottom of this plan.

---

## Acceptance log

### 2026-05-28 — Section 2A + 2B automated acceptance

- Local Rust gates: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` (rustc 1.95.0), `cargo test --workspace --all-targets` — **all green**.
  - New tests: `sink::frame::tests::*` (2), `mediapipeline::*` (6), `app::tests::*` (2), `app/tests/e2e_mock_iphone.rs` (1), `app/tests/e2e_raw_video.rs` (1, drives mock-iphone NV12 → MediaPipeline → CapturingSink), `app/tests/e2e_camera_switch.rs` (1, SET_CAMERA → wire envelope), Tauri preview unit test (1).
- Local UI: `pnpm typecheck && pnpm lint && pnpm format:check && pnpm build` — green.
- Local iOS: `swift build` + `swift test` — **34 tests passing**.
  - New: `RawEncoderTests` (3), `CameraEnumeratorTests` (1), `CameraSwitchTests` (1).
- `bash scripts/regen-xcode.sh` regenerates the Xcode project from `project.yml` after the new ClearCamAppKit sources.

### Section 2 — manual on-device check (deferred to user)

After this push, the user should:

```bash
cd desktop && cargo tauri dev
# in another terminal, copy cport/mport/token from the UI and run:
cargo run --release -p mock-iphone -- --host 127.0.0.1 --cport <CPORT> --mport <MPORT> --token <TOKEN>
# desktop UI should display: live 720p preview canvas, four-button camera picker,
# device card with live battery telemetry; clicking another camera button sends
# SET_CAMERA but the mock-iphone currently doesn't react with new content (it
# keeps generating the synthetic stream — Phase 4 will switch real iPhone cams).
```

For on-device iPhone testing: `bash scripts/regen-xcode.sh`, open `ios/ClearCam.xcodeproj`, set DEVELOPMENT_TEAM, run on device with the Camera + Local Network permissions.

## Retrospective

### What landed

**Rust crates**
- `sink` (was a Phase-0 stub): `Frame` type (NV12 planes via `bytes::Bytes`, `pts_usec`, `seq`, `Codec`, `full_range`) + `FrameSink` async trait. Two unit tests.
- `mediapipeline`: `read_media_frame`/`write_media_frame` over `MediaStream`, `split_nv12` (zero-copy slice into Y/UV via `Bytes::slice`), `MediaPipeline::spawn` which loops the socket and fans frames out to registered sinks. Drop-oldest is delegated to each sink's `submit()` impl. 6 unit tests covering happy path + payload-too-large + plane-size-mismatch + end-to-end fanout to a capturing sink.
- `app`: now owns `Arc<RwLock<Vec<Arc<dyn FrameSink>>>>` and `Arc<RwLock<Option<OutboundSender>>>`. `register_sink()` adds a sink before the next handshake; `set_camera(id)` puts SET_CAMERA on the active session's outbound queue. e2e tests verify both paths.
- `session::controlplane`: added an outbound `mpsc<ControlMessage>` queue; `run_with_outbound()` selects between keepalive, recv(), and outbound to stamp seq and write envelopes. `run()` preserved as a no-outbound wrapper for backwards compatibility.
- `tools/mock-iphone`: synthetic 1280×720 NV12 generator running at the requested fps (default 30). Animated Y gradient + slowly shifting Cr.

**Tauri**
- `PreviewSink`: receives `Frame`s, throttles to ≤30 emits/s, does NV12 → RGB → JPEG (quality 70) on `spawn_blocking`, emits `session://preview` with `jpegBase64`. Unit-tested with a 16×16 frame (asserts JPEG SOI marker).
- `set_camera` Tauri command forwards to `AppHandle::set_camera`.

**UI**
- `PreviewCanvas` uses `createImageBitmap` from base64-decoded JPEG and paints into a `<canvas>` aspect-ratio 16:9.
- `CameraPicker` renders pill buttons per `CameraEntry`, calls `setCamera()` on click.
- `App.tsx` shows `PreviewCanvas` and `DeviceCard` once `state.kind === "ready"` and a device is known.

**iOS (`ClearCamCore` + `ClearCamAppKit`)**
- `RawEncoder.packNV12(y, uv, width, height, seq, ptsUsec, fullRange)` — pure bytes-in/bytes-out NV12 packer. `PackedFrame.wireBytes()` returns header+payload ready to write.
- `CVPixelBufferPacker` (iOS-only, `#if canImport(CoreVideo)`): locks a `CVPixelBuffer`, copies into tight buffers honoring stride, calls `RawEncoder.packNV12`.
- `CameraDiscovery` protocol with `StubCameraDiscovery` (tests) and `AVCameraDiscovery` (production iOS).
- `CaptureEngine` protocol with `AVCaptureEngine` (production iOS, AVCaptureVideoDataOutput driver).
- `ControlStream.sendRaw(_:)` — write raw bytes onto the underlying socket, bypassing length-prefix framing. Used for media frames after MEDIA_HELLO.
- `SessionController`:
  - Optional `cameraDiscovery` + `captureEngine` (mutable, attach-able post-init via `attachCameraSystem()`).
  - `publishCameraListAndStart()` emits CAMERA_LIST and starts the first back camera.
  - `sendCapturedNV12(y, uv, width, height, ptsUsec, fullRange)` packs and writes one record on the media channel.
  - `startPump()` reacts to PING (PONG) and SET_CAMERA (engine.setCamera + CAMERA_STATE).
- `ConnectViewModel` wires `AVCaptureEngine`'s `onFrame` callback to `controller.sendCapturedNV12()`.

### Deviations from the plan

1. **Drop-oldest policy** — the plan said the pipeline would own a bounded mpsc; I pushed the responsibility into each `FrameSink` impl instead (PreviewSink throttles by time, future VirtualCameraSink will do its own bounded queue). This kept the pipeline itself stateless and easier to reason about for Phase 3+ when multiple sinks exist.
2. **`MediaWire` module not needed** — `MediaHeader.encode()` already lives in `ClearCamProtocol` from Phase 0 and emits the exact wire bytes. The plan's separate `MediaWire.swift` would have been a duplicate. Tests still cover the bytes via `RawEncoderTests`.
3. **`attachCameraSystem` post-handshake** — the plan called `engine.start()` directly inside `SessionController.connect()`. That created a chicken-and-egg in production: the `AVCaptureEngine`'s `onFrame` closure needs a reference to the controller, but the controller is what owns the engine. I made `cameraDiscovery`/`captureEngine` mutable on the actor and exposed `attachCameraSystem + publishCameraListAndStart` so production code can `controller = SessionController(...); engine = AVCaptureEngine { ... controller ... }; controller.attachCameraSystem(engine, discovery); controller.publishCameraListAndStart()`. Tests still construct everything up-front and exercise the same path through `connect()` which auto-publishes when discovery is wired.
4. **`fast_image_resize` dropped** — the `image` crate already has `imageops::resize` and downscaling 720p → 720 is a single op per frame at 30 fps; an extra dep was overkill for the wins.

### Open questions / Phase 3+ wiring

- **Virtual camera sink.** Phase 3 adds a `FrameSink` that publishes into macOS CMIO. The current `register_sink()` API supports it directly; the sink just lands in the `Vec<Arc<dyn FrameSink>>` alongside `PreviewSink`.
- **Bitrate accounting** — `Telemetry::sent_bitrate_kbps` is still 0 on the iPhone side. Phase 4 adds the encoder and that metric.
- **AVCaptureSession-iPhone-only smoke test** — the AVCaptureEngine path is `#if canImport(UIKit)`-gated and cannot run via `swift test` on macOS. It is covered by the iOS on-device manual check.
- **`CapturingSink` lock granularity** — for Phase 6 we should switch from `Mutex<Vec<Frame>>` to a single-frame `tokio::sync::watch` for the preview path, so we never accumulate even briefly.
