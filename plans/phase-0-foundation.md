# Phase 0 — Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Within each task, use `superpowers:test-driven-development` for any task that writes code (tests first, then implementation, then verify). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the project skeleton: a Rust workspace at `desktop/` with the `ccp-protocol` crate (serde control types, length-prefixed JSON framing, binary media-header codec, version/caps) plus 7 empty stub crates; a Tauri 2 + React + Vite + TypeScript + Tailwind UI shell that opens an empty window; an iOS SwiftPM package with a `ClearCamProtocol` module mirroring the Rust types byte-for-byte (binary) and value-for-value (JSON); GitHub Actions CI that runs `fmt`/`clippy`/`test` on Rust, lint/typecheck/build on UI, and `swift test` on iOS.

**Architecture:** Monorepo with strict module boundaries (`desktop/`, `ios/`, `docs/`). `ccp-protocol` is **pure data + (de)serialization** — no I/O, no async, no platform code, no networking. Stub crates exist so the workspace builds and CI gates the project shape, but contain no logic. iOS `ClearCamProtocol` is a SwiftPM library + XCTest target; the eventual iOS app target is **not** part of Phase 0. The Tauri scaffold opens a minimal page so that the build chain (Rust + Vite + Tauri) is wired and verified end-to-end.

**Tech Stack:**
- Rust: stable channel; crates `serde`, `serde_json`, `bytes`, `byteorder`, `bitflags`, `thiserror`, `anyhow`.
- Tauri 2.x, React 18, Vite, TypeScript, Tailwind CSS, pnpm.
- Swift 5.10+ (Xcode 15+), SwiftPM, XCTest.
- GitHub Actions (`ubuntu-latest` for Rust + UI, `macos-latest` for Swift).

**Strict scope boundaries (do NOT do any of these in Phase 0):**
- Any networking (TCP, mDNS, USB, usbmuxd).
- Any media capture, encode, decode, or pipeline logic.
- Any virtual camera sink, Camera Extension, MF, v4l2loopback code.
- Any session state machine, adaptive engine, or speed test logic.
- Any iOS app target or AVFoundation usage in Swift code.
- Any Tauri commands beyond the empty default.

If you find yourself adding code beyond pure types + tests + scaffolding, **stop and re-check this section**.

**Reference docs (read these before starting):**
- `docs/01-vision-and-requirements.md` — what we're building, constraints
- `docs/02-architecture.md` — components and boundaries
- `docs/03-protocol.md` — CCP wire format (this plan implements exactly the §5–§7 schemas)
- `docs/08-project-structure.md` — target repo layout
- `docs/09-tech-stack.md` — library choices
- `docs/10-roadmap-and-plan.md` — Phase 0 acceptance criteria
- `docs/11-testing-strategy.md` — what to test where
- `docs/12-decisions-log.md` — ADRs (especially ADR-009/012/017)

---

## File Structure (target after Phase 0)

```
iphone-webcam/
├── .editorconfig                              # editor conventions
├── .github/workflows/ci.yml                   # CI: rust, ui, swift
├── rustfmt.toml                               # rustfmt config
├── plans/phase-0-foundation.md                # this file
├── desktop/
│   ├── Cargo.toml                             # workspace manifest
│   ├── rust-toolchain.toml                    # stable channel
│   ├── crates/
│   │   ├── ccp-protocol/                      # the only crate with logic in Phase 0
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs                     # re-exports
│   │   │       ├── version.rs                 # PROTO_VER, Capability
│   │   │       ├── control/
│   │   │       │   ├── mod.rs                 # ControlEnvelope + ControlMessage enum
│   │   │       │   ├── envelope.rs            # length-prefixed framing (frame/try_unframe)
│   │   │       │   ├── handshake.rs           # Hello, HelloAck, Auth, AuthOk, MediaHello, Bye, ErrorMsg, ErrorCode
│   │   │       │   ├── device.rs              # DeviceInfo, CameraList, SetCamera, CameraState (+ CameraEntry, BatteryState, ThermalState)
│   │   │       │   ├── streaming.rs           # Mode, Start, Stop, SetMode, ModeApplied (+ FormatKind, PixelFormat)
│   │   │       │   ├── telemetry.rs           # Telemetry, Ping, Pong
│   │   │       │   └── speedtest.rs           # SpeedtestStart, SpeedtestTick, SpeedtestResult
│   │   │       └── media/
│   │   │           ├── mod.rs                 # MediaHeader struct + encode/decode + Flags, MediaType, Codec, DecodeError
│   │   ├── transport/Cargo.toml, src/lib.rs   # stub
│   │   ├── session/Cargo.toml, src/lib.rs     # stub
│   │   ├── adaptive/Cargo.toml, src/lib.rs    # stub
│   │   ├── mediapipeline/Cargo.toml, src/lib.rs # stub
│   │   ├── decode/Cargo.toml, src/lib.rs      # stub
│   │   ├── sink/Cargo.toml, src/lib.rs        # stub
│   │   └── app/Cargo.toml, src/main.rs        # stub binary
│   ├── src-tauri/                             # Tauri 2 scaffold
│   │   ├── Cargo.toml
│   │   ├── tauri.conf.json
│   │   ├── build.rs
│   │   ├── capabilities/default.json
│   │   ├── icons/icon.png                     # placeholder
│   │   └── src/
│   │       ├── main.rs
│   │       └── lib.rs
│   ├── ui/                                    # React + Vite + TS + Tailwind
│   │   ├── package.json
│   │   ├── pnpm-lock.yaml
│   │   ├── tsconfig.json
│   │   ├── tsconfig.node.json
│   │   ├── vite.config.ts
│   │   ├── tailwind.config.js
│   │   ├── postcss.config.js
│   │   ├── .eslintrc.cjs
│   │   ├── .prettierrc.json
│   │   ├── index.html
│   │   └── src/
│   │       ├── main.tsx
│   │       ├── App.tsx
│   │       ├── index.css
│   │       └── vite-env.d.ts
│   └── sinks/.gitkeep                         # empty for now
└── ios/
    ├── Package.swift                           # SwiftPM
    ├── Sources/ClearCamProtocol/
    │   ├── Version.swift                       # protoVer, Capability
    │   ├── Mode.swift                          # Mode, FormatKind, PixelFormat
    │   ├── MediaHeader.swift                   # MediaHeader codec (28-byte big-endian)
    │   ├── ControlEnvelope.swift               # length-prefixed framing + envelope wrapper
    │   ├── ControlMessage.swift                # discriminated union with all variants
    │   ├── Handshake.swift                     # Hello, HelloAck, Auth, AuthOk, MediaHello, Bye, ErrorMsg, ErrorCode
    │   ├── Device.swift                        # DeviceInfo, CameraList, SetCamera, CameraState (+ CameraEntry, BatteryState, ThermalState)
    │   ├── Streaming.swift                     # Start, Stop, SetMode, ModeApplied
    │   ├── Telemetry.swift                     # Telemetry, Ping, Pong
    │   └── Speedtest.swift                     # SpeedtestStart, SpeedtestTick, SpeedtestResult
    └── Tests/ClearCamProtocolTests/
        ├── ControlEnvelopeTests.swift          # framing round-trip
        ├── ControlMessageJSONTests.swift       # round-trip + golden JSON shape
        └── MediaHeaderTests.swift              # known-bytes binary round-trip
```

---

## Cross-task type contracts (read once; later tasks rely on these exact names)

These names appear across multiple tasks. **Do not rename** them in any task.

### Rust (ccp-protocol)

- `pub const PROTO_VER: u32 = 1;`
- `pub enum Capability { Hevc, H264, RawNv12, Usb3, SpeedtestV1, Other(String) }` — serde-encodes as string; unknown strings parse to `Other(_)` (forward compat).
- `pub struct ControlEnvelope { pub seq: u64, pub ack: Option<u64>, pub body: ControlMessage }` — `body` is `#[serde(flatten)]`.
- `pub enum ControlMessage` — `#[serde(tag = "t")]` internally-tagged enum; variant names match protocol strings exactly via `#[serde(rename = "HELLO")]` etc.
- All message body structs derive `serde::{Serialize, Deserialize}` with `#[serde(rename_all = "camelCase")]` and `#[derive(Debug, Clone, PartialEq, Eq)]`. Floats use `f64` and only appear in fields that the protocol defines as percentages/measurements.
- `pub struct MediaHeader { pub media_type: MediaType, pub flags: Flags, pub codec: Codec, pub width: u16, pub height: u16, pub seq: u32, pub pts_usec: u64, pub payload_len: u32 }` — magic and reserved bytes are constants, not struct fields.
- `pub const MAGIC: u16 = 0xCC01;` and `pub const HEADER_LEN: usize = 28;` in `media::mod`.
- `pub struct Flags : u8` via `bitflags!` macro with `KEYFRAME=1, ENCODED=2, FULL_RANGE=4, CONFIG=8`.
- `MediaType::Video = 1`. `Codec::Raw = 0, Codec::Hevc = 1, Codec::H264 = 2`.

### Swift (ClearCamProtocol)

- `public let PROTO_VER: UInt32 = 1` (or `enum Protocol { public static let version: UInt32 = 1 }`).
- Property names use camelCase exactly matching the Rust serde JSON output (so JSON round-trip is identical).
- `public struct ControlEnvelope` with custom `Codable` (see Task 10).
- `public enum ControlMessage` with associated values for each variant; custom `Codable` reading/writing the `t` discriminator into the parent container.
- `public struct MediaHeader` with `encode() -> Data` and `static func decode(_:) throws -> MediaHeader`. Same field set as Rust.

---

## Task 1 — Repository conventions

**Files:**
- Create: `.editorconfig`
- Create: `rustfmt.toml`

- [ ] **Step 1: Create `.editorconfig` with project conventions**

Write `.editorconfig`:

```ini
root = true

[*]
end_of_line = lf
charset = utf-8
trim_trailing_whitespace = true
insert_final_newline = true
indent_style = space
indent_size = 2

[*.{rs,swift}]
indent_size = 4

[Makefile]
indent_style = tab

[*.md]
trim_trailing_whitespace = false
```

- [ ] **Step 2: Create `rustfmt.toml`**

Write `rustfmt.toml`:

```toml
edition = "2021"
max_width = 100
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
newline_style = "Unix"
use_field_init_shorthand = true
use_try_shorthand = true
```

- [ ] **Step 3: Commit**

```bash
git add .editorconfig rustfmt.toml
git commit -m "chore: add editor and rustfmt conventions"
```

---

## Task 2 — Rust workspace skeleton

**Files:**
- Create: `desktop/Cargo.toml`
- Create: `desktop/rust-toolchain.toml`
- Create: `desktop/crates/{ccp-protocol,transport,session,adaptive,mediapipeline,decode,sink,app}/Cargo.toml`
- Create: `desktop/crates/{transport,session,adaptive,mediapipeline,decode,sink}/src/lib.rs` (stubs)
- Create: `desktop/crates/ccp-protocol/src/lib.rs` (empty re-export for now; filled in Tasks 3–8)
- Create: `desktop/crates/app/src/main.rs` (stub binary)
- Create: `desktop/sinks/.gitkeep`

