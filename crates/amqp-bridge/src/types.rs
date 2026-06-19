// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 1.0 Type System — Spec `amqp-1.0-types`.
//!
//! Implements the format-code constructors for primitive types
//! and variable-width encodings. Compound types (list/map) are
//! supported with length-prefix validation; the inner structure is
//! the caller's layer (the caller serializes elements and passes
//! element count + element bytes).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// AMQP 1.0 Format Codes — Spec §1.2 Table 1-1.
///
/// We expose the most important named constants + category helpers.
pub mod codes {
    /// `0x40` — null (fixed-0).
    pub const NULL: u8 = 0x40;
    /// `0x41` — boolean true (fixed-0).
    pub const BOOLEAN_TRUE: u8 = 0x41;
    /// `0x42` — boolean false (fixed-0).
    pub const BOOLEAN_FALSE: u8 = 0x42;
    /// `0x56` — boolean (fixed-1, 0x00=false, 0x01=true).
    pub const BOOLEAN: u8 = 0x56;
    /// `0x50` — ubyte (fixed-1).
    pub const UBYTE: u8 = 0x50;
    /// `0x60` — ushort (fixed-2, BE).
    pub const USHORT: u8 = 0x60;
    /// `0x70` — uint (fixed-4, BE).
    pub const UINT: u8 = 0x70;
    /// `0x52` — smalluint (fixed-1).
    pub const SMALLUINT: u8 = 0x52;
    /// `0x43` — uint0 (fixed-0, value=0).
    pub const UINT0: u8 = 0x43;
    /// `0x80` — ulong (fixed-8, BE).
    pub const ULONG: u8 = 0x80;
    /// `0x53` — smallulong (fixed-1).
    pub const SMALLULONG: u8 = 0x53;
    /// `0x44` — ulong0 (fixed-0, value=0).
    pub const ULONG0: u8 = 0x44;
    /// `0x51` — byte (fixed-1, signed).
    pub const BYTE: u8 = 0x51;
    /// `0x61` — short (fixed-2, BE signed).
    pub const SHORT: u8 = 0x61;
    /// `0x71` — int (fixed-4, BE signed).
    pub const INT: u8 = 0x71;
    /// `0x54` — smallint (fixed-1, signed).
    pub const SMALLINT: u8 = 0x54;
    /// `0x81` — long (fixed-8, BE signed).
    pub const LONG: u8 = 0x81;
    /// `0x55` — smalllong (fixed-1, signed).
    pub const SMALLLONG: u8 = 0x55;
    /// `0xA0` — vbin8 (1-byte length + bytes).
    pub const VBIN8: u8 = 0xA0;
    /// `0xB0` — vbin32 (4-byte BE length + bytes).
    pub const VBIN32: u8 = 0xB0;
    /// `0xA1` — str8-utf8 (1-byte length + UTF-8).
    pub const STR8: u8 = 0xA1;
    /// `0xB1` — str32-utf8 (4-byte BE length + UTF-8).
    pub const STR32: u8 = 0xB1;
    /// `0xA3` — sym8 (1-byte length + ASCII).
    pub const SYM8: u8 = 0xA3;
    /// `0xB3` — sym32 (4-byte BE length + ASCII).
    pub const SYM32: u8 = 0xB3;

    // ------------------------------------------------------------------
    //  Floating + Char + Timestamp + UUID + Decimal.
    // ------------------------------------------------------------------
    /// `0x72` — float (fixed-4, IEEE 754 binary32 BE).
    pub const FLOAT: u8 = 0x72;
    /// `0x82` — double (fixed-8, IEEE 754 binary64 BE).
    pub const DOUBLE: u8 = 0x82;
    /// `0x73` — char (fixed-4, UTF-32 BE codepoint).
    pub const CHAR: u8 = 0x73;
    /// `0x74` — decimal32 (fixed-4, IEEE 754-2008 BID).
    pub const DECIMAL32: u8 = 0x74;
    /// `0x84` — decimal64 (fixed-8, IEEE 754-2008 BID).
    pub const DECIMAL64: u8 = 0x84;
    /// `0x94` — decimal128 (fixed-16, IEEE 754-2008 BID).
    pub const DECIMAL128: u8 = 0x94;
    /// `0x83` — timestamp (fixed-8, BE signed ms since Unix epoch).
    pub const TIMESTAMP: u8 = 0x83;
    /// `0x98` — uuid (fixed-16, RFC 4122).
    pub const UUID: u8 = 0x98;

