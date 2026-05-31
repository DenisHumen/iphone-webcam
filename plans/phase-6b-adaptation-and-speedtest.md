# Phase 6b — AdaptationDriver in production + live speedtest + Ready transport tag

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Steps use checkbox (`- [ ]`).

**Goal:** Promote Phase 6a's headless adaptation pieces to production paths and close two remaining Phase 4/5 hangovers that can be exercised without a real iPhone: live ramp speedtest over the media socket, and the `Ready { transport }` enrichment that removes the brittle "last-handshake-wins" heuristic from `events::pump`.

**Architecture:**
- `ControlPlane::run_with_outbound` gains an optional `telemetry_tx: mpsc::Sender<Telemetry>` parameter; when set, each `Telemetry` inbound is forked into both the existing `ControlPlaneEvent::Telemetry(...)` (for UI) and the dedicated channel (for `AdaptationDriver`). Phase 6a path becomes the production path.
- `SessionStateKind::Ready { transport: TransportTag }` — enrichment is backward-compatible at JSON via `#[serde(rename = "ready")]` keep + new field. `events::pump` now consults the actual variant, eliminating the prior heuristic.
- mock-iphone gains a `SPEEDTEST_TICK` arm — when it receives `SPEEDTEST_START(id, pattern, durationMs)`, it pushes synthetic NV12 frames at the requested per-step bitrates over the media socket, marked with `seq` in the reserved `0xFFFF_0000+` range. The desktop's `mediapipeline` recognises that range and routes those bytes to a `SpeedTestCounter` instead of fanning out to sinks.
- `IdeviceConductor::subscribe` overrides the default polling with idevice's native `UsbmuxdConnection::listen_for_devices` (verified during the implementation — if the API name differs, adapt).
- New e2e test exercises `AdaptationDriver` over the wire: scripted Telemetry payloads → desktop driver decides → `SET_MODE` lands on mock-iphone → `MODE_APPLIED` returned.

**Tech Stack:** Rust 1.95, tokio. No new crate deps. No new system deps.

**Acceptance:**
- `cargo test --workspace --all-targets` green; tests cover ControlPlane fan-out, SessionSnapshot Ready{transport} JSON, speedtest counter, idevice subscribe (mocked), and the SET_MODE wire round-trip.
- `swift test` green (no iOS code changes expected).
- `pnpm -C desktop/ui typecheck && pnpm -C desktop/ui build` green.
- CI green; tag `v0.6.0b-phase6b` after sign-off.

**Out of scope:**
- Real VideoToolbox `VTEncoder` (iOS) / `VTDecoder` (macOS) — Phase 6c, hardware-dependent.
- Mid-stream Wi-Fi↔USB switchover — Phase 6c.
- `ffmpeg-next` decoder — Phase 6c.
- CMIO Camera Extension — blocked on Apple Dev ID (Phase 3, see `docs/14-apple-developer-id-guide.md`).

---

## File Structure

```
desktop/
├── crates/
│   ├── session/src/
│   │   ├── state.rs                                # MOD: SessionStateKind::Ready → Ready { transport: Option<TransportTag> }
│   │   └── controlplane.rs                         # MOD: optional telemetry_tx in run_with_outbound; emit Ready{transport} from the right places
│   ├── adaptive/src/
│   │   └── speedtest.rs                            # MOD: SpeedTestRunner observe_arrival; lives next to existing evaluate()
│   ├── mediapipeline/src/
│   │   └── pipeline.rs                             # MOD: route seqs in 0xFFFF_0000+ range to a SpeedTestCounter sink, not the fanout
│   └── app/
│       ├── src/lib.rs                              # MOD: per-session AdaptationDriver spawned from the new telemetry_tx fork; pass TransportTag to ControlPlane
│       └── tests/
│           └── set_mode_wire_e2e.rs                # NEW: scripted Telemetry → driver → wire SET_MODE → mock MODE_APPLIED
└── tools/mock-iphone/src/lib.rs                    # MOD: SPEEDTEST_START arm — ramp generator on media socket
```

iOS: no changes expected.

## Conventions

- TDD per task. Russian prose / English code (ADR-020).
- Each task ends with one `git commit`.

---

# Section 6b-A — `SessionStateKind::Ready { transport }`

### Task 1: Enrich `Ready` variant