- [ ] **Step 1: Create `desktop/rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 2: Create workspace manifest `desktop/Cargo.toml`**

```toml
[workspace]
resolver = "2"
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
]
exclude = ["ui"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
authors = ["ClearCam contributors"]
license = "Apache-2.0 OR MIT"
# repository = "..."  # set once published

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bytes = "1"
byteorder = "1"
bitflags = { version = "2", features = ["serde"] }
thiserror = "1"
anyhow = "1"

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = "symbols"
```

> Note: `src-tauri` is listed as a workspace member but the directory does not yet exist; Cargo errors if a listed member is missing. We add `src-tauri` to the workspace in Task 9. **Until then, remove the `"src-tauri"` line.** Re-add it in Task 9 Step 1.

For Step 2 only, the `members` array should be:

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
]
```

- [ ] **Step 3: Create stub crates**

For each of `transport`, `session`, `adaptive`, `mediapipeline`, `decode`, `sink`, create `desktop/crates/<name>/Cargo.toml`:

```toml
[package]
name = "<name>"   # replace per crate
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "ClearCam <name> — stub crate, see docs/02-architecture.md (filled in later phases)"

[dependencies]
```

And `desktop/crates/<name>/src/lib.rs`:

```rust
//! `<name>` — stub crate for Phase 0.
//!
//! See `docs/02-architecture.md` for the planned responsibilities of this module.
//! Implementation lands in later phases per `docs/10-roadmap-and-plan.md`.

#![forbid(unsafe_code)]
```

For `ccp-protocol`, create `desktop/crates/ccp-protocol/Cargo.toml`:

```toml
[package]
name = "ccp-protocol"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "ClearCam protocol (CCP) — pure data + (de)serialization."

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
bytes = { workspace = true }
byteorder = { workspace = true }
bitflags = { workspace = true }
thiserror = { workspace = true }

[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = "warn"
pedantic = "warn"
```

And `desktop/crates/ccp-protocol/src/lib.rs`:

```rust
//! ClearCam Protocol (CCP) — pure data types and (de)serialization.
//!
//! Wire format spec: `docs/03-protocol.md`. No I/O, no async, no platform code.

#![forbid(unsafe_code)]

// Modules are added in later tasks of Phase 0.
```

For the binary crate `app`, create `desktop/crates/app/Cargo.toml`:

```toml
[package]
name = "clearcam-app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[[bin]]
name = "clearcam"
path = "src/main.rs"
```

And `desktop/crates/app/src/main.rs`:

```rust
//! `clearcam-app` — desktop assembly entry. Phase 0 stub.

fn main() {
    println!("clearcam: phase-0 stub");
}
```

- [ ] **Step 4: Sink platform directory placeholder**

Create `desktop/sinks/.gitkeep` (empty file).

- [ ] **Step 5: Verify the workspace builds and tests pass (no tests yet, but command must succeed)**

```bash
cd desktop && cargo build --workspace
```

Expected: `Compiling … Finished`, no errors. Warnings about unused workspace deps are acceptable in Phase 0; they go away as later tasks consume them.

```bash
cd desktop && cargo test --workspace
```

Expected: `0 passed; 0 failed` for each crate; overall succeeds.

- [ ] **Step 6: Commit**

```bash
git add desktop/
git commit -m "feat(workspace): scaffold Rust workspace with 8 stub crates"
```

---

## Task 3 — `ccp-protocol`: length-prefixed JSON framing

**TDD task.** This implements `control::envelope` per `docs/03-protocol.md` §5.1.

**Files:**
- Create: `desktop/crates/ccp-protocol/src/control/mod.rs`
- Create: `desktop/crates/ccp-protocol/src/control/envelope.rs`
- Test: tests live next to code via `#[cfg(test)] mod tests` (inline).
- Modify: `desktop/crates/ccp-protocol/src/lib.rs`

- [ ] **Step 1: Add module declarations to `lib.rs`**

Append to `desktop/crates/ccp-protocol/src/lib.rs`:

```rust
pub mod control;
```

Create `desktop/crates/ccp-protocol/src/control/mod.rs`:

```rust
//! Control plane: length-prefixed JSON messages.
//!
//! Wire format: `[uint32 BE length][UTF-8 JSON payload]` per `docs/03-protocol.md` §5.1.

pub mod envelope;
```

- [ ] **Step 2: Write the failing tests in `envelope.rs`**

Create `desktop/crates/ccp-protocol/src/control/envelope.rs`:

```rust
//! Length-prefixed framing for control-plane JSON messages.

use thiserror::Error;

/// Default maximum payload size accepted by [`try_unframe`] (1 MiB).
///
/// Control messages are small JSON; this is a sanity cap to prevent
/// memory-exhaustion attacks via crafted length prefixes.
pub const DEFAULT_MAX_PAYLOAD: u32 = 1024 * 1024;

/// Length prefix is 4 bytes (BE u32).
pub const PREFIX_LEN: usize = 4;

/// Errors that can happen while reading a framed payload from a buffer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    /// Declared payload length exceeds the caller's maximum.
    #[error("framed payload length {len} exceeds maximum {max}")]
    TooLarge { len: u32, max: u32 },
}

/// Encode a payload as `[BE u32 length][payload]`.
#[must_use]
pub fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(PREFIX_LEN + payload.len());
    let len = u32::try_from(payload.len()).expect("payload < 4 GiB");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// Try to read one length-prefixed payload from the front of `buf`.
///
/// Returns:
/// - `Ok(Some((payload, frame_total_len)))` if a complete frame is available.
///   `payload` is a slice of `buf`; `frame_total_len = PREFIX_LEN + payload.len()` —
///   the caller should advance its read cursor by this amount.
/// - `Ok(None)` if more bytes are needed (no allocation; safe to call again later).
/// - `Err(FrameError)` if the declared length exceeds `max_payload`.
pub fn try_unframe(buf: &[u8], max_payload: u32) -> Result<Option<(&[u8], usize)>, FrameError> {
    if buf.len() < PREFIX_LEN {
        return Ok(None);
    }
    let mut len_bytes = [0u8; PREFIX_LEN];
    len_bytes.copy_from_slice(&buf[..PREFIX_LEN]);
    let len = u32::from_be_bytes(len_bytes);
    if len > max_payload {
        return Err(FrameError::TooLarge { len, max: max_payload });
    }
    let total = PREFIX_LEN + len as usize;
    if buf.len() < total {
        return Ok(None);
    }
    Ok(Some((&buf[PREFIX_LEN..total], total)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_writes_be_length_prefix() {
        let bytes = frame(b"abc");
        assert_eq!(&bytes[..PREFIX_LEN], &[0, 0, 0, 3]);
        assert_eq!(&bytes[PREFIX_LEN..], b"abc");
    }

    #[test]
    fn frame_empty_payload() {
        let bytes = frame(b"");
        assert_eq!(bytes, vec![0, 0, 0, 0]);
    }

    #[test]
    fn try_unframe_round_trip() {
        let payload = br#"{"t":"HELLO","seq":1}"#;
        let framed = frame(payload);
        let (got, n) = try_unframe(&framed, DEFAULT_MAX_PAYLOAD).unwrap().unwrap();
        assert_eq!(got, &payload[..]);
        assert_eq!(n, framed.len());
    }

    #[test]
    fn try_unframe_returns_none_when_prefix_incomplete() {
        let buf = [0u8, 0, 0];
        assert_eq!(try_unframe(&buf, DEFAULT_MAX_PAYLOAD).unwrap(), None);
    }

    #[test]
    fn try_unframe_returns_none_when_payload_partial() {
        // declared 5 bytes, only 2 available
        let buf = [0u8, 0, 0, 5, b'h', b'e'];
        assert_eq!(try_unframe(&buf, DEFAULT_MAX_PAYLOAD).unwrap(), None);
    }

    #[test]
    fn try_unframe_consumes_only_one_frame() {
        // two concatenated frames; should return only the first
        let first = frame(b"first");
        let second = frame(b"second");
        let mut combined = first.clone();
        combined.extend_from_slice(&second);
        let (payload, n) = try_unframe(&combined, DEFAULT_MAX_PAYLOAD).unwrap().unwrap();
        assert_eq!(payload, b"first");
        assert_eq!(n, first.len());
    }

    #[test]
    fn try_unframe_too_large() {
        let buf = [0xFFu8, 0xFF, 0xFF, 0xFF];
        let err = try_unframe(&buf, 1024).unwrap_err();
        assert_eq!(err, FrameError::TooLarge { len: u32::MAX, max: 1024 });
    }
}
```

- [ ] **Step 3: Run the tests and verify they fail (compilation error: `try_unframe` not yet linked from `lib.rs`)**

```bash
cd desktop && cargo test -p ccp-protocol --lib
```

Expected: tests compile and pass. (Step 2 already shipped the implementation; we verify here.) If you prefer strict TDD, place the implementation behind a stub returning `unimplemented!()` first, watch tests fail, then paste the real impl back in. **Either way you must see tests passing before moving on.**

- [ ] **Step 4: Re-export from `control/mod.rs`**

Replace the contents of `desktop/crates/ccp-protocol/src/control/mod.rs` with:

```rust
//! Control plane: length-prefixed JSON messages.
//!
//! Wire format: `[uint32 BE length][UTF-8 JSON payload]` per `docs/03-protocol.md` §5.1.

pub mod envelope;

pub use envelope::{frame, try_unframe, FrameError, DEFAULT_MAX_PAYLOAD, PREFIX_LEN};
```

- [ ] **Step 5: Re-run tests + fmt + clippy gate**

```bash
cd desktop && cargo fmt --check && cargo clippy -p ccp-protocol -- -D warnings && cargo test -p ccp-protocol --lib
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add desktop/crates/ccp-protocol/
git commit -m "feat(ccp-protocol): add length-prefixed control framing with tests"
```

---

## Task 4 — `ccp-protocol`: version + capability

**TDD task.** Implements `version` per `docs/03-protocol.md` §9.

**Files:**
- Create: `desktop/crates/ccp-protocol/src/version.rs`
- Modify: `desktop/crates/ccp-protocol/src/lib.rs`

- [ ] **Step 1: Write the failing tests + implementation in `version.rs`**

Create `desktop/crates/ccp-protocol/src/version.rs`:

```rust
//! Protocol version and capability negotiation.

use serde::{Deserialize, Serialize};

/// Current CCP protocol version. Bumped on incompatible changes.
pub const PROTO_VER: u32 = 1;

/// A protocol capability advertised in `HELLO`/`HELLO_ACK`.
///
/// Unknown capability strings deserialize to `Capability::Other(_)` so older
/// readers do not break when newer peers advertise new flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    Hevc,
    H264,
    RawNv12,
    Usb3,
    SpeedtestV1,
    Other(String),
}

impl Capability {
    /// Canonical wire string for known capabilities.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Hevc => "hevc",
            Self::H264 => "h264",
            Self::RawNv12 => "raw_nv12",
            Self::Usb3 => "usb3",
            Self::SpeedtestV1 => "speedtest_v1",
            Self::Other(s) => s.as_str(),
        }
    }
}

impl From<&str> for Capability {
    fn from(s: &str) -> Self {
        match s {
            "hevc" => Self::Hevc,
            "h264" => Self::H264,
            "raw_nv12" => Self::RawNv12,
            "usb3" => Self::Usb3,
            "speedtest_v1" => Self::SpeedtestV1,
            other => Self::Other(other.to_string()),
        }
    }
}

impl Serialize for Capability {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(Self::from(s.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proto_ver_is_one() {
        assert_eq!(PROTO_VER, 1);
    }

    #[test]
    fn known_capability_round_trip() {
        for cap in [
            Capability::Hevc,
            Capability::H264,
            Capability::RawNv12,
            Capability::Usb3,
            Capability::SpeedtestV1,
        ] {
            let s = serde_json::to_string(&cap).unwrap();
            let back: Capability = serde_json::from_str(&s).unwrap();
            assert_eq!(back, cap);
        }
    }

    #[test]
    fn unknown_capability_parses_as_other() {
        let cap: Capability = serde_json::from_str(r#""future_codec""#).unwrap();
        assert_eq!(cap, Capability::Other("future_codec".into()));
        // and round-trips
        let s = serde_json::to_string(&cap).unwrap();
        assert_eq!(s, r#""future_codec""#);
    }

    #[test]
    fn capability_in_array_round_trip() {
        let caps = vec![Capability::Hevc, Capability::H264, Capability::Other("x".into())];
        let s = serde_json::to_string(&caps).unwrap();
        assert_eq!(s, r#"["hevc","h264","x"]"#);
        let back: Vec<Capability> = serde_json::from_str(&s).unwrap();
        assert_eq!(back, caps);
    }
}
```

- [ ] **Step 2: Re-export from `lib.rs`**

Replace contents of `desktop/crates/ccp-protocol/src/lib.rs`:

```rust
//! ClearCam Protocol (CCP) — pure data types and (de)serialization.
//!
//! Wire format spec: `docs/03-protocol.md`. No I/O, no async, no platform code.

#![forbid(unsafe_code)]

pub mod control;
pub mod version;

pub use version::{Capability, PROTO_VER};
```

- [ ] **Step 3: Run tests + lint gate**

```bash
cd desktop && cargo fmt --check && cargo clippy -p ccp-protocol -- -D warnings && cargo test -p ccp-protocol --lib
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add desktop/crates/ccp-protocol/
git commit -m "feat(ccp-protocol): add PROTO_VER and Capability with forward-compat Other()"
```

---

## Task 5 — `ccp-protocol`: handshake messages + `ControlEnvelope` + `ControlMessage` enum (handshake variants only)

**TDD task.** Adds `handshake.rs` and the discriminated-union machinery. Per `docs/03-protocol.md` §6.1.

**Files:**
- Create: `desktop/crates/ccp-protocol/src/control/handshake.rs`
- Modify: `desktop/crates/ccp-protocol/src/control/mod.rs`

- [ ] **Step 1: Write the handshake message bodies + ErrorCode**

Create `desktop/crates/ccp-protocol/src/control/handshake.rs`:

```rust
//! Handshake & error messages (§6.1).

use serde::{Deserialize, Serialize};

use crate::version::Capability;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdent {
    pub model: String,
    pub os_ver: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub proto_ver: u32,
    pub app: String,
    pub device: DeviceIdent,
    pub session_id: String,
    pub caps: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAck {
    pub proto_ver: u32,
    pub caps: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auth {
    /// Either a fresh QR token (Wi-Fi) or a stored pairing key (USB).
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthOk {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaHello {
    pub session_id: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bye {
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    IncompatibleVersion,
    Unauthorized,
    BadRequest,
    UnsupportedMode,
    CameraUnavailable,
    Internal,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMsg {
    pub code: ErrorCode,
    pub message: String,
    /// `seq` of the request this error replies to, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_round_trip() {
        let hello = Hello {
            proto_ver: 1,
            app: "ClearCam-iOS/0.1.0".into(),
            device: DeviceIdent {
                model: "iPhone15,3".into(),
                os_ver: "iOS 18.0".into(),
            },
            session_id: "sess-abc".into(),
            caps: vec![Capability::Hevc, Capability::RawNv12],
        };
        let s = serde_json::to_string(&hello).unwrap();
        let back: Hello = serde_json::from_str(&s).unwrap();
        assert_eq!(back, hello);

        // Field name check (camelCase)
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["protoVer"], 1);
        assert_eq!(v["sessionId"], "sess-abc");
        assert_eq!(v["device"]["osVer"], "iOS 18.0");
    }

    #[test]
    fn error_omits_ack_when_none() {
        let e = ErrorMsg {
            code: ErrorCode::Unauthorized,
            message: "bad token".into(),
            ack: None,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(!s.contains("\"ack\""));
    }

    #[test]
    fn error_code_snake_case() {
        let s = serde_json::to_string(&ErrorCode::IncompatibleVersion).unwrap();
        assert_eq!(s, "\"incompatible_version\"");
    }
}
```

- [ ] **Step 2: Add `ControlEnvelope` and the discriminated union in `control/mod.rs`**

Replace `desktop/crates/ccp-protocol/src/control/mod.rs`:

```rust
//! Control plane: length-prefixed JSON messages.
//!
//! Wire format: `[uint32 BE length][UTF-8 JSON payload]` per `docs/03-protocol.md` §5.1.
//!
//! Each JSON object has a `t` (type) discriminator, a `seq` counter, and an optional
//! `ack` referencing the request `seq`.

pub mod envelope;
pub mod handshake;

use serde::{Deserialize, Serialize};

pub use envelope::{frame, try_unframe, FrameError, DEFAULT_MAX_PAYLOAD, PREFIX_LEN};
pub use handshake::{
    Auth, AuthOk, Bye, DeviceIdent, ErrorCode, ErrorMsg, Hello, HelloAck, MediaHello,
};

/// Wire-level wrapper carrying `seq`, optional `ack`, and a typed body.
///
/// JSON shape (flattened): `{ "seq": .., "ack": .., "t": "...", ... body fields ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlEnvelope {
    pub seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<u64>,
    #[serde(flatten)]
    pub body: ControlMessage,
}

/// Internally-tagged discriminated union over all CCP control messages.
///
/// Tag values mirror the wire strings in `docs/03-protocol.md` §6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum ControlMessage {
    #[serde(rename = "HELLO")] Hello(Hello),
    #[serde(rename = "HELLO_ACK")] HelloAck(HelloAck),
    #[serde(rename = "AUTH")] Auth(Auth),
    #[serde(rename = "AUTH_OK")] AuthOk(AuthOk),
    #[serde(rename = "MEDIA_HELLO")] MediaHello(MediaHello),
    #[serde(rename = "BYE")] Bye(Bye),
    #[serde(rename = "ERROR")] Error(ErrorMsg),
    // Additional variants added in Tasks 6–7.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Capability;

    #[test]
    fn envelope_hello_round_trip_and_shape() {
        let env = ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: 1,
                app: "ClearCam-iOS/0.1.0".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "iOS 18.0".into(),
                },
                session_id: "sess-abc".into(),
                caps: vec![Capability::Hevc],
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["seq"], 1);
        assert_eq!(v["t"], "HELLO");
        assert_eq!(v["protoVer"], 1);
        assert!(v.get("ack").is_none());
        let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn envelope_error_includes_ack_when_set() {
        let env = ControlEnvelope {
            seq: 100,
            ack: Some(99),
            body: ControlMessage::Error(ErrorMsg {
                code: ErrorCode::BadRequest,
                message: "oops".into(),
                ack: None,
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["ack"], 99);
        assert_eq!(v["t"], "ERROR");
        assert_eq!(v["code"], "bad_request");
    }
}
```

- [ ] **Step 3: Run tests + lints**

```bash
cd desktop && cargo fmt --check && cargo clippy -p ccp-protocol -- -D warnings && cargo test -p ccp-protocol --lib
```

Expected: all green. Pay attention to any serde error about flatten + internally-tagged enum interaction; if it complains, ensure `ControlMessage` has `#[serde(tag = "t")]` and `body` is `#[serde(flatten)]` on the envelope — that combination is supported.

- [ ] **Step 4: Commit**

```bash
git add desktop/crates/ccp-protocol/
git commit -m "feat(ccp-protocol): handshake messages + envelope + ControlMessage enum"
```

---

## Task 6 — `ccp-protocol`: device, streaming, telemetry messages

**TDD task.** Per `docs/03-protocol.md` §6.2, §6.3, §6.4.

**Files:**
- Create: `desktop/crates/ccp-protocol/src/control/device.rs`
- Create: `desktop/crates/ccp-protocol/src/control/streaming.rs`
- Create: `desktop/crates/ccp-protocol/src/control/telemetry.rs`
- Modify: `desktop/crates/ccp-protocol/src/control/mod.rs`

- [ ] **Step 1: `device.rs`**

```rust
//! Device & camera messages (§6.2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    Unknown,
    Unplugged,
    Charging,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub model: String,
    pub os_ver: String,
    /// 0.0..=1.0
    pub battery_level: f64,
    pub battery_state: BatteryState,
    pub thermal_state: ThermalState,
    pub usb3_capable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraPosition {
    Front,
    Back,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraEntry {
    pub id: String,
    pub name: String,
    pub position: CameraPosition,
    pub max_width: u16,
    pub max_height: u16,
    pub max_fps: u16,
    /// Supported pixel formats as wire strings (`"nv12"`, `"hevc"`, `"h264"`).
    pub supported_formats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraList {
    pub cameras: Vec<CameraEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCamera {
    pub camera_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraState {
    pub active_camera_id: String,
    /// Mode string, e.g. `"1920x1080@30 hevc"`. Free-form for diagnostics.
    pub applied_format: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_info_round_trip() {
        let d = DeviceInfo {
            model: "iPhone15,3".into(),
            os_ver: "iOS 18.0".into(),
            battery_level: 0.82,
            battery_state: BatteryState::Unplugged,
            thermal_state: ThermalState::Nominal,
            usb3_capable: true,
        };
        let s = serde_json::to_string(&d).unwrap();
        let back: DeviceInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(back, d);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["batteryLevel"], 0.82);
        assert_eq!(v["batteryState"], "unplugged");
        assert_eq!(v["usb3Capable"], true);
    }

    #[test]
    fn camera_list_round_trip() {
        let list = CameraList {
            cameras: vec![CameraEntry {
                id: "wide".into(),
                name: "Wide".into(),
                position: CameraPosition::Back,
                max_width: 3840,
                max_height: 2160,
                max_fps: 60,
                supported_formats: vec!["nv12".into(), "hevc".into(), "h264".into()],
            }],
        };
        let s = serde_json::to_string(&list).unwrap();
        let back: CameraList = serde_json::from_str(&s).unwrap();
        assert_eq!(back, list);
    }
}
```

- [ ] **Step 2: `streaming.rs`**

```rust
//! Streaming control & mode (§6.3 + §7).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatKind {
    Raw,
    Encoded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecName {
    None,
    Hevc,
    H264,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    Nv12,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    pub format: FormatKind,
    pub codec: CodecName,
    pub width: u16,
    pub height: u16,
    pub fps: u16,
    /// For `Encoded`; ignored for `Raw`.
    pub bitrate_kbps: u32,
    pub pixel_format: PixelFormat,
    pub full_range: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    pub mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stop {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMode {
    pub mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeApplied {
    pub mode: Mode,
    /// First media `seq` produced under the new mode.
    pub at_seq: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mode() -> Mode {
        Mode {
            format: FormatKind::Encoded,
            codec: CodecName::Hevc,
            width: 1920,
            height: 1080,
            fps: 30,
            bitrate_kbps: 30_000,
            pixel_format: PixelFormat::Nv12,
            full_range: true,
        }
    }

    #[test]
    fn mode_round_trip() {
        let m = sample_mode();
        let s = serde_json::to_string(&m).unwrap();
        let back: Mode = serde_json::from_str(&s).unwrap();
        assert_eq!(back, m);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["format"], "encoded");
        assert_eq!(v["codec"], "hevc");
        assert_eq!(v["bitrateKbps"], 30_000);
        assert_eq!(v["pixelFormat"], "nv12");
        assert_eq!(v["fullRange"], true);
    }

    #[test]
    fn mode_applied_round_trip() {
        let ma = ModeApplied { mode: sample_mode(), at_seq: 4242 };
        let s = serde_json::to_string(&ma).unwrap();
        let back: ModeApplied = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ma);
    }
}
```

- [ ] **Step 3: `telemetry.rs`**

```rust
//! Telemetry, ping/pong (§6.4).

use serde::{Deserialize, Serialize};

use crate::control::device::{BatteryState, ThermalState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    /// Device monotonic clock, microseconds.
    pub ts_usec: u64,
    pub battery_level: f64,
    pub battery_state: BatteryState,
    pub thermal_state: ThermalState,
    pub sent_bitrate_kbps: u32,
    pub enc_fps: u32,
    pub capture_fps: u32,
    pub queue_depth: u32,
    pub drop_count: u64,
}

// Telemetry contains `f64` so it cannot be `Eq`; only `PartialEq`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ping {
    pub ts_usec: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pong {
    pub ts_usec: u64,
    pub echo_usec: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_round_trip() {
        let t = Telemetry {
            ts_usec: 1_000_000,
            battery_level: 0.5,
            battery_state: BatteryState::Charging,
            thermal_state: ThermalState::Fair,
            sent_bitrate_kbps: 25_000,
            enc_fps: 30,
            capture_fps: 30,
            queue_depth: 1,
            drop_count: 2,
        };
        let s = serde_json::to_string(&t).unwrap();
        let back: Telemetry = serde_json::from_str(&s).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn ping_pong_round_trip() {
        let p = Ping { ts_usec: 12345 };
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Ping>(&s).unwrap(), p);

        let q = Pong { ts_usec: 23456, echo_usec: 12345 };
        let s = serde_json::to_string(&q).unwrap();
        assert_eq!(serde_json::from_str::<Pong>(&s).unwrap(), q);
    }
}
```

- [ ] **Step 4: Extend `ControlMessage` enum**

In `desktop/crates/ccp-protocol/src/control/mod.rs`, update the `pub mod` declarations:

```rust
pub mod device;
pub mod streaming;
pub mod telemetry;
```

Then re-exports:

```rust
pub use device::{
    BatteryState, CameraEntry, CameraList, CameraPosition, CameraState, DeviceInfo, SetCamera,
    ThermalState,
};
pub use streaming::{CodecName, FormatKind, Mode, ModeApplied, PixelFormat, SetMode, Start, Stop};
pub use telemetry::{Ping, Pong, Telemetry};
```

Add variants to `ControlMessage`:

```rust
#[serde(rename = "DEVICE_INFO")] DeviceInfo(DeviceInfo),
#[serde(rename = "CAMERA_LIST")] CameraList(CameraList),
#[serde(rename = "SET_CAMERA")] SetCamera(SetCamera),
#[serde(rename = "CAMERA_STATE")] CameraState(CameraState),
#[serde(rename = "START")] Start(Start),
#[serde(rename = "STOP")] Stop(Stop),
#[serde(rename = "SET_MODE")] SetMode(SetMode),
#[serde(rename = "MODE_APPLIED")] ModeApplied(ModeApplied),
#[serde(rename = "TELEMETRY")] Telemetry(Telemetry),
#[serde(rename = "PING")] Ping(Ping),
#[serde(rename = "PONG")] Pong(Pong),
```

Note: because `Telemetry` contains `f64`, `ControlMessage` cannot derive `Eq`. **Remove `Eq` from the `ControlMessage` derive** (keep `PartialEq`). Likewise drop `Eq` from `ControlEnvelope` derive. Update any test that compared with `assert_eq!` to use `PartialEq` (no change needed; `assert_eq!` uses `PartialEq`).

- [ ] **Step 5: Cross-variant envelope test**

Add to `control/mod.rs` tests module:

```rust
#[test]
fn envelope_set_camera_round_trip() {
    let env = ControlEnvelope {
        seq: 42,
        ack: None,
        body: ControlMessage::SetCamera(SetCamera { camera_id: "wide".into() }),
    };
    let s = serde_json::to_string(&env).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["t"], "SET_CAMERA");
    assert_eq!(v["cameraId"], "wide");
    let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
    assert_eq!(back, env);
}

#[test]
fn envelope_telemetry_round_trip() {
    let env = ControlEnvelope {
        seq: 7,
        ack: None,
        body: ControlMessage::Telemetry(Telemetry {
            ts_usec: 1,
            battery_level: 1.0,
            battery_state: BatteryState::Full,
            thermal_state: ThermalState::Nominal,
            sent_bitrate_kbps: 0,
            enc_fps: 0,
            capture_fps: 0,
            queue_depth: 0,
            drop_count: 0,
        }),
    };
    let s = serde_json::to_string(&env).unwrap();
    let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
    assert_eq!(back, env);
}
```

- [ ] **Step 6: Lint + test gate**

```bash
cd desktop && cargo fmt --check && cargo clippy -p ccp-protocol -- -D warnings && cargo test -p ccp-protocol --lib
```

Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add desktop/crates/ccp-protocol/
git commit -m "feat(ccp-protocol): device + streaming + telemetry messages"
```

---

## Task 7 — `ccp-protocol`: speedtest messages + final ControlMessage enum

**TDD task.** Per `docs/03-protocol.md` §6.5.

**Files:**
- Create: `desktop/crates/ccp-protocol/src/control/speedtest.rs`
- Modify: `desktop/crates/ccp-protocol/src/control/mod.rs`

- [ ] **Step 1: `speedtest.rs`**

```rust
//! Speedtest messages (§6.5).

use serde::{Deserialize, Serialize};

use crate::control::streaming::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestPattern {
    /// Step pattern: short increasing bitrate ramps.
    Ramp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestStart {
    pub id: String,
    pub target_bitrate_kbps: u32,
    pub duration_ms: u32,
    pub pattern: SpeedtestPattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestTick {
    pub id_seq: u32,
    pub seq: u32,
    pub ts_usec: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestResult {
    pub id: String,
    pub goodput_mbps: f64,
    pub rtt_ms: f64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub recommended_mode: Mode,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::streaming::{CodecName, FormatKind, PixelFormat};

    #[test]
    fn speedtest_start_round_trip() {
        let s = SpeedtestStart {
            id: "st-1".into(),
            target_bitrate_kbps: 800_000,
            duration_ms: 3000,
            pattern: SpeedtestPattern::Ramp,
        };
        let j = serde_json::to_string(&s).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["targetBitrateKbps"], 800_000);
        assert_eq!(v["pattern"], "ramp");
        assert_eq!(serde_json::from_str::<SpeedtestStart>(&j).unwrap(), s);
    }

    #[test]
    fn speedtest_result_round_trip() {
        let r = SpeedtestResult {
            id: "st-1".into(),
            goodput_mbps: 412.5,
            rtt_ms: 8.2,
            jitter_ms: 1.3,
            loss_pct: 0.0,
            recommended_mode: Mode {
                format: FormatKind::Encoded,
                codec: CodecName::Hevc,
                width: 1920, height: 1080, fps: 30,
                bitrate_kbps: 30_000,
                pixel_format: PixelFormat::Nv12,
                full_range: true,
            },
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: SpeedtestResult = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
    }
}
```

- [ ] **Step 2: Extend `ControlMessage` enum (final)**

Add to `control/mod.rs`:

```rust
pub mod speedtest;

pub use speedtest::{SpeedtestPattern, SpeedtestResult, SpeedtestStart, SpeedtestTick};
```

And add variants:

```rust
#[serde(rename = "SPEEDTEST_START")] SpeedtestStart(SpeedtestStart),
#[serde(rename = "SPEEDTEST_TICK")] SpeedtestTick(SpeedtestTick),
#[serde(rename = "SPEEDTEST_RESULT")] SpeedtestResult(SpeedtestResult),
```

- [ ] **Step 3: Add a comprehensive variant-coverage test**

Add to `control/mod.rs` tests module:

```rust
#[test]
fn all_variants_round_trip_through_envelope() {
    use crate::control::streaming::{CodecName, FormatKind, PixelFormat};
    use crate::Capability;

    let mode = Mode {
        format: FormatKind::Raw,
        codec: CodecName::None,
        width: 1280, height: 720, fps: 30,
        bitrate_kbps: 0,
        pixel_format: PixelFormat::Nv12,
        full_range: false,
    };

    let cases: Vec<ControlMessage> = vec![
        ControlMessage::Hello(Hello {
            proto_ver: 1, app: "x".into(),
            device: DeviceIdent { model: "m".into(), os_ver: "v".into() },
            session_id: "s".into(),
            caps: vec![Capability::Hevc],
        }),
        ControlMessage::HelloAck(HelloAck { proto_ver: 1, caps: vec![] }),
        ControlMessage::Auth(Auth { token: "t".into() }),
        ControlMessage::AuthOk(AuthOk { session_id: "s".into() }),
        ControlMessage::MediaHello(MediaHello { session_id: "s".into(), token: "t".into() }),
        ControlMessage::Bye(Bye { reason: "done".into() }),
        ControlMessage::Error(ErrorMsg {
            code: ErrorCode::Timeout, message: "x".into(), ack: Some(1),
        }),
        ControlMessage::DeviceInfo(DeviceInfo {
            model: "m".into(), os_ver: "v".into(),
            battery_level: 0.5,
            battery_state: BatteryState::Charging,
            thermal_state: ThermalState::Nominal,
            usb3_capable: false,
        }),
        ControlMessage::CameraList(CameraList { cameras: vec![] }),
        ControlMessage::SetCamera(SetCamera { camera_id: "wide".into() }),
        ControlMessage::CameraState(CameraState {
            active_camera_id: "wide".into(), applied_format: "1080p30 hevc".into(),
        }),
        ControlMessage::Start(Start { mode: mode.clone() }),
        ControlMessage::Stop(Stop {}),
        ControlMessage::SetMode(SetMode { mode: mode.clone() }),
        ControlMessage::ModeApplied(ModeApplied { mode: mode.clone(), at_seq: 0 }),
        ControlMessage::Telemetry(Telemetry {
            ts_usec: 0, battery_level: 0.0,
            battery_state: BatteryState::Unknown,
            thermal_state: ThermalState::Nominal,
            sent_bitrate_kbps: 0, enc_fps: 0, capture_fps: 0,
            queue_depth: 0, drop_count: 0,
        }),
        ControlMessage::Ping(Ping { ts_usec: 0 }),
        ControlMessage::Pong(Pong { ts_usec: 0, echo_usec: 0 }),
        ControlMessage::SpeedtestStart(SpeedtestStart {
            id: "x".into(), target_bitrate_kbps: 0, duration_ms: 0,
            pattern: SpeedtestPattern::Ramp,
        }),
        ControlMessage::SpeedtestTick(SpeedtestTick { id_seq: 0, seq: 0, ts_usec: 0 }),
        ControlMessage::SpeedtestResult(SpeedtestResult {
            id: "x".into(),
            goodput_mbps: 0.0, rtt_ms: 0.0, jitter_ms: 0.0, loss_pct: 0.0,
            recommended_mode: mode,
        }),
    ];

    for body in cases {
        let env = ControlEnvelope { seq: 1, ack: None, body: body.clone() };
        let s = serde_json::to_string(&env).unwrap();
        let back: ControlEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back, env, "round-trip failed for {body:?}");
    }
}
```

- [ ] **Step 4: Lint + test gate**

```bash
cd desktop && cargo fmt --check && cargo clippy -p ccp-protocol -- -D warnings && cargo test -p ccp-protocol --lib
```

Expected: all green; the variant-coverage test must list every variant of `ControlMessage`. If you add a variant later, this test fails to compile — that's intentional.

- [ ] **Step 5: Commit**

```bash
git add desktop/crates/ccp-protocol/
git commit -m "feat(ccp-protocol): speedtest messages + comprehensive variant round-trip"
```

---

## Task 8 — `ccp-protocol`: media header binary codec

**TDD task.** Per `docs/03-protocol.md` §5.2.

**Files:**
- Create: `desktop/crates/ccp-protocol/src/media/mod.rs`
- Modify: `desktop/crates/ccp-protocol/src/lib.rs`

- [ ] **Step 1: Add module declaration**

In `desktop/crates/ccp-protocol/src/lib.rs` add:

```rust
pub mod media;

pub use media::{Codec, DecodeError as MediaDecodeError, Flags, MediaHeader, MediaType, HEADER_LEN, MAGIC};
```

- [ ] **Step 2: Write the failing tests + implementation**

Create `desktop/crates/ccp-protocol/src/media/mod.rs`:

```rust
//! Binary media header (28 bytes, big-endian).
//!
//! Spec: `docs/03-protocol.md` §5.2.

use bitflags::bitflags;
use thiserror::Error;

/// Magic marker (also identifies media-format version).
pub const MAGIC: u16 = 0xCC01;

/// Fixed header length in bytes.
pub const HEADER_LEN: usize = 28;

bitflags! {
    /// Header flags (low byte).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u8 {
        /// Frame is a keyframe / random-access point.
        const KEYFRAME   = 1 << 0;
        /// Payload is an encoded access unit (vs raw).
        const ENCODED    = 1 << 1;
        /// YUV uses full-range sampling (vs video-range).
        const FULL_RANGE = 1 << 2;
        /// Payload contains codec configuration (e.g., VPS/SPS/PPS).
        const CONFIG     = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MediaType {
    Video = 1,
    // 2 = audio reserved (see §10)
}

impl TryFrom<u8> for MediaType {
    type Error = DecodeError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(Self::Video),
            other => Err(DecodeError::UnknownMediaType(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Codec {
    Raw = 0,
    Hevc = 1,
    H264 = 2,
}

impl TryFrom<u8> for Codec {
    type Error = DecodeError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Raw),
            1 => Ok(Self::Hevc),
            2 => Ok(Self::H264),
            other => Err(DecodeError::UnknownCodec(other)),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("buffer too short for media header (need {HEADER_LEN}, got {got})")]
    TooShort { got: usize },
    #[error("bad magic 0x{0:04X}, expected 0x{:04X}", MAGIC)]
    BadMagic(u16),
    #[error("unknown media type {0}")]
    UnknownMediaType(u8),
    #[error("unknown codec {0}")]
    UnknownCodec(u8),
}

/// The 28-byte fixed binary header carried before every media payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaHeader {
    pub media_type: MediaType,
    pub flags: Flags,
    pub codec: Codec,
    pub width: u16,
    pub height: u16,
    pub seq: u32,
    pub pts_usec: u64,
    pub payload_len: u32,
}

impl MediaHeader {
    /// Encode the header as 28 big-endian bytes. `magic` and the 3 reserved
    /// bytes are written automatically.
    #[must_use]
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..2].copy_from_slice(&MAGIC.to_be_bytes());
        out[2] = self.media_type as u8;
        out[3] = self.flags.bits();
        out[4] = self.codec as u8;
        // out[5..8] = reserved (already zero)
        out[8..10].copy_from_slice(&self.width.to_be_bytes());
        out[10..12].copy_from_slice(&self.height.to_be_bytes());
        out[12..16].copy_from_slice(&self.seq.to_be_bytes());
        out[16..24].copy_from_slice(&self.pts_usec.to_be_bytes());
        out[24..28].copy_from_slice(&self.payload_len.to_be_bytes());
        out
    }

    /// Decode from a buffer of at least [`HEADER_LEN`] bytes. Reserved bytes
    /// are ignored (forward compatibility).
    pub fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < HEADER_LEN {
            return Err(DecodeError::TooShort { got: buf.len() });
        }
        let magic = u16::from_be_bytes([buf[0], buf[1]]);
        if magic != MAGIC {
            return Err(DecodeError::BadMagic(magic));
        }
        let media_type = MediaType::try_from(buf[2])?;
        let flags = Flags::from_bits_truncate(buf[3]);
        let codec = Codec::try_from(buf[4])?;
        // ignore buf[5..8] (reserved)
        let width = u16::from_be_bytes([buf[8], buf[9]]);
        let height = u16::from_be_bytes([buf[10], buf[11]]);
        let seq = u32::from_be_bytes(buf[12..16].try_into().unwrap());
        let pts_usec = u64::from_be_bytes(buf[16..24].try_into().unwrap());
        let payload_len = u32::from_be_bytes(buf[24..28].try_into().unwrap());
        Ok(Self { media_type, flags, codec, width, height, seq, pts_usec, payload_len })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MediaHeader {
        MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::KEYFRAME | Flags::ENCODED | Flags::FULL_RANGE,
            codec: Codec::Hevc,
            width: 1920,
            height: 1080,
            seq: 42,
            pts_usec: 1_234_567_890,
            payload_len: 12345,
        }
    }

    #[test]
    fn known_bytes_encode() {
        let h = sample();
        let bytes = h.encode();
        let expected: [u8; HEADER_LEN] = [
            0xCC, 0x01,             // magic
            0x01,                   // media type = video
            0x07,                   // flags = keyframe | encoded | fullRange
            0x01,                   // codec = HEVC
            0x00, 0x00, 0x00,       // reserved[3]
            0x07, 0x80,             // width 1920
            0x04, 0x38,             // height 1080
            0x00, 0x00, 0x00, 0x2A, // seq 42
            0x00, 0x00, 0x00, 0x00, 0x49, 0x96, 0x02, 0xD2, // ptsUsec
            0x00, 0x00, 0x30, 0x39, // payloadLen 12345
        ];
        assert_eq!(bytes, expected);
    }

    #[test]
    fn round_trip() {
        let h = sample();
        let back = MediaHeader::decode(&h.encode()).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn round_trip_zero() {
        let h = MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::empty(),
            codec: Codec::Raw,
            width: 0, height: 0, seq: 0, pts_usec: 0, payload_len: 0,
        };
        let back = MediaHeader::decode(&h.encode()).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn too_short() {
        let err = MediaHeader::decode(&[0u8; HEADER_LEN - 1]).unwrap_err();
        assert_eq!(err, DecodeError::TooShort { got: HEADER_LEN - 1 });
    }

    #[test]
    fn bad_magic() {
        let mut buf = sample().encode();
        buf[0] = 0;
        let err = MediaHeader::decode(&buf).unwrap_err();
        assert!(matches!(err, DecodeError::BadMagic(_)));
    }

    #[test]
    fn unknown_codec() {
        let mut buf = sample().encode();
        buf[4] = 99;
        let err = MediaHeader::decode(&buf).unwrap_err();
        assert_eq!(err, DecodeError::UnknownCodec(99));
    }

    #[test]
    fn unknown_media_type() {
        let mut buf = sample().encode();
        buf[2] = 99;
        let err = MediaHeader::decode(&buf).unwrap_err();
        assert_eq!(err, DecodeError::UnknownMediaType(99));
    }

    #[test]
    fn reserved_bytes_ignored_on_decode() {
        let mut buf = sample().encode();
        buf[5] = 0xAB;
        buf[6] = 0xCD;
        buf[7] = 0xEF;
        let back = MediaHeader::decode(&buf).unwrap();
        assert_eq!(back, sample());
    }

    #[test]
    fn flags_unknown_bits_truncated_on_decode() {
        let mut buf = sample().encode();
        buf[3] = 0xFF; // all bits set, including unknown ones
        let back = MediaHeader::decode(&buf).unwrap();
        // only defined bits should round-trip
        assert_eq!(
            back.flags,
            Flags::KEYFRAME | Flags::ENCODED | Flags::FULL_RANGE | Flags::CONFIG
        );
    }
}
```

- [ ] **Step 3: Run tests + lint gate**

```bash
cd desktop && cargo fmt --check && cargo clippy -p ccp-protocol -- -D warnings && cargo test -p ccp-protocol --lib
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add desktop/crates/ccp-protocol/
git commit -m "feat(ccp-protocol): binary media header codec with known-byte and error tests"
```

---

## Task 9 — Tauri 2 scaffold + React/Vite/TS/Tailwind UI shell

**Files:** see "Files" lines below.

- [ ] **Step 1: Add `src-tauri` to the workspace**

Edit `desktop/Cargo.toml` `members`, adding `"src-tauri"`. Final list:

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
]
```

- [ ] **Step 2: Create `desktop/src-tauri/Cargo.toml`**

```toml
[package]
name = "clearcam-desktop"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "ClearCam Tauri shell (Phase 0)"

[lib]
name = "clearcam_desktop_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 3: Create `desktop/src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4: Create `desktop/src-tauri/src/lib.rs`**

```rust
//! ClearCam desktop application shell.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running ClearCam application");
}
```

- [ ] **Step 5: Create `desktop/src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    clearcam_desktop_lib::run();
}
```

- [ ] **Step 6: Create `desktop/src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ClearCam",
  "version": "0.1.0",
  "identifier": "app.clearcam.desktop",
  "build": {
    "beforeDevCommand": "pnpm --dir ../ui dev",
    "beforeBuildCommand": "pnpm --dir ../ui build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../ui/dist"
  },
  "app": {
    "windows": [
      {
        "title": "ClearCam",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png"]
  }
}
```

- [ ] **Step 7: Create `desktop/src-tauri/capabilities/default.json`**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for the main window",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 8: Create a placeholder icon**

```bash
mkdir -p desktop/src-tauri/icons
# 1x1 transparent PNG (98 bytes); Tauri requires *some* icon file
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\rIDATx\x9cc\xfc\xcf\xc0\xc0\xc0\x00\x00\x00\x05\x00\x01\xb5\xfa\xf6\xc8\x00\x00\x00\x00IEND\xaeB`\x82' > desktop/src-tauri/icons/icon.png
```

> A proper icon set (ico, icns, multiple png sizes) is added in Phase 6 polish. Phase 0 only needs the build to succeed.

- [ ] **Step 9: Create the UI shell**

`desktop/ui/package.json`:

```json
{
  "name": "@clearcam/ui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "lint": "eslint . --ext ts,tsx --report-unused-disable-directives --max-warnings 0",
    "format:check": "prettier --check \"src/**/*.{ts,tsx,css}\" \"*.{ts,js,json,html}\"",
    "format:write": "prettier --write \"src/**/*.{ts,tsx,css}\" \"*.{ts,js,json,html}\"",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@types/react": "^18.3.0",
    "@types/react-dom": "^18.3.0",
    "@typescript-eslint/eslint-plugin": "^7.0.0",
    "@typescript-eslint/parser": "^7.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "autoprefixer": "^10.4.0",
    "eslint": "^8.57.0",
    "eslint-plugin-react-hooks": "^4.6.0",
    "eslint-plugin-react-refresh": "^0.4.0",
    "postcss": "^8.4.0",
    "prettier": "^3.3.0",
    "tailwindcss": "^3.4.0",
    "typescript": "^5.5.0",
    "vite": "^5.4.0"
  }
}
```

`desktop/ui/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ClearCam</title>
  </head>
  <body class="bg-neutral-950 text-neutral-100">
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`desktop/ui/vite.config.ts`:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
```

`desktop/ui/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "useDefineForClassFields": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

`desktop/ui/tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true
  },
  "include": ["vite.config.ts"]
}
```

`desktop/ui/tailwind.config.js`:

```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: { extend: {} },
  plugins: [],
};
```

`desktop/ui/postcss.config.js`:

```js
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

`desktop/ui/.prettierrc.json`:

```json
{
  "semi": true,
  "singleQuote": false,
  "trailingComma": "all",
  "printWidth": 100
}
```

`desktop/ui/.eslintrc.cjs`:

```js
module.exports = {
  root: true,
  env: { browser: true, es2022: true },
  extends: [
    "eslint:recommended",
    "plugin:@typescript-eslint/recommended",
    "plugin:react-hooks/recommended",
  ],
  ignorePatterns: ["dist", ".eslintrc.cjs"],
  parser: "@typescript-eslint/parser",
  plugins: ["react-refresh"],
  rules: {
    "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
  },
};
```

`desktop/ui/src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

`desktop/ui/src/App.tsx`:

```tsx
export default function App() {
  return (
    <main className="min-h-screen flex items-center justify-center">
      <div className="text-center space-y-2">
        <h1 className="text-3xl font-semibold">ClearCam</h1>
        <p className="text-neutral-400">Phase 0 — foundation</p>
      </div>
    </main>
  );
}
```

`desktop/ui/src/index.css`:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

html,
body,
#root {
  height: 100%;
}
```

`desktop/ui/src/vite-env.d.ts`:

```ts
/// <reference types="vite/client" />
```

- [ ] **Step 10: Install UI deps and verify UI builds standalone**

```bash
cd desktop/ui && pnpm install --frozen-lockfile=false && pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
```

Expected: all commands succeed. `dist/` is created.

- [ ] **Step 11: Verify Tauri compiles**

```bash
cd desktop && cargo build -p clearcam-desktop
```

Expected: compiles. (No `cargo tauri dev` smoke yet — that requires GUI; a human runs it locally per the verification task.)

- [ ] **Step 12: Commit**

```bash
git add desktop/Cargo.toml desktop/src-tauri/ desktop/ui/
git commit -m "feat(desktop): Tauri 2 scaffold + React/Vite/TS/Tailwind UI shell"
```

---

## Task 10 — iOS `Package.swift` + `ClearCamProtocol` module

**Files:** see "Files" lines below.

- [ ] **Step 1: `ios/Package.swift`**

```swift
// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "ClearCam",
    platforms: [
        .iOS(.v17),
        .macOS(.v13),
    ],
    products: [
        .library(name: "ClearCamProtocol", targets: ["ClearCamProtocol"]),
    ],
    targets: [
        .target(
            name: "ClearCamProtocol",
            path: "Sources/ClearCamProtocol"
        ),
        .testTarget(
            name: "ClearCamProtocolTests",
            dependencies: ["ClearCamProtocol"],
            path: "Tests/ClearCamProtocolTests"
        ),
    ]
)
```

- [ ] **Step 2: `Version.swift`**

```swift
import Foundation

