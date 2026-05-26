//! Protocol version and capability negotiation.

use serde::{Deserialize, Serialize};

/// Current CCP protocol version. Bumped on incompatible changes.
pub const PROTO_VER: u32 = 1;

/// A protocol capability advertised in `HELLO`/`HELLO_ACK`.
///
/// Unknown capability strings deserialize to [`Capability::Other`] so older
/// readers do not break when newer peers advertise new flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    Hevc,
    H264,
    RawNv12,
    Usb3,
    SpeedtestV1,
    Other(String),
}

impl Capability {
    /// Canonical wire string for known capabilities.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Hevc => "hevc",
            Self::H264 => "h264",
            Self::RawNv12 => "raw_nv12",
            Self::Usb3 => "usb3",
            Self::SpeedtestV1 => "speedtest_v1",
            Self::Other(s) => s.as_str(),
        }
    }
}

impl From<&str> for Capability {
    fn from(s: &str) -> Self {
        match s {
            "hevc" => Self::Hevc,
            "h264" => Self::H264,
            "raw_nv12" => Self::RawNv12,
            "usb3" => Self::Usb3,
            "speedtest_v1" => Self::SpeedtestV1,
            other => Self::Other(other.to_string()),
        }
    }
}

impl Serialize for Capability {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(Self::from(s.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proto_ver_is_one() {
        assert_eq!(PROTO_VER, 1);
    }

    #[test]
    fn known_capability_round_trip() {
        for cap in [
            Capability::Hevc,
            Capability::H264,
            Capability::RawNv12,
            Capability::Usb3,
            Capability::SpeedtestV1,
        ] {
            let s = serde_json::to_string(&cap).unwrap();
            let back: Capability = serde_json::from_str(&s).unwrap();
            assert_eq!(back, cap);
        }
    }

    #[test]
    fn unknown_capability_parses_as_other() {
        let cap: Capability = serde_json::from_str(r#""future_codec""#).unwrap();
        assert_eq!(cap, Capability::Other("future_codec".into()));
        let s = serde_json::to_string(&cap).unwrap();
        assert_eq!(s, r#""future_codec""#);
    }

    #[test]
    fn capability_in_array_round_trip() {
        let caps = vec![
            Capability::Hevc,
            Capability::H264,
            Capability::Other("x".into()),
        ];
        let s = serde_json::to_string(&caps).unwrap();
        assert_eq!(s, r#"["hevc","h264","x"]"#);
        let back: Vec<Capability> = serde_json::from_str(&s).unwrap();
        assert_eq!(back, caps);
    }
}
