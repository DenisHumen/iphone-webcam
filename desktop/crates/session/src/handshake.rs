//! Server-side control-plane handshake.
//!
//! Sequence per `docs/03-protocol.md` §4:
//!     ← HELLO         → HELLO_ACK or ERROR(incompatible_version)
//!     ← AUTH          → AUTH_OK or ERROR(unauthorized)
//! After AUTH_OK, the session is "ready" pending media binding.

use std::time::Duration;

use ccp_protocol::{
    Auth, AuthOk, Capability, ControlEnvelope, ControlMessage, ErrorCode, ErrorMsg, Hello,
    HelloAck, PROTO_VER,
};
use tokio::time::timeout;
use tracing::{info, warn};
use transport::ControlStream;
use uuid::Uuid;

use crate::error::SessionError;

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct AcceptedSession {
    pub session_id: String,
    pub token: String,
    pub hello: Hello,
}

pub async fn accept_control(
    stream: &mut ControlStream,
    expected_token: &str,
    server_caps: &[Capability],
) -> Result<AcceptedSession, SessionError> {
    let hello_env = timeout(HANDSHAKE_TIMEOUT, stream.recv())
        .await
        .map_err(|_| SessionError::HandshakeTimeout)??;
    let hello = match hello_env.body {
        ControlMessage::Hello(h) => h,
        other => {
            warn!(?other, "first frame was not HELLO");
            send_error(
                stream,
                hello_env.seq,
                ErrorCode::BadRequest,
                "expected HELLO",
            )
            .await?;
            return Err(SessionError::UnexpectedMessage("expected HELLO"));
        }
    };
    if hello.proto_ver != PROTO_VER {
        send_error(
            stream,
            hello_env.seq,
            ErrorCode::IncompatibleVersion,
            "proto version mismatch",
        )
        .await?;
        return Err(SessionError::IncompatibleProtoVer {
            peer: hello.proto_ver,
            local: PROTO_VER,
        });
    }
    let agreed_caps: Vec<Capability> = server_caps
        .iter()
        .filter(|c| hello.caps.iter().any(|h| h == *c))
        .cloned()
        .collect();
    stream
        .send(&ControlEnvelope {
            seq: 0,
            ack: Some(hello_env.seq),
            body: ControlMessage::HelloAck(HelloAck {
                proto_ver: PROTO_VER,
                caps: agreed_caps,
            }),
        })
        .await?;

    let auth_env = timeout(HANDSHAKE_TIMEOUT, stream.recv())
        .await
        .map_err(|_| SessionError::HandshakeTimeout)??;
    let auth: Auth = match auth_env.body {
        ControlMessage::Auth(a) => a,
        other => {
            warn!(?other, "expected AUTH");
            send_error(stream, auth_env.seq, ErrorCode::BadRequest, "expected AUTH").await?;
            return Err(SessionError::UnexpectedMessage("expected AUTH"));
        }
    };
    let presented_token = match &auth {
        Auth::Token { token } => token.as_str(),
        Auth::PairingKey { .. } => {
            send_error(stream, auth_env.seq, ErrorCode::Unauthorized, "bad token").await?;
            return Err(SessionError::Unauthorized);
        }
    };
    if presented_token != expected_token {
        send_error(stream, auth_env.seq, ErrorCode::Unauthorized, "bad token").await?;
        return Err(SessionError::Unauthorized);
    }
    let session_id = format!("sess-{}", Uuid::new_v4().simple());
    stream
        .send(&ControlEnvelope {
            seq: 1,
            ack: Some(auth_env.seq),
            body: ControlMessage::AuthOk(AuthOk {
                session_id: session_id.clone(),
            }),
        })
        .await?;
    info!(%session_id, device.model = %hello.device.model, "control handshake complete");
    Ok(AcceptedSession {
        session_id,
        token: presented_token.to_owned(),
        hello,
    })
}