public enum ClearCamProtocolVersion {
    /// Current CCP protocol version.
    public static let current: UInt32 = 1
}

public enum Capability: Hashable {
    case hevc
    case h264
    case rawNv12
    case usb3
    case speedtestV1
    case other(String)

    public var wireString: String {
        switch self {
        case .hevc: return "hevc"
        case .h264: return "h264"
        case .rawNv12: return "raw_nv12"
        case .usb3: return "usb3"
        case .speedtestV1: return "speedtest_v1"
        case .other(let s): return s
        }
    }

    public init(wireString s: String) {
        switch s {
        case "hevc": self = .hevc
        case "h264": self = .h264
        case "raw_nv12": self = .rawNv12
        case "usb3": self = .usb3
        case "speedtest_v1": self = .speedtestV1
        default: self = .other(s)
        }
    }
}

extension Capability: Codable {
    public init(from decoder: Decoder) throws {
        let s = try decoder.singleValueContainer().decode(String.self)
        self = Capability(wireString: s)
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode(wireString)
    }
}
```

- [ ] **Step 3: `MediaHeader.swift`** (full binary codec mirroring Rust)

```swift
import Foundation

public enum MediaConstants {
    public static let magic: UInt16 = 0xCC01
    public static let headerLen: Int = 28
}

