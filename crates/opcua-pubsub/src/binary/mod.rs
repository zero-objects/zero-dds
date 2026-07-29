// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OPC-UA binary encoding (OPC-UA Foundation **Part 6** §5.2).
//!
//! This is the wire primitive the DDS-OPCUA gateway previously delegated
//! to an external stack (open62541 / opcua-rs). UADP DataSetMessages
//! (Part 14) carry their payload in exactly this encoding, so it is the
//! foundation for the whole Pub/Sub stack — and it simultaneously lets
//! the Client/Server gateway encode natively.
//!
//! The encoders/decoders operate directly on the `zerodds-opcua-gateway`
//! type model (`NodeId`, `Variant`, `DataValue`, …) — there is no
//! parallel type hierarchy.
//!
//! # Encoding rules (Part 6 §5.2.2)
//!
//! * Integers/floats: little-endian, no alignment padding.
//! * `String`/`ByteString`/`XmlElement`: `Int32` length prefix, `-1`
//!   denotes the null form, `0` the empty form, followed by the bytes.
//! * Arrays: `Int32` length prefix (`-1` = null) followed by the
//!   elements back-to-back.

mod builtin;
mod io;

use alloc::string::String;
use alloc::vec::Vec;

pub(crate) use builtin::{builtin_type_from_value, decode_builtin_value};
pub use io::{UaReader, UaWriter};

use crate::error::{DecodeError, EncodeError};

/// A value that can be serialised into the OPC-UA binary encoding.
pub trait UaEncode {
    /// Appends the binary form of `self` to `w`.
    ///
    /// Returns [`EncodeError::LengthOverflow`] if a contained
    /// `String`/`ByteString`/array exceeds the `Int32` length ceiling
    /// mandated by Part 6 §5.2.2.4.
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError>;
}

/// A value that can be parsed from the OPC-UA binary encoding.
pub trait UaDecode: Sized {
    /// Reads one value of `Self` from `r`, advancing the cursor.
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError>;
}

/// Converts a buffer length to the `Int32` the wire format requires,
/// rejecting values above `i32::MAX` (Part 6 §5.2.2.4).
pub(crate) fn len_i32(what: &'static str, len: usize) -> Result<i32, EncodeError> {
    i32::try_from(len).map_err(|_| EncodeError::LengthOverflow { what, len })
}

/// Converts a count to the `UInt16` several UADP headers use (field
/// counts, DataSetMessage counts), rejecting values above `u16::MAX`.
pub(crate) fn len_u16(what: &'static str, len: usize) -> Result<u16, EncodeError> {
    u16::try_from(len).map_err(|_| EncodeError::LengthOverflow { what, len })
}

/// Writes a non-nullable UTF-8 string (`Int32` byte length + bytes).
pub(crate) fn write_string(w: &mut UaWriter, s: &str) -> Result<(), EncodeError> {
    w.write_i32(len_i32("String", s.len())?);
    w.write_bytes(s.as_bytes());
    Ok(())
}

/// Writes a non-nullable byte string (`Int32` length + bytes).
pub(crate) fn write_byte_string(w: &mut UaWriter, b: &[u8]) -> Result<(), EncodeError> {
    w.write_i32(len_i32("ByteString", b.len())?);
    w.write_bytes(b);
    Ok(())
}

/// Reads a non-nullable UTF-8 string; the null form (`-1`) maps to the
/// empty string, matching the gateway model which uses a plain `String`
/// for these fields.
pub(crate) fn read_string(r: &mut UaReader<'_>) -> Result<String, DecodeError> {
    Ok(read_opt_string(r)?.unwrap_or_default())
}

/// Reads an optional UTF-8 string; the null form (`-1`) maps to `None`.
pub(crate) fn read_opt_string(r: &mut UaReader<'_>) -> Result<Option<String>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(None);
    }
    let bytes = r.read_bytes(len as usize)?;
    let s = core::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
    Ok(Some(String::from(s)))
}

/// Reads a byte string; the null form (`-1`) maps to an empty vector.
pub(crate) fn read_byte_string(r: &mut UaReader<'_>) -> Result<Vec<u8>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    Ok(r.read_bytes(len as usize)?.to_vec())
}