/// USB-side handshake variant. Mirrors `accept_control` but expects an
/// `Auth::PairingKey` whose value equals `expected_pairing_key_b64`. The
/// returned `AcceptedSession.token` is set to the pairing key so downstream
/// code (media-stream pairing) sees a single uniform "credential" string.
///
/// Reject paths:
/// - `Auth::Token { .. }` on this path → `SessionError::Unauthorized`
///   (USB sessions must present the pairing key).
/// - `Auth::PairingKey { pairing_key }` where the value mismatches → same.
pub async fn accept_control_usb(
    stream: &mut ControlStream,
    expected_pairing_key_b64: &str,
    server_caps: &[Capability],
) -> Result<AcceptedSession, SessionError> {
    // HELLO / HELLO_ACK
    let hello_env = timeout(HANDSHAKE_TIMEOUT, stream.recv())
        .await
        .map_err(|_| SessionError::HandshakeTimeout)??;
    let hello = match hello_env.body {
        ControlMessage::Hello(h) => h,
        other => {
            warn!(?other, "first frame was not HELLO");
            send_error(
                stream,
                hello_env.seq,
                ErrorCode::BadRequest,
                "expected HELLO",
            )
            .await?;
            return Err(SessionError::UnexpectedMessage("expected HELLO"));
        }
    };
    if hello.proto_ver != PROTO_VER {
        send_error(
            stream,
            hello_env.seq,
            ErrorCode::IncompatibleVersion,
            "proto version mismatch",
        )
        .await?;
        return Err(SessionError::IncompatibleProtoVer {
            peer: hello.proto_ver,
            local: PROTO_VER,
        });
    }
    let agreed_caps: Vec<Capability> = server_caps
        .iter()
        .filter(|c| hello.caps.iter().any(|h| h == *c))
        .cloned()
        .collect();
    stream
        .send(&ControlEnvelope {
            seq: 0,
            ack: Some(hello_env.seq),
            body: ControlMessage::HelloAck(HelloAck {
                proto_ver: PROTO_VER,
                caps: agreed_caps,
            }),
        })
        .await?;

    // AUTH (PairingKey only)
    let auth_env = timeout(HANDSHAKE_TIMEOUT, stream.recv())
        .await
        .map_err(|_| SessionError::HandshakeTimeout)??;
    let auth: Auth = match auth_env.body {
        ControlMessage::Auth(a) => a,
        other => {
            warn!(?other, "expected AUTH");
            send_error(stream, auth_env.seq, ErrorCode::BadRequest, "expected AUTH").await?;
            return Err(SessionError::UnexpectedMessage("expected AUTH"));
        }
    };
    let presented_key = match &auth {
        Auth::PairingKey { pairing_key } => pairing_key.clone(),
        Auth::Token { .. } => {
            send_error(
                stream,
                auth_env.seq,
                ErrorCode::Unauthorized,
                "USB session requires pairingKey, not token",
            )
            .await?;
            return Err(SessionError::Unauthorized);
        }
    };
    if presented_key != expected_pairing_key_b64 {
        send_error(
            stream,
            auth_env.seq,
            ErrorCode::Unauthorized,
            "pairing key mismatch",
        )
        .await?;
        return Err(SessionError::Unauthorized);
    }

    // AUTH_OK
    let session_id = format!("sess-{}", Uuid::new_v4().simple());
    stream
        .send(&ControlEnvelope {
            seq: 1,
            ack: Some(auth_env.seq),
            body: ControlMessage::AuthOk(AuthOk {
                session_id: session_id.clone(),
            }),
        })
        .await?;
    info!(%session_id, device.model = %hello.device.model, "USB control handshake complete");
    Ok(AcceptedSession {
        session_id,
        token: presented_key,
        hello,
    })
}