public struct MediaFlags: OptionSet, Hashable {
    public let rawValue: UInt8
    public init(rawValue: UInt8) { self.rawValue = rawValue }
    public static let keyframe  = MediaFlags(rawValue: 1 << 0)
    public static let encoded   = MediaFlags(rawValue: 1 << 1)
    public static let fullRange = MediaFlags(rawValue: 1 << 2)
    public static let config    = MediaFlags(rawValue: 1 << 3)

    /// All defined flag bits (used by decode to mask unknown bits).
    public static let known: MediaFlags = [.keyframe, .encoded, .fullRange, .config]
}

public enum MediaType: UInt8, Hashable {
    case video = 1
}

public enum MediaCodec: UInt8, Hashable {
    case raw  = 0
    case hevc = 1
    case h264 = 2
}

public enum MediaHeaderError: Error, Equatable {
    case tooShort(got: Int)
    case badMagic(UInt16)
    case unknownMediaType(UInt8)
    case unknownCodec(UInt8)
}

public struct MediaHeader: Hashable {
    public var mediaType: MediaType
    public var flags: MediaFlags
    public var codec: MediaCodec
    public var width: UInt16
    public var height: UInt16
    public var seq: UInt32
    public var ptsUsec: UInt64
    public var payloadLen: UInt32

