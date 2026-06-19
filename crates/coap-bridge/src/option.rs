// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP option model — RFC 7252 §3.1, §5.10 + RFC 7641 §2.

use alloc::string::String;
use alloc::vec::Vec;

/// Option-number registry — RFC 7252 §12.2 + RFC 7641 §2.
///
/// Modeled as `u16` (spec: option numbers up to 16-bit theoretical)
/// — we provide named constants for the standard numbers.
pub type OptionNumber = u16;

/// Standard option numbers from RFC 7252 §5.10 + RFC 7641 §2.
pub mod numbers {
    /// `If-Match` (RFC 7252 §5.10.8.1).
    pub const IF_MATCH: u16 = 1;
    /// `Uri-Host` (§5.10.1).
    pub const URI_HOST: u16 = 3;
    /// `ETag` (§5.10.6).
    pub const ETAG: u16 = 4;
    /// `If-None-Match` (§5.10.8.2).
    pub const IF_NONE_MATCH: u16 = 5;
    /// `Observe` (RFC 7641 §2).
    pub const OBSERVE: u16 = 6;
    /// `Uri-Port` (§5.10.1).
    pub const URI_PORT: u16 = 7;
    /// `Location-Path` (§5.10.7).
    pub const LOCATION_PATH: u16 = 8;
    /// `Uri-Path` (§5.10.1).
    pub const URI_PATH: u16 = 11;
    /// `Content-Format` (§5.10.3).
    pub const CONTENT_FORMAT: u16 = 12;
    /// `Max-Age` (§5.10.5).
    pub const MAX_AGE: u16 = 14;
    /// `Uri-Query` (§5.10.1).
    pub const URI_QUERY: u16 = 15;
    /// `Accept` (§5.10.4).
    pub const ACCEPT: u16 = 17;
    /// `Location-Query` (§5.10.7).
    pub const LOCATION_QUERY: u16 = 20;
    /// `Proxy-Uri` (§5.10.2).
    pub const PROXY_URI: u16 = 35;
    /// `Proxy-Scheme` (§5.10.2).
    pub const PROXY_SCHEME: u16 = 39;
    /// `Size1` (§5.10.9).
    pub const SIZE1: u16 = 60;
    /// `Block2` (RFC 7959 §2.1).
    pub const BLOCK2: u16 = 23;
    /// `Block1` (RFC 7959 §2.1).
    pub const BLOCK1: u16 = 27;
    /// `Size2` (RFC 7959 §4).
    pub const SIZE2: u16 = 28;
}

/// Spec §3.2 — Option-Value-Format (`empty`, `opaque`, `uint`, `string`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    /// `empty` — zero-length sequence.
    Empty,
    /// `opaque` — opaque bytes.
    Opaque(Vec<u8>),
    /// `uint` — Network-byte-order unsigned integer.
    Uint(u64),
    /// `string` — UTF-8 Net-Unicode.
    String(String),
}

impl OptionValue {
    /// Returns the wire bytes (RFC 7252 §3.2).
    #[must_use]
    pub fn to_wire_bytes(&self) -> Vec<u8> {
        match self {
            Self::Empty => Vec::new(),
            Self::Opaque(b) => b.clone(),
            Self::Uint(v) => uint_to_minimal_bytes(*v),
            Self::String(s) => s.as_bytes().to_vec(),
        }
    }
}

/// Encodes a `uint` value with the minimal number of leading bytes
/// (RFC 7252 §3.2: "without leading zero bytes").
fn uint_to_minimal_bytes(v: u64) -> Vec<u8> {
    if v == 0 {
        return Vec::new();
    }
    let mut buf = v.to_be_bytes().to_vec();
    while let Some(0) = buf.first() {
        buf.remove(0);
    }
    buf
}

/// Decodes a `uint` value from raw bytes (RFC 7252 §3.2: "MUST be
/// prepared to process values with leading zero bytes").
///
/// # Errors
/// Returns `None` if `bytes.len() > 8` (a uint can be at most `u64`).
#[must_use]
pub fn uint_from_bytes(bytes: &[u8]) -> Option<u64> {
    if bytes.len() > 8 {
        return None;
    }
    let mut acc: u64 = 0;
    for b in bytes {
        acc = (acc << 8) | u64::from(*b);
    }
    Some(acc)
}

/// CoAP Option (Number + Value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoapOption {
    /// Option number (RFC 7252 §3.1).
    pub number: OptionNumber,
    /// Option value.
    pub value: OptionValue,
}

impl CoapOption {
    /// Constructor with number + value.
    #[must_use]
    pub const fn new(number: OptionNumber, value: OptionValue) -> Self {
        Self { number, value }
    }

    /// Convenience constructor for the `Uri-Path` option (RFC 7252 §5.10.1).
    #[must_use]
    pub fn uri_path(segment: impl Into<String>) -> Self {
        Self::new(numbers::URI_PATH, OptionValue::String(segment.into()))
    }

