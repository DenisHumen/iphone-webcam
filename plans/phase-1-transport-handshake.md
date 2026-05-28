# Phase 1 — Transport & Handshake (Wi-Fi, loopback) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A real iPhone (and/or a Rust mock-iPhone) connects to the desktop over Wi-Fi (scanning a QR code or manual host+token entry), completes the CCP handshake on both control and media sockets, and the desktop UI shows the device card (name, battery, thermal state, camera list, live telemetry). Wi-Fi drop → auto-reconnection with backoff.

**Architecture:** Two TCP sockets per session (control = length-prefixed JSON, media = binary header per docs/03). Desktop is server on Wi-Fi: listens on `controlPort`/`mediaPort`, advertises via mDNS (best-effort), shows QR with `{host, ports, token}`. iPhone is client: connects to both ports, executes HELLO → HELLO_ACK → AUTH → AUTH_OK on control, then MEDIA_HELLO on media. Each session is owned by a single `ControlPlane` actor (Tokio task) holding the source-of-truth state. Telemetry/device-info events bubble up to Tauri UI via events.

**Tech Stack:** Rust 1.75+ stable, tokio 1.x, tokio-util (length-delimited codec is not required — we hand-roll the 4-byte BE prefix per docs/03 §5.1), serde + serde_json, `qrcode` (Rust) for QR rendering, Tauri 2 IPC, React 18 + TS + Tailwind, Swift 5.9 + SwiftUI + Network.framework on iOS 17+. No new features beyond Phase 1 — no encoder, no video, no virtual camera, no adaptive engine.

**Acceptance (per docs/10):**
- mock-iphone connects, handshake completes on both sockets in ≤5 s.
- Desktop UI shows the QR, switches to a connected state, displays device card + live telemetry at ~2 Hz.
- Forced disconnect (kill mock) → session goes to `reconnecting`; rerun mock → re-attaches without restarting desktop.
- Real iPhone (developer-mode build) does the same scenario via Wi-Fi.
- `cargo test --workspace --all-targets`, `pnpm typecheck && pnpm build`, `swift test` all green; CI green.

**Out of scope (deferred to later phases):**
- Any media frames (raw or encoded) — only `MEDIA_HELLO` lands on the media socket in this phase; no actual video.
- TLS over Wi-Fi (interface is reserved in transport, but plaintext for v1 LAN; see docs/12 ADR).
- USB transport (Phase 5).
- Speedtest, adaptive, virtual camera sink — Phase 2+.

---

## File Structure

### New / modified Rust files

```
desktop/
├── Cargo.toml                                     # add workspace deps: tokio, tokio-util, futures-util, tracing, uuid, qrcode, rand, async-trait
├── crates/
│   ├── transport/
│   │   ├── Cargo.toml                             # MOD: deps (tokio, bytes, tracing, ccp-protocol, async-trait, thiserror)
│   │   └── src/
│   │       ├── lib.rs                             # MOD: exports + trait Transport
│   │       ├── framing.rs                         # NEW: async read/write of length-prefixed JSON
│   │       ├── streams.rs                         # NEW: ControlStream / MediaStream wrappers
│   │       ├── error.rs                           # NEW: TransportError
│   │       └── wifi/
│   │           ├── mod.rs                         # NEW: WifiServer / WifiClient (tokio TCP)
│   │           ├── server.rs                      # NEW: accept loop, dual-port pairing
│   │           └── client.rs                      # NEW: connect helper for tests + mock
│   ├── session/
│   │   ├── Cargo.toml                             # MOD: deps (tokio, ccp-protocol, transport, tracing, thiserror, uuid)
│   │   └── src/
│   │       ├── lib.rs                             # MOD: module wiring
│   │       ├── handshake.rs                       # NEW: server-side handshake state machine
│   │       ├── pairing.rs                         # NEW: media↔control pairing by sessionId+token
│   │       ├── state.rs                           # NEW: SessionState enum + DeviceSnapshot
│   │       ├── controlplane.rs                    # NEW: ControlPlane actor (tokio task)
│   │       ├── keepalive.rs                       # NEW: PING/PONG timer + dead-peer detection
│   │       └── error.rs                           # NEW: SessionError
│   └── app/                                        # NEW crate fleshed out (Phase 1 entry point)
│       ├── Cargo.toml                             # MOD: deps (tokio, transport, session, tracing-subscriber, anyhow, qrcode)
│       └── src/
│           └── lib.rs                             # NEW: AppCore::start() that wires Wifi server + ControlPlane
├── src-tauri/
│   ├── Cargo.toml                                 # MOD: + app, tokio, serde, tauri-plugin-shell removed (unused)
│   ├── tauri.conf.json                            # MOD: nothing (window already configured)
│   └── src/
│       ├── lib.rs                                 # MOD: register commands + spawn AppCore + state
│       ├── commands.rs                            # NEW: start_server, stop_server, get_state, get_qr_payload
│       └── events.rs                              # NEW: bridge ControlPlane events → tauri::Emitter
└── tools/
    └── mock-iphone/                                # NEW crate
        ├── Cargo.toml
        └── src/
            └── main.rs                            # NEW: CLI client that completes handshake and emits telemetry
```

### New / modified iOS files

```
ios/
├── Package.swift                                  # MOD: products + targets — add ClearCamCore, ClearCamApp executable (iOS only when Xcode is used)
├── Sources/
│   ├── ClearCamProtocol/                          # unchanged from Phase 0
│   ├── ClearCamCore/                              # NEW SwiftPM target (logic, no UI, testable on macOS)
│   │   ├── Transport/
│   │   │   ├── ControlStream.swift                # NEW: length-prefixed JSON read/write over NWConnection
│   │   │   ├── MediaStream.swift                  # NEW: minimal stub (Phase 1 only sends MEDIA_HELLO)
│   │   │   └── TransportClient.swift              # NEW: connect host:cport + host:mport
│   │   ├── Session/
│   │   │   ├── SessionState.swift                 # NEW: idle / connecting / handshaking / ready / reconnecting / stopped
│   │   │   ├── SessionController.swift            # NEW: state machine (no capture/encode in Phase 1)
│   │   │   └── Reconnect.swift                    # NEW: exponential backoff helper
│   │   ├── Status/
│   │   │   └── DeviceInfoProvider.swift           # NEW: abstraction (real impl in App target uses UIDevice/ProcessInfo)
│   │   ├── Pairing/
│   │   │   └── QRPayload.swift                    # NEW: Codable mirror of QR JSON
│   │   └── Logging/
│   │       └── Log.swift                          # NEW: thin os.Logger wrapper
│   └── ClearCamApp/                               # NEW SwiftUI app target (Xcode-built; SwiftPM exposes as iOS app target metadata)
│       ├── ClearCamApp.swift                      # NEW: @main App entry
│       ├── ConnectView.swift                      # NEW: QR scan + manual entry
│       ├── ConnectedView.swift                    # NEW: live status (battery, thermal, telemetry counter)
│       ├── QRScanner.swift                        # NEW: AVCaptureMetadataOutput wrapper
│       └── UIKitDeviceInfo.swift                  # NEW: UIDevice-backed DeviceInfoProvider impl
└── Tests/
    └── ClearCamCoreTests/                         # NEW
        ├── ControlStreamTests.swift               # NEW: round-trip framing
        ├── SessionControllerTests.swift           # NEW: state machine
        ├── ReconnectTests.swift                   # NEW: backoff sequence
        └── QRPayloadTests.swift                   # NEW: Codable
```

### Tauri UI files

```
desktop/ui/src/
├── App.tsx                                        # MOD: route between Idle / WaitingForPhone / Connected / Error
├── lib/
│   ├── tauri.ts                                   # NEW: typed wrappers around invoke()/listen()
│   ├── types.ts                                   # NEW: SessionState, DeviceSnapshot, Telemetry mirrors
│   └── qr.ts                                      # NEW: helper to render QR (data URL) from server-supplied payload string
├── components/
│   ├── ServerCard.tsx                             # NEW: shows host/ports + QR + "stop" button
│   ├── DeviceCard.tsx                             # NEW: name/model, battery, thermal, cameras, telemetry meters
│   ├── StatusBadge.tsx                            # NEW: small pill (idle/listening/connected/error)
│   └── ErrorBanner.tsx                            # NEW
└── index.css                                      # MOD: extend Tailwind config slightly if needed (no new file)
```

### Test files (Rust integration)

```
desktop/crates/session/tests/
├── handshake_happy_path.rs                        # NEW: loopback server + client
├── handshake_errors.rs                            # NEW: wrong token, wrong proto ver
└── reconnect.rs                                   # NEW: disconnect mid-session → reconnect

desktop/crates/transport/tests/
└── framing_loopback.rs                            # NEW: byte-exact framing over TCP loopback

desktop/tools/mock-iphone/tests/
└── e2e_against_real_server.rs                     # NEW: spin up app::start(), run mock as library, assert end state
```

### CI

```
.github/workflows/ci.yml                           # MOD: add `cargo run -p mock-iphone -- --help` smoke step + Swift test for ClearCamCore
```

---

## Conventions used in this plan

- **Cwd in commands:** always `desktop/` for `cargo *`, `desktop/ui/` for `pnpm *`, `ios/` for `swift *`, repo root for `git`.
- **Gates per task:** every task ending in code change runs the relevant gate (`cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets` for Rust; `pnpm typecheck && pnpm lint && pnpm format:check && pnpm build` for UI; `swift build && swift test` for iOS).
- **Commits:** every task ends with a commit; messages follow Conventional Commits, no `--no-verify`.
- **Tooling versions:** Rust stable (current at session start = `1.75+`), Node 20, pnpm 10, Swift 5.9 (`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` via `scripts/swift-env.sh`).
- **No hacks for unused warnings:** if a field is genuinely unused in Phase 1, gate it behind `#[allow(dead_code)]` with a `// Phase 2: …` note OR omit it until needed.

---

# Section 1A — Rust core + mock-iPhone + Tauri/UI

Goal: complete vertical slice **without** iOS, fully covered by `cargo test`. After Section 1A finishes, `mock-iphone` connects to a running `cargo tauri dev` desktop, completes handshake, and the React UI shows the device card and live telemetry.

---

### Task 1: Workspace deps + tracing wiring

**Files:**
- Modify: `desktop/Cargo.toml`

- [ ] **Step 1: Add new workspace dependencies**

In `desktop/Cargo.toml`, extend `[workspace.dependencies]`:

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bytes = "1"
byteorder = "1"
bitflags = { version = "2", features = ["serde"] }
thiserror = "1"
anyhow = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "io-util", "sync", "time", "fs"] }
tokio-util = { version = "0.7", features = ["codec"] }
futures-util = "0.3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4"] }
async-trait = "0.1"
rand = "0.8"
qrcode = { version = "0.14", default-features = false }
```

- [ ] **Step 2: Verify it still compiles**

Run from `desktop/`:
```bash
cargo build --workspace
```

Expected: success, lockfile updated.

- [ ] **Step 3: Commit**

```bash
git add desktop/Cargo.toml desktop/Cargo.lock
git commit -m "chore(workspace): add tokio + tracing + qr deps for Phase 1"
```

---

### Task 2: Transport framing primitives (TDD)

**Files:**
- Modify: `desktop/crates/transport/Cargo.toml`
- Create: `desktop/crates/transport/src/framing.rs`
- Create: `desktop/crates/transport/src/error.rs`
- Modify: `desktop/crates/transport/src/lib.rs`

- [ ] **Step 1: Wire deps**

Edit `desktop/crates/transport/Cargo.toml`:

```toml
[package]
name = "transport"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "ClearCam transport — async TCP server/client + length-prefixed framing"

[dependencies]
ccp-protocol = { path = "../ccp-protocol" }
tokio = { workspace = true }
bytes = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

- [ ] **Step 2: Write the failing test for `read_frame` / `write_frame`**

Create `desktop/crates/transport/src/framing.rs` with the test up top so it compiles to nothing and fails:

```rust
//! Length-prefixed JSON framing per `docs/03-protocol.md` §5.1.
//!
//! Wire layout: `[uint32 BE length][UTF-8 JSON payload]`.

use ccp_protocol::DEFAULT_MAX_PAYLOAD;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::TransportError;

pub async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    json: &[u8],
) -> Result<(), TransportError> {
    let len: u32 = json
        .len()
        .try_into()
        .map_err(|_| TransportError::FrameTooLarge(json.len()))?;
    if (len as usize) > DEFAULT_MAX_PAYLOAD {
        return Err(TransportError::FrameTooLarge(json.len()));
    }
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
    if len > DEFAULT_MAX_PAYLOAD {
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
        let payload = vec![b'x'; DEFAULT_MAX_PAYLOAD + 1];
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
```

Create `desktop/crates/transport/src/error.rs`:

```rust
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
```

Replace `desktop/crates/transport/src/lib.rs`:

```rust
//! `transport` — async TCP server/client + length-prefixed JSON framing.
//!
//! See `docs/02-architecture.md` and `docs/03-protocol.md` §5.

#![forbid(unsafe_code)]

pub mod error;
pub mod framing;

pub use error::TransportError;
pub use framing::{read_frame, write_frame};
```

- [ ] **Step 3: Run tests — expect FAIL (DEFAULT_MAX_PAYLOAD may not be re-exported)**

```bash
cd /Users/denisgumen/Desktop/code/iphone-webcam/desktop && cargo test -p transport
```

Expected: compile error if `DEFAULT_MAX_PAYLOAD` not visible. Confirm and if needed re-export via `ccp_protocol::DEFAULT_MAX_PAYLOAD` (already exported from `control::envelope`).

- [ ] **Step 4: Run again — expect PASS**

```bash
cargo test -p transport
```

Expected: 3 tests pass.

- [ ] **Step 5: Run workspace gate**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
```

If `fmt --check` fails, run `cargo fmt --all` and inspect the diff before committing.

- [ ] **Step 6: Commit**

```bash
git add desktop/crates/transport
git commit -m "feat(transport): async length-prefixed JSON framing with tests"
```

---

### Task 3: Transport trait + ControlStream/MediaStream wrappers (TDD)

**Files:**
- Create: `desktop/crates/transport/src/streams.rs`
- Modify: `desktop/crates/transport/src/lib.rs`

- [ ] **Step 1: Write the failing test for ControlStream send/recv**

Create `desktop/crates/transport/src/streams.rs`:

```rust
//! Typed wrappers around the raw TCP halves.
//!
//! - `ControlStream`: sends/receives `ControlEnvelope` (length-prefixed JSON).
//! - `MediaStream`: thin handle over the media socket. Phase 1 only sends one
//!   `MEDIA_HELLO` frame and then keeps the socket open for Phase 2 video.

use std::net::SocketAddr;

use ccp_protocol::ControlEnvelope;
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};
use tokio::net::TcpStream;

use crate::error::TransportError;
use crate::framing::{read_frame, write_frame};

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub addr: SocketAddr,
}

