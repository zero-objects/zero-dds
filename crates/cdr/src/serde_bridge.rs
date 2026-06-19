// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Serde bridge for the XCDR2 encoder/decoder.
//!
//! Implements `zerodds-xcdr2-rust-1.0` §11.3 — the optional `serde-bridge`
//! feature. Lets you encode any `serde::Serialize` type, via a JSON
//! intermediate form, into a `Vec<u8>` that follows the **same wire
//! format** as a native `DdsType` implementation — provided the type has
//! a deterministic field layout.
//!
//! **Important:** This bridge is **not a replacement** for `#[derive(DdsType)]`
//! or `idl-rust` codegen. It is a convenience layer for apps that already
//! model their data with `serde` and want to produce XCDR2 wire output
//! without a separate type definition.
//!
//! Wire format: serde -> JSON string -> XCDR2 string-encoded. The decoder
//! parses the JSON string back. Compatible with any DDS topic type
//! `Greeting{ string text; }` that receives the JSON as a plain string.
//!
//! The module is only compiled when the `serde-bridge` feature is active;
//! the `#[cfg(...)]` gate sits on `pub mod serde_bridge` in `lib.rs`.

use alloc::string::String;
use alloc::vec::Vec;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::buffer::{BufferReader, BufferWriter};
use crate::endianness::Endianness;
use crate::error::{DecodeError, EncodeError};

/// Serializes a `serde::Serialize` type to JSON, then XCDR2-string
/// encodes it. Default endianness: little-endian.
///
/// # Errors
/// - `EncodeError::ValueOutOfRange` on a serde serialization error.
/// - `EncodeError::*` from the CDR string writer.
pub fn encode_via_serde<T: Serialize>(value: &T) -> Result<Vec<u8>, EncodeError> {
    let json = serde_json::to_string(value).map_err(|_| EncodeError::ValueOutOfRange {
        message: "serde_json::to_string failed for serde-bridge",
    })?;
    let mut writer = BufferWriter::new(Endianness::Little);
    writer.write_string(&json)?;
    Ok(writer.into_bytes())
}

/// Deserializes an XCDR2-encoded string back into a
/// `serde::DeserializeOwned` type via JSON.
///
/// # Errors
/// - `DecodeError::*` from the CDR string reader.
/// - `DecodeError::InvalidUtf8` on a serde deserialization error
///   (offset = 0; the serde error detail is currently not propagated in
///   the error string, because `DecodeError` variants use
///   `&'static str`).
pub fn decode_via_serde<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    let mut reader = BufferReader::new(bytes, Endianness::Little);
    let json = reader.read_string()?;
    serde_json::from_str(&json).map_err(|_| DecodeError::InvalidUtf8 { offset: 0 })
}

/// Convenience wrapper: return encoded bytes as a JSON string.
///
/// Useful for logging/debugging — returns the JSON intermediate form,
/// not the bytes.
///
/// # Errors
/// `DecodeError` if the bytes are not a valid XCDR2 string.
pub fn decoded_json_repr(bytes: &[u8]) -> Result<String, DecodeError> {
    let mut reader = BufferReader::new(bytes, Endianness::Little);
    reader.read_string()
}
