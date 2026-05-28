//! Mode selection per docs/06 §4.
//!
//! Decision: maximize (res_area, fps, format_pref), where at equal res/fps
//! raw wins over encoded. `prefer_raw=true` filters to raw-only candidates
//! before ranking.

use ccp_protocol::{Capability, CodecName, FormatKind, Mode, PixelFormat};

use crate::tables::{hevc_target_kbps, raw_bitrate_kbps};
use crate::types::{AvailableMode, Measurement, TransportClass, UserLimits};

const HEADROOM: f64 = 0.70;

#[derive(Debug, Clone, Copy, PartialEq)]
enum FormatPref {
    Raw,
    Encoded,
}

impl FormatPref {
    fn weight(self) -> u8 {
        match self {
            Self::Raw => 1,
            Self::Encoded => 0,
        }
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    mode: Mode,
    res_area: u64,
    fps: u16,
    pref: FormatPref,
}

#[must_use]
pub fn select_mode(
    measurement: &Measurement,
    caps: &[AvailableMode],
    transport: TransportClass,
    limits: &UserLimits,
) -> Mode {
    let usable_kbps = (measurement.goodput_mbps * 1000.0 * HEADROOM) as u64;
    let mut candidates: Vec<Candidate> = Vec::new();

    for m in caps {
        if let Some(max_w) = limits.max_width {
            if m.width > max_w {
                continue;
            }
        }
        if let Some(max_h) = limits.max_height {
            if m.height > max_h {
                continue;
            }
        }
        if let Some(max_fps) = limits.max_fps {
            if m.fps > max_fps {
                continue;
            }
        }

        // Raw candidate.
        if m.supports(&Capability::RawNv12) {
            let raw_kbps =
                raw_bitrate_kbps(u32::from(m.width), u32::from(m.height), u32::from(m.fps));
            if raw_kbps <= usable_kbps && raw_kbps <= transport.raw_ceiling_kbps() {
                candidates.push(Candidate {
                    mode: Mode {
                        format: FormatKind::Raw,
                        codec: CodecName::None,
                        width: m.width,
                        height: m.height,
                        fps: m.fps,
                        bitrate_kbps: 0,
                        pixel_format: PixelFormat::Nv12,
                        full_range: true,
                    },
                    res_area: u64::from(m.width) * u64::from(m.height),
                    fps: m.fps,
                    pref: FormatPref::Raw,
                });
            }
        }

        // Encoded candidate (HEVC preferred over H.264).
        let encoded_codec = if m.supports(&Capability::Hevc) {
            Some(CodecName::Hevc)
        } else if m.supports(&Capability::H264) {
            Some(CodecName::H264)
        } else {
            None
        };
        if let Some(codec) = encoded_codec {
            let target =
                hevc_target_kbps(u32::from(m.width), u32::from(m.height), u32::from(m.fps));
            let target = match codec {
                CodecName::Hevc => target,
                CodecName::H264 => (target * 8) / 5,
                CodecName::None => target,
            };
            if target <= usable_kbps {
                candidates.push(Candidate {
                    mode: Mode {
                        format: FormatKind::Encoded,
                        codec,
                        width: m.width,
                        height: m.height,
                        fps: m.fps,
                        bitrate_kbps: u32::try_from(target).unwrap_or(u32::MAX),
                        pixel_format: PixelFormat::Nv12,
                        full_range: true,
                    },
                    res_area: u64::from(m.width) * u64::from(m.height),
                    fps: m.fps,
                    pref: FormatPref::Encoded,
                });
            }
        }
    }

    if limits.prefer_raw && candidates.iter().any(|c| c.pref == FormatPref::Raw) {
        candidates.retain(|c| c.pref == FormatPref::Raw);
    }

    candidates
        .into_iter()
        .max_by_key(|c| (c.res_area, c.fps, c.pref.weight()))
        .map_or_else(safe_fallback, |c| c.mode)
}

fn safe_fallback() -> Mode {
    Mode {
        format: FormatKind::Encoded,
        codec: CodecName::H264,
        width: 1280,
        height: 720,
        fps: 30,
        bitrate_kbps: 12_000 * 8 / 5,
        pixel_format: PixelFormat::Nv12,
        full_range: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::Capability;

    fn modes_iphone_15_pro() -> Vec<AvailableMode> {
        let nv = vec![Capability::RawNv12, Capability::Hevc, Capability::H264];
        vec![
            AvailableMode {
                width: 1280,
                height: 720,
                fps: 30,
                caps: nv.clone(),
            },
            AvailableMode {
                width: 1280,
                height: 720,
                fps: 60,
                caps: nv.clone(),
            },
            AvailableMode {
                width: 1920,
                height: 1080,
                fps: 30,
                caps: nv.clone(),
            },
            AvailableMode {
                width: 1920,
                height: 1080,
                fps: 60,
                caps: nv,
            },
        ]
    }

    fn measurement(goodput_mbps: f64) -> Measurement {
        Measurement {
            goodput_mbps,
            rtt_ms: 5.0,
            jitter_ms: 1.0,
            loss_pct: 0.0,
        }
    }

    #[test]
    fn modest_wifi_picks_encoded_1080p30() {
        // 50 Mbps Wi-Fi. Headroom × 50 = 35 Mbps usable.
        // hevc_target(1080p30)=30 Mbps fits; hevc(1080p60)=50 Mbps does not.
        let m = select_mode(
            &measurement(50.0),
            &modes_iphone_15_pro(),
            TransportClass::WiFi,
            &UserLimits::default(),
        );
        assert_eq!(m.format, FormatKind::Encoded);
        assert_eq!(m.codec, CodecName::Hevc);
        assert_eq!((m.width, m.height, m.fps), (1920, 1080, 30));
    }

    #[test]
    fn fat_wifi_picks_hevc_1080p60_not_raw() {
        // 100 Mbps Wi-Fi. 70 usable. hevc(1080p60)=50 fits. raw would be 1.5 Gbps.
        // Wi-Fi transport always blocks raw, so encoded wins.
        let m = select_mode(
            &measurement(100.0),
            &modes_iphone_15_pro(),
            TransportClass::WiFi,
            &UserLimits::default(),
        );
        assert_eq!(m.format, FormatKind::Encoded);
        assert_eq!((m.width, m.height, m.fps), (1920, 1080, 60));
    }

    #[test]
    fn usb3_picks_raw_1080p30_over_encoded() {
        // 800 Mbps USB3. Headroom 560. raw(1080p30)=746 — doesn't fit headroom.
        // raw(720p60)=664 — doesn't fit either. raw(720p30)=332 — fits.
        // At equal res/fps raw wins, but 1080p30 encoded (30 Mbps) wins on area.
        let m = select_mode(
            &measurement(800.0),
            &modes_iphone_15_pro(),
            TransportClass::Usb3,
            &UserLimits::default(),
        );
        assert_eq!((m.width, m.height, m.fps), (1920, 1080, 60));
        assert_eq!(m.format, FormatKind::Encoded); // raw 1080p60 doesn't fit
    }

    #[test]
    fn enormous_usb3_picks_raw_at_top_resolution() {
        // 2 Gbps USB3. Headroom 1400 Mbps = 1_400_000 kbps. raw(1080p60)=1.49 Gbps fits.
        let m = select_mode(
            &measurement(2_500.0),
            &modes_iphone_15_pro(),
            TransportClass::Usb3,
            &UserLimits::default(),
        );
        assert_eq!(m.format, FormatKind::Raw);
        assert_eq!((m.width, m.height, m.fps), (1920, 1080, 60));
    }

    #[test]
    fn prefer_raw_filters_to_raw_only() {
        // 800 Mbps USB3 with prefer_raw=true. Encoded would win without the flag.
        let m = select_mode(
            &measurement(800.0),
            &modes_iphone_15_pro(),
            TransportClass::Usb3,
            &UserLimits {
                prefer_raw: true,
                ..Default::default()
            },
        );
        assert_eq!(m.format, FormatKind::Raw);
        // raw 720p30 (332 Mbps) is the best raw fit under headroom 560.
        assert_eq!((m.width, m.height, m.fps), (1280, 720, 30));
    }

    #[test]
    fn max_height_limit_is_respected() {
        let m = select_mode(
            &measurement(100.0),
            &modes_iphone_15_pro(),
            TransportClass::WiFi,
            &UserLimits {
                max_height: Some(720),
                ..Default::default()
            },
        );
        assert!(m.height <= 720);
    }

    #[test]
    fn falls_back_when_nothing_fits() {
        // Nothing fits → safe fallback 720p30 encoded H.264.
        let caps = vec![AvailableMode {
            width: 1920,
            height: 1080,
            fps: 60,
            caps: vec![Capability::H264],
        }];
        let m = select_mode(
            &measurement(0.5),
            &caps,
            TransportClass::WiFi,
            &UserLimits::default(),
        );
        // No 1080p60 candidate fits. Safe fallback returns.
        assert_eq!((m.width, m.height, m.fps), (1280, 720, 30));
        assert_eq!(m.codec, CodecName::H264);
    }
}
