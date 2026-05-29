//! `transport` — async TCP server/client + length-prefixed JSON framing.
//!
//! See `docs/02-architecture.md` and `docs/03-protocol.md` §5.

#![forbid(unsafe_code)]

pub mod error;
pub mod framing;
pub mod source;
pub mod streams;
pub mod usb;
pub mod wifi;

pub use error::TransportError;
pub use framing::{read_frame, write_frame};
pub use source::Source;
pub use streams::{ControlStream, MediaStream, PeerInfo};
pub use usb::{UsbConductor, UsbDevice, UsbEvent, UsbTransportError};
pub use wifi::{
    client::connect as wifi_connect,
    server::{BoundPorts, BoundPortsWithListeners, WifiServerEvent},
};
