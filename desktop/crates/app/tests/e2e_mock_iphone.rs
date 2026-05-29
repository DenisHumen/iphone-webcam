//! End-to-end Phase 1 acceptance: a mock iPhone speaks CCP into a live
//! AppCore, the server-side ControlPlane drives the session to Ready, and
//! the SessionSnapshot reflects the device.

use std::time::Duration;

use app::AppCore;
use ccp_protocol::{
    Auth, BatteryState, Capability, ControlEnvelope, ControlMessage, DeviceIdent, DeviceInfo,
    Hello, ThermalState, PROTO_VER,
};
use session::{write_media_hello, MediaBinding, SessionStateKind};
use tokio::time::sleep;
use transport::wifi_connect;

#[tokio::test]
async fn mock_iphone_drives_session_to_ready() {
    let core = AppCore {
        host: "127.0.0.1".into(),
    };
    let handle = core.start().await.unwrap();
    let cport = handle.qr.cport;
    let mport = handle.qr.mport;
    let token = handle.qr.token.clone();

    let (mut control, mut media) = wifi_connect(
        format!("127.0.0.1:{cport}").parse().unwrap(),
        format!("127.0.0.1:{mport}").parse().unwrap(),
    )
    .await
    .unwrap();

    control
        .send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "test/0.1".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "18.0".into(),
                },
                session_id: "x".into(),
                caps: vec![Capability::RawNv12],
            }),
        })
        .await
        .unwrap();
    let _hello_ack = control.recv().await.unwrap();

    control
        .send(&ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth::token(token.clone())),
        })
        .await
        .unwrap();
    let auth_ok = control.recv().await.unwrap();
    let session_id = match auth_ok.body {
        ControlMessage::AuthOk(ok) => ok.session_id,
        other => panic!("expected AUTH_OK, got {other:?}"),
    };

    write_media_hello(&mut media, &MediaBinding { session_id, token })
        .await
        .unwrap();

    control
        .send(&ControlEnvelope {
            seq: 3,
            ack: None,
            body: ControlMessage::DeviceInfo(DeviceInfo {
                model: "iPhone15,3".into(),
                os_ver: "18.0".into(),
                battery_level: 0.9,
                battery_state: BatteryState::Unplugged,
                thermal_state: ThermalState::Nominal,
                usb3_capable: true,
            }),
        })
        .await
        .unwrap();

    let mut snap = handle.snapshot.clone();
    for _ in 0..50 {
        sleep(Duration::from_millis(50)).await;
        let s = snap.borrow_and_update().clone();
        if matches!(s.state, SessionStateKind::Ready) {
            if let Some(device) = s.device.as_ref() {
                assert_eq!(device.model, "iPhone15,3");
                handle.shutdown();
                return;
            }
        }
    }
    panic!(
        "session never reached Ready with device info: {:?}",
        *snap.borrow()
    );
}