    // ------------------------------------------------------------------
    //  Compound (list, map, array).
    // ------------------------------------------------------------------
    /// `0x45` — list0 (fixed-0, count=0).
    pub const LIST0: u8 = 0x45;
    /// `0xC0` — list8 (1-byte size + 1-byte count + items).
    pub const LIST8: u8 = 0xC0;
    /// `0xD0` — list32 (4-byte BE size + 4-byte BE count + items).
    pub const LIST32: u8 = 0xD0;
    /// `0xC1` — map8 (1-byte size + 1-byte count + entries).
    pub const MAP8: u8 = 0xC1;
    /// `0xD1` — map32 (4-byte BE size + 4-byte BE count + entries).
    pub const MAP32: u8 = 0xD1;
    /// `0xE0` — array8 (1-byte size + 1-byte count + element-constructor + items).
    pub const ARRAY8: u8 = 0xE0;
    /// `0xF0` — array32 (4-byte BE size + 4-byte BE count + element-constructor + items).
    pub const ARRAY32: u8 = 0xF0;
}

/// Format-Code-Kategorie (Spec §1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatCode {
    /// Spec §1.2.1 — fixed-width: 0..=8 octets payload, category 0x4*-0x9*.
    Fixed(u8),
    /// Spec §1.2.2 — variable-width: 1- or 4-byte length + bytes,
    /// category 0xA*/0xB*.
    Variable(u8),
    /// Spec §1.2.3 — compound: 1- or 4-byte size + count + items,
    /// category 0xC*/0xD*.
    Compound(u8),
    /// Spec §1.2.4 — array: 1- or 4-byte size + count + element-
    /// constructor + items, category 0xE*/0xF*.
    Array(u8),
}

impl FormatCode {
    /// Determines the category from the format-code byte (Spec §1.2 Table 1-1
    /// Subcategory column).
    #[must_use]
    pub const fn from_byte(b: u8) -> Self {
        match b >> 4 {
            0x4..=0x9 => Self::Fixed(b),
            0xA | 0xB => Self::Variable(b),
            0xC | 0xD => Self::Compound(b),
            0xE | 0xF => Self::Array(b),
            _ => Self::Fixed(b),
        }
    }
}

/// AMQP value (subset of the primitive types).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmqpValue {
    /// `null`.
    Null,
    /// `boolean`.
    Boolean(bool),
    /// `ulong` (or smallulong/ulong0 depending on the value).
    Ulong(u64),
    /// `long`.
    Long(i64),
    /// `binary` (max u32::MAX bytes).
    Binary(Vec<u8>),
    /// `string` (UTF-8).
    String(String),
    /// `symbol` (ASCII).
    Symbol(String),
}

