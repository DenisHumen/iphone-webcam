//! Control plane: length-prefixed JSON messages.
//!
//! Wire format: `[uint32 BE length][UTF-8 JSON payload]` per `docs/03-protocol.md` §5.1.

pub mod envelope;

pub use envelope::{frame, try_unframe, FrameError, DEFAULT_MAX_PAYLOAD, PREFIX_LEN};
