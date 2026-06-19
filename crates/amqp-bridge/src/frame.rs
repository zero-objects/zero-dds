// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 1.0 Frame Format — Spec `amqp-1.0-transport` §2.3.

use core::fmt;

/// Spec §2.3.1.1 — AMQP frame type (0x00) or SASL frame type (0x01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameType {
    /// `0x00` — AMQP frame.
    Amqp,
    /// `0x01` — SASL frame.
    Sasl,
    /// Other reserved values (forwards-compat).
    Reserved(u8),
}

impl FrameType {
    /// Wire value.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Amqp => 0x00,
            Self::Sasl => 0x01,
            Self::Reserved(v) => v,
        }
    }

    /// Decodes from the wire value.
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0x00 => Self::Amqp,
            0x01 => Self::Sasl,
            other => Self::Reserved(other),
        }
    }
}

/// Spec §2.3.1 — frame header (8 bytes).
///
/// Wire-Layout:
/// ```text
///  0       4       5       6                  8
///  +-------+-------+-------+------------------+
///  | SIZE  | DOFF  | TYPE  |     CHANNEL      |
///  +-------+-------+-------+------------------+
/// ```
///
/// * `SIZE` — Total frame size in bytes (header + extended-header +
///   body). Spec: 32-bit unsigned BE.
/// * `DOFF` — Data offset in 4-byte words. MUST >= 2 (the header is
///   itself 8 bytes / 4-byte word = 2). Spec §2.3.1.3.
/// * `TYPE` — `0x00` AMQP / `0x01` SASL.
/// * `CHANNEL` — 16-bit unsigned BE channel number (or Reserved=0
///   for SASL frames).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Spec §2.3.1.2 — total frame size.
    pub size: u32,
    /// Spec §2.3.1.3 — data offset (in 4-byte words).
    pub doff: u8,
    /// Spec §2.3.1.4 — frame type.
    pub frame_type: FrameType,
    /// Spec §2.3.1.5 — channel.
    pub channel: u16,
}

impl FrameHeader {
    /// Constructs an AMQP frame header. `size_total` is the
    /// **total** frame size incl. header (Spec §2.3.1.2:
    /// "computed by adding 8 byte fixed frame header [...] + extended
    /// header + body").
    ///
    /// `doff_words` is the `DOFF` in 4-byte words (MUST >= 2).
    /// Default value 2 = "no extended header".
    #[must_use]
    pub const fn new_amqp(size_total: u32, doff_words: u8, channel: u16) -> Self {
        Self {
            size: size_total,
            doff: doff_words,
            frame_type: FrameType::Amqp,
            channel,
        }
    }

    /// Spec §2.3.1.3 — position of the frame body in bytes from the start.
    #[must_use]
    pub const fn body_offset(self) -> usize {
        (self.doff as usize) * 4
    }
}

/// Frame-header codec error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Header < 8 bytes.
    HeaderTooShort,
    /// Spec §2.3.1.3 — `DOFF < 2`.
    InvalidDataOffset(u8),
    /// Spec §2.3.1.2 — `SIZE < 8` (the header alone is 8 bytes).
    SizeBelowMinimum(u32),
    /// `body_offset > size`.
    BodyOffsetExceedsSize,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooShort => f.write_str("frame header < 8 bytes"),
            Self::InvalidDataOffset(d) => write!(f, "invalid DOFF {d} (< 2)"),
            Self::SizeBelowMinimum(s) => write!(f, "frame SIZE {s} < 8 minimum"),
            Self::BodyOffsetExceedsSize => f.write_str("body offset exceeds SIZE"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameError {}

/// Encodes a frame header into 8 bytes.
#[must_use]
pub fn encode_frame_header(h: FrameHeader) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[0..4].copy_from_slice(&h.size.to_be_bytes());
    out[4] = h.doff;
    out[5] = h.frame_type.to_u8();
    out[6..8].copy_from_slice(&h.channel.to_be_bytes());
    out
}

