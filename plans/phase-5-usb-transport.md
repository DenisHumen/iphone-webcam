# Phase 5 — USB Transport (usbmuxd) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ClearCam works end-to-end over a Lightning/USB-C cable. The desktop dials the iPhone through **usbmuxd**, the iPhone serves on `NWListener`, and the same CCP handshake / RAW pipeline that Phase 1–2 already ship over Wi-Fi runs unchanged over the cable. Cable wins when both Wi-Fi and USB are available for the same device (ADR-004, ADR-010).

**Architecture:**

- New crate-internal module `transport::usb`. Behind a `UsbConductor` trait, so the production `IdeviceConductor` (pure-Rust async usbmuxd client via the `idevice` crate, ADR-019) and a `LoopbackConductor` (in-process TCP, CI-only) share one surface. The session/handshake/media code already takes `ControlStream`+`MediaStream` and is transport-agnostic — USB just produces another pair of those.
- New `transport::Source` enum (`Wifi` vs `Usb { udid }`) annotates streams so the session layer / UI can tell which path delivered them. WiFi server keeps its existing API; the new path is a *dialer* on the desktop and a *listener* on the iPhone — symmetric to ADR-010.
- Desktop gets a `usb::DeviceWatcher` that polls usbmuxd at ~2 Hz and emits `DeviceConnected{udid}` / `DeviceLost{udid}` events. `AppCore::start_usb_supervisor` reacts: dials the device, runs the handshake, hands the streams to `Session`, and on disconnect, drops the session cleanly.
- App-level pairing (the `pairingKey` path in protocol §4 / ADR-011): on first USB connection without a known key, the iPhone shows a trust sheet; on accept, both sides persist a random 32-byte key (Keychain on iOS, `pairings.toml` on the desktop).
- Channel selection: an `AppCore` rule — if a session is already active over Wi-Fi *and* a USB cable for the same UDID appears, the new USB session supersedes the Wi-Fi one with a clean `BYE`. On USB disconnect, the desktop is willing to accept Wi-Fi for the same device again.
- iOS adds `TransportListener` (NWListener pair on `controlPort`/`mediaPort`). It listens at all times when the app is foregrounded; cable presence is irrelevant from the iOS side — usbmuxd decides who can actually reach the listener.

**Tech Stack:** Rust 1.95 stable, `tokio`, **`idevice` crate** (pure-Rust async usbmuxd client; ADR-019), `keyring`/`directories` for desktop pairing storage, Swift `Network.NWListener`, iOS Keychain via `Security.framework`. No FFI to libimobiledevice in this phase.

**Acceptance:**

- `cargo test --workspace --all-targets` — green; new tests cover `UsbConductor` mock backend, USB dial round-trip through `LoopbackConductor`, pairing-store load/save round-trip, channel-selection rule, and event-driven `DeviceWatcher` emit ordering.
- `swift test` — green; new tests cover `TransportListener` accepts a pair of connections, `PairingStore` Keychain mock, and the trust-prompt state machine.
- `pnpm typecheck && pnpm build` — green; UI shows a `TransportBadge` (Wi-Fi/USB pill on the active session) and a `UsbDevicesList` with a "Trust" CTA on unpaired devices.
- e2e test (Rust, `desktop/crates/app/tests/usb_e2e.rs`): `LoopbackConductor` exposes a fake device; the mock-iPhone (Phase 1) is wired to listen behind it; `AppHandle::start_usb_supervisor()` dials, handshakes, receives `DEVICE_INFO`/`CAMERA_LIST`, and a synthetic RAW frame round-trips through `mediapipeline`.
- e2e test (Rust): with both a Wi-Fi peer and a `LoopbackConductor` peer for the same `udid`, the resulting `AppCore` session reports `transport = Usb` (cable wins).
- CI green on Ubuntu (no real device); tag `v0.5.0-phase5` after sign-off.

**Out of scope (explicitly):**

- Real VideoToolbox encoder/decoder (still Phase 6 polish per Phase 4 retro).
- mid-stream Wi-Fi ↔ USB switchover without dropping the active stream (Phase 6).
- Windows usbmuxd via Apple Mobile Device Support (Phase 7).
- Linux `usbmuxd` packaging beyond a `scripts/setup-linux-usbmuxd.sh` doc-only helper.
- Virtual camera sink (Phase 3, still waiting on Developer ID).
- USB3 capability negotiation beyond the existing `usb3Capable` field in `DEVICE_INFO` (the value is reported, not enforced).

---

## File Structure

### Rust changes

```
desktop/
├── Cargo.toml                                              # MOD: workspace-level deps (idevice, keyring, dirs)
├── crates/
│   ├── ccp-protocol/
│   │   └── src/control/auth.rs                              # MOD: Auth now has Token | PairingKey arms; KeyMaterial helpers
│   ├── transport/
│   │   ├── Cargo.toml                                       # MOD: optional `idevice` dep behind `usb-idevice` feature
│   │   └── src/
│   │       ├── lib.rs                                       # MOD: re-export Source, UsbConductor, usb::*
│   │       ├── source.rs                                    # NEW: Source enum (Wifi / Usb { udid })
│   │       └── usb/
│   │           ├── mod.rs                                   # NEW: module wiring + DeviceWatcher
│   │           ├── conductor.rs                             # NEW: UsbConductor trait + UsbDevice + UsbEvent
│   │           ├── idevice_backend.rs                       # NEW (feature `usb-idevice`): IdeviceConductor
│   │           ├── loopback.rs                              # NEW (cfg(test)+pub): LoopbackConductor (TCP-backed)
│   │           ├── dial.rs                                  # NEW: open_pair() helper — wraps a conductor into (Control, Media)
│   │           └── error.rs                                 # NEW: UsbTransportError
│   ├── session/
│   │   └── src/state.rs                                     # MOD: SessionStateKind gains UsbHandshake; PairingMaterial enum
│   ├── app/
│   │   ├── Cargo.toml                                       # MOD: + transport's usb-idevice feature
│   │   ├── src/
│   │   │   ├── lib.rs                                       # MOD: AppHandle::start_usb_supervisor, transport_source()
│   │   │   ├── pairing_store.rs                             # NEW: PairingStore (toml on disk, per-UDID)
│   │   │   ├── transport_selector.rs                        # NEW: prefer-USB rule
│   │   │   └── usb_supervisor.rs                            # NEW: spawns DeviceWatcher, dials, owns session lifetimes
│   │   └── tests/
│   │       └── usb_e2e.rs                                   # NEW: end-to-end through LoopbackConductor
│   └── tools/
│       └── mock-iphone/src/main.rs                          # MOD: --transport usb (binds NWListener-equivalent on a TcpListener)
├── src-tauri/
│   └── src/
│       ├── commands.rs                                      # MOD: list_usb_devices, trust_usb_device, forget_usb_device
│       ├── events.rs                                        # MOD: session://transport_changed, session://usb_trust_request
│       └── lib.rs                                           # MOD: register PairingStore + usb supervisor
```

### iOS changes

```
ios/Sources/ClearCamCore/
├── Transport/
│   ├── TransportListener.swift                              # NEW: NWListener pair for USB / loopback service
│   ├── TransportClient.swift                                # MOD: factored to share NWConnectionIO with the listener
│   └── TransportRole.swift                                  # NEW: enum Role { client, listener } + factory
├── Pairing/
│   ├── PairingStore.swift                                   # NEW: Keychain-backed key store (was placeholder)
│   ├── TrustController.swift                                # NEW: drives the trust-prompt state machine
│   └── PairingKey.swift                                     # NEW: random-bytes generator + Codable record
├── Session/
│   └── SessionController.swift                              # MOD: accepts inbound streams from TransportListener; honors PairingKey AUTH
└── App/
    └── ClearCamApp.swift                                    # MOD: spins up TransportListener at launch (foreground only)

ios/Tests/ClearCamCoreTests/
├── TransportListenerTests.swift                             # NEW: accept pair of loopback connections
├── PairingStoreTests.swift                                  # NEW: round-trip + replace + delete (uses InMemory provider)
└── TrustControllerTests.swift                               # NEW: prompt → accept → key persisted; prompt → reject → connection closed
```

### UI changes

```
desktop/ui/src/
├── lib/types.ts                                             # MOD: TransportSource = "wifi" | "usb"; UsbDevice; TrustRequest
├── lib/tauri.ts                                             # MOD: listUsbDevices, trustUsbDevice, forgetUsbDevice, onTrustRequest
├── components/
│   ├── TransportBadge.tsx                                   # NEW: pill icon (Wi-Fi / USB) on header
│   ├── UsbDevicesList.tsx                                   # NEW: tray of USB-visible iPhones
│   └── TrustDialog.tsx                                      # NEW: modal triggered by usb_trust_request
├── App.tsx                                                  # MOD: render TransportBadge, mount TrustDialog
└── components/__tests__/TransportBadge.test.tsx             # NEW: snapshot + a11y
```

### Scripts / CI

```
scripts/
├── setup-linux-usbmuxd.sh                                   # NEW: apt-get install + systemd enable + udev hint
└── dev-usb.sh                                                # NEW: starts desktop with mock loopback usbmuxd

.github/workflows/ci.yml                                      # MOD: ensure `cargo test -p transport --features usb-idevice` runs in macOS job only
```

---

## Conventions

- TDD per task: red → green → commit. **Run** the test command shown; do not just stare at code.
- Keep the existing `ControlStream`/`MediaStream` API untouched. USB only adds a new producer for `(ControlStream, MediaStream)`.
- All platform-conditional code is `#[cfg(feature = "usb-idevice")]` (Rust) or `#if canImport(Network)` (Swift). CI on Ubuntu must compile and test the `transport` and `app` crates *without* `usb-idevice`.
- `pub` surface that ends up in `lib.rs` re-exports only — internal modules stay `pub(crate)`.
- No `unwrap()` outside tests. New errors return `UsbTransportError`/`PairingError`; map to `SessionError` at the seams.
- `info!` for lifecycle events (device connected/lost, handshake start/end). `warn!` for retryable failures. `error!` only for things that surface in the UI.
- Russian docs / English code (ADR-020). Comments may be Russian where it clarifies *why*.
- Each task ends with a single `git commit` (no batching). Commit message format: `feat(<crate>): <imperative>` or `test(<crate>): ...`; protocol-touching commits prefix `feat(protocol)`.

---

# Section 5A — Desktop USB transport: conductor abstraction

### Task 1: Add `idevice` dep + verify connect-to-port API

**Why first:** the `idevice` crate's `connect_to_device` (or equivalent) name is the only thing we cannot read from existing source. We need to pin it before designing the trait.

**Files:**
- Modify: `desktop/Cargo.toml` (workspace deps table)
- Modify: `desktop/crates/transport/Cargo.toml`

- [ ] **Step 1: Add the dep, gated by a feature flag.**

In `desktop/Cargo.toml` workspace section:

```toml
[workspace.dependencies]
# ... existing ...
idevice = { version = "0.2", default-features = false, features = ["usbmuxd", "tokio"] }
keyring = "3"
directories = "5"
```

In `desktop/crates/transport/Cargo.toml`:

```toml
[features]
default = []
usb-idevice = ["dep:idevice"]

[dependencies]
# ... existing ...
idevice = { workspace = true, optional = true }
```

- [ ] **Step 2: Verify the API.**

Run:

```bash
cd /Users/denisgumen/Desktop/code/iphone-webcam/desktop
cargo doc -p idevice --no-deps --features usbmuxd
open target/doc/idevice/usbmuxd/index.html
```

