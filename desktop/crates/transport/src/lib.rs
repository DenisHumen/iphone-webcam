//! `transport` — async TCP server/client + length-prefixed JSON framing.
//!
//! See `docs/02-architecture.md` and `docs/03-protocol.md` §5.

#![forbid(unsafe_code)]

pub mod error;
pub mod framing;

pub use error::TransportError;
pub use framing::{read_frame, write_frame};
