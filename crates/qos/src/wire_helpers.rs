// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Gemeinsame Wire-Helper fuer Policy-Encoder/Decoder.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// Schreibt 1 bool + 3 byte padding (4-byte aligned).
///
/// # Errors
/// Buffer-Overflow.
pub(crate) fn write_bool_padded(w: &mut BufferWriter, v: bool) -> Result<(), EncodeError> {
    w.write_u8(u8::from(v))?;
    w.write_u8(0)?;
    w.write_u8(0)?;
    w.write_u8(0)
}

/// Liest 1 bool + 3 byte padding.
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

    /// Roundtrip fuer `true`: muss als `0x01 0x00 0x00 0x00` rauskommen
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

    /// Roundtrip fuer `false`: 4 Null-Bytes.
    #[test]
    fn bool_padded_roundtrip_false() {
        let mut w = BufferWriter::new(Endianness::Little);
        write_bool_padded(&mut w, false).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[..], &[0u8, 0, 0, 0][..]);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(!read_bool_padded(&mut r).unwrap());
    }

    /// Jedes Nicht-Null-Byte → `true` (Spec erlaubt nur 0/1, aber Decoder
    /// normalisiert defensiv).
    #[test]
    fn read_bool_padded_nonzero_is_true() {
        let bytes = [0x42u8, 0x00, 0x00, 0x00];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(read_bool_padded(&mut r).unwrap());
    }

    /// Buffer-Underflow im Wert-Byte → `DecodeError`.
    #[test]
    fn read_bool_padded_short_buffer_value_errors() {
        let bytes: [u8; 0] = [];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(read_bool_padded(&mut r).is_err());
    }

    /// Buffer-Underflow im Padding (nur 1 byte da) → `DecodeError`.
    /// Deckt die drei read_u8()?-Padding-Aufrufe einzeln ab.
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