/// Decodes a frame header from 8+ bytes.
///
/// # Errors
/// See [`FrameError`].
pub fn decode_frame_header(bytes: &[u8]) -> Result<FrameHeader, FrameError> {
    if bytes.len() < 8 {
        return Err(FrameError::HeaderTooShort);
    }
    let size = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if size < 8 {
        return Err(FrameError::SizeBelowMinimum(size));
    }
    let doff = bytes[4];
    if doff < 2 {
        return Err(FrameError::InvalidDataOffset(doff));
    }
    let body_offset = u32::from(doff) * 4;
    if body_offset > size {
        return Err(FrameError::BodyOffsetExceedsSize);
    }
    let frame_type = FrameType::from_u8(bytes[5]);
    let channel = u16::from_be_bytes([bytes[6], bytes[7]]);
    Ok(FrameHeader {
        size,
        doff,
        frame_type,
        channel,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn frame_type_round_trip() {
        for ft in [FrameType::Amqp, FrameType::Sasl, FrameType::Reserved(0xFE)] {
            assert_eq!(FrameType::from_u8(ft.to_u8()), ft);
        }
    }

    #[test]
    fn header_round_trip_minimum_size() {
        // Minimum: SIZE=8, DOFF=2 (no extended header), Type=AMQP,
        // Channel=0.
        let h = FrameHeader::new_amqp(8, 2, 0);
        let bytes = encode_frame_header(h);
        assert_eq!(bytes, [0, 0, 0, 8, 2, 0, 0, 0]);
        let parsed = decode_frame_header(&bytes).expect("decode");
        assert_eq!(parsed, h);
    }

    #[test]
    fn header_with_channel_42_and_size_1024() {
        let h = FrameHeader {
            size: 1024,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 42,
        };
        let bytes = encode_frame_header(h);
        let parsed = decode_frame_header(&bytes).expect("decode");
        assert_eq!(parsed, h);
        assert_eq!(parsed.channel, 42);
        assert_eq!(parsed.size, 1024);
    }

    #[test]
    fn body_offset_is_doff_times_4() {
        // Spec §2.3.1.3.
        let h = FrameHeader::new_amqp(20, 3, 0); // 3 words = 12 bytes.
        assert_eq!(h.body_offset(), 12);
        let h2 = FrameHeader::new_amqp(8, 2, 0); // 2 words = 8 bytes (no ext header).
        assert_eq!(h2.body_offset(), 8);
    }

    #[test]
    fn header_too_short_decode_fails() {
        assert_eq!(decode_frame_header(&[]), Err(FrameError::HeaderTooShort));
        assert_eq!(
            decode_frame_header(&[0; 7]),
            Err(FrameError::HeaderTooShort)
        );
    }

    #[test]
    fn doff_below_2_rejected() {
        // Spec §2.3.1.3 — DOFF MUST be >= 2.
        let bytes = [0u8, 0, 0, 8, 1, 0, 0, 0];
        assert_eq!(
            decode_frame_header(&bytes),
            Err(FrameError::InvalidDataOffset(1))
        );
    }

    #[test]
    fn size_below_8_rejected() {
        // Spec §2.3.1.2 — SIZE incl. header (8 bytes minimum).
        let bytes = [0u8, 0, 0, 4, 2, 0, 0, 0];
        assert_eq!(
            decode_frame_header(&bytes),
            Err(FrameError::SizeBelowMinimum(4))
        );
    }

    #[test]
    fn body_offset_exceeding_size_rejected() {
        // Doff*4 > SIZE → invalid.
        let bytes = [0u8, 0, 0, 8, 4, 0, 0, 0];
        assert_eq!(
            decode_frame_header(&bytes),
            Err(FrameError::BodyOffsetExceedsSize)
        );
    }

    #[test]
    fn sasl_frame_type_byte_is_1() {
        let h = FrameHeader {
            size: 8,
            doff: 2,
            frame_type: FrameType::Sasl,
            channel: 0,
        };
        let bytes = encode_frame_header(h);
        assert_eq!(bytes[5], 0x01);
    }
}
