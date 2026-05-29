//! Provenance tag attached to every (Control|Media)Stream so consumers can
//! distinguish Wi-Fi from USB without inspecting socket internals.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Wifi,
    Usb { udid: String },
}

impl Source {
    pub fn udid(&self) -> Option<&str> {
        match self {
            Source::Usb { udid } => Some(udid),
            Source::Wifi => None,
        }
    }

    pub fn is_usb(&self) -> bool {
        matches!(self, Source::Usb { .. })
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Wifi => write!(f, "wifi"),
            Source::Usb { udid } => write!(f, "usb({udid})"),
        }
    }
}