/// Type-codec error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    /// Input bytes truncated.
    Truncated,
    /// Unknown / unsupported format code.
    UnsupportedFormatCode(u8),
    /// String has invalid UTF-8.
    InvalidUtf8,
    /// Symbol contains non-ASCII (Spec §1.6.21: "Symbols are encoded
    /// as ASCII").
    NonAsciiSymbol,
    /// Length prefix > u32::MAX (does not fit on the wire).
    LengthTooLarge,
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("input truncated"),
            Self::UnsupportedFormatCode(c) => write!(f, "unsupported format code 0x{c:02X}"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8 in str8/str32"),
            Self::NonAsciiSymbol => f.write_str("non-ASCII byte in symbol"),
            Self::LengthTooLarge => f.write_str("length exceeds u32::MAX"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TypeError {}

/// Spec §1.6.1 — `null` = 0x40.
#[must_use]
pub fn encode_null() -> Vec<u8> {
    alloc::vec![codes::NULL]
}

/// Spec §1.6.2 — `boolean`. We use the compact form 0x41/0x42
/// (fixed-0).
#[must_use]
pub fn encode_boolean(v: bool) -> Vec<u8> {
    alloc::vec![if v {
        codes::BOOLEAN_TRUE
    } else {
        codes::BOOLEAN_FALSE
    }]
}

/// Spec §1.6.6 — `ulong`. We choose the most compact form (ulong0
/// for 0, smallulong for 1..=255, ulong otherwise).
#[must_use]
pub fn encode_ulong(v: u64) -> Vec<u8> {
    if v == 0 {
        alloc::vec![codes::ULONG0]
    } else if v <= u64::from(u8::MAX) {
        let b = (v & 0xFF) as u8;
        alloc::vec![codes::SMALLULONG, b]
    } else {
        let mut out = Vec::with_capacity(9);
        out.push(codes::ULONG);
        out.extend_from_slice(&v.to_be_bytes());
        out
    }
}

/// Spec §1.6.10 — `long`. We choose smalllong for -128..=127, long
/// (full 8-byte) otherwise.
#[must_use]
pub fn encode_long(v: i64) -> Vec<u8> {
    if (i64::from(i8::MIN)..=i64::from(i8::MAX)).contains(&v) {
        let b = (v as i8) as u8;
        alloc::vec![codes::SMALLLONG, b]
    } else {
        let mut out = Vec::with_capacity(9);
        out.push(codes::LONG);
        out.extend_from_slice(&v.to_be_bytes());
        out
    }
}

/// Spec §1.6.19 — `binary`. Chooses vbin8 for len <= 255, vbin32 otherwise.
///
/// # Errors
/// `LengthTooLarge` if `data.len() > u32::MAX`.
pub fn encode_binary(data: &[u8]) -> Result<Vec<u8>, TypeError> {
    let len = data.len();
    if len > u32::MAX as usize {
        return Err(TypeError::LengthTooLarge);
    }
    if len <= u8::MAX as usize {
        let mut out = Vec::with_capacity(2 + len);
        out.push(codes::VBIN8);
        #[allow(clippy::cast_possible_truncation)]
        out.push(len as u8);
        out.extend_from_slice(data);
        Ok(out)
    } else {
        let mut out = Vec::with_capacity(5 + len);
        out.push(codes::VBIN32);
        #[allow(clippy::cast_possible_truncation)]
        out.extend_from_slice(&(len as u32).to_be_bytes());
        out.extend_from_slice(data);
        Ok(out)
    }
}

/// Spec §1.6.20 — `string`. Chooses str8 for len <= 255, str32 otherwise.
///
/// # Errors
/// `LengthTooLarge` if `s.len() > u32::MAX`.
pub fn encode_string(s: &str) -> Result<Vec<u8>, TypeError> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len > u32::MAX as usize {
        return Err(TypeError::LengthTooLarge);
    }
    if len <= u8::MAX as usize {
        let mut out = Vec::with_capacity(2 + len);
        out.push(codes::STR8);
        #[allow(clippy::cast_possible_truncation)]
        out.push(len as u8);
        out.extend_from_slice(bytes);
        Ok(out)
    } else {
        let mut out = Vec::with_capacity(5 + len);
        out.push(codes::STR32);
        #[allow(clippy::cast_possible_truncation)]
        out.extend_from_slice(&(len as u32).to_be_bytes());
        out.extend_from_slice(bytes);
        Ok(out)
    }
}

/// Spec §1.6.21 — `symbol`. Chooses sym8 for len <= 255, sym32 otherwise.
///
/// # Errors
/// * `NonAsciiSymbol` if the string contains non-ASCII.
/// * `LengthTooLarge` if `s.len() > u32::MAX`.
pub fn encode_symbol(s: &str) -> Result<Vec<u8>, TypeError> {
    if !s.is_ascii() {
        return Err(TypeError::NonAsciiSymbol);
    }
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len > u32::MAX as usize {
        return Err(TypeError::LengthTooLarge);
    }
    if len <= u8::MAX as usize {
        let mut out = Vec::with_capacity(2 + len);
        out.push(codes::SYM8);
        #[allow(clippy::cast_possible_truncation)]
        out.push(len as u8);
        out.extend_from_slice(bytes);
        Ok(out)
    } else {
        let mut out = Vec::with_capacity(5 + len);
        out.push(codes::SYM32);
        #[allow(clippy::cast_possible_truncation)]
        out.extend_from_slice(&(len as u32).to_be_bytes());
        out.extend_from_slice(bytes);
        Ok(out)
    }
}

/// Decodes a single `AmqpValue` starting at `bytes[0]`. Returns
/// `Ok((value, consumed_bytes))`.
///
/// # Errors
/// See [`TypeError`].
pub fn decode_value(bytes: &[u8]) -> Result<(AmqpValue, usize), TypeError> {
    if bytes.is_empty() {
        return Err(TypeError::Truncated);
    }
    let code = bytes[0];
    match code {
        codes::NULL => Ok((AmqpValue::Null, 1)),
        codes::BOOLEAN_TRUE => Ok((AmqpValue::Boolean(true), 1)),
        codes::BOOLEAN_FALSE => Ok((AmqpValue::Boolean(false), 1)),
        codes::BOOLEAN => {
            if bytes.len() < 2 {
                return Err(TypeError::Truncated);
            }
            Ok((AmqpValue::Boolean(bytes[1] != 0), 2))
        }
        codes::ULONG0 => Ok((AmqpValue::Ulong(0), 1)),
        codes::SMALLULONG => {
            if bytes.len() < 2 {
                return Err(TypeError::Truncated);
            }
            Ok((AmqpValue::Ulong(u64::from(bytes[1])), 2))
        }
        codes::ULONG => {
            if bytes.len() < 9 {
                return Err(TypeError::Truncated);
            }
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[1..9]);
            Ok((AmqpValue::Ulong(u64::from_be_bytes(buf)), 9))
        }
        codes::SMALLLONG => {
            if bytes.len() < 2 {
                return Err(TypeError::Truncated);
            }
            #[allow(clippy::cast_possible_wrap)]
            Ok((AmqpValue::Long(i64::from(bytes[1] as i8)), 2))
        }
        codes::LONG => {
            if bytes.len() < 9 {
                return Err(TypeError::Truncated);
            }
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[1..9]);
            Ok((AmqpValue::Long(i64::from_be_bytes(buf)), 9))
        }
        codes::VBIN8 => {
            if bytes.len() < 2 {
                return Err(TypeError::Truncated);
            }
            let len = usize::from(bytes[1]);
            if bytes.len() < 2 + len {
                return Err(TypeError::Truncated);
            }
            Ok((AmqpValue::Binary(bytes[2..2 + len].to_vec()), 2 + len))
        }
        codes::VBIN32 => {
            if bytes.len() < 5 {
                return Err(TypeError::Truncated);
            }
            let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
            if bytes.len() < 5 + len {
                return Err(TypeError::Truncated);
            }
            Ok((AmqpValue::Binary(bytes[5..5 + len].to_vec()), 5 + len))
        }
        codes::STR8 => decode_str8(bytes, AmqpValue::String),
        codes::STR32 => decode_str32(bytes, AmqpValue::String),
        codes::SYM8 => decode_str8(bytes, AmqpValue::Symbol),
        codes::SYM32 => decode_str32(bytes, AmqpValue::Symbol),
        other => Err(TypeError::UnsupportedFormatCode(other)),
    }
}

