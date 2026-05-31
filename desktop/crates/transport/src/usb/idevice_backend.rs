//! Pure-Rust production backend over `idevice` crate (ADR-019 / ADR-022).
//! Compiled only when `usb-idevice` feature is on.

#![cfg(feature = "usb-idevice")]

use async_trait::async_trait;
use idevice::usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdConnection, UsbmuxdDevice};

use super::conductor::{
    BoxedReader, BoxedWriter, ConnectionType, UsbConductor, UsbDevice, UsbEvent,
};
use super::error::UsbTransportError;

pub struct IdeviceConductor {
    addr: UsbmuxdAddr,
}

impl IdeviceConductor {
    pub fn new() -> Result<Self, UsbTransportError> {
        let addr = UsbmuxdAddr::from_env_var().unwrap_or_else(|_| UsbmuxdAddr::default());
        Ok(Self { addr })
    }

    /// Open a fresh usbmuxd Unix-socket connection. `idevice` consumes the
    /// `UsbmuxdConnection` on every operation (e.g. `connect_to_device` takes
    /// `self`), so we re-dial per-call rather than caching one.
    async fn new_muxd(&self) -> Result<UsbmuxdConnection, UsbTransportError> {
        self.addr
            .connect(0)
            .await
            .map_err(|e| UsbTransportError::MuxdUnreachable(e.to_string()))
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
        let mut muxd = self.new_muxd().await?;
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
        // `connect_to_device` consumes `muxd` (takes `self` by value), hence
        // a fresh muxd per call.
        let muxd = self.new_muxd().await?;

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

    fn subscribe(
        self: std::sync::Arc<Self>,
        _poll_interval: std::time::Duration,
    ) -> tokio::sync::mpsc::Receiver<UsbEvent>
    where
        Self: Sized,
    {
        use futures::StreamExt;
        use idevice::usbmuxd::UsbmuxdListenEvent;
        use std::collections::HashMap;

        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let addr = self.addr.clone();

        // `muxd.listen()` returns `Pin<Box<dyn Stream<...> + 'a>>` without `+Send`,
        // so we can't use `tokio::spawn`. Run the listener in a dedicated OS thread
        // with its own single-threaded Tokio runtime — this is safe because the
        // underlying `ReadWrite` socket IS Send and Sync; only the type annotation
        // omits the bound.
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::warn!(error = %e, "usb subscribe: failed to build runtime");
                    return;
                }
            };
            rt.block_on(async move {
                // Own the muxd connection for the whole listen lifetime; the stream
                // borrows it, so both must stay in this scope.
                let mut muxd = match addr.connect(0).await {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(error = %e, "usb subscribe: connect to usbmuxd failed");
                        return;
                    }
                };
                let mut stream = match muxd.listen().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(error = %e, "usb subscribe: Listen request failed");
                        return;
                    }
                };

                // Track id → udid so Disconnected(id) can be reported with its udid.
                let mut known: HashMap<u32, String> = HashMap::new();

                while let Some(item) = stream.next().await {
                    match item {
                        Ok(UsbmuxdListenEvent::Connected(dev)) => {
                            let mapped = map_device(dev);
                            // Only surface USB-attached devices (skip network-only).
                            if matches!(mapped.connection, ConnectionType::Usb) {
                                known.insert(mapped.id, mapped.udid.clone());
                                if tx.send(UsbEvent::Connected(mapped)).await.is_err() {
                                    return; // receiver dropped
                                }
                            }
                        }
                        Ok(UsbmuxdListenEvent::Disconnected(id)) => {
                            let udid = known.remove(&id).unwrap_or_default();
                            if tx.send(UsbEvent::Lost { id, udid }).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "usb subscribe: listen stream error; stopping"
                            );
                            return;
                        }
                    }
                }
            });
        });
        rx
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
