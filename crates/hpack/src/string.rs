// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! String-literal coding — RFC 7541 §5.2.
//!
//! Header strings can be plain (raw) or Huffman-coded. We provide a
//! convenience API; the Huffman path delegates to
//! `crate::huffman`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::huffman;
use crate::integer::{IntegerError, decode_integer, encode_integer};

/// String coding error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringError {
    /// Integer length decode error.
    Integer(IntegerError),
    /// Buffer ends before the announced length.
    Truncated,
    /// Huffman decode error.
    Huffman,
    /// String contains invalid UTF-8 (HPACK allows octet strings,
    /// but HTTP headers are ASCII; the caller can read raw bytes via
    /// `decode_bytes`).
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

/// Encode a string. `huffman_compress=true` enables Huffman.
///
/// Format: 1-byte header (`H` flag in bit 7 + 7-bit length prefix) +
/// length continuation + octets.
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

/// Decode a string from a byte slice.
///
/// Returns `(decoded_string, bytes_consumed)`.
///
/// # Errors
/// Siehe [`StringError`].
pub fn decode_string(input: &[u8]) -> Result<(String, usize), StringError> {
    let (bytes, consumed) = decode_bytes(input)?;
    let s = String::from_utf8(bytes).map_err(|_| StringError::NotUtf8)?;
    Ok((s, consumed))
}

/// Decode raw as bytes (no UTF-8 check). Spec §5.2.
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