    public init(
        mediaType: MediaType,
        flags: MediaFlags,
        codec: MediaCodec,
        width: UInt16,
        height: UInt16,
        seq: UInt32,
        ptsUsec: UInt64,
        payloadLen: UInt32
    ) {
        self.mediaType = mediaType
        self.flags = flags
        self.codec = codec
        self.width = width
        self.height = height
        self.seq = seq
        self.ptsUsec = ptsUsec
        self.payloadLen = payloadLen
    }

    public func encode() -> Data {
        var out = Data(count: MediaConstants.headerLen)
        out.withUnsafeMutableBytes { raw in
            let p = raw.baseAddress!.assumingMemoryBound(to: UInt8.self)
            // magic
            p[0] = UInt8(MediaConstants.magic >> 8)
            p[1] = UInt8(MediaConstants.magic & 0xFF)
            p[2] = mediaType.rawValue
            p[3] = flags.rawValue
            p[4] = codec.rawValue
            // reserved p[5..8] zero
            p[8]  = UInt8(width >> 8); p[9]  = UInt8(width & 0xFF)
            p[10] = UInt8(height >> 8); p[11] = UInt8(height & 0xFF)
            writeBE(seq, into: p, offset: 12)
            writeBE64(ptsUsec, into: p, offset: 16)
            writeBE(payloadLen, into: p, offset: 24)
        }
        return out
    }