Identify the function that takes `(device_id: u32, port: u16, label: &str)` and returns an `AsyncRead + AsyncWrite` handle. (Best candidate per the crate's design: `UsbmuxdConnection::connect_to_device(device_id, port, label) -> Result<Idevice, IdeviceError>` where `Idevice` itself implements `AsyncRead + AsyncWrite`.) Record the **exact** path and the type returned in an ADR stub at `docs/12-decisions-log.md` (ADR-022 below).

- [ ] **Step 3: Draft ADR-022 in `docs/12-decisions-log.md`.**

Append to the "Принятые решения (ADR)" section:

```markdown
### ADR-022 ✅ USB Transport — pure-Rust `idevice` (locked-in choice from ADR-019)
- **Решение:** использовать крейт `idevice` (фич `usbmuxd` + `tokio`); конкретная функция `<…>`,
  возвращающая `<…>` (заполнено по результатам Task 1 Фазы 5).
- **Обоснование:** ADR-019 предусматривал подтверждение в Фазе 5; крейт собирается из коробки на
  macOS/Linux/Win, не требует C-зависимостей, async/Tokio из коробки.
- **Последствия:** на Windows нужен Apple Mobile Device Support (входит в iTunes/драйверы Apple);
  на Linux — пакет `usbmuxd` (см. `scripts/setup-linux-usbmuxd.sh`).
```

- [ ] **Step 4: Build with the feature.**

```bash
cargo build -p transport --features usb-idevice
```

Expected: success. If the dep version drifts, lower to the latest `0.x` and re-pin.

- [ ] **Step 5: Commit.**

```bash
git add desktop/Cargo.toml desktop/crates/transport/Cargo.toml docs/12-decisions-log.md
git commit -m "feat(transport): add idevice dep behind usb-idevice feature; ADR-022"
```

---

### Task 2: `Source` enum on streams

**Files:**
- Create: `desktop/crates/transport/src/source.rs`
- Modify: `desktop/crates/transport/src/streams.rs`
- Modify: `desktop/crates/transport/src/lib.rs`

- [ ] **Step 1: Write the failing test.**

Append to `desktop/crates/transport/src/streams.rs`:

```rust
#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::source::Source;

    #[test]
    fn control_stream_carries_source_label() {
        let cs = ControlStream::from_halves(
            PeerInfo {
                addr: "127.0.0.1:0".parse().unwrap(),
                source: Source::Usb { udid: "ABCD-1234".into() },
            },
            tokio::io::empty(),
            tokio::io::sink(),
        );
        assert!(matches!(cs.peer.source, Source::Usb { .. }));
        assert_eq!(cs.peer.source.udid(), Some("ABCD-1234"));
    }
}
```

- [ ] **Step 2: Run; expect failure (missing `Source`, missing `peer.source`).**

```bash
cargo test -p transport source_tests::control_stream_carries_source_label
```

Expected: compile error `cannot find type Source` and `no field source on type PeerInfo`.

- [ ] **Step 3: Create `source.rs`.**

```rust
//! Provenance tag attached to every (Control|Media)Stream so consumers can
//! distinguish Wi-Fi from USB without inspecting socket internals.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Wifi,
    Usb { udid: String },
}

impl Source {
    pub fn udid(&self) -> Option<&str> {
        match self {
            Source::Usb { udid } => Some(udid),
            Source::Wifi => None,
        }
    }

    pub fn is_usb(&self) -> bool {
        matches!(self, Source::Usb { .. })
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Wifi => write!(f, "wifi"),
            Source::Usb { udid } => write!(f, "usb({udid})"),
        }
    }
}
```

- [ ] **Step 4: Wire `peer.source`.**

In `streams.rs`:

```rust
use crate::source::Source;

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub source: Source,
}
```

Update all existing `PeerInfo { addr }` constructions in this file's tests to `PeerInfo { addr, source: Source::Wifi }`. Same for `wifi::{client,server}.rs` and `tools/mock-iphone/src/main.rs` (any place that constructs `PeerInfo` literally).

Add to `lib.rs`:

```rust
pub mod source;
pub use source::Source;
```

- [ ] **Step 5: Run and expect green.**

```bash
cargo test -p transport
cargo test -p session
cargo test -p mediapipeline
cargo test -p app
```

Expected: green across the workspace. If a downstream crate prints `pattern does not bind any fields`, that crate matched on the old `PeerInfo`; add the new field there.

- [ ] **Step 6: Commit.**

```bash
git add -u
git commit -m "feat(transport): introduce Source enum on PeerInfo (Wifi|Usb{udid})"
```

---

### Task 3: `UsbConductor` trait + `UsbDevice` + events

**Files:**
- Create: `desktop/crates/transport/src/usb/mod.rs`
- Create: `desktop/crates/transport/src/usb/conductor.rs`
- Create: `desktop/crates/transport/src/usb/error.rs`

- [ ] **Step 1: Write the failing test (mock conductor compiles).**

Create `desktop/crates/transport/src/usb/conductor.rs` shell with the trait skeleton, then in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Minimal in-test stub: returns one device, supports no actual TCP.
    #[derive(Default, Clone)]
    struct StubConductor {
        device: Arc<Mutex<Option<UsbDevice>>>,
    }

    #[async_trait::async_trait]
    impl UsbConductor for StubConductor {
        async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError> {
            Ok(self.device.lock().await.iter().cloned().collect())
        }
        async fn open_port(
            &self,
            _device_id: u32,
            _port: u16,
            _label: &str,
        ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError> {
            Err(UsbTransportError::Unsupported)
        }
    }

    #[tokio::test]
    async fn stub_lists_zero_or_one_device() {
        let stub = StubConductor::default();
        assert!(stub.list_devices().await.unwrap().is_empty());
        *stub.device.lock().await = Some(UsbDevice {
            id: 7,
            udid: "ABCD".into(),
            connection: ConnectionType::Usb,
            product_id: Some(0x12A8),
        });
        let v = stub.list_devices().await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].udid, "ABCD");
    }
}
```

- [ ] **Step 2: Run; expect failure (types not defined).**

```bash
cargo test -p transport usb::conductor::tests::stub_lists_zero_or_one_device
```

Expected: compile errors for `UsbConductor`, `UsbDevice`, `UsbTransportError`, `BoxedReader`, `BoxedWriter`, `ConnectionType`.

- [ ] **Step 3: Implement the types.**

In `desktop/crates/transport/src/usb/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UsbTransportError {
    #[error("usbmuxd not reachable: {0}")]
    MuxdUnreachable(String),
    #[error("device not found: {udid}")]
    DeviceNotFound { udid: String },
    #[error("connect to port {port} failed: {reason}")]
    ConnectFailed { port: u16, reason: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported operation in this backend")]
    Unsupported,
    #[cfg(feature = "usb-idevice")]
    #[error("idevice: {0}")]
    Idevice(String),
}
```

In `desktop/crates/transport/src/usb/conductor.rs`:

```rust
use std::pin::Pin;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use super::error::UsbTransportError;

pub type BoxedReader = Pin<Box<dyn AsyncRead + Send + Unpin>>;
pub type BoxedWriter = Pin<Box<dyn AsyncWrite + Send + Unpin>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    /// Some backends (idevice) also report network-only devices; we skip them.
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDevice {
    pub id: u32,        // usbmuxd device id (stable while plugged in)
    pub udid: String,   // 25–40 char Apple UDID
    pub connection: ConnectionType,
    pub product_id: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbEvent {
    Connected(UsbDevice),
    Lost { id: u32, udid: String },
}

#[async_trait]
pub trait UsbConductor: Send + Sync + 'static {
    async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError>;

    async fn open_port(
        &self,
        device_id: u32,
        port: u16,
        label: &str,
    ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError>;

    /// Polling fallback: default implementation diffs `list_devices()` every
    /// `poll_interval`. Backends that have a native event stream (idevice's
    /// `subscribe_events`) should override.
    fn subscribe(self: std::sync::Arc<Self>, poll_interval: std::time::Duration)
        -> mpsc::Receiver<UsbEvent>
    where
        Self: Sized,
    {
        let (tx, rx) = mpsc::channel(16);
        let me = self.clone();
        tokio::spawn(async move {
            let mut prev: Vec<UsbDevice> = Vec::new();
            loop {
                tokio::time::sleep(poll_interval).await;
                let now = match me.list_devices().await {
                    Ok(d) => d.into_iter().filter(|d| matches!(d.connection, ConnectionType::Usb)).collect::<Vec<_>>(),
                    Err(e) => {
                        tracing::debug!(error = %e, "usb list_devices failed");
                        continue;
                    }
                };
                // emit "connected" for new ids
                for d in &now {
                    if !prev.iter().any(|p| p.id == d.id) {
                        let _ = tx.send(UsbEvent::Connected(d.clone())).await;
                    }
                }
                // emit "lost" for missing ids
                for p in &prev {
                    if !now.iter().any(|d| d.id == p.id) {
                        let _ = tx.send(UsbEvent::Lost { id: p.id, udid: p.udid.clone() }).await;
                    }
                }
                prev = now;
            }
        });
        rx
    }
}
```

In `desktop/crates/transport/src/usb/mod.rs`:

```rust
pub mod conductor;
pub mod error;

pub use conductor::{
    BoxedReader, BoxedWriter, ConnectionType, UsbConductor, UsbDevice, UsbEvent,
};
pub use error::UsbTransportError;
```

In `desktop/crates/transport/src/lib.rs`, add:

```rust
pub mod usb;
pub use usb::{UsbConductor, UsbDevice, UsbEvent, UsbTransportError};
```

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p transport usb
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/crates/transport/src/usb desktop/crates/transport/src/lib.rs
git commit -m "feat(transport): UsbConductor trait + UsbDevice/UsbEvent + polling-fallback subscribe"
```

---

### Task 4: `LoopbackConductor` — TCP-backed stand-in for CI

This is the workhorse for every USB test. It binds two real TcpListeners (one per port we expect — control + media), maps `device_id` → `(host, port_offset)`, and `open_port` just dials the listener.

**Files:**
- Create: `desktop/crates/transport/src/usb/loopback.rs`
- Modify: `desktop/crates/transport/src/usb/mod.rs`

- [ ] **Step 1: Write the failing test.**

In `loopback.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn list_devices_returns_registered() {
        let lb = LoopbackConductor::new();
        lb.register(LoopbackDevice {
            id: 1,
            udid: "TEST-UDID".into(),
            control_addr: "127.0.0.1:0".parse().unwrap(),
            media_addr: "127.0.0.1:0".parse().unwrap(),
        })
        .await;
        let devs = lb.list_devices().await.unwrap();
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].udid, "TEST-UDID");
    }

    #[tokio::test]
    async fn open_port_dials_registered_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let lb = LoopbackConductor::new();
        lb.register(LoopbackDevice {
            id: 7,
            udid: "X".into(),
            control_addr: addr,
            media_addr: "127.0.0.1:0".parse().unwrap(),
        })
        .await;

        let dial = tokio::spawn(async move {
            let (mut r, mut w) = lb.open_port(7, 7000, "test").await.unwrap();
            w.write_all(b"hello").await.unwrap();
            let mut buf = [0u8; 5];
            r.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"world");
        });

        let (mut server_sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 5];
        server_sock.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
        server_sock.write_all(b"world").await.unwrap();
        dial.await.unwrap();
    }
}
```

- [ ] **Step 2: Run; expect failure.**

```bash
cargo test -p transport usb::loopback
```

- [ ] **Step 3: Implement `LoopbackConductor`.**

```rust
//! In-process stand-in for `usbmuxd`. Tests can register virtual devices,
//! each of which points to a (real) TCP listener on `127.0.0.1`. `open_port`
//! routes by the protocol's `cport`/`mport` constants:
//! * any `port == ports.control` → dials `device.control_addr`
//! * any `port == ports.media`   → dials `device.media_addr`
//!
//! This lets the rest of the test stack (handshake, media pipeline) run end-
//! to-end against a real socketpair without depending on a real iPhone.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use super::conductor::{
    BoxedReader, BoxedWriter, ConnectionType, UsbConductor, UsbDevice,
};
use super::error::UsbTransportError;

#[derive(Debug, Clone)]
pub struct LoopbackDevice {
    pub id: u32,
    pub udid: String,
    pub control_addr: SocketAddr,
    pub media_addr: SocketAddr,
}

#[derive(Default)]
pub struct LoopbackConductor {
    devices: Arc<Mutex<Vec<LoopbackDevice>>>,
    /// CCP fixed ports (mirror docs/03 §2). Set on construction; tests can
    /// override via `with_ports`.
    pub control_port: u16,
    pub media_port: u16,
}

impl LoopbackConductor {
    pub const DEFAULT_CONTROL_PORT: u16 = 7000;
    pub const DEFAULT_MEDIA_PORT: u16 = 7001;

    pub fn new() -> Self {
        Self {
            devices: Arc::new(Mutex::new(Vec::new())),
            control_port: Self::DEFAULT_CONTROL_PORT,
            media_port: Self::DEFAULT_MEDIA_PORT,
        }
    }

    pub fn with_ports(mut self, control: u16, media: u16) -> Self {
        self.control_port = control;
        self.media_port = media;
        self
    }

    pub async fn register(&self, dev: LoopbackDevice) {
        self.devices.lock().await.push(dev);
    }

    pub async fn unregister(&self, id: u32) {
        self.devices.lock().await.retain(|d| d.id != id);
    }

    pub fn arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Convenience: drives the polling-based `subscribe` once and returns the
    /// initial snapshot (used by deterministic tests instead of sleeping).
    pub async fn snapshot(&self) -> Vec<UsbDevice> {
        self.list_devices().await.unwrap_or_default()
    }
}

#[async_trait]
impl UsbConductor for LoopbackConductor {
    async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError> {
        let lock = self.devices.lock().await;
        Ok(lock
            .iter()
            .map(|d| UsbDevice {
                id: d.id,
                udid: d.udid.clone(),
                connection: ConnectionType::Usb,
                product_id: None,
            })
            .collect())
    }

    async fn open_port(
        &self,
        device_id: u32,
        port: u16,
        _label: &str,
    ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError> {
        let lock = self.devices.lock().await;
        let dev = lock.iter().find(|d| d.id == device_id).cloned().ok_or_else(|| {
            UsbTransportError::DeviceNotFound {
                udid: format!("#{device_id}"),
            }
        })?;
        drop(lock);
        let target = if port == self.control_port {
            dev.control_addr
        } else if port == self.media_port {
            dev.media_addr
        } else {
            return Err(UsbTransportError::ConnectFailed {
                port,
                reason: "loopback conductor only routes the two CCP ports".into(),
            });
        };
        let sock =
            tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(target))
                .await
                .map_err(|_| UsbTransportError::ConnectFailed {
                    port,
                    reason: "timeout".into(),
                })?
                .map_err(|e| UsbTransportError::ConnectFailed {
                    port,
                    reason: e.to_string(),
                })?;
        let (r, w) = tokio::io::split(sock);
        Ok((Box::pin(r), Box::pin(w)))
    }
}
```

In `usb/mod.rs`:

```rust
pub mod loopback;
pub use loopback::{LoopbackConductor, LoopbackDevice};
```

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p transport usb::loopback
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/crates/transport/src/usb/loopback.rs desktop/crates/transport/src/usb/mod.rs
git commit -m "feat(transport): LoopbackConductor for CI-friendly USB testing"
```

---

### Task 5: `open_pair` — dial both ports through any conductor

This is the function the supervisor calls. It wraps a conductor's two raw streams into typed `ControlStream` + `MediaStream` with `Source::Usb`.

**Files:**
- Create: `desktop/crates/transport/src/usb/dial.rs`
- Modify: `desktop/crates/transport/src/usb/mod.rs`

- [ ] **Step 1: Write the failing test (against LoopbackConductor + a tiny server pair).**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::loopback::{LoopbackConductor, LoopbackDevice};
    use crate::wifi::server::BoundPortsWithListeners;
    use crate::WifiServerEvent;
    use ccp_protocol::{Bye, ControlEnvelope, ControlMessage};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn dial_pair_then_round_trip_control_envelope() {
        // Use the existing Wi-Fi server listeners as a stand-in for "the
        // service listening inside the device": they bind real TCP ports,
        // accept connections, give us back a ControlStream/MediaStream.
        let bound = BoundPortsWithListeners::bind(
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        let cport = bound.bound.control;
        let mport = bound.bound.media;
        let (tx, mut rx) = mpsc::channel(8);
        bound.spawn(tx);

        let lb = LoopbackConductor::new()
            .with_ports(cport.port(), mport.port());
        lb.register(LoopbackDevice {
            id: 1,
            udid: "UDID-1".into(),
            control_addr: cport,
            media_addr: mport,
        })
        .await;

        let (mut client_ctl, _client_media) =
            open_pair(&lb, 1, "UDID-1", "test").await.unwrap();
        assert_eq!(client_ctl.peer.source, crate::Source::Usb { udid: "UDID-1".into() });

        // Drain events until we see the server-side control stream.
        let mut server_ctl = loop {
            match rx.recv().await.unwrap() {
                WifiServerEvent::Control(c) => break c,
                WifiServerEvent::Media(_) => continue,
            }
        };

        let env = ControlEnvelope {
            seq: 42,
            ack: None,
            body: ControlMessage::Bye(Bye { reason: "ok".into() }),
        };
        client_ctl.send(&env).await.unwrap();
        let got = server_ctl.recv().await.unwrap();
        assert_eq!(got, env);
    }
}
```

- [ ] **Step 2: Run; expect failure (function not defined).**

```bash
cargo test -p transport usb::dial
```

- [ ] **Step 3: Implement.**

```rust
//! Adapter that turns a conductor + device-id into the canonical
//! `(ControlStream, MediaStream)` pair the rest of the stack expects.

use std::net::Ipv4Addr;

use crate::source::Source;
use crate::streams::{ControlStream, MediaStream, PeerInfo};

use super::conductor::UsbConductor;
use super::error::UsbTransportError;

pub const CCP_USB_CONTROL_PORT: u16 = 7000;
pub const CCP_USB_MEDIA_PORT: u16 = 7001;

/// Open control + media streams to the given device through `conductor`.
/// On any failure half-way through, the partial control stream is dropped
/// (closes the socket) — there is no resource leak.
pub async fn open_pair<C: UsbConductor + ?Sized>(
    conductor: &C,
    device_id: u32,
    udid: &str,
    label: &str,
) -> Result<(ControlStream, MediaStream), UsbTransportError> {
    let (cr, cw) = conductor
        .open_port(device_id, CCP_USB_CONTROL_PORT, label)
        .await?;
    let (mr, mw) = conductor
        .open_port(device_id, CCP_USB_MEDIA_PORT, label)
        .await?;
    let peer_ctl = PeerInfo {
        // usbmuxd doesn't surface a real socket addr; we use 0.0.0.0:0 as a
        // sentinel and rely on `source` for routing decisions.
        addr: (Ipv4Addr::UNSPECIFIED, 0).into(),
        source: Source::Usb { udid: udid.into() },
    };
    let peer_media = peer_ctl.clone();
    let control = ControlStream::from_halves(peer_ctl, cr, cw);
    let media = MediaStream::from_halves(peer_media, mr, mw);
    Ok((control, media))
}
```

Add a tiny constructor to `MediaStream` (mirror `ControlStream::from_halves`) — it doesn't have one yet:

```rust
// in streams.rs
impl MediaStream {
    pub fn from_halves<R, W>(peer: PeerInfo, r: R, w: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        Self {
            peer,
            reader: Box::pin(r),
            writer: Box::pin(w),
        }
    }
}
```

Re-export from `usb/mod.rs`:

```rust
pub mod dial;
pub use dial::{open_pair, CCP_USB_CONTROL_PORT, CCP_USB_MEDIA_PORT};
```

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p transport usb
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/crates/transport/src/usb/dial.rs desktop/crates/transport/src/usb/mod.rs desktop/crates/transport/src/streams.rs
git commit -m "feat(transport): open_pair() dials control+media through any UsbConductor"
```

---

### Task 6: `IdeviceConductor` — production backend behind the trait

**Files:**
- Create: `desktop/crates/transport/src/usb/idevice_backend.rs`
- Modify: `desktop/crates/transport/src/usb/mod.rs`

- [ ] **Step 1: Skeleton + cfg.**

```rust
//! Pure-Rust production backend over `idevice` crate (ADR-019 / ADR-022).
//! Compiled only when `usb-idevice` feature is on.

#![cfg(feature = "usb-idevice")]

use std::sync::Arc;
use async_trait::async_trait;
use idevice::usbmuxd::{UsbmuxdAddr, UsbmuxdConnection};
use tokio::io::{AsyncRead, AsyncWrite};

use super::conductor::{
    BoxedReader, BoxedWriter, ConnectionType, UsbConductor, UsbDevice,
};
use super::error::UsbTransportError;

pub struct IdeviceConductor {
    addr: UsbmuxdAddr,
}

impl IdeviceConductor {
    pub fn new() -> Result<Self, UsbTransportError> {
        let addr = UsbmuxdAddr::from_env_var()
            .unwrap_or_else(|_| UsbmuxdAddr::default());
        Ok(Self { addr })
    }

    async fn new_muxd(&self) -> Result<UsbmuxdConnection, UsbTransportError> {
        self.addr
            .connect(0)
            .await
            .map_err(|e| UsbTransportError::MuxdUnreachable(e.to_string()))
    }
}

#[async_trait]
impl UsbConductor for IdeviceConductor {
    async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError> {
        let mut muxd = self.new_muxd().await?;
        let devs = muxd
            .get_devices()
            .await
            .map_err(|e| UsbTransportError::Idevice(e.to_string()))?;
        Ok(devs.into_iter().map(map_device).collect())
    }

    async fn open_port(
        &self,
        device_id: u32,
        port: u16,
        label: &str,
    ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError> {
        let mut muxd = self.new_muxd().await?;
        // Exact method name pinned in Task 1 (ADR-022). The call below uses
        // the connect-to-device entrypoint that returns the underlying socket.
        let stream = muxd
            .connect_to_device(device_id, port, label)
            .await
            .map_err(|e| UsbTransportError::ConnectFailed {
                port,
                reason: e.to_string(),
            })?;
        // `Idevice` from the crate exposes the underlying connection split.
        let (r, w) = tokio::io::split(stream);
        Ok((Box::pin(r), Box::pin(w)))
    }
}

fn map_device(d: idevice::usbmuxd::UsbmuxdDevice) -> UsbDevice {
    let connection = if format!("{:?}", d.connection_type).contains("Usb") {
        ConnectionType::Usb
    } else {
        ConnectionType::Network
    };
    UsbDevice {
        id: d.device_id,
        udid: d.udid,
        connection,
        product_id: d.product_id,
    }
}
```

> If Task 1's recorded API differs from `connect_to_device`/`device_id` field names, **update this file in the same commit** as the recorded ADR so the two stay in sync. No placeholders.

- [ ] **Step 2: Build with the feature on.**

```bash
cargo build -p transport --features usb-idevice
```

Expected: success. If the API differs, adjust here (renames are local to this file).

- [ ] **Step 3: Build without the feature.**

```bash
cargo build -p transport
```

Expected: success — the file is gated by `cfg(feature = "usb-idevice")` so its contents disappear.

- [ ] **Step 4: Add the module wire-up.**

In `usb/mod.rs`:

```rust
#[cfg(feature = "usb-idevice")]
pub mod idevice_backend;
#[cfg(feature = "usb-idevice")]
pub use idevice_backend::IdeviceConductor;
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/crates/transport/src/usb/idevice_backend.rs desktop/crates/transport/src/usb/mod.rs
git commit -m "feat(transport): IdeviceConductor (feature usb-idevice) — production usbmuxd backend"
```

---

# Section 5B — Protocol: pairingKey AUTH variant

The protocol already specifies `AUTH` accepts either `token` or `pairingKey` (§4 step 3; ADR-011). The current `ccp-protocol` only models `token`. We extend it without bumping `protoVer` — same `t: "AUTH"`, additional optional field, treated as fallback when `token` is absent.

### Task 7: Extend `Auth` and add wire-format tests

**Files:**
- Modify: `desktop/crates/ccp-protocol/src/control/auth.rs`
- Modify: `desktop/crates/ccp-protocol/src/control/mod.rs`

- [ ] **Step 1: Write the failing test.**

In `auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_with_token_still_decodes() {
        let v: Auth = serde_json::from_str(r#"{"token":"abc"}"#).unwrap();
        assert!(matches!(v, Auth::Token { ref token } if token == "abc"));
    }

    #[test]
    fn json_with_pairing_key_decodes() {
        let v: Auth = serde_json::from_str(r#"{"pairingKey":"AA=="}"#).unwrap();
        assert!(matches!(v, Auth::PairingKey { .. }));
    }

    #[test]
    fn token_round_trip_is_unchanged_on_the_wire() {
        let original = Auth::Token { token: "abc".into() };
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, r#"{"token":"abc"}"#);
    }

    #[test]
    fn pairing_key_round_trip() {
        let original = Auth::PairingKey { pairing_key: "AA==".into() };
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, r#"{"pairingKey":"AA=="}"#);
    }
}
```

- [ ] **Step 2: Run; expect failure (currently `Auth` is a struct, not enum).**

```bash
cargo test -p ccp-protocol auth
```

- [ ] **Step 3: Replace `Auth` with an untagged enum.**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum Auth {
    Token { token: String },
    #[serde(rename_all = "camelCase")]
    PairingKey { pairing_key: String },
}

impl Auth {
    pub fn token(t: impl Into<String>) -> Self {
        Self::Token { token: t.into() }
    }
    pub fn pairing_key(k: impl Into<String>) -> Self {
        Self::PairingKey { pairing_key: k.into() }
    }
}
```

- [ ] **Step 4: Update all call sites** (`session`, `mock-iphone`, tests). Search:

```bash
grep -RIn "Auth {" desktop/crates desktop/tools
```

Replace each `Auth { token: x }` with `Auth::token(x)`. Compile.

- [ ] **Step 5: Run.**

```bash
cargo test --workspace
```

Expected: green.

- [ ] **Step 6: Commit.**

```bash
git add -u
git commit -m "feat(protocol): Auth supports {token} | {pairingKey} (no protoVer bump; spec §4)"
```

---

# Section 5C — Pairing material storage (desktop)

### Task 8: `PairingStore` — per-UDID `pairingKey` persistence

**Files:**
- Create: `desktop/crates/app/src/pairing_store.rs`
- Modify: `desktop/crates/app/Cargo.toml`
- Modify: `desktop/crates/app/src/lib.rs`

- [ ] **Step 1: Deps.**

In `desktop/crates/app/Cargo.toml`:

```toml
[dependencies]
# ... existing ...
directories = { workspace = true }
toml = "0.8"
serde = { workspace = true, features = ["derive"] }
```

- [ ] **Step 2: Write the failing test.**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn round_trip_save_and_load_pairing_key() {
        let tmp = tempdir().unwrap();
        let store = PairingStore::open_at(tmp.path().join("pairings.toml")).await.unwrap();
        store.put("UDID-A", PairingMaterial::new_random()).await.unwrap();
        let key = store.get("UDID-A").await.unwrap();
        assert!(key.is_some());

        let reopened = PairingStore::open_at(tmp.path().join("pairings.toml")).await.unwrap();
        let key2 = reopened.get("UDID-A").await.unwrap();
        assert_eq!(key.unwrap().bytes(), key2.unwrap().bytes());
    }

    #[tokio::test]
    async fn forget_removes_pairing() {
        let tmp = tempdir().unwrap();
        let store = PairingStore::open_at(tmp.path().join("pairings.toml")).await.unwrap();
        store.put("UDID-A", PairingMaterial::new_random()).await.unwrap();
        store.forget("UDID-A").await.unwrap();
        assert!(store.get("UDID-A").await.unwrap().is_none());
    }
}
```

Add to `Cargo.toml [dev-dependencies]`: `tempfile = "3"`.

- [ ] **Step 3: Run; expect failure.**

```bash
cargo test -p app pairing_store
```

- [ ] **Step 4: Implement.**

```rust
//! Persistent per-UDID `pairingKey` storage (ADR-011 §3.4).
//! File layout: TOML at `<config>/clearcam/pairings.toml`:
//!
//! ```toml
//! [pairings."UDID-1"]
//! pairing_key_b64 = "..."
//! paired_at = "2026-05-28T20:00:00Z"
//! ```

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum PairingStoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml decode: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("toml encode: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("base64 decode: {0}")]
    B64(#[from] base64::DecodeError),
    #[error("no config dir")]
    NoConfigDir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingMaterial(Vec<u8>);

impl PairingMaterial {
    pub fn new_random() -> Self {
        let mut buf = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        Self(buf)
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
    pub fn as_b64(&self) -> String {
        STANDARD_NO_PAD.encode(&self.0)
    }
    pub fn from_b64(s: &str) -> Result<Self, PairingStoreError> {
        Ok(Self(STANDARD_NO_PAD.decode(s)?))
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct OnDisk {
    pairings: HashMap<String, OnDiskEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OnDiskEntry {
    pairing_key_b64: String,
    paired_at: String,
}

pub struct PairingStore {
    path: PathBuf,
    data: RwLock<OnDisk>,
}

impl PairingStore {
    pub async fn open_default() -> Result<Self, PairingStoreError> {
        let dirs = directories::ProjectDirs::from("dev", "ClearCam", "ClearCam")
            .ok_or(PairingStoreError::NoConfigDir)?;
        let path = dirs.config_dir().join("pairings.toml");
        Self::open_at(path).await
    }

    pub async fn open_at(path: impl AsRef<Path>) -> Result<Self, PairingStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let data: OnDisk = if path.exists() {
            let text = tokio::fs::read_to_string(&path).await?;
            toml::from_str(&text)?
        } else {
            OnDisk::default()
        };
        Ok(Self {
            path,
            data: RwLock::new(data),
        })
    }

    pub async fn get(&self, udid: &str) -> Result<Option<PairingMaterial>, PairingStoreError> {
        let g = self.data.read().await;
        match g.pairings.get(udid) {
            Some(entry) => Ok(Some(PairingMaterial::from_b64(&entry.pairing_key_b64)?)),
            None => Ok(None),
        }
    }

    pub async fn put(
        &self,
        udid: &str,
        mat: PairingMaterial,
    ) -> Result<(), PairingStoreError> {
        let mut g = self.data.write().await;
        g.pairings.insert(
            udid.to_owned(),
            OnDiskEntry {
                pairing_key_b64: mat.as_b64(),
                paired_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        let text = toml::to_string_pretty(&*g)?;
        tokio::fs::write(&self.path, text).await?;
        Ok(())
    }

    pub async fn forget(&self, udid: &str) -> Result<(), PairingStoreError> {
        let mut g = self.data.write().await;
        g.pairings.remove(udid);
        let text = toml::to_string_pretty(&*g)?;
        tokio::fs::write(&self.path, text).await?;
        Ok(())
    }

    pub async fn list(&self) -> Vec<String> {
        self.data.read().await.pairings.keys().cloned().collect()
    }
}
```

> If `chrono` isn't already in the workspace, add `chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }`. If `rand` and `base64` aren't either, add them to workspace deps.

- [ ] **Step 5: Wire to `app::lib`.**

```rust
pub mod pairing_store;
pub use pairing_store::{PairingMaterial, PairingStore, PairingStoreError};
```

- [ ] **Step 6: Run.**

```bash
cargo test -p app pairing_store
```

- [ ] **Step 7: Commit.**

```bash
git add desktop/crates/app/src/pairing_store.rs desktop/crates/app/Cargo.toml desktop/crates/app/src/lib.rs desktop/Cargo.toml
git commit -m "feat(app): PairingStore — per-UDID pairing_key persistence (toml)"
```

---

# Section 5D — Desktop USB supervisor

### Task 9: `usb_supervisor` — watches devices, dials, owns session lifetime

**Files:**
- Create: `desktop/crates/app/src/usb_supervisor.rs`
- Modify: `desktop/crates/app/src/lib.rs`

- [ ] **Step 1: Write the failing test (uses LoopbackConductor + the existing Wi-Fi server as a stand-in for "the iPhone").**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use transport::usb::{LoopbackConductor, LoopbackDevice};
    use transport::wifi::server::{BoundPortsWithListeners, WifiServerEvent};
    use transport::Source;

    #[tokio::test]
    async fn connect_event_triggers_dial_with_pairing_key() {
        // Set up a TCP listener pair acting as the "iPhone".
        let bound = BoundPortsWithListeners::bind(
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        let cport = bound.bound.control;
        let mport = bound.bound.media;
        let (server_tx, mut server_rx) = tokio::sync::mpsc::channel(8);
        bound.spawn(server_tx);

        let lb = Arc::new(
            LoopbackConductor::new().with_ports(cport.port(), mport.port()),
        );
        lb.register(LoopbackDevice {
            id: 1,
            udid: "UDID-1".into(),
            control_addr: cport,
            media_addr: mport,
        })
        .await;

        let tmp = tempfile::tempdir().unwrap();
        let store = Arc::new(
            PairingStore::open_at(tmp.path().join("p.toml")).await.unwrap(),
        );

        let (dialed_tx, mut dialed_rx) = tokio::sync::mpsc::channel(4);
        let sup = UsbSupervisor::new(lb.clone(), store, dialed_tx);
        let _handle = sup.spawn(Duration::from_millis(50));

        // Verify a dial happens.
        let dialed = tokio::time::timeout(Duration::from_secs(2), dialed_rx.recv())
            .await
            .unwrap()
            .expect("dial event");
        assert_eq!(dialed.peer.source, Source::Usb { udid: "UDID-1".into() });

        // Drain the server side to prove the supervisor really connected.
        let _ = tokio::time::timeout(Duration::from_secs(1), server_rx.recv()).await;
    }
}
```

Add `tempfile = "3"` to `dev-dependencies`.

- [ ] **Step 2: Run; expect failure.**

```bash
cargo test -p app usb_supervisor
```

- [ ] **Step 3: Implement.**

```rust
//! Watches the USB bus for iOS devices; when one appears, it dials the CCP
//! ports and forwards the (control, media) streams to whatever owns sessions
//! (currently a test channel; in production: `AppCore::accept_streams`).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use transport::{
    usb::{open_pair, UsbConductor, UsbEvent},
    ControlStream, MediaStream,
};

use crate::pairing_store::PairingStore;

/// Pair of streams handed back to whoever runs the session loop.
#[derive(Debug)]
pub struct DialedStreams {
    pub udid: String,
    pub control: ControlStream,
    pub media: MediaStream,
    /// `Some` if we already have a pairing key for this UDID; `None` if the
    /// session must run the trust ceremony.
    pub pairing_key_b64: Option<String>,
}

impl DialedStreams {
    pub fn peer(&self) -> &transport::PeerInfo {
        &self.control.peer
    }
}

pub struct UsbSupervisor<C: UsbConductor + ?Sized> {
    conductor: Arc<C>,
    store: Arc<PairingStore>,
    out: mpsc::Sender<DialedStreams>,
    label: String,
}

impl<C: UsbConductor + ?Sized + 'static> UsbSupervisor<C> {
    pub fn new(
        conductor: Arc<C>,
        store: Arc<PairingStore>,
        out: mpsc::Sender<DialedStreams>,
    ) -> Self {
        Self {
            conductor,
            store,
            out,
            label: "ClearCam/0.5".into(),
        }
    }

    pub fn spawn(self, poll_interval: Duration) -> JoinHandle<()> {
        tokio::spawn(self.run(poll_interval))
    }

    async fn run(self, poll_interval: Duration) {
        let mut events = self.conductor.clone().subscribe(poll_interval);
        info!("usb supervisor started");
        while let Some(ev) = events.recv().await {
            match ev {
                UsbEvent::Connected(dev) => {
                    info!(udid = %dev.udid, id = dev.id, "usb device connected");
                    match open_pair(self.conductor.as_ref(), dev.id, &dev.udid, &self.label).await {
                        Ok((control, media)) => {
                            let key = match self.store.get(&dev.udid).await {
                                Ok(Some(k)) => Some(k.as_b64()),
                                _ => None,
                            };
                            if self
                                .out
                                .send(DialedStreams {
                                    udid: dev.udid.clone(),
                                    control,
                                    media,
                                    pairing_key_b64: key,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(e) => {
                            warn!(udid = %dev.udid, error = %e, "usb dial failed");
                        }
                    }
                }
                UsbEvent::Lost { udid, id } => {
                    info!(udid = %udid, id, "usb device lost");
                    // We don't tear down sessions here — Session sees the
                    // socket EOF and unwinds on its own (it already does that
                    // for Wi-Fi). This keeps the supervisor minimal.
                }
            }
        }
    }
}
```

Re-export from `lib.rs`:

```rust
pub mod usb_supervisor;
pub use usb_supervisor::{DialedStreams, UsbSupervisor};
```

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p app usb_supervisor
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/crates/app/src/usb_supervisor.rs desktop/crates/app/src/lib.rs
git commit -m "feat(app): UsbSupervisor — DeviceWatcher loop + open_pair + pairing lookup"
```

---

### Task 10: `transport_selector` — cable wins over Wi-Fi

**Files:**
- Create: `desktop/crates/app/src/transport_selector.rs`
- Modify: `desktop/crates/app/src/lib.rs`

- [ ] **Step 1: Write the failing test.**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_wins_against_wifi_for_same_udid() {
        let s = TransportSelector::new();
        assert_eq!(s.decide_active("UDID-1", &[Candidate::wifi(), Candidate::usb()]), Some(Candidate::usb()));
    }

    #[test]
    fn lone_wifi_is_kept() {
        let s = TransportSelector::new();
        assert_eq!(s.decide_active("UDID-1", &[Candidate::wifi()]), Some(Candidate::wifi()));
    }

    #[test]
    fn no_candidates_means_no_active() {
        let s = TransportSelector::new();
        assert_eq!(s.decide_active("UDID-1", &[]), None);
    }
}
```

- [ ] **Step 2: Run; expect failure.**

```bash
cargo test -p app transport_selector
```

- [ ] **Step 3: Implement.**

```rust
//! Picks the active transport for a given UDID when both Wi-Fi and USB are
//! visible at the same time. Per ADR-004 + ADR-010, the cable wins.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    Wifi,
    Usb,
}

impl Candidate {
    pub fn wifi() -> Self {
        Self::Wifi
    }
    pub fn usb() -> Self {
        Self::Usb
    }
}

#[derive(Default)]
pub struct TransportSelector;

impl TransportSelector {
    pub fn new() -> Self {
        Self
    }

    pub fn decide_active(&self, _udid: &str, candidates: &[Candidate]) -> Option<Candidate> {
        if candidates.contains(&Candidate::Usb) {
            Some(Candidate::Usb)
        } else if candidates.contains(&Candidate::Wifi) {
            Some(Candidate::Wifi)
        } else {
            None
        }
    }
}
```

- [ ] **Step 4: Re-export & run.**

```rust
pub mod transport_selector;
pub use transport_selector::{Candidate, TransportSelector};
```

```bash
cargo test -p app transport_selector
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/crates/app/src/transport_selector.rs desktop/crates/app/src/lib.rs
git commit -m "feat(app): TransportSelector — cable wins over Wi-Fi (ADR-010)"
```

---

# Section 5E — Session integration

### Task 11: `SessionStateKind::UsbHandshake` + accept dialed streams

The Wi-Fi server today produces inbound streams that the session picks up. For USB the streams are *outbound* (we dialed them), but post-handshake everything is the same. Adapt the session entrypoint so it can take **both** sides.

**Files:**
- Modify: `desktop/crates/session/src/state.rs`
- Modify: `desktop/crates/session/src/lib.rs` (or wherever the entrypoint lives)

- [ ] **Step 1: Read current entrypoint.** Locate where `Session` is constructed and where its `handshake` step assumes "server accepted us". Note: the spec (§4) says **the iPhone always sends `HELLO` first** regardless of who dialed whom — so the role logic on the desktop is identical for USB and Wi-Fi: desktop waits for `HELLO`, replies `HELLO_ACK`, then `AUTH`.

- [ ] **Step 2: Write the failing test.**

```rust
#[cfg(test)]
mod usb_state_tests {
    use super::*;

    #[test]
    fn from_streams_via_usb_starts_in_handshake() {
        let s = SessionStateKind::usb_handshake("UDID-1".to_string());
        assert_eq!(s.label(), "usb_handshake");
        assert_eq!(s.udid().as_deref(), Some("UDID-1"));
    }

    #[test]
    fn wifi_state_still_present() {
        let s = SessionStateKind::wifi_handshake();
        assert!(s.udid().is_none());
        assert_eq!(s.label(), "wifi_handshake");
    }
}
```

- [ ] **Step 3: Add the variants.**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStateKind {
    WifiHandshake,
    UsbHandshake { udid: String },
    Authed { session_id: String, transport: TransportTag },
    // ... existing variants kept verbatim
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportTag { Wifi, Usb }

impl SessionStateKind {
    pub fn wifi_handshake() -> Self { Self::WifiHandshake }
    pub fn usb_handshake(udid: String) -> Self { Self::UsbHandshake { udid } }
    pub fn label(&self) -> &'static str {
        match self {
            Self::WifiHandshake => "wifi_handshake",
            Self::UsbHandshake { .. } => "usb_handshake",
            Self::Authed { .. } => "authed",
            // ... existing arms unchanged
        }
    }
    pub fn udid(&self) -> Option<&str> {
        match self {
            Self::UsbHandshake { udid } => Some(udid),
            Self::Authed { transport: TransportTag::Usb, .. } => None, // udid lives elsewhere on Authed
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p session
```

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(session): SessionStateKind::UsbHandshake + TransportTag"
```

---

### Task 12: AppCore — wire `UsbSupervisor` into `AppHandle`

**Files:**
- Modify: `desktop/crates/app/src/lib.rs`

This connects the supervisor's `DialedStreams` channel into whatever code already accepts the Wi-Fi `WifiServerEvent::{Control, Media}` events. The exact glue depends on the current shape; the steps below assume the existing `AppCore` has an `accept_streams(control, media)` method (if it's named differently, adapt — search the code).

- [ ] **Step 1: Read the existing AppCore wiring** (file: `desktop/crates/app/src/lib.rs`). Identify the function that consumes Wi-Fi events. If it's named `handle_wifi_event`, you'll add a sibling `handle_usb_streams(udid, control, media)` next to it.

- [ ] **Step 2: Write the failing integration test.**

In `desktop/crates/app/tests/usb_e2e.rs`:

```rust
//! End-to-end USB path: LoopbackConductor → UsbSupervisor → AppCore session.

use std::sync::Arc;
use std::time::Duration;
use transport::usb::{LoopbackConductor, LoopbackDevice};
use transport::wifi::server::BoundPortsWithListeners;

#[tokio::test]
async fn usb_handshake_completes_against_mock_iphone() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    // 1. Spin up the "iPhone" side using the Wi-Fi server crate; the loopback
    //    conductor will route the desktop's USB-bound dials to these ports.
    let bound = BoundPortsWithListeners::bind(
        "127.0.0.1:0".parse().unwrap(),
        "127.0.0.1:0".parse().unwrap(),
    )
    .await
    .unwrap();
    let cport = bound.bound.control;
    let mport = bound.bound.media;

    // 2. Start the AppCore with both transports.
    let lb = Arc::new(LoopbackConductor::new().with_ports(cport.port(), mport.port()));
    lb.register(LoopbackDevice {
        id: 1,
        udid: "UDID-USB".into(),
        control_addr: cport,
        media_addr: mport,
    })
    .await;

    let tmp = tempfile::tempdir().unwrap();
    let app = app::AppHandle::for_tests_with_usb(lb.clone(), tmp.path()).await;
    app.start_wifi_server(bound).await;
    app.start_usb_supervisor(Duration::from_millis(50)).await;

    // 3. Drive a mock-iPhone client through usbmuxd: connect via LoopbackConductor,
    //    play the handshake. The supervisor dials so we just need the inbound
    //    handshake to land.
    let token = app.qr_token().await; // same QR-style token works for `Auth::Token`
    let mock = tokio::spawn(app::tests_util::mock_iphone_run(
        cport,
        mport,
        token,
    ));

    // 4. Assert the AppCore reports an Authed USB session within 3 s.
    let state = tokio::time::timeout(
        Duration::from_secs(3),
        app.wait_for_authed_via(transport::Source::Usb { udid: "UDID-USB".into() }),
    )
    .await
    .expect("timeout waiting for authed");

    assert!(state.is_some(), "expected authed session via USB");
    mock.abort();
}
```

The test references `AppHandle::for_tests_with_usb`, `AppHandle::start_wifi_server`, `AppHandle::start_usb_supervisor`, `AppHandle::qr_token`, `AppHandle::wait_for_authed_via`, and `app::tests_util::mock_iphone_run`. **These are the contract we will implement next** — write them now.

- [ ] **Step 3: Run; expect failure.**

```bash
cargo test -p app --test usb_e2e
```

- [ ] **Step 4: Implement just enough on `AppHandle`.**

Sketch (adapt to the actual current shape of `AppHandle`):

```rust
impl AppHandle {
    pub async fn for_tests_with_usb(
        conductor: Arc<dyn UsbConductor>,
        config_dir: &std::path::Path,
    ) -> Self {
        let pairing = Arc::new(PairingStore::open_at(config_dir.join("pairings.toml")).await.unwrap());
        // ... initialize AppCore as in Phase 4, but with PairingStore + conductor.
        Self { /* ... */ }
    }

    pub async fn start_usb_supervisor(&self, poll: std::time::Duration) {
        let (tx, mut rx) = mpsc::channel::<DialedStreams>(4);
        let sup = UsbSupervisor::new(self.conductor.clone(), self.pairing.clone(), tx);
        sup.spawn(poll);
        let core = self.core.clone();
        tokio::spawn(async move {
            while let Some(d) = rx.recv().await {
                core.accept_usb_streams(d).await;
            }
        });
    }

    pub async fn wait_for_authed_via(&self, source: transport::Source) -> Option<AuthedSnapshot> {
        let mut sub = self.core.subscribe_state();
        loop {
            let s = sub.borrow().clone();
            if let Some(authed) = s.authed_for(&source) { return Some(authed); }
            if sub.changed().await.is_err() { return None; }
        }
    }
}
```

In `app::tests_util` (new module behind `#[cfg(any(test, feature = "test-util"))]`):

```rust
pub async fn mock_iphone_run(
    cport: std::net::SocketAddr,
    mport: std::net::SocketAddr,
    token: String,
) -> anyhow::Result<()> {
    // Reuse the binary logic from tools/mock-iphone (extract to a lib in Task 16
    // so this can call it instead of duplicating).
    todo!("delegate to mock_iphone::run(...) — wired in Task 16")
}
```

> The `todo!` is on a function whose **definition** is finished in **Task 16** as part of the same PR series — Task 12's failing test waits for Task 16. Run the test until the end of Task 16, not earlier. Mark the test `#[ignore]` here, remove the `#[ignore]` in Task 16.

- [ ] **Step 5: Compile only (no run).**

```bash
cargo build -p app --tests
```

Expected: compiles. Test is ignored.

- [ ] **Step 6: Commit.**

```bash
git add -u
git commit -m "feat(app): AppHandle::start_usb_supervisor + e2e scaffold (mock pending Task 16)"
```

---

# Section 5F — Mock-iPhone gets a USB-listener mode

### Task 13: Refactor `mock-iphone` so its runtime is a library

**Files:**
- Modify: `desktop/tools/mock-iphone/Cargo.toml`
- Create: `desktop/tools/mock-iphone/src/lib.rs`
- Modify: `desktop/tools/mock-iphone/src/main.rs`

- [ ] **Step 1: Add `[lib]` next to the existing `[[bin]]`** in `Cargo.toml`:

```toml
[lib]
name = "mock_iphone"
path = "src/lib.rs"
```

- [ ] **Step 2: Move all of today's `main.rs` non-`main` code into `lib.rs`** as `pub async fn run(args: Args) -> anyhow::Result<()>` + `pub struct Args { ... }`. Keep `main.rs` to argument parsing + `run(args).await`.

- [ ] **Step 3: Run existing tests + bin.**

```bash
cargo build -p mock-iphone --bin mock-iphone
cargo run -p mock-iphone --bin mock-iphone -- --help
```

Expected: prints help unchanged.

- [ ] **Step 4: Commit.**

```bash
git add -u
git commit -m "refactor(mock-iphone): split logic into lib + thin main shim"
```

---

### Task 14: `mock-iphone --transport usb` (listener mode)

**Files:**
- Modify: `desktop/tools/mock-iphone/src/lib.rs`

- [ ] **Step 1: Extend `Args`.**

```rust
#[derive(Debug, Clone, Copy)]
pub enum TransportMode { WifiClient, UsbListener }

impl std::str::FromStr for TransportMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "wifi" | "wifi-client" => Ok(Self::WifiClient),
            "usb"  | "usb-listener" => Ok(Self::UsbListener),
            other => anyhow::bail!("unknown --transport {other:?}"),
        }
    }
}

pub struct Args {
    // ... existing fields ...
    pub transport: TransportMode,
}
```

Update arg-parsing: `--transport wifi|usb`. Default: `wifi`.

- [ ] **Step 2: Implement the listener path.**

```rust
async fn run_usb_listener(args: Args) -> anyhow::Result<()> {
    use tokio::net::TcpListener;
    let cl = TcpListener::bind(format!("127.0.0.1:{}", args.cport)).await?;
    let ml = TcpListener::bind(format!("127.0.0.1:{}", args.mport)).await?;
    info!("listening for desktop dial on {} / {}", args.cport, args.mport);
    let (control_sock, _) = cl.accept().await?;
    let (media_sock, _) = ml.accept().await?;
    run_session(
        ControlStream::from_tcp(PeerInfo { addr: control_sock.peer_addr()?, source: Source::Wifi }, control_sock),
        MediaStream::from_tcp(PeerInfo { addr: media_sock.peer_addr()?, source: Source::Wifi }, media_sock),
        args,
    )
    .await
}
```

Refactor the existing `run` so the per-session loop is `run_session(control, media, args)`, callable from both transports. (Wi-Fi client mode calls it after `wifi_connect`; USB listener mode after `accept`.)

- [ ] **Step 3: Run a manual smoke test.**

In one shell:

```bash
cargo run -p mock-iphone --bin mock-iphone -- \
  --host 127.0.0.1 --cport 17000 --mport 17001 --token testtoken \
  --transport usb
```

In another:

```bash
cargo run -p mock-iphone --bin mock-iphone -- \
  --host 127.0.0.1 --cport 17000 --mport 17001 --token testtoken
```

Expected: the second instance never finishes handshake (no desktop yet). Both processes exit on `Ctrl+C`. This just proves the binary parses both modes; the real wiring is in Task 16.

- [ ] **Step 4: Commit.**

```bash
git add -u
git commit -m "feat(mock-iphone): --transport usb (binds NWListener-equivalent on TcpListener)"
```

---

### Task 15: `app::tests_util::mock_iphone_run` calls the lib

**Files:**
- Modify: `desktop/crates/app/Cargo.toml`
- Modify: `desktop/crates/app/src/lib.rs`

- [ ] **Step 1: Add the dep (test-only).**

```toml
[dev-dependencies]
mock-iphone = { path = "../../tools/mock-iphone", package = "mock-iphone" }
```

(If the crate name in `Cargo.toml` is `mock-iphone`, the path-dep above is correct; `lib.name = "mock_iphone"` makes the import `use mock_iphone::run`.)

- [ ] **Step 2: Fill in `tests_util` from Task 12.**

```rust
#[cfg(any(test, feature = "test-util"))]
pub mod tests_util {
    use std::net::SocketAddr;

    pub async fn mock_iphone_run(
        cport: SocketAddr,
        mport: SocketAddr,
        token: String,
    ) -> anyhow::Result<()> {
        let args = mock_iphone::Args {
            host: cport.ip().to_string(),
            cport: cport.port(),
            mport: mport.port(),
            token,
            width: 1280,
            height: 720,
            fps: 30,
            send_video: true,
            transport: mock_iphone::TransportMode::UsbListener,
        };
        mock_iphone::run(args).await
    }
}
```

- [ ] **Step 3: Remove `#[ignore]` from `usb_e2e.rs`.**

- [ ] **Step 4: Run.**

```bash
cargo test -p app --test usb_e2e
```

Expected: green. If it hangs, double-check ports — `cport.port()` must match the `LoopbackConductor::with_ports` value (it does, by construction).

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "test(app): usb_e2e end-to-end through LoopbackConductor + mock-iphone listener"
```

---

# Section 5G — iOS: TransportListener + Pairing

### Task 16: `TransportRole` + `TransportListener` skeleton

**Files:**
- Create: `ios/Sources/ClearCamCore/Transport/TransportRole.swift`
- Create: `ios/Sources/ClearCamCore/Transport/TransportListener.swift`

- [ ] **Step 1: Write the failing test.**

`ios/Tests/ClearCamCoreTests/TransportListenerTests.swift`:

```swift
import XCTest
import Network
@testable import ClearCamCore

final class TransportListenerTests: XCTestCase {
    func testListenerAcceptsControlAndMediaConnections() async throws {
        let listener = try TransportListener(controlPort: 0, mediaPort: 0)
        let bound = try await listener.start()
        let cport = bound.control
        let mport = bound.media

        async let _control = openClient(to: cport)
        async let _media = openClient(to: mport)

        let pair = try await listener.nextSession()
        XCTAssertNotNil(pair.control)
        XCTAssertNotNil(pair.media)

        listener.stop()
    }

    private func openClient(to port: NWEndpoint.Port) async throws -> NWConnection {
        let conn = NWConnection(host: "127.0.0.1", port: port, using: .tcp)
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
            conn.stateUpdateHandler = { s in
                if case .ready = s { c.resume() }
                if case .failed(let e) = s { c.resume(throwing: e) }
            }
            conn.start(queue: .global())
        }
        return conn
    }
}
```

- [ ] **Step 2: Run; expect failure.**

```bash
cd ios && swift test --filter TransportListenerTests
```

- [ ] **Step 3: Implement `TransportRole`.**

```swift
public enum TransportRole: Sendable {
    case client(host: String, cport: Int, mport: Int)
    case listener(cport: Int, mport: Int)
}
```

- [ ] **Step 4: Implement `TransportListener`.**

```swift
import Foundation
import Network

public enum TransportListenerError: Error {
    case invalidPort
    case startFailed(String)
}

public actor TransportListener {
    public struct BoundPorts: Sendable {
        public let control: NWEndpoint.Port
        public let media:   NWEndpoint.Port
    }

    public struct InboundPair: Sendable {
        public let control: ControlStream
        public let media:   ControlStream
    }

    private let controlPort: NWEndpoint.Port
    private let mediaPort:   NWEndpoint.Port
    private var controlListener: NWListener?
    private var mediaListener:   NWListener?
    private var pendingControl: [NWConnection] = []
    private var pendingMedia:   [NWConnection] = []
    private var sessionContinuations: [CheckedContinuation<InboundPair, Error>] = []

    public init(controlPort: Int, mediaPort: Int) throws {
        guard let c = NWEndpoint.Port(rawValue: UInt16(controlPort == 0 ? 0 : controlPort)),
              let m = NWEndpoint.Port(rawValue: UInt16(mediaPort == 0 ? 0 : mediaPort))
        else { throw TransportListenerError.invalidPort }
        self.controlPort = c
        self.mediaPort   = m
    }

    public func start() async throws -> BoundPorts {
        let cParams = NWParameters.tcp
        cParams.allowLocalEndpointReuse = true
        let mParams = NWParameters.tcp
        mParams.allowLocalEndpointReuse = true
        let cListener = try NWListener(using: cParams, on: controlPort)
        let mListener = try NWListener(using: mParams, on: mediaPort)

        cListener.newConnectionHandler = { [weak self] conn in
            conn.start(queue: .global())
            Task { await self?.acceptControl(conn) }
        }
        mListener.newConnectionHandler = { [weak self] conn in
            conn.start(queue: .global())
            Task { await self?.acceptMedia(conn) }
        }

        cListener.start(queue: .global())
        mListener.start(queue: .global())
        self.controlListener = cListener
        self.mediaListener   = mListener

        // NWListener .port is non-nil once started; poll briefly to be safe.
        for _ in 0..<50 {
            if let cp = cListener.port, let mp = mListener.port {
                return BoundPorts(control: cp, media: mp)
            }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        throw TransportListenerError.startFailed("listener never published a port")
    }

    public func nextSession() async throws -> InboundPair {
        try await withCheckedThrowingContinuation { cont in
            Task { await self.appendContinuation(cont) }
        }
    }

    public func stop() {
        controlListener?.cancel()
        mediaListener?.cancel()
        controlListener = nil
        mediaListener   = nil
    }

    private func appendContinuation(_ cont: CheckedContinuation<InboundPair, Error>) {
        sessionContinuations.append(cont)
        tryMatch()
    }

    private func acceptControl(_ conn: NWConnection) {
        pendingControl.append(conn)
        tryMatch()
    }

    private func acceptMedia(_ conn: NWConnection) {
        pendingMedia.append(conn)
        tryMatch()
    }

    private func tryMatch() {
        while !sessionContinuations.isEmpty, !pendingControl.isEmpty, !pendingMedia.isEmpty {
            let cont = sessionContinuations.removeFirst()
            let cConn = pendingControl.removeFirst()
            let mConn = pendingMedia.removeFirst()
            let pair = InboundPair(
                control: ControlStream(io: NWConnectionIO(connection: cConn)),
                media:   ControlStream(io: NWConnectionIO(connection: mConn))
            )
            cont.resume(returning: pair)
        }
    }
}
```

- [ ] **Step 5: Run; expect green.**

```bash
swift test --filter TransportListenerTests
```

- [ ] **Step 6: Commit.**

```bash
git add ios/Sources/ClearCamCore/Transport/TransportListener.swift ios/Sources/ClearCamCore/Transport/TransportRole.swift ios/Tests/ClearCamCoreTests/TransportListenerTests.swift
git commit -m "feat(ios/transport): TransportListener pair (control + media) for USB mode"
```

---

### Task 17: iOS `PairingStore` (Keychain)

**Files:**
- Create: `ios/Sources/ClearCamCore/Pairing/PairingStore.swift`
- Create: `ios/Sources/ClearCamCore/Pairing/PairingKey.swift`

- [ ] **Step 1: Test (using in-memory provider since CI runs `swift test` on macOS host without entitlements).**

`PairingStoreTests.swift`:

```swift
import XCTest
@testable import ClearCamCore

final class PairingStoreTests: XCTestCase {
    func testRoundTripPutGet() async throws {
        let store = PairingStore(provider: .inMemory())
        let key = PairingKey.random()
        try await store.put(udid: "UDID-A", key: key)
        let got = try await store.get(udid: "UDID-A")
        XCTAssertEqual(got?.bytes, key.bytes)
    }

    func testForgetRemoves() async throws {
        let store = PairingStore(provider: .inMemory())
        try await store.put(udid: "U", key: .random())
        try await store.forget(udid: "U")
        let got = try await store.get(udid: "U")
        XCTAssertNil(got)
    }
}
```

- [ ] **Step 2: Run; expect failure.**

```bash
swift test --filter PairingStoreTests
```

- [ ] **Step 3: Implement.**

`PairingKey.swift`:

```swift
import Foundation

public struct PairingKey: Sendable, Equatable {
    public let bytes: Data
    public init(_ bytes: Data) { self.bytes = bytes }
    public static func random() -> PairingKey {
        var b = Data(count: 32)
        _ = b.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }
        return PairingKey(b)
    }
    public var base64NoPad: String {
        bytes.base64EncodedString().trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }
}
```

`PairingStore.swift`:

```swift
import Foundation
import Security

public enum PairingStoreProvider {
    case keychain(service: String)
    case inMemory

    public static func inMemory() -> Self { .inMemory }
}

public actor PairingStore {
    private let provider: PairingStoreProvider
    private var memory: [String: PairingKey] = [:]

    public init(provider: PairingStoreProvider) {
        self.provider = provider
    }

    public func get(udid: String) async throws -> PairingKey? {
        switch provider {
        case .inMemory:
            return memory[udid]
        case .keychain(let service):
            return try keychainRead(service: service, account: udid)
        }
    }

    public func put(udid: String, key: PairingKey) async throws {
        switch provider {
        case .inMemory:
            memory[udid] = key
        case .keychain(let service):
            try keychainWrite(service: service, account: udid, data: key.bytes)
        }
    }

    public func forget(udid: String) async throws {
        switch provider {
        case .inMemory:
            memory.removeValue(forKey: udid)
        case .keychain(let service):
            try keychainDelete(service: service, account: udid)
        }
    }

    // MARK: keychain backend (skipped in CI; covered by manual on-device tests)

    private func keychainWrite(service: String, account: String, data: Data) throws {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
        ]
        SecItemDelete(q as CFDictionary)
        let status = SecItemAdd(q as CFDictionary, nil)
        if status != errSecSuccess { throw NSError(domain: "PairingStore", code: Int(status)) }
    }

    private func keychainRead(service: String, account: String) throws -> PairingKey? {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        if status != errSecSuccess { throw NSError(domain: "PairingStore", code: Int(status)) }
        guard let data = item as? Data else { return nil }
        return PairingKey(data)
    }

    private func keychainDelete(service: String, account: String) throws {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(q as CFDictionary)
    }
}
```

- [ ] **Step 4: Run.**

```bash
swift test --filter PairingStoreTests
```

- [ ] **Step 5: Commit.**

```bash
git add ios/Sources/ClearCamCore/Pairing/*.swift ios/Tests/ClearCamCoreTests/PairingStoreTests.swift
git commit -m "feat(ios/pairing): PairingStore (Keychain + InMemory provider)"
```

---

### Task 18: `TrustController` — first-USB-connection prompt state machine

**Files:**
- Create: `ios/Sources/ClearCamCore/Pairing/TrustController.swift`
- Create: `ios/Tests/ClearCamCoreTests/TrustControllerTests.swift`

- [ ] **Step 1: Test.**

```swift
import XCTest
@testable import ClearCamCore

@MainActor
final class TrustControllerTests: XCTestCase {
    func testUnpairedDevicePromptsUntilAccept() async throws {
        let store = PairingStore(provider: .inMemory())
        let controller = TrustController(store: store)
        let outcome = Task<TrustOutcome, Never> {
            await controller.requestTrust(forDesktopId: "DESKTOP-1")
        }
        await Task.yield()
        XCTAssertEqual(controller.pendingPrompt?.desktopId, "DESKTOP-1")
        controller.respond(accept: true)
        let result = await outcome.value
        guard case .accepted(let key) = result else { return XCTFail("expected accepted") }
        let stored = try await store.get(udid: "DESKTOP-1")
        XCTAssertEqual(stored?.bytes, key.bytes)
    }

    func testRejectionReturnsDeniedAndStoresNothing() async {
        let store = PairingStore(provider: .inMemory())
        let controller = TrustController(store: store)
        let outcome = Task<TrustOutcome, Never> {
            await controller.requestTrust(forDesktopId: "DESKTOP-2")
        }
        await Task.yield()
        controller.respond(accept: false)
        let result = await outcome.value
        XCTAssertEqual(result, .denied)
    }
}
```

- [ ] **Step 2: Run; expect failure.**

```bash
swift test --filter TrustControllerTests
```

- [ ] **Step 3: Implement.**

```swift
import Foundation

public enum TrustOutcome: Sendable, Equatable {
    case accepted(PairingKey)
    case denied
}

public struct TrustPrompt: Sendable, Equatable {
    public let desktopId: String
}

@MainActor
public final class TrustController: ObservableObject {
    @Published public private(set) var pendingPrompt: TrustPrompt?
    private var pendingContinuation: CheckedContinuation<TrustOutcome, Never>?
    private let store: PairingStore

    public init(store: PairingStore) {
        self.store = store
    }

    public func requestTrust(forDesktopId id: String) async -> TrustOutcome {
        await withCheckedContinuation { (cont: CheckedContinuation<TrustOutcome, Never>) in
            pendingPrompt = TrustPrompt(desktopId: id)
            pendingContinuation = cont
        }
    }

    public func respond(accept: Bool) {
        guard let prompt = pendingPrompt, let cont = pendingContinuation else { return }
        pendingPrompt = nil
        pendingContinuation = nil
        if accept {
            let key = PairingKey.random()
            Task {
                try? await store.put(udid: prompt.desktopId, key: key)
            }
            cont.resume(returning: .accepted(key))
        } else {
            cont.resume(returning: .denied)
        }
    }
}
```

- [ ] **Step 4: Run.**

```bash
swift test --filter TrustControllerTests
```

- [ ] **Step 5: Commit.**

```bash
git add ios/Sources/ClearCamCore/Pairing/TrustController.swift ios/Tests/ClearCamCoreTests/TrustControllerTests.swift
git commit -m "feat(ios/pairing): TrustController — first-connect trust prompt + key persistence"
```

---

### Task 19: SessionController routes USB inbound + honors PairingKey

**Files:**
- Modify: `ios/Sources/ClearCamCore/Session/SessionController.swift`

- [ ] **Step 1: Read the current `SessionController.handshake()` flow.** Identify where it sends `AUTH { token }`. We replace that with logic that:
  1. If we already have a `PairingKey` for the peer ⇒ send `AUTH { pairingKey }`.
  2. Else, request `TrustController.requestTrust(...)`. If `.accepted(key)`, persist and send `AUTH { pairingKey }`. If `.denied`, send `BYE` and close.

- [ ] **Step 2: Test (extends the existing `SessionControllerTests`).**

```swift
func testHandshakeOverInboundUsesPairingKeyWhenStored() async throws {
    let store = PairingStore(provider: .inMemory())
    try await store.put(udid: "PC-1", key: .random())
    // ... construct an inbound ControlStream/MediaStream pair via a loopback
    // ... drive the controller; assert AUTH frame body matches `pairingKey`
}
```

(Detailed code in the existing test style — mirror the Phase-1 `SessionControllerTests` setup.)

- [ ] **Step 3: Implement the branch.** Add a constructor:

```swift
public static func usbInbound(
    pair: TransportListener.InboundPair,
    store: PairingStore,
    trust: TrustController,
    peerId: String   // received in HELLO; until HELLO arrives this is the placeholder; we update on parse
) -> SessionController { ... }
```

Inside the handshake, pick `Auth.pairingKey(...)` vs trust-prompt branch.

- [ ] **Step 4: Run.**

```bash
swift test
```

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(ios/session): SessionController accepts USB inbound + honors PairingKey AUTH"
```

---

# Section 5H — UI: transport indicator + USB tray + trust dialog

### Task 20: Types + Tauri commands

**Files:**
- Modify: `desktop/ui/src/lib/types.ts`
- Modify: `desktop/ui/src/lib/tauri.ts`
- Modify: `desktop/src-tauri/src/commands.rs`
- Modify: `desktop/src-tauri/src/events.rs`

- [ ] **Step 1: TS types.**

In `lib/types.ts`:

```ts
export type TransportSource = "wifi" | "usb";

export interface UsbDevice {
  id: number;
  udid: string;
  productId?: number;
  trusted: boolean;
}

export interface TrustRequest {
  udid: string;
  productId?: number;
}
```

- [ ] **Step 2: Tauri binding.**

In `lib/tauri.ts`:

```ts
export const listUsbDevices = (): Promise<UsbDevice[]> => invoke("list_usb_devices");
export const trustUsbDevice = (udid: string): Promise<void> => invoke("trust_usb_device", { udid });
export const forgetUsbDevice = (udid: string): Promise<void> => invoke("forget_usb_device", { udid });
export const onTransportChanged = (cb: (src: TransportSource) => void) =>
  listen<TransportSource>("session://transport_changed", e => cb(e.payload));
export const onTrustRequest = (cb: (req: TrustRequest) => void) =>
  listen<TrustRequest>("session://usb_trust_request", e => cb(e.payload));
```

- [ ] **Step 3: Rust commands.**

```rust
#[tauri::command]
pub async fn list_usb_devices(state: tauri::State<'_, AppHandle>) -> Result<Vec<UsbDeviceDto>, String> {
    state.list_usb_devices().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trust_usb_device(udid: String, state: tauri::State<'_, AppHandle>) -> Result<(), String> {
    state.trust_usb_device(&udid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn forget_usb_device(udid: String, state: tauri::State<'_, AppHandle>) -> Result<(), String> {
    state.forget_usb_device(&udid).await.map_err(|e| e.to_string())
}
```

`UsbDeviceDto`: serde-derived camelCase struct matching the TS shape.

Register both events in `events.rs`:

```rust
pub const TRANSPORT_CHANGED: &str = "session://transport_changed";
pub const USB_TRUST_REQUEST: &str = "session://usb_trust_request";
```

- [ ] **Step 4: Build + typecheck.**

```bash
cargo build -p ccp-clearcam-tauri
pnpm -C desktop/ui typecheck
```

Expected: green.

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(ui+tauri): list_usb_devices / trust_usb_device commands + events"
```

---

### Task 21: `TransportBadge` component

**Files:**
- Create: `desktop/ui/src/components/TransportBadge.tsx`
- Create: `desktop/ui/src/components/__tests__/TransportBadge.test.tsx`

- [ ] **Step 1: Test.**

```tsx
import { render, screen } from "@testing-library/react";
import { TransportBadge } from "../TransportBadge";

test("renders wifi label", () => {
  render(<TransportBadge source="wifi" />);
  expect(screen.getByRole("status")).toHaveTextContent(/Wi[- ]?Fi/i);
});

test("renders usb label", () => {
  render(<TransportBadge source="usb" />);
  expect(screen.getByRole("status")).toHaveTextContent(/USB/i);
});
```

- [ ] **Step 2: Run; expect failure.**

```bash
pnpm -C desktop/ui test -- TransportBadge
```

- [ ] **Step 3: Implement.**

```tsx
import { TransportSource } from "../lib/types";

interface Props {
  source: TransportSource;
}

export function TransportBadge({ source }: Props) {
  const label = source === "usb" ? "USB" : "Wi-Fi";
  const cls = source === "usb"
    ? "bg-emerald-100 text-emerald-900"
    : "bg-sky-100 text-sky-900";
  return (
    <span
      role="status"
      aria-label={`Active transport: ${label}`}
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${cls}`}
    >
      <span aria-hidden>{source === "usb" ? "🔌" : "📶"}</span>
      {label}
    </span>
  );
}
```

- [ ] **Step 4: Run.**

```bash
pnpm -C desktop/ui test -- TransportBadge
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/ui/src/components/TransportBadge.tsx desktop/ui/src/components/__tests__/TransportBadge.test.tsx
git commit -m "feat(ui): TransportBadge — Wi-Fi/USB indicator pill"
```

---

### Task 22: `UsbDevicesList` + `TrustDialog` + `App.tsx` integration

**Files:**
- Create: `desktop/ui/src/components/UsbDevicesList.tsx`
- Create: `desktop/ui/src/components/TrustDialog.tsx`
- Modify: `desktop/ui/src/App.tsx`

- [ ] **Step 1: Implement `UsbDevicesList`.**

```tsx
import { useEffect, useState } from "react";
import { listUsbDevices, trustUsbDevice, forgetUsbDevice } from "../lib/tauri";
import type { UsbDevice } from "../lib/types";

export function UsbDevicesList() {
  const [devices, setDevices] = useState<UsbDevice[]>([]);
  useEffect(() => {
    let alive = true;
    const tick = async () => {
      try {
        const d = await listUsbDevices();
        if (alive) setDevices(d);
      } catch {/* ignore */}
    };
    tick();
    const handle = setInterval(tick, 1000);
    return () => { alive = false; clearInterval(handle); };
  }, []);
  if (devices.length === 0) {
    return <p className="text-sm text-slate-500">No USB devices.</p>;
  }
  return (
    <ul className="space-y-1">
      {devices.map(d => (
        <li key={d.udid} className="flex items-center justify-between text-sm">
          <span className="font-mono">{d.udid.slice(0, 8)}…</span>
          {d.trusted ? (
            <button onClick={() => forgetUsbDevice(d.udid)} className="text-rose-700 hover:underline">Forget</button>
          ) : (
            <button onClick={() => trustUsbDevice(d.udid)} className="text-emerald-700 hover:underline">Trust</button>
          )}
        </li>
      ))}
    </ul>
  );
}
```

- [ ] **Step 2: Implement `TrustDialog`.**

```tsx
import { useEffect, useState } from "react";
import { onTrustRequest, trustUsbDevice } from "../lib/tauri";
import type { TrustRequest } from "../lib/types";

export function TrustDialog() {
  const [req, setReq] = useState<TrustRequest | null>(null);
  useEffect(() => {
    const off = onTrustRequest(setReq);
    return () => { off.then(u => u()); };
  }, []);
  if (!req) return null;
  return (
    <div role="dialog" aria-modal className="fixed inset-0 bg-black/40 flex items-center justify-center">
      <div className="bg-white rounded-2xl p-6 max-w-md shadow-xl">
        <h2 className="text-lg font-semibold">Trust this iPhone?</h2>
        <p className="text-sm text-slate-600 mt-2">
          UDID <span className="font-mono">{req.udid.slice(0, 12)}…</span> is connected via USB. Allow it to stream to this computer?
        </p>
        <div className="flex justify-end gap-2 mt-4">
          <button onClick={() => setReq(null)} className="px-3 py-1.5 rounded-md text-slate-700 hover:bg-slate-100">Cancel</button>
          <button
            onClick={async () => { await trustUsbDevice(req.udid); setReq(null); }}
            className="px-3 py-1.5 rounded-md bg-emerald-600 text-white hover:bg-emerald-700"
          >Trust</button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Mount in `App.tsx`.**

```tsx
import { TransportBadge } from "./components/TransportBadge";
import { UsbDevicesList } from "./components/UsbDevicesList";
import { TrustDialog } from "./components/TrustDialog";
// ... existing imports

function App() {
  // ... existing state
  const [transport, setTransport] = useState<TransportSource>("wifi");
  useEffect(() => {
    const off = onTransportChanged(setTransport);
    return () => { off.then(u => u()); };
  }, []);
  return (
    <>
      <header className="flex items-center gap-2 px-4 py-2 border-b">
        <h1 className="font-semibold">ClearCam</h1>
        <TransportBadge source={transport} />
      </header>
      {/* ... existing layout */}
      <aside className="border-l p-3 w-56">
        <h3 className="text-xs uppercase tracking-wide text-slate-500 mb-2">USB devices</h3>
        <UsbDevicesList />
      </aside>
      <TrustDialog />
    </>
  );
}
```

- [ ] **Step 4: Run typecheck + build.**

```bash
pnpm -C desktop/ui typecheck
pnpm -C desktop/ui build
```

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(ui): UsbDevicesList + TrustDialog + transport pill in header"
```

---

# Section 5I — Setup script + acceptance + retrospective

### Task 23: `scripts/setup-linux-usbmuxd.sh` + `scripts/dev-usb.sh`

**Files:**
- Create: `scripts/setup-linux-usbmuxd.sh`
- Create: `scripts/dev-usb.sh`

- [ ] **Step 1: Linux setup helper.**

```bash
#!/usr/bin/env bash
set -euo pipefail
# Installs and enables usbmuxd on Debian/Ubuntu. Used in the README's
# "Linux: prerequisites" section. macOS users do not need this; the daemon
# ships with macOS as part of "Apple Mobile Device" launchd service.
if ! command -v apt-get >/dev/null; then
  echo "Non-apt distro detected; please install 'usbmuxd' via your package manager." >&2
  exit 0
fi
sudo apt-get update
sudo apt-get install -y usbmuxd libimobiledevice6 libimobiledevice-utils
sudo systemctl enable --now usbmuxd
echo "OK. Plug an iPhone and run 'idevice_id -l' to verify."
```

- [ ] **Step 2: Local dev helper.**

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Starts desktop with the LoopbackConductor pointed at a mock-iphone listener.
( cargo run -p mock-iphone --bin mock-iphone -- --host 127.0.0.1 --cport 17000 --mport 17001 --token devtoken --transport usb ) &
mock_pid=$!
trap "kill $mock_pid 2>/dev/null || true" EXIT
sleep 0.3
CLEARCAM_USB_LOOPBACK=1 \
CLEARCAM_USB_CPORT=17000 \
CLEARCAM_USB_MPORT=17001 \
CLEARCAM_USB_UDID=DEVICE-DEV \
cd desktop && cargo tauri dev
```

- [ ] **Step 3: Make them executable.**

```bash
chmod +x scripts/setup-linux-usbmuxd.sh scripts/dev-usb.sh
```

- [ ] **Step 4: Wire `CLEARCAM_USB_LOOPBACK` in `AppHandle::new`.** When the env var is set, build the supervisor with `LoopbackConductor` instead of `IdeviceConductor`. Mention this in the README and Acceptance log.

- [ ] **Step 5: Commit.**

```bash
git add scripts/setup-linux-usbmuxd.sh scripts/dev-usb.sh desktop/crates/app/src/lib.rs
git commit -m "chore(scripts): setup-linux-usbmuxd + dev-usb helpers; CLEARCAM_USB_LOOPBACK env"
```

---

### Task 24: Documentation updates

**Files:**
- Modify: `docs/03-protocol.md`
- Modify: `docs/05-desktop-app.md`
- Modify: `docs/12-decisions-log.md`
- Modify: `README.md`

- [ ] **Step 1: Doc deltas.**

- `03-protocol.md` §4: add a paragraph clarifying that AUTH carries either `{token}` or `{pairingKey}` — the existing wire-tables already show both; just add a "JSON schema" subsection.
- `05-desktop-app.md`: add a "USB transport" subsection describing the supervisor + conductor split.
- `12-decisions-log.md`: confirm ADR-022 is final (Task 1 wrote a draft).
- `README.md`: add USB to the quickstart matrix (`scripts/setup-linux-usbmuxd.sh` on Linux; nothing extra on macOS).

- [ ] **Step 2: Run markdown lint.**

```bash
pnpm exec markdownlint-cli2 "docs/**/*.md" "README.md"
```

- [ ] **Step 3: Commit.**

```bash
git add -u
git commit -m "docs: USB transport — protocol detail, supervisor architecture, README quickstart"
```

---

### Task 25: Final gates + acceptance log entry + tag

**Files:**
- Modify: `plans/phase-5-usb-transport.md` (this file — add an Acceptance log section)

- [ ] **Step 1: Run all gates.**

```bash
cd desktop
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build -p transport --features usb-idevice          # macOS / dev box
pnpm -C ui typecheck && pnpm -C ui lint && pnpm -C ui format:check && pnpm -C ui build
cd ../ios && swift test
```

Expected: all green.

- [ ] **Step 2: Append the Acceptance log.**

Add this section to the end of `plans/phase-5-usb-transport.md`:

```markdown
## Acceptance log

### YYYY-MM-DD — Phase 5 USB transport

- **transport** — `UsbConductor` trait, `LoopbackConductor`, `IdeviceConductor` (feature `usb-idevice`); `open_pair` helper; `Source::Usb` annotation. 8 new unit tests.
- **app** — `PairingStore` (TOML), `UsbSupervisor`, `TransportSelector` (USB > Wi-Fi). e2e test `usb_e2e` drives the mock-iPhone through `LoopbackConductor`.
- **iOS** — `TransportListener` (NWListener pair), `PairingStore` (Keychain + InMemory), `TrustController`, `SessionController` USB-inbound path.
- **UI** — `TransportBadge`, `UsbDevicesList`, `TrustDialog`; Tauri commands `list_usb_devices`/`trust_usb_device`/`forget_usb_device`; events `session://transport_changed`, `session://usb_trust_request`.
- **scripts** — `setup-linux-usbmuxd.sh`, `dev-usb.sh`. `CLEARCAM_USB_LOOPBACK=1` flips the supervisor to LoopbackConductor for hostage-free desktop dev.

Gates: cargo fmt/clippy/test ✓ ; transport+usb-idevice build ✓ ; ui typecheck/lint/format/build ✓ ; swift test ✓.

### Deferred to Phase 6 polish

- Real on-device usbmuxd round-trip — needs a physical iPhone.
- Mid-stream Wi-Fi → USB hot-switchover (cable plugged in *during* a session). Current behavior: new USB session supersedes; brief gap is acceptable for v1.
- Reconnect throttling / backoff tuning on `UsbEvent::Lost`.
```

- [ ] **Step 3: Tag.**

```bash
git tag v0.5.0-phase5
git push --tags    # only if the user explicitly asked for a push
```

- [ ] **Step 4: Commit.**

```bash
git add plans/phase-5-usb-transport.md
git commit -m "docs(plans): Phase 5 acceptance log + Phase 6 deferred items"
```

---

## Retrospective (fill in during Task 25)

Write a short retrospective at the bottom of this file under `## Retrospective`, mirroring Phase 4's style. Cover:

1. **What landed** — bullet list, one line per crate.
2. **Deviations from the plan** — anything we adjusted while writing it.
3. **Open questions** — what to verify on a real iPhone first time Phase 5 runs against hardware.
