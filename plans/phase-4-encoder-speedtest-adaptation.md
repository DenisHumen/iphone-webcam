# Phase 4 — Encoder + Speedtest + Adaptation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** End-to-end "auto" quality on regular Wi-Fi. A speedtest measures sustained goodput, an adaptive engine picks the best `Mode` (RAW or HEVC visually-lossless), and a live adaptation loop step-up/down on congestion. iOS gains a VideoToolbox HEVC/H.264 encoder; desktop gets a `Decoder` trait so encoded frames join the same `mediapipeline` fanout as RAW.

**Architecture:**
- `adaptive::SpeedTest` orchestrates a ramp pattern over the media socket. The iPhone (or mock) generates synthetic media frames at increasing target bitrates; the desktop measures sustained goodput + RTT/jitter and emits `SPEEDTEST_RESULT(recommendedMode)`.
- `adaptive::select_mode(measurement, caps, limits)` picks the best `Mode` from a table-driven cost function (raw vs encoded, res×fps, prefer_raw).
- `adaptive::adaptation_loop` watches `TELEMETRY` + local pipeline metrics; on congestion it steps down (fps → bitrate → raw→encoded → res), with hysteresis + cooldown.
- iOS `Encoder` protocol with `VTEncoder` impl (VideoToolbox `VTCompressionSession`). Config keyframes carry VPS/SPS/PPS.
- Desktop `decode::Decoder` trait. `mediapipeline` routes by `Codec`: `Raw` → direct fanout (Phase 2 path), `Hevc|H264` → decoder → NV12 → same fanout.
- Decoder strategy: **ship a stub `PassthroughDecoder`** that mirrors the encoded payload as if it were a fake NV12 plane (enough to exercise the routing in CI without pulling ffmpeg). Production `VideoToolboxDecoder` (macOS) and `FfmpegDecoder` (Linux/Win) land in Phase 6 polish — out of scope here.

**Tech Stack:** Rust 1.95 stable, tokio. New iOS: VideoToolbox. No new system deps in CI.

**Acceptance:**
- `cargo test --workspace --all-targets` — all green; new tests cover `select_mode` table, adaptation state machine, speedtest measurement aggregator, encoded routing in `mediapipeline`.
- `swift test` — all green; new tests cover VT encoder config injection (golden NAL unit prefixes) via an injectable `EncodedSession` mock, and mode-picker plumbing.
- `pnpm typecheck && pnpm build` — green; UI shows speedtest overlay + Auto/RAW/Lossy mode picker + adaptation banner.
- e2e test: mock-iphone receives `SPEEDTEST_START`, generates synthetic ramp, desktop produces a sensible `recommendedMode`.
- e2e test: telemetry showing growing `queueDepth` triggers a `step_down` and SET_MODE on the wire.
- CI green; tag `v0.4.0-phase4`.

**Out of scope:**
- Real VideoToolbox decoder on the desktop (Phase 6 polish — uses the same `Decoder` trait).
- ffmpeg adapter (Phase 6).
- USB transport (Phase 5).
- Virtual camera sink (Phase 3, deferred until Apple Developer ID is in hand).

---

## File Structure

### Rust changes
```
desktop/
├── Cargo.toml                                       # MOD: + maybe nalgebra-free stats; no new deps if possible
├── crates/
│   ├── adaptive/
│   │   ├── Cargo.toml                               # MOD: deps ccp-protocol + tokio + tracing + serde
│   │   └── src/
│   │       ├── lib.rs                               # NEW: module wiring
│   │       ├── tables.rs                            # NEW: raw_bitrate_kbps(), hevc_target_kbps()
│   │       ├── types.rs                             # NEW: Measurement, UserLimits, ChannelClass
│   │       ├── select_mode.rs                       # NEW: ranker
│   │       ├── speedtest.rs                         # NEW: ramp generator + aggregator
│   │       └── adaptation.rs                        # NEW: state machine (step up/down with cooldown)
│   ├── decode/
│   │   ├── Cargo.toml                               # MOD: trait crate
│   │   └── src/
│   │       ├── lib.rs                               # NEW: Decoder trait
│   │       └── passthrough.rs                       # NEW: stub decoder for tests/Phase 4 CI
│   ├── mediapipeline/
│   │   └── src/pipeline.rs                          # MOD: route by Codec; lookup Decoder for ENCODED
│   ├── app/
│   │   └── src/lib.rs                               # MOD: AppCore owns AdaptiveEngine + SpeedTest; new outbound paths
│   └── session/
│       └── src/state.rs                             # MOD: SessionStateKind gains SpeedTest variant
├── src-tauri/
│   └── src/
│       ├── commands.rs                              # MOD: run_speedtest, set_mode, get_recommended_mode
│       └── events.rs                                # MOD: session://speedtest_progress, session://mode_applied
└── tools/
    └── mock-iphone/
        └── src/main.rs                              # MOD: respond to SPEEDTEST_START with synthetic ramp
```

