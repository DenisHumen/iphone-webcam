//! Adapter that turns a conductor + device-id into the canonical
//! `(ControlStream, MediaStream)` pair the rest of the stack expects.

use std::net::Ipv4Addr;

use crate::source::Source;
use crate::streams::{ControlStream, MediaStream, PeerInfo};

use super::conductor::UsbConductor;
use super::error::UsbTransportError;

pub const CCP_USB_CONTROL_PORT: u16 = 7000;
pub const CCP_USB_MEDIA_PORT: u16 = 7001;

/// Open control + media streams to the given device through `conductor`.
/// On any failure half-way through, the partial control stream is dropped
/// (closes the socket) — there is no resource leak.
pub async fn open_pair<C: UsbConductor + ?Sized>(
    conductor: &C,
    device_id: u32,
    udid: &str,
    label: &str,
) -> Result<(ControlStream, MediaStream), UsbTransportError> {
    let (cr, cw) = conductor
        .open_port(device_id, CCP_USB_CONTROL_PORT, label)
        .await?;
    let (mr, mw) = conductor
        .open_port(device_id, CCP_USB_MEDIA_PORT, label)
        .await?;
    let peer_ctl = PeerInfo {
        // usbmuxd doesn't surface a real socket addr; we use 0.0.0.0:0 as a
        // sentinel and rely on `source` for routing decisions.
        addr: (Ipv4Addr::UNSPECIFIED, 0).into(),
        source: Source::Usb { udid: udid.into() },
    };
    let peer_media = peer_ctl.clone();
    let control = ControlStream::from_halves(peer_ctl, cr, cw);
    let media = MediaStream::from_halves(peer_media, mr, mw);
    Ok((control, media))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::loopback::{LoopbackConductor, LoopbackDevice};
    use crate::wifi::server::BoundPortsWithListeners;
    use crate::WifiServerEvent;
    use ccp_protocol::{Bye, ControlEnvelope, ControlMessage};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn dial_pair_then_round_trip_control_envelope() {
        // Use the existing Wi-Fi server listeners as a stand-in for "the
        // service listening inside the device": they bind real TCP ports,
        // accept connections, give us back a ControlStream/MediaStream.
        let bound = BoundPortsWithListeners::bind(
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        let cport = bound.bound.control;
        let mport = bound.bound.media;
        let (tx, mut rx) = mpsc::channel(8);
        bound.spawn(tx);

        // LoopbackConductor routes open_port(_, 7000, _) → device.control_addr
        // and open_port(_, 7001, _) → device.media_addr by default.
        // open_pair() dials CCP_USB_CONTROL_PORT (7000) and CCP_USB_MEDIA_PORT (7001),
        // so we keep the default port routing keys and just supply the real listener
        // addresses in the device record.
        let lb = LoopbackConductor::new();
        lb.register(LoopbackDevice {
            id: 1,
            udid: "UDID-1".into(),
            control_addr: cport,
            media_addr: mport,
        })
        .await;

        let (mut client_ctl, _client_media) = open_pair(&lb, 1, "UDID-1", "test").await.unwrap();
        assert_eq!(
            client_ctl.peer.source,
            crate::Source::Usb {
                udid: "UDID-1".into()
            }
        );

        // Drain events until we see the server-side control stream.
        let mut server_ctl = loop {
            match rx.recv().await.unwrap() {
                WifiServerEvent::Control(c) => break c,
                WifiServerEvent::Media(_) => continue,
            }
        };

        let env = ControlEnvelope {
            seq: 42,
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