**Files:**
- Modify: `desktop/crates/session/src/state.rs`
- Modify: `desktop/crates/session/src/controlplane.rs` (broadcast sites use the new variant)
- Modify: `desktop/src-tauri/src/events.rs` (`transport_hint` now consults `Ready { transport }` directly)
- Modify: any other `SessionStateKind::Ready` literal site in the workspace.

- [ ] **Step 1: Read current state.**

```bash
rg -n "SessionStateKind::Ready" desktop/
```

You'll find broadcast sites in `controlplane.rs` plus the snapshot reader in `events.rs`.

- [ ] **Step 2: Write failing test.**

In `desktop/crates/session/src/state.rs`'s existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn ready_carries_optional_transport_tag() {
    let r = SessionStateKind::ready(Some(TransportTag::Usb));
    assert_eq!(r.label(), "ready");
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains(r#""kind":"ready""#));
    assert!(json.contains(r#""transport":"usb""#),
        "expected transport tag in JSON, got: {json}");
}

#[test]
fn legacy_ready_payload_still_deserializes() {
    let s: SessionStateKind = serde_json::from_str(r#"{"kind":"ready"}"#).unwrap();
    assert!(matches!(s, SessionStateKind::Ready { transport: None }),
        "got {s:?}");
}
```

- [ ] **Step 3: Run; expect failure (Ready is currently unit variant).**

```bash
cd desktop && cargo test -p session ready_carries
```

- [ ] **Step 4: Change `Ready` to a struct variant.**

In `state.rs`:

```rust
pub enum SessionStateKind {
    Idle,
    Listening { control_port: u16, media_port: u16 },
    #[serde(rename = "handshaking")]
    WifiHandshake,
    #[serde(rename = "usb_handshake")]
    UsbHandshake { udid: String },
    Ready {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transport: Option<TransportTag>,
    },
    Reconnecting,
    Closed { reason: String },
}

impl SessionStateKind {
    pub fn ready(transport: Option<TransportTag>) -> Self {
        Self::Ready { transport }
    }
    // … existing helpers unchanged …
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Listening { .. } => "listening",
            Self::WifiHandshake => "handshaking",
            Self::UsbHandshake { .. } => "usb_handshake",
            Self::Ready { .. } => "ready",
            Self::Reconnecting => "reconnecting",
            Self::Closed { .. } => "closed",
        }
    }
}
```

`#[serde(default)]` + `skip_serializing_if = Option::is_none` makes the new field backward-compatible: old JSON `{"kind":"ready"}` decodes as `Ready { transport: None }`; serializing `Ready { transport: None }` produces `{"kind":"ready"}` again.

- [ ] **Step 5: Update all `SessionStateKind::Ready` literal sites.**

Find sites:

```bash
rg -n "SessionStateKind::Ready" desktop/ ios/
```

Replace each with the appropriate constructor:
- In `session/src/controlplane.rs` — the Wi-Fi path emits `Ready { transport: Some(TransportTag::Wifi) }`; the USB path will set `Some(Usb)` after Task 2 plumbs it through.
- In `app/src/lib.rs` USB consumer — pass `Some(TransportTag::Usb)` to whatever broadcasts the state.
- Test sites — most can use `SessionStateKind::ready(None)` for backwards compat.

Note: `ControlPlane` doesn't currently know which transport it's running on. Add an `Arc<TransportTag>` or just a `TransportTag` field on `ControlPlane`, set in `ControlPlane::new` (add the param). Update callers (in `app/src/lib.rs`).

- [ ] **Step 6: Update `events::pump` in `src-tauri`.**

Replace the existing `transport_hint(kind)` body (which looked only at handshake variants) with one that ALSO looks at `Ready { transport: Some(t) }`:

```rust
fn transport_hint(kind: &SessionStateKind) -> Option<&'static str> {
    match kind {
        SessionStateKind::WifiHandshake => Some("wifi"),
        SessionStateKind::UsbHandshake { .. } => Some("usb"),
        SessionStateKind::Ready { transport: Some(TransportTag::Wifi) } => Some("wifi"),
        SessionStateKind::Ready { transport: Some(TransportTag::Usb) } => Some("usb"),
        _ => None,
    }
}
```

(import `TransportTag` from `session`).

The `last_transport` heuristic in `pump` can stay — it now handles legacy `Ready { transport: None }` gracefully.

