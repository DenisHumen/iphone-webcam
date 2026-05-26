//! Telemetry, ping/pong (§6.4).

use serde::{Deserialize, Serialize};

use crate::control::device::{BatteryState, ThermalState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    /// Device monotonic clock, microseconds.
    pub ts_usec: u64,
    pub battery_level: f64,
    pub battery_state: BatteryState,
    pub thermal_state: ThermalState,
    pub sent_bitrate_kbps: u32,
    pub enc_fps: u32,
    pub capture_fps: u32,
    pub queue_depth: u32,
    pub drop_count: u64,
}

// Telemetry contains `f64` so it cannot be `Eq`; only `PartialEq`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ping {
    pub ts_usec: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pong {
    pub ts_usec: u64,
    pub echo_usec: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_round_trip() {
        let t = Telemetry {
            ts_usec: 1_000_000,
            battery_level: 0.5,
            battery_state: BatteryState::Charging,
            thermal_state: ThermalState::Fair,
            sent_bitrate_kbps: 25_000,
            enc_fps: 30,
            capture_fps: 30,
            queue_depth: 1,
            drop_count: 2,
        };
        let s = serde_json::to_string(&t).unwrap();
        let back: Telemetry = serde_json::from_str(&s).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn ping_pong_round_trip() {
        let p = Ping { ts_usec: 12345 };
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Ping>(&s).unwrap(), p);

        let q = Pong {
            ts_usec: 23456,
            echo_usec: 12345,
        };
        let s = serde_json::to_string(&q).unwrap();
        assert_eq!(serde_json::from_str::<Pong>(&s).unwrap(), q);
    }
}
