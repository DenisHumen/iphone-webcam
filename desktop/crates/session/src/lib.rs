//! `session` — control plane actor, handshake, and media pairing.

#![forbid(unsafe_code)]

pub mod error;
pub mod handshake;
pub mod state;

pub use error::SessionError;
pub use handshake::{accept_control, AcceptedSession, HANDSHAKE_TIMEOUT};
pub use state::{DeviceSnapshot, PendingMediaBindings, SessionSnapshot, SessionStateKind};