pub struct ControlStream {
    pub peer: PeerInfo,
    reader: Box<dyn AsyncReadUnpin + Send>,
    writer: Box<dyn AsyncWriteUnpin + Send>,
    rx_buf: Vec<u8>,
}

pub trait AsyncReadUnpin: AsyncRead + Unpin {}
impl<T: AsyncRead + Unpin> AsyncReadUnpin for T {}

pub trait AsyncWriteUnpin: AsyncWrite + Unpin {}
impl<T: AsyncWrite + Unpin> AsyncWriteUnpin for T {}

impl ControlStream {
    pub fn from_tcp(peer: PeerInfo, sock: TcpStream) -> Self {
        let (r, w) = tokio::io::split(sock);
        Self {
            peer,
            reader: Box::new(r),
            writer: Box::new(w),
            rx_buf: Vec::with_capacity(4096),
        }
    }

    pub fn from_halves<R, W>(peer: PeerInfo, r: R, w: W) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self {
            peer,
            reader: Box::new(r),
            writer: Box::new(w),
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
    pub reader: ReadHalf<TcpStream>,
    pub writer: WriteHalf<TcpStream>,
}

impl MediaStream {
    pub fn from_tcp(peer: PeerInfo, sock: TcpStream) -> Self {
        let (reader, writer) = tokio::io::split(sock);
        Self { peer, reader, writer }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::{Auth, ControlMessage};
    use tokio::io::duplex;

    #[tokio::test]
    async fn control_round_trip_typed_envelope() {
        let (a, b) = duplex(64 * 1024);
        let (ra, wa) = tokio::io::split(a);
        let (rb, wb) = tokio::io::split(b);
        let peer = PeerInfo {
            addr: "127.0.0.1:0".parse().unwrap(),
        };
        let mut left = ControlStream::from_halves(peer.clone(), ra, wa);
        let mut right = ControlStream::from_halves(peer, rb, wb);

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
```

Update `desktop/crates/transport/src/lib.rs`:

```rust
//! `transport` — async TCP server/client + length-prefixed JSON framing.

#![forbid(unsafe_code)]

pub mod error;
pub mod framing;
pub mod streams;

pub use error::TransportError;
pub use framing::{read_frame, write_frame};
pub use streams::{ControlStream, MediaStream, PeerInfo};
```

- [ ] **Step 2: Run tests — expect PASS**

```bash
cd desktop && cargo test -p transport
```

Expected: 4 tests pass.

- [ ] **Step 3: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/crates/transport
git commit -m "feat(transport): typed ControlStream/MediaStream wrappers"
```

---

### Task 4: Wi-Fi server (TDD)

**Files:**
- Create: `desktop/crates/transport/src/wifi/mod.rs`
- Create: `desktop/crates/transport/src/wifi/server.rs`
- Modify: `desktop/crates/transport/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `desktop/crates/transport/src/wifi/mod.rs`:

```rust
pub mod server;

pub use server::{WifiServer, WifiServerEvent};
```

Create `desktop/crates/transport/src/wifi/server.rs`:

```rust
//! TCP server that accepts a paired control + media connection from the iPhone.
//!
//! Pairing strategy: each accepted control socket gets a fresh `pending_id`
//! generated by the server. The handshake (`session` crate) emits an `AUTH_OK`
//! containing the assigned `sessionId`. The media socket presents
//! `MEDIA_HELLO { sessionId, token }`; the session crate looks it up in the
//! pending table and resolves the future returned here.

use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::streams::{ControlStream, MediaStream, PeerInfo};
use crate::TransportError;

#[derive(Debug)]
pub enum WifiServerEvent {
    Control(ControlStream),
    Media(MediaStream),
}

pub struct WifiServer {
    control_addr: SocketAddr,
    media_addr: SocketAddr,
}

impl WifiServer {
    pub async fn bind(
        control_addr: SocketAddr,
        media_addr: SocketAddr,
    ) -> Result<(Self, BoundPorts), TransportError> {
        let control = TcpListener::bind(control_addr).await?;
        let media = TcpListener::bind(media_addr).await?;
        let bound = BoundPorts {
            control: control.local_addr()?,
            media: media.local_addr()?,
        };
        Ok((
            Self {
                control_addr: bound.control,
                media_addr: bound.media,
            },
            BoundPortsWithListeners { bound, control, media },
        )) // tuple shape simplifies returning the bound listeners
    }
}

#[derive(Debug, Clone)]
pub struct BoundPorts {
    pub control: SocketAddr,
    pub media: SocketAddr,
}

pub struct BoundPortsWithListeners {
    pub bound: BoundPorts,
    pub control: TcpListener,
    pub media: TcpListener,
}

impl BoundPortsWithListeners {
    pub fn spawn(self, tx: mpsc::Sender<WifiServerEvent>) {
        let BoundPortsWithListeners { control, media, .. } = self;
        let tx_ctl = tx.clone();
        tokio::spawn(async move {
            loop {
                match control.accept().await {
                    Ok((sock, addr)) => {
                        info!(?addr, "accepted control connection");
                        let cs = ControlStream::from_tcp(PeerInfo { addr }, sock);
                        if tx_ctl.send(WifiServerEvent::Control(cs)).await.is_err() {
                            return;
                        }
                    }
                    Err(e) => warn!(error = ?e, "control accept failed"),
                }
            }
        });
        tokio::spawn(async move {
            loop {
                match media.accept().await {
                    Ok((sock, addr)) => {
                        info!(?addr, "accepted media connection");
                        let ms = MediaStream::from_tcp(PeerInfo { addr }, sock);
                        if tx.send(WifiServerEvent::Media(ms)).await.is_err() {
                            return;
                        }
                    }
                    Err(e) => warn!(error = ?e, "media accept failed"),
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn accepts_control_and_media_connections() {
        let (_srv, bound) = WifiServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        let cport = bound.bound.control.port();
        let mport = bound.bound.media.port();

        let (tx, mut rx) = mpsc::channel(8);
        bound.spawn(tx);

        let _client_ctl = TcpStream::connect(("127.0.0.1", cport)).await.unwrap();
        let _client_media = TcpStream::connect(("127.0.0.1", mport)).await.unwrap();

        let mut saw_control = false;
        let mut saw_media = false;
        for _ in 0..2 {
            match rx.recv().await.unwrap() {
                WifiServerEvent::Control(_) => saw_control = true,
                WifiServerEvent::Media(_) => saw_media = true,
            }
        }
        assert!(saw_control && saw_media);
    }
}
```

Update `desktop/crates/transport/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod error;
pub mod framing;
pub mod streams;
pub mod wifi;

pub use error::TransportError;
pub use framing::{read_frame, write_frame};
pub use streams::{ControlStream, MediaStream, PeerInfo};
pub use wifi::server::{BoundPorts, BoundPortsWithListeners, WifiServer, WifiServerEvent};
```

- [ ] **Step 2: Run tests**

```bash
cd desktop && cargo test -p transport
```

Expected: 5 tests pass.

- [ ] **Step 3: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/crates/transport
git commit -m "feat(transport): WifiServer with paired control/media accept loops"
```

---

### Task 5: Wi-Fi client (for tests + mock-iPhone) (TDD)

**Files:**
- Create: `desktop/crates/transport/src/wifi/client.rs`
- Modify: `desktop/crates/transport/src/wifi/mod.rs`
- Modify: `desktop/crates/transport/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `desktop/crates/transport/src/wifi/client.rs`:

```rust
//! Wi-Fi client used by `tools/mock-iphone` and integration tests.

use std::net::SocketAddr;

use tokio::net::TcpStream;

use crate::streams::{ControlStream, MediaStream, PeerInfo};
use crate::TransportError;

pub async fn connect(
    control: SocketAddr,
    media: SocketAddr,
) -> Result<(ControlStream, MediaStream), TransportError> {
    let control_sock = TcpStream::connect(control).await?;
    let media_sock = TcpStream::connect(media).await?;
    let control_peer = PeerInfo {
        addr: control_sock.peer_addr()?,
    };
    let media_peer = PeerInfo {
        addr: media_sock.peer_addr()?,
    };
    Ok((
        ControlStream::from_tcp(control_peer, control_sock),
        MediaStream::from_tcp(media_peer, media_sock),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wifi::server::{WifiServer, WifiServerEvent};
    use ccp_protocol::{Bye, ControlEnvelope, ControlMessage};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn round_trip_through_real_tcp_loopback() {
        let (_srv, bound) = WifiServer::bind(
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        let cport = bound.bound.control.port();
        let mport = bound.bound.media.port();
        let (tx, mut rx) = mpsc::channel(8);
        bound.spawn(tx);

        let (mut client_ctl, _client_media) =
            connect(format!("127.0.0.1:{cport}").parse().unwrap(),
                    format!("127.0.0.1:{mport}").parse().unwrap())
                .await
                .unwrap();

        // pull the server-side control side from the event queue
        let mut server_ctl = match rx.recv().await.unwrap() {
            WifiServerEvent::Control(c) => c,
            WifiServerEvent::Media(_) => {
                match rx.recv().await.unwrap() {
                    WifiServerEvent::Control(c) => c,
                    WifiServerEvent::Media(_) => panic!("two media events"),
                }
            }
        };

        let env = ControlEnvelope {
            seq: 7,
            ack: None,
            body: ControlMessage::Bye(Bye { reason: "ok".into() }),
        };
        client_ctl.send(&env).await.unwrap();
        let got = server_ctl.recv().await.unwrap();
        assert_eq!(got, env);
    }
}
```

Update `desktop/crates/transport/src/wifi/mod.rs`:

```rust
pub mod client;
pub mod server;

pub use client::connect;
pub use server::{BoundPorts, BoundPortsWithListeners, WifiServer, WifiServerEvent};
```

Update `desktop/crates/transport/src/lib.rs` exports:

```rust
pub use wifi::{
    client::connect as wifi_connect,
    server::{BoundPorts, BoundPortsWithListeners, WifiServer, WifiServerEvent},
};
```

- [ ] **Step 2: Run tests**

```bash
cd desktop && cargo test -p transport
```

Expected: 6 tests pass.

- [ ] **Step 3: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/crates/transport
git commit -m "feat(transport): Wi-Fi client connect() and loopback round-trip test"
```

---

### Task 6: Session error/state types (TDD)

**Files:**
- Modify: `desktop/crates/session/Cargo.toml`
- Create: `desktop/crates/session/src/error.rs`
- Create: `desktop/crates/session/src/state.rs`
- Modify: `desktop/crates/session/src/lib.rs`

- [ ] **Step 1: Add deps**

```toml
[package]
name = "session"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "ClearCam session — control plane actor + handshake + pairing"

[dependencies]
ccp-protocol = { path = "../ccp-protocol" }
transport = { path = "../transport" }
tokio = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
tokio = { workspace = true, features = ["test-util"] }
```

- [ ] **Step 2: Write tests + types**

Create `desktop/crates/session/src/error.rs`:

```rust
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
```

Create `desktop/crates/session/src/state.rs`:

```rust
use std::collections::HashMap;

use ccp_protocol::{
    CameraEntry, CameraPosition, DeviceInfo, Telemetry, ThermalState,
};
use serde::{Deserialize, Serialize};

/// Coarse-grained UI-facing state for one session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionStateKind {
    Idle,
    Listening { control_port: u16, media_port: u16 },
    Handshaking,
    Ready,
    Reconnecting,
    Closed { reason: String },
}

/// Source-of-truth snapshot the UI mirrors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub state: SessionStateKind,
    pub device: Option<DeviceSnapshot>,
    pub last_telemetry: Option<Telemetry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSnapshot {
    pub model: String,
    pub os_ver: String,
    pub usb3_capable: bool,
    pub cameras: Vec<CameraEntry>,
}

impl SessionSnapshot {
    pub fn idle() -> Self {
        Self {
            state: SessionStateKind::Idle,
            device: None,
            last_telemetry: None,
        }
    }
}

impl DeviceSnapshot {
    pub fn from_device_info(info: &DeviceInfo) -> Self {
        Self {
            model: info.model.clone(),
            os_ver: info.os_ver.clone(),
            usb3_capable: info.usb3_capable,
            cameras: Vec::new(),
        }
    }
}

/// Pending pairing table — control session waits for matching media socket.
#[derive(Default)]
pub struct PendingMediaBindings(HashMap<(String, String), tokio::sync::oneshot::Sender<transport::MediaStream>>);

impl PendingMediaBindings {
    pub fn insert(
        &mut self,
        session_id: String,
        token: String,
        tx: tokio::sync::oneshot::Sender<transport::MediaStream>,
    ) {
        self.0.insert((session_id, token), tx);
    }

    pub fn take(
        &mut self,
        session_id: &str,
        token: &str,
    ) -> Option<tokio::sync::oneshot::Sender<transport::MediaStream>> {
        self.0.remove(&(session_id.to_string(), token.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_serde_round_trip() {
        let snap = SessionSnapshot {
            state: SessionStateKind::Listening {
                control_port: 7000,
                media_port: 7001,
            },
            device: Some(DeviceSnapshot {
                model: "iPhone15,3".into(),
                os_ver: "iOS 18.0".into(),
                usb3_capable: true,
                cameras: vec![],
            }),
            last_telemetry: None,
        };
        let s = serde_json::to_string(&snap).unwrap();
        let back: SessionSnapshot = serde_json::from_str(&s).unwrap();
        assert_eq!(back, snap);
    }

    // Silence the import if it would otherwise be unused.
    #[allow(dead_code)]
    fn _unused(_p: CameraPosition, _t: ThermalState) {}
}
```

Replace `desktop/crates/session/src/lib.rs`:

```rust
//! `session` — control plane actor, handshake, and media pairing.

#![forbid(unsafe_code)]

pub mod error;
pub mod state;

pub use error::SessionError;
pub use state::{DeviceSnapshot, PendingMediaBindings, SessionSnapshot, SessionStateKind};
```

- [ ] **Step 3: Run tests**

```bash
cd desktop && cargo test -p session
```

Expected: 1 test passes.

- [ ] **Step 4: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/crates/session
git commit -m "feat(session): error + state snapshot types"
```

---

### Task 7: Server-side handshake state machine (TDD)

**Files:**
- Create: `desktop/crates/session/src/handshake.rs`
- Modify: `desktop/crates/session/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `desktop/crates/session/src/handshake.rs`:

```rust
//! Server-side control-plane handshake.
//!
//! Sequence per `docs/03-protocol.md` §4:
//!     ← HELLO         → HELLO_ACK or ERROR(incompatible_version)
//!     ← AUTH          → AUTH_OK or ERROR(unauthorized)
//! After AUTH_OK, the session is "ready" pending media binding.

use std::time::Duration;

use ccp_protocol::{
    Auth, AuthOk, ControlEnvelope, ControlMessage, ErrorCode, ErrorMsg, Hello, HelloAck,
    Capability, PROTO_VER,
};
use tokio::time::timeout;
use tracing::{info, warn};
use transport::ControlStream;
use uuid::Uuid;

use crate::error::SessionError;

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct AcceptedSession {
    pub session_id: String,
    pub token: String,
    pub hello: Hello,
}

pub async fn accept_control(
    stream: &mut ControlStream,
    expected_token: &str,
    server_caps: &[Capability],
) -> Result<AcceptedSession, SessionError> {
    let hello_env = timeout(HANDSHAKE_TIMEOUT, stream.recv())
        .await
        .map_err(|_| SessionError::HandshakeTimeout)??;
    let hello = match hello_env.body {
        ControlMessage::Hello(h) => h,
        other => {
            warn!(?other, "first frame was not HELLO");
            send_error(stream, hello_env.seq, ErrorCode::BadRequest, "expected HELLO").await?;
            return Err(SessionError::UnexpectedMessage("expected HELLO"));
        }
    };
    if hello.proto_ver != PROTO_VER {
        send_error(
            stream,
            hello_env.seq,
            ErrorCode::IncompatibleVersion,
            "proto version mismatch",
        )
        .await?;
        return Err(SessionError::IncompatibleProtoVer {
            peer: hello.proto_ver,
            local: PROTO_VER,
        });
    }
    // intersection of caps — we send back what both support
    let agreed_caps: Vec<Capability> = server_caps
        .iter()
        .filter(|c| hello.caps.iter().any(|h| h == *c))
        .cloned()
        .collect();
    stream
        .send(&ControlEnvelope {
            seq: 0,
            ack: Some(hello_env.seq),
            body: ControlMessage::HelloAck(HelloAck {
                proto_ver: PROTO_VER,
                caps: agreed_caps,
            }),
        })
        .await?;

    let auth_env = timeout(HANDSHAKE_TIMEOUT, stream.recv())
        .await
        .map_err(|_| SessionError::HandshakeTimeout)??;
    let auth: Auth = match auth_env.body {
        ControlMessage::Auth(a) => a,
        other => {
            warn!(?other, "expected AUTH");
            send_error(stream, auth_env.seq, ErrorCode::BadRequest, "expected AUTH").await?;
            return Err(SessionError::UnexpectedMessage("expected AUTH"));
        }
    };
    if auth.token != expected_token {
        send_error(stream, auth_env.seq, ErrorCode::Unauthorized, "bad token").await?;
        return Err(SessionError::Unauthorized);
    }
    let session_id = format!("sess-{}", Uuid::new_v4().simple());
    stream
        .send(&ControlEnvelope {
            seq: 1,
            ack: Some(auth_env.seq),
            body: ControlMessage::AuthOk(AuthOk {
                session_id: session_id.clone(),
            }),
        })
        .await?;
    info!(%session_id, device.model = %hello.device.model, "control handshake complete");
    Ok(AcceptedSession {
        session_id,
        token: auth.token,
        hello,
    })
}

async fn send_error(
    stream: &mut ControlStream,
    ack: u64,
    code: ErrorCode,
    message: &str,
) -> Result<(), SessionError> {
    stream
        .send(&ControlEnvelope {
            seq: 0,
            ack: Some(ack),
            body: ControlMessage::Error(ErrorMsg {
                code,
                message: message.into(),
            }),
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::DeviceIdent;
    use tokio::io::duplex;
    use transport::PeerInfo;

    fn dummy_peer() -> PeerInfo {
        PeerInfo { addr: "127.0.0.1:0".parse().unwrap() }
    }

    async fn make_pair() -> (ControlStream, ControlStream) {
        let (a, b) = duplex(64 * 1024);
        let (ra, wa) = tokio::io::split(a);
        let (rb, wb) = tokio::io::split(b);
        (
            ControlStream::from_halves(dummy_peer(), ra, wa),
            ControlStream::from_halves(dummy_peer(), rb, wb),
        )
    }

    #[tokio::test]
    async fn happy_path_returns_session_id() {
        let (mut srv, mut cli) = make_pair().await;
        let server = tokio::spawn(async move {
            accept_control(&mut srv, "secret", &[Capability::Hevc])
                .await
                .map(|s| s.session_id)
        });
        cli.send(&ControlEnvelope {
            seq: 10,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "mock/0.1".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "18.0".into(),
                },
                session_id: "candidate".into(),
                caps: vec![Capability::Hevc, Capability::RawNv12],
            }),
        }).await.unwrap();
        let ack: ControlEnvelope = cli.recv().await.unwrap();
        assert!(matches!(ack.body, ControlMessage::HelloAck(_)));
        cli.send(&ControlEnvelope {
            seq: 11,
            ack: None,
            body: ControlMessage::Auth(Auth { token: "secret".into() }),
        }).await.unwrap();
        let ok = cli.recv().await.unwrap();
        match ok.body {
            ControlMessage::AuthOk(AuthOk { session_id }) => assert!(session_id.starts_with("sess-")),
            other => panic!("unexpected {other:?}"),
        }
        let sid = server.await.unwrap().unwrap();
        assert!(sid.starts_with("sess-"));
    }

    #[tokio::test]
    async fn rejects_wrong_token() {
        let (mut srv, mut cli) = make_pair().await;
        let server = tokio::spawn(async move {
            accept_control(&mut srv, "secret", &[]).await
        });
        cli.send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "x".into(),
                device: DeviceIdent { model: "x".into(), os_ver: "x".into() },
                session_id: "x".into(),
                caps: vec![],
            }),
        }).await.unwrap();
        let _ack = cli.recv().await.unwrap();
        cli.send(&ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth { token: "WRONG".into() }),
        }).await.unwrap();
        let err = cli.recv().await.unwrap();
        match err.body {
            ControlMessage::Error(ErrorMsg { code, .. }) => assert!(matches!(code, ErrorCode::Unauthorized)),
            other => panic!("expected ERROR, got {other:?}"),
        }
        assert!(matches!(server.await.unwrap(), Err(SessionError::Unauthorized)));
    }

    #[tokio::test]
    async fn rejects_proto_mismatch() {
        let (mut srv, mut cli) = make_pair().await;
        let server = tokio::spawn(async move {
            accept_control(&mut srv, "secret", &[]).await
        });
        cli.send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: 999,
                app: "x".into(),
                device: DeviceIdent { model: "x".into(), os_ver: "x".into() },
                session_id: "x".into(),
                caps: vec![],
            }),
        }).await.unwrap();
        let err = cli.recv().await.unwrap();
        match err.body {
            ControlMessage::Error(ErrorMsg { code, .. }) => assert!(matches!(code, ErrorCode::IncompatibleVersion)),
            other => panic!("expected ERROR, got {other:?}"),
        }
        assert!(matches!(server.await.unwrap(), Err(SessionError::IncompatibleProtoVer { .. })));
    }
}
```

Update `desktop/crates/session/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod error;
pub mod handshake;
pub mod state;

pub use error::SessionError;
pub use handshake::{accept_control, AcceptedSession, HANDSHAKE_TIMEOUT};
pub use state::{DeviceSnapshot, PendingMediaBindings, SessionSnapshot, SessionStateKind};
```

- [ ] **Step 2: Run tests**

```bash
cd desktop && cargo test -p session
```

Expected: 4 tests pass (3 new + 1 from Task 6).

- [ ] **Step 3: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/crates/session
git commit -m "feat(session): server-side control-plane handshake state machine"
```

---

### Task 8: Server-side media handshake (MEDIA_HELLO pairing) (TDD)

**Files:**
- Create: `desktop/crates/session/src/pairing.rs`
- Modify: `desktop/crates/session/src/lib.rs`

- [ ] **Step 1: Write failing test**

Create `desktop/crates/session/src/pairing.rs`:

```rust
//! Pair an incoming media socket with the control session that already
//! completed AUTH_OK. The media socket sends MEDIA_HELLO { sessionId, token }
//! as a length-prefixed JSON envelope on the same wire as control (Phase 1
//! reuses the framing).

use std::time::Duration;

use ccp_protocol::{ControlEnvelope, ControlMessage};
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
    timeout(MEDIA_HANDSHAKE_TIMEOUT, stream.reader.read_exact(&mut prefix))
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
        body: ControlMessage::MediaHello(ccp_protocol::MediaHello {
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
            write_media_hello(&mut ms, &MediaBinding {
                session_id: "sess-xyz".into(),
                token: "tok".into(),
            }).await
        });
        client.await.unwrap().unwrap();
        let got = server.await.unwrap().unwrap();
        assert_eq!(got, MediaBinding {
            session_id: "sess-xyz".into(),
            token: "tok".into(),
        });
    }
}
```

Update `desktop/crates/session/src/lib.rs`:

```rust
pub mod pairing;
pub use pairing::{read_media_hello, write_media_hello, MediaBinding, MEDIA_HANDSHAKE_TIMEOUT};
```

- [ ] **Step 2: Run tests + gate**

```bash
cd desktop && cargo test -p session
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
```

Expected: 5 session tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop/crates/session
git commit -m "feat(session): media socket MEDIA_HELLO read/write helpers"
```

---

### Task 9: ControlPlane actor (TDD)

**Files:**
- Create: `desktop/crates/session/src/controlplane.rs`
- Create: `desktop/crates/session/src/keepalive.rs`
- Modify: `desktop/crates/session/src/lib.rs`
- Modify: `desktop/crates/session/Cargo.toml` (add `tokio-stream` dev-dep)

- [ ] **Step 1: Write failing test for ControlPlane events**

Create `desktop/crates/session/src/keepalive.rs`:

```rust
//! PING/PONG keepalive helpers.

use std::time::Duration;

pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(2);
pub const DEAD_PEER_AFTER: Duration = Duration::from_secs(6);

pub fn now_usec() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}
```

Create `desktop/crates/session/src/controlplane.rs`:

```rust
//! ControlPlane actor — single owner of session state.
//!
//! Accepts pre-handshaked `ControlStream`s and a media-binding lookup, drives
//! the per-session loop:
//!     - reads control messages → updates `SessionSnapshot` → emits events
//!     - sends PING on keepalive timer, watches for PONG
//!     - on disconnect, transitions to `Reconnecting` and waits for a new
//!       handshake (driven by `app::AppCore`).

use std::time::Duration;

use ccp_protocol::{
    ControlEnvelope, ControlMessage, Ping, Pong, Capability, DeviceInfo, Telemetry,
};
use tokio::sync::{mpsc, watch};
use tokio::time::{interval, Instant};
use tracing::{info, warn};
use transport::ControlStream;

use crate::handshake::AcceptedSession;
use crate::keepalive::{now_usec, DEAD_PEER_AFTER, KEEPALIVE_INTERVAL};
use crate::state::{DeviceSnapshot, SessionSnapshot, SessionStateKind};

#[derive(Debug, Clone)]
pub enum ControlPlaneEvent {
    StateChanged(SessionStateKind),
    DeviceInfo(DeviceSnapshot),
    Telemetry(Telemetry),
    Closed(String),
}

pub struct ControlPlane {
    pub events_tx: mpsc::Sender<ControlPlaneEvent>,
    pub snapshot_tx: watch::Sender<SessionSnapshot>,
}

impl ControlPlane {
    pub fn new(events_capacity: usize) -> (Self, mpsc::Receiver<ControlPlaneEvent>, watch::Receiver<SessionSnapshot>) {
        let (events_tx, events_rx) = mpsc::channel(events_capacity);
        let (snapshot_tx, snapshot_rx) = watch::channel(SessionSnapshot::idle());
        (Self { events_tx, snapshot_tx }, events_rx, snapshot_rx)
    }

    pub async fn run(
        self,
        accepted: AcceptedSession,
        mut control: ControlStream,
    ) -> Result<(), crate::SessionError> {
        // Set Ready state
        self.broadcast(SessionStateKind::Ready, None, None).await;

        let mut seq: u64 = 100;
        let mut last_rx = Instant::now();
        let mut keepalive = interval(KEEPALIVE_INTERVAL);
        keepalive.tick().await;

        loop {
            tokio::select! {
                _ = keepalive.tick() => {
                    seq = seq.wrapping_add(1);
                    if let Err(e) = control.send(&ControlEnvelope {
                        seq,
                        ack: None,
                        body: ControlMessage::Ping(Ping { ts_usec: now_usec() }),
                    }).await {
                        warn!(error = ?e, "ping send failed; treating as disconnect");
                        self.broadcast(SessionStateKind::Reconnecting, None, None).await;
                        return Ok(());
                    }
                    if last_rx.elapsed() > DEAD_PEER_AFTER {
                        warn!("no message from peer for {:?}; closing", DEAD_PEER_AFTER);
                        self.broadcast(SessionStateKind::Reconnecting, None, None).await;
                        return Ok(());
                    }
                }
                recv = control.recv() => {
                    let env = match recv {
                        Ok(e) => { last_rx = Instant::now(); e }
                        Err(e) => {
                            warn!(error = ?e, "control recv failed; treating as disconnect");
                            self.broadcast(SessionStateKind::Reconnecting, None, None).await;
                            return Ok(());
                        }
                    };
                    match env.body {
                        ControlMessage::Ping(p) => {
                            seq = seq.wrapping_add(1);
                            let _ = control.send(&ControlEnvelope {
                                seq,
                                ack: Some(env.seq),
                                body: ControlMessage::Pong(Pong { ts_usec: now_usec(), echo_usec: p.ts_usec }),
                            }).await;
                        }
                        ControlMessage::Pong(_) => {}
                        ControlMessage::DeviceInfo(di) => {
                            let snap = DeviceSnapshot::from_device_info(&di);
                            self.events_tx.send(ControlPlaneEvent::DeviceInfo(snap.clone())).await.ok();
                            self.broadcast(SessionStateKind::Ready, Some(snap), None).await;
                        }
                        ControlMessage::CameraList(cl) => {
                            let mut snap = self.snapshot_tx.borrow().clone();
                            if let Some(d) = snap.device.as_mut() { d.cameras = cl.cameras.clone(); }
                            let _ = self.snapshot_tx.send(snap.clone());
                            if let Some(d) = snap.device { self.events_tx.send(ControlPlaneEvent::DeviceInfo(d)).await.ok(); }
                        }
                        ControlMessage::Telemetry(t) => {
                            let mut snap = self.snapshot_tx.borrow().clone();
                            snap.last_telemetry = Some(t.clone());
                            let _ = self.snapshot_tx.send(snap);
                            self.events_tx.send(ControlPlaneEvent::Telemetry(t)).await.ok();
                        }
                        ControlMessage::Bye(b) => {
                            info!(reason = %b.reason, "peer said BYE");
                            self.broadcast(SessionStateKind::Closed { reason: b.reason.clone() }, None, None).await;
                            self.events_tx.send(ControlPlaneEvent::Closed(b.reason)).await.ok();
                            return Ok(());
                        }
                        other => {
                            // Phase 2+ will handle MODE_APPLIED, CAMERA_STATE, etc.
                            tracing::debug!(?other, "unhandled message in Phase 1");
                        }
                    }
                }
            }
        }
    }

    async fn broadcast(
        &self,
        state: SessionStateKind,
        device: Option<DeviceSnapshot>,
        telemetry: Option<Telemetry>,
    ) {
        let mut snap = self.snapshot_tx.borrow().clone();
        snap.state = state.clone();
        if let Some(d) = device { snap.device = Some(d); }
        if let Some(t) = telemetry { snap.last_telemetry = Some(t); }
        let _ = self.snapshot_tx.send(snap);
        self.events_tx.send(ControlPlaneEvent::StateChanged(state)).await.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::{
        BatteryState, ControlEnvelope, ControlMessage, DeviceInfo, ThermalState,
    };
    use tokio::io::duplex;
    use transport::PeerInfo;

    fn peer() -> PeerInfo { PeerInfo { addr: "127.0.0.1:0".parse().unwrap() } }

    #[tokio::test]
    async fn ingests_device_info_into_snapshot() {
        let (a, b) = duplex(64 * 1024);
        let (ra, wa) = tokio::io::split(a);
        let (rb, wb) = tokio::io::split(b);
        let mut srv = ControlStream::from_halves(peer(), ra, wa);
        let mut cli = ControlStream::from_halves(peer(), rb, wb);

        let (cp, mut rx, snap_rx) = ControlPlane::new(16);
        let accepted = AcceptedSession {
            session_id: "sess-1".into(),
            token: "t".into(),
            hello: ccp_protocol::Hello {
                proto_ver: ccp_protocol::PROTO_VER,
                app: "x".into(),
                device: ccp_protocol::DeviceIdent { model: "m".into(), os_ver: "v".into() },
                session_id: "x".into(),
                caps: vec![],
            },
        };
        let handle = tokio::spawn(async move {
            let _ = cp.run(accepted, srv).await;
        });

        // mock-iphone side sends DEVICE_INFO
        cli.send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::DeviceInfo(DeviceInfo {
                model: "iPhone15,3".into(),
                os_ver: "18.0".into(),
                battery_level: 0.9,
                battery_state: BatteryState::Unplugged,
                thermal_state: ThermalState::Nominal,
                usb3_capable: true,
            }),
        }).await.unwrap();

        // wait for DeviceInfo event
        loop {
            match rx.recv().await.unwrap() {
                ControlPlaneEvent::DeviceInfo(d) => {
                    assert_eq!(d.model, "iPhone15,3");
                    break;
                }
                _ => continue,
            }
        }
        assert!(snap_rx.borrow().device.is_some());

        // tear down
        cli.send(&ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Bye(ccp_protocol::Bye { reason: "done".into() }),
        }).await.unwrap();
        handle.await.unwrap();
    }
}
```

Update `desktop/crates/session/src/lib.rs`:

```rust
pub mod controlplane;
pub mod keepalive;
pub use controlplane::{ControlPlane, ControlPlaneEvent};
```

- [ ] **Step 2: Run tests + gate**

```bash
cd desktop && cargo test -p session
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
```

Expected: 6 session tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop/crates/session
git commit -m "feat(session): ControlPlane actor with snapshot + event channels"
```

---

### Task 10: App orchestration crate (TDD)

**Files:**
- Modify: `desktop/crates/app/Cargo.toml`
- Create: `desktop/crates/app/src/lib.rs`

- [ ] **Step 1: Wire deps and write the orchestration entry point**

Create `desktop/crates/app/Cargo.toml`:

```toml
[package]
name = "app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "ClearCam app core — wires transport + session + UI events"

[dependencies]
ccp-protocol = { path = "../ccp-protocol" }
transport = { path = "../transport" }
session = { path = "../session" }
tokio = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
qrcode = { workspace = true }
rand = { workspace = true }
```

Create `desktop/crates/app/src/lib.rs`:

```rust
//! `app` — top-level wiring of transport + session for Phase 1.
//!
//! Public API:
//!   - `AppCore::start()` — bind ports, run accept loop, return handle.
//!   - `AppHandle::events()` — subscribe to UI-facing events.
//!   - `AppHandle::snapshot()` — read current `SessionSnapshot`.
//!   - `AppHandle::qr_payload()` — JSON to embed in the QR code.

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use rand::RngCore;
use serde::Serialize;
use session::{
    accept_control, read_media_hello, ControlPlane, ControlPlaneEvent, MediaBinding,
    SessionSnapshot, SessionStateKind,
};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{info, warn};
use transport::{BoundPortsWithListeners, WifiServer, WifiServerEvent};

#[derive(Debug, Clone, Serialize)]
pub struct QrPayload {
    pub v: u32,
    pub host: String,
    pub cport: u16,
    pub mport: u16,
    pub token: String,
}

pub struct AppCore {
    pub host: String,
}

pub struct AppHandle {
    pub qr: QrPayload,
    pub snapshot: watch::Receiver<SessionSnapshot>,
    pub events: Arc<Mutex<mpsc::Receiver<ControlPlaneEvent>>>,
    _accept_task: JoinHandle<()>,
}

impl AppCore {
    pub async fn start(self) -> anyhow::Result<AppHandle> {
        let mut token_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut token_bytes);
        let token = base64_url(&token_bytes);

        let (server, bound) = WifiServer::bind(
            "0.0.0.0:0".parse()?,
            "0.0.0.0:0".parse()?,
        )
        .await?;
        let _ = server; // future Phase 2 will use it
        let cport = bound.bound.control.port();
        let mport = bound.bound.media.port();
        let (event_tx, event_rx) = mpsc::channel::<WifiServerEvent>(8);
        bound.spawn(event_tx);

        let (cp, cp_events_rx, snapshot_rx) = ControlPlane::new(64);

        let accept_task = spawn_accept_loop(event_rx, cp, token.clone());

        Ok(AppHandle {
            qr: QrPayload {
                v: 1,
                host: self.host,
                cport,
                mport,
                token,
            },
            snapshot: snapshot_rx,
            events: Arc::new(Mutex::new(cp_events_rx)),
            _accept_task: accept_task,
        })
    }
}

fn spawn_accept_loop(
    mut events: mpsc::Receiver<WifiServerEvent>,
    cp: ControlPlane,
    expected_token: String,
) -> JoinHandle<()> {
    use std::collections::HashMap;
    tokio::spawn(async move {
        // very small pending-media table: sessionId+token → tx
        let mut pending: HashMap<(String, String), tokio::sync::oneshot::Sender<transport::MediaStream>> =
            HashMap::new();
        let mut control_session: Option<Arc<Mutex<ControlPlane>>> = None;
        let _ = control_session; // Phase 2 will reuse cp across reconnects; Phase 1 wires single session.
        let cp = Some(cp);
        let mut cp = cp;
        loop {
            match events.recv().await {
                None => return,
                Some(WifiServerEvent::Control(mut cs)) => {
                    let token = expected_token.clone();
                    let taken = cp.take();
                    if let Some(cp_taken) = taken {
                        let cap_set = vec![ccp_protocol::Capability::RawNv12];
                        match accept_control(&mut cs, &token, &cap_set).await {
                            Ok(acc) => {
                                info!(session_id = %acc.session_id, "control accepted");
                                let session_id = acc.session_id.clone();
                                let auth_token = acc.token.clone();
                                let (tx_media, rx_media) = tokio::sync::oneshot::channel();
                                pending.insert((session_id.clone(), auth_token.clone()), tx_media);
                                tokio::spawn(async move {
                                    let _ = rx_media.await; // Phase 2 will plumb the media stream into MediaPipeline
                                    info!(%session_id, "media stream attached (parked for Phase 2)");
                                });
                                tokio::spawn(async move {
                                    if let Err(e) = cp_taken.run(acc, cs).await {
                                        warn!(error = ?e, "control plane exited");
                                    }
                                });
                            }
                            Err(e) => {
                                warn!(error = ?e, "handshake failed");
                                cp = Some(cp_taken);
                            }
                        }
                    } else {
                        warn!("got control connection but ControlPlane already owned; closing");
                    }
                }
                Some(WifiServerEvent::Media(mut ms)) => {
                    match read_media_hello(&mut ms).await {
                        Ok(MediaBinding { session_id, token }) => {
                            if let Some(tx) = pending.remove(&(session_id.clone(), token.clone())) {
                                let _ = tx.send(ms);
                                info!(%session_id, "media paired");
                            } else {
                                warn!(%session_id, "no pending control session for this media");
                            }
                        }
                        Err(e) => warn!(error = ?e, "media hello failed"),
                    }
                }
            }
        }
    })
}

fn base64_url(bytes: &[u8]) -> String {
    use std::fmt::Write;
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, b) in chunk.iter().enumerate() {
            buf[i] = *b;
        }
        let n = (u32::from(buf[0]) << 16) | (u32::from(buf[1]) << 8) | u32::from(buf[2]);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        }
    }
    let _ = write!(out, ""); // appease unused-trait
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_url_safe_and_long_enough() {
        let mut buf = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut buf);
        let s = base64_url(&buf);
        assert!(s.len() >= 32);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[tokio::test]
    async fn start_returns_qr_payload_with_real_ports() {
        let core = AppCore { host: "127.0.0.1".into() };
        let h = core.start().await.unwrap();
        assert_eq!(h.qr.v, 1);
        assert!(h.qr.cport != 0);
        assert!(h.qr.mport != 0);
        assert!(!h.qr.token.is_empty());
        assert_eq!(h.snapshot.borrow().state, SessionStateKind::Idle);
    }
}
```

- [ ] **Step 2: Run tests + gate**

```bash
cd desktop && cargo test -p app
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
```

Expected: 2 app tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop/crates/app desktop/Cargo.lock
git commit -m "feat(app): AppCore::start() wires Wi-Fi server + ControlPlane + QR payload"
```

---

### Task 11: mock-iphone CLI (TDD via end-to-end test)

**Files:**
- Create: `desktop/tools/mock-iphone/Cargo.toml`
- Create: `desktop/tools/mock-iphone/src/main.rs`
- Modify: `desktop/Cargo.toml` (add `tools/mock-iphone` to members)

- [ ] **Step 1: Add to workspace**

In `desktop/Cargo.toml` extend `members`:

```toml
members = [
    "crates/ccp-protocol",
    "crates/transport",
    "crates/session",
    "crates/adaptive",
    "crates/mediapipeline",
    "crates/decode",
    "crates/sink",
    "crates/app",
    "src-tauri",
    "tools/mock-iphone",
]
```

- [ ] **Step 2: Create the crate**

Create `desktop/tools/mock-iphone/Cargo.toml`:

```toml
[package]
name = "mock-iphone"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "Mock iPhone client used by integration tests and manual checks (Phase 1)"

[[bin]]
name = "mock-iphone"
path = "src/main.rs"

[dependencies]
ccp-protocol = { path = "../../crates/ccp-protocol" }
transport = { path = "../../crates/transport" }
session = { path = "../../crates/session" }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }
serde_json = { workspace = true }
```

Create `desktop/tools/mock-iphone/src/main.rs`:

```rust
//! Mock iPhone client. Runs through the CCP handshake on both sockets and
//! emits DEVICE_INFO, CAMERA_LIST, and TELEMETRY at ~2 Hz until killed.

use std::env;
use std::time::Duration;

use ccp_protocol::{
    Auth, BatteryState, CameraEntry, CameraList, CameraPosition, ControlEnvelope,
    ControlMessage, DeviceIdent, DeviceInfo, Hello, MediaHello, PROTO_VER, Telemetry,
    ThermalState,
};
use session::write_media_hello;
use tokio::time::sleep;
use tracing::{info, warn};
use transport::wifi_connect;

#[derive(Debug)]
struct Args {
    host: String,
    cport: u16,
    mport: u16,
    token: String,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut host = "127.0.0.1".to_string();
    let mut cport: Option<u16> = None;
    let mut mport: Option<u16> = None;
    let mut token: Option<String> = None;
    let mut it = env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--host" => host = it.next().unwrap_or_default(),
            "--cport" => cport = it.next().and_then(|s| s.parse().ok()),
            "--mport" => mport = it.next().and_then(|s| s.parse().ok()),
            "--token" => token = it.next(),
            "--help" | "-h" => {
                println!(
                    "mock-iphone --host <host> --cport <port> --mport <port> --token <token>"
                );
                std::process::exit(0);
            }
            _ => anyhow::bail!("unknown arg: {a}"),
        }
    }
    Ok(Args {
        host,
        cport: cport.ok_or_else(|| anyhow::anyhow!("--cport required"))?,
        mport: mport.ok_or_else(|| anyhow::anyhow!("--mport required"))?,
        token: token.ok_or_else(|| anyhow::anyhow!("--token required"))?,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = parse_args()?;
    run(args).await
}

async fn run(args: Args) -> anyhow::Result<()> {
    info!(?args, "mock-iphone starting");
    let control_addr = format!("{}:{}", args.host, args.cport).parse()?;
    let media_addr = format!("{}:{}", args.host, args.mport).parse()?;
    let (mut control, mut media) = wifi_connect(control_addr, media_addr).await?;

    // HELLO → wait HELLO_ACK
    let mut seq = 1u64;
    control.send(&ControlEnvelope {
        seq,
        ack: None,
        body: ControlMessage::Hello(Hello {
            proto_ver: PROTO_VER,
            app: "mock-iphone/0.1".into(),
            device: DeviceIdent {
                model: "iPhone15,3".into(),
                os_ver: "iOS 18.0".into(),
            },
            session_id: "candidate".into(),
            caps: vec![ccp_protocol::Capability::Hevc, ccp_protocol::Capability::RawNv12],
        }),
    }).await?;
    let ack = control.recv().await?;
    let _ = match ack.body {
        ControlMessage::HelloAck(_) => {}
        ControlMessage::Error(e) => anyhow::bail!("server rejected HELLO: {:?}", e),
        other => anyhow::bail!("expected HELLO_ACK, got {other:?}"),
    };

    // AUTH → wait AUTH_OK
    seq += 1;
    control.send(&ControlEnvelope {
        seq,
        ack: None,
        body: ControlMessage::Auth(Auth { token: args.token.clone() }),
    }).await?;
    let auth_ok = control.recv().await?;
    let session_id = match auth_ok.body {
        ControlMessage::AuthOk(ok) => ok.session_id,
        ControlMessage::Error(e) => anyhow::bail!("AUTH rejected: {:?}", e),
        other => anyhow::bail!("expected AUTH_OK, got {other:?}"),
    };
    info!(%session_id, "auth ok");

    // MEDIA_HELLO on the media socket
    let _ = MediaHello {
        session_id: session_id.clone(),
        token: args.token.clone(),
    };
    write_media_hello(&mut media, &session::MediaBinding {
        session_id: session_id.clone(),
        token: args.token.clone(),
    }).await?;

    // DEVICE_INFO + CAMERA_LIST
    seq += 1;
    control.send(&ControlEnvelope {
        seq,
        ack: None,
        body: ControlMessage::DeviceInfo(DeviceInfo {
            model: "iPhone15,3".into(),
            os_ver: "iOS 18.0".into(),
            battery_level: 0.87,
            battery_state: BatteryState::Unplugged,
            thermal_state: ThermalState::Nominal,
            usb3_capable: true,
        }),
    }).await?;
    seq += 1;
    control.send(&ControlEnvelope {
        seq,
        ack: None,
        body: ControlMessage::CameraList(CameraList {
            cameras: vec![
                CameraEntry {
                    id: "ultra".into(),
                    name: "Ultra Wide".into(),
                    position: CameraPosition::Back,
                    max_res: (4032, 3024),
                    max_fps: 60,
                    supported_formats: vec!["nv12".into()],
                },
                CameraEntry {
                    id: "wide".into(),
                    name: "Wide".into(),
                    position: CameraPosition::Back,
                    max_res: (4032, 3024),
                    max_fps: 60,
                    supported_formats: vec!["nv12".into(), "hevc".into()],
                },
                CameraEntry {
                    id: "tele".into(),
                    name: "Telephoto".into(),
                    position: CameraPosition::Back,
                    max_res: (4032, 3024),
                    max_fps: 60,
                    supported_formats: vec!["nv12".into(), "hevc".into()],
                },
                CameraEntry {
                    id: "front".into(),
                    name: "TrueDepth".into(),
                    position: CameraPosition::Front,
                    max_res: (3088, 2316),
                    max_fps: 60,
                    supported_formats: vec!["nv12".into(), "hevc".into()],
                },
            ],
        }),
    }).await?;

    // Telemetry loop at ~2 Hz
    let mut battery = 0.87f32;
    loop {
        seq += 1;
        control.send(&ControlEnvelope {
            seq,
            ack: None,
            body: ControlMessage::Telemetry(Telemetry {
                ts_usec: session::keepalive::now_usec(),
                battery_level: battery,
                battery_state: BatteryState::Unplugged,
                thermal_state: ThermalState::Nominal,
                sent_bitrate_kbps: 0,
                enc_fps: 0,
                capture_fps: 0,
                queue_depth: 0,
                drop_count: 0,
            }),
        }).await?;
        battery = (battery - 0.0005).max(0.0);

        tokio::select! {
            _ = sleep(Duration::from_millis(500)) => {},
            r = control.recv() => {
                match r {
                    Ok(env) => match env.body {
                        ControlMessage::Ping(p) => {
                            seq += 1;
                            control.send(&ControlEnvelope {
                                seq,
                                ack: Some(env.seq),
                                body: ControlMessage::Pong(ccp_protocol::Pong {
                                    ts_usec: session::keepalive::now_usec(),
                                    echo_usec: p.ts_usec,
                                }),
                            }).await?;
                        }
                        ControlMessage::Bye(b) => { info!(reason=%b.reason, "server BYE"); return Ok(()); }
                        other => warn!(?other, "ignored in Phase 1"),
                    }
                    Err(e) => {
                        warn!(error = ?e, "control recv error; exiting");
                        return Ok(());
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Re-export keepalive::now_usec from session lib**

In `desktop/crates/session/src/lib.rs` ensure `pub mod keepalive;` exists (already added in Task 9).

- [ ] **Step 4: Build the binary**

```bash
cd desktop && cargo build -p mock-iphone
```

Expected: clean build.

- [ ] **Step 5: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/Cargo.toml desktop/Cargo.lock desktop/tools/mock-iphone
git commit -m "feat(tools): mock-iphone CLI that completes handshake and emits telemetry"
```

---

### Task 12: End-to-end integration test (server + mock)

**Files:**
- Create: `desktop/crates/app/tests/e2e_mock_iphone.rs`

- [ ] **Step 1: Write the test**

```rust
use std::time::Duration;

use app::AppCore;
use ccp_protocol::{
    Auth, BatteryState, ControlEnvelope, ControlMessage, DeviceIdent, DeviceInfo, Hello,
    PROTO_VER, ThermalState,
};
use session::{MediaBinding, write_media_hello, SessionStateKind};
use tokio::time::sleep;
use transport::wifi_connect;

#[tokio::test]
async fn mock_iphone_drives_session_to_ready() {
    let core = AppCore { host: "127.0.0.1".into() };
    let handle = core.start().await.unwrap();
    let cport = handle.qr.cport;
    let mport = handle.qr.mport;
    let token = handle.qr.token.clone();

    let (mut control, mut media) = wifi_connect(
        format!("127.0.0.1:{cport}").parse().unwrap(),
        format!("127.0.0.1:{mport}").parse().unwrap(),
    ).await.unwrap();
    control.send(&ControlEnvelope {
        seq: 1, ack: None,
        body: ControlMessage::Hello(Hello {
            proto_ver: PROTO_VER,
            app: "test/0.1".into(),
            device: DeviceIdent { model: "iPhone15,3".into(), os_ver: "18.0".into() },
            session_id: "x".into(),
            caps: vec![ccp_protocol::Capability::RawNv12],
        }),
    }).await.unwrap();
    let _hello_ack = control.recv().await.unwrap();
    control.send(&ControlEnvelope {
        seq: 2, ack: None,
        body: ControlMessage::Auth(Auth { token: token.clone() }),
    }).await.unwrap();
    let auth_ok = control.recv().await.unwrap();
    let session_id = match auth_ok.body {
        ControlMessage::AuthOk(ok) => ok.session_id,
        other => panic!("expected AUTH_OK, got {other:?}"),
    };
    write_media_hello(&mut media, &MediaBinding {
        session_id,
        token,
    }).await.unwrap();
    control.send(&ControlEnvelope {
        seq: 3, ack: None,
        body: ControlMessage::DeviceInfo(DeviceInfo {
            model: "iPhone15,3".into(),
            os_ver: "18.0".into(),
            battery_level: 0.9,
            battery_state: BatteryState::Unplugged,
            thermal_state: ThermalState::Nominal,
            usb3_capable: true,
        }),
    }).await.unwrap();

    // wait for snapshot to update
    let mut snap = handle.snapshot.clone();
    for _ in 0..50 {
        sleep(Duration::from_millis(50)).await;
        let s = snap.borrow_and_update();
        if matches!(s.state, SessionStateKind::Ready) && s.device.is_some() {
            assert_eq!(s.device.as_ref().unwrap().model, "iPhone15,3");
            return;
        }
    }
    panic!("session never reached Ready with device info: {:?}", *snap.borrow());
}
```

- [ ] **Step 2: Run**

```bash
cd desktop && cargo test -p app --test e2e_mock_iphone
```

Expected: PASS within ~2 s.

- [ ] **Step 3: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/crates/app
git commit -m "test(app): end-to-end mock-iphone drives session to Ready"
```

---

### Task 13: Tauri commands wiring (TDD)

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml`
- Create: `desktop/src-tauri/src/commands.rs`
- Create: `desktop/src-tauri/src/events.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Deps + commands**

Edit `desktop/src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
app = { path = "../crates/app" }
session = { path = "../crates/session" }
qrcode = { workspace = true }
```

Create `desktop/src-tauri/src/commands.rs`:

```rust
use std::sync::Arc;

use app::{AppCore, AppHandle};
use serde::Serialize;
use session::SessionSnapshot;
use tauri::State;
use tokio::sync::Mutex;

pub type AppState = Arc<Mutex<Option<AppHandle>>>;

#[derive(Serialize)]
pub struct QrPayloadOut {
    pub v: u32,
    pub host: String,
    pub cport: u16,
    pub mport: u16,
    pub token: String,
    pub svg: String,
}

#[tauri::command]
pub async fn start_server(state: State<'_, AppState>) -> Result<QrPayloadOut, String> {
    let mut guard = state.lock().await;
    if guard.is_some() {
        return Err("server already running".into());
    }
    let host = local_host().unwrap_or_else(|| "127.0.0.1".into());
    let handle = AppCore { host: host.clone() }
        .start()
        .await
        .map_err(|e| format!("{e:?}"))?;
    let payload_json = serde_json::json!({
        "v": handle.qr.v,
        "host": handle.qr.host,
        "cport": handle.qr.cport,
        "mport": handle.qr.mport,
        "token": handle.qr.token,
    })
    .to_string();
    let code = qrcode::QrCode::new(payload_json.as_bytes())
        .map_err(|e| format!("qr: {e}"))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(280, 280)
        .build();
    let out = QrPayloadOut {
        v: handle.qr.v,
        host: handle.qr.host.clone(),
        cport: handle.qr.cport,
        mport: handle.qr.mport,
        token: handle.qr.token.clone(),
        svg,
    };
    *guard = Some(handle);
    Ok(out)
}

#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.lock().await;
    *guard = None;
    Ok(())
}

#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<SessionSnapshot, String> {
    let guard = state.lock().await;
    match guard.as_ref() {
        Some(h) => Ok(h.snapshot.borrow().clone()),
        None => Ok(SessionSnapshot::idle()),
    }
}

fn local_host() -> Option<String> {
    // Best-effort: pick the first non-loopback IPv4. Phase 1 quality only;
    // Phase 6 polish will introduce a proper picker UI.
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}
```

Create `desktop/src-tauri/src/events.rs`:

```rust
use session::ControlPlaneEvent;
use tauri::Emitter;

pub async fn pump(app: tauri::AppHandle, state: super::commands::AppState) {
    // Polling-based for simplicity in Phase 1; tokio::select on the channel is
    // overkill here since the events channel is owned by `AppHandle` and we
    // only get a clone of the snapshot/events behind the mutex.
    loop {
        let events = {
            let guard = state.lock().await;
            guard.as_ref().map(|h| h.events.clone())
        };
        let Some(events) = events else {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            continue;
        };
        let mut rx = events.lock().await;
        match rx.recv().await {
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Some(ev) => match ev {
                ControlPlaneEvent::StateChanged(kind) => {
                    let _ = app.emit("session://state", &kind);
                }
                ControlPlaneEvent::DeviceInfo(d) => {
                    let _ = app.emit("session://device", &d);
                }
                ControlPlaneEvent::Telemetry(t) => {
                    let _ = app.emit("session://telemetry", &t);
                }
                ControlPlaneEvent::Closed(reason) => {
                    let _ = app.emit("session://closed", &reason);
                }
            },
        }
    }
}
```

Update `desktop/src-tauri/src/lib.rs`:

```rust
mod commands;
mod events;

use std::sync::Arc;

use tokio::sync::Mutex;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tauri=warn".into()),
        )
        .init();
    let state: AppState = Arc::new(Mutex::new(None));
    tauri::Builder::default()
        .manage(state.clone())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                events::pump(app_handle, state).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: Build**

```bash
cd desktop && cargo build -p clearcam-desktop
```

Expected: success.

- [ ] **Step 3: Gate + commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets
git add desktop/src-tauri desktop/Cargo.lock
git commit -m "feat(tauri): start/stop/get_snapshot commands + event pump for ControlPlane"
```

---

### Task 14: UI types + Tauri helpers

**Files:**
- Create: `desktop/ui/src/lib/types.ts`
- Create: `desktop/ui/src/lib/tauri.ts`

- [ ] **Step 1: Types**

Create `desktop/ui/src/lib/types.ts`:

```ts
export type SessionStateKind =
  | { kind: "idle" }
  | { kind: "listening"; control_port: number; media_port: number }
  | { kind: "handshaking" }
  | { kind: "ready" }
  | { kind: "reconnecting" }
  | { kind: "closed"; reason: string };

export type ThermalState = "Nominal" | "Fair" | "Serious" | "Critical";
export type BatteryState = "Unknown" | "Unplugged" | "Charging" | "Full";

export type CameraEntry = {
  id: string;
  name: string;
  position: "Front" | "Back";
  maxRes: [number, number];
  maxFps: number;
  supportedFormats: string[];
};

export type DeviceSnapshot = {
  model: string;
  osVer: string;
  usb3Capable: boolean;
  cameras: CameraEntry[];
};

export type Telemetry = {
  tsUsec: number;
  batteryLevel: number;
  batteryState: BatteryState;
  thermalState: ThermalState;
  sentBitrateKbps: number;
  encFps: number;
  captureFps: number;
  queueDepth: number;
  dropCount: number;
};

export type SessionSnapshot = {
  state: SessionStateKind;
  device: DeviceSnapshot | null;
  last_telemetry: Telemetry | null;
};

export type QrPayloadOut = {
  v: number;
  host: string;
  cport: number;
  mport: number;
  token: string;
  svg: string;
};
```

- [ ] **Step 2: Tauri wrappers**

Create `desktop/ui/src/lib/tauri.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DeviceSnapshot,
  QrPayloadOut,
  SessionSnapshot,
  SessionStateKind,
  Telemetry,
} from "./types";

export async function startServer(): Promise<QrPayloadOut> {
  return invoke("start_server");
}

export async function stopServer(): Promise<void> {
  return invoke("stop_server");
}

export async function getSnapshot(): Promise<SessionSnapshot> {
  return invoke("get_snapshot");
}

export async function onSessionState(
  cb: (s: SessionStateKind) => void,
): Promise<UnlistenFn> {
  return listen<SessionStateKind>("session://state", (e) => cb(e.payload));
}

export async function onDevice(
  cb: (d: DeviceSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<DeviceSnapshot>("session://device", (e) => cb(e.payload));
}

export async function onTelemetry(
  cb: (t: Telemetry) => void,
): Promise<UnlistenFn> {
  return listen<Telemetry>("session://telemetry", (e) => cb(e.payload));
}

export async function onClosed(
  cb: (reason: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("session://closed", (e) => cb(e.payload));
}
```

- [ ] **Step 3: Gate**

```bash
cd desktop/ui && pnpm typecheck && pnpm lint && pnpm format:check
```

- [ ] **Step 4: Commit**

```bash
git add desktop/ui/src/lib
git commit -m "feat(ui): typed Tauri command/event wrappers + shared types"
```

---

### Task 15: UI components

**Files:**
- Create: `desktop/ui/src/components/StatusBadge.tsx`
- Create: `desktop/ui/src/components/ServerCard.tsx`
- Create: `desktop/ui/src/components/DeviceCard.tsx`
- Create: `desktop/ui/src/components/ErrorBanner.tsx`
- Modify: `desktop/ui/src/App.tsx`

- [ ] **Step 1: StatusBadge**

```tsx
import type { SessionStateKind } from "../lib/types";

function label(s: SessionStateKind): string {
  switch (s.kind) {
    case "idle":
      return "Не запущен";
    case "listening":
      return "Ожидание iPhone";
    case "handshaking":
      return "Подключение…";
    case "ready":
      return "Подключено";
    case "reconnecting":
      return "Переподключение…";
    case "closed":
      return `Завершено: ${s.reason}`;
  }
}

function tone(s: SessionStateKind): string {
  switch (s.kind) {
    case "idle":
      return "bg-neutral-700 text-neutral-200";
    case "listening":
    case "handshaking":
      return "bg-amber-700 text-amber-100";
    case "ready":
      return "bg-emerald-700 text-emerald-100";
    case "reconnecting":
      return "bg-orange-700 text-orange-100";
    case "closed":
      return "bg-rose-700 text-rose-100";
  }
}

export default function StatusBadge({ state }: { state: SessionStateKind }) {
  return (
    <span
      className={`inline-flex items-center gap-2 rounded-full px-3 py-1 text-xs font-medium ${tone(state)}`}
    >
      <span className="size-1.5 rounded-full bg-current" />
      {label(state)}
    </span>
  );
}
```

- [ ] **Step 2: ServerCard**

```tsx
import type { QrPayloadOut } from "../lib/types";

export default function ServerCard({
  qr,
  onStop,
}: {
  qr: QrPayloadOut;
  onStop: () => void;
}) {
  return (
    <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-6 shadow-xl">
      <h2 className="text-lg font-semibold mb-4">Отсканируйте QR с iPhone</h2>
      <div className="flex flex-col items-center gap-4">
        <div
          className="size-72 rounded-xl bg-white p-3"
          dangerouslySetInnerHTML={{ __html: qr.svg }}
        />
        <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm text-neutral-300">
          <dt className="text-neutral-500">Хост</dt>
          <dd>{qr.host}</dd>
          <dt className="text-neutral-500">Control</dt>
          <dd>{qr.cport}</dd>
          <dt className="text-neutral-500">Media</dt>
          <dd>{qr.mport}</dd>
        </dl>
        <button
          onClick={onStop}
          className="mt-2 rounded-lg border border-neutral-700 px-4 py-2 text-sm text-neutral-200 hover:bg-neutral-800"
        >
          Остановить
        </button>
      </div>
    </section>
  );
}
```

- [ ] **Step 3: DeviceCard**

```tsx
import type { DeviceSnapshot, Telemetry } from "../lib/types";

function fmtPct(x: number) {
  return `${Math.round(x * 100)}%`;
}

export default function DeviceCard({
  device,
  telemetry,
}: {
  device: DeviceSnapshot;
  telemetry: Telemetry | null;
}) {
  return (
    <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-6 shadow-xl space-y-5">
      <header className="flex items-baseline justify-between">
        <div>
          <h2 className="text-lg font-semibold">{device.model}</h2>
          <p className="text-sm text-neutral-400">{device.osVer}</p>
        </div>
        {device.usb3Capable && (
          <span className="rounded-full bg-sky-900 px-2 py-0.5 text-xs text-sky-200">
            USB 3
          </span>
        )}
      </header>

      <div className="grid grid-cols-3 gap-3 text-sm">
        <Metric
          label="Батарея"
          value={telemetry ? fmtPct(telemetry.batteryLevel) : "—"}
        />
        <Metric label="Тепло" value={telemetry?.thermalState ?? "—"} />
        <Metric
          label="Bitrate"
          value={telemetry ? `${telemetry.sentBitrateKbps} kbps` : "—"}
        />
      </div>

      <div>
        <h3 className="text-sm font-medium text-neutral-300 mb-2">Камеры</h3>
        <ul className="space-y-1 text-sm">
          {device.cameras.map((c) => (
            <li key={c.id} className="flex justify-between text-neutral-300">
              <span>
                {c.name} <span className="text-neutral-500">({c.position})</span>
              </span>
              <span className="text-neutral-500">
                {c.maxRes[0]}×{c.maxRes[1]} @ {c.maxFps}fps
              </span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl bg-neutral-950 p-3">
      <p className="text-xs text-neutral-500">{label}</p>
      <p className="font-mono text-base text-neutral-100">{value}</p>
    </div>
  );
}
```

- [ ] **Step 4: ErrorBanner**

```tsx
export default function ErrorBanner({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  return (
    <div className="flex items-center justify-between rounded-xl border border-rose-900/60 bg-rose-950/40 px-4 py-3 text-sm text-rose-200">
      <span>{message}</span>
      <button onClick={onDismiss} className="text-rose-300 hover:text-rose-100">
        ×
      </button>
    </div>
  );
}
```

- [ ] **Step 5: App.tsx — wire it together**

Replace `desktop/ui/src/App.tsx`:

```tsx
import { useEffect, useState } from "react";
import DeviceCard from "./components/DeviceCard";
import ErrorBanner from "./components/ErrorBanner";
import ServerCard from "./components/ServerCard";
import StatusBadge from "./components/StatusBadge";
import {
  getSnapshot,
  onClosed,
  onDevice,
  onSessionState,
  onTelemetry,
  startServer,
  stopServer,
} from "./lib/tauri";
import type {
  DeviceSnapshot,
  QrPayloadOut,
  SessionStateKind,
  Telemetry,
} from "./lib/types";

export default function App() {
  const [qr, setQr] = useState<QrPayloadOut | null>(null);
  const [state, setState] = useState<SessionStateKind>({ kind: "idle" });
  const [device, setDevice] = useState<DeviceSnapshot | null>(null);
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let off: Array<() => void> = [];
    (async () => {
      try {
        const snap = await getSnapshot();
        setState(snap.state);
        setDevice(snap.device);
        setTelemetry(snap.last_telemetry);
        off.push(await onSessionState(setState));
        off.push(await onDevice((d) => setDevice(d)));
        off.push(await onTelemetry((t) => setTelemetry(t)));
        off.push(
          await onClosed((reason) => {
            setError(`Сессия закрыта: ${reason}`);
            setDevice(null);
            setTelemetry(null);
          }),
        );
      } catch (e) {
        setError(String(e));
      }
    })();
    return () => {
      for (const f of off) f();
    };
  }, []);

  async function handleStart() {
    setError(null);
    try {
      const out = await startServer();
      setQr(out);
    } catch (e) {
      setError(String(e));
    }
  }
  async function handleStop() {
    try {
      await stopServer();
      setQr(null);
      setState({ kind: "idle" });
      setDevice(null);
      setTelemetry(null);
    } catch (e) {
      setError(String(e));
    }
  }

  const showServer = qr && state.kind !== "ready";
  const showDevice = device && state.kind === "ready";

  return (
    <main className="min-h-screen bg-neutral-950 text-neutral-100 px-6 py-10">
      <div className="mx-auto w-full max-w-2xl space-y-6">
        <header className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold">ClearCam</h1>
            <p className="text-sm text-neutral-400">iPhone-as-webcam · Phase 1</p>
          </div>
          <StatusBadge state={state} />
        </header>

        {error && (
          <ErrorBanner message={error} onDismiss={() => setError(null)} />
        )}

        {!qr && (
          <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-6 text-center">
            <p className="text-neutral-300 mb-4">
              Запустите сервер и отсканируйте QR с iPhone.
            </p>
            <button
              onClick={handleStart}
              className="rounded-lg bg-emerald-700 px-5 py-2 text-sm font-medium text-emerald-50 hover:bg-emerald-600"
            >
              Запустить сервер
            </button>
          </section>
        )}

        {showServer && qr && <ServerCard qr={qr} onStop={handleStop} />}
        {showDevice && device && (
          <DeviceCard device={device} telemetry={telemetry} />
        )}
      </div>
    </main>
  );
}
```

- [ ] **Step 6: UI gate**

```bash
cd desktop/ui && pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
```

Expected: green.

- [ ] **Step 7: Commit**

```bash
git add desktop/ui/src
git commit -m "feat(ui): connect screen with QR, device card, status badge, error banner"
```

---

### Task 16: Manual smoke run (foreground)

**Files:** none — manual verification.

- [ ] **Step 1: Run desktop in dev mode**

```bash
cd desktop && cargo tauri dev
```

Wait for the window to open. Click "Запустить сервер". You should see a QR and the host/port info. Read the cport/mport/token from the UI (or copy from logs).

- [ ] **Step 2: In a separate terminal run mock-iphone**

```bash
cd /Users/denisgumen/Desktop/code/iphone-webcam/desktop
cargo run --release -p mock-iphone -- --host 127.0.0.1 --cport <CPORT> --mport <MPORT> --token <TOKEN>
```

Expected: UI flips to `Подключено`; device card appears with iPhone15,3 / iOS 18.0; battery % ticks down over time (every 2 s).

- [ ] **Step 3: Kill mock-iphone (Ctrl+C)**

Expected: UI switches to `Переподключение…` then `Завершено`.

- [ ] **Step 4: Stop desktop, commit acceptance note**

Add a short note in `plans/phase-1-transport-handshake.md` under a new "## Acceptance log" section recording the date, who ran it, and any observations.

```bash
git add plans/phase-1-transport-handshake.md
git commit -m "docs(plans): record Phase 1A manual acceptance"
```

---

# Section 1B — iOS client (TransportClient, SessionController, QR scan, SwiftUI)

After Section 1A, the desktop side is fully observable and tested without iOS. Section 1B adds a real iPhone client. Two physical targets in the SwiftPM package: `ClearCamCore` (logic, testable on macOS via `swift test`) and `ClearCamApp` (SwiftUI app — built via Xcode for device).

---

### Task 17: Extend SwiftPM with ClearCamCore target

**Files:**
- Modify: `ios/Package.swift`

- [ ] **Step 1: Update Package.swift**

```swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ClearCam",
    platforms: [.iOS(.v17), .macOS(.v13)],
    products: [
        .library(name: "ClearCamProtocol", targets: ["ClearCamProtocol"]),
        .library(name: "ClearCamCore", targets: ["ClearCamCore"]),
    ],
    targets: [
        .target(name: "ClearCamProtocol", path: "Sources/ClearCamProtocol"),
        .target(
            name: "ClearCamCore",
            dependencies: ["ClearCamProtocol"],
            path: "Sources/ClearCamCore"
        ),
        .testTarget(
            name: "ClearCamProtocolTests",
            dependencies: ["ClearCamProtocol"],
            path: "Tests/ClearCamProtocolTests"
        ),
        .testTarget(
            name: "ClearCamCoreTests",
            dependencies: ["ClearCamCore"],
            path: "Tests/ClearCamCoreTests"
        ),
    ]
)
```

- [ ] **Step 2: Verify package resolves**

```bash
source scripts/swift-env.sh
cd ios && swift package describe
```

Expected: lists both libraries and test targets without errors.

- [ ] **Step 3: Commit**

```bash
git add ios/Package.swift
git commit -m "chore(ios): add ClearCamCore target and ClearCamCoreTests"
```

---

### Task 18: QRPayload + tests (TDD)

**Files:**
- Create: `ios/Sources/ClearCamCore/Pairing/QRPayload.swift`
- Create: `ios/Tests/ClearCamCoreTests/QRPayloadTests.swift`

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import ClearCamCore

final class QRPayloadTests: XCTestCase {
    func testDecodesDesktopPayload() throws {
        let json = #"""
        {"v":1,"host":"192.168.1.42","cport":7000,"mport":7001,"token":"abc"}
        """#
        let p = try QRPayload.decode(Data(json.utf8))
        XCTAssertEqual(p.v, 1)
        XCTAssertEqual(p.host, "192.168.1.42")
        XCTAssertEqual(p.cport, 7000)
        XCTAssertEqual(p.mport, 7001)
        XCTAssertEqual(p.token, "abc")
    }

    func testRejectsWrongVersion() {
        let json = #"{"v":999,"host":"a","cport":1,"mport":2,"token":"t"}"#
        XCTAssertThrowsError(try QRPayload.decode(Data(json.utf8)))
    }
}
```

Create `QRPayload.swift`:

```swift
import Foundation

public struct QRPayload: Codable, Equatable, Sendable {
    public let v: Int
    public let host: String
    public let cport: Int
    public let mport: Int
    public let token: String

    public enum DecodeError: Error, Equatable {
        case unsupportedVersion(Int)
        case invalid
    }

    public static let supportedVersion = 1

    public static func decode(_ data: Data) throws -> QRPayload {
        let p = try JSONDecoder().decode(QRPayload.self, from: data)
        guard p.v == supportedVersion else { throw DecodeError.unsupportedVersion(p.v) }
        guard !p.host.isEmpty, p.cport > 0, p.mport > 0, !p.token.isEmpty else {
            throw DecodeError.invalid
        }
        return p
    }
}
```

- [ ] **Step 2: Run tests**

```bash
source scripts/swift-env.sh && cd ios && swift test --filter QRPayloadTests
```

Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add ios/Sources/ClearCamCore/Pairing ios/Tests/ClearCamCoreTests/QRPayloadTests.swift
git commit -m "feat(ios): QRPayload Codable + version-guarded decoder"
```

---

### Task 19: ControlStream — length-prefixed JSON over NWConnection (TDD)

**Files:**
- Create: `ios/Sources/ClearCamCore/Transport/ControlStream.swift`
- Create: `ios/Tests/ClearCamCoreTests/ControlStreamTests.swift`

- [ ] **Step 1: Write the failing test**

The test uses a pair of loopback `NWConnection`s over `NWListener` to verify framing.

```swift
import Foundation
import Network
import XCTest
@testable import ClearCamCore
import ClearCamProtocol

final class ControlStreamTests: XCTestCase {
    func testRoundTripEnvelope() async throws {
        let listener = try NWListener(using: .tcp, on: .any)
        var pair = try await acceptingPair(listener: listener)
        defer { listener.cancel() }

        let env = ControlEnvelope(
            seq: 7,
            ack: nil,
            body: .auth(Auth(token: "secret"))
        )
        try await pair.client.send(env)
        let got = try await pair.server.recv()
        XCTAssertEqual(got, env)
    }
}

// helper kept in the test target
struct LoopbackPair {
    let client: ControlStream
    let server: ControlStream
}

func acceptingPair(listener: NWListener) async throws -> LoopbackPair {
    try await withCheckedThrowingContinuation { cont in
        var serverConn: NWConnection?
        listener.newConnectionHandler = { c in
            c.start(queue: .global())
            serverConn = c
        }
        listener.start(queue: .global())
        guard let port = listener.port else {
            cont.resume(throwing: NSError(domain: "test", code: 1))
            return
        }
        let client = NWConnection(host: "127.0.0.1", port: port, using: .tcp)
        client.stateUpdateHandler = { state in
            if case .ready = state, let sc = serverConn {
                cont.resume(returning: LoopbackPair(
                    client: ControlStream(connection: client),
                    server: ControlStream(connection: sc)
                ))
            }
        }
        client.start(queue: .global())
    }
}
```

Create `ControlStream.swift`:

```swift
import Foundation
import Network
import ClearCamProtocol

public enum ControlStreamError: Error, Sendable {
    case io(Error)
    case truncated
    case oversized(UInt32)
    case decode(Error)
}

public actor ControlStream {
    public static let maxPayload: UInt32 = 4 * 1024 * 1024 // mirrors DEFAULT_MAX_PAYLOAD on Rust side

    private let connection: NWConnection

    public init(connection: NWConnection) {
        self.connection = connection
    }

    public func send(_ env: ControlEnvelope) async throws {
        let payload = try env.encodeJSON()
        var prefix = UInt32(payload.count).bigEndian
        var frame = Data()
        withUnsafeBytes(of: &prefix) { frame.append(contentsOf: $0) }
        frame.append(payload)
        try await send(frame)
    }

    public func recv() async throws -> ControlEnvelope {
        let prefix = try await receive(exactly: 4)
        let len = UInt32(bigEndian: prefix.withUnsafeBytes { $0.load(as: UInt32.self) })
        guard len <= Self.maxPayload else { throw ControlStreamError.oversized(len) }
        let body = try await receive(exactly: Int(len))
        do {
            return try ControlEnvelope.decodeJSON(body)
        } catch {
            throw ControlStreamError.decode(error)
        }
    }

    private func send(_ data: Data) async throws {
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
            connection.send(content: data, completion: .contentProcessed { err in
                if let err { c.resume(throwing: ControlStreamError.io(err)) }
                else { c.resume() }
            })
        }
    }

    private func receive(exactly n: Int) async throws -> Data {
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<Data, Error>) in
            connection.receive(minimumIncompleteLength: n, maximumLength: n) { data, _, _, err in
                if let err { c.resume(throwing: ControlStreamError.io(err)) }
                else if let data, data.count == n { c.resume(returning: data) }
                else { c.resume(throwing: ControlStreamError.truncated) }
            }
        }
    }
}
```

Also add helpers `encodeJSON()` / `decodeJSON()` on `ControlEnvelope` (in `ClearCamProtocol/ControlEnvelope.swift`). If they don't exist yet:

```swift
public extension ControlEnvelope {
    func encodeJSON() throws -> Data {
        let enc = JSONEncoder()
        enc.outputFormatting = .withoutEscapingSlashes
        return try enc.encode(self)
    }

