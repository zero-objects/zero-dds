// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Body-Encoding-Mode-Mapping (Pass-Through / JSON / AMQP-Native).
//!
//! Spec-Quelle: dds-amqp-1.0-beta1.pdf §8.1 Body Encoding Modes.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Spec §8.1 — Body-Encoding-Mode pro Topic-Mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyEncodingMode {
    /// Spec §8.1.1 — Pass-Through (XCDR2-Bytes verbatim).
    #[default]
    PassThrough,
    /// Spec §8.1.2 — JSON-Mapping.
    Json,
    /// Spec §8.1.3 — AMQP-Native typed mapping.
    AmqpNative,
}

/// Mapping-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    /// JSON-Body ist nicht UTF-8.
    InvalidUtf8,
    /// JSON-Body ist nicht gueltiges JSON.
    InvalidJson(String),
    /// Pass-Through-Body ist leer (typischerweise illegal).
    EmptyBody,
}

/// Spec §8.1 — Encode eines DDS-Sample-Bodies in den AMQP-Body.
///
/// Returns Tuple (content_type, body_bytes).
///
/// # Errors
/// `MappingError`.
pub fn encode_dds_to_amqp_body(
    sample_xcdr2: &[u8],
    mode: BodyEncodingMode,
) -> Result<(&'static str, Vec<u8>), MappingError> {
    if sample_xcdr2.is_empty() {
        return Err(MappingError::EmptyBody);
    }
    match mode {
        BodyEncodingMode::PassThrough => Ok(("application/vnd.dds.xcdr2", sample_xcdr2.to_vec())),
        BodyEncodingMode::Json => {
            // Wir koennen XCDR2 nicht ohne Type-Information in JSON
            // umsetzen — Spec §8.1.2 verlangt Type-Reflektion. Hier
            // liefern wir nur Pass-Through-Base64 als Fallback;
            // der Caller mit echtem TypeObject kann eine richtige
            // Konvertierung liefern.
            let mut json = String::from("{\"_xcdr2\":\"");
            for b in sample_xcdr2 {
                let _ = core::fmt::Write::write_fmt(&mut json, core::format_args!("{b:02x}"));
            }
            json.push_str("\"}");
            Ok(("application/json", json.into_bytes()))
        }
        BodyEncodingMode::AmqpNative => {
            Ok(("application/vnd.dds.amqp-native", sample_xcdr2.to_vec()))
        }
    }
}

/// Spec §8.1 — Parse eines AMQP-Bodies in einen DDS-Sample-Body
/// (XCDR2-Bytes).
///
/// # Errors
/// `MappingError`.
pub fn parse_amqp_body(body: &[u8], content_type: Option<&str>) -> Result<Vec<u8>, MappingError> {
    if body.is_empty() {
        return Err(MappingError::EmptyBody);
    }
    let mode = match content_type {
        Some("application/vnd.dds.xcdr2") => BodyEncodingMode::PassThrough,
        Some("application/json") => BodyEncodingMode::Json,
        Some("application/vnd.dds.amqp-native") => BodyEncodingMode::AmqpNative,
        _ => BodyEncodingMode::PassThrough, // default
    };
    match mode {
        BodyEncodingMode::PassThrough | BodyEncodingMode::AmqpNative => Ok(body.to_vec()),
        BodyEncodingMode::Json => {
            let s = core::str::from_utf8(body).map_err(|_| MappingError::InvalidUtf8)?;
            // Suchen wir das _xcdr2-Hex-Field aus unserem Reverse-
            // Encoding (rudimentaerer Roundtrip ohne Type-Info).
            let key = "\"_xcdr2\":\"";
            let start = s
                .find(key)
                .ok_or_else(|| MappingError::InvalidJson("missing _xcdr2 field".to_string()))?;
            let hex_start = start + key.len();
            let hex_end = s[hex_start..]
                .find('"')
                .ok_or_else(|| MappingError::InvalidJson("unterminated _xcdr2 hex".to_string()))?;
            let hex = &s[hex_start..hex_start + hex_end];
            decode_hex(hex).map_err(|e| MappingError::InvalidJson(e.to_string()))
        }
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, &'static str> {
    if s.len() % 2 != 0 {
        return Err("hex length not even");
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_digit(bytes[i])?;
        let lo = hex_digit(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

const fn hex_digit(b: u8) -> Result<u8, &'static str> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err("invalid hex digit"),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_round_trip_preserves_bytes() {
        let sample = alloc::vec![0xDE, 0xAD, 0xBE, 0xEF];
        let (ct, body) =
            encode_dds_to_amqp_body(&sample, BodyEncodingMode::PassThrough).expect("encode");
        assert_eq!(ct, "application/vnd.dds.xcdr2");
        let parsed = parse_amqp_body(&body, Some(ct)).expect("parse");
        assert_eq!(parsed, sample);
    }

    #[test]
    fn json_round_trip_via_hex_field() {
        let sample = alloc::vec![0x01, 0x02, 0x03, 0xFE];
        let (ct, body) = encode_dds_to_amqp_body(&sample, BodyEncodingMode::Json).expect("encode");
        assert_eq!(ct, "application/json");
        let parsed = parse_amqp_body(&body, Some(ct)).expect("parse");
        assert_eq!(parsed, sample);
    }

    #[test]
    fn amqp_native_uses_correct_content_type() {
        let sample = alloc::vec![0xCA, 0xFE];
        let (ct, _) =
            encode_dds_to_amqp_body(&sample, BodyEncodingMode::AmqpNative).expect("encode");
        assert_eq!(ct, "application/vnd.dds.amqp-native");
    }

    #[test]
    fn empty_body_yields_error_in_encode() {
        assert!(matches!(
            encode_dds_to_amqp_body(&[], BodyEncodingMode::PassThrough),
            Err(MappingError::EmptyBody)
        ));
    }

    #[test]
    fn empty_body_yields_error_in_parse() {
        assert!(matches!(
            parse_amqp_body(&[], None),
            Err(MappingError::EmptyBody)
        ));
    }

    #[test]
    fn unknown_content_type_falls_back_to_pass_through() {
        let body = alloc::vec![1, 2, 3];
        let parsed = parse_amqp_body(&body, Some("application/octet-stream")).expect("parse");
        assert_eq!(parsed, body);
    }

    #[test]
    fn invalid_json_yields_error() {
        let r = parse_amqp_body(b"{invalid", Some("application/json"));
        assert!(matches!(r, Err(MappingError::InvalidJson(_))));
    }

    #[test]
    fn default_mode_is_pass_through() {
        assert_eq!(BodyEncodingMode::default(), BodyEncodingMode::PassThrough);
    }
}
