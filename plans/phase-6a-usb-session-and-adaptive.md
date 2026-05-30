# Phase 6a — USB session integration + adaptive loop wiring

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the highest-leverage Phase 4 + Phase 5 deferrals so end-to-end sessions are functional over both Wi-Fi *and* USB, with the adaptive engine actually steering mode based on live telemetry, and the UI getting real device + transport events.

**Architecture:** `AppCore` gains a real USB-side handshake (consumer of `DialedStreams` runs `accept_control_usb` + pairs media inline; no `MEDIA_HELLO` because supervisor already owns both sockets); the existing `AdaptationState` is wired to `ControlPlaneEvent::Telemetry` with a single-loop driver that emits `SET_MODE` on congestion; iOS `Auth` enum gains a `pairingKey` variant; Tauri commands `list_usb_devices`/`trust_usb_device`/`forget_usb_device` are backed by the live `PairingStore` + `UsbSupervisor`; AppCore emits `session://transport_changed` and `session://usb_trust_request` events; mock-iphone responds to `SET_MODE` with `MODE_APPLIED`.

**Tech Stack:** Rust 1.95, tokio. Swift `Codable` enums. No new system deps.

**Acceptance:**
- `cargo test --workspace --all-targets` green; +6 new integration tests covering USB-handshake via supervisor, adaptation loop driving SET_MODE, Tauri live commands, transport_changed emission.
- `swift test` green; +2 new tests covering `Auth.pairingKey(...)` wire-format and `SessionController.handleSetMode` mock.
- `pnpm -C desktop/ui typecheck && pnpm -C desktop/ui build` green.
- New `desktop/crates/app/tests/usb_full_session_e2e.rs` test reaches `SessionStateKind::Ready` via `UsbSupervisor` (no Wi-Fi server involved).
- New `desktop/crates/app/tests/adaptive_loop_e2e.rs` test scripts telemetry sequence and asserts `SET_MODE` lands on the outbound channel.
- CI green; tag `v0.6.0a-phase6a` after sign-off.

**Out of scope (deferred to Phase 6b/6c):**
- Real VTEncoder / VTDecoder (Phase 6b — VideoToolbox).
- Live ramp speedtest over media socket (Phase 6b).
- ffmpeg decoder adapter (Phase 6c).
- Mid-stream Wi-Fi↔USB switchover (Phase 6c — supervisor + selector currently dial fresh; mid-stream replacement-without-drop is its own piece).
- Reconnect throttling / backoff tuning (Phase 6c).
- CMIO Camera Extension (Phase 3 — blocked on Apple Dev ID; see `docs/14-apple-developer-id-guide.md`).
- iOS NWListener on-device acceptance testing (Phase 6c — needs hardware).

---

## File Structure

### Rust

```
desktop/
├── crates/
│   ├── session/
│   │   └── src/
│   │       ├── handshake.rs                 # MOD: accept_control_usb (PairingKey path)
│   │       └── lib.rs                       # MOD: re-export accept_control_usb
│   └── app/
│       ├── src/
│       │   ├── lib.rs                       # MOD: USB-handshake consumer; transport_changed emitter; live Tauri command backends; adaptation loop
│       │   └── adaptation_driver.rs         # NEW: glue between ControlPlaneEvent::Telemetry → AdaptationState → SET_MODE
│       └── tests/
│           ├── usb_full_session_e2e.rs      # NEW: supervisor → AppCore → Ready
│           └── adaptive_loop_e2e.rs         # NEW: scripted telemetry → SET_MODE on wire
├── src-tauri/
│   └── src/
│       ├── commands.rs                      # MOD: list_usb_devices/trust_usb_device/forget_usb_device hit live state
│       └── lib.rs                           # MOD: state holds Arc<PairingStore>, Arc<dyn UsbConductor>; spawn supervisor on startup
└── tools/
    └── mock-iphone/
        └── src/lib.rs                       # MOD: handle SET_MODE → emit MODE_APPLIED
```

### iOS

```
ios/Sources/ClearCamProtocol/
└── Handshake.swift                          # MOD: Auth becomes enum { token | pairingKey }

ios/Sources/ClearCamCore/Session/
└── SessionController.swift                  # MOD: connect(credential:) uses new Auth.pairingKey for USB; handleSetMode emits MODE_APPLIED

ios/Tests/ClearCamCoreTests/
├── AuthCodableTests.swift                   # NEW: Auth.token / Auth.pairingKey wire round-trip
└── SetModeRoundTripTests.swift              # NEW: SessionController responds to SET_MODE with MODE_APPLIED
```

---

## Conventions

- TDD per task. Red → green → commit.
- Russian prose in docs, English code (ADR-020).
- No `unwrap()` outside tests; surface real errors with `tracing::warn!`/`error!`.
- Reuse existing crate patterns: `ControlPlane::run_with_outbound`, `MediaPipeline::spawn`, `AdaptationState`.
- Each task ends with a single `git commit` — message format `feat(<crate>): <…>` / `test(<crate>): <…>`.

---

# Section 6a-A — Protocol layer

### Task 1: iOS `Auth` becomes a discriminated enum (Token | PairingKey)

**Files:**
- Modify: `ios/Sources/ClearCamProtocol/Handshake.swift`
- Create: `ios/Tests/ClearCamCoreTests/AuthCodableTests.swift`
- Modify: `ios/Sources/ClearCamCore/Session/SessionController.swift` (call-site update)

- [ ] **Step 1: Write the failing test.**

`ios/Tests/ClearCamCoreTests/AuthCodableTests.swift`:

```swift
import XCTest
@testable import ClearCamProtocol

final class AuthCodableTests: XCTestCase {
    func testTokenEncodesWithTokenField() throws {
        let auth = Auth.token("abc")
        let data = try JSONEncoder().encode(auth)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertEqual(json, #"{"token":"abc"}"#)
    }

    func testPairingKeyEncodesWithCamelCaseField() throws {
        let auth = Auth.pairingKey("KEY-B64")
        let data = try JSONEncoder().encode(auth)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertEqual(json, #"{"pairingKey":"KEY-B64"}"#)
    }

    func testTokenDecodes() throws {
        let auth = try JSONDecoder().decode(Auth.self, from: Data(#"{"token":"x"}"#.utf8))
        XCTAssertEqual(auth, .token("x"))
    }

    func testPairingKeyDecodes() throws {
        let auth = try JSONDecoder().decode(Auth.self, from: Data(#"{"pairingKey":"k"}"#.utf8))
        XCTAssertEqual(auth, .pairingKey("k"))
    }
}
```

- [ ] **Step 2: Run; expect failure.**