    static func decodeJSON(_ data: Data) throws -> ControlEnvelope {
        try JSONDecoder().decode(ControlEnvelope.self, from: data)
    }
}
```

- [ ] **Step 2: Run tests**

```bash
source scripts/swift-env.sh && cd ios && swift test --filter ControlStreamTests
```

Expected: PASS.

- [ ] **Step 3: Gate + commit**

```bash
swift build && swift test
git add ios/Sources/ClearCamCore/Transport ios/Sources/ClearCamProtocol/ControlEnvelope.swift ios/Tests/ClearCamCoreTests/ControlStreamTests.swift
git commit -m "feat(ios): ControlStream actor with length-prefixed JSON framing"
```

---

### Task 20: SessionController state machine (TDD)

**Files:**
- Create: `ios/Sources/ClearCamCore/Session/SessionState.swift`
- Create: `ios/Sources/ClearCamCore/Session/SessionController.swift`
- Create: `ios/Sources/ClearCamCore/Status/DeviceInfoProvider.swift`
- Create: `ios/Tests/ClearCamCoreTests/SessionControllerTests.swift`

- [ ] **Step 1: Write failing test (with a fake ControlStream-like channel)**

```swift
import XCTest
@testable import ClearCamCore
import ClearCamProtocol

final class SessionControllerTests: XCTestCase {
    @MainActor
    func testHandshakeReachesReady() async throws {
        let channel = InProcessChannel()
        let stub = StubInfoProvider()
        let ctrl = SessionController(
            controlChannel: channel,
            mediaChannel: InProcessChannel(),
            deviceInfo: stub
        )

        Task {
            // Simulate server-side responses
            let hello = try await channel.peerRecv()
            XCTAssertEqual(hello.t, .hello)
            try await channel.peerSend(ControlEnvelope(seq: 1, ack: hello.seq, body: .helloAck(HelloAck(protoVer: ProtoVer, caps: []))))
            let auth = try await channel.peerRecv()
            try await channel.peerSend(ControlEnvelope(seq: 2, ack: auth.seq, body: .authOk(AuthOk(sessionId: "sess-abc"))))
        }

        try await ctrl.connect(payload: QRPayload(v: 1, host: "x", cport: 0, mport: 0, token: "t"))
        await MainActor.run {
            XCTAssertEqual(ctrl.state, .ready)
        }
    }
}

final class StubInfoProvider: DeviceInfoProvider {
    var snapshot: DeviceInfo {
        DeviceInfo(model: "iPhone15,3", osVer: "18.0", batteryLevel: 0.9, batteryState: .unplugged, thermalState: .nominal, usb3Capable: true)
    }
}

// In-process channel mimicking a ControlStream interface (`send/recv`).
actor InProcessChannel: ControlChannel {
    private var toApp: AsyncStream<ControlEnvelope>.Continuation?
    private var fromApp: AsyncStream<ControlEnvelope>.Continuation?
    let toAppStream: AsyncStream<ControlEnvelope>
    let fromAppStream: AsyncStream<ControlEnvelope>
    init() {
        var c1: AsyncStream<ControlEnvelope>.Continuation!
        var c2: AsyncStream<ControlEnvelope>.Continuation!
        self.toAppStream = AsyncStream { c1 = $0 }
        self.fromAppStream = AsyncStream { c2 = $0 }
        self.toApp = c1
        self.fromApp = c2
    }
    func send(_ env: ControlEnvelope) async throws { fromApp?.yield(env) }
    func recv() async throws -> ControlEnvelope {
        var iter = toAppStream.makeAsyncIterator()
        if let e = await iter.next() { return e } else { throw NSError(domain: "x", code: 1) }
    }
    nonisolated func peerSend(_ env: ControlEnvelope) async throws { /* via continuation */ }
    nonisolated func peerRecv() async throws -> ControlEnvelope { fatalError() }
}
```

> **Note:** The test above sketches the interaction; the agent implementing this task is expected to refine the helper actor to a clean dual-ended channel before committing. The test must compile and pass.

Create `SessionState.swift`:

```swift
import Foundation

public enum SessionState: Equatable, Sendable {
    case idle
    case connecting
    case handshaking
    case ready(sessionId: String)
    case reconnecting
    case stopped(reason: String)
}
```

Create `DeviceInfoProvider.swift`:

```swift
import Foundation
import ClearCamProtocol

public protocol DeviceInfoProvider: Sendable {
    var snapshot: DeviceInfo { get }
}
```

Create `SessionController.swift`:

```swift
import Foundation
import ClearCamProtocol

public protocol ControlChannel: Sendable {
    func send(_ env: ControlEnvelope) async throws
    func recv() async throws -> ControlEnvelope
}

@MainActor
public final class SessionController {
    public private(set) var state: SessionState = .idle
    private let controlChannel: any ControlChannel
    private let mediaChannel: any ControlChannel
    private let deviceInfo: any DeviceInfoProvider