    public static func decode(from data: Data) throws -> MediaHeader {
        guard data.count >= MediaConstants.headerLen else {
            throw MediaHeaderError.tooShort(got: data.count)
        }
        let magic = (UInt16(data[data.startIndex]) << 8) | UInt16(data[data.startIndex + 1])
        guard magic == MediaConstants.magic else { throw MediaHeaderError.badMagic(magic) }

        guard let mediaType = MediaType(rawValue: data[data.startIndex + 2]) else {
            throw MediaHeaderError.unknownMediaType(data[data.startIndex + 2])
        }
        let rawFlags = data[data.startIndex + 3]
        let flags = MediaFlags(rawValue: rawFlags).intersection(.known)
        guard let codec = MediaCodec(rawValue: data[data.startIndex + 4]) else {
            throw MediaHeaderError.unknownCodec(data[data.startIndex + 4])
        }
        // ignore data[5..8] (reserved)
        let width  = (UInt16(data[data.startIndex + 8]) << 8) | UInt16(data[data.startIndex + 9])
        let height = (UInt16(data[data.startIndex + 10]) << 8) | UInt16(data[data.startIndex + 11])
        let seq    = readBE32(data, offset: data.startIndex + 12)
        let pts    = readBE64(data, offset: data.startIndex + 16)
        let plen   = readBE32(data, offset: data.startIndex + 24)
        return MediaHeader(
            mediaType: mediaType, flags: flags, codec: codec,
            width: width, height: height, seq: seq, ptsUsec: pts, payloadLen: plen
        )
    }
}

@inline(__always)
private func writeBE(_ v: UInt32, into p: UnsafeMutablePointer<UInt8>, offset: Int) {
    p[offset    ] = UInt8((v >> 24) & 0xFF)
    p[offset + 1] = UInt8((v >> 16) & 0xFF)
    p[offset + 2] = UInt8((v >>  8) & 0xFF)
    p[offset + 3] = UInt8( v        & 0xFF)
}

@inline(__always)
private func writeBE64(_ v: UInt64, into p: UnsafeMutablePointer<UInt8>, offset: Int) {
    for i in 0..<8 {
        p[offset + i] = UInt8((v >> (UInt64(56 - i * 8))) & 0xFF)
    }
}

@inline(__always)
private func readBE32(_ d: Data, offset: Int) -> UInt32 {
    return (UInt32(d[offset    ]) << 24)
         | (UInt32(d[offset + 1]) << 16)
         | (UInt32(d[offset + 2]) <<  8)
         |  UInt32(d[offset + 3])
}

@inline(__always)
private func readBE64(_ d: Data, offset: Int) -> UInt64 {
    var v: UInt64 = 0
    for i in 0..<8 {
        v = (v << 8) | UInt64(d[offset + i])
    }
    return v
}
```

- [ ] **Step 4: Control message body types — `Handshake.swift`**

```swift
import Foundation

public struct DeviceIdent: Codable, Equatable {
    public var model: String
    public var osVer: String

    public init(model: String, osVer: String) {
        self.model = model; self.osVer = osVer
    }
}

public struct Hello: Codable, Equatable {
    public var protoVer: UInt32
    public var app: String
    public var device: DeviceIdent
    public var sessionId: String
    public var caps: [Capability]

    public init(protoVer: UInt32, app: String, device: DeviceIdent, sessionId: String, caps: [Capability]) {
        self.protoVer = protoVer; self.app = app; self.device = device
        self.sessionId = sessionId; self.caps = caps
    }
}

public struct HelloAck: Codable, Equatable {
    public var protoVer: UInt32
    public var caps: [Capability]
    public init(protoVer: UInt32, caps: [Capability]) { self.protoVer = protoVer; self.caps = caps }
}

public struct Auth: Codable, Equatable {
    public var token: String
    public init(token: String) { self.token = token }
}

public struct AuthOk: Codable, Equatable {
    public var sessionId: String
    public init(sessionId: String) { self.sessionId = sessionId }
}

public struct MediaHello: Codable, Equatable {
    public var sessionId: String
    public var token: String
    public init(sessionId: String, token: String) { self.sessionId = sessionId; self.token = token }
}

public struct Bye: Codable, Equatable {
    public var reason: String
    public init(reason: String) { self.reason = reason }
}

public enum ErrorCode: String, Codable, Equatable {
    case incompatibleVersion = "incompatible_version"
    case unauthorized
    case badRequest = "bad_request"
    case unsupportedMode = "unsupported_mode"
    case cameraUnavailable = "camera_unavailable"
    case `internal`
    case timeout
}

public struct ErrorMsg: Codable, Equatable {
    public var code: ErrorCode
    public var message: String
    public var ack: UInt64?
    public init(code: ErrorCode, message: String, ack: UInt64? = nil) {
        self.code = code; self.message = message; self.ack = ack
    }
}
```

- [ ] **Step 5: `Device.swift`**

```swift
import Foundation

public enum BatteryState: String, Codable, Equatable {
    case unknown, unplugged, charging, full
}

public enum ThermalState: String, Codable, Equatable {
    case nominal, fair, serious, critical
}

public enum CameraPosition: String, Codable, Equatable {
    case front, back
}

public struct DeviceInfo: Codable, Equatable {
    public var model: String
    public var osVer: String
    public var batteryLevel: Double
    public var batteryState: BatteryState
    public var thermalState: ThermalState
    public var usb3Capable: Bool

    public init(model: String, osVer: String, batteryLevel: Double, batteryState: BatteryState, thermalState: ThermalState, usb3Capable: Bool) {
        self.model = model; self.osVer = osVer
        self.batteryLevel = batteryLevel
        self.batteryState = batteryState; self.thermalState = thermalState
        self.usb3Capable = usb3Capable
    }
}

public struct CameraEntry: Codable, Equatable {
    public var id: String
    public var name: String
    public var position: CameraPosition
    public var maxWidth: UInt16
    public var maxHeight: UInt16
    public var maxFps: UInt16
    public var supportedFormats: [String]
    public init(id: String, name: String, position: CameraPosition, maxWidth: UInt16, maxHeight: UInt16, maxFps: UInt16, supportedFormats: [String]) {
        self.id = id; self.name = name; self.position = position
        self.maxWidth = maxWidth; self.maxHeight = maxHeight; self.maxFps = maxFps
        self.supportedFormats = supportedFormats
    }
}

public struct CameraList: Codable, Equatable {
    public var cameras: [CameraEntry]
    public init(cameras: [CameraEntry]) { self.cameras = cameras }
}

public struct SetCamera: Codable, Equatable {
    public var cameraId: String
    public init(cameraId: String) { self.cameraId = cameraId }
}

public struct CameraState: Codable, Equatable {
    public var activeCameraId: String
    public var appliedFormat: String
    public init(activeCameraId: String, appliedFormat: String) {
        self.activeCameraId = activeCameraId; self.appliedFormat = appliedFormat
    }
}
```

- [ ] **Step 6: `Streaming.swift`**

```swift
import Foundation

public enum FormatKind: String, Codable, Equatable { case raw, encoded }
public enum CodecName: String, Codable, Equatable { case none, hevc, h264 }
public enum PixelFormat: String, Codable, Equatable { case nv12 }

public struct Mode: Codable, Equatable {
    public var format: FormatKind
    public var codec: CodecName
    public var width: UInt16
    public var height: UInt16
    public var fps: UInt16
    public var bitrateKbps: UInt32
    public var pixelFormat: PixelFormat
    public var fullRange: Bool

    public init(format: FormatKind, codec: CodecName, width: UInt16, height: UInt16, fps: UInt16, bitrateKbps: UInt32, pixelFormat: PixelFormat, fullRange: Bool) {
        self.format = format; self.codec = codec
        self.width = width; self.height = height; self.fps = fps
        self.bitrateKbps = bitrateKbps
        self.pixelFormat = pixelFormat; self.fullRange = fullRange
    }
}

public struct Start: Codable, Equatable { public var mode: Mode; public init(mode: Mode) { self.mode = mode } }
public struct Stop: Codable, Equatable { public init() {} }
public struct SetMode: Codable, Equatable { public var mode: Mode; public init(mode: Mode) { self.mode = mode } }
public struct ModeApplied: Codable, Equatable {
    public var mode: Mode
    public var atSeq: UInt32
    public init(mode: Mode, atSeq: UInt32) { self.mode = mode; self.atSeq = atSeq }
}
```

- [ ] **Step 7: `Telemetry.swift`**

```swift
import Foundation

public struct Telemetry: Codable, Equatable {
    public var tsUsec: UInt64
    public var batteryLevel: Double
    public var batteryState: BatteryState
    public var thermalState: ThermalState
    public var sentBitrateKbps: UInt32
    public var encFps: UInt32
    public var captureFps: UInt32
    public var queueDepth: UInt32
    public var dropCount: UInt64

    public init(tsUsec: UInt64, batteryLevel: Double, batteryState: BatteryState, thermalState: ThermalState, sentBitrateKbps: UInt32, encFps: UInt32, captureFps: UInt32, queueDepth: UInt32, dropCount: UInt64) {
        self.tsUsec = tsUsec; self.batteryLevel = batteryLevel
        self.batteryState = batteryState; self.thermalState = thermalState
        self.sentBitrateKbps = sentBitrateKbps
        self.encFps = encFps; self.captureFps = captureFps
        self.queueDepth = queueDepth; self.dropCount = dropCount
    }
}

public struct Ping: Codable, Equatable {
    public var tsUsec: UInt64
    public init(tsUsec: UInt64) { self.tsUsec = tsUsec }
}

public struct Pong: Codable, Equatable {
    public var tsUsec: UInt64
    public var echoUsec: UInt64
    public init(tsUsec: UInt64, echoUsec: UInt64) {
        self.tsUsec = tsUsec; self.echoUsec = echoUsec
    }
}
```

- [ ] **Step 8: `Speedtest.swift`**

```swift
import Foundation

public enum SpeedtestPattern: String, Codable, Equatable { case ramp }

public struct SpeedtestStart: Codable, Equatable {
    public var id: String
    public var targetBitrateKbps: UInt32
    public var durationMs: UInt32
    public var pattern: SpeedtestPattern
    public init(id: String, targetBitrateKbps: UInt32, durationMs: UInt32, pattern: SpeedtestPattern) {
        self.id = id; self.targetBitrateKbps = targetBitrateKbps
        self.durationMs = durationMs; self.pattern = pattern
    }
}