- [ ] **Step 7: Run all gates.**

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

All green.

- [ ] **Step 8: Commit.**

```bash
git add -u
git commit -m "feat(session): SessionStateKind::Ready { transport: Option<TransportTag> }"
```

---

# Section 6b-B — ControlPlane telemetry fan-out

### Task 2: `ControlPlane::run_with_outbound` accepts optional `telemetry_tx`

**Files:**
- Modify: `desktop/crates/session/src/controlplane.rs`
- Modify: `desktop/crates/app/src/lib.rs` (callers — Wi-Fi accept loop AND USB supervisor consumer)

- [ ] **Step 1: Read** `controlplane.rs` to understand the existing signature. It's likely:

```rust
pub async fn run_with_outbound(
    self,
    accepted: AcceptedSession,
    control: ControlStream,
    out_rx: mpsc::Receiver<ControlMessage>,
) -> Result<(), SessionError>
```

The existing code already pattern-matches on `ControlMessage::Telemetry(t)` and dispatches `ControlPlaneEvent::Telemetry(t.clone())` via `events_tx`. Add a parallel send to the new optional channel.

- [ ] **Step 2: Add the param.**

```rust
pub async fn run_with_outbound(
    self,
    accepted: AcceptedSession,
    control: ControlStream,
    out_rx: mpsc::Receiver<ControlMessage>,
    telemetry_tx: Option<mpsc::Sender<Telemetry>>,
) -> Result<(), SessionError>
```

In the `ControlMessage::Telemetry(t)` arm, add (best-effort, non-blocking):

```rust
if let Some(tt) = &telemetry_tx {
    let _ = tt.try_send(t.clone());
}
```

`try_send` (not `send().await`) prevents the control plane from blocking when the driver is overloaded — congestion in the driver should not propagate back to the wire.

- [ ] **Step 3: Update callers in `app/src/lib.rs`.**

Both Wi-Fi `spawn_accept_loop` and USB consumer call `cp.run_with_outbound(...)`. Update each:

```rust
let (telemetry_tx, mut telemetry_rx) = mpsc::channel::<ccp_protocol::Telemetry>(16);

// Spawn the AdaptationDriver task that consumes telemetry_rx.
let driver_outbound = out_tx.clone();
tokio::spawn(async move {
    use app::AdaptationDriver;
    let mut driver = AdaptationDriver::new(driver_outbound, /* initial mode */ default_streaming_mode_runtime());
    while let Some(t) = telemetry_rx.recv().await {
        driver.observe(&t, 0.0).await;  // RTT = 0 for Phase 6b
    }
});

if let Err(e) = cp.run_with_outbound(acc, cs, out_rx, Some(telemetry_tx)).await {
    warn!(error = ?e, "control plane exited");
}
```

You'll need `default_streaming_mode_runtime()` — define it inside `app/src/lib.rs` as a private helper returning a sensible default `Mode` (encoded HEVC 1080p30). The test helper from `adaptation_driver.rs` was `#[cfg(test)]` — duplicate the body in production code or expose a public `Mode::default_streaming_1080p30()` helper in `ccp-protocol` (the latter is cleaner; do that if it doesn't already exist).

- [ ] **Step 4: Test.**

Add to `controlplane.rs` tests:

```rust
#[tokio::test]
async fn telemetry_tx_receives_inbound_telemetry() {
    // existing scaffolding pattern — drive a HELLO/AUTH happy path, then
    // inject a Telemetry message and assert the (events_tx, telemetry_tx)
    // both observe it.
    /* … mirror an existing controlplane test … */
}
```

(If the existing test infrastructure for `ControlPlane` is non-trivial, this test can also be added as a small integration test in `desktop/crates/app/tests/`. Pick the level where similar tests already live.)

- [ ] **Step 5: Run all gates.**

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 6: Commit.**

```bash
git add -u
git commit -m "feat(session): ControlPlane forks Telemetry to optional adaptation channel; AppCore wires AdaptationDriver per session"
```

---

# Section 6b-C — SET_MODE wire round-trip test

### Task 3: e2e — driver decides → SET_MODE on wire → mock MODE_APPLIED

**Files:**
- Create: `desktop/crates/app/tests/set_mode_wire_e2e.rs`

- [ ] **Step 1: Write the integration test.**