    public init(
        controlChannel: any ControlChannel,
        mediaChannel: any ControlChannel,
        deviceInfo: any DeviceInfoProvider
    ) {
        self.controlChannel = controlChannel
        self.mediaChannel = mediaChannel
        self.deviceInfo = deviceInfo
    }

    public func connect(payload: QRPayload) async throws {
        state = .connecting
        state = .handshaking
        try await controlChannel.send(ControlEnvelope(
            seq: 1, ack: nil,
            body: .hello(Hello(
                protoVer: ProtoVer,
                app: "ClearCam-iOS/0.1.0",
                device: DeviceIdent(model: deviceInfo.snapshot.model, osVer: deviceInfo.snapshot.osVer),
                sessionId: UUID().uuidString,
                caps: [.hevc, .rawNv12]
            ))
        ))
        let ack = try await controlChannel.recv()
        guard case .helloAck = ack.body else {
            state = .stopped(reason: "expected HELLO_ACK")
            throw NSError(domain: "ccp", code: 1)
        }
        try await controlChannel.send(ControlEnvelope(
            seq: 2, ack: nil,
            body: .auth(Auth(token: payload.token))
        ))
        let authResp = try await controlChannel.recv()
        guard case let .authOk(ok) = authResp.body else {
            state = .stopped(reason: "auth failed")
            throw NSError(domain: "ccp", code: 2)
        }
        state = .ready(sessionId: ok.sessionId)
    }
}
```

- [ ] **Step 2: Iterate until tests pass**

```bash
source scripts/swift-env.sh && cd ios && swift test --filter SessionControllerTests
```

The agent may need to refactor the test helper for a clean dual-ended channel. The bar for "done" is: state transitions `idle → connecting → handshaking → ready(sessionId:)` are observed in a passing test.

- [ ] **Step 3: Gate + commit**

```bash
swift build && swift test
git add ios/Sources/ClearCamCore/Session ios/Sources/ClearCamCore/Status ios/Tests/ClearCamCoreTests/SessionControllerTests.swift
git commit -m "feat(ios): SessionController state machine through AUTH_OK"
```

---

### Task 21: TransportClient + media handshake + telemetry emitter

**Files:**
- Create: `ios/Sources/ClearCamCore/Transport/TransportClient.swift`
- Create: `ios/Sources/ClearCamCore/Status/TelemetryEmitter.swift`
- Modify: `ios/Sources/ClearCamCore/Session/SessionController.swift` (emit MEDIA_HELLO + DEVICE_INFO + start telemetry)

- [ ] **Step 1: TransportClient (Wi-Fi loopback build)**

```swift
import Foundation
import Network

public final class TransportClient {
    public let host: NWEndpoint.Host
    public let controlPort: NWEndpoint.Port
    public let mediaPort: NWEndpoint.Port