### iOS changes
```
ios/Sources/ClearCamCore/
├── Encode/
│   ├── EncoderProtocol.swift                        # NEW: protocol + EncoderMode
│   ├── VTEncoder.swift                              # NEW (iOS-only #if): VTCompressionSession driver
│   └── EncodedFramePacker.swift                     # NEW: AU + config → bytes ready for media socket
└── Session/
    └── SessionController.swift                      # MOD: handle SET_MODE; route raw vs encoded path

ios/Tests/ClearCamCoreTests/
├── EncoderProtocolTests.swift                       # NEW: stub encoder + flag bits + config-frame layout
└── ModeSwitchTests.swift                            # NEW: SET_MODE → MODE_APPLIED roundtrip
```

### UI changes
```
desktop/ui/src/
├── lib/types.ts                                     # MOD: Mode, SpeedtestProgress, SpeedtestResult
├── lib/tauri.ts                                     # MOD: runSpeedtest(), setMode(), onSpeedtestProgress
├── components/
│   ├── ModePicker.tsx                               # NEW: Auto/RAW/Lossy buttons
│   ├── SpeedtestOverlay.tsx                         # NEW: animated ramp progress + final result
│   └── AdaptationBanner.tsx                         # NEW: transient "mode changed" toast
└── App.tsx                                          # MOD: hook commands + events
```

### CI
- No changes. Phase 4 stub decoder keeps the suite Ubuntu-pure.

---

## Conventions

- TDD per task; gate after each task.
- Pure-Rust adaptive engine — no async in the decision algorithms (synchronous, easy to unit-test).
- Mock encoder/decoder stubs are first-class — they let the entire pipeline run on CI without VT/ffmpeg.

---

# Section 4A — Adaptive engine (headless-testable)

### Task 1: Bitrate tables + types

**Files:**
- Modify: `desktop/crates/adaptive/Cargo.toml`
- Create: `desktop/crates/adaptive/src/tables.rs`
- Create: `desktop/crates/adaptive/src/types.rs`
- Create: `desktop/crates/adaptive/src/lib.rs`

- [ ] **Step 1: Cargo.toml**

```toml
[dependencies]
ccp-protocol = { path = "../ccp-protocol" }
serde = { workspace = true }
tracing = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 2: tables.rs — bitrate math**

```rust
pub fn raw_bitrate_kbps(width: u32, height: u32, fps: u32) -> u64 {
    (u64::from(width) * u64::from(height) * 3 / 2 * 8 * u64::from(fps)) / 1000
}

pub fn hevc_target_kbps(width: u32, height: u32, fps: u32) -> u64 {
    // Table per docs/06 §2.2. Linear-interpolated for unknown modes.
    match (width, height, fps) {
        (1280, 720, 30) => 12_000,
        (1280, 720, 60) => 20_000,
        (1920, 1080, 30) => 30_000,
        (1920, 1080, 60) => 50_000,
        (3840, 2160, 30) => 90_000,
        _ => {
            let area = u64::from(width) * u64::from(height);
            (area * u64::from(fps)) / 16_000  // coarse fallback
        }
    }
}

