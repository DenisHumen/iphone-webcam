//! Camera switch e2e: AppHandle::set_camera should put a SET_CAMERA envelope
//! onto the active session's control wire.

use std::time::Duration;

use app::AppCore;
use ccp_protocol::{
    Auth, Capability, ControlEnvelope, ControlMessage, DeviceIdent, Hello, PROTO_VER,
};
use tokio::time::sleep;
use transport::wifi_connect;

#[tokio::test]
async fn set_camera_command_lands_as_set_camera_envelope() {
    let core = AppCore {
        host: "127.0.0.1".into(),
    };
    let handle = core.start().await.unwrap();
    let token = handle.qr.token.clone();
    let (mut control, _media) = wifi_connect(
        format!("127.0.0.1:{}", handle.qr.cport).parse().unwrap(),
        format!("127.0.0.1:{}", handle.qr.mport).parse().unwrap(),
    )
    .await
    .unwrap();

    control
        .send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "t".into(),
                device: DeviceIdent {
                    model: "m".into(),
                    os_ver: "v".into(),
                },
                session_id: "x".into(),
                caps: vec![Capability::RawNv12],
            }),
        })
        .await
        .unwrap();
    let _ = control.recv().await.unwrap();

    control
        .send(&ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth::token(token)),
        })
        .await
        .unwrap();
    let auth_ok = control.recv().await.unwrap();
    assert!(matches!(auth_ok.body, ControlMessage::AuthOk(_)));

    // Wait briefly for the ControlPlane to register its outbound sender.
    for _ in 0..50 {
        if handle.set_camera("wide".into()).await.is_ok() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    // Drain envelopes until we see SET_CAMERA(cameraId="wide").
    for _ in 0..50 {
        let got = tokio::time::timeout(Duration::from_millis(200), control.recv()).await;
        match got {
            Ok(Ok(env)) => {
                if let ControlMessage::SetCamera(sc) = env.body {
                    assert_eq!(sc.camera_id, "wide");
                    handle.shutdown();
                    return;
                }
                // ignore PING / other side-band traffic
            }
            _ => break,
        }
    }
    panic!("SET_CAMERA envelope never observed");
}