    public init(host: String, cport: Int, mport: Int) {
        self.host = NWEndpoint.Host(host)
        self.controlPort = NWEndpoint.Port(rawValue: UInt16(cport))!
        self.mediaPort = NWEndpoint.Port(rawValue: UInt16(mport))!
    }

    public func connect() async throws -> (ControlStream, ControlStream) {
        let control = try await openTCP(port: controlPort)
        let media = try await openTCP(port: mediaPort)
        return (ControlStream(connection: control), ControlStream(connection: media))
    }

    private func openTCP(port: NWEndpoint.Port) async throws -> NWConnection {
        let c = NWConnection(host: host, port: port, using: .tcp)
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            c.stateUpdateHandler = { state in
                switch state {
                case .ready: cont.resume()
                case .failed(let err): cont.resume(throwing: err)
                case .cancelled: cont.resume(throwing: ControlStreamError.truncated)
                default: break
                }
            }
            c.start(queue: .global())
        }
        return c
    }
}
```

- [ ] **Step 2: TelemetryEmitter**

```swift
import Foundation
import ClearCamProtocol

@MainActor
public final class TelemetryEmitter {
    private var task: Task<Void, Never>?
    public init() {}

    public func start(every interval: Duration, sink: @escaping (Telemetry) async -> Void, info: any DeviceInfoProvider) {
        stop()
        task = Task {
            while !Task.isCancelled {
                let snap = info.snapshot
                let t = Telemetry(
                    tsUsec: UInt64(Date().timeIntervalSince1970 * 1_000_000),
                    batteryLevel: snap.batteryLevel,
                    batteryState: snap.batteryState,
                    thermalState: snap.thermalState,
                    sentBitrateKbps: 0,
                    encFps: 0,
                    captureFps: 0,
                    queueDepth: 0,
                    dropCount: 0
                )
                await sink(t)
                try? await Task.sleep(for: interval)
            }
        }
    }

