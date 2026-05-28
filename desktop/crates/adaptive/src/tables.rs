//! Bitrate tables per docs/06 §2.

/// Raw NV12 bitrate in kilobits per second.
#[must_use]
pub fn raw_bitrate_kbps(width: u32, height: u32, fps: u32) -> u64 {
    (u64::from(width) * u64::from(height) * 3 / 2 * 8 * u64::from(fps)) / 1000
}

/// Visually-lossless HEVC target bitrate in kilobits per second.
/// Table is the docs/06 §2.2 defaults; unknown res×fps gets a coarse fallback.
#[must_use]
pub fn hevc_target_kbps(width: u32, height: u32, fps: u32) -> u64 {
    match (width, height, fps) {
        (1280, 720, 30) => 12_000,
        (1280, 720, 60) => 20_000,
        (1920, 1080, 30) => 30_000,
        (1920, 1080, 60) => 50_000,
        (3840, 2160, 30) => 90_000,
        _ => {
            // Coarse fallback. Tuned so 1280x720@30 → ~10800.
            let area = u64::from(width) * u64::from(height);
            (area * u64::from(fps)) / 2_560
        }
    }
}

/// H.264 baseline targets are ~1.6× HEVC at the same perceived quality.
#[must_use]
pub fn h264_target_kbps(width: u32, height: u32, fps: u32) -> u64 {
    (hevc_target_kbps(width, height, fps) * 8) / 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_720p30_matches_docs() {
        // docs/06 §2.1: 720p30 ≈ 332 Mbps.
        let kbps = raw_bitrate_kbps(1280, 720, 30);
        assert_eq!(kbps, 331_776);
    }

    #[test]
    fn raw_1080p30_matches_docs() {
        // docs/06: 1080p30 ≈ 746 Mbps.
        let kbps = raw_bitrate_kbps(1920, 1080, 30);
        assert_eq!(kbps, 746_496);
    }

    #[test]
    fn hevc_targets_match_table() {
        assert_eq!(hevc_target_kbps(1280, 720, 30), 12_000);
        assert_eq!(hevc_target_kbps(1280, 720, 60), 20_000);
        assert_eq!(hevc_target_kbps(1920, 1080, 30), 30_000);
        assert_eq!(hevc_target_kbps(1920, 1080, 60), 50_000);
        assert_eq!(hevc_target_kbps(3840, 2160, 30), 90_000);
    }

    #[test]
    fn h264_costs_more_than_hevc_for_same_quality() {
        let hevc = hevc_target_kbps(1920, 1080, 30);
        let h264 = h264_target_kbps(1920, 1080, 30);
        assert!(h264 > hevc);
    }
}