This test composes Phase 5/6a primitives:

```rust
//! Phase 6b end-to-end: scripted congestion telemetry from mock-iphone makes
//! AdaptationDriver emit SET_MODE on the wire; mock-iphone receives SET_MODE
//! and acks MODE_APPLIED back through control.

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
async fn driver_emits_set_mode_under_congestion_and_mock_acks() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    // Standard USB session setup (copied from usb_full_session_e2e).
    let cport = pick_free_port().await;
    let mport = pick_free_port().await;

    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PairingStore::open_at(tmp.path().join("p.toml")).await.unwrap());
    let key = PairingMaterial::new_random();
    let key_b64 = key.as_b64();
    store.put("E2E-UDID", key).await.unwrap();

    let mock_args = mock_iphone::Args {
        host: "127.0.0.1".into(), cport, mport,
        token: key_b64.clone(),
        width: 1280, height: 720, fps: 30, send_video: false,
        transport: mock_iphone::TransportMode::UsbListener,
    };
    let mock_task = tokio::spawn(mock_iphone::run(mock_args));
    tokio::time::sleep(Duration::from_millis(150)).await;

    let lb = Arc::new(LoopbackConductor::new());
    lb.register(LoopbackDevice {
        id: 1, udid: "E2E-UDID".into(),
        control_addr: format!("127.0.0.1:{cport}").parse().unwrap(),
        media_addr:   format!("127.0.0.1:{mport}").parse().unwrap(),
    }).await;

    let app = AppHandle::for_tests_with_pairing(store.clone()).await;
    app.start_usb_supervisor(lb, store, Duration::from_millis(50)).await;

    // Wait for Ready.
    let mut snap = app.snapshot.clone();
    for _ in 0..200 {
        if matches!(snap.borrow().state, SessionStateKind::Ready { .. }) { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(matches!(snap.borrow().state, SessionStateKind::Ready { .. }),
            "session never reached Ready, got {:?}", snap.borrow().state);

    // Mock-iphone normally emits stable Telemetry every 500ms. Phase 6b can't
    // easily *force* congested telemetry from mock-iphone without making the
    // mock more configurable. For this Phase 6b test we instead verify that
    // the driver-fanout machinery is wired (telemetry channel exists and
    // delivers samples) — by counting that AT LEAST ONE Telemetry event was
    // observable to AppCore within 1.5 seconds.
    //
    // Real congestion → SET_MODE → MODE_APPLIED end-to-end requires a mock
    // that can be commanded to send Telemetry with rising queue_depth. That
    // upgrade lands in Phase 6c when the speedtest (Task 4 below) also makes
    // mock-iphone more controllable.
    //
    // For now this test asserts the structural soundness of the Phase 6b
    // wiring: a Ready USB session, with the driver bridge running, sees at
    // least one telemetry frame.
    let mut events = app.events.lock().await;
    let mut saw_telemetry = false;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(1500);
    while tokio::time::Instant::now() < deadline {
        tokio::select! {
            ev = events.recv() => {
                if let Some(session::ControlPlaneEvent::Telemetry(_)) = ev { saw_telemetry = true; break; }
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
        }
    }
    drop(events);
    assert!(saw_telemetry, "driver bridge never saw a Telemetry sample");

    mock_task.abort();
}
```

> The "Phase 6c-class" full SET_MODE round-trip is deferred — see comment in the test body — because mock-iphone needs a knob to send congested telemetry. Task 4 below makes mock more controllable in service of speedtest; once that lands a follow-up test can drive real SET_MODE.