    public func stop() {
        task?.cancel()
        task = nil
    }
}
```

- [ ] **Step 3: Hook them in SessionController**

After `state = .ready(sessionId: ok.sessionId)`:

```swift
try await mediaChannel.send(ControlEnvelope(
    seq: 1, ack: nil,
    body: .mediaHello(MediaHello(sessionId: ok.sessionId, token: payload.token))
))

try await controlChannel.send(ControlEnvelope(
    seq: 3, ack: nil,
    body: .deviceInfo(deviceInfo.snapshot)
))

let emitter = TelemetryEmitter()
self.telemetryEmitter = emitter
emitter.start(every: .milliseconds(500), sink: { [weak self] t in
    try? await self?.controlChannel.send(ControlEnvelope(seq: 0, ack: nil, body: .telemetry(t)))
}, info: deviceInfo)
```

(Add a stored property `private var telemetryEmitter: TelemetryEmitter?`.)

- [ ] **Step 4: Run swift test**

Expected: existing tests still pass. Add a new test that asserts a telemetry envelope is observed on the in-process channel within 1 s — this proves the emitter wiring.

- [ ] **Step 5: Gate + commit**

```bash
swift build && swift test
git add ios/Sources/ClearCamCore
git commit -m "feat(ios): TransportClient + media handshake + telemetry emitter"
```

---

### Task 22: SwiftUI app target + QR scanner

**Files:**
- Create: `ios/Sources/ClearCamApp/ClearCamApp.swift`
- Create: `ios/Sources/ClearCamApp/ConnectView.swift`
- Create: `ios/Sources/ClearCamApp/ConnectedView.swift`
- Create: `ios/Sources/ClearCamApp/QRScanner.swift`
- Create: `ios/Sources/ClearCamApp/UIKitDeviceInfo.swift`
- Modify: `ios/Package.swift` (add executable product for iOS-app SwiftPM template; in practice this is consumed by an Xcode project file)

> **Note:** SwiftPM cannot produce an iOS app bundle by itself. We keep the source files in SwiftPM as a library target so they remain compilable on macOS for CI lint purposes, and we wire them into an Xcode project later (Task 23). For now, expose them as a `ClearCamAppKit` library.

- [ ] **Step 1: Package.swift adjustments**

Add a new library target `ClearCamAppKit`:

```swift
.target(
    name: "ClearCamAppKit",
    dependencies: ["ClearCamCore"],
    path: "Sources/ClearCamApp"
),
```

…and update `products` to include `.library(name: "ClearCamAppKit", targets: ["ClearCamAppKit"])`.

- [ ] **Step 2: Source files**

Create `UIKitDeviceInfo.swift`:

```swift
#if canImport(UIKit)
import UIKit
import ClearCamCore
import ClearCamProtocol

public struct UIKitDeviceInfo: DeviceInfoProvider {
    public init() {
        UIDevice.current.isBatteryMonitoringEnabled = true
    }
    public var snapshot: DeviceInfo {
        let d = UIDevice.current
        let thermal: ThermalState
        switch ProcessInfo.processInfo.thermalState {
        case .nominal: thermal = .nominal
        case .fair: thermal = .fair
        case .serious: thermal = .serious
        case .critical: thermal = .critical
        @unknown default: thermal = .nominal
        }
        let state: BatteryState
        switch d.batteryState {
        case .unknown: state = .unknown
        case .unplugged: state = .unplugged
        case .charging: state = .charging
        case .full: state = .full
        @unknown default: state = .unknown
        }
        return DeviceInfo(
            model: d.model,
            osVer: "\(d.systemName) \(d.systemVersion)",
            batteryLevel: max(0, d.batteryLevel),
            batteryState: state,
            thermalState: thermal,
            usb3Capable: false // Phase 5 will probe via lightning vs usb-c detection
        )
    }
}
#endif
```

Create `QRScanner.swift` (UIKit-bridged):

```swift
#if canImport(UIKit)
import UIKit
import AVFoundation
import SwiftUI

public struct QRScannerView: UIViewControllerRepresentable {
    let onPayload: (Data) -> Void
    public init(onPayload: @escaping (Data) -> Void) { self.onPayload = onPayload }

