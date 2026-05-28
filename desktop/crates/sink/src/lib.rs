//! `sink` — canonical Frame type + async FrameSink trait.

#![forbid(unsafe_code)]

pub mod frame;
mod sink_trait;

pub use frame::Frame;
pub use sink_trait::FrameSink;
