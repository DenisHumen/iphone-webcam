use thiserror::Error;

#[derive(Debug, Error)]
pub enum UsbTransportError {
    #[error("usbmuxd not reachable: {0}")]
    MuxdUnreachable(String),
    #[error("device not found: {udid}")]
    DeviceNotFound { udid: String },
    #[error("connect to port {port} failed: {reason}")]
    ConnectFailed { port: u16, reason: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported operation in this backend")]
    Unsupported,
    #[cfg(feature = "usb-idevice")]
    #[error("idevice: {0}")]
    Idevice(String),
}
