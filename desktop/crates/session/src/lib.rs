//! `session` — control plane actor, handshake, and media pairing.

#![forbid(unsafe_code)]

pub mod error;
pub mod state;

pub use error::SessionError;
pub use state::{DeviceSnapshot, PendingMediaBindings, SessionSnapshot, SessionStateKind};
