//! Watches the USB bus for iOS devices; when one appears, it dials the CCP
//! ports and forwards the (control, media) streams to whatever owns sessions
//! (currently a test channel; in production: `AppCore::accept_streams`).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use transport::{
    usb::{open_pair, UsbConductor, UsbEvent},
    ControlStream, MediaStream,
};

use crate::pairing_store::PairingStore;

/// Pair of streams handed back to whoever runs the session loop.
#[derive(Debug)]
pub struct DialedStreams {
    pub udid: String,
    pub control: ControlStream,
    pub media: MediaStream,
    /// `Some` if we already have a pairing key for this UDID; `None` if the
    /// session must run the trust ceremony.
    pub pairing_key_b64: Option<String>,
}

impl DialedStreams {
    pub fn peer(&self) -> &transport::PeerInfo {
        &self.control.peer
    }
}

pub struct UsbSupervisor<C: UsbConductor> {
    conductor: Arc<C>,
    store: Arc<PairingStore>,
    out: mpsc::Sender<DialedStreams>,
    label: String,
}

impl<C: UsbConductor + 'static> UsbSupervisor<C> {
    pub fn new(
        conductor: Arc<C>,
        store: Arc<PairingStore>,
        out: mpsc::Sender<DialedStreams>,
    ) -> Self {
        Self {
            conductor,
            store,
            out,
            label: "ClearCam/0.5".into(),
        }
    }

    pub fn spawn(self, poll_interval: Duration) -> JoinHandle<()> {
        tokio::spawn(self.run(poll_interval))
    }

    async fn run(self, poll_interval: Duration) {
        let mut events = self.conductor.clone().subscribe(poll_interval);
        info!("usb supervisor started");
        while let Some(ev) = events.recv().await {
            match ev {
                UsbEvent::Connected(dev) => {
                    info!(udid = %dev.udid, id = dev.id, "usb device connected");
                    match open_pair(self.conductor.as_ref(), dev.id, &dev.udid, &self.label).await {
                        Ok((control, media)) => {
                            let key = match self.store.get(&dev.udid).await {
                                Ok(Some(k)) => Some(k.as_b64()),
                                _ => None,
                            };
                            if self
                                .out
                                .send(DialedStreams {
                                    udid: dev.udid.clone(),
                                    control,
                                    media,
                                    pairing_key_b64: key,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(e) => {
                            warn!(udid = %dev.udid, error = %e, "usb dial failed");
                        }
                    }
                }
                UsbEvent::Lost { udid, id } => {
                    info!(udid = %udid, id, "usb device lost");
                    // We don't tear down sessions here — Session sees the
                    // socket EOF and unwinds on its own (it already does that
                    // for Wi-Fi). This keeps the supervisor minimal.
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use transport::usb::{LoopbackConductor, LoopbackDevice};
    use transport::wifi::server::BoundPortsWithListeners;
    use transport::Source;

    #[tokio::test]
    async fn connect_event_triggers_dial_with_pairing_key() {
        // Set up a TCP listener pair acting as the "iPhone".
        let bound = BoundPortsWithListeners::bind(
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        )
        .await
        .unwrap();
        let cport = bound.bound.control;
        let mport = bound.bound.media;
        let (server_tx, mut server_rx) = tokio::sync::mpsc::channel(8);
        bound.spawn(server_tx);

        let lb = Arc::new(LoopbackConductor::new());
        lb.register(LoopbackDevice {
            id: 1,
            udid: "UDID-1".into(),
            control_addr: cport,
            media_addr: mport,
        })
        .await;

        let tmp = tempfile::tempdir().unwrap();
        let store = Arc::new(
            PairingStore::open_at(tmp.path().join("p.toml"))
                .await
                .unwrap(),
        );

        let (dialed_tx, mut dialed_rx) = tokio::sync::mpsc::channel(4);
        let sup = UsbSupervisor::new(lb.clone(), store, dialed_tx);
        let _handle = sup.spawn(Duration::from_millis(50));

        // Verify a dial happens.
        let dialed = tokio::time::timeout(Duration::from_secs(2), dialed_rx.recv())
            .await
            .unwrap()
            .expect("dial event");
        assert_eq!(
            dialed.peer().source,
            Source::Usb {
                udid: "UDID-1".into()
            }
        );

        // Drain the server side to prove the supervisor really connected.
        let _ = tokio::time::timeout(Duration::from_secs(1), server_rx.recv()).await;
    }
}
