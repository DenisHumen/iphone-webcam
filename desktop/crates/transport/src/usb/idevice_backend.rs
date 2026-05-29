//! Pure-Rust production backend over `idevice` crate (ADR-019 / ADR-022).
//! Compiled only when `usb-idevice` feature is on.

#![cfg(feature = "usb-idevice")]

use async_trait::async_trait;
use idevice::usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdDevice};

use super::conductor::{BoxedReader, BoxedWriter, ConnectionType, UsbConductor, UsbDevice};
use super::error::UsbTransportError;

pub struct IdeviceConductor {
    addr: UsbmuxdAddr,
}

impl IdeviceConductor {
    pub fn new() -> Result<Self, UsbTransportError> {
        let addr = UsbmuxdAddr::from_env_var().unwrap_or_else(|_| UsbmuxdAddr::default());
        Ok(Self { addr })
    }
}

impl Default for IdeviceConductor {
    fn default() -> Self {
        Self::new().expect("IdeviceConductor::default() failed to construct addr")
    }
}

#[async_trait]
impl UsbConductor for IdeviceConductor {
    async fn list_devices(&self) -> Result<Vec<UsbDevice>, UsbTransportError> {
        let mut muxd = self
            .addr
            .connect(0)
            .await
            .map_err(|e| UsbTransportError::MuxdUnreachable(e.to_string()))?;
        let devs = muxd
            .get_devices()
            .await
            .map_err(|e| UsbTransportError::Idevice(e.to_string()))?;
        Ok(devs.into_iter().map(map_device).collect())
    }

    async fn open_port(
        &self,
        device_id: u32,
        port: u16,
        label: &str,
    ) -> Result<(BoxedReader, BoxedWriter), UsbTransportError> {
        // `connect_to_device` consumes `muxd` (takes `self` by value), so we
        // create a fresh connection per call — that is exactly what the helper
        // in the original plan did via `new_muxd()`.
        let muxd = self
            .addr
            .connect(0)
            .await
            .map_err(|e| UsbTransportError::MuxdUnreachable(e.to_string()))?;

        let idev = muxd
            .connect_to_device(device_id, port, label)
            .await
            .map_err(|e| UsbTransportError::ConnectFailed {
                port,
                reason: e.to_string(),
            })?;

        // `Idevice::get_socket` takes `self` by value and returns the inner
        // `Box<dyn ReadWrite>` (ADR-022 / verified in idevice-0.1.61/src/lib.rs).
        let stream = idev
            .get_socket()
            .ok_or_else(|| UsbTransportError::ConnectFailed {
                port,
                reason: "idevice returned no socket".into(),
            })?;

        let (r, w) = tokio::io::split(stream);
        Ok((Box::pin(r), Box::pin(w)))
    }
}

fn map_device(d: UsbmuxdDevice) -> UsbDevice {
    // idevice 0.1.61: UsbmuxdDevice has {device_id: u32, udid: String,
    // connection_type: Connection}. There is no product_id field.
    let connection = match &d.connection_type {
        Connection::Usb => ConnectionType::Usb,
        Connection::Network(_) | Connection::Unknown(_) => ConnectionType::Network,
    };
    UsbDevice {
        id: d.device_id,
        udid: d.udid,
        connection,
        product_id: None, // idevice 0.1.61 does not expose product_id via UsbmuxdDevice
    }
}
