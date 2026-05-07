// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Composite-Type-Encoder/-Decoder (W2).
//!
//! XCDR2-Wire-Format-Konventionen (OMG XTypes 1.3 §7.4):
//!
//! - **String** (§7.4.4): `uint32`-Laenge in Bytes **inklusive Null-
//!   Terminator** + UTF-8-Bytes + `\0`.
//! - **Sequence** (§7.4.4.2): `uint32`-Element-Anzahl + Elemente
//!   (jedes Element nach seinem eigenen Alignment).
//! - **Array** (§7.4.4.3): `N` Elemente ohne Laengen-Prefix.
//! - **Optional** (§7.4.5.1.4): `uint8`-Present-Flag (0/1) + Wert
//!   falls present.

// Modul ist nur unter `alloc`-Feature kompiliert (Re-Export in lib.rs
// hat das `cfg`); dieses File haengt von `Vec`/`String` ab.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::buffer::{BufferReader, BufferWriter};
use crate::encode::{CdrDecode, CdrEncode};
use crate::error::{DecodeError, EncodeError};

// ============================================================================
// String / &str
// ============================================================================

impl CdrEncode for str {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        let bytes = self.as_bytes();
        // Laenge in Bytes inklusive Null-Terminator.
        let len_with_nul = bytes
            .len()
            .checked_add(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(EncodeError::ValueOutOfRange {
                message: "string length exceeds u32::MAX",
            })?;
        writer.write_u32(len_with_nul)?;
        writer.write_bytes(bytes)?;
        writer.write_u8(0)?;
        Ok(())
    }
}

impl CdrEncode for String {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        self.as_str().encode(writer)
    }
}

impl CdrDecode for String {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len_with_nul = reader.read_u32()? as usize;
        if len_with_nul == 0 {
            return Err(DecodeError::LengthExceeded {
                announced: 0,
                remaining: reader.remaining(),
                offset: reader.position(),
            });
        }
        if len_with_nul > reader.remaining() {
            return Err(DecodeError::LengthExceeded {
                announced: len_with_nul,
                remaining: reader.remaining(),
                offset: reader.position(),
            });
        }
        // Letztes Byte muss Null-Terminator sein.
        let payload_len = len_with_nul - 1;
        let offset = reader.position();
        let bytes = reader.read_bytes(payload_len)?;
        let s = core::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8 { offset })?;
        let owned = String::from(s);
        // Null-Terminator konsumieren.
        let nul = reader.read_u8()?;
        if nul != 0 {
            return Err(DecodeError::InvalidUtf8 { offset });
        }
        Ok(owned)
    }
}

// ============================================================================
// Sequence (Vec<T>)
// ============================================================================

impl<T: CdrEncode> CdrEncode for Vec<T> {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        let len = u32::try_from(self.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "sequence length exceeds u32::MAX",
        })?;
        writer.write_u32(len)?;
        for item in self {
            item.encode(writer)?;
        }
        Ok(())
    }
}

impl<T: CdrDecode> CdrDecode for Vec<T> {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len = reader.read_u32()? as usize;
        // Defensive Sanity-Check: kann nicht mehr Elemente haben als
        // Bytes verbleiben (zumindest 1 Byte pro Element).
        if len > reader.remaining() {
            return Err(DecodeError::LengthExceeded {
                announced: len,
                remaining: reader.remaining(),
                offset: reader.position(),
            });
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(T::decode(reader)?);
        }
        Ok(out)
    }
}

// ============================================================================
// Array [T; N]
// ============================================================================

impl<T: CdrEncode, const N: usize> CdrEncode for [T; N] {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        for item in self {
            item.encode(writer)?;
        }
        Ok(())
    }
}

impl<T: CdrDecode + Default + Copy, const N: usize> CdrDecode for [T; N] {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let mut out = [T::default(); N];
        for slot in &mut out {
            *slot = T::decode(reader)?;
        }
        Ok(out)
    }
}

// ============================================================================
// Optional<T>
// ============================================================================

impl<T: CdrEncode> CdrEncode for Option<T> {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        match self {
            None => writer.write_u8(0),
            Some(value) => {
                writer.write_u8(1)?;
                value.encode(writer)
            }
        }
    }
}

impl<T: CdrDecode> CdrDecode for Option<T> {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let offset = reader.position();
        let flag = reader.read_u8()?;
        match flag {
            0 => Ok(None),
            1 => Ok(Some(T::decode(reader)?)),
            // Andere Werte sind in XCDR-Spec verboten — wir nutzen
            // InvalidBool als pragmatischen Match (Boolean-Semantik).
            other => Err(DecodeError::InvalidBool {
                value: other,
                offset,
            }),
        }
    }
}

// ============================================================================
// Map<K, V> — XCDR2 §7.4.4.6
// ============================================================================
//
// Wire-Format: 4-byte u32 entry-count + N × (K, V) pairs. Wir
// serialisieren entries in BTreeMap-iter-order (das ist key-sortiert,
// damit reproduzierbar). Decode rebuildet einen BTreeMap.

use alloc::collections::BTreeMap;

impl<K, V> CdrEncode for BTreeMap<K, V>
where
    K: CdrEncode + Ord,
    V: CdrEncode,
{
    fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        let len = u32::try_from(self.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "map: entry-count > u32::MAX",
        })?;
        w.write_u32(len)?;
        for (k, v) in self {
            k.encode(w)?;
            v.encode(w)?;
        }
        Ok(())
    }
}