async fn send_error(
    stream: &mut ControlStream,
    ack: u64,
    code: ErrorCode,
    message: &str,
) -> Result<(), SessionError> {
    stream
        .send(&ControlEnvelope {
            seq: 0,
            ack: Some(ack),
            body: ControlMessage::Error(ErrorMsg {
                code,
                message: message.into(),
            }),
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::DeviceIdent;
    use tokio::io::duplex;
    use transport::{PeerInfo, Source};

    fn dummy_peer() -> PeerInfo {
        PeerInfo {
            addr: "127.0.0.1:0".parse().unwrap(),
            source: Source::Wifi,
        }
    }

    fn make_pair() -> (ControlStream, ControlStream) {
        let (a, b) = duplex(64 * 1024);
        let (ra, wa) = tokio::io::split(a);
        let (rb, wb) = tokio::io::split(b);
        (
            ControlStream::from_halves(dummy_peer(), ra, wa),
            ControlStream::from_halves(dummy_peer(), rb, wb),
        )
    }

    #[tokio::test]
    async fn happy_path_returns_session_id() {
        let (mut srv, mut cli) = make_pair();
        let server = tokio::spawn(async move {
            accept_control(&mut srv, "secret", &[Capability::Hevc])
                .await
                .map(|s| s.session_id)
        });
        cli.send(&ControlEnvelope {
            seq: 10,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "mock/0.1".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "18.0".into(),
                },
                session_id: "candidate".into(),
                caps: vec![Capability::Hevc, Capability::RawNv12],
            }),
        })
        .await
        .unwrap();
        let ack: ControlEnvelope = cli.recv().await.unwrap();
        assert!(matches!(ack.body, ControlMessage::HelloAck(_)));
        cli.send(&ControlEnvelope {
            seq: 11,
            ack: None,
            body: ControlMessage::Auth(Auth::token("secret")),
        })
        .await
        .unwrap();
        let ok = cli.recv().await.unwrap();
        match ok.body {
            ControlMessage::AuthOk(AuthOk { session_id }) => {
                assert!(session_id.starts_with("sess-"));
            }
            other => panic!("unexpected {other:?}"),
        }
        let sid = server.await.unwrap().unwrap();
        assert!(sid.starts_with("sess-"));
    }

    #[tokio::test]
    async fn rejects_wrong_token() {
        let (mut srv, mut cli) = make_pair();
        let server = tokio::spawn(async move { accept_control(&mut srv, "secret", &[]).await });
        cli.send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "x".into(),
                device: DeviceIdent {
                    model: "x".into(),
                    os_ver: "x".into(),
                },
                session_id: "x".into(),
                caps: vec![],
            }),
        })
        .await
        .unwrap();
        let _ack = cli.recv().await.unwrap();
        cli.send(&ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth::token("WRONG")),
        })
        .await
        .unwrap();
        let err = cli.recv().await.unwrap();
        match err.body {
            ControlMessage::Error(ErrorMsg { code, .. }) => {
                assert!(matches!(code, ErrorCode::Unauthorized));
            }
            other => panic!("expected ERROR, got {other:?}"),
        }
        assert!(matches!(
            server.await.unwrap(),
            Err(SessionError::Unauthorized)
        ));
    }

    #[tokio::test]
    async fn rejects_pairing_key_on_wifi() {
        let (mut srv, mut cli) = make_pair();
        let server = tokio::spawn(async move { accept_control(&mut srv, "secret", &[]).await });
        cli.send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "x".into(),
                device: DeviceIdent {
                    model: "x".into(),
                    os_ver: "x".into(),
                },
                session_id: "x".into(),
                caps: vec![],
            }),
        })
        .await
        .unwrap();
        let _ack = cli.recv().await.unwrap();
        cli.send(&ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth::pairing_key("base64keydata")),
        })
        .await
        .unwrap();
        let err = cli.recv().await.unwrap();
        match err.body {
            ControlMessage::Error(ErrorMsg { code, .. }) => {
                assert!(matches!(code, ErrorCode::Unauthorized));
            }
            other => panic!("expected ERROR(Unauthorized), got {other:?}"),
        }
        assert!(matches!(
            server.await.unwrap(),
            Err(SessionError::Unauthorized)
        ));
    }

    async fn drive_hello_authpk_client(client: &mut ControlStream, key_b64: &str) {
        let hello = ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "test-usb".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "iOS 18.0".into(),
                },
                session_id: "candidate".into(),
                caps: vec![],
            }),
        };
        client.send(&hello).await.unwrap();
        let _ack = client.recv().await.unwrap();
        let auth = ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth::pairing_key(key_b64)),
        };
        client.send(&auth).await.unwrap();
        let _ok = client.recv().await.unwrap();
    }

    #[tokio::test]
    async fn accept_control_usb_happy_path() {
        let (mut srv, mut cli) = make_pair();
        let caps = vec![Capability::Hevc];
        let expected = "stored-key-b64".to_string();

        let task =
            tokio::spawn(async move { accept_control_usb(&mut srv, &expected, &caps).await });

        drive_hello_authpk_client(&mut cli, "stored-key-b64").await;

        let result = task.await.unwrap().expect("handshake should succeed");
        assert_eq!(result.token, "stored-key-b64");
    }

    #[tokio::test]
    async fn accept_control_usb_rejects_wrong_pairing_key() {
        let (mut srv, mut cli) = make_pair();
        let caps = vec![Capability::Hevc];
        let expected = "stored-key-b64".to_string();

        let task =
            tokio::spawn(async move { accept_control_usb(&mut srv, &expected, &caps).await });

        // drive_hello_authpk_client sends AUTH_OK recv — but server returns error,
        // so the client recv will get an error frame instead; we don't assert the
        // client side, only the server result.
        let hello = ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "test-usb".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "iOS 18.0".into(),
                },
                session_id: "candidate".into(),
                caps: vec![],
            }),
        };
        cli.send(&hello).await.unwrap();
        let _ack = cli.recv().await.unwrap();
        let auth = ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth::pairing_key("wrong-key")),
        };
        cli.send(&auth).await.unwrap();
        let _err = cli.recv().await.unwrap(); // server sends ERROR(Unauthorized)

        let err = task.await.unwrap().unwrap_err();
        assert!(matches!(err, SessionError::Unauthorized));
    }

    #[tokio::test]
    async fn accept_control_usb_rejects_token_auth() {
        let (mut srv, mut cli) = make_pair();
        let caps: Vec<Capability> = vec![];

        let task = tokio::spawn(async move { accept_control_usb(&mut srv, "k", &caps).await });

        // Client sends HELLO then AUTH with a plain token (legacy Wi-Fi path).
        let hello = ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "test".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "iOS 18.0".into(),
                },
                session_id: "candidate".into(),
                caps: vec![],
            }),
        };
        cli.send(&hello).await.unwrap();
        let _ack = cli.recv().await.unwrap();
        let auth = ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Auth(Auth::token("plain-token")),
        };
        cli.send(&auth).await.unwrap();
        let _err = cli.recv().await.unwrap(); // server sends ERROR(Unauthorized)

        let err = task.await.unwrap().unwrap_err();
        assert!(matches!(err, SessionError::Unauthorized));
    }

    #[tokio::test]
    async fn rejects_proto_mismatch() {
        let (mut srv, mut cli) = make_pair();
        let server = tokio::spawn(async move { accept_control(&mut srv, "secret", &[]).await });
        cli.send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: 999,
                app: "x".into(),
                device: DeviceIdent {
                    model: "x".into(),
                    os_ver: "x".into(),
                },
                session_id: "x".into(),
                caps: vec![],
            }),
        })
        .await
        .unwrap();
        let err = cli.recv().await.unwrap();
        match err.body {
            ControlMessage::Error(ErrorMsg { code, .. }) => {
                assert!(matches!(code, ErrorCode::IncompatibleVersion));
            }
            other => panic!("expected ERROR, got {other:?}"),
        }
        assert!(matches!(
            server.await.unwrap(),
            Err(SessionError::IncompatibleProtoVer { .. })
        ));
    }
}