/// Writes an `Int32`-length-prefixed array of encodable elements
/// (Part 6 §5.2.5).
pub(crate) fn write_array<T: UaEncode>(
    w: &mut UaWriter,
    items: &[T],
    what: &'static str,
) -> Result<(), EncodeError> {
    w.write_i32(len_i32(what, items.len())?);
    for it in items {
        it.encode(w)?;
    }
    Ok(())
}

/// Rejects a wire-supplied element `count` that cannot possibly fit the
/// bytes actually remaining, given the minimum wire size of one element
/// (`min_elem`). Mirrors `crates/cdr/src/composite.rs`'s
/// `len > reader.remaining()` guard: it must run *before* any
/// `Vec::with_capacity`/`reserve` call, so a huge wire count cannot force
/// an oversized allocation ahead of the (bounds-checked) per-element reads.
///
/// `pub` (not `pub(crate)`) so downstream OPC-UA binary-encoding consumers
/// (e.g. `zerodds-opcua-server`'s `wire.rs`, which reuses this crate's
/// [`UaReader`]/[`DecodeError`]) can apply the same guard instead of
/// re-inventing it.
pub fn check_array_len(
    r: &UaReader<'_>,
    count: usize,
    min_elem: usize,
    message: &'static str,
) -> Result<(), DecodeError> {
    let max_count = r.remaining() / min_elem.max(1);
    if count > max_count {
        return Err(DecodeError::MalformedMessage { message });
    }
    Ok(())
}

/// Reads an `Int32`-length-prefixed array (`-1` = null → empty).
pub(crate) fn read_array<T: UaDecode>(r: &mut UaReader<'_>) -> Result<Vec<T>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    let len = len as usize;
    // Conservative bound: every element needs >= 1 wire byte, so a count
    // above the remaining bytes cannot be genuine. Reject before
    // `Vec::with_capacity` — see `check_array_len`.
    check_array_len(r, len, 1, "array length exceeds remaining bytes")?;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(T::decode(r)?);
    }
    Ok(out)
}

/// Writes an `Int32`-length-prefixed `UInt16` array.
pub(crate) fn write_u16_array(w: &mut UaWriter, items: &[u16]) -> Result<(), EncodeError> {
    w.write_i32(len_i32("UInt16 array", items.len())?);
    for x in items {
        w.write_u16(*x);
    }
    Ok(())
}

/// Reads an `Int32`-length-prefixed `UInt16` array (`-1` = null → empty).
pub(crate) fn read_u16_array(r: &mut UaReader<'_>) -> Result<Vec<u16>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    let len = len as usize;
    check_array_len(r, len, 2, "UInt16 array length exceeds remaining bytes")?;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(r.read_u16()?);
    }
    Ok(out)
}

/// Writes an `Int32`-length-prefixed `UInt32` array.
pub(crate) fn write_u32_array(w: &mut UaWriter, items: &[u32]) -> Result<(), EncodeError> {
    w.write_i32(len_i32("UInt32 array", items.len())?);
    for x in items {
        w.write_u32(*x);
    }
    Ok(())
}

/// Reads an `Int32`-length-prefixed `UInt32` array (`-1` = null → empty).
pub(crate) fn read_u32_array(r: &mut UaReader<'_>) -> Result<Vec<u32>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    let len = len as usize;
    check_array_len(r, len, 4, "UInt32 array length exceeds remaining bytes")?;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(r.read_u32()?);
    }
    Ok(out)
}

/// Writes an `Int32`-length-prefixed array of non-nullable UTF-8 strings.
pub(crate) fn write_string_array(w: &mut UaWriter, items: &[String]) -> Result<(), EncodeError> {
    w.write_i32(len_i32("String array", items.len())?);
    for s in items {
        write_string(w, s)?;
    }
    Ok(())
}

/// Reads an `Int32`-length-prefixed string array (`-1` = null → empty).
pub(crate) fn read_string_array(r: &mut UaReader<'_>) -> Result<Vec<String>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    let len = len as usize;
    // Every string carries at least its Int32 length prefix.
    check_array_len(r, len, 4, "String array length exceeds remaining bytes")?;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(read_string(r)?);
    }
    Ok(out)
}

