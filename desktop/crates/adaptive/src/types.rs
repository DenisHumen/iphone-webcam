use ccp_protocol::Capability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// Sustained goodput in megabits per second.
    pub goodput_mbps: f64,
    pub rtt_ms: f64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportClass {
    WiFi,
    Usb2,
    Usb3,
}

impl TransportClass {
    /// Highest raw bitrate (in kbps) we will ever attempt on this transport.
    /// docs/06 §4: blocks "obviously impossible" raw combos even if a one-off
    /// measurement showed high goodput.
    #[must_use]
    pub fn raw_ceiling_kbps(self) -> u64 {
        match self {
            // Wi-Fi we never advertise raw — encoded covers visually-lossless.
            Self::WiFi => 0,
            Self::Usb2 => 480_000,
            Self::Usb3 => 5_000_000,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserLimits {
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
    pub max_fps: Option<u16>,
    #[serde(default)]
    pub prefer_raw: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableMode {
    pub width: u16,
    pub height: u16,
    pub fps: u16,
    pub caps: Vec<Capability>,
}

impl AvailableMode {
    #[must_use]
    pub fn supports(&self, cap: &Capability) -> bool {
        self.caps.iter().any(|c| c == cap)
    }
}