#[cfg(test)]
mod tests { /* assert exact numbers from docs/06 */ }
```

- [ ] **Step 3: types.rs**

```rust
use ccp_protocol::Capability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub goodput_mbps: f64,
    pub rtt_ms: f64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportClass { WiFi, Usb2, Usb3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLimits {
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
    pub max_fps: Option<u16>,
    pub prefer_raw: bool,
}

impl Default for UserLimits {
    fn default() -> Self {
        Self { max_width: None, max_height: None, max_fps: None, prefer_raw: false }
    }
}

#[derive(Debug, Clone)]
pub struct AvailableMode {
    pub width: u16,
    pub height: u16,
    pub fps: u16,
    pub caps: Vec<Capability>,
}
```

- [ ] **Step 4: Wire + tests + commit.**

---

### Task 2: select_mode (TDD)

**Files:**
- Create: `desktop/crates/adaptive/src/select_mode.rs`

- [ ] **Step 1: Spec from docs/06 §4. Rank by (res_area, fps, format_pref) where raw>encoded at equal res/fps; prefer_raw filters to raw-only candidates first.**
- [ ] **Step 2: Tests cover:**
  - 50 Mbps Wi-Fi + 1080p caps → encoded HEVC 1080p30 (~30 Mbps under 0.7×50=35 Mbps headroom).
  - 500 Mbps USB3 + 1080p caps → raw 1080p30 (746/0.7 ≈ 1066, so 500 too low for raw; encoded picked).
  - 1.2 Gbps USB3 → raw 1080p30 wins (1066 < 1200×0.7=840…actually 1080p30 raw=746, headroom=840 OK).
  - `prefer_raw=true`, 800 Mbps Wi-Fi → raw 720p60 (664 Mbps) preferred over encoded 1080p.
  - Caps don't list HEVC and channel insufficient for raw → safe fallback (encoded 720p30 if H.264 available).

---

### Task 3: Speedtest aggregator (TDD)

**Files:**
- Create: `desktop/crates/adaptive/src/speedtest.rs`

- [ ] **Step 1:** Ramp pattern: `[50, 100, 200, 400, 800] Mbps`, `step_ms = 500`.
- [ ] **Step 2:** `SpeedTestRunner::start(out: OutboundSender)` sends `SPEEDTEST_START`. As media frames arrive (received-bytes counter on `mediapipeline`), the runner aggregates per-step throughput.
- [ ] **Step 3:** Tests: stub PING/PONG + injected throughput counter → returns expected `Measurement` and `recommended_mode` via select_mode.

---

### Task 4: Adaptation state machine (TDD)

**Files:**
- Create: `desktop/crates/adaptive/src/adaptation.rs`

- [ ] **Step 1:** State: `current_mode`, `consecutive_congestion`, `consecutive_stable`, `last_step_at`.
- [ ] **Step 2:** Signals: `queueDepth>0`, `dropCount` rising, RTT EWMA > 2× base. Each invocation passes telemetry; output is `Action::None | StepDown(new_mode) | StepUp(new_mode)` with hysteresis (down: 1 sample; up: 4 consecutive stable + 5s cooldown).
- [ ] **Step 3:** Tests: scripted telemetry sequences assert exact step decisions.

---

# Section 4B — Speedtest wire integration

### Task 5: mock-iphone responds to SPEEDTEST_START

- iPhone-side: on `SPEEDTEST_START(id, durationMs, pattern)`, push synthetic NV12 at the requested per-step bitrate. Mark frames with `seq` in a reserved range (e.g., 0xFFFF_0000+) so MediaPipeline can route them to the speedtest counter, not the preview.

### Task 6: Desktop run_speedtest Tauri command

- Returns `SpeedtestResult` to the UI; emits `session://speedtest_progress` per step.

### Task 7: e2e speedtest test

- mock-iphone connects, desktop calls `app.run_speedtest()`, assert the returned `Measurement` has positive goodput.

---

# Section 4C — Encoded media path on the desktop

### Task 8: `decode::Decoder` trait + `PassthroughDecoder`

```rust
#[async_trait]
pub trait Decoder: Send + Sync {
    async fn decode(&self, codec: Codec, payload: Bytes, hint: FrameHint) -> Option<Frame>;
}

pub struct PassthroughDecoder; // makes a synthetic 16x16 NV12 from the AU's hash
```

### Task 9: `mediapipeline` routes by codec

- For RAW: existing path (Phase 2).
- For HEVC/H264: pass to `decoder.decode()`; if `Some(frame)`, fanout to sinks.
- Wire `PassthroughDecoder` in `AppCore::start()` by default; future `VideoToolboxDecoder` slots in here.

### Task 10: e2e — encoded frame routed through PassthroughDecoder → CapturingSink.

---

# Section 4D — iOS encoder

### Task 11: `EncoderProtocol` + `EncoderMode`

```swift
public protocol Encoder: Sendable {
    func configure(mode: EncoderMode) async throws
    func encode(_ pixelBuffer: CVPixelBuffer, pts: CMTime, onAccessUnit: @Sendable (EncodedAU) -> Void)
    func stop() async
}

public struct EncodedAU: Sendable {
    public let isKeyframe: Bool
    public let isConfig: Bool        // VPS/SPS/PPS-only frame
    public let nalUnits: [Data]      // length-prefixed in wireBytes()
}
```

### Task 12: `VTEncoder` (iOS-only #if canImport(VideoToolbox))

- `VTCompressionSession`, `RealTime=true`, `AllowFrameReordering=false`, `AverageBitRate`, `DataRateLimits`, keyframe interval.
- On format change: emit a `config` frame carrying VPS/SPS/PPS as length-prefixed NALs (`Flags.config|encoded`).

### Task 13: SessionController routes RAW vs ENCODED

- Currently captures NV12 and calls `sendCapturedNV12`. After Phase 4: when current mode is encoded, feed the CVPixelBuffer to the encoder; encoder emits NAL units; we pack them and call `mediaChannel.sendRaw` with `Flags.ENCODED` set.
- React to `SET_MODE`: reconfigure encoder; send `MODE_APPLIED(atSeq)`.

---

# Section 4E — UI controls

### Task 14: `lib/types.ts` Mode + Measurement.

### Task 15: `ModePicker` component (Auto/RAW/Lossy).

### Task 16: `SpeedtestOverlay` (per-step progress) + `AdaptationBanner` (transient toast on mode change).

---

# Section 4F — Integration & tag

### Task 17: Tauri commands wiring + events.

### Task 18: CI updates if needed; tag `v0.4.0-phase4`.

### Task 19: Retrospective.

---

## Acceptance log

_(populated as the plan executes)_

## Retrospective

_(populated at end of Phase 4)_
