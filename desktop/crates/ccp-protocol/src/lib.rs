//! ClearCam Protocol (CCP) — pure data types and (de)serialization.
//!
//! Wire format spec: `docs/03-protocol.md`. No I/O, no async, no platform code.

#![forbid(unsafe_code)]

pub mod control;
pub mod version;

pub use version::{Capability, PROTO_VER};
