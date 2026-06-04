//! Phase 6b closed-loop adaptation: a congested mock-iphone makes the desktop
//! AdaptationDriver emit SET_MODE on the wire; mock-iphone acks MODE_APPLIED,
//! which the desktop surfaces as ControlPlaneEvent::ModeApplied.

use std::sync::Arc;
use std::time::Duration;

use app::{AppHandle, PairingMaterial, PairingStore};
use session::{ControlPlaneEvent, SessionStateKind};
use transport::usb::{LoopbackConductor, LoopbackDevice};

async fn pick_free_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[tokio::test]
async fn congestion_drives_set_mode_and_mock_acks_mode_applied() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let cport = pick_free_port().await;
    let mport = pick_free_port().await;

    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(
        PairingStore::open_at(tmp.path().join("p.toml"))
            .await
            .unwrap(),
    );
    let key = PairingMaterial::new_random();
    let key_b64 = key.as_b64();
    store.put("E2E-UDID", key).await.unwrap();

    let mock_args = mock_iphone::Args {
        host: "127.0.0.1".into(),
        cport,
        mport,
        token: key_b64.clone(),
        width: 1280,
        height: 720,
        fps: 30,
        send_video: false,
        transport: mock_iphone::TransportMode::UsbListener,
        congested: true,
    };
    let mock_task = tokio::spawn(mock_iphone::run(mock_args));
    tokio::time::sleep(Duration::from_millis(150)).await;

    let lb = Arc::new(LoopbackConductor::new());
    lb.register(LoopbackDevice {
        id: 1,
        udid: "E2E-UDID".into(),
        control_addr: format!("127.0.0.1:{cport}").parse().unwrap(),
        media_addr: format!("127.0.0.1:{mport}").parse().unwrap(),
    })
    .await;

    let app = AppHandle::for_tests_with_pairing(store.clone()).await;
    app.start_usb_supervisor(lb, store, Duration::from_millis(50))
        .await;

    // Wait for Ready.
    let snap = app.snapshot.clone();
    for _ in 0..200 {
        if matches!(snap.borrow().state, SessionStateKind::Ready { .. }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        matches!(snap.borrow().state, SessionStateKind::Ready { .. }),
        "session never reached Ready"
    );

    // Observe events until we see ModeApplied (the full loop closed), or time out.
    let mut events = app.events.lock().await;
    let mut saw_mode_applied = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while tokio::time::Instant::now() < deadline {
        tokio::select! {
            ev = events.recv() => {
                match ev {
                    Some(ControlPlaneEvent::ModeApplied { .. }) => {
                        saw_mode_applied = true;
                        break;
                    }
                    Some(_) => {}  // ignore Telemetry/DeviceInfo/StateChanged/etc.
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
    drop(events);
    assert!(
        saw_mode_applied,
        "expected the adaptation loop to close: congested telemetry → SET_MODE → MODE_APPLIED"
    );

    mock_task.abort();
}