```bash
cd ios && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --filter AuthCodableTests
```

Expected: compile error — `Auth.token(...)` factory doesn't exist; Auth is a struct.

- [ ] **Step 3: Replace `Auth` struct with enum.**

In `ios/Sources/ClearCamProtocol/Handshake.swift`, replace the existing `public struct Auth: Codable, Equatable { ... }` block with:

```swift
public enum Auth: Codable, Equatable, Sendable {
    case token(String)
    case pairingKey(String)

    public static func token(_ value: String) -> Auth { .token(value) }
    public static func pairingKey(_ value: String) -> Auth { .pairingKey(value) }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .token(let v):      try c.encode(v, forKey: .token)
        case .pairingKey(let v): try c.encode(v, forKey: .pairingKey)
        }
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        if let token = try c.decodeIfPresent(String.self, forKey: .token) {
            self = .token(token)
            return
        }
        if let pk = try c.decodeIfPresent(String.self, forKey: .pairingKey) {
            self = .pairingKey(pk)
            return
        }
        throw DecodingError.dataCorrupted(.init(codingPath: c.codingPath,
            debugDescription: "AUTH frame has neither token nor pairingKey"))
    }

    private enum CodingKeys: String, CodingKey {
        case token
        case pairingKey
    }
}
```

- [ ] **Step 4: Update SessionController call sites.**

In `ios/Sources/ClearCamCore/Session/SessionController.swift`, find any place that currently writes `Auth(token: ...)` (the struct initializer). Replace with `Auth.token(...)` or `Auth.pairingKey(...)` based on the `AuthCredential` case:

```swift
let authBody: Auth
switch credential {
case .token(let t):      authBody = .token(t)
case .pairingKey(let k): authBody = .pairingKey(k)
}
```

(Use `rg "Auth(token:" ios/` to locate all sites.)

- [ ] **Step 5: Run; expect green.**

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --filter AuthCodableTests
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test
```

Both green.

- [ ] **Step 6: Commit.**

```bash
git add -u
git add ios/Tests/ClearCamCoreTests/AuthCodableTests.swift
git commit -m "feat(ios/protocol): Auth becomes enum {token, pairingKey} mirroring Rust untagged form"
```

---

# Section 6a-B — Desktop USB session integration

### Task 2: `session::accept_control_usb` accepts PairingKey AUTH

**Files:**
- Modify: `desktop/crates/session/src/handshake.rs`
- Modify: `desktop/crates/session/src/lib.rs`

- [ ] **Step 1: Write the failing test.**

In `desktop/crates/session/src/handshake.rs`, in the existing `#[cfg(test)] mod tests` block, add:

```rust
#[tokio::test]
async fn accept_control_usb_happy_path() {
    use ccp_protocol::Auth;
    let (mut server, mut client) = handshake_pair().await;
    let caps = vec![ccp_protocol::Capability::Hevc];
    let expected_pairing_key = "stored-key-b64".to_string();

    let task = tokio::spawn(async move {
        accept_control_usb(&mut server, &expected_pairing_key, &caps).await
    });

    drive_hello_authpk_client(&mut client, "stored-key-b64").await;

    let result = task.await.unwrap().expect("handshake should succeed");
    assert_eq!(result.token, "stored-key-b64");
}

#[tokio::test]
async fn accept_control_usb_rejects_wrong_pairing_key() {
    use ccp_protocol::Auth;
    let (mut server, mut client) = handshake_pair().await;
    let caps = vec![ccp_protocol::Capability::Hevc];
    let expected_pairing_key = "stored-key-b64".to_string();

    let task = tokio::spawn(async move {
        accept_control_usb(&mut server, &expected_pairing_key, &caps).await
    });

    drive_hello_authpk_client(&mut client, "wrong-key").await;

    let err = task.await.unwrap().unwrap_err();
    assert!(matches!(err, SessionError::Unauthorized));
}

#[tokio::test]
async fn accept_control_usb_rejects_token_auth() {
    use ccp_protocol::Auth;
    let (mut server, mut client) = handshake_pair().await;
    let caps = vec![ccp_protocol::Capability::Hevc];

    let task = tokio::spawn(async move {
        accept_control_usb(&mut server, "k", &caps).await
    });

    // Client sends HELLO then AUTH with a plain token (legacy Wi-Fi path).
    let hello = ControlEnvelope {
        seq: 1, ack: None,
        body: ControlMessage::Hello(Hello {
            proto_ver: PROTO_VER,
            app: "test".into(),
            device: DeviceIdent { model: "iPhone15,3".into(), os_ver: "iOS 18.0".into() },
            session_id: "candidate".into(),
            caps: vec![],
        }),
    };
    client.send(&hello).await.unwrap();
    let _ack = client.recv().await.unwrap();
    let auth = ControlEnvelope {
        seq: 2, ack: None,
        body: ControlMessage::Auth(Auth::token("plain-token")),
    };
    client.send(&auth).await.unwrap();

    let err = task.await.unwrap().unwrap_err();
    assert!(matches!(err, SessionError::Unauthorized));
}

async fn drive_hello_authpk_client(client: &mut ControlStream, key_b64: &str) {
    use ccp_protocol::{Auth, ControlEnvelope, ControlMessage, DeviceIdent, Hello, PROTO_VER};
    let hello = ControlEnvelope {
        seq: 1, ack: None,
        body: ControlMessage::Hello(Hello {
            proto_ver: PROTO_VER,
            app: "test-usb".into(),
            device: DeviceIdent { model: "iPhone15,3".into(), os_ver: "iOS 18.0".into() },
            session_id: "candidate".into(),
            caps: vec![],
        }),
    };
    client.send(&hello).await.unwrap();
    let _ack = client.recv().await.unwrap();
    let auth = ControlEnvelope {
        seq: 2, ack: None,
        body: ControlMessage::Auth(Auth::pairing_key(key_b64)),
    };
    client.send(&auth).await.unwrap();
    let _ok = client.recv().await.unwrap();
}
```

- [ ] **Step 2: Run; expect failure (function not defined).**

```bash
cd desktop && cargo test -p session accept_control_usb
```

- [ ] **Step 3: Implement `accept_control_usb`.**

In `desktop/crates/session/src/handshake.rs`, add after the existing `accept_control`:

