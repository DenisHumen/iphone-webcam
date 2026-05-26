//! Length-prefixed framing for control-plane JSON messages.

use thiserror::Error;

/// Default maximum payload size accepted by [`try_unframe`] (1 MiB).
///
/// Control messages are small JSON; this is a sanity cap to prevent
/// memory-exhaustion attacks via crafted length prefixes.
pub const DEFAULT_MAX_PAYLOAD: u32 = 1024 * 1024;

/// Length prefix is 4 bytes (BE u32).
pub const PREFIX_LEN: usize = 4;

/// Errors that can happen while reading a framed payload from a buffer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    /// Declared payload length exceeds the caller's maximum.
    #[error("framed payload length {len} exceeds maximum {max}")]
    TooLarge { len: u32, max: u32 },
}

/// Encode a payload as `[BE u32 length][payload]`.
#[must_use]
pub fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(PREFIX_LEN + payload.len());
    let len = u32::try_from(payload.len()).expect("payload < 4 GiB");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// Try to read one length-prefixed payload from the front of `buf`.
///
/// Returns:
/// - `Ok(Some((payload, frame_total_len)))` if a complete frame is available.
///   `payload` is a slice of `buf`; `frame_total_len = PREFIX_LEN + payload.len()` —
///   the caller should advance its read cursor by this amount.
/// - `Ok(None)` if more bytes are needed (no allocation; safe to call again later).
/// - `Err(FrameError)` if the declared length exceeds `max_payload`.
pub fn try_unframe(buf: &[u8], max_payload: u32) -> Result<Option<(&[u8], usize)>, FrameError> {
    if buf.len() < PREFIX_LEN {
        return Ok(None);
    }
    let mut len_bytes = [0u8; PREFIX_LEN];
    len_bytes.copy_from_slice(&buf[..PREFIX_LEN]);
    let len = u32::from_be_bytes(len_bytes);
    if len > max_payload {
        return Err(FrameError::TooLarge {
            len,
            max: max_payload,
        });
    }
    let total = PREFIX_LEN + len as usize;
    if buf.len() < total {
        return Ok(None);
    }
    Ok(Some((&buf[PREFIX_LEN..total], total)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_writes_be_length_prefix() {
        let bytes = frame(b"abc");
        assert_eq!(&bytes[..PREFIX_LEN], &[0, 0, 0, 3]);
        assert_eq!(&bytes[PREFIX_LEN..], b"abc");
    }

    #[test]
    fn frame_empty_payload() {
        let bytes = frame(b"");
        assert_eq!(bytes, vec![0, 0, 0, 0]);
    }

    #[test]
    fn try_unframe_round_trip() {
        let payload = br#"{"t":"HELLO","seq":1}"#;
        let framed = frame(payload);
        let (got, n) = try_unframe(&framed, DEFAULT_MAX_PAYLOAD).unwrap().unwrap();
        assert_eq!(got, &payload[..]);
        assert_eq!(n, framed.len());
    }

    #[test]
    fn try_unframe_returns_none_when_prefix_incomplete() {
        let buf = [0u8, 0, 0];
        assert_eq!(try_unframe(&buf, DEFAULT_MAX_PAYLOAD).unwrap(), None);
    }

    #[test]
    fn try_unframe_returns_none_when_payload_partial() {
        // declared 5 bytes, only 2 available
        let buf = [0u8, 0, 0, 5, b'h', b'e'];
        assert_eq!(try_unframe(&buf, DEFAULT_MAX_PAYLOAD).unwrap(), None);
    }

    #[test]
    fn try_unframe_consumes_only_one_frame() {
        // two concatenated frames; should return only the first
        let first = frame(b"first");
        let second = frame(b"second");
        let mut combined = first.clone();
        combined.extend_from_slice(&second);
        let (payload, n) = try_unframe(&combined, DEFAULT_MAX_PAYLOAD)
            .unwrap()
            .unwrap();
        assert_eq!(payload, b"first");
        assert_eq!(n, first.len());
    }

    #[test]
    fn try_unframe_too_large() {
        let buf = [0xFFu8, 0xFF, 0xFF, 0xFF];
        let err = try_unframe(&buf, 1024).unwrap_err();
        assert_eq!(
            err,
            FrameError::TooLarge {
                len: u32::MAX,
                max: 1024
            }
        );
    }
}
