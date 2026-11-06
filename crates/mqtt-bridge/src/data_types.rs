// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT v5.0 Data Types — §1.5.

use alloc::string::String;
use alloc::vec::Vec;

/// Spec §1.5.1 — `Bits`.
/// Spec §1.5.2 — `Two Byte Integer` (BE u16).
/// Spec §1.5.3 — `Four Byte Integer` (BE u32).
/// Spec §1.5.4 — `UTF-8 Encoded String` (u16 BE length + UTF-8).
/// Spec §1.5.6 — `Binary Data` (u16 BE length + bytes).
/// Spec §1.5.7 — `UTF-8 String Pair`.
///
/// This module provides encode/decode helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataTypeError {
    /// Input bytes truncated.
    Truncated,
    /// String has invalid UTF-8 (Spec §1.5.4 requires UTF-8 net
    /// unicode).
    InvalidUtf8,
    /// String/BinaryData over the u16 limit (>65535).
    LengthTooLarge,
}

impl core::fmt::Display for DataTypeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Truncated => "input truncated",
            Self::InvalidUtf8 => "invalid UTF-8 in MQTT string",
            Self::LengthTooLarge => "length exceeds u16 max",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DataTypeError {}

/// Spec §1.5.2 — Two Byte Integer (BE).
#[must_use]
pub const fn encode_two_byte_int(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

/// Spec §1.5.2 — Decode Two Byte Integer.
///
/// # Errors
/// `Truncated` if `bytes.len() < 2`.
pub fn decode_two_byte_int(bytes: &[u8]) -> Result<(u16, usize), DataTypeError> {
    if bytes.len() < 2 {
        return Err(DataTypeError::Truncated);
    }
    Ok((u16::from_be_bytes([bytes[0], bytes[1]]), 2))
}

/// Spec §1.5.4 — `UTF-8 Encoded String`. Returns the wire form with a u16 BE
/// length prefix.
///
/// # Errors
/// `LengthTooLarge` if `s.len() > u16::MAX`.
pub fn encode_utf8_string(s: &str) -> Result<Vec<u8>, DataTypeError> {
    let len = s.len();
    if len > u16::MAX as usize {
        return Err(DataTypeError::LengthTooLarge);
    }
    let mut out = Vec::with_capacity(2 + len);
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(out)
}

/// Spec §1.5.4 — Decode UTF-8 String.
///
/// # Errors
/// * `Truncated` if fewer than 2 length bytes or the string-length
///   bytes are missing.
/// * `InvalidUtf8` if the string is not valid UTF-8.
pub fn decode_utf8_string(bytes: &[u8]) -> Result<(String, usize), DataTypeError> {
    let (len, hdr) = decode_two_byte_int(bytes)?;
    let end = hdr + usize::from(len);
    if bytes.len() < end {
        return Err(DataTypeError::Truncated);
    }
    let s = core::str::from_utf8(&bytes[hdr..end])
        .map_err(|_| DataTypeError::InvalidUtf8)?
        .to_owned();
    Ok((s, end))
}

/// Spec §1.5.6 — `Binary Data` with a u16 BE length prefix.
///
/// # Errors
/// `LengthTooLarge` if `data.len() > u16::MAX`.
pub fn encode_binary_data(data: &[u8]) -> Result<Vec<u8>, DataTypeError> {
    let len = data.len();
    if len > u16::MAX as usize {
        return Err(DataTypeError::LengthTooLarge);
    }
    let mut out = Vec::with_capacity(2 + len);
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(data);
    Ok(out)
}

/// Spec §1.5.6 — Decode Binary Data.
///
/// # Errors
/// `Truncated` if fewer than 2 length bytes or the data bytes are missing.
pub fn decode_binary_data(bytes: &[u8]) -> Result<(Vec<u8>, usize), DataTypeError> {
    let (len, hdr) = decode_two_byte_int(bytes)?;
    let end = hdr + usize::from(len);
    if bytes.len() < end {
        return Err(DataTypeError::Truncated);
    }
    Ok((bytes[hdr..end].to_vec(), end))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn mqtt5_1_5_2_a_two_byte_integer_is_big_endian() {
        for (v, octets) in [
            (0u16, [0x00, 0x00]),
            (1, [0x00, 0x01]),
            (0xFF, [0x00, 0xFF]),
            (0x100, [0x01, 0x00]),
            (0xABCD, [0xAB, 0xCD]),
            (0xFFFF, [0xFF, 0xFF]),
        ] {
            assert_eq!(encode_two_byte_int(v), octets);
            assert_eq!(decode_two_byte_int(&octets).expect("decode"), (v, 2));
        }
    }

    #[test]
    fn two_byte_int_decode_truncated() {
        assert_eq!(decode_two_byte_int(&[]), Err(DataTypeError::Truncated));
        assert_eq!(decode_two_byte_int(&[0]), Err(DataTypeError::Truncated));
    }

    #[test]
    fn mqtt5_1_5_4_a_string_is_its_length_and_its_utf8_octets() {
        // Spec §1.5.4, Figure 1-1: "A\u{2A6D4}" is 0x0005 0x41 0xF0 0xAA 0x9B 0x94.
        for (s, octets) in [
            ("", alloc::vec![0x00, 0x00]),
            (
                "A\u{2A6D4}",
                alloc::vec![0x00, 0x05, 0x41, 0xF0, 0xAA, 0x9B, 0x94],
            ),
            (
                "Käfer",
                alloc::vec![0x00, 0x06, b'K', 0xC3, 0xA4, b'f', b'e', b'r'],
            ),
        ] {
            assert_eq!(encode_utf8_string(s).expect("encode"), octets);
            let (decoded, consumed) = decode_utf8_string(&octets).expect("decode");
            assert_eq!(decoded, s);
            assert_eq!(consumed, octets.len());
        }
    }

    #[test]
    fn mqtt5_1_5_utf8_string_starts_with_be_length() {
        let bytes = encode_utf8_string("hi").expect("encode");
        assert_eq!(&bytes[..2], &[0x00, 0x02]);
        assert_eq!(&bytes[2..], b"hi");
    }

    #[test]
    fn mqtt5_1_5_utf8_string_decode_invalid_utf8() {
        // 2 bytes len + 1 byte 0xFF (invalid utf-8 starting byte).
        let bytes = [0x00u8, 0x01, 0xFF];
        assert_eq!(decode_utf8_string(&bytes), Err(DataTypeError::InvalidUtf8));
    }

    #[test]
    fn mqtt5_1_5_utf8_string_decode_truncated() {
        // Length=10 but only 3 bytes.
        let bytes = [0x00u8, 0x0A, b'a'];
        assert_eq!(decode_utf8_string(&bytes), Err(DataTypeError::Truncated));
    }

    #[test]
    fn mqtt5_1_5_6_binary_data_is_its_length_and_its_octets() {
        for (data, length) in [
            (alloc::vec![], [0x00, 0x00]),
            (alloc::vec![0xDE, 0xAD, 0xBE, 0xEF], [0x00, 0x04]),
            (alloc::vec![0xFFu8; 1000], [0x03, 0xE8]),
        ] {
            let octets = [&length[..], &data[..]].concat();
            assert_eq!(encode_binary_data(&data).expect("encode"), octets);
            assert_eq!(
                decode_binary_data(&octets).expect("decode"),
                (data, octets.len())
            );
        }
    }
}