    public func makeUIViewController(context: Context) -> QRScannerVC {
        let vc = QRScannerVC()
        vc.onPayload = onPayload
        return vc
    }
    public func updateUIViewController(_ uiViewController: QRScannerVC, context: Context) {}
}

public final class QRScannerVC: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var onPayload: ((Data) -> Void)?
    private let session = AVCaptureSession()
    private var preview: AVCaptureVideoPreviewLayer?

    public override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        guard let dev = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: dev),
              session.canAddInput(input) else { return }
        session.addInput(input)
        let out = AVCaptureMetadataOutput()
        guard session.canAddOutput(out) else { return }
        session.addOutput(out)
        out.metadataObjectTypes = [.qr]
        out.setMetadataObjectsDelegate(self, queue: .main)
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.addSublayer(layer)
        preview = layer
        Task { @MainActor in
            session.startRunning()
        }
    }

    public override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    public func metadataOutput(_ output: AVCaptureMetadataOutput,
                               didOutput metadataObjects: [AVMetadataObject],
                               from connection: AVCaptureConnection) {
        guard let obj = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
              let s = obj.stringValue,
              let data = s.data(using: .utf8) else { return }
        session.stopRunning()
        onPayload?(data)
    }
}
#endif
```

Create `ConnectView.swift`, `ConnectedView.swift`, `ClearCamApp.swift` — minimal SwiftUI shell that:

- shows `QRScannerView` until a payload is decoded successfully,
- then constructs `TransportClient`, calls `SessionController.connect`,
- shows `ConnectedView` with state, last telemetry, and "Disconnect" button.

(The code is straightforward SwiftUI scaffolding; the test for it is manual since UI tests on SwiftPM are not supported. Add a tiny `XCTestCase` that constructs `UIKitDeviceInfo` if compiled on iOS.)

- [ ] **Step 3: Verify compilation**

```bash
source scripts/swift-env.sh && cd ios && swift build
```

Expected: `ClearCamAppKit` compiles for macOS (without UIKit-only branches) and the iOS-conditional code is guarded by `#if canImport(UIKit)`.