```rust
/// USB-side handshake variant. Mirrors `accept_control` but expects an
/// `Auth::PairingKey` whose value equals `expected_pairing_key_b64`. The
/// returned `AcceptOutcome.token` is set to the pairing key so downstream
/// code (media-stream pairing) sees a single uniform "credential" string.
///
/// Reject paths:
/// - `Auth::Token { .. }` on this path → `SessionError::Unauthorized`
///   (USB sessions must present the pairing key).
/// - `Auth::PairingKey { pairing_key }` where the value mismatches → same.
pub async fn accept_control_usb(
    stream: &mut ControlStream,
    expected_pairing_key_b64: &str,
    server_caps: &[Capability],
) -> Result<AcceptOutcome, SessionError> {
    // HELLO / HELLO_ACK (identical to accept_control)
    let hello_env = stream.recv().await?;
    let hello = match hello_env.body {
        ControlMessage::Hello(h) => h,
        other => {
            send_error(stream, hello_env.seq, ErrorCode::BadRequest, "expected HELLO").await?;
            return Err(SessionError::Protocol(format!("expected HELLO, got {other:?}")));
        }
    };
    if hello.proto_ver != PROTO_VER {
        send_error(stream, hello_env.seq, ErrorCode::IncompatibleVersion,
                   &format!("proto {} vs {}", hello.proto_ver, PROTO_VER)).await?;
        return Err(SessionError::IncompatibleVersion);
    }
    let ack_caps = server_caps.iter().filter(|c| hello.caps.contains(c)).cloned().collect();
    let ack = ControlEnvelope {
        seq: hello_env.seq.wrapping_add(1),
        ack: Some(hello_env.seq),
        body: ControlMessage::HelloAck(HelloAck { proto_ver: PROTO_VER, caps: ack_caps }),
    };
    stream.send(&ack).await?;

    // AUTH (PairingKey only)
    let auth_env = stream.recv().await?;
    let auth = match auth_env.body {
        ControlMessage::Auth(a) => a,
        other => {
            send_error(stream, auth_env.seq, ErrorCode::BadRequest, "expected AUTH").await?;
            return Err(SessionError::Protocol(format!("expected AUTH, got {other:?}")));
        }
    };
    let presented_key = match &auth {
        Auth::PairingKey { pairing_key } => pairing_key.clone(),
        Auth::Token { .. } => {
            send_error(stream, auth_env.seq, ErrorCode::Unauthorized,
                       "USB session requires pairingKey, not token").await?;
            return Err(SessionError::Unauthorized);
        }
    };
    if presented_key != expected_pairing_key_b64 {
        send_error(stream, auth_env.seq, ErrorCode::Unauthorized, "pairing key mismatch").await?;
        return Err(SessionError::Unauthorized);
    }

    // AUTH_OK
    let session_id = hello.session_id.clone();
    let ok = ControlEnvelope {
        seq: auth_env.seq.wrapping_add(1),
        ack: Some(auth_env.seq),
        body: ControlMessage::AuthOk(AuthOk { session_id: session_id.clone() }),
    };
    stream.send(&ok).await?;

    Ok(AcceptOutcome {
        session_id,
        token: presented_key,
        hello,
    })
}
```

> If `AuthOk` / `send_error` / `AcceptOutcome` / `handshake_pair` aren't pre-existing helpers, look for them in `handshake.rs` (most are; see `accept_control`). The `handshake_pair` test helper produces a TCP-loopback `ControlStream` pair — reuse the one already in `rejects_wrong_token`.

In `desktop/crates/session/src/lib.rs`, add to the re-exports:

```rust
pub use handshake::accept_control_usb;
```

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p session accept_control_usb
```

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(session): accept_control_usb — handshake variant requiring Auth::PairingKey"
```

---

### Task 3: `AppHandle::start_usb_supervisor` runs real handshake → Ready

**Files:**
- Modify: `desktop/crates/app/src/lib.rs`
- Create: `desktop/crates/app/tests/usb_full_session_e2e.rs`

- [ ] **Step 1: Test (drives mock-iphone listener through to `SessionStateKind::Ready`).**

`desktop/crates/app/tests/usb_full_session_e2e.rs`:

```rust
//! Full USB session: UsbSupervisor dials a mock-iphone listener, then
//! AppCore consumes DialedStreams, runs accept_control_usb, and reaches
//! SessionStateKind::Ready.

use std::sync::Arc;
use std::time::Duration;

use app::{AppHandle, PairingMaterial, PairingStore};
use session::SessionStateKind;
use transport::usb::{LoopbackConductor, LoopbackDevice};

async fn pick_free_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[tokio::test]
async fn usb_supervisor_drives_session_to_ready() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let cport = pick_free_port().await;
    let mport = pick_free_port().await;

    // Pre-pair the device.
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(
        PairingStore::open_at(tmp.path().join("p.toml")).await.unwrap(),
    );
    let key = PairingMaterial::new_random();
    let key_b64 = key.as_b64();
    store.put("TEST-USB-UDID", key).await.unwrap();

    // Mock-iphone listens in USB mode and will use `key_b64` as its AUTH token
    // (mock-iphone currently sends Auth::Token { token: <token-arg> }; for
    // a USB-handshake test we need it to send PairingKey instead; see Task 4
    // for the mock update. This test therefore implicitly depends on Task 4.).
    let mock_args = mock_iphone::Args {
        host: "127.0.0.1".into(),
        cport,
        mport,
        token: key_b64.clone(),                        // re-used as pairing-key value
        width: 1280, height: 720, fps: 30, send_video: false,
        transport: mock_iphone::TransportMode::UsbListener,
    };
    let mock_task = tokio::spawn(mock_iphone::run(mock_args));
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Conductor + AppCore.
    let lb = Arc::new(LoopbackConductor::new());
    lb.register(LoopbackDevice {
        id: 1,
        udid: "TEST-USB-UDID".into(),
        control_addr: format!("127.0.0.1:{cport}").parse().unwrap(),
        media_addr:   format!("127.0.0.1:{mport}").parse().unwrap(),
    })
    .await;

    let app = AppHandle::for_tests_with_pairing(store.clone()).await;
    app.start_usb_supervisor(lb, store, Duration::from_millis(50)).await;

    // Wait for state to reach Ready.
    let mut snap = app.snapshot.clone();
    for _ in 0..200 {
        if matches!(snap.borrow().state, SessionStateKind::Ready) { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        matches!(snap.borrow().state, SessionStateKind::Ready),
        "expected SessionStateKind::Ready, got {:?}",
        snap.borrow().state
    );

    mock_task.abort();
}
```

