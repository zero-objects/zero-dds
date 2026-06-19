// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Shared wire helpers for policy encoders/decoders.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// Writes 1 bool + 3 byte padding (4-byte aligned).
///
/// # Errors
/// Buffer-Overflow.
pub(crate) fn write_bool_padded(w: &mut BufferWriter, v: bool) -> Result<(), EncodeError> {
    w.write_u8(u8::from(v))?;
    w.write_u8(0)?;
    w.write_u8(0)?;
    w.write_u8(0)
}

/// Reads 1 bool + 3 byte padding.
///
/// # Errors
/// Buffer-Underflow.
pub(crate) fn read_bool_padded(r: &mut BufferReader<'_>) -> Result<bool, DecodeError> {
    let v = r.read_u8()? != 0;
    let _ = r.read_u8()?;
    let _ = r.read_u8()?;
    let _ = r.read_u8()?;
    Ok(v)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    /// Roundtrip for `true`: must come out as `0x01 0x00 0x00 0x00`
    /// (DDSI-RTPS §9.6.3.2 Table 9.13 — 4-byte aligned bool).
    #[test]
    fn bool_padded_roundtrip_true() {
        let mut w = BufferWriter::new(Endianness::Little);
        write_bool_padded(&mut w, true).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[..], &[0x01u8, 0x00, 0x00, 0x00][..]);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(read_bool_padded(&mut r).unwrap());
    }

    /// Roundtrip for `false`: 4 null bytes.
    #[test]
    fn bool_padded_roundtrip_false() {
        let mut w = BufferWriter::new(Endianness::Little);
        write_bool_padded(&mut w, false).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[..], &[0u8, 0, 0, 0][..]);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(!read_bool_padded(&mut r).unwrap());
    }

    /// Every non-zero byte → `true` (the spec allows only 0/1, but the decoder
    /// normalizes defensively).
    #[test]
    fn read_bool_padded_nonzero_is_true() {
        let bytes = [0x42u8, 0x00, 0x00, 0x00];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(read_bool_padded(&mut r).unwrap());
    }

    /// Buffer underflow in the value byte → `DecodeError`.
    #[test]
    fn read_bool_padded_short_buffer_value_errors() {
        let bytes: [u8; 0] = [];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(read_bool_padded(&mut r).is_err());
    }

    /// Buffer-Underflow im Padding (nur 1 byte da) → `DecodeError`.
    /// Covers the three read_u8()? padding calls individually.
    #[test]
    fn read_bool_padded_short_buffer_padding_errors() {
        for len in 1usize..=3 {
            let bytes = alloc::vec![0u8; len];
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            assert!(
                read_bool_padded(&mut r).is_err(),
                "len={len} must short-read error"
            );
        }
    }
}
