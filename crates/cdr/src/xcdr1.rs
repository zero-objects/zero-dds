// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR1 / PL_CDR1 — Plain CDR Version 1 with a parameter list for
//! `@mutable` structs (XTypes 1.3 §7.4.2 / §7.4.1.2).
//!
//! Wire form per member:
//!
//! - Standard header (16-bit member ID, 16-bit length):
//!   `[id_lo id_hi length_lo length_hi] [body padded to 4 bytes]`
//!   Valid when `member_id < 0x3F00` and `length <= 0xFFFF`.
//!
//! - Extended header (PID_EXTENDED + 32-bit member ID + 32-bit length):
//!   `[0x01 0x3F 0x08 0x00] [member_id u32 LE] [length u32 LE]`
//!   `[body padded to 4 bytes]`
//!   Mandatory when `member_id >= 0x3F00` OR `length > 0xFFFF`.
//!
//! The list ends with the sentinel `[0x02 0x3F 0x00 0x00]` (PID_LIST_END).
//!
//! # Spec source
//! XTypes 1.3 §7.4.1.2 ("Parameter ID layout" tables); §7.4.2.

extern crate alloc;
use alloc::vec::Vec;

use crate::buffer::{BufferReader, BufferWriter};
use crate::error::{DecodeError, EncodeError};

/// PID_LIST_END (XTypes 1.3 §7.4.1.2.4) — sentinel terminator of a
/// PL_CDR1 parameter list.
pub const PID_LIST_END: u16 = 0x3F02;

/// PID_EXTENDED (XTypes 1.3 §7.4.1.2.2) — indicator for the long header
/// with a 32-bit member ID and 32-bit length.
pub const PID_EXTENDED: u16 = 0x3F01;

/// Threshold for member IDs at and above which the long header
/// (PID_EXTENDED) is mandatory. IDs >= this value collide with the
/// reserved 0x3FXX PIDs and MUST be encoded extended.
pub const PID_EXTENDED_THRESHOLD: u32 = 0x3F00;

/// Parsed PL_CDR1 member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlCdr1Member {
    /// 32-bit member ID (extended header) or 16-bit (standard).
    pub member_id: u32,
    /// Body bytes (without padding, the caller-relevant part).
    pub body: Vec<u8>,
}

/// Encodes a single member; automatically chooses between the standard
/// and extended header.
///
/// # Errors
/// `ValueOutOfRange` if the body is larger than `u32::MAX`.
pub fn encode_pl_cdr1_member<F>(
    writer: &mut BufferWriter,
    member_id: u32,
    body: F,
) -> Result<(), EncodeError>
where
    F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
{
    let mut inner = BufferWriter::new(writer.endianness());
    body(&mut inner)?;
    let mut body_bytes = inner.into_bytes();
    // Pad to a 4-byte boundary.
    while body_bytes.len() % 4 != 0 {
        body_bytes.push(0);
    }
    let body_len = body_bytes.len();
    let needs_extended = member_id >= PID_EXTENDED_THRESHOLD || body_len > 0xFFFF;
    if needs_extended {
        // Extended header: PID_EXTENDED, length=8 (header length), then
        // 32-bit member id + 32-bit length, then body.
        writer.write_u16(PID_EXTENDED)?;
        writer.write_u16(8u16)?;
        writer.write_u32(member_id)?;
        let len_u32 = u32::try_from(body_len).map_err(|_| EncodeError::ValueOutOfRange {
            message: "PL_CDR1 body length exceeds u32::MAX",
        })?;
        writer.write_u32(len_u32)?;
        writer.write_bytes(&body_bytes)?;
    } else {
        let id_u16 = u16::try_from(member_id).map_err(|_| EncodeError::ValueOutOfRange {
            message: "PL_CDR1 standard member_id must fit in u16",
        })?;
        let len_u16 = u16::try_from(body_len).map_err(|_| EncodeError::ValueOutOfRange {
            message: "PL_CDR1 standard length must fit in u16",
        })?;
        writer.write_u16(id_u16)?;
        writer.write_u16(len_u16)?;
        writer.write_bytes(&body_bytes)?;
    }
    Ok(())
}

/// Writes the sentinel terminator (PID_LIST_END).
///
/// # Errors
/// Buffer overflow.
pub fn write_pl_cdr1_sentinel(writer: &mut BufferWriter) -> Result<(), EncodeError> {
    writer.write_u16(PID_LIST_END)?;
    writer.write_u16(0u16)
}

/// Decodes a single PL_CDR1 member. Returns `None` if the next PID is the
/// sentinel.
///
/// # Errors
/// `UnexpectedEof` on truncation; `LengthExceeded` on an oversize body.
pub fn read_pl_cdr1_member(
    reader: &mut BufferReader<'_>,
) -> Result<Option<PlCdr1Member>, DecodeError> {
    if reader.remaining() < 4 {
        return Err(DecodeError::UnexpectedEof {
            needed: 4,
            offset: reader.position(),
        });
    }
    let pid = reader.read_u16()?;
    let len_u16 = reader.read_u16()?;
    if pid == PID_LIST_END {
        return Ok(None);
    }
    let (member_id, body_len) = if pid == PID_EXTENDED {
        if len_u16 != 8 {
            return Err(DecodeError::LengthExceeded {
                announced: usize::from(len_u16),
                remaining: 8,
                offset: reader.position(),
            });
        }
        let m_id = reader.read_u32()?;
        let b_len_u32 = reader.read_u32()?;
        let b_len = usize::try_from(b_len_u32).map_err(|_| DecodeError::LengthExceeded {
            announced: usize::MAX,
            remaining: reader.remaining(),
            offset: reader.position(),
        })?;
        (m_id, b_len)
    } else {
        (u32::from(pid), usize::from(len_u16))
    };
    if body_len > reader.remaining() {
        return Err(DecodeError::LengthExceeded {
            announced: body_len,
            remaining: reader.remaining(),
            offset: reader.position(),
        });
    }
    let body = reader.read_bytes(body_len)?.to_vec();
    Ok(Some(PlCdr1Member { member_id, body }))
}