    /// Convenience for the `Observe` option (RFC 7641 §2). Value 0 =
    /// "register", 1 = "deregister", 2-0xFFFFFF = sequence number.
    #[must_use]
    pub const fn observe(value: u32) -> Self {
        Self::new(numbers::OBSERVE, OptionValue::Uint(value as u64))
    }

    /// Convenience for `Content-Format` (RFC 7252 §5.10.3).
    #[must_use]
    pub const fn content_format(format: u16) -> Self {
        Self::new(numbers::CONTENT_FORMAT, OptionValue::Uint(format as u64))
    }

    /// Convenience for `Block1` (RFC 7959 §2.1) as an opaque-encoded value.
    #[must_use]
    pub fn block1(num: u32, more: bool, szx: u8) -> Self {
        let v = crate::blockwise::BlockValue { num, more, szx };
        let bytes = v.encode().unwrap_or_default();
        Self::new(numbers::BLOCK1, OptionValue::Opaque(bytes))
    }

    /// Convenience for `Block2` (RFC 7959 §2.1) as an opaque-encoded value.
    #[must_use]
    pub fn block2(num: u32, more: bool, szx: u8) -> Self {
        let v = crate::blockwise::BlockValue { num, more, szx };
        let bytes = v.encode().unwrap_or_default();
        Self::new(numbers::BLOCK2, OptionValue::Opaque(bytes))
    }

    /// Convenience for `Size2` (RFC 7959 §4).
    #[must_use]
    pub const fn size2(size: u64) -> Self {
        Self::new(numbers::SIZE2, OptionValue::Uint(size))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn uint_zero_is_empty_byte_sequence() {
        // RFC 7252 §3.2 — "the number 0 is represented with an empty
        // option value (a zero-length sequence of bytes)".
        assert!(uint_to_minimal_bytes(0).is_empty());
    }

    #[test]
    fn uint_one_is_single_byte() {
        // RFC 7252 §3.2 — "the number 1 by a single byte with the
        // numerical value of 1".
        assert_eq!(uint_to_minimal_bytes(1), alloc::vec![1]);
    }

    #[test]
    fn uint_strips_leading_zero_bytes() {
        assert_eq!(uint_to_minimal_bytes(0xFF), alloc::vec![0xFF]);
        assert_eq!(uint_to_minimal_bytes(0x100), alloc::vec![0x01, 0x00]);
        assert_eq!(
            uint_to_minimal_bytes(0x12_3456),
            alloc::vec![0x12, 0x34, 0x56]
        );
    }

    #[test]
    fn uint_from_bytes_decodes_minimal_form() {
        assert_eq!(uint_from_bytes(&[]), Some(0));
        assert_eq!(uint_from_bytes(&[1]), Some(1));
        assert_eq!(uint_from_bytes(&[0xFF, 0xFF]), Some(0xFFFF));
    }

    #[test]
    fn uint_from_bytes_handles_leading_zeros() {
        // RFC 7252 §3.2 — "MUST be prepared to process values with
        // leading zero bytes".
        assert_eq!(uint_from_bytes(&[0, 0, 0, 1]), Some(1));
    }

    #[test]
    fn uint_from_bytes_rejects_more_than_8_bytes() {
        assert_eq!(uint_from_bytes(&[0; 9]), None);
    }

    #[test]
    fn option_uri_path_constructs_string_value() {
        // RFC 7252 §5.10.1.
        let o = CoapOption::uri_path("sensors");
        assert_eq!(o.number, numbers::URI_PATH);
        assert_eq!(o.value, OptionValue::String(String::from("sensors")));
    }

    #[test]
    fn observe_option_uses_number_6() {
        // RFC 7641 §2.
        let o = CoapOption::observe(0);
        assert_eq!(o.number, numbers::OBSERVE);
        assert_eq!(o.value, OptionValue::Uint(0));
    }

    #[test]
    fn content_format_uses_number_12() {
        // RFC 7252 §5.10.3.
        let o = CoapOption::content_format(50); // "application/json".
        assert_eq!(o.number, numbers::CONTENT_FORMAT);
        assert_eq!(o.value, OptionValue::Uint(50));
    }

    #[test]
    fn option_value_to_wire_bytes_matches_format() {
        assert!(OptionValue::Empty.to_wire_bytes().is_empty());
        assert_eq!(
            OptionValue::Opaque(alloc::vec![1, 2, 3]).to_wire_bytes(),
            alloc::vec![1, 2, 3]
        );
        assert_eq!(OptionValue::Uint(0).to_wire_bytes(), Vec::<u8>::new());
        assert_eq!(OptionValue::Uint(1).to_wire_bytes(), alloc::vec![1]);
        assert_eq!(
            OptionValue::String(String::from("hi")).to_wire_bytes(),
            alloc::vec![b'h', b'i']
        );
    }
}