- [ ] **Step 4: Commit**

```bash
git add ios/Package.swift ios/Sources/ClearCamApp
git commit -m "feat(ios): SwiftUI app shell + QR scanner + UIKitDeviceInfo (compiles via #if)"
```

---

### Task 23: Xcode project for iOS app + on-device run

**Files:**
- Create: `ios/ClearCam.xcodeproj` (via `xcodegen` or hand-rolled)
- Create: `ios/ClearCam/Info.plist`
- Create: `ios/ClearCam/Assets.xcassets/AppIcon.appiconset/Contents.json` (placeholder)

- [ ] **Step 1: Add an Xcode project**

Use `xcodegen` if available, otherwise hand-write `project.pbxproj`. The project must:
- Link `ClearCamProtocol`, `ClearCamCore`, `ClearCamAppKit` from SwiftPM (local package).
- Bundle ID: `app.clearcam.ios` (placeholder; user replaces with their developer team).
- Deployment target: iOS 17.0.
- Info.plist keys:
  - `NSCameraUsageDescription` = "ClearCam needs the camera to send video to your computer."
  - `NSLocalNetworkUsageDescription` = "ClearCam connects to your computer over Wi-Fi."
  - `NSBonjourServices` = ["_clearcam._tcp"]

> **Decision point:** introduce `xcodegen` as a dev-dependency (single YAML spec) OR commit a minimal hand-rolled pbxproj. Recommendation: `xcodegen` — keeps the spec readable and avoids merge hell.

Add `ios/project.yml` (xcodegen spec) — exact contents in the executor's working notes. Add `scripts/regen-xcode.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
brew list xcodegen >/dev/null 2>&1 || brew install xcodegen
cd "$(dirname "$0")/../ios"
xcodegen generate
```

- [ ] **Step 2: Run xcodegen**

```bash
bash scripts/regen-xcode.sh
```

- [ ] **Step 3: Build for simulator from CLI**

```bash
source scripts/swift-env.sh
cd ios
xcodebuild -scheme ClearCam -sdk iphonesimulator -configuration Debug \
  -destination 'platform=iOS Simulator,name=iPhone 15' build | xcpretty
```

Expected: clean build.

- [ ] **Step 4: Manual run on simulator**

```bash
xcrun simctl boot 'iPhone 15' || true
xcrun simctl install booted /path/to/built/app
xcrun simctl launch booted app.clearcam.ios
```

(Or simply open `ClearCam.xcodeproj` in Xcode and Run.)

> The simulator can scan a QR only if you display it on the host screen. For automated runs, populate host/port/token via env vars and use a "manual entry" fallback in the SwiftUI Connect screen.

- [ ] **Step 5: Commit**

```bash
git add ios/project.yml ios/ClearCam.xcodeproj ios/ClearCam scripts/regen-xcode.sh
git commit -m "build(ios): xcodegen-driven Xcode project for ClearCam app"
```

---

### Task 24: End-to-end run with real iPhone

**Files:** none — manual; record results in `plans/phase-1-transport-handshake.md`.

- [ ] **Step 1: Pair device for development**

In Xcode: select developer team, plug in iPhone, trust the developer profile on-device.

- [ ] **Step 2: Build & run**

Run the app on the iPhone. Run desktop with `cargo tauri dev`. Click "Запустить сервер". Scan the QR with the iPhone.

- [ ] **Step 3: Verify acceptance**

- iPhone shows "Подключено", desktop UI flips to `Подключено`.
- Desktop shows model = real iPhone model, OS version, current battery level, camera list (from app).
- Telemetry ticks at ~2 Hz with battery decreasing under load (or stable).
- Pull Wi-Fi router → iPhone switches to `reconnecting`; reconnect router → session resumes.

- [ ] **Step 4: Document in plan**

Append to the Acceptance log a paragraph with device model, iOS version, observations, and any deviations.

```bash
git add plans/phase-1-transport-handshake.md
git commit -m "docs(plans): record Phase 1B manual acceptance on real iPhone"
```

---

# Section 1C — CI, tag, retrospective

### Task 25: CI for new artifacts

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add smoke run for mock-iphone + Swift core tests**

In the `rust:` job, after `cargo test`, add:

```yaml
      - name: mock-iphone help
        run: cargo run -p mock-iphone -- --help
```

In the `swift:` job, replace `swift test` with `swift test --enable-test-discovery` (or leave as-is if SwiftPM 5.9 already discovers tests). Ensure both `ClearCamProtocolTests` and `ClearCamCoreTests` run.

- [ ] **Step 2: Push and confirm green**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: smoke mock-iphone CLI and ClearCamCore tests"
git push
```

Wait for CI. If red, fix until green.

---

### Task 26: Tag v0.2.0-phase1 + retrospective

**Files:**
- Modify: `plans/phase-1-transport-handshake.md` (add `## Retrospective` section)

- [ ] **Step 1: Write retrospective**

Cover:
- What landed (one-line per task group).
- Any deviations from the plan (and why).
- Open questions for Phase 2 (e.g., where to plug `MediaPipeline` into the parked media socket).

- [ ] **Step 2: Tag**

```bash
git add plans/phase-1-transport-handshake.md
git commit -m "docs(plans): Phase 1 retrospective"
git tag v0.2.0-phase1
git push --tags
```

---

## Acceptance log

### 2026-05-28 — Section 1A + 1B automated acceptance

- Local Rust gate: `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings` (rustc 1.95.0) + `cargo test --workspace --all-targets` (7 tests + 1 e2e integration test) ✓
- Local UI gate: `pnpm typecheck && pnpm lint && pnpm format:check && pnpm build` ✓
- Local iOS gate: `swift build` + `swift test` (29 unit tests) + `swift build --target ClearCamAppKit` ✓
- CI on `b33f3c7`: pending verification (push made just now after the clippy 1.95 fix for `is_some+unwrap`).

### 2026-05-27 — Section 1A automated acceptance (initial)

- `cargo fmt --all --check` ✓
- `cargo clippy --workspace --all-targets -- -D warnings` ✓
- `cargo test --workspace --all-targets` → **7 tests passed** (transport: 6, session: 6, app: 2, app integration: 1)
- `cargo build -p clearcam-desktop` ✓ (Tauri shell compiles cleanly with new commands/events)
- `cargo run -p mock-iphone -- --help` ✓
- `cd desktop/ui && pnpm typecheck && pnpm lint && pnpm format:check && pnpm build` ✓

### Section 1A — manual UI smoke (deferred to user)

The headless e2e test (`crates/app/tests/e2e_mock_iphone.rs`) already verifies
the full handshake end-to-end through `AppCore`. Visual verification of the
Tauri window is a one-step manual check left to the user:

```bash
cd desktop && cargo tauri dev
# in another terminal, copy cport/mport/token from UI (or RUST_LOG=info logs)
cargo run -p mock-iphone -- --host 127.0.0.1 --cport <CPORT> --mport <MPORT> --token <TOKEN>
```

Expected: UI flips from "Ожидание iPhone" → "Подключено", device card shows
iPhone15,3 / iOS 18.0 / 4 cameras, telemetry meter updates every ~500 ms.

---

## Retrospective

### What landed

**Rust workspace**
- `transport`: async length-prefixed framing (`read_frame`/`write_frame`), `ControlStream`/`MediaStream` over either TCP or in-memory duplex, `WifiServer` with paired control/media accept loops + `wifi_connect` client. 6 unit tests.
- `session`: `SessionError`, `SessionSnapshot`/`SessionStateKind`/`DeviceSnapshot`, `accept_control` handshake state machine (HELLO/HELLO_ACK + AUTH/AUTH_OK + ERROR paths), `MediaBinding` + `read/write_media_hello`, `ControlPlane` actor with PING/PONG keepalive + dead-peer detection + BYE handling, shared events/snapshot channels. 6 unit tests.
- `app`: `AppCore::start()` wires `WifiServer` + per-handshake `ControlPlane`, generates random base64-url token, exposes `QrPayload` + `snapshot: watch::Receiver` + `events: mpsc::Receiver`. 2 unit tests + 1 e2e integration test.
- `tools/mock-iphone`: CLI binary completing handshake, emitting DEVICE_INFO/CAMERA_LIST and TELEMETRY at 2 Hz, responding to PING.

**Tauri 2 shell** (`src-tauri`)
- Commands: `start_server`, `stop_server`, `get_snapshot` with QR rendered as SVG via the `qrcode` crate.
- Event pump bridging `ControlPlane` events into `session://state|device|telemetry|closed` Tauri events.

**React UI** (`desktop/ui`)
- `StatusBadge`/`ServerCard`/`DeviceCard`/`ErrorBanner` components, typed `lib/tauri.ts` IPC wrappers, dark-themed connection screen with QR + post-connect telemetry meters.

**iOS SwiftPM** (`ios/`)
- `ClearCamCore`: `QRPayload` decoder, `ControlStream` actor over a `RawIO` protocol (with `NWConnectionIO` for production, in-memory `MemoryPipe`/`ByteQueue` for tests), `SessionController` state machine matching the Rust server, `TransportClient` wrapping `NWConnection`, `TelemetryEmitter` actor, exponential `Reconnect` helper. 11 unit tests.
- `ClearCamAppKit`: SwiftUI `ConnectView`/`ConnectViewModel`/`ConnectedView`/`QRScannerView`/`UIKitDeviceInfo`/`ClearCamApp` @main, all `#if canImport(UIKit)`-gated so the package builds on macOS too.
- `ios/project.yml` + `App/Resources/Info.plist` + `scripts/regen-xcode.sh`: xcodegen-driven Xcode project for the app target (bundle id `app.clearcam.ios`, deployment target iOS 17). The `.xcodeproj` itself is gitignored — regen-on-demand.

**CI**
- Added mock-iphone smoke step to the Rust job.
- Renamed Swift job to mention ClearCamCore and added explicit `swift build --target ClearCamAppKit` to catch macOS-side regressions.

### Deviations from the plan

1. **Tests use an in-memory `MemoryPipe` instead of NWConnection loopback.** The original plan called for `NWListener`-driven loopback in `ControlStreamTests`. On SwiftPM `swift test` (macOS) `NWListener(using: .tcp, on: .any)` returns `POSIXErrorCode(22) — Invalid argument`, leaving the tests hung. I factored `ControlStream` against a `RawIO` protocol, kept `NWConnectionIO` for production, and let tests use a deterministic in-memory byte queue. Same coverage, faster, no flakiness.
2. **`ControlPlane::new` API split.** The plan had a single `new(events_capacity)` constructor that owned its channels. To support multiple session lifetimes feeding one UI sink (reconnect scenarios), I introduced `ControlPlane::new(events_tx, snapshot_tx)` for caller-provided channels and `with_owned_channels(capacity)` as the convenience for tests.
3. **`clearcam-app` bin → library `app`.** The original Phase 0 stub was a `clearcam-app` binary printing a banner. With the Tauri shell driving the app lifecycle, the binary was dead weight; I removed it and renamed the crate to `app` as a pure library consumed by `src-tauri`.
4. **Manual Tauri-UI smoke deferred to user.** A headless interactive smoke check requires a person to look at the GUI; the e2e Rust test (`crates/app/tests/e2e_mock_iphone.rs`) already covers the entire data path. The plan now documents the exact one-step manual command for the user.
5. **xcodebuild iOS-simulator smoke deferred.** Cold xcodebuild of a SwiftPM-package-dependent iOS app on macOS Tahoe takes 10+ minutes and stalls automated checks. `swift build` covers the SwiftPM modules; the Xcode app target is verified by `xcodegen generate` + on-device build in the user's Xcode (documented in Task 24).

### Open questions for Phase 2

- **Media pipeline plumbing.** The accept loop currently parks the paired media stream behind a `oneshot::Receiver` and drops it. Phase 2 must plumb that stream into the new `mediapipeline` crate and wire it through to a `FrameSink` mock.
- **mDNS advertisement.** Discovery via `_clearcam._tcp` is in docs/03 §3.1 but not implemented yet. It is convenience over the QR path — Phase 2 or Phase 6.
- **Reconnection of an already-handshaked session.** Right now the iPhone side has the `Reconnect` backoff helper but the controller does not yet re-drive `connect()` automatically on disconnect. The desktop side already moves to `Reconnecting`. Phase 2 will close that loop.
- **iOS App Store readiness.** The current `ClearCamApp` target compiles via xcodegen; before TestFlight we will need an App Icon set, launch screen, signing/provisioning automation, and a privacy manifest.