public struct SpeedtestTick: Codable, Equatable {
    public var idSeq: UInt32
    public var seq: UInt32
    public var tsUsec: UInt64
    public init(idSeq: UInt32, seq: UInt32, tsUsec: UInt64) {
        self.idSeq = idSeq; self.seq = seq; self.tsUsec = tsUsec
    }
}

public struct SpeedtestResult: Codable, Equatable {
    public var id: String
    public var goodputMbps: Double
    public var rttMs: Double
    public var jitterMs: Double
    public var lossPct: Double
    public var recommendedMode: Mode
    public init(id: String, goodputMbps: Double, rttMs: Double, jitterMs: Double, lossPct: Double, recommendedMode: Mode) {
        self.id = id
        self.goodputMbps = goodputMbps; self.rttMs = rttMs
        self.jitterMs = jitterMs; self.lossPct = lossPct
        self.recommendedMode = recommendedMode
    }
}
```

- [ ] **Step 9: `ControlMessage.swift` + `ControlEnvelope.swift`**

`ControlMessage.swift`:

```swift
import Foundation

/// Discriminated union over all CCP control message bodies.
/// Encoded with the parent ControlEnvelope into a flat JSON object
/// (`t`/`seq`/`ack` siblings to body fields).
public enum ControlMessage: Equatable {
    case hello(Hello)
    case helloAck(HelloAck)
    case auth(Auth)
    case authOk(AuthOk)
    case mediaHello(MediaHello)
    case bye(Bye)
    case error(ErrorMsg)
    case deviceInfo(DeviceInfo)
    case cameraList(CameraList)
    case setCamera(SetCamera)
    case cameraState(CameraState)
    case start(Start)
    case stop(Stop)
    case setMode(SetMode)
    case modeApplied(ModeApplied)
    case telemetry(Telemetry)
    case ping(Ping)
    case pong(Pong)
    case speedtestStart(SpeedtestStart)
    case speedtestTick(SpeedtestTick)
    case speedtestResult(SpeedtestResult)

    public var typeTag: String {
        switch self {
        case .hello:           return "HELLO"
        case .helloAck:        return "HELLO_ACK"
        case .auth:            return "AUTH"
        case .authOk:          return "AUTH_OK"
        case .mediaHello:      return "MEDIA_HELLO"
        case .bye:             return "BYE"
        case .error:           return "ERROR"
        case .deviceInfo:      return "DEVICE_INFO"
        case .cameraList:      return "CAMERA_LIST"
        case .setCamera:       return "SET_CAMERA"
        case .cameraState:     return "CAMERA_STATE"
        case .start:           return "START"
        case .stop:            return "STOP"
        case .setMode:         return "SET_MODE"
        case .modeApplied:     return "MODE_APPLIED"
        case .telemetry:       return "TELEMETRY"
        case .ping:            return "PING"
        case .pong:            return "PONG"
        case .speedtestStart:  return "SPEEDTEST_START"
        case .speedtestTick:   return "SPEEDTEST_TICK"
        case .speedtestResult: return "SPEEDTEST_RESULT"
        }
    }
}
```

`ControlEnvelope.swift`:

```swift
import Foundation

public struct ControlEnvelope: Equatable {
    public var seq: UInt64
    public var ack: UInt64?
    public var body: ControlMessage

    public init(seq: UInt64, ack: UInt64? = nil, body: ControlMessage) {
        self.seq = seq; self.ack = ack; self.body = body
    }

    public func encodeJSON() throws -> Data {
        var dict = try ControlEnvelope.bodyDict(self.body)
        dict["t"] = self.body.typeTag
        dict["seq"] = self.seq
        if let ack = self.ack { dict["ack"] = ack }
        return try JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys])
    }

    public static func decodeJSON(_ data: Data) throws -> ControlEnvelope {
        guard let obj = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ControlEnvelopeError.notAnObject
        }
        guard let t = obj["t"] as? String else { throw ControlEnvelopeError.missingTag }
        let seq = (obj["seq"] as? UInt64) ?? UInt64(obj["seq"] as? Int ?? 0)
        let ack: UInt64?
        if let a = obj["ack"] as? UInt64 { ack = a }
        else if let a = obj["ack"] as? Int { ack = UInt64(a) }
        else { ack = nil }

        // Re-encode the *body fields only* and decode into the typed struct.
        var bodyDict = obj
        bodyDict.removeValue(forKey: "t")
        bodyDict.removeValue(forKey: "seq")
        bodyDict.removeValue(forKey: "ack")
        let bodyData = try JSONSerialization.data(withJSONObject: bodyDict, options: [.sortedKeys])
        let dec = JSONDecoder()

        let body: ControlMessage
        switch t {
        case "HELLO":            body = .hello(try dec.decode(Hello.self, from: bodyData))
        case "HELLO_ACK":        body = .helloAck(try dec.decode(HelloAck.self, from: bodyData))
        case "AUTH":             body = .auth(try dec.decode(Auth.self, from: bodyData))
        case "AUTH_OK":          body = .authOk(try dec.decode(AuthOk.self, from: bodyData))
        case "MEDIA_HELLO":      body = .mediaHello(try dec.decode(MediaHello.self, from: bodyData))
        case "BYE":              body = .bye(try dec.decode(Bye.self, from: bodyData))
        case "ERROR":            body = .error(try dec.decode(ErrorMsg.self, from: bodyData))
        case "DEVICE_INFO":      body = .deviceInfo(try dec.decode(DeviceInfo.self, from: bodyData))
        case "CAMERA_LIST":      body = .cameraList(try dec.decode(CameraList.self, from: bodyData))
        case "SET_CAMERA":       body = .setCamera(try dec.decode(SetCamera.self, from: bodyData))
        case "CAMERA_STATE":     body = .cameraState(try dec.decode(CameraState.self, from: bodyData))
        case "START":            body = .start(try dec.decode(Start.self, from: bodyData))
        case "STOP":             body = .stop(try dec.decode(Stop.self, from: bodyData))
        case "SET_MODE":         body = .setMode(try dec.decode(SetMode.self, from: bodyData))
        case "MODE_APPLIED":     body = .modeApplied(try dec.decode(ModeApplied.self, from: bodyData))
        case "TELEMETRY":        body = .telemetry(try dec.decode(Telemetry.self, from: bodyData))
        case "PING":             body = .ping(try dec.decode(Ping.self, from: bodyData))
        case "PONG":             body = .pong(try dec.decode(Pong.self, from: bodyData))
        case "SPEEDTEST_START":  body = .speedtestStart(try dec.decode(SpeedtestStart.self, from: bodyData))
        case "SPEEDTEST_TICK":   body = .speedtestTick(try dec.decode(SpeedtestTick.self, from: bodyData))
        case "SPEEDTEST_RESULT": body = .speedtestResult(try dec.decode(SpeedtestResult.self, from: bodyData))
        default: throw ControlEnvelopeError.unknownType(t)
        }
        return ControlEnvelope(seq: seq, ack: ack, body: body)
    }

    /// Encode the body fields (NOT including `t`/`seq`/`ack`) to a [String: Any] dict.
    private static func bodyDict(_ body: ControlMessage) throws -> [String: Any] {
        let enc = JSONEncoder()
        let data: Data
        switch body {
        case .hello(let v):           data = try enc.encode(v)
        case .helloAck(let v):        data = try enc.encode(v)
        case .auth(let v):            data = try enc.encode(v)
        case .authOk(let v):          data = try enc.encode(v)
        case .mediaHello(let v):      data = try enc.encode(v)
        case .bye(let v):             data = try enc.encode(v)
        case .error(let v):           data = try enc.encode(v)
        case .deviceInfo(let v):      data = try enc.encode(v)
        case .cameraList(let v):      data = try enc.encode(v)
        case .setCamera(let v):       data = try enc.encode(v)
        case .cameraState(let v):     data = try enc.encode(v)
        case .start(let v):           data = try enc.encode(v)
        case .stop(let v):            data = try enc.encode(v)
        case .setMode(let v):         data = try enc.encode(v)
        case .modeApplied(let v):     data = try enc.encode(v)
        case .telemetry(let v):       data = try enc.encode(v)
        case .ping(let v):            data = try enc.encode(v)
        case .pong(let v):            data = try enc.encode(v)
        case .speedtestStart(let v):  data = try enc.encode(v)
        case .speedtestTick(let v):   data = try enc.encode(v)
        case .speedtestResult(let v): data = try enc.encode(v)
        }
        guard let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ControlEnvelopeError.notAnObject
        }
        return dict
    }
}

public enum ControlEnvelopeError: Error, Equatable {
    case notAnObject
    case missingTag
    case unknownType(String)
}

/// Length-prefixed framing for the control channel.
public enum ControlFraming {
    public static let prefixLen: Int = 4
    public static let defaultMaxPayload: UInt32 = 1 << 20  // 1 MiB

    public enum FrameError: Error, Equatable {
        case tooLarge(len: UInt32, max: UInt32)
    }

    public static func frame(_ payload: Data) -> Data {
        var out = Data(capacity: prefixLen + payload.count)
        let len = UInt32(payload.count)
        out.append(UInt8((len >> 24) & 0xFF))
        out.append(UInt8((len >> 16) & 0xFF))
        out.append(UInt8((len >>  8) & 0xFF))
        out.append(UInt8( len        & 0xFF))
        out.append(payload)
        return out
    }

    /// Returns `(payload, total)` if a full frame is present, `nil` if more bytes are needed.
    public static func tryUnframe(_ buf: Data, maxPayload: UInt32 = defaultMaxPayload) throws -> (Data, Int)? {
        guard buf.count >= prefixLen else { return nil }
        let i = buf.startIndex
        let len = (UInt32(buf[i]) << 24)
                | (UInt32(buf[i+1]) << 16)
                | (UInt32(buf[i+2]) <<  8)
                |  UInt32(buf[i+3])
        if len > maxPayload { throw FrameError.tooLarge(len: len, max: maxPayload) }
        let total = prefixLen + Int(len)
        guard buf.count >= total else { return nil }
        let payload = buf.subdata(in: (i + prefixLen)..<(i + total))
        return (payload, total)
    }
}
```

- [ ] **Step 10: Build the Swift package (no tests yet)**

```bash
cd ios && swift build
```

Expected: builds without errors. Tests come in Task 11.

- [ ] **Step 11: Commit**

```bash
git add ios/
git commit -m "feat(ios): SwiftPM ClearCamProtocol module mirroring CCP types"
```

---

## Task 11 — iOS round-trip tests (XCTest)

**Files:**
- Create: `ios/Tests/ClearCamProtocolTests/MediaHeaderTests.swift`
- Create: `ios/Tests/ClearCamProtocolTests/ControlEnvelopeTests.swift`
- Create: `ios/Tests/ClearCamProtocolTests/ControlMessageJSONTests.swift`

- [ ] **Step 1: `MediaHeaderTests.swift`** (golden bytes identical to Rust)

```swift
import XCTest
@testable import ClearCamProtocol

final class MediaHeaderTests: XCTestCase {
    func sample() -> MediaHeader {
        MediaHeader(
            mediaType: .video,
            flags: [.keyframe, .encoded, .fullRange],
            codec: .hevc,
            width: 1920, height: 1080,
            seq: 42, ptsUsec: 1_234_567_890,
            payloadLen: 12345
        )
    }

    func testKnownBytesEncode() {
        let h = sample()
        let bytes = h.encode()
        let expected: [UInt8] = [
            0xCC, 0x01, 0x01, 0x07, 0x01, 0x00, 0x00, 0x00,
            0x07, 0x80, 0x04, 0x38,
            0x00, 0x00, 0x00, 0x2A,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x96, 0x02, 0xD2,
            0x00, 0x00, 0x30, 0x39,
        ]
        XCTAssertEqual(Array(bytes), expected)
    }

    func testRoundTrip() throws {
        let h = sample()
        let back = try MediaHeader.decode(from: h.encode())
        XCTAssertEqual(back, h)
    }

    func testTooShort() {
        do {
            _ = try MediaHeader.decode(from: Data(count: MediaConstants.headerLen - 1))
            XCTFail("expected error")
        } catch let MediaHeaderError.tooShort(got) {
            XCTAssertEqual(got, MediaConstants.headerLen - 1)
        } catch { XCTFail("wrong error: \(error)") }
    }

