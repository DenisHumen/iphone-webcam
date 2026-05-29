//! TCP server that accepts control + media connections from the iPhone.
//!
//! Pairing strategy lives in the `session` crate: each accepted control socket
//! gets a `sessionId` from `AUTH_OK`, and the media socket presents
//! `MEDIA_HELLO { sessionId, token }`. The `session` crate looks it up in a
//! pending table and attaches the media stream to the matching session.

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::source::Source;
use crate::streams::{ControlStream, MediaStream, PeerInfo};
use crate::TransportError;

#[derive(Debug)]
pub enum WifiServerEvent {
    Control(ControlStream),
    Media(MediaStream),
}

#[derive(Debug, Clone, Copy)]
pub struct BoundPorts {
    pub control: SocketAddr,
    pub media: SocketAddr,
}

pub struct BoundPortsWithListeners {
    pub bound: BoundPorts,
    control: TcpListener,
    media: TcpListener,
}

impl BoundPortsWithListeners {
    pub async fn bind(
        control_addr: SocketAddr,
        media_addr: SocketAddr,
    ) -> Result<Self, TransportError> {
        let control = TcpListener::bind(control_addr).await?;
        let media = TcpListener::bind(media_addr).await?;
        let bound = BoundPorts {
            control: control.local_addr()?,
            media: media.local_addr()?,
        };
        Ok(Self {
            bound,
            control,
            media,
        })
    }

    pub fn spawn(self, tx: mpsc::Sender<WifiServerEvent>) {
        let Self { control, media, .. } = self;
        let tx_ctl = tx.clone();
        tokio::spawn(async move {
            loop {
                match control.accept().await {
                    Ok((sock, addr)) => {
                        info!(?addr, "accepted control connection");
                        let cs = ControlStream::from_tcp(
                            PeerInfo {
                                addr,
                                source: Source::Wifi,
                            },
                            sock,
                        );
                        if tx_ctl.send(WifiServerEvent::Control(cs)).await.is_err() {
                            return;
                        }
                    }
                    Err(e) => warn!(error = ?e, "control accept failed"),
                }
            }
        });
        tokio::spawn(async move {
            loop {
                match media.accept().await {
                    Ok((sock, addr)) => {
                        info!(?addr, "accepted media connection");
                        let ms = MediaStream::from_tcp(
                            PeerInfo {
                                addr,
                                source: Source::Wifi,
                            },
                            sock,
                        );
                        if tx.send(WifiServerEvent::Media(ms)).await.is_err() {
                            return;
                        }
                    }
                    Err(e) => warn!(error = ?e, "media accept failed"),
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn accepts_control_and_media_connections() {
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

        let _client_ctl = TcpStream::connect(("127.0.0.1", cport)).await.unwrap();
        let _client_media = TcpStream::connect(("127.0.0.1", mport)).await.unwrap();

        let mut saw_control = false;
        let mut saw_media = false;
        for _ in 0..2 {
            match rx.recv().await.unwrap() {
                WifiServerEvent::Control(_) => saw_control = true,
                WifiServerEvent::Media(_) => saw_media = true,
            }
        }
        assert!(saw_control && saw_media);
    }
}