- [ ] **Step 2: Run; expect green (it tests Task 2's plumbing).**

```bash
cargo test -p app --test set_mode_wire_e2e
```

If the mock's normal telemetry emission is too fast or too slow to be observable at this layer, the 1.5s deadline might need tuning. Verify the test is reliable across 10 consecutive runs:

```bash
for i in $(seq 1 10); do cargo test -p app --test set_mode_wire_e2e --quiet || break; done
```

- [ ] **Step 3: Commit.**

```bash
git add -u
git commit -m "test(app): set_mode_wire_e2e — verifies driver fan-out delivers Telemetry to AdaptationDriver"
```

---

# Section 6b-D — Live speedtest ramp

### Task 4: mock-iphone responds to SPEEDTEST_START with synthetic ramp

**Files:**
- Modify: `desktop/tools/mock-iphone/src/lib.rs`
- Modify: `desktop/crates/mediapipeline/src/pipeline.rs` (route reserved seq range → counter)
- Modify: `desktop/crates/adaptive/src/speedtest.rs` (consume per-step throughput)

- [ ] **Step 1: Mediapipeline — split inbound by seq range.**

In `desktop/crates/mediapipeline/src/pipeline.rs` (or wherever inbound media frames are dispatched to sinks), detect:

```rust
const SPEEDTEST_SEQ_BASE: u32 = 0xFFFF_0000;

fn is_speedtest_marker(seq: u32) -> bool { seq >= SPEEDTEST_SEQ_BASE }
```

When true, do not invoke the FrameSink fanout. Instead, count the bytes and the per-step index (seq's lower 16 bits) in a separate `SpeedTestCounter` that the speedtest runner reads. Add `pub struct SpeedTestCounter { /* mpsc::Sender<StepObservation> */ }` and let `MediaPipeline::spawn(...)` accept an optional `Option<SpeedTestCounter>` param.

- [ ] **Step 2: mock-iphone — SPEEDTEST_START arm.**

In the per-session control loop in `tools/mock-iphone/src/lib.rs`, add a `ControlMessage::SpeedtestStart(st)` arm. Pull the per-step targets from the message; for each step, blast synthetic NV12 frames at the step's bitrate via the existing `write_media_frame` helper, with `seq` starting at `0xFFFF_0000 + step_index * 1000` to mark them.

(Reuse the existing `fill_synthetic_nv12` helper.)

- [ ] **Step 3: Adaptive — add `SpeedTestRunner` that drives & observes.**

Already-existing `adaptive::speedtest::evaluate(observations)` (from Phase 4) takes a slice. Phase 6b adds the runner around it: `SpeedTestRunner::start(out: &mut ControlStream, pattern, duration_ms)` sends `SPEEDTEST_START`, listens on the `StepObservation` channel from mediapipeline, accumulates per-step throughput, then calls `evaluate`.

- [ ] **Step 4: Test.**

Add a unit test that runs the whole speedtest against a mock that pushes deterministic frames per step. The assertion: returned `Measurement.goodput_mbps` matches the step the runner ramped to before "congestion" was detected (simulate by having mock stop responding at step N).

Skip if scope balloons — Phase 6c can finalize the runner.

- [ ] **Step 5: Run gates.**

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 6: Commit.**

```bash
git add -u
git commit -m "feat(speedtest): live ramp generator (mock) + per-step counter (mediapipeline) + runner (adaptive)"
```

> **Task 4 may grow large.** If implementing the full SPEEDTEST_START arm + mediapipeline counter + runner takes more than ~200 lines, **stop and split**. Land the mediapipeline reserved-seq routing first (it's strictly defensive and useful by itself), then split the mock-iphone + runner into a follow-up Task 4b. Acceptable to ship Phase 6b with only the routing piece — runner becomes Phase 6c.

---

# Section 6b-E — IdeviceConductor native subscribe (best effort)

### Task 5: Override `UsbConductor::subscribe` for `IdeviceConductor` (or skip)

**Files:**
- Modify: `desktop/crates/transport/src/usb/idevice_backend.rs`

- [ ] **Step 1: Check `idevice` 0.1.61 for a listen API.**

```bash
cargo doc -p idevice --no-deps --features usbmuxd,ring
grep -rIn "listen\|subscribe\|stream\|events" ~/.cargo/registry/src/index.crates.io-*/idevice-0.1.61/src/usbmuxd/ | head -20
```

If a `UsbmuxdConnection::listen` (or similar streaming method) exists that yields device-add/device-remove events as they happen, override `UsbConductor::subscribe` on `IdeviceConductor` to use it. The default polling-impl in the trait stays for `LoopbackConductor` (which doesn't need it — registrations are explicit).

- [ ] **Step 2: If the native listen API isn't there, skip this task.**

Add a note in the Phase 6b acceptance log: "idevice 0.1.61 does not expose a native device-event stream; polling-fallback remains the implementation. Re-investigate when crate ≥ 0.2."

- [ ] **Step 3: If implemented:**

```rust
fn subscribe(self: Arc<Self>, _poll: Duration) -> mpsc::Receiver<UsbEvent>
where Self: Sized
{
    let (tx, rx) = mpsc::channel(16);
    tokio::spawn(async move {
        // ... use idevice's listen() to feed `tx` ...
    });
    rx
}
```

- [ ] **Step 4: Commit (either with the override, or with the acceptance-log note in the Phase 6b retro).**

```bash
git commit -m "chore(transport): IdeviceConductor.subscribe override (or skip note)"
```

---

# Section 6b-F — Finalise

### Task 6: Acceptance log + tag

- [ ] **Step 1: Run all gates.**

```bash
cd desktop
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p transport --features usb-idevice
cd ui && pnpm typecheck && pnpm build
cd ../../ios && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test
```

All green.

- [ ] **Step 2: Append acceptance log** at the end of `plans/phase-6b-adaptation-and-speedtest.md`. Mirror Phase 6a's structure (What landed / Gates / Deferred / Retrospective).

- [ ] **Step 3: Tag.**

```bash
git add plans/phase-6b-adaptation-and-speedtest.md
git commit -m "docs(plans): Phase 6b acceptance log"
git tag v0.6.0b-phase6b
```

- [ ] **Step 4: Merge.**

```bash
git checkout main
git merge --ff-only feature/phase-6b-adaptation-and-speedtest
git branch -d feature/phase-6b-adaptation-and-speedtest
```

---

## Acceptance log

### 2026-05-31 — Phase 6b adaptation production + live speedtest

**What landed**

- **session** — `SessionStateKind::Ready { transport: Option<TransportTag> }` (backward-compat JSON via `#[serde(default, skip_serializing_if)]`); `ControlPlane` carries a `TransportTag` and broadcasts it. `run_with_outbound` gains `telemetry_tx: Option<mpsc::Sender<Telemetry>>` and forks inbound telemetry via `try_send` (non-blocking — slow driver can't back-pressure the wire). New `ControlPlaneEvent::ModeApplied { mode }` surfaced from inbound `MODE_APPLIED`.
- **app** — per-session `AdaptationDriver` bridge spawned on both Wi-Fi and USB paths, consuming the forked telemetry and emitting `SET_MODE` on congestion (Phase 6a's headless driver is now a production path). Per-session `SpeedTestCounter` slot; `run_speedtest()` is now a real ramp (sends `SPEEDTEST_START`, drains the counter, builds `StepObservation`s, `adaptive::evaluate`), with the telemetry-proxy stub retained as a no-session fallback. **Latent bug fixed:** the USB media path never consumed `MEDIA_HELLO` before spawning `MediaPipeline` — the pipeline silently died on `BadMagic`; added an inline `read_media_hello` (mirrors the Wi-Fi path).
- **ccp-protocol** — `Mode::default_streaming_1080p30()` production helper (HEVC 1080p30 @ 30 Mbps).
- **mediapipeline** — `SpeedTestCounter` + reserved-seq routing: media frames with `seq >= 0xFFFF_0000` are tallied per-step and never fanned to the video sinks. `MediaPipeline::spawn` takes `Option<SpeedTestCounter>`.
- **adaptive** — reused existing `SpeedtestPlan`/`evaluate`/`frames_per_step_for_target` (Phase 4 scaffolding) — no changes needed beyond consumption.
- **mock-iphone** — `--congested` knob (rising queue_depth/drop_count for closed-loop adaptation tests); `SPEEDTEST_START` arm spawning `speedtest_ramp` (per-step reserved-seq NV12 filler at ramp bitrates).
- **transport** — `IdeviceConductor::subscribe` overrides the polling default with usbmuxd's native `Listen` stream (instant device-add/remove, no 500ms poll latency). Runs in a dedicated OS thread + current-thread runtime because idevice's stream is non-`Send`. `LoopbackConductor` keeps the polling default.
- **tauri** — `events::pump` emits `session://transport_changed` (from `Ready { transport }` + handshake variants) and `session://mode_applied`.

**New tests**

- `session`: `ready_*` (3), `telemetry_is_forked_to_optional_channel`.
- `ccp-protocol`: `default_streaming_mode_is_hevc_1080p30`.
- `mediapipeline`: `speedtest_marker_frames_bypass_sinks_and_hit_counter`.
- `app` e2e: `set_mode_wire_e2e` (closed-loop congestion → SET_MODE → MODE_APPLIED, 5/5 stable), `speedtest_live_e2e` (SPEEDTEST_START → ramp → counter → evaluate → positive goodput, 5/5 stable).

### Gates

- `cargo fmt --check` ✓
- `cargo clippy --workspace --all-targets -- -D warnings` ✓
- `cargo test --workspace` ✓ (121 passed, 0 failed)
- `cargo build -p transport --features usb-idevice` ✓
- `cargo clippy -p transport --features usb-idevice` ✓
- `pnpm -C desktop/ui typecheck && build` ✓
- `DEVELOPER_DIR=Xcode swift test` ✓ (58 passed, 0 failed)

### Deferred to Phase 6c (hardware-dependent)

1. **Real VideoToolbox `VTEncoder` (iOS) / `VTDecoder` (macOS).** Needs `VTCompressionSession` + objc2 bindings; only meaningful with a device + camera.
2. **Mid-stream Wi-Fi↔USB switchover** without dropping the active session (supervisor currently dials fresh; the selector logic exists but no graceful handover).
3. **`ffmpeg-next` decoder adapter** (Linux/Windows fallback when no VideoToolbox).
4. **Reconnect throttling / backoff tuning** on real network failures.
5. **iOS `NWListener` on-device acceptance testing** (sandbox blocks `swift test` from accepting inbound connections).
6. **Live RTT into the AdaptationDriver** — currently passes `rtt_ms = 0.0`; real PING/PONG-derived RTT would let the step-down logic use the RTT-doubling signal.
7. **`run_speedtest` real-network calibration** — over loopback the ramp always saturates (≈800 Mbps); thresholds need tuning against an actual Wi-Fi link before v1.

### CMIO Camera Extension (Phase 3)

Still blocked on Apple Developer ID — see [docs/14-apple-developer-id-guide.md](../docs/14-apple-developer-id-guide.md).

## Retrospective

### What landed

- The adaptation loop is now **closed end-to-end headlessly**: congested telemetry from the mock makes the desktop emit `SET_MODE`, the mock acks `MODE_APPLIED`, and the desktop surfaces it as an event. This was the single biggest "wired but not connected" gap from Phase 6a.
- The speedtest is **live**: `run_speedtest()` actually exercises the media socket with a ramp and measures goodput via the reserved-seq counter. The whole `SPEEDTEST_START → ramp → counter → evaluate` path runs in a test.
- `IdeviceConductor` now gets **instant** device events on real hardware instead of polling.
- A real latent bug (USB media pipeline dying on unconsumed `MEDIA_HELLO`) was caught only because the live speedtest needed the pipeline alive — a good argument for end-to-end tests over unit-only coverage.

### Deviations from the plan

1. **Task 3 upgraded from structural to a true closed loop.** The plan scoped Task 3 as "verify telemetry reaches the driver"; since Task 2 already wired the driver, we instead added `ControlPlaneEvent::ModeApplied` + a mock congestion knob and asserted the full loop closes. Higher value, slightly more code.
2. **`IdeviceConductor::subscribe` runs on a dedicated OS thread**, not `tokio::spawn`. idevice 0.1.61's `listen()` returns a non-`Send` stream; a current-thread runtime on its own thread is the clean workaround. `tx` is `Send` so events cross the boundary fine.
3. **USB `read_media_hello` fix** was not in the plan — it surfaced as a prerequisite for the live speedtest (the pipeline must survive to count ramp frames). It also hardens real USB video streaming.
4. **`run_speedtest` keeps the telemetry-proxy fallback** for the no-active-session case rather than erroring — keeps the existing UI contract intact.

### Open questions for real hardware (Phase 6c)

- `IdeviceConductor::subscribe`'s dedicated thread loops on `stream.next()` forever; it exits when the receiver drops, but only on the next event. For a long-lived app-startup subscription this is fine; verify no thread accumulation if subscribe is ever called repeatedly.
- The speedtest ramp saturates loopback (≈800 Mbps); real-Wi-Fi step thresholds in `adaptive::evaluate` (85% of target, RTT-doubling) need calibration against an actual link.
- `AdaptationDriver` passes `rtt_ms = 0.0` — confirm the queue_depth/drop_count signals alone are sufficient on real telemetry, or wire PING/PONG RTT.
