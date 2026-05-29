use std::pin::Pin;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use super::error::UsbTransportError;

pub type BoxedReader = Pin<Box<dyn AsyncRead + Send + Unpin>>;
pub type BoxedWriter = Pin<Box<dyn AsyncWrite + Send + Unpin>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    /// Some backends (idevice) also report network-only devices; we skip them.
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDevice {
    pub id: u32,      // usbmuxd device id (stable while plugged in)
    pub udid: String, // 25–40 char Apple UDID
    pub connection: ConnectionType,
    pub product_id: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbEvent {
    Connected(UsbDevice),
    Lost { id: u32, udid: String },
}

#[async_trait]
pub trait UsbConductor: Send + Sync + 'static {
    async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError>;

    async fn open_port(
        &self,
        device_id: u32,
        port: u16,
        label: &str,
    ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError>;

    /// Polling fallback: default implementation diffs `list_devices()` every
    /// `poll_interval`. Backends that have a native event stream (idevice's
    /// `subscribe_events`) should override.
    fn subscribe(
        self: std::sync::Arc<Self>,
        poll_interval: std::time::Duration,
    ) -> mpsc::Receiver<UsbEvent>
    where
        Self: Sized,
    {
        let (tx, rx) = mpsc::channel(16);
        let me = self.clone();
        tokio::spawn(async move {
            let mut prev: Vec<UsbDevice> = Vec::new();
            loop {
                tokio::time::sleep(poll_interval).await;
                let now = match me.list_devices().await {
                    Ok(d) => d
                        .into_iter()
                        .filter(|d| matches!(d.connection, ConnectionType::Usb))
                        .collect::<Vec<_>>(),
                    Err(e) => {
                        tracing::debug!(error = %e, "usb list_devices failed");
                        continue;
                    }
                };
                // emit "connected" for new ids
                for d in &now {
                    if !prev.iter().any(|p| p.id == d.id) {
                        let _ = tx.send(UsbEvent::Connected(d.clone())).await;
                    }
                }
                // emit "lost" for missing ids
                for p in &prev {
                    if !now.iter().any(|d| d.id == p.id) {
                        let _ = tx
                            .send(UsbEvent::Lost {
                                id: p.id,
                                udid: p.udid.clone(),
                            })
                            .await;
                    }
                }
                prev = now;
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Minimal in-test stub: returns one device, supports no actual TCP.
    #[derive(Default, Clone)]
    struct StubConductor {
        device: Arc<Mutex<Option<UsbDevice>>>,
    }

    #[async_trait::async_trait]
    impl UsbConductor for StubConductor {
        async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError> {
            Ok(self.device.lock().await.iter().cloned().collect())
        }
        async fn open_port(
            &self,
            _device_id: u32,
            _port: u16,
            _label: &str,
        ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError> {
            Err(UsbTransportError::Unsupported)
        }
    }

    #[tokio::test]
    async fn stub_lists_zero_or_one_device() {
        let stub = StubConductor::default();
        assert!(stub.list_devices().await.unwrap().is_empty());
        *stub.device.lock().await = Some(UsbDevice {
            id: 7,
            udid: "ABCD".into(),
            connection: ConnectionType::Usb,
            product_id: Some(0x12A8),
        });
        let v = stub.list_devices().await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].udid, "ABCD");
    }
}
