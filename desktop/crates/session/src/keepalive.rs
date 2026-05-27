//! PING/PONG keepalive helpers.

use std::time::Duration;

pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(2);
pub const DEAD_PEER_AFTER: Duration = Duration::from_secs(6);

pub fn now_usec() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
}
