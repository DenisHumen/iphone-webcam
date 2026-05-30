//! Adaptive driver: feed scripted telemetry, assert SET_MODE on the outbound.

use app::AdaptationDriver;
use ccp_protocol::{BatteryState, ControlMessage, Telemetry, ThermalState};
use std::time::Duration;
use tokio::sync::mpsc;

fn telemetry(queue_depth: u32, drop_count: u32) -> Telemetry {
    Telemetry {
        ts_usec: 0,
        battery_level: 0.9,
        battery_state: BatteryState::Unplugged,
        thermal_state: ThermalState::Nominal,
        sent_bitrate_kbps: 0,
        enc_fps: 30,
        capture_fps: 30,
        queue_depth,
        drop_count: u64::from(drop_count),
    }
}

#[tokio::test]
async fn sustained_congestion_triggers_step_down_on_wire() {
    let (out_tx, mut out_rx) = mpsc::channel::<ControlMessage>(8);
    let mut driver = AdaptationDriver::new_for_tests(out_tx);

    // Sustained congestion: deep queue with drops.
    for i in 0..6 {
        driver.observe(&telemetry(2 + i, 5 * i), 0.0).await;
    }

    let msg = tokio::time::timeout(Duration::from_millis(100), out_rx.recv())
        .await
        .expect("timeout waiting for SET_MODE")
        .expect("channel closed");
    assert!(matches!(msg, ControlMessage::SetMode(_)), "got {msg:?}");
}

/// Verifies that purely stable telemetry does NOT cause flapping / repeated
/// SET_MODE messages.  We start with a freshly-constructed driver whose
/// cooldowns have NOT yet expired, so a step-up cannot fire on the very first
/// run of stable samples — the state machine must be quiet.
#[tokio::test]
async fn stable_telemetry_emits_nothing() {
    use ccp_protocol::{CodecName, FormatKind, Mode, PixelFormat};

    // Use a fresh (non-backdated) driver: cooldown starts at Instant::now(),
    // so 10 rapid stable observations cannot cross the 5 s step-up cooldown.
    let (out_tx, mut out_rx) = mpsc::channel::<ControlMessage>(8);
    let initial = Mode {
        format: FormatKind::Encoded,
        codec: CodecName::Hevc,
        width: 1920,
        height: 1080,
        fps: 30,
        bitrate_kbps: 30_000,
        pixel_format: PixelFormat::Nv12,
        full_range: true,
    };
    let mut driver = AdaptationDriver::new(out_tx, initial);

    for _ in 0..10 {
        driver.observe(&telemetry(0, 0), 5.0).await;
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), out_rx.recv())
            .await
            .is_err(),
        "no SET_MODE expected while step-up cooldown has not elapsed"
    );
}
