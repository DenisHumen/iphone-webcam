//! Phase 6b live speedtest: desktop run_speedtest() sends SPEEDTEST_START; the
//! mock-iphone ramps filler frames over media; the SpeedTestCounter tallies
//! per-step bytes; evaluate() yields a positive goodput.

use std::sync::Arc;
use std::time::Duration;

use app::{AppHandle, PairingMaterial, PairingStore};
use session::SessionStateKind;
use transport::usb::{LoopbackConductor, LoopbackDevice};

async fn pick_free_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[tokio::test]
async fn run_speedtest_measures_positive_goodput_over_usb() {
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
    store.put("ST-UDID", key).await.unwrap();

    let mock_args = mock_iphone::Args {
        host: "127.0.0.1".into(),
        cport,
        mport,
        token: key_b64.clone(),
        width: 1280,
        height: 720,
        fps: 30,
        send_video: false, // no steady video; only the ramp
        transport: mock_iphone::TransportMode::UsbListener,
        congested: false,
    };
    let mock_task = tokio::spawn(mock_iphone::run(mock_args));
    tokio::time::sleep(Duration::from_millis(150)).await;

    let lb = Arc::new(LoopbackConductor::new());
    lb.register(LoopbackDevice {
        id: 1,
        udid: "ST-UDID".into(),
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
        "session never reached Ready; state = {:?}",
        snap.borrow().state
    );

    // Run the live speedtest.
    let outcome = app.run_speedtest().await.expect("speedtest");
    assert!(
        outcome.measurement.goodput_mbps > 0.0,
        "expected positive goodput, got {:?}",
        outcome.measurement
    );

    mock_task.abort();
}