/// Reads all PL_CDR1 members up to the sentinel.
///
/// # Errors
/// Same as [`read_pl_cdr1_member`].
pub fn read_all_pl_cdr1_members(
    reader: &mut BufferReader<'_>,
) -> Result<Vec<PlCdr1Member>, DecodeError> {
    let mut out = Vec::new();
    while let Some(m) = read_pl_cdr1_member(reader)? {
        out.push(m);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Endianness;
    use crate::encode::CdrEncode;
    use alloc::vec;

    #[test]
    fn standard_header_for_small_id_and_length() {
        let mut w = BufferWriter::new(Endianness::Little);
        encode_pl_cdr1_member(&mut w, 7, |w| 42u32.encode(w)).unwrap();
        let bytes = w.into_bytes();
        // Header: id=0x0007, length=4 (LE) → 07 00 04 00; body 42 00 00 00.
        assert_eq!(&bytes[0..4], &[0x07, 0x00, 0x04, 0x00]);
        assert_eq!(&bytes[4..8], &[42, 0, 0, 0]);
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn extended_header_for_id_above_threshold() {
        // member_id = 16129 (0x3F01) — collides with the PID_EXTENDED slot,
        // so it must be encoded with PID_EXTENDED.
        let mut w = BufferWriter::new(Endianness::Little);
        encode_pl_cdr1_member(&mut w, 16_129, |w| 99u32.encode(w)).unwrap();
        let bytes = w.into_bytes();
        // PID_EXTENDED 0x3F01 + length=8.
        assert_eq!(&bytes[0..4], &[0x01, 0x3F, 0x08, 0x00]);
        // member_id = 16129 LE.
        assert_eq!(&bytes[4..8], &[0x01, 0x3F, 0x00, 0x00]);
        // length field = 4 LE.
        assert_eq!(&bytes[8..12], &[0x04, 0x00, 0x00, 0x00]);
        // body 99 00 00 00.
        assert_eq!(&bytes[12..16], &[99, 0, 0, 0]);
    }

    #[test]
    fn extended_header_for_large_body_length() {
        // Body > 0xFFFF bytes → must be extended.
        let mut w = BufferWriter::new(Endianness::Little);
        let big = vec![0xABu8; 70_000];
        encode_pl_cdr1_member(&mut w, 1, |w| {
            for b in &big {
                w.write_u8(*b)?;
            }
            Ok(())
        })
        .unwrap();
        let bytes = w.into_bytes();
        // Header: PID_EXTENDED + length=8.
        assert_eq!(&bytes[0..4], &[0x01, 0x3F, 0x08, 0x00]);
        // member_id = 1 LE.
        assert_eq!(&bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);
        // body length 70000 padded to the next 4-byte boundary = 70000.
        let len_field = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        assert_eq!(len_field, 70_000);
    }

    #[test]
    fn xcdr1_pl_cdr_long_header_roundtrip() {
        // Multiple members: one standard, one extended.
        let mut w = BufferWriter::new(Endianness::Little);
        encode_pl_cdr1_member(&mut w, 10, |w| 0xCAFEu32.encode(w)).unwrap();
        encode_pl_cdr1_member(&mut w, 70_000, |w| 0xBEEFu32.encode(w)).unwrap();
        write_pl_cdr1_sentinel(&mut w).unwrap();
        let bytes = w.into_bytes();

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let members = read_all_pl_cdr1_members(&mut r).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].member_id, 10);
        assert_eq!(members[1].member_id, 70_000);
        assert_eq!(&members[0].body[..4], &0xCAFEu32.to_le_bytes());
        assert_eq!(&members[1].body[..4], &0xBEEFu32.to_le_bytes());
    }

    #[test]
    fn xcdr1_member_id_above_threshold_uses_extended_pid() {
        // Exact threshold value: 0x3F00 = 16128 → must be extended.
        let mut w = BufferWriter::new(Endianness::Little);
        encode_pl_cdr1_member(&mut w, 0x3F00, |w| 1u8.encode(w)).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[0..2], &[0x01, 0x3F]); // PID_EXTENDED LE
    }

    #[test]
    fn xcdr1_member_id_just_below_threshold_uses_standard() {
        // 0x3EFF (16127) — just barely standard.
        let mut w = BufferWriter::new(Endianness::Little);
        encode_pl_cdr1_member(&mut w, 0x3EFF, |w| 1u8.encode(w)).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[0..2], &[0xFF, 0x3E]); // member_id LE, NOT PID_EXTENDED
    }

    #[test]
    fn xcdr1_sentinel_terminates_decode() {
        // Sentinel only: read_all returns an empty list.
        let bytes = vec![0x02, 0x3F, 0x00, 0x00];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let members = read_all_pl_cdr1_members(&mut r).unwrap();
        assert!(members.is_empty());
    }

    #[test]
    fn xcdr1_truncated_extended_header_rejected() {
        // PID_EXTENDED announced, but length field != 8 → error.
        let bytes = vec![0x01, 0x3F, 0x04, 0x00, 0, 0, 0, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = read_pl_cdr1_member(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    #[test]
    fn xcdr1_truncated_body_rejected() {
        // Standard header announces a 100-byte body, only 8 present.
        let bytes = vec![0x01, 0x00, 0x64, 0x00, 1, 2, 3, 4, 5, 6, 7, 8];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = read_pl_cdr1_member(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }
}
