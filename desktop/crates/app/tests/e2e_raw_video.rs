//! End-to-end Phase 2 acceptance: a mock iPhone speaks CCP, sends RAW NV12
//! frames through the media socket, and AppCore's MediaPipeline forwards them
//! to a registered `FrameSink`.

use std::sync::Arc;
use std::time::Duration;

use app::AppCore;
use async_trait::async_trait;
use ccp_protocol::{
    Auth, Capability, Codec, ControlEnvelope, ControlMessage, DeviceIdent, Flags, Hello,
    MediaHeader, MediaType, PROTO_VER,
};
use mediapipeline::write_media_frame;
use session::{write_media_hello, MediaBinding};
use sink::{Frame, FrameSink};
use tokio::sync::Mutex;
use tokio::time::sleep;
use transport::wifi_connect;

struct CapturingSink {
    inner: Arc<Mutex<Vec<Frame>>>,
}

#[async_trait]
impl FrameSink for CapturingSink {
    async fn submit(&self, f: Frame) {
        self.inner.lock().await.push(f);
    }
}

#[tokio::test]
async fn mock_iphone_streams_raw_frames_to_sink() {
    let core = AppCore {
        host: "127.0.0.1".into(),
    };
    let handle = core.start().await.unwrap();
    let captured = Arc::new(Mutex::new(Vec::<Frame>::new()));
    handle
        .register_sink(Arc::new(CapturingSink {
            inner: captured.clone(),
        }))
        .await;

    let token = handle.qr.token.clone();
    let (mut control, mut media) = wifi_connect(
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
                app: "test/0.1".into(),
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
            body: ControlMessage::Auth(Auth::token(token.clone())),
        })
        .await
        .unwrap();
    let ok = control.recv().await.unwrap();
    let sid = match ok.body {
        ControlMessage::AuthOk(o) => o.session_id,
        other => panic!("unexpected {other:?}"),
    };
    write_media_hello(
        &mut media,
        &MediaBinding {
            session_id: sid,
            token,
        },
    )
    .await
    .unwrap();

    // Send 5 raw NV12 frames (16x16, 384-byte payload each).
    for seq in 0..5u32 {
        let header = MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::FULL_RANGE,
            codec: Codec::Raw,
            width: 16,
            height: 16,
            seq,
            pts_usec: u64::from(seq) * 33_000,
            payload_len: 384,
        };
        let payload = vec![seq as u8; 384];
        write_media_frame(&mut media.writer, &header, &payload)
            .await
            .unwrap();
    }

    for _ in 0..50 {
        sleep(Duration::from_millis(50)).await;
        if captured.lock().await.len() >= 5 {
            break;
        }
    }
    let got = captured.lock().await;
    assert_eq!(got.len(), 5, "expected 5 frames, got {}", got.len());
    assert_eq!(got[0].seq, 0);
    assert_eq!(got[4].seq, 4);
    assert_eq!(got[0].plane_y.len(), 256);
    assert_eq!(got[0].plane_uv.len(), 128);
    assert!(got[0].full_range);
    handle.shutdown();
}
