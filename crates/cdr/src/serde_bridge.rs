// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Serde-Bridge fuer XCDR2 Encoder/Decoder.
//!
//! Implementiert `zerodds-xcdr2-rust-1.0` §11.3 — Optional-Feature
//! `serde-bridge`. Erlaubt es, einen beliebigen `serde::Serialize`-Type
//! ueber JSON-Zwischenform in einen `Vec<u8>` zu encodieren der **dem
//! gleichen Wire-Format** folgt wie eine native `DdsType`-Implementation
//! — vorausgesetzt der Type hat ein deterministisches Field-Layout.
//!
//! **Wichtig:** Diese Bridge ist **kein Ersatz** fuer `#[derive(DdsType)]`
//! oder `idl-rust`-Codegen. Sie ist eine Convenience-Schicht fuer
//! Apps die ihre Datenmodelle bereits `serde`-modelliert haben und
//! XCDR2-Wire ohne separate Type-Definition erzeugen wollen.
//!
//! Wire-Format: serde -> JSON-String -> XCDR2-string-encoded. Decoder
//! parst JSON-String zurueck. Kompatibel mit jedem DDS-Topic-Type
//! `Greeting{ string text; }` der das JSON als Plain-String empfaengt.
//!
//! Das Modul wird nur kompiliert wenn das `serde-bridge` Feature aktiv
//! ist; das `#[cfg(...)]`-Gate liegt am `pub mod serde_bridge` in
//! `lib.rs`.

use alloc::string::String;
use alloc::vec::Vec;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::buffer::{BufferReader, BufferWriter};
use crate::endianness::Endianness;
use crate::error::{DecodeError, EncodeError};

/// Serialisiert einen `serde::Serialize`-Type zu JSON, dann als
/// XCDR2-string encoded. Default-Endian: Little-Endian.
///
/// # Errors
/// - `EncodeError::ValueOutOfRange` bei serde-Serialization-Fehler.
/// - `EncodeError::*` aus dem CDR-String-Writer.
pub fn encode_via_serde<T: Serialize>(value: &T) -> Result<Vec<u8>, EncodeError> {
    let json = serde_json::to_string(value).map_err(|_| EncodeError::ValueOutOfRange {
        message: "serde_json::to_string failed for serde-bridge",
    })?;
    let mut writer = BufferWriter::new(Endianness::Little);
    writer.write_string(&json)?;
    Ok(writer.into_bytes())
}

/// Deserialisiert einen XCDR2-encoded String zurueck zu einem
/// `serde::DeserializeOwned`-Type via JSON.
///
/// # Errors
/// - `DecodeError::*` aus dem CDR-String-Reader.
/// - `DecodeError::InvalidUtf8` bei serde-Deserialization-Fehler
///   (offset = 0; serde-error-detail im Fehler-String wird heute nicht
///   weitergereicht, weil `DecodeError`-Variants `&'static str`
///   verwenden).
pub fn decode_via_serde<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    let mut reader = BufferReader::new(bytes, Endianness::Little);
    let json = reader.read_string()?;
    serde_json::from_str(&json).map_err(|_| DecodeError::InvalidUtf8 { offset: 0 })
}

/// Convenience-Wrapper: encoded Bytes als JSON-String zurueck.
///
/// Nuetzlich fuer Logging/Debugging — gibt die JSON-Zwischenform
/// zurueck, nicht die Bytes.
///
/// # Errors
/// `DecodeError` wenn die Bytes kein gueltiger XCDR2-string sind.
pub fn decoded_json_repr(bytes: &[u8]) -> Result<String, DecodeError> {
    let mut reader = BufferReader::new(bytes, Endianness::Little);
    reader.read_string()
}
