// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! String-Literal-Coding — RFC 7541 §5.2.
//!
//! Header-Strings koennen plain (raw) oder Huffman-coded sein. Wir
//! liefern eine Convenience-API; Huffman-Pfad delegiert an
//! `crate::huffman`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::huffman;
use crate::integer::{IntegerError, decode_integer, encode_integer};

/// String-Coding-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringError {
    /// Integer-Length-Decode-Fehler.
    Integer(IntegerError),
    /// Buffer endet vor angekuendigter Length.
    Truncated,
    /// Huffman-Decode-Fehler.
    Huffman,
    /// String enthaelt invalides UTF-8 (HPACK erlaubt Octet-Strings,
    /// aber HTTP-Header sind ASCII; Caller kann ueber `decode_bytes`
    /// rohe Bytes lesen).
    NotUtf8,
}

impl core::fmt::Display for StringError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Integer(e) => write!(f, "integer: {e}"),
            Self::Truncated => f.write_str("string truncated"),
            Self::Huffman => f.write_str("huffman decode failed"),
            Self::NotUtf8 => f.write_str("string is not valid UTF-8"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StringError {}

impl From<IntegerError> for StringError {
    fn from(e: IntegerError) -> Self {
        Self::Integer(e)
    }
}

/// Encode einen String. `huffman_compress=true` aktiviert Huffman.
///
/// Format: 1 Byte Header (`H`-Flag in Bit 7 + 7-Bit-Length-Prefix) +
/// Length-Continuation + Octets.
#[must_use]
pub fn encode_string(s: &str, huffman_compress: bool) -> Vec<u8> {
    let octets: Vec<u8> = if huffman_compress {
        huffman::encode(s.as_bytes())
    } else {
        s.as_bytes().to_vec()
    };
    let prefix_byte = if huffman_compress { 0x80 } else { 0x00 };
    let mut out = encode_integer(octets.len() as u64, 7, prefix_byte);
    out.extend_from_slice(&octets);
    out
}

/// Decode einen String aus einem Byte-Slice.
///
/// Liefert `(decoded_string, bytes_consumed)`.
///
/// # Errors
/// Siehe [`StringError`].
pub fn decode_string(input: &[u8]) -> Result<(String, usize), StringError> {
    let (bytes, consumed) = decode_bytes(input)?;
    let s = String::from_utf8(bytes).map_err(|_| StringError::NotUtf8)?;
    Ok((s, consumed))
}

/// Decode roh als Bytes (kein UTF-8-Check). Spec §5.2.
///
/// # Errors
/// `Integer` / `Truncated` / `Huffman`.
pub fn decode_bytes(input: &[u8]) -> Result<(Vec<u8>, usize), StringError> {
    if input.is_empty() {
        return Err(StringError::Truncated);
    }
    let huffman_flag = (input[0] & 0x80) != 0;
    let (length, prefix_consumed) = decode_integer(input, 7)?;
    let length = length as usize;
    let total = prefix_consumed + length;
    if input.len() < total {
        return Err(StringError::Truncated);
    }
    let raw = &input[prefix_consumed..prefix_consumed + length];
    let decoded = if huffman_flag {
        huffman::decode(raw).map_err(|_| StringError::Huffman)?
    } else {
        raw.to_vec()
    };
    Ok((decoded, total))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn encode_plain_no_huffman() {
        let buf = encode_string("hello", false);
        assert_eq!(buf[0], 0x05); // length=5, no H-flag
        assert_eq!(&buf[1..], b"hello");
    }

    #[test]
    fn round_trip_plain() {
        let buf = encode_string("Content-Type", false);
        let (s, _) = decode_string(&buf).unwrap();
        assert_eq!(s, "Content-Type");
    }

    #[test]
    fn round_trip_huffman() {
        let buf = encode_string("www.example.com", true);
        assert_eq!(buf[0] & 0x80, 0x80);
        let (s, _) = decode_string(&buf).unwrap();
        assert_eq!(s, "www.example.com");
    }

    #[test]
    fn truncated_input_rejected() {
        let buf = alloc::vec![0x05, b'h'];
        assert!(matches!(decode_string(&buf), Err(StringError::Truncated)));
    }

    #[test]
    fn empty_input_rejected() {
        assert!(matches!(decode_string(&[]), Err(StringError::Truncated)));
    }

    #[test]
    fn long_string_uses_continuation() {
        let s: String = "a".repeat(200);
        let buf = encode_string(&s, false);
        assert_eq!(buf[0], 0x7f);
        let (back, _) = decode_string(&buf).unwrap();
        assert_eq!(back.len(), 200);
    }

    #[test]
    fn empty_string_round_trip() {
        let buf = encode_string("", false);
        assert_eq!(buf, alloc::vec![0x00]);
        let (s, _) = decode_string(&buf).unwrap();
        assert!(s.is_empty());
    }
}
