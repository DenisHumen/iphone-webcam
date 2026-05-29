//! Wi-Fi client used by `tools/mock-iphone` and integration tests.

use std::net::SocketAddr;

use tokio::net::TcpStream;

use crate::source::Source;
use crate::streams::{ControlStream, MediaStream, PeerInfo};
use crate::TransportError;

pub async fn connect(
    control: SocketAddr,
    media: SocketAddr,
) -> Result<(ControlStream, MediaStream), TransportError> {
    let control_sock = TcpStream::connect(control).await?;
    let media_sock = TcpStream::connect(media).await?;
    let control_peer = PeerInfo {
        addr: control_sock.peer_addr()?,
        source: Source::Wifi,
    };
    let media_peer = PeerInfo {
        addr: media_sock.peer_addr()?,
        source: Source::Wifi,
    };
    Ok((
        ControlStream::from_tcp(control_peer, control_sock),
        MediaStream::from_tcp(media_peer, media_sock),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wifi::server::{BoundPortsWithListeners, WifiServerEvent};
    use ccp_protocol::{Bye, ControlEnvelope, ControlMessage};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn round_trip_through_real_tcp_loopback() {
        let bound = BoundPortsWithListeners::bind(
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        let cport = bound.bound.control.port();
        let mport = bound.bound.media.port();
        let (tx, mut rx) = mpsc::channel(8);
        bound.spawn(tx);

        let (mut client_ctl, _client_media) = connect(
            format!("127.0.0.1:{cport}").parse().unwrap(),
            format!("127.0.0.1:{mport}").parse().unwrap(),
        )
        .await
        .unwrap();

        // Drain events until we get the server-side control stream.
        let mut server_ctl = loop {
            match rx.recv().await.unwrap() {
                WifiServerEvent::Control(c) => break c,
                WifiServerEvent::Media(_) => continue,
            }
        };

        let env = ControlEnvelope {
            seq: 7,
            ack: None,
            body: ControlMessage::Bye(Bye {
                reason: "ok".into(),
            }),
        };
        client_ctl.send(&env).await.unwrap();
        let got = server_ctl.recv().await.unwrap();
        assert_eq!(got, env);
    }
}
