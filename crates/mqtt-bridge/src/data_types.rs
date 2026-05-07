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
/// Diese Module bietet Encode-/Decode-Helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataTypeError {
    /// Input bytes truncated.
    Truncated,
    /// String hat invalides UTF-8 (Spec §1.5.4 verlangt UTF-8 Net-
    /// Unicode).
    InvalidUtf8,
    /// String/BinaryData ueber u16-Limit (>65535).
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
/// `Truncated` wenn `bytes.len() < 2`.
pub fn decode_two_byte_int(bytes: &[u8]) -> Result<(u16, usize), DataTypeError> {
    if bytes.len() < 2 {
        return Err(DataTypeError::Truncated);
    }
    Ok((u16::from_be_bytes([bytes[0], bytes[1]]), 2))
}

/// Spec §1.5.4 — `UTF-8 Encoded String`. Liefert wire-form mit u16 BE
/// Length-Prefix.
///
/// # Errors
/// `LengthTooLarge` wenn `s.len() > u16::MAX`.
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
/// * `Truncated` wenn weniger als 2 Length-Bytes oder String-Length-
///   Bytes fehlen.
/// * `InvalidUtf8` wenn der String kein gueltiges UTF-8 ist.
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

/// Spec §1.5.6 — `Binary Data` mit u16 BE Length-Prefix.
///
/// # Errors
/// `LengthTooLarge` wenn `data.len() > u16::MAX`.
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
/// `Truncated` wenn weniger als 2 Length-Bytes oder Daten-Bytes fehlen.
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
    fn two_byte_int_round_trip() {
        for v in [0u16, 1, 0xFF, 0x100, 0xABCD, 0xFFFF] {
            let bytes = encode_two_byte_int(v);
            let (decoded, consumed) = decode_two_byte_int(&bytes).expect("decode");
            assert_eq!(decoded, v);
            assert_eq!(consumed, 2);
        }
    }

    #[test]
    fn two_byte_int_decode_truncated() {
        assert_eq!(decode_two_byte_int(&[]), Err(DataTypeError::Truncated));
        assert_eq!(decode_two_byte_int(&[0]), Err(DataTypeError::Truncated));
    }

    #[test]
    fn utf8_string_round_trip() {
        // Spec §1.5.4.
        for s in ["", "hello", "topic/foo/bar", "Käfer äöü"] {
            let bytes = encode_utf8_string(s).expect("encode");
            let (decoded, consumed) = decode_utf8_string(&bytes).expect("decode");
            assert_eq!(decoded, s);
            assert_eq!(consumed, bytes.len());
        }
    }

    #[test]
    fn utf8_string_starts_with_be_length() {
        let bytes = encode_utf8_string("hi").expect("encode");
        assert_eq!(&bytes[..2], &[0x00, 0x02]);
        assert_eq!(&bytes[2..], b"hi");
    }

    #[test]
    fn utf8_string_decode_invalid_utf8() {
        // 2 bytes len + 1 byte 0xFF (invalid utf-8 starting byte).
        let bytes = [0x00u8, 0x01, 0xFF];
        assert_eq!(decode_utf8_string(&bytes), Err(DataTypeError::InvalidUtf8));
    }

    #[test]
    fn utf8_string_decode_truncated() {
        // Length=10 but only 3 bytes.
        let bytes = [0x00u8, 0x0A, b'a'];
        assert_eq!(decode_utf8_string(&bytes), Err(DataTypeError::Truncated));
    }

    #[test]
    fn binary_data_round_trip() {
        // Spec §1.5.6.
        for data in [
            alloc::vec![],
            alloc::vec![0u8],
            alloc::vec![0xDE, 0xAD, 0xBE, 0xEF],
            alloc::vec![0xFFu8; 1000],
        ] {
            let bytes = encode_binary_data(&data).expect("encode");
            let (decoded, _) = decode_binary_data(&bytes).expect("decode");
            assert_eq!(decoded, data);
        }
    }
}