    func testBadMagic() {
        var b = sample().encode(); b[0] = 0
        XCTAssertThrowsError(try MediaHeader.decode(from: b)) { err in
            if case .badMagic(_) = (err as? MediaHeaderError) {} else { XCTFail("wrong: \(err)") }
        }
    }

    func testUnknownCodec() {
        var b = sample().encode(); b[4] = 99
        XCTAssertThrowsError(try MediaHeader.decode(from: b)) { err in
            XCTAssertEqual(err as? MediaHeaderError, .unknownCodec(99))
        }
    }

    func testUnknownMediaType() {
        var b = sample().encode(); b[2] = 99
        XCTAssertThrowsError(try MediaHeader.decode(from: b)) { err in
            XCTAssertEqual(err as? MediaHeaderError, .unknownMediaType(99))
        }
    }

    func testReservedIgnored() throws {
        var b = sample().encode(); b[5] = 0xAB; b[6] = 0xCD; b[7] = 0xEF
        XCTAssertEqual(try MediaHeader.decode(from: b), sample())
    }

    func testUnknownFlagsMasked() throws {
        var b = sample().encode(); b[3] = 0xFF
        let h = try MediaHeader.decode(from: b)
        XCTAssertEqual(h.flags, MediaFlags.known)
    }
}
```

- [ ] **Step 2: `ControlEnvelopeTests.swift`** (framing)

```swift
import XCTest
@testable import ClearCamProtocol

final class ControlEnvelopeTests: XCTestCase {
    func testFrameWritesBELengthPrefix() {
        let framed = ControlFraming.frame(Data("abc".utf8))
        XCTAssertEqual(Array(framed.prefix(4)), [0, 0, 0, 3])
        XCTAssertEqual(framed.suffix(3), Data("abc".utf8))
    }

    func testFrameEmpty() {
        XCTAssertEqual(Array(ControlFraming.frame(Data())), [0, 0, 0, 0])
    }

    func testRoundTrip() throws {
        let payload = Data(#"{"t":"HELLO","seq":1}"#.utf8)
        let framed = ControlFraming.frame(payload)
        guard let (got, n) = try ControlFraming.tryUnframe(framed) else {
            XCTFail("no frame"); return
        }
        XCTAssertEqual(got, payload)
        XCTAssertEqual(n, framed.count)
    }

    func testPartial() throws {
        let buf = Data([0, 0, 0, 5, UInt8(ascii: "h"), UInt8(ascii: "e")])
        XCTAssertNil(try ControlFraming.tryUnframe(buf))
    }

    func testTooLarge() {
        let buf = Data([0xFF, 0xFF, 0xFF, 0xFF])
        XCTAssertThrowsError(try ControlFraming.tryUnframe(buf, maxPayload: 1024)) { err in
            XCTAssertEqual(err as? ControlFraming.FrameError, .tooLarge(len: .max, max: 1024))
        }
    }
}
```

- [ ] **Step 3: `ControlMessageJSONTests.swift`** (envelope round-trip; shape checks)

```swift
import XCTest
@testable import ClearCamProtocol

final class ControlMessageJSONTests: XCTestCase {
    func testHelloEnvelope() throws {
        let env = ControlEnvelope(
            seq: 1, ack: nil,
            body: .hello(Hello(
                protoVer: 1,
                app: "ClearCam-iOS/0.1.0",
                device: DeviceIdent(model: "iPhone15,3", osVer: "iOS 18.0"),
                sessionId: "sess-abc",
                caps: [.hevc, .rawNv12]
            ))
        )
        let bytes = try env.encodeJSON()
        let obj = try JSONSerialization.jsonObject(with: bytes) as! [String: Any]
        XCTAssertEqual(obj["t"] as? String, "HELLO")
        XCTAssertEqual(obj["seq"] as? Int, 1)
        XCTAssertEqual(obj["protoVer"] as? Int, 1)
        XCTAssertNil(obj["ack"])

        let back = try ControlEnvelope.decodeJSON(bytes)
        XCTAssertEqual(back, env)
    }

    func testErrorEnvelopeWithAck() throws {
        let env = ControlEnvelope(
            seq: 100, ack: 99,
            body: .error(ErrorMsg(code: .badRequest, message: "oops"))
        )
        let bytes = try env.encodeJSON()
        let obj = try JSONSerialization.jsonObject(with: bytes) as! [String: Any]
        XCTAssertEqual(obj["ack"] as? Int, 99)
        XCTAssertEqual(obj["code"] as? String, "bad_request")
        let back = try ControlEnvelope.decodeJSON(bytes)
        XCTAssertEqual(back, env)
    }

    func testAllVariantsRoundTrip() throws {
        let mode = Mode(format: .raw, codec: .none, width: 1280, height: 720, fps: 30,
                        bitrateKbps: 0, pixelFormat: .nv12, fullRange: false)

        let bodies: [ControlMessage] = [
            .hello(Hello(protoVer: 1, app: "x",
                         device: DeviceIdent(model: "m", osVer: "v"),
                         sessionId: "s", caps: [.hevc])),
            .helloAck(HelloAck(protoVer: 1, caps: [])),
            .auth(Auth(token: "t")),
            .authOk(AuthOk(sessionId: "s")),
            .mediaHello(MediaHello(sessionId: "s", token: "t")),
            .bye(Bye(reason: "done")),
            .error(ErrorMsg(code: .timeout, message: "x", ack: 1)),
            .deviceInfo(DeviceInfo(model: "m", osVer: "v", batteryLevel: 0.5,
                                   batteryState: .charging, thermalState: .nominal,
                                   usb3Capable: false)),
            .cameraList(CameraList(cameras: [])),
            .setCamera(SetCamera(cameraId: "wide")),
            .cameraState(CameraState(activeCameraId: "wide", appliedFormat: "1080p30 hevc")),
            .start(Start(mode: mode)),
            .stop(Stop()),
            .setMode(SetMode(mode: mode)),
            .modeApplied(ModeApplied(mode: mode, atSeq: 0)),
            .telemetry(Telemetry(tsUsec: 0, batteryLevel: 0,
                                 batteryState: .unknown, thermalState: .nominal,
                                 sentBitrateKbps: 0, encFps: 0, captureFps: 0,
                                 queueDepth: 0, dropCount: 0)),
            .ping(Ping(tsUsec: 0)),
            .pong(Pong(tsUsec: 0, echoUsec: 0)),
            .speedtestStart(SpeedtestStart(id: "x", targetBitrateKbps: 0, durationMs: 0, pattern: .ramp)),
            .speedtestTick(SpeedtestTick(idSeq: 0, seq: 0, tsUsec: 0)),
            .speedtestResult(SpeedtestResult(id: "x", goodputMbps: 0, rttMs: 0, jitterMs: 0, lossPct: 0, recommendedMode: mode)),
        ]

        for body in bodies {
            let env = ControlEnvelope(seq: 1, body: body)
            let bytes = try env.encodeJSON()
            let back = try ControlEnvelope.decodeJSON(bytes)
            XCTAssertEqual(back, env, "round-trip failed for \(body.typeTag)")
        }
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd ios && swift test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add ios/Tests/
git commit -m "test(ios): XCTest round-trip + golden bytes for ClearCamProtocol"
```

---

## Task 12 — GitHub Actions CI

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write the workflow**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  rust:
    name: Rust (workspace)
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: desktop
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: desktop
      - name: cargo fmt --check
        run: cargo fmt --all -- --check
      - name: cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: cargo test
        run: cargo test --workspace --all-targets

  ui:
    name: UI (TypeScript / Vite)
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: desktop/ui
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v3
        with:
          version: 9
      - uses: actions/setup-node@v4
        with:
          node-version: "20"
          cache: pnpm
          cache-dependency-path: desktop/ui/pnpm-lock.yaml
      - name: install
        run: pnpm install --frozen-lockfile
      - name: typecheck
        run: pnpm typecheck
      - name: lint
        run: pnpm lint
      - name: format check
        run: pnpm format:check
      - name: build
        run: pnpm build

  swift:
    name: Swift (ClearCamProtocol)
    runs-on: macos-latest
    defaults:
      run:
        working-directory: ios
    steps:
      - uses: actions/checkout@v4
      - name: select Xcode
        run: sudo xcode-select -s /Applications/Xcode.app
      - name: swift build
        run: swift build
      - name: swift test
        run: swift test
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add fmt/clippy/test for Rust, lint/build for UI, swift test for iOS"
```

> Note: CI runs on push; verify it locally first per Task 13 before pushing. If the user has not set up a GitHub remote yet, the workflow file is still useful as the source of truth for local checks.

---

## Task 13 — Verification & Phase 0 completion

This task runs the full local validation chain and tags the commit.

- [ ] **Step 1: Rust gate (locally, exactly what CI runs)**

```bash
cd desktop && cargo fmt --all -- --check
cd desktop && cargo clippy --workspace --all-targets -- -D warnings
cd desktop && cargo test --workspace --all-targets
```

Expected: all green.

- [ ] **Step 2: UI gate**

```bash
cd desktop/ui && pnpm install --frozen-lockfile
cd desktop/ui && pnpm typecheck && pnpm lint && pnpm format:check && pnpm build
```

Expected: all green; `dist/` produced.

- [ ] **Step 3: Swift gate**

```bash
cd ios && swift build && swift test
```

Expected: all green.

- [ ] **Step 4: Tauri smoke (manual, only on the dev machine)**

```bash
cd desktop && cargo tauri dev --no-watch
```

Expected: a window opens showing "ClearCam / Phase 0 — foundation". Close the window.

If `cargo tauri` is not installed: `cargo install tauri-cli --version "^2"` once, then retry.

- [ ] **Step 5: Workspace listing sanity**

```bash
git ls-files | sort
git status --short
```

Expected: working tree clean; the file listing matches the "File Structure" section near the top of this plan (less the `pnpm-lock.yaml`, which is generated and committed via Task 9).

- [ ] **Step 6: Tag Phase 0**

```bash
git tag -a v0.1.0-phase0 -m "Phase 0 — foundation complete"
git tag -l
```

- [ ] **Step 7: Phase-0 retrospective note**

Append a `## Phase 0 — DONE (YYYY-MM-DD)` section at the bottom of `plans/phase-0-foundation.md` listing:
- the commit range,
- any deviations from the plan,
- any new ADRs added during execution (if none, write "none"),
- any items deferred to Phase 1 (should be "none" — Phase 0 has no features).

Commit:

```bash
git add plans/phase-0-foundation.md
git commit -m "docs(plans): close Phase 0 with retrospective"
```

- [ ] **Step 8: Hand off to Phase 1**

Phase 0 is complete. The next worker should invoke `superpowers:writing-plans` again with the spec for **Phase 1 — Transport & handshake (Wi-Fi loopback)** per `docs/10-roadmap-and-plan.md`. Do not start Phase 1 work in the same session.

---

## Spec coverage self-check

| Spec requirement (Phase 0 acceptance, docs/10) | Implemented by |
|---|---|
| Rust workspace at `desktop/`, empty crates per docs/08 | Task 2 |
| `ccp-protocol` crate: serde control types | Tasks 4–7 |
| `ccp-protocol` crate: binary media-header codec | Task 8 |
| `ccp-protocol` crate: version/caps | Task 4 |
| iOS project skeleton mirroring formats | Tasks 10–11 |
| CI with fmt/clippy/test (Rust) | Task 12 (`rust` job) |
| Tauri scaffold opening empty window | Task 9 |
| Round-trip tests for ccp-protocol pass (control JSON + media header) | Tasks 3, 5–8, 11 |
| CI green | Task 13 (verify locally; CI runs on push) |
| NO feature code yet | Scope boundary at top of plan |

## Notes for the executing agent

1. **Do not skip the fmt/clippy/test gate after every task.** Phase 0 is the foundation — its CI gates exist precisely to catch drift early.
2. **If a step's expected outcome differs from what you see**, stop and investigate the root cause before changing the test or the plan. Use `superpowers:systematic-debugging`.
3. **Resist scope expansion.** Do not add tokio, networking, mDNS, QR generation, decoders, or any platform code in Phase 0. Those crates' `lib.rs` files contain only the doc-comment stub from Task 2. Any addition belongs to a later phase plan.
4. **Cross-language byte-exactness.** The Rust and Swift media-header tests assert the **same** byte sequence (28 bytes). If you change anything about the header encoding, both test files must be updated and must agree.
5. **Commit hygiene.** One commit per task is the default; if a task's steps decompose naturally into smaller commits, that's fine. Never combine multiple tasks into a single commit.
