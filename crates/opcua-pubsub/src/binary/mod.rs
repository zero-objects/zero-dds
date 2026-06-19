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

/// Reads an `Int32`-length-prefixed array (`-1` = null → empty).
pub(crate) fn read_array<T: UaDecode>(r: &mut UaReader<'_>) -> Result<Vec<T>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(len as usize);
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
    let mut out = Vec::with_capacity(len as usize);
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
    let mut out = Vec::with_capacity(len as usize);
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
    let mut out = Vec::with_capacity(len as usize);
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