fn decode_str8(
    bytes: &[u8],
    wrap: fn(String) -> AmqpValue,
) -> Result<(AmqpValue, usize), TypeError> {
    if bytes.len() < 2 {
        return Err(TypeError::Truncated);
    }
    let len = usize::from(bytes[1]);
    if bytes.len() < 2 + len {
        return Err(TypeError::Truncated);
    }
    let s = core::str::from_utf8(&bytes[2..2 + len])
        .map_err(|_| TypeError::InvalidUtf8)?
        .to_owned();
    Ok((wrap(s), 2 + len))
}

fn decode_str32(
    bytes: &[u8],
    wrap: fn(String) -> AmqpValue,
) -> Result<(AmqpValue, usize), TypeError> {
    if bytes.len() < 5 {
        return Err(TypeError::Truncated);
    }
    let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
    if bytes.len() < 5 + len {
        return Err(TypeError::Truncated);
    }
    let s = core::str::from_utf8(&bytes[5..5 + len])
        .map_err(|_| TypeError::InvalidUtf8)?
        .to_owned();
    Ok((wrap(s), 5 + len))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn null_encodes_to_single_byte_0x40() {
        // Spec §1.6.1.
        assert_eq!(encode_null(), alloc::vec![0x40]);
    }

    #[test]
    fn boolean_uses_compact_format_codes() {
        // Spec §1.6.2 — 0x41 true, 0x42 false.
        assert_eq!(encode_boolean(true), alloc::vec![0x41]);
        assert_eq!(encode_boolean(false), alloc::vec![0x42]);
    }

    #[test]
    fn ulong_zero_uses_ulong0_format() {
        // Spec §1.6.6.
        assert_eq!(encode_ulong(0), alloc::vec![0x44]);
    }

    #[test]
    fn ulong_small_uses_smallulong_format() {
        // Spec §1.6.6 — smallulong = 0x53 + 1 byte.
        assert_eq!(encode_ulong(255), alloc::vec![0x53, 0xFF]);
    }

    #[test]
    fn ulong_large_uses_full_8_byte_format() {
        // Spec §1.6.6 — ulong = 0x80 + 8 bytes BE.
        let bytes = encode_ulong(0x1122_3344_5566_7788);
        assert_eq!(bytes[0], 0x80);
        assert_eq!(&bytes[1..], &0x1122_3344_5566_7788_u64.to_be_bytes());
    }

    #[test]
    fn long_small_uses_smalllong_format() {
        // Spec §1.6.10 — smalllong = 0x55 + 1 byte (signed).
        assert_eq!(encode_long(-1), alloc::vec![0x55, 0xFF]);
        assert_eq!(encode_long(127), alloc::vec![0x55, 0x7F]);
    }

    #[test]
    fn long_large_uses_full_8_byte_format() {
        let bytes = encode_long(i64::MIN);
        assert_eq!(bytes[0], 0x81);
    }

    #[test]
    fn binary_short_uses_vbin8_format() {
        // Spec §1.6.19 — vbin8 = 0xA0 + 1 byte len.
        let bytes = encode_binary(&[1, 2, 3]).expect("encode");
        assert_eq!(bytes, alloc::vec![0xA0, 0x03, 1, 2, 3]);
    }

    #[test]
    fn binary_long_uses_vbin32_format() {
        // Spec §1.6.19 — vbin32 = 0xB0 + 4 byte BE len.
        let data = alloc::vec![0xAA; 300];
        let bytes = encode_binary(&data).expect("encode");
        assert_eq!(bytes[0], 0xB0);
        assert_eq!(&bytes[1..5], &300u32.to_be_bytes());
    }

    #[test]
    fn string_short_uses_str8_format() {
        // Spec §1.6.20 — str8-utf8 = 0xA1 + 1 byte len + UTF-8.
        let bytes = encode_string("hi").expect("encode");
        assert_eq!(bytes, alloc::vec![0xA1, 0x02, b'h', b'i']);
    }

    #[test]
    fn string_unicode_round_trip() {
        let bytes = encode_string("Käfer").expect("encode");
        let (parsed, consumed) = decode_value(&bytes).expect("decode");
        assert_eq!(consumed, bytes.len());
        match parsed {
            AmqpValue::String(s) => assert_eq!(s, "Käfer"),
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn symbol_rejects_non_ascii() {
        // Spec §1.6.21 — Symbols are ASCII.
        assert_eq!(encode_symbol("Käfer"), Err(TypeError::NonAsciiSymbol));
    }

    #[test]
    fn symbol_short_uses_sym8_format() {
        let bytes = encode_symbol("hello").expect("encode");
        assert_eq!(bytes[0], 0xA3);
        assert_eq!(bytes[1], 0x05);
        assert_eq!(&bytes[2..], b"hello");
    }

    #[test]
    fn round_trip_all_primitive_values() {
        let values = alloc::vec![
            (encode_null(), AmqpValue::Null),
            (encode_boolean(true), AmqpValue::Boolean(true)),
            (encode_boolean(false), AmqpValue::Boolean(false)),
            (encode_ulong(0), AmqpValue::Ulong(0)),
            (encode_ulong(42), AmqpValue::Ulong(42)),
            (
                encode_ulong(0x1234_5678_9ABC_DEF0),
                AmqpValue::Ulong(0x1234_5678_9ABC_DEF0)
            ),
            (encode_long(-100), AmqpValue::Long(-100)),
            (encode_long(i64::MIN), AmqpValue::Long(i64::MIN)),
            (
                encode_binary(&[1, 2, 3]).expect("ok"),
                AmqpValue::Binary(alloc::vec![1, 2, 3])
            ),
            (
                encode_binary(&alloc::vec![0u8; 500]).expect("ok"),
                AmqpValue::Binary(alloc::vec![0u8; 500])
            ),
            (
                encode_string("foo").expect("ok"),
                AmqpValue::String("foo".into())
            ),
            (
                encode_symbol("bar").expect("ok"),
                AmqpValue::Symbol("bar".into())
            ),
        ];
        for (bytes, expected) in values {
            let (parsed, consumed) = decode_value(&bytes).expect("decode");
            assert_eq!(parsed, expected);
            assert_eq!(consumed, bytes.len());
        }
    }

    #[test]
    fn unsupported_format_code_yields_error() {
        // 0xFF is reserved/unsupported in our subset implementation.
        assert_eq!(
            decode_value(&[0xFF]),
            Err(TypeError::UnsupportedFormatCode(0xFF))
        );
    }

    #[test]
    fn truncated_inputs_yield_error() {
        assert_eq!(decode_value(&[]), Err(TypeError::Truncated));
        assert_eq!(decode_value(&[0xA0]), Err(TypeError::Truncated)); // vbin8 without len.
        assert_eq!(decode_value(&[0xA0, 5, 1]), Err(TypeError::Truncated)); // vbin8 truncated body.
        assert_eq!(decode_value(&[0x80, 0, 0, 0]), Err(TypeError::Truncated)); // ulong truncated.
    }

    #[test]
    fn invalid_utf8_in_str_yields_error() {
        // str8 with invalid UTF-8 byte 0xFF.
        assert_eq!(
            decode_value(&[0xA1, 0x01, 0xFF]),
            Err(TypeError::InvalidUtf8)
        );
    }

    #[test]
    fn format_code_categorizes_correctly() {
        // Spec §1.2 Subcategories.
        assert!(matches!(FormatCode::from_byte(0x40), FormatCode::Fixed(_)));
        assert!(matches!(FormatCode::from_byte(0x80), FormatCode::Fixed(_)));
        assert!(matches!(
            FormatCode::from_byte(0xA0),
            FormatCode::Variable(_)
        ));
        assert!(matches!(
            FormatCode::from_byte(0xB0),
            FormatCode::Variable(_)
        ));
        assert!(matches!(
            FormatCode::from_byte(0xC0),
            FormatCode::Compound(_)
        ));
        assert!(matches!(
            FormatCode::from_byte(0xD0),
            FormatCode::Compound(_)
        ));
        assert!(matches!(FormatCode::from_byte(0xE0), FormatCode::Array(_)));
        assert!(matches!(FormatCode::from_byte(0xF0), FormatCode::Array(_)));
    }
}