/// Convenience: encode any [`UaEncode`] value to a fresh `Vec<u8>`.
pub fn to_binary<T: UaEncode>(value: &T) -> Result<Vec<u8>, EncodeError> {
    let mut w = UaWriter::new();
    value.encode(&mut w)?;
    Ok(w.into_vec())
}

/// Convenience: decode a [`UaDecode`] value from a byte slice.
pub fn from_binary<T: UaDecode>(bytes: &[u8]) -> Result<T, DecodeError> {
    let mut r = UaReader::new(bytes);
    T::decode(&mut r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_opcua_gateway::types::Guid;

    // -------------------------------------------------------------
    // Buffer-cap hardening — a wire-supplied `Int32` element count
    // must be rejected cleanly (no OOM, no panic) before
    // `Vec::with_capacity` runs, when it cannot possibly fit the
    // bytes actually remaining. Mirrors the established
    // `len > reader.remaining()` guard in
    // crates/cdr/src/composite.rs.
    // -------------------------------------------------------------

    #[test]
    fn read_array_rejects_oversized_length() {
        let mut w = UaWriter::new();
        // Announce a huge element count, but supply no element bytes.
        w.write_i32(1_000_000_000);
        let mut r = UaReader::new(w.as_slice());
        let res = read_array::<Guid>(&mut r);
        assert!(
            matches!(res, Err(DecodeError::MalformedMessage { .. })),
            "expected clean rejection, got {res:?}"
        );
    }

    #[test]
    fn read_array_roundtrip_within_bound() {
        let items = alloc::vec![
            Guid {
                data1: 1,
                data2: 2,
                data3: 3,
                data4: [0u8; 8],
            },
            Guid {
                data1: 4,
                data2: 5,
                data3: 6,
                data4: [1u8; 8],
            },
        ];
        let mut w = UaWriter::new();
        write_array(&mut w, &items, "Guid array").expect("encode");
        let mut r = UaReader::new(w.as_slice());
        let back: Vec<Guid> = read_array(&mut r).expect("decode");
        assert_eq!(back, items);
    }

    #[test]
    fn read_u16_array_rejects_oversized_length() {
        let mut w = UaWriter::new();
        w.write_i32(1_000_000_000);
        let mut r = UaReader::new(w.as_slice());
        let res = read_u16_array(&mut r);
        assert!(matches!(res, Err(DecodeError::MalformedMessage { .. })));
    }

    #[test]
    fn read_u16_array_roundtrip_within_bound() {
        let items = alloc::vec![1u16, 2, 3, 65535];
        let mut w = UaWriter::new();
        write_u16_array(&mut w, &items).expect("encode");
        let mut r = UaReader::new(w.as_slice());
        assert_eq!(read_u16_array(&mut r).expect("decode"), items);
    }

    #[test]
    fn read_u32_array_rejects_oversized_length() {
        let mut w = UaWriter::new();
        w.write_i32(1_000_000_000);
        let mut r = UaReader::new(w.as_slice());
        let res = read_u32_array(&mut r);
        assert!(matches!(res, Err(DecodeError::MalformedMessage { .. })));
    }

    #[test]
    fn read_u32_array_roundtrip_within_bound() {
        let items = alloc::vec![1u32, 2, 3, u32::MAX];
        let mut w = UaWriter::new();
        write_u32_array(&mut w, &items).expect("encode");
        let mut r = UaReader::new(w.as_slice());
        assert_eq!(read_u32_array(&mut r).expect("decode"), items);
    }

    #[test]
    fn read_string_array_rejects_oversized_length() {
        let mut w = UaWriter::new();
        w.write_i32(1_000_000_000);
        let mut r = UaReader::new(w.as_slice());
        let res = read_string_array(&mut r);
        assert!(matches!(res, Err(DecodeError::MalformedMessage { .. })));
    }

    #[test]
    fn read_string_array_roundtrip_within_bound() {
        let items = alloc::vec![String::from("alpha"), String::from("beta")];
        let mut w = UaWriter::new();
        write_string_array(&mut w, &items).expect("encode");
        let mut r = UaReader::new(w.as_slice());
        assert_eq!(read_string_array(&mut r).expect("decode"), items);
    }
}
