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

    // OASIS AMQP 1.0 Part 2 (`amqp-1.0-transport`) §2.3.1, Figure 2.14:
    // SIZE (4 octets), DOFF (1), TYPE (1), two type-specific octets (the
    // channel of an AMQP frame), the extended header, the body.

    /// §2.3.1: the frame type octet is 0x00 for AMQP and 0x01 for SASL.
    #[test]
    fn amqp10_2_3_1_the_type_octet_is_0x00_for_amqp_and_0x01_for_sasl() {
        assert_eq!(FrameType::from_u8(0x00), FrameType::Amqp);
        assert_eq!(FrameType::from_u8(0x01), FrameType::Sasl);
        assert_eq!(FrameType::from_u8(0xfe), FrameType::Reserved(0xfe));
        let sasl = FrameHeader {
            size: 8,
            doff: 2,
            frame_type: FrameType::Sasl,
            channel: 0,
        };
        assert_eq!(encode_frame_header(sasl), [0, 0, 0, 8, 2, 0x01, 0, 0]);
    }

    /// §2.3.1: the smallest frame is its eight-octet header, DOFF 2.
    #[test]
    fn amqp10_2_3_1_the_smallest_frame_is_its_eight_octet_header() {
        let h = FrameHeader::new_amqp(8, 2, 0);
        assert_eq!(encode_frame_header(h), [0, 0, 0, 8, 2, 0, 0, 0]);
        assert_eq!(decode_frame_header(&[0, 0, 0, 8, 2, 0, 0, 0]), Ok(h));
    }

    /// §2.3.1: SIZE and the channel in network byte order.
    #[test]
    fn amqp10_2_3_1_size_and_channel_are_in_network_byte_order() {
        let h = FrameHeader {
            size: 1024,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 42,
        };
        let octets = [0x00, 0x00, 0x04, 0x00, 0x02, 0x00, 0x00, 0x2a];
        assert_eq!(encode_frame_header(h), octets);
        assert_eq!(decode_frame_header(&octets), Ok(h));
    }

    /// §2.3.1.3 (DOFF): the body begins DOFF times four octets into the
    /// frame.
    #[test]
    fn amqp10_2_3_1_the_body_begins_doff_times_four_octets_in() {
        assert_eq!(FrameHeader::new_amqp(20, 3, 0).body_offset(), 12);
        assert_eq!(FrameHeader::new_amqp(8, 2, 0).body_offset(), 8);
    }

    /// §2.3.1: fewer than eight octets are no header.
    #[test]
    fn amqp10_2_3_1_fewer_than_eight_octets_are_no_header() {
        assert_eq!(decode_frame_header(&[]), Err(FrameError::HeaderTooShort));
        assert_eq!(
            decode_frame_header(&[0; 7]),
            Err(FrameError::HeaderTooShort)
        );
    }

    /// §2.3.1.3: DOFF "MUST be at least 2".
    #[test]
    fn amqp10_2_3_1_a_doff_below_2_is_refused() {
        assert_eq!(
            decode_frame_header(&[0u8, 0, 0, 8, 1, 0, 0, 0]),
            Err(FrameError::InvalidDataOffset(1))
        );
    }

    /// §2.3.1.2: SIZE counts the whole frame, at least its eight-octet
    /// header.
    #[test]
    fn amqp10_2_3_1_a_size_below_8_is_refused() {
        assert_eq!(
            decode_frame_header(&[0u8, 0, 0, 4, 2, 0, 0, 0]),
            Err(FrameError::SizeBelowMinimum(4))
        );
    }

    /// §2.3.1: a body offset beyond SIZE is malformed.
    #[test]
    fn amqp10_2_3_1_a_body_offset_beyond_size_is_refused() {
        assert_eq!(
            decode_frame_header(&[0u8, 0, 0, 8, 4, 0, 0, 0]),
            Err(FrameError::BodyOffsetExceedsSize)
        );
    }
}
