//! Binary media header (28 bytes, big-endian).
//!
//! Spec: `docs/03-protocol.md` §5.2.

use bitflags::bitflags;
use thiserror::Error;

/// Magic marker (also identifies media-format version).
pub const MAGIC: u16 = 0xCC01;

/// Fixed header length in bytes.
pub const HEADER_LEN: usize = 28;

bitflags! {
    /// Header flags (low byte).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u8 {
        /// Frame is a keyframe / random-access point.
        const KEYFRAME   = 1 << 0;
        /// Payload is an encoded access unit (vs raw).
        const ENCODED    = 1 << 1;
        /// YUV uses full-range sampling (vs video-range).
        const FULL_RANGE = 1 << 2;
        /// Payload contains codec configuration (e.g., VPS/SPS/PPS).
        const CONFIG     = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MediaType {
    Video = 1,
    // 2 = audio reserved (see §10)
}

impl TryFrom<u8> for MediaType {
    type Error = DecodeError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(Self::Video),
            other => Err(DecodeError::UnknownMediaType(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Codec {
    Raw = 0,
    Hevc = 1,
    H264 = 2,
}

impl TryFrom<u8> for Codec {
    type Error = DecodeError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Raw),
            1 => Ok(Self::Hevc),
            2 => Ok(Self::H264),
            other => Err(DecodeError::UnknownCodec(other)),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("buffer too short for media header (need {HEADER_LEN}, got {got})")]
    TooShort { got: usize },
    #[error("bad magic 0x{0:04X}, expected 0xCC01")]
    BadMagic(u16),
    #[error("unknown media type {0}")]
    UnknownMediaType(u8),
    #[error("unknown codec {0}")]
    UnknownCodec(u8),
}

/// The 28-byte fixed binary header carried before every media payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaHeader {
    pub media_type: MediaType,
    pub flags: Flags,
    pub codec: Codec,
    pub width: u16,
    pub height: u16,
    pub seq: u32,
    pub pts_usec: u64,
    pub payload_len: u32,
}

impl MediaHeader {
    /// Encode the header as 28 big-endian bytes. `magic` and the 3 reserved
    /// bytes are written automatically.
    #[must_use]
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..2].copy_from_slice(&MAGIC.to_be_bytes());
        out[2] = self.media_type as u8;
        out[3] = self.flags.bits();
        out[4] = self.codec as u8;
        // out[5..8] = reserved (already zero)
        out[8..10].copy_from_slice(&self.width.to_be_bytes());
        out[10..12].copy_from_slice(&self.height.to_be_bytes());
        out[12..16].copy_from_slice(&self.seq.to_be_bytes());
        out[16..24].copy_from_slice(&self.pts_usec.to_be_bytes());
        out[24..28].copy_from_slice(&self.payload_len.to_be_bytes());
        out
    }

    /// Decode from a buffer of at least [`HEADER_LEN`] bytes. Reserved bytes
    /// are ignored (forward compatibility).
    pub fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < HEADER_LEN {
            return Err(DecodeError::TooShort { got: buf.len() });
        }
        let magic = u16::from_be_bytes([buf[0], buf[1]]);
        if magic != MAGIC {
            return Err(DecodeError::BadMagic(magic));
        }
        let media_type = MediaType::try_from(buf[2])?;
        let flags = Flags::from_bits_truncate(buf[3]);
        let codec = Codec::try_from(buf[4])?;
        // ignore buf[5..8] (reserved)
        let width = u16::from_be_bytes([buf[8], buf[9]]);
        let height = u16::from_be_bytes([buf[10], buf[11]]);
        let seq = u32::from_be_bytes(buf[12..16].try_into().unwrap());
        let pts_usec = u64::from_be_bytes(buf[16..24].try_into().unwrap());
        let payload_len = u32::from_be_bytes(buf[24..28].try_into().unwrap());
        Ok(Self {
            media_type,
            flags,
            codec,
            width,
            height,
            seq,
            pts_usec,
            payload_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MediaHeader {
        MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::KEYFRAME | Flags::ENCODED | Flags::FULL_RANGE,
            codec: Codec::Hevc,
            width: 1920,
            height: 1080,
            seq: 42,
            pts_usec: 1_234_567_890,
            payload_len: 12345,
        }
    }

    #[test]
    fn known_bytes_encode() {
        let h = sample();
        let bytes = h.encode();
        let expected: [u8; HEADER_LEN] = [
            0xCC, 0x01, // magic
            0x01, // media type = video
            0x07, // flags = keyframe | encoded | fullRange
            0x01, // codec = HEVC
            0x00, 0x00, 0x00, // reserved[3]
            0x07, 0x80, // width 1920
            0x04, 0x38, // height 1080
            0x00, 0x00, 0x00, 0x2A, // seq 42
            0x00, 0x00, 0x00, 0x00, 0x49, 0x96, 0x02, 0xD2, // ptsUsec
            0x00, 0x00, 0x30, 0x39, // payloadLen 12345
        ];
        assert_eq!(bytes, expected);
    }

    #[test]
    fn round_trip() {
        let h = sample();
        let back = MediaHeader::decode(&h.encode()).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn round_trip_zero() {
        let h = MediaHeader {
            media_type: MediaType::Video,
            flags: Flags::empty(),
            codec: Codec::Raw,
            width: 0,
            height: 0,
            seq: 0,
            pts_usec: 0,
            payload_len: 0,
        };
        let back = MediaHeader::decode(&h.encode()).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn too_short() {
        let err = MediaHeader::decode(&[0u8; HEADER_LEN - 1]).unwrap_err();
        assert_eq!(
            err,
            DecodeError::TooShort {
                got: HEADER_LEN - 1
            }
        );
    }

    #[test]
    fn bad_magic() {
        let mut buf = sample().encode();
        buf[0] = 0;
        let err = MediaHeader::decode(&buf).unwrap_err();
        assert!(matches!(err, DecodeError::BadMagic(_)));
    }

    #[test]
    fn unknown_codec() {
        let mut buf = sample().encode();
        buf[4] = 99;
        let err = MediaHeader::decode(&buf).unwrap_err();
        assert_eq!(err, DecodeError::UnknownCodec(99));
    }

    #[test]
    fn unknown_media_type() {
        let mut buf = sample().encode();
        buf[2] = 99;
        let err = MediaHeader::decode(&buf).unwrap_err();
        assert_eq!(err, DecodeError::UnknownMediaType(99));
    }

    #[test]
    fn reserved_bytes_ignored_on_decode() {
        let mut buf = sample().encode();
        buf[5] = 0xAB;
        buf[6] = 0xCD;
        buf[7] = 0xEF;
        let back = MediaHeader::decode(&buf).unwrap();
        assert_eq!(back, sample());
    }

    #[test]
    fn flags_unknown_bits_truncated_on_decode() {
        let mut buf = sample().encode();
        buf[3] = 0xFF; // all bits set, including unknown ones
        let back = MediaHeader::decode(&buf).unwrap();
        assert_eq!(
            back.flags,
            Flags::KEYFRAME | Flags::ENCODED | Flags::FULL_RANGE | Flags::CONFIG
        );
    }
}