impl<K, V> CdrDecode for BTreeMap<K, V>
where
    K: CdrDecode + Ord,
    V: CdrDecode,
{
    fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len = r.read_u32()? as usize;
        let mut map = BTreeMap::new();
        for _ in 0..len {
            let k = K::decode(r)?;
            let v = V::decode(r)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::Endianness;
    use alloc::string::ToString;
    use alloc::vec;

    fn rt_le<T>(value: T)
    where
        T: CdrEncode + CdrDecode + PartialEq + core::fmt::Debug,
    {
        let mut w = BufferWriter::new(Endianness::Little);
        value.encode(&mut w).expect("encode");
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = T::decode(&mut r).expect("decode");
        assert_eq!(decoded, value);
        assert_eq!(r.remaining(), 0);
    }

    // ---- String ----

    #[test]
    fn string_roundtrip_ascii() {
        rt_le(String::from("hello"));
    }

    #[test]
    fn string_roundtrip_unicode() {
        rt_le(String::from("Hällo, 🌍 Welt"));
    }

    #[test]
    fn string_roundtrip_empty() {
        rt_le(String::new());
    }

    #[test]
    fn string_wire_format_includes_null_terminator() {
        let mut w = BufferWriter::new(Endianness::Little);
        "ab".encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        // u32 len = 3 (ab + null) | 'a' 'b' null
        assert_eq!(bytes, vec![3, 0, 0, 0, b'a', b'b', 0]);
    }

    #[test]
    fn string_decode_rejects_zero_length() {
        let bytes = [0u8, 0, 0, 0]; // u32 len = 0 — kein Null-Terminator vorhanden
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = String::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    #[test]
    fn string_decode_rejects_announced_overrun() {
        let bytes = [100u8, 0, 0, 0, b'x'];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = String::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    #[test]
    fn string_decode_rejects_missing_null_terminator() {
        // Length 3 (a, b, x) — letztes Byte ist 'x' statt 0.
        let bytes = [3u8, 0, 0, 0, b'a', b'b', b'x'];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = String::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::InvalidUtf8 { .. })));
    }

    // ---- Sequence (Vec<T>) ----

    #[test]
    fn sequence_u8_roundtrip() {
        rt_le::<Vec<u8>>(vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn sequence_u32_roundtrip() {
        rt_le::<Vec<u32>>(vec![0xDEAD, 0xBEEF, 0x1234]);
    }

    #[test]
    fn sequence_empty_roundtrip() {
        rt_le::<Vec<u32>>(vec![]);
    }

    #[test]
    fn sequence_string_roundtrip() {
        rt_le::<Vec<String>>(vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn sequence_decode_rejects_overrun_length() {
        // Length 999 in 4 bytes Vec<u8>
        let bytes = [0xE7u8, 0x03, 0, 0, b'x']; // 999 announced, 1 byte data
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = Vec::<u8>::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    #[test]
    fn sequence_alignment_4_byte_prefix() {
        // u8 + Vec<u8> → u8 + 3 pad + u32 len + bytes
        let mut w = BufferWriter::new(Endianness::Little);
        1u8.encode(&mut w).unwrap();
        vec![10u8, 20, 30].encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes[0], 1); // u8
        assert_eq!(&bytes[1..4], &[0, 0, 0]); // padding
        assert_eq!(&bytes[4..8], &[3, 0, 0, 0]); // u32 length
        assert_eq!(&bytes[8..11], &[10, 20, 30]); // payload
    }

    // ---- Array ----

    #[test]
    fn array_u8_roundtrip() {
        rt_le::<[u8; 4]>([1, 2, 3, 4]);
    }

    #[test]
    fn array_u32_roundtrip() {
        rt_le::<[u32; 3]>([100, 200, 300]);
    }

    #[test]
    fn array_no_length_prefix() {
        let mut w = BufferWriter::new(Endianness::Little);
        [1u8, 2, 3].encode(&mut w).unwrap();
        // Keine u32-Laenge — nur Elemente.
        assert_eq!(w.into_bytes(), vec![1, 2, 3]);
    }

    #[test]
    fn array_zero_size() {
        let arr: [u32; 0] = [];
        let mut w = BufferWriter::new(Endianness::Little);
        arr.encode(&mut w).unwrap();
        assert!(w.into_bytes().is_empty());
    }

    // ---- Optional ----

    #[test]
    fn optional_none_roundtrip() {
        rt_le::<Option<u32>>(None);
    }

    #[test]
    fn optional_some_roundtrip() {
        rt_le::<Option<u32>>(Some(42));
    }

    #[test]
    fn optional_some_string_roundtrip() {
        rt_le::<Option<String>>(Some("hi".to_string()));
    }

    #[test]
    fn optional_wire_format_none_is_zero_byte() {
        let mut w = BufferWriter::new(Endianness::Little);
        Option::<u32>::None.encode(&mut w).unwrap();
        assert_eq!(w.into_bytes(), vec![0]);
    }

    #[test]
    fn optional_wire_format_some_is_one_then_value() {
        let mut w = BufferWriter::new(Endianness::Little);
        Some(0xABCDu32).encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes[0], 1); // present-flag
        // 3 byte padding + 4 byte u32
        assert_eq!(&bytes[1..4], &[0, 0, 0]);
        assert_eq!(&bytes[4..8], &[0xCD, 0xAB, 0, 0]);
    }

    #[test]
    fn optional_decode_rejects_invalid_flag() {
        let bytes = [0xFFu8];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = Option::<u32>::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::InvalidBool { .. })));
    }

    // ---- Mixed nested ----

    #[test]
    fn nested_optional_sequence_string() {
        let value: Option<Vec<String>> = Some(vec!["a".to_string(), "bb".to_string()]);
        rt_le(value);
    }

    #[test]
    fn nested_array_of_optionals() {
        let value: [Option<u32>; 3] = [Some(1), None, Some(3)];
        rt_le(value);
    }
}