- [ ] **Step 2: Run; expect failure (`AppHandle::for_tests_with_pairing` doesn't exist; consumer still just logs).**

```bash
cargo test -p app --test usb_full_session_e2e
```

- [ ] **Step 3: Replace the logging consumer in `AppHandle::start_usb_supervisor` with a real handshake.**

In `desktop/crates/app/src/lib.rs`, find the existing `pub async fn start_usb_supervisor<C: ...>(...)` (Task 12 of Phase 5 added it). Replace the `tracing::info!` consumer body with:

```rust
let consumer = tokio::spawn({
    let snapshot_tx = self.snapshot_tx.clone();
    let events_tx   = self.events_tx.clone();
    let sinks       = self.sinks.clone();
    let outbound    = self.outbound.clone();
    let pairing     = store.clone();
    async move {
        while let Some(mut dialed) = rx.recv().await {
            tracing::info!(udid = %dialed.udid, "usb session: starting handshake");
            snapshot_tx.send_modify(|s| {
                s.state = SessionStateKind::usb_handshake(dialed.udid.clone());
            });

            // Resolve the pairing key. If we have nothing for this UDID, the
            // session can't proceed — emit the trust-request event so the
            // UI can prompt the user, then drop the streams.
            let key_b64 = match pairing.get(&dialed.udid).await {
                Ok(Some(k)) => k.as_b64(),
                Ok(None) => {
                    let _ = events_tx
                        .send(ControlPlaneEvent::UsbTrustRequest {
                            udid: dialed.udid.clone(),
                        })
                        .await;
                    snapshot_tx.send_modify(|s| {
                        s.state = SessionStateKind::Idle;
                    });
                    continue;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "pairing store read failed");
                    continue;
                }
            };

            let caps = vec![
                ccp_protocol::Capability::Hevc,
                ccp_protocol::Capability::H264,
                ccp_protocol::Capability::RawNv12,
            ];
            match session::accept_control_usb(&mut dialed.control, &key_b64, &caps).await {
                Ok(acc) => {
                    let cp = ControlPlane::new(events_tx.clone(), snapshot_tx.clone());
                    let (out_tx, out_rx) = mpsc::channel::<ControlMessage>(16);
                    *outbound.write().await = Some(out_tx);
                    let outbound_slot = outbound.clone();

                    // Media stream is already opened by the supervisor — start
                    // MediaPipeline directly (no MEDIA_HELLO indirection).
                    let snapshot_sinks: Vec<_> = sinks.read().await.iter().cloned().collect();
                    let decoder: Arc<dyn Decoder> = Arc::new(PassthroughDecoder);
                    let _pipeline = MediaPipeline::spawn(dialed.media, snapshot_sinks, decoder);

                    tokio::spawn(async move {
                        if let Err(e) = cp.run_with_outbound(acc, dialed.control, out_rx).await {
                            tracing::warn!(error = ?e, "usb control plane exited");
                        }
                        *outbound_slot.write().await = None;
                    });
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "usb handshake failed");
                    snapshot_tx.send_modify(|s| { s.state = SessionStateKind::Idle; });
                }
            }
        }
        sup_handle.abort();
    }
});
```

For this to compile we need:
1. `ControlPlaneEvent::UsbTrustRequest { udid: String }` — add this variant in `session/src/lib.rs` (or wherever the enum lives).
2. `AppHandle` exposes its internal `snapshot_tx` and `events_tx` to itself — they should already be in scope from `AppCore::start`. If they're not on `&self`, store them on it.
3. `AppHandle::for_tests_with_pairing(store)` — new test constructor that boots an empty AppCore (no Wi-Fi server) wired with the supplied PairingStore. Implement:

```rust
impl AppHandle {
    /// Test-only constructor: builds an AppHandle with no Wi-Fi listeners,
    /// suitable for driving USB-only sessions through the supervisor.
    pub async fn for_tests_with_pairing(_store: Arc<PairingStore>) -> Self {
        let (events_tx, events_rx) = mpsc::channel(32);
        let (snapshot_tx, snapshot_rx) = watch::channel(SessionSnapshot::idle());
        let sinks = Arc::new(RwLock::new(Vec::new()));
        let outbound = Arc::new(RwLock::new(None));
        Self {
            qr: QrPayload {
                v: 1, host: "127.0.0.1".into(),
                cport: 0, mport: 0, token: "test".into(),
            },
            snapshot: snapshot_rx,
            events: Arc::new(Mutex::new(events_rx)),
            sinks,
            outbound,
            accept_task: tokio::spawn(async {}),  // no-op
            usb_supervisor_task: tokio::sync::Mutex::new(None),
            snapshot_tx,
            events_tx,
        }
    }
}
```

Add `snapshot_tx: watch::Sender<SessionSnapshot>` and `events_tx: mpsc::Sender<ControlPlaneEvent>` fields on `AppHandle` (if not already there). In `AppCore::start`, when constructing the existing `AppHandle`, also stash the senders.

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p app --test usb_full_session_e2e
```

Will still fail until Task 4 updates mock-iphone to send `Auth::PairingKey` when the token looks like a pairing key. **OK to leave failing through end of Task 4; gate the test with `#[ignore = "pending Task 4 mock update"]` while iterating if it blocks other work.**

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(app): start_usb_supervisor runs real handshake → Ready (PairingKey path)"
```

---

### Task 4: mock-iphone sends `Auth::PairingKey` in USB mode

**Files:**
- Modify: `desktop/tools/mock-iphone/src/lib.rs`

- [ ] **Step 1: Modify `run_session` (or wherever AUTH is sent).**

Find the line that builds the AUTH ControlEnvelope (currently `Auth::token(args.token.clone())`). Change to:

```rust
let auth_body = match args.transport {
    TransportMode::WifiClient  => ControlMessage::Auth(Auth::token(args.token.clone())),
    TransportMode::UsbListener => ControlMessage::Auth(Auth::pairing_key(args.token.clone())),
};
```

The `--token` CLI arg keeps its name (overloaded — represents either token or pairing key value).

- [ ] **Step 2: Run the e2e test.**

```bash
cargo test -p app --test usb_full_session_e2e
```

Expected: green. If still red, double-check the supervisor consumer is wired and not the stub.

- [ ] **Step 3: Commit.**

```bash
git add -u
git commit -m "feat(mock-iphone): send Auth::PairingKey when --transport usb (matches accept_control_usb)"
```

---

# Section 6a-C — Adaptive loop wired live

### Task 5: `AdaptationDriver` glues telemetry → SET_MODE

**Files:**
- Create: `desktop/crates/app/src/adaptation_driver.rs`
- Modify: `desktop/crates/app/src/lib.rs`
- Create: `desktop/crates/app/tests/adaptive_loop_e2e.rs`

- [ ] **Step 1: Write the failing test.**

`desktop/crates/app/tests/adaptive_loop_e2e.rs`:

```rust
//! Adaptive driver: feed scripted telemetry, assert SET_MODE on the outbound.

use std::time::Duration;
use adaptive::{AdaptationState, AvailableMode, Measurement, TransportClass, UserLimits};
use app::AdaptationDriver;
use ccp_protocol::{Capability, ControlMessage, Mode, Telemetry};
use tokio::sync::mpsc;

fn telemetry(queue_depth: u32, drop_count: u32, rtt_ms_proxy: u32) -> Telemetry {
    Telemetry {
        ts_usec: 0,
        battery_level: 0.9,
        battery_state: ccp_protocol::BatteryState::Unplugged,
        thermal_state: ccp_protocol::ThermalState::Nominal,
        sent_bitrate_kbps: 0,
        enc_fps: 30,
        capture_fps: 30,
        queue_depth,
        drop_count,
    }
}

#[tokio::test]
async fn sustained_congestion_triggers_step_down_on_wire() {
    let (out_tx, mut out_rx) = mpsc::channel::<ControlMessage>(8);
    let mut driver = AdaptationDriver::new_for_tests(out_tx);

    // Burst: queue depth keeps growing, drops appear.
    for i in 0..6 {
        driver.observe(telemetry(2 + i, 5 * i, 0)).await;
    }

    let msg = tokio::time::timeout(Duration::from_millis(100), out_rx.recv())
        .await
        .expect("timeout")
        .expect("SET_MODE expected");
    assert!(matches!(msg, ControlMessage::SetMode(_)), "got {msg:?}");
}

#[tokio::test]
async fn stable_telemetry_emits_nothing() {
    let (out_tx, mut out_rx) = mpsc::channel::<ControlMessage>(8);
    let mut driver = AdaptationDriver::new_for_tests(out_tx);

    for _ in 0..10 {
        driver.observe(telemetry(0, 0, 0)).await;
    }
    assert!(tokio::time::timeout(Duration::from_millis(50), out_rx.recv()).await.is_err());
}
```

- [ ] **Step 2: Run; expect failure.**

```bash
cargo test -p app --test adaptive_loop_e2e
```

- [ ] **Step 3: Implement `adaptation_driver.rs`.**

```rust
//! Glue between live `ControlPlaneEvent::Telemetry` and the pure-Rust
//! `adaptive::AdaptationState` machine. Owns one `AdaptationState` instance,
//! feeds telemetry samples, and emits `SET_MODE` on the outbound channel
//! whenever the state decides to step.

use adaptive::{AdaptationState, AvailableMode, Measurement, TransportClass, UserLimits, Action};
use ccp_protocol::{Capability, ControlMessage, Mode, SetMode, Telemetry};
use tokio::sync::mpsc;
use tracing::info;

pub struct AdaptationDriver {
    state: AdaptationState,
    outbound: mpsc::Sender<ControlMessage>,
}

impl AdaptationDriver {
    pub fn new(outbound: mpsc::Sender<ControlMessage>, initial_mode: Mode) -> Self {
        Self {
            state: AdaptationState::new(initial_mode),
            outbound,
        }
    }

    /// Test-only constructor: starts in a sensible default mode (encoded 1080p30).
    #[cfg(any(test, feature = "test-util"))]
    pub fn new_for_tests(outbound: mpsc::Sender<ControlMessage>) -> Self {
        Self::new(outbound, default_test_mode())
    }

    pub async fn observe(&mut self, t: Telemetry) {
        match self.state.observe(&t) {
            Action::None => {}
            Action::StepDown(new_mode) | Action::StepUp(new_mode) => {
                info!(?new_mode, "adaptation: emitting SET_MODE");
                let _ = self.outbound
                    .send(ControlMessage::SetMode(SetMode { mode: new_mode }))
                    .await;
            }
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
fn default_test_mode() -> Mode {
    Mode {
        format: "encoded".into(),
        codec: "hevc".into(),
        width: 1920, height: 1080, fps: 30,
        bitrate_kbps: 30_000,
        pixel_format: "nv12".into(),
        full_range: true,
    }
}
```

> **Read `desktop/crates/adaptive/src/adaptation.rs` first** — verify that the existing `AdaptationState` has constructor `new(initial_mode)` and method `observe(&Telemetry) -> Action`. If they differ, adapt the driver. The spec assumes the Phase 4 shape ([adaptive/src/adaptation.rs](../desktop/crates/adaptive/src/adaptation.rs) verified previously). If `Action` is missing variants, the test imports will tell you.

Wire into `lib.rs`:

```rust
pub mod adaptation_driver;
pub use adaptation_driver::AdaptationDriver;
```

- [ ] **Step 4: Run; expect green.**

```bash
cargo test -p app --test adaptive_loop_e2e
```

- [ ] **Step 5: Commit.**

```bash
git add desktop/crates/app/src/adaptation_driver.rs desktop/crates/app/src/lib.rs desktop/crates/app/tests/adaptive_loop_e2e.rs
git commit -m "feat(app): AdaptationDriver — wires AdaptationState to live telemetry → SET_MODE"
```

---

### Task 6: Wire `AdaptationDriver` into AppCore + iOS responds with `MODE_APPLIED`

**Files:**
- Modify: `desktop/crates/app/src/lib.rs` (consume `ControlPlaneEvent::Telemetry` → `AdaptationDriver::observe`)
- Modify: `desktop/tools/mock-iphone/src/lib.rs` (respond to `SET_MODE` with `MODE_APPLIED`)
- Create: `ios/Tests/ClearCamCoreTests/SetModeRoundTripTests.swift`

- [ ] **Step 1: AppCore — Telemetry → AdaptationDriver.**

In `AppCore::start` (after the events channel is created), spawn a small task:

```rust
let driver_outbound = outbound.clone();   // Arc<RwLock<Option<OutboundSender>>>
let mut events_rx_for_driver = …;          // need a clone of events_rx OR a separate broadcast

// Easier: subscribe via watch — but ControlPlaneEvent is an mpsc, so fan-out
// requires either splitting into broadcast or pulling from the same channel.
// Simplest pragmatic: add a separate adaptation-events mpsc that
// ControlPlane sends to.
```

> **Implementation note:** the existing `ControlPlane` already routes
> `ControlMessage::Telemetry` into `ControlPlaneEvent::Telemetry`. The
> simplest non-invasive wire is **inside `spawn_accept_loop`'s control-plane
> task**: when it observes the inbound telemetry directly (one layer up),
> also `driver.observe(...).await`. Add a `driver: Arc<Mutex<AdaptationDriver>>`
> handle into the spawned task. Read the existing flow before deciding which
> seam is least invasive.

Concrete steps:
- Add `driver_slot: Arc<Mutex<Option<AdaptationDriver>>>` to `AppHandle` so it can be set when an `Authed` session begins.
- In the spawned per-session task (both Wi-Fi and USB paths), once `OutboundSender` is bound, build `AdaptationDriver::new(out_tx.clone(), default_streaming_mode())` and stash in `driver_slot`.
- Inside `ControlPlane::run_with_outbound`, when handling a `Telemetry` message, call `app::adaptation_driver_observe_static(...)` — or expose a callback. Pragmatic alternative: ControlPlane writes telemetry to `events_tx`; spawn a separate consumer that owns the driver:

```rust
let driver_consumer = tokio::spawn({
    let driver_slot = driver_slot.clone();
    let mut local_events_rx = events_tx.subscribe(); // requires broadcast
    async move {
        while let Some(ev) = local_events_rx.recv().await {
            if let ControlPlaneEvent::Telemetry(t) = ev {
                if let Some(driver) = driver_slot.lock().await.as_mut() {
                    driver.observe(t).await;
                }
            }
        }
    }
});
```

If `events_tx` is `mpsc::Sender` (not broadcast), introduce a one-line conversion: change `ControlPlaneEvent` to flow through a `broadcast::Sender` instead, OR keep `mpsc` and have the per-session control-plane task own the driver directly (no fan-out needed).

**Pick whichever is shorter to implement; the test (Task 5) already covers the driver behavior in isolation.** For Task 6, also add a brief unit test in the file that confirms `driver_slot` is populated after a USB session reaches Authed.

- [ ] **Step 2: mock-iphone — respond to SET_MODE.**

In `run_session` (mock-iphone), the existing message-handling loop watches for `Ping` and `Bye`. Add a `SetMode` arm:

```rust
ControlMessage::SetMode(sm) => {
    let new_seq = next_seq();
    let env = ControlEnvelope {
        seq: new_seq,
        ack: Some(env.seq),
        body: ControlMessage::ModeApplied(ModeApplied {
            mode: sm.mode.clone(),
            at_seq: 0,
        }),
    };
    control.send(&env).await?;
}
```

> Check that `ModeApplied` already exists in `ccp-protocol::streaming` (Phase 4 likely added it). If absent, define it inline.

- [ ] **Step 3: iOS test — round-trip stub.**

`ios/Tests/ClearCamCoreTests/SetModeRoundTripTests.swift`:

```swift
import XCTest
@testable import ClearCamProtocol

final class SetModeRoundTripTests: XCTestCase {
    func testSetModeCodableRoundTrip() throws {
        // SET_MODE body shape is the same as MODE_APPLIED's `mode` field —
        // pin the Codable mapping so both sides agree on JSON shape.
        let mode = Mode(
            format: "encoded", codec: "hevc",
            width: 1920, height: 1080, fps: 30,
            bitrateKbps: 30_000, pixelFormat: "nv12", fullRange: true
        )
        let data = try JSONEncoder().encode(mode)
        let back = try JSONDecoder().decode(Mode.self, from: data)
        XCTAssertEqual(back, mode)
    }
}
```

If `Mode` Codable struct doesn't yet exist on iOS, add it in
`ios/Sources/ClearCamProtocol/Streaming.swift` (or whichever existing protocol
file holds streaming messages), mirroring the Rust shape (`Mode { format, codec,
width, height, fps, bitrateKbps, pixelFormat, fullRange }`).

- [ ] **Step 4: Run all gates.**

```bash
cargo test --workspace
cd ios && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test
```

All green.

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(app+mock+ios): AdaptationDriver hooked into live telemetry; SET_MODE↔MODE_APPLIED roundtrip"
```

---

# Section 6a-D — UI/Tauri live commands

### Task 7: Tauri state holds Arc<PairingStore> + Arc<dyn UsbConductor>

**Files:**
- Modify: `desktop/src-tauri/src/lib.rs` (or `main.rs` — wherever `tauri::Builder::default()...build()` is)
- Modify: `desktop/src-tauri/src/commands.rs`

- [ ] **Step 1: Test (manual smoke; no unit test framework for tauri commands).**

We rely on:
- `cargo build -p clearcam-desktop` succeeds.
- `cargo clippy --workspace --all-targets -- -D warnings` succeeds.
- The three commands compile against `tauri::State<...>`.

- [ ] **Step 2: Replace stub commands with live ones.**

Update `desktop/src-tauri/src/commands.rs`:

```rust
use std::sync::Arc;

use app::PairingStore;
use transport::usb::UsbConductor;

#[derive(serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbDeviceDto {
    pub id: u32,
    pub udid: String,
    pub product_id: Option<u16>,
    pub trusted: bool,
}

#[derive(Clone)]
pub struct UsbState {
    pub conductor: Arc<dyn UsbConductor>,
    pub pairing:   Arc<PairingStore>,
}

#[tauri::command]
pub async fn list_usb_devices(
    state: tauri::State<'_, UsbState>,
) -> Result<Vec<UsbDeviceDto>, String> {
    let devices = state.conductor.list_devices().await.map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(devices.len());
    for d in devices {
        let trusted = state.pairing.get(&d.udid).await
            .map_err(|e| e.to_string())?
            .is_some();
        out.push(UsbDeviceDto {
            id: d.id,
            udid: d.udid,
            product_id: d.product_id,
            trusted,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn trust_usb_device(
    udid: String,
    state: tauri::State<'_, UsbState>,
) -> Result<(), String> {
    let key = app::PairingMaterial::new_random();
    state.pairing.put(&udid, key).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn forget_usb_device(
    udid: String,
    state: tauri::State<'_, UsbState>,
) -> Result<(), String> {
    state.pairing.forget(&udid).await.map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: Wire state in `lib.rs`.**

In `desktop/src-tauri/src/lib.rs`, inside the builder setup, register the state:

```rust
let pairing = std::sync::Arc::new(
    app::PairingStore::open_default().await.expect("pairing store open"),
);

#[cfg(feature = "usb-idevice")]
let conductor: std::sync::Arc<dyn transport::usb::UsbConductor> =
    std::sync::Arc::new(transport::usb::IdeviceConductor::new().expect("idevice"));
#[cfg(not(feature = "usb-idevice"))]
let conductor: std::sync::Arc<dyn transport::usb::UsbConductor> =
    std::sync::Arc::new(transport::usb::LoopbackConductor::new());

let usb_state = commands::UsbState { conductor: conductor.clone(), pairing: pairing.clone() };

// Then on the AppHandle side, spawn the supervisor:
app_handle.start_usb_supervisor(conductor, pairing, std::time::Duration::from_millis(500)).await;

tauri::Builder::default()
    .manage(usb_state)
    .invoke_handler(tauri::generate_handler![
        commands::list_usb_devices,
        commands::trust_usb_device,
        commands::forget_usb_device,
        /* existing handlers… */
    ])
    /* … */
```

> Adapt to the actual existing builder setup; the example above is illustrative.

- [ ] **Step 4: Build + smoke.**

```bash
cargo build -p clearcam-desktop
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All green.

- [ ] **Step 5: Commit.**

```bash
git add -u
git commit -m "feat(tauri): list_usb_devices / trust_usb_device / forget_usb_device hit live PairingStore + UsbConductor"
```

---

### Task 8: Emit `session://transport_changed` and `session://usb_trust_request`

**Files:**
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src-tauri/src/events.rs` (drop `#[allow(dead_code)]` once consumed)

- [ ] **Step 1: Bridge `ControlPlaneEvent` to Tauri emit.**

In the existing Tauri setup (likely in `setup` closure), after building `AppHandle`, spawn a task that drains `AppHandle::events` and forwards select events:

```rust
let app_handle_for_events = app.clone();
let tauri_handle = handle.clone();
tokio::spawn(async move {
    let mut rx = app_handle_for_events.events.lock().await;
    while let Some(ev) = rx.recv().await {
        match ev {
            session::ControlPlaneEvent::UsbTrustRequest { udid } => {
                let _ = tauri_handle.emit(events::USB_TRUST_REQUEST,
                    serde_json::json!({ "udid": udid }));
            }
            // … other emit mappings
            _ => {}
        }
    }
});
```

For `session://transport_changed`, watch `app.snapshot` (which now carries `SessionStateKind::UsbHandshake { udid }` vs `WifiHandshake`):

```rust
let mut snap_rx = app.snapshot.clone();
tokio::spawn(async move {
    let mut last_source: Option<String> = None;
    loop {
        let curr = snap_rx.borrow().clone();
        let curr_source = match &curr.state {
            session::SessionStateKind::UsbHandshake { .. } |
            session::SessionStateKind::Ready { /* if you've enriched Ready */ } => Some("usb".to_string()),
            session::SessionStateKind::WifiHandshake => Some("wifi".to_string()),
            _ => None,
        };
        if curr_source != last_source && curr_source.is_some() {
            let _ = tauri_handle.emit(events::TRANSPORT_CHANGED,
                serde_json::json!(curr_source.clone().unwrap()));
            last_source = curr_source;
        }
        if snap_rx.changed().await.is_err() { break; }
    }
});
```

> The exact mapping depends on how `SessionStateKind::Ready` is currently
> tagged. Phase 5 left `Ready` without a transport tag — if needed, enrich
> it to `Ready { transport: TransportTag }` first (small change in
> `session/state.rs`). Pick whichever shape is least invasive.

- [ ] **Step 2: Remove `#[allow(dead_code)]` from the event constants** in `events.rs` (they're now consumed).

- [ ] **Step 3: Build + workspace gates.**

```bash
cargo build -p clearcam-desktop
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 4: Commit.**

```bash
git add -u
git commit -m "feat(tauri): emit session://transport_changed + session://usb_trust_request"
```

---

# Section 6a-E — Tag

### Task 9: Final gates + acceptance log + tag

- [ ] **Step 1: Run all gates.**

```bash
cd desktop
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p transport --features usb-idevice
cd ../desktop && pnpm -C ui typecheck && pnpm -C ui build
cd ../ios && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test
```

All green.

- [ ] **Step 2: Append Acceptance log.**

Add to the end of `plans/phase-6a-usb-session-and-adaptive.md`:

```markdown
## Acceptance log

### YYYY-MM-DD — Phase 6a USB session + adaptive

- iOS `Auth` enum with `.token` / `.pairingKey`; SessionController honours both via `AuthCredential`.
- `session::accept_control_usb` rejects token auth; pairs `Auth::PairingKey` against `PairingStore`.
- `AppHandle::start_usb_supervisor` runs the full USB handshake → `MediaPipeline` → `Ready`.
- `app::AdaptationDriver` consumes live `Telemetry`, emits `SET_MODE` on congestion.
- mock-iphone responds to `SET_MODE` with `MODE_APPLIED` round-trip.
- Tauri `list_usb_devices` / `trust_usb_device` / `forget_usb_device` are live against `PairingStore` + `UsbConductor`.
- Events `session://transport_changed` and `session://usb_trust_request` emit from AppCore.

Gates: cargo fmt/clippy/test ✓ ; transport+usb-idevice ✓ ; pnpm ui ✓ ; swift test ✓.
```

- [ ] **Step 3: Tag.**

```bash
git add plans/phase-6a-usb-session-and-adaptive.md
git commit -m "docs(plans): Phase 6a acceptance log"
git tag v0.6.0a-phase6a
```

- [ ] **Step 4: Merge to main.**

Per project convention (Phase 5 was merged locally to main):

```bash
git checkout main
git merge --ff-only feature/phase-6a-usb-session-and-adaptive
git branch -d feature/phase-6a-usb-session-and-adaptive
```

---

## Acceptance log

### 2026-05-30 — Phase 6a USB session + adaptive

**iOS (ClearCamProtocol + ClearCamCore)**

- `Auth` теперь enum `.token(String) | .pairingKey(String)` с ручным `Codable`, который пишет `{"token":"..."}` или `{"pairingKey":"..."}` — точное зеркало desktop'ного untagged-enum (Phase 5 Task 7).
- `SessionController.connect(credential:)` теперь маршрутизирует на правильный `Auth` вариант (раньше pairingKey шёл в `Auth(token:)` — был latent-баг).
- Существующие call-site `Auth(token:)` в тестах мигрированы на `Auth.token(...)`.
- 4 новых теста `AuthCodableTests` + 3 теста `ModeCodableTests`.

**Rust session crate**

- `accept_control_usb(stream, expected_pairing_key_b64, server_caps)` — handshake-вариант, требующий `Auth::PairingKey`. Token-AUTH отвергается с `ErrorCode::Unauthorized`. 3 теста.
- `ControlPlaneEvent::UsbTrustRequest { udid }` — добавлен; все exhaustive-match сайты обновлены.

**Rust app crate**

- `AppHandle` обзавёлся полями `snapshot_tx` и `events_tx` (Phase 5 их прятал внутри accept-loop'а).
- `start_usb_supervisor` теперь делает реальный handshake: `accept_control_usb` → `MediaPipeline::spawn` → `ControlPlane::run_with_outbound` → `SessionStateKind::Ready`. На пустом PairingStore — эмиттит `UsbTrustRequest`.
- `AdaptationDriver::observe(&Telemetry, rtt_ms)` — тонкая обёртка над `adaptive::AdaptationState`, шлёт `SET_MODE` на outbound при congestion. `new_for_tests` бэк-датит `start_at` на 10s в прошлое, чтобы пропустить step-down cooldown в тестах. 2 теста.
- `for_tests_with_pairing(store)` — тест-конструктор без Wi-Fi listener'а.
- `feature = "test-util"` + self-referential dev-dep для интеграционных тестов.
- e2e тест `usb_full_session_e2e` теперь активен (не `#[ignore]`) и реально доводит сессию до `Ready`.

**Tools (mock-iphone)**

- В USB-listener режиме шлёт `Auth::PairingKey(args.token)` вместо `Auth::Token` — `--token` стал полиморфным (token для Wi-Fi, pairing-key b64 для USB).
- В per-session loop добавлен `SetMode` arm: возвращает `MODE_APPLIED(mode, at_seq=0)` с правильным `ack`.

**Tauri**

- `UsbState { conductor: Arc<dyn UsbConductor>, pairing: Arc<PairingStore> }` — общее состояние.
- `list_usb_devices` теперь делает реальный `conductor.list_devices()` + lookup в PairingStore (поле `trusted`).
- `trust_usb_device(udid)` — `pairing.put(udid, PairingMaterial::new_random())`.
- `forget_usb_device(udid)` — `pairing.forget(udid)`.
- `lib.rs::run` строит `UsbState` через `tauri::async_runtime::block_on` (LoopbackConductor по умолчанию; IdeviceConductor под `usb-idevice` feature).
- `src-tauri/Cargo.toml`: добавлен прямой dep на `transport` + feature-passthrough `usb-idevice = ["transport/usb-idevice"]`.
- `events::pump` теперь эмиттит `session://transport_changed` (по handshake-варианту) и `session://usb_trust_request` (получено от ControlPlane). `#[allow(dead_code)]` снято с обеих констант.

### Gates

- `cargo fmt --check` ✓
- `cargo clippy --workspace --all-targets -- -D warnings` ✓
- `cargo test --workspace` ✓ (113 passed, 0 failed)
- `cargo build -p transport --features usb-idevice` ✓
- `pnpm -C desktop/ui typecheck` ✓
- `pnpm -C desktop/ui build` ✓
- `DEVELOPER_DIR=Xcode swift test` ✓ (58 passed, 0 failed)

### Deferred to Phase 6b

1. **`AdaptationDriver` wired into production `ControlPlane`.** Сам driver полностью unit-tested (Task 5), но чтобы вставить его в живой поток `Telemetry`, нужно фан-аут из `ControlPlane::run_with_outbound`: либо отдельный telemetry-канал в `ControlPlane::new`, либо broadcast вместо mpsc для events. Не сделано чтобы не трогать ControlPlane API сейчас.
2. **Real VideoToolbox `VTEncoder` / `VTDecoder`.** План Phase 4 это пометил «out of scope here»; реальный энкодер/декодер — Phase 6b. Сейчас на проводе только raw NV12.
3. **`SessionStateKind::Ready` без transport-тэга.** `events::pump` инфорсит «последний handshake = текущий транспорт», что работает в обычном сценарии, но если между Wi-Fi и USB-сессиями есть intermediate `Idle` — `last_transport` останется висеть на старом. Phase 6b: добавить `Ready { transport: TransportTag }` (Phase 5 Task 11 уже завёл TransportTag).
4. **Mid-stream Wi-Fi↔USB switchover без разрыва.** Сейчас новый dial просто переписывает `outbound`-slot; правильное завершение старой сессии и плавный переход — Phase 6c.
5. **idevice native event-stream override `UsbConductor::subscribe`** (вместо polling-fallback) — Phase 6b/c.
6. **Live ramp speedtest** через media socket — Phase 6b (Phase 4 hangover).
7. **`SET_MODE`/`MODE_APPLIED` round-trip-тест по проводу** (mock-iphone уже отвечает; добавить интеграционный тест, проверяющий driver→wire→mock→ack) — Phase 6b.

## Retrospective

### Что зашло

- Целая USB-сессия теперь идёт end-to-end без живого iPhone: `cargo test -p app --test usb_full_session_e2e` проходит за 0.26 сек.
- iOS-сторона `Auth` теперь шлёт правильный формат на проводе — pairing-key больше не подменяется на «token»-поле.
- `AdaptationDriver` готов к подключению (unit-tests pinning поведение).
- Tauri/UI команды стали реальными — `trust_usb_device` правда пишет ключ в `~/Library/Application Support/ClearCam/pairings.toml`.

### Отклонения от плана

1. **`AdaptationState::observe` берёт `(telemetry, rtt_ms, now)`**, не только `&Telemetry`. План был упрощённым — driver адаптирован к реальной сигнатуре. RTT пока всегда `0.0`; реальный PING/PONG-derived RTT — Phase 6b.
2. **`session_id` в `accept_control_usb` генерируется как UUID** (как в `accept_control`), а не берётся из `hello.session_id`. Это защищает от client-injected session-id и матчит существующий паттерн.
3. **`Mode` уже существовал на iOS** как Codable struct (`PixelFormat` enum, не строка) — тест использует `.nv12` enum case вместо буквальной строки.
4. **`AdaptationDriver` production-wiring deferred** в Phase 6b — добавлять fan-out в `ControlPlane` сейчас означало бы трогать публичный API ради одной интеграции. Лучше сделать это вместе с другими ControlPlane-улучшениями (live RTT, mid-stream-switch).
5. **`AppHandle::for_tests_with_pairing` — `#[cfg(any(test, feature = "test-util"))]`** + self-referential dev-dep. Это стандартный Cargo-паттерн но Cargo пересобирает `app` дважды. Стоит — даёт нам реальный e2e без production-only бэкдоров.

### Открытые вопросы для проверки на железе

- `accept_control_usb` правильно отрабатывает на iOS-Swift `Auth.pairingKey(...)` codable. Должен (тесты Mode и Auth Codable обе стороны проверяют) — но подтвердить на устройстве.
- `IdeviceConductor::connect_to_device` против iOS NWListener на 7000/7001 — Phase 5 deferred, ещё не проверено.
- Tauri-команда `list_usb_devices` с реальным `IdeviceConductor` должна возвращать настоящий список из usbmuxd; CI этого не покрывает.
- `pairings.toml` access patterns на macOS — sandbox? Locations? Должен лежать в `~/Library/Application Support/ClearCam/`.
