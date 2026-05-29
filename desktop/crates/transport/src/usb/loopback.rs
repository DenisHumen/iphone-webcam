//! In-process stand-in for `usbmuxd`. Tests can register virtual devices,
//! each of which points to a (real) TCP listener on `127.0.0.1`. `open_port`
//! routes by the protocol's `cport`/`mport` constants:
//! * any `port == ports.control` → dials `device.control_addr`
//! * any `port == ports.media`   → dials `device.media_addr`
//!
//! This lets the rest of the test stack (handshake, media pipeline) run end-
//! to-end against a real socketpair without depending on a real iPhone.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use super::conductor::{BoxedReader, BoxedWriter, ConnectionType, UsbConductor, UsbDevice};
use super::error::UsbTransportError;

#[derive(Debug, Clone)]
pub struct LoopbackDevice {
    pub id: u32,
    pub udid: String,
    pub control_addr: SocketAddr,
    pub media_addr: SocketAddr,
}

#[derive(Default)]
pub struct LoopbackConductor {
    devices: Arc<Mutex<Vec<LoopbackDevice>>>,
    /// CCP fixed ports (mirror docs/03 §2). Set on construction; tests can
    /// override via `with_ports`.
    pub control_port: u16,
    pub media_port: u16,
}

impl LoopbackConductor {
    pub const DEFAULT_CONTROL_PORT: u16 = 7000;
    pub const DEFAULT_MEDIA_PORT: u16 = 7001;

    pub fn new() -> Self {
        Self {
            devices: Arc::new(Mutex::new(Vec::new())),
            control_port: Self::DEFAULT_CONTROL_PORT,
            media_port: Self::DEFAULT_MEDIA_PORT,
        }
    }

    pub fn with_ports(mut self, control: u16, media: u16) -> Self {
        self.control_port = control;
        self.media_port = media;
        self
    }

    pub async fn register(&self, dev: LoopbackDevice) {
        self.devices.lock().await.push(dev);
    }

    pub async fn unregister(&self, id: u32) {
        self.devices.lock().await.retain(|d| d.id != id);
    }

    pub fn arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Convenience: drives the polling-based `subscribe` once and returns the
    /// initial snapshot (used by deterministic tests instead of sleeping).
    pub async fn snapshot(&self) -> Vec<UsbDevice> {
        self.list_devices().await.unwrap_or_default()
    }
}

#[async_trait]
impl UsbConductor for LoopbackConductor {
    async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError> {
        let lock = self.devices.lock().await;
        Ok(lock
            .iter()
            .map(|d| UsbDevice {
                id: d.id,
                udid: d.udid.clone(),
                connection: ConnectionType::Usb,
                product_id: None,
            })
            .collect())
    }

    async fn open_port(
        &self,
        device_id: u32,
        port: u16,
        _label: &str,
    ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError> {
        let lock = self.devices.lock().await;
        let dev = lock
            .iter()
            .find(|d| d.id == device_id)
            .cloned()
            .ok_or_else(|| UsbTransportError::DeviceNotFound {
                udid: format!("#{device_id}"),
            })?;
        drop(lock);
        let target = if port == self.control_port {
            dev.control_addr
        } else if port == self.media_port {
            dev.media_addr
        } else {
            return Err(UsbTransportError::ConnectFailed {
                port,
                reason: "loopback conductor only routes the two CCP ports".into(),
            });
        };
        let sock = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(target))
            .await
            .map_err(|_| UsbTransportError::ConnectFailed {
                port,
                reason: "timeout".into(),
            })?
            .map_err(|e| UsbTransportError::ConnectFailed {
                port,
                reason: e.to_string(),
            })?;
        let (r, w) = tokio::io::split(sock);
        Ok((Box::pin(r), Box::pin(w)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn list_devices_returns_registered() {
        let lb = LoopbackConductor::new();
        lb.register(LoopbackDevice {
            id: 1,
            udid: "TEST-UDID".into(),
            control_addr: "127.0.0.1:0".parse().unwrap(),
            media_addr: "127.0.0.1:0".parse().unwrap(),
        })
        .await;
        let devs = lb.list_devices().await.unwrap();
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].udid, "TEST-UDID");
    }

    #[tokio::test]
    async fn open_port_dials_registered_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let lb = LoopbackConductor::new();
        lb.register(LoopbackDevice {
            id: 7,
            udid: "X".into(),
            control_addr: addr,
            media_addr: "127.0.0.1:0".parse().unwrap(),
        })
        .await;

        let dial = tokio::spawn(async move {
            let (mut r, mut w) = lb.open_port(7, 7000, "test").await.unwrap();
            w.write_all(b"hello").await.unwrap();
            let mut buf = [0u8; 5];
            r.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"world");
        });

        let (mut server_sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 5];
        server_sock.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
        server_sock.write_all(b"world").await.unwrap();
        dial.await.unwrap();
    }
}
