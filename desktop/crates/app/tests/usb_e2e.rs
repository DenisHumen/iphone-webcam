//! Phase 5 end-to-end test: a virtual usbmuxd backend (LoopbackConductor)
//! sees a mock-iPhone listener; AppCore dials it and the full Phase 2 RAW
//! pipeline runs through USB. **This test is wired in Task 15** after the
//! mock-iphone library refactor (Tasks 13-14). Until then it is `#[ignore]`d.

#![allow(unused_imports)] // until the test body lands in Task 15

use std::sync::Arc;
use std::time::Duration;

use app::{PairingStore, UsbSupervisor};
use transport::usb::{LoopbackConductor, LoopbackDevice};

#[ignore = "wired up by Task 15; needs mock_iphone library refactor (Task 13)"]
#[tokio::test]
async fn usb_handshake_completes_against_mock_iphone() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    // Set up the iPhone-side listeners (real TCP listeners stand in for the
    // device's NWListener pair) and a LoopbackConductor pointing at them.
    let tmp = tempfile::tempdir().unwrap();
    let _store = PairingStore::open_at(tmp.path().join("p.toml"))
        .await
        .unwrap();
    let _lb = Arc::new(LoopbackConductor::new());
    // Task 15 will: spawn a mock-iphone listener; call
    //   AppHandle::start_usb_supervisor(lb.clone(), store, Duration::from_millis(50)).await;
    // wait for an authed USB session; assert RAW frame flows.
    let _ = (_lb, Duration::from_millis(50));
}
