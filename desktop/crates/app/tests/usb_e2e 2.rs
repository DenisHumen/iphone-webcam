//! Phase 5 end-to-end test: a virtual usbmuxd backend (LoopbackConductor)
//! sees a mock-iPhone listener; the desktop side dials it via UsbSupervisor
//! and receives a DialedStreams event.  The assertion stops at DialedStreams
//! because the full USB-side session handshake is a Phase 6 deliverable
//! (Task 12 left a TODO comment marking that boundary explicitly).

use std::sync::Arc;
use std::time::Duration;

use app::{PairingStore, UsbSupervisor};
use transport::usb::{LoopbackConductor, LoopbackDevice};
use transport::Source;

/// Bind a TCP listener on an ephemeral port, record the port, drop the
/// listener so mock-iphone can re-bind it.
async fn pick_free_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[tokio::test]
async fn supervisor_delivers_dialed_streams_for_usb_device() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    // Pick ephemeral ports that mock-iphone will bind.
    let cport = pick_free_port().await;
    let mport = pick_free_port().await;

    // Spawn the mock-iphone in USB-listener mode.  It binds cport + mport and
    // waits for the desktop to dial in.  We don't send video and don't care
    // about the full session handshake at this scope; the test asserts only
    // that DialedStreams arrives (the TCP connect succeeded).
    let mock_args = mock_iphone::Args {
        host: "127.0.0.1".into(),
        cport,
        mport,
        token: "phase5-test-token".into(),
        width: 1280,
        height: 720,
        fps: 30,
        send_video: false,
        transport: mock_iphone::TransportMode::UsbListener,
    };
    let mock_task = tokio::spawn(mock_iphone::run(mock_args));

    // Give the mock-iphone listeners a moment to bind before the desktop dials.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // LoopbackConductor routes open_pair's open_port(_, CCP_USB_CONTROL_PORT)
    // and open_port(_, CCP_USB_MEDIA_PORT) to the ephemeral addresses above.
    // open_pair() dials the hardcoded CCP ports (7000/7001); LoopbackConductor
    // checks `requested_port == self.control_port` (default 7000) and routes to
    // device.control_addr.  So we keep the default conductor port routing and
    // supply the actual listener addresses via LoopbackDevice.
    let lb = Arc::new(LoopbackConductor::new());
    lb.register(LoopbackDevice {
        id: 1,
        udid: "TEST-UDID".into(),
        control_addr: format!("127.0.0.1:{cport}").parse().unwrap(),
        media_addr: format!("127.0.0.1:{mport}").parse().unwrap(),
    })
    .await;

    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(
        PairingStore::open_at(tmp.path().join("p.toml"))
            .await
            .unwrap(),
    );

    // Use UsbSupervisor directly so this test owns the receiver end of the
    // DialedStreams channel — no AppHandle indirection needed.
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let sup = UsbSupervisor::new(lb, store, tx);
    let _handle = sup.spawn(Duration::from_millis(50));

    let dialed = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timeout waiting for DialedStreams — supervisor did not dial within 3 s")
        .expect("channel closed before DialedStreams arrived");

    assert_eq!(dialed.udid, "TEST-UDID");
    assert_eq!(
        dialed.peer().source,
        Source::Usb {
            udid: "TEST-UDID".into()
        }
    );
    assert!(
        dialed.pairing_key_b64.is_none(),
        "fresh pairing store should have no key for TEST-UDID"
    );

    mock_task.abort();
}
