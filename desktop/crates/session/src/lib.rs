//! `session` — control plane actor, handshake, and media pairing.

#![forbid(unsafe_code)]

pub mod controlplane;
pub mod error;
pub mod handshake;
pub mod keepalive;
pub mod pairing;
pub mod state;

pub use controlplane::{ControlPlane, ControlPlaneEvent, OutboundReceiver, OutboundSender};
pub use error::SessionError;
pub use handshake::{accept_control, accept_control_usb, AcceptedSession, HANDSHAKE_TIMEOUT};
pub use pairing::{read_media_hello, write_media_hello, MediaBinding, MEDIA_HANDSHAKE_TIMEOUT};
pub use state::{DeviceSnapshot, PendingMediaBindings, SessionSnapshot, SessionStateKind};
