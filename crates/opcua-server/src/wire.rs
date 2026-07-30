// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Small OPC-UA Part-6 §5.2 wire helpers (Int32-prefixed strings / byte
//! strings / arrays) used by the service codecs. Built on the
//! `zerodds-opcua-pubsub` `UaReader`/`UaWriter`. (Candidate for extraction
//! into a shared `opcua-binary` crate together with the PubSub codec.)

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_pubsub::binary::check_array_len;
use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaDecode, UaEncode, UaReader, UaWriter};

/// Writes a non-nullable UTF-8 string (`Int32` byte length + bytes).
pub fn write_string(w: &mut UaWriter, s: &str) -> Result<(), EncodeError> {
    let len = i32::try_from(s.len()).map_err(|_| EncodeError::LengthOverflow {
        what: "String",
        len: s.len(),
    })?;
    w.write_i32(len);
    w.write_bytes(s.as_bytes());
    Ok(())
}

/// Reads a string; the null form (`-1`) maps to the empty string.
pub fn read_string(r: &mut UaReader<'_>) -> Result<String, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(String::new());
    }
    let bytes = r.read_bytes(len as usize)?;
    core::str::from_utf8(bytes)
        .map(String::from)
        .map_err(|_| DecodeError::InvalidUtf8)
}

/// Writes a byte string (`Int32` length + bytes); empty encodes as null (`-1`).
pub fn write_byte_string(w: &mut UaWriter, b: &[u8]) -> Result<(), EncodeError> {
    if b.is_empty() {
        w.write_i32(-1);
        return Ok(());
    }
    let len = i32::try_from(b.len()).map_err(|_| EncodeError::LengthOverflow {
        what: "ByteString",
        len: b.len(),
    })?;
    w.write_i32(len);
    w.write_bytes(b);
    Ok(())
}

/// Reads a byte string; the null form (`-1`) maps to an empty vector.
pub fn read_byte_string(r: &mut UaReader<'_>) -> Result<Vec<u8>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    Ok(r.read_bytes(len as usize)?.to_vec())
}

/// Writes an `Int32`-length-prefixed array of encodable elements.
pub fn write_array<T: UaEncode>(
    w: &mut UaWriter,
    items: &[T],
    what: &'static str,
) -> Result<(), EncodeError> {
    let len = i32::try_from(items.len()).map_err(|_| EncodeError::LengthOverflow {
        what,
        len: items.len(),
    })?;
    w.write_i32(len);
    for it in items {
        it.encode(w)?;
    }
    Ok(())
}

/// Reads an `Int32`-length-prefixed array (`-1` = null → empty).
pub fn read_array<T: UaDecode>(r: &mut UaReader<'_>) -> Result<Vec<T>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    let len = len as usize;
    // Conservative bound: every element needs >= 1 wire byte, so a count
    // above the remaining bytes cannot be genuine. Reject before
    // `Vec::with_capacity` (mirrors `crates/cdr/src/composite.rs`'s
    // `len > reader.remaining()` guard).
    check_array_len(r, len, 1, "array length exceeds remaining bytes")?;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(T::decode(r)?);
    }
    Ok(out)
}

/// Writes an `Int32`-length-prefixed `u32`/StatusCode array.
pub fn write_u32_array(w: &mut UaWriter, items: &[u32]) -> Result<(), EncodeError> {
    let len = i32::try_from(items.len()).map_err(|_| EncodeError::LengthOverflow {
        what: "UInt32 array",
        len: items.len(),
    })?;
    w.write_i32(len);
    for x in items {
        w.write_u32(*x);
    }
    Ok(())
}

/// Reads an `Int32`-length-prefixed `u32` array (`-1` = null → empty).
pub fn read_u32_array(r: &mut UaReader<'_>) -> Result<Vec<u32>, DecodeError> {
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

/// Writes an `Int32`-length-prefixed string array.
pub fn write_string_array(w: &mut UaWriter, items: &[String]) -> Result<(), EncodeError> {
    let len = i32::try_from(items.len()).map_err(|_| EncodeError::LengthOverflow {
        what: "String array",
        len: items.len(),
    })?;
    w.write_i32(len);
    for s in items {
        write_string(w, s)?;
    }
    Ok(())
}

/// Reads an `Int32`-length-prefixed string array (`-1` = null → empty).
pub fn read_string_array(r: &mut UaReader<'_>) -> Result<Vec<String>, DecodeError> {
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

/// Hard cap on `InnerDiagnosticInfo` nesting — untrusted input could otherwise
/// nest arbitrarily deep and overflow the stack.
const MAX_DIAGNOSTIC_INFO_DEPTH: u8 = 16;

/// Reads (and discards) one `DiagnosticInfo` (Part 6 §5.2.2.12), advancing past
/// all present fields.
pub fn skip_one_diagnostic_info(r: &mut UaReader<'_>) -> Result<(), DecodeError> {
    skip_one_diagnostic_info_depth(r, 0)
}

/// zerodds-lint: recursion-depth 16 (InnerDiagnosticInfo nesting; hard-capped by
/// `MAX_DIAGNOSTIC_INFO_DEPTH`, untrusted input rejected beyond it).
fn skip_one_diagnostic_info_depth(r: &mut UaReader<'_>, depth: u8) -> Result<(), DecodeError> {
    if depth >= MAX_DIAGNOSTIC_INFO_DEPTH {
        return Err(DecodeError::MalformedMessage {
            message: "DiagnosticInfo nested beyond the allowed depth",
        });
    }
    let mask = r.read_u8()?;
    if mask & 0x01 != 0 {
        r.read_i32()?;
    }
    if mask & 0x02 != 0 {
        r.read_i32()?;
    }
    if mask & 0x04 != 0 {
        r.read_i32()?;
    }
    if mask & 0x08 != 0 {
        r.read_i32()?;
    }
    if mask & 0x10 != 0 {
        read_string(r)?;
    }
    if mask & 0x20 != 0 {
        r.read_u32()?;
    }
    if mask & 0x40 != 0 {
        skip_one_diagnostic_info_depth(r, depth + 1)?;
    }
    Ok(())
}

/// Writes an empty `DiagnosticInfo[]` (the usual service `diagnostic_infos`).
pub fn write_empty_diagnostic_info_array(w: &mut UaWriter) {
    w.write_i32(0);
}

/// Reads and discards a `DiagnosticInfo[]`.
pub fn skip_diagnostic_info_array(r: &mut UaReader<'_>) -> Result<(), DecodeError> {
    let n = r.read_i32()?;
    if n > 0 {
        for _ in 0..n {
            skip_one_diagnostic_info(r)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_opcua_gateway::types::Guid;

    // -------------------------------------------------------------
    // Buffer-cap hardening — a wire-supplied `Int32` element count
    // must be rejected cleanly (no OOM, no panic) before
    // `Vec::with_capacity` runs. Mirrors the established
    // `len > reader.remaining()` guard in
    // crates/cdr/src/composite.rs.
    // -------------------------------------------------------------

    #[test]
    fn read_array_rejects_oversized_length() {
        let mut w = UaWriter::new();
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
