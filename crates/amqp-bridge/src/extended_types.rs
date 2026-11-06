// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 1.0 Extended Type Codecs — Spec §1.6 Tail + §1.6.22-§1.6.24.
//!
//! Extends the primitive set from `types` with the missing integer
//! types (`ubyte`/`ushort`/`uint`/`byte`/`short`/`int`), floating-
//! point (`float`/`double`), `char`, `decimal32`/`64`/`128`,
//! `timestamp` and `uuid`, plus the compound types `list`, `map` and
//! `array` with a DoS cap on the recursion depth.
//!
//! Cross-Ref: DDS-AMQP-1.0 §7.1 (Type-System-Mapping) + §7.2
//! (Composite-Type-Mapping).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::types::{AmqpValue, TypeError, codes};

// DoS cap for compound recursion (analogous to Spec §3.3.1.4,
// DDS-AMQP-1.0 §6.1 implementation note).
const MAX_COMPOUND_DEPTH: usize = 32;

// ============================================================================
//  Integer-Tail (ubyte / ushort / uint / byte / short / int).
// ============================================================================

/// Spec §1.6.3 ubyte = 0x50 + 1 byte.
#[must_use]
pub fn encode_ubyte(v: u8) -> Vec<u8> {
    alloc::vec![codes::UBYTE, v]
}

/// Decode ubyte.
///
/// # Errors
/// `Truncated`.
pub fn decode_ubyte(bytes: &[u8]) -> Result<(u8, usize), TypeError> {
    if bytes.len() < 2 || bytes[0] != codes::UBYTE {
        return Err(TypeError::Truncated);
    }
    Ok((bytes[1], 2))
}

/// Spec §1.6.4 ushort = 0x60 + 2-byte BE.
#[must_use]
pub fn encode_ushort(v: u16) -> Vec<u8> {
    let mut out = alloc::vec![codes::USHORT];
    out.extend_from_slice(&v.to_be_bytes());
    out
}

/// Decode ushort.
///
/// # Errors
/// `Truncated`.
pub fn decode_ushort(bytes: &[u8]) -> Result<(u16, usize), TypeError> {
    if bytes.len() < 3 || bytes[0] != codes::USHORT {
        return Err(TypeError::Truncated);
    }
    Ok((u16::from_be_bytes([bytes[1], bytes[2]]), 3))
}

/// Spec §1.6.5 uint with compact selection (uint0 / smalluint / uint).
#[must_use]
pub fn encode_uint(v: u32) -> Vec<u8> {
    if v == 0 {
        alloc::vec![codes::UINT0]
    } else if v <= u32::from(u8::MAX) {
        let b = (v & 0xFF) as u8;
        alloc::vec![codes::SMALLUINT, b]
    } else {
        let mut out = alloc::vec![codes::UINT];
        out.extend_from_slice(&v.to_be_bytes());
        out
    }
}

/// Decode uint (all three forms).
///
/// # Errors
/// `Truncated` / `UnsupportedFormatCode`.
pub fn decode_uint(bytes: &[u8]) -> Result<(u32, usize), TypeError> {
    if bytes.is_empty() {
        return Err(TypeError::Truncated);
    }
    match bytes[0] {
        codes::UINT0 => Ok((0, 1)),
        codes::SMALLUINT => {
            if bytes.len() < 2 {
                return Err(TypeError::Truncated);
            }
            Ok((u32::from(bytes[1]), 2))
        }
        codes::UINT => {
            if bytes.len() < 5 {
                return Err(TypeError::Truncated);
            }
            Ok((
                u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]),
                5,
            ))
        }
        c => Err(TypeError::UnsupportedFormatCode(c)),
    }
}

/// Spec §1.6.7 byte (signed) = 0x51 + 1 byte.
#[must_use]
pub fn encode_byte(v: i8) -> Vec<u8> {
    #[allow(clippy::cast_sign_loss)]
    let b = v as u8;
    alloc::vec![codes::BYTE, b]
}

/// Decode byte.
///
/// # Errors
/// `Truncated`.
pub fn decode_byte(bytes: &[u8]) -> Result<(i8, usize), TypeError> {
    if bytes.len() < 2 || bytes[0] != codes::BYTE {
        return Err(TypeError::Truncated);
    }
    #[allow(clippy::cast_possible_wrap)]
    Ok((bytes[1] as i8, 2))
}

/// Spec §1.6.8 short = 0x61 + 2-byte BE signed.
#[must_use]
pub fn encode_short(v: i16) -> Vec<u8> {
    let mut out = alloc::vec![codes::SHORT];
    out.extend_from_slice(&v.to_be_bytes());
    out
}

/// Decode short.
///
/// # Errors
/// `Truncated`.
pub fn decode_short(bytes: &[u8]) -> Result<(i16, usize), TypeError> {
    if bytes.len() < 3 || bytes[0] != codes::SHORT {
        return Err(TypeError::Truncated);
    }
    Ok((i16::from_be_bytes([bytes[1], bytes[2]]), 3))
}

/// Spec §1.6.9 int with compact selection (smallint / int).
#[must_use]
pub fn encode_int(v: i32) -> Vec<u8> {
    if (-128..=127).contains(&v) {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let b = v as i8 as u8;
        alloc::vec![codes::SMALLINT, b]
    } else {
        let mut out = alloc::vec![codes::INT];
        out.extend_from_slice(&v.to_be_bytes());
        out
    }
}

/// Decode int.
///
/// # Errors
/// `Truncated` / `UnsupportedFormatCode`.
pub fn decode_int(bytes: &[u8]) -> Result<(i32, usize), TypeError> {
    if bytes.is_empty() {
        return Err(TypeError::Truncated);
    }
    match bytes[0] {
        codes::SMALLINT => {
            if bytes.len() < 2 {
                return Err(TypeError::Truncated);
            }
            #[allow(clippy::cast_possible_wrap)]
            Ok((i32::from(bytes[1] as i8), 2))
        }
        codes::INT => {
            if bytes.len() < 5 {
                return Err(TypeError::Truncated);
            }
            Ok((
                i32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]),
                5,
            ))
        }
        c => Err(TypeError::UnsupportedFormatCode(c)),
    }
}

// ============================================================================
//  Floating + Char + Timestamp + UUID + Decimal.
// ============================================================================

/// Spec §1.6.11 float (IEEE 754 binary32 BE).
#[must_use]
pub fn encode_float(v: f32) -> Vec<u8> {
    let mut out = alloc::vec![codes::FLOAT];
    out.extend_from_slice(&v.to_be_bytes());
    out
}

/// Decode float.
///
/// # Errors
/// `Truncated`.
pub fn decode_float(bytes: &[u8]) -> Result<(f32, usize), TypeError> {
    if bytes.len() < 5 || bytes[0] != codes::FLOAT {
        return Err(TypeError::Truncated);
    }
    Ok((
        f32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]),
        5,
    ))
}

/// Spec §1.6.12 double (IEEE 754 binary64 BE).
#[must_use]
pub fn encode_double(v: f64) -> Vec<u8> {
    let mut out = alloc::vec![codes::DOUBLE];
    out.extend_from_slice(&v.to_be_bytes());
    out
}

/// Decode double.
///
/// # Errors
/// `Truncated`.
pub fn decode_double(bytes: &[u8]) -> Result<(f64, usize), TypeError> {
    if bytes.len() < 9 || bytes[0] != codes::DOUBLE {
        return Err(TypeError::Truncated);
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[1..9]);
    Ok((f64::from_be_bytes(buf), 9))
}

/// Spec §1.6.16 char = `0x73` + UTF-32-BE codepoint.
///
/// # Errors
/// `LengthTooLarge` if the char is not a valid Unicode scalar.
pub fn encode_char(c: char) -> Result<Vec<u8>, TypeError> {
    let codepoint = u32::from(c);
    let mut out = alloc::vec![codes::CHAR];
    out.extend_from_slice(&codepoint.to_be_bytes());
    Ok(out)
}

/// Decode char.
///
/// # Errors
/// `Truncated` / `LengthTooLarge` if codepoint isn't a valid char.
pub fn decode_char(bytes: &[u8]) -> Result<(char, usize), TypeError> {
    if bytes.len() < 5 || bytes[0] != codes::CHAR {
        return Err(TypeError::Truncated);
    }
    let cp = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
    char::from_u32(cp)
        .map(|c| (c, 5))
        .ok_or(TypeError::LengthTooLarge)
}

/// Spec §1.6.17 timestamp = `0x83` + 8-byte BE signed ms since epoch.
#[must_use]
pub fn encode_timestamp(ms_since_epoch: i64) -> Vec<u8> {
    let mut out = alloc::vec![codes::TIMESTAMP];
    out.extend_from_slice(&ms_since_epoch.to_be_bytes());
    out
}

/// Decode timestamp.
///
/// # Errors
/// `Truncated`.
pub fn decode_timestamp(bytes: &[u8]) -> Result<(i64, usize), TypeError> {
    if bytes.len() < 9 || bytes[0] != codes::TIMESTAMP {
        return Err(TypeError::Truncated);
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[1..9]);
    Ok((i64::from_be_bytes(buf), 9))
}

/// Spec §1.6.18 uuid = `0x98` + 16 bytes RFC 4122.
#[must_use]
pub fn encode_uuid(uuid: [u8; 16]) -> Vec<u8> {
    let mut out = alloc::vec![codes::UUID];
    out.extend_from_slice(&uuid);
    out
}

/// Decode uuid.
///
/// # Errors
/// `Truncated`.
pub fn decode_uuid(bytes: &[u8]) -> Result<([u8; 16], usize), TypeError> {
    if bytes.len() < 17 || bytes[0] != codes::UUID {
        return Err(TypeError::Truncated);
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&bytes[1..17]);
    Ok((out, 17))
}

/// Spec §1.6.13 decimal32 = `0x74` + 4 bytes BID.
#[must_use]
pub fn encode_decimal32(bid: [u8; 4]) -> Vec<u8> {
    let mut out = alloc::vec![codes::DECIMAL32];
    out.extend_from_slice(&bid);
    out
}

/// Spec §1.6.14 decimal64 = `0x84` + 8 bytes BID.
#[must_use]
pub fn encode_decimal64(bid: [u8; 8]) -> Vec<u8> {
    let mut out = alloc::vec![codes::DECIMAL64];
    out.extend_from_slice(&bid);
    out
}

/// Spec §1.6.15 decimal128 = `0x94` + 16 bytes BID.
#[must_use]
pub fn encode_decimal128(bid: [u8; 16]) -> Vec<u8> {
    let mut out = alloc::vec![codes::DECIMAL128];
    out.extend_from_slice(&bid);
    out
}

// ============================================================================
//  Compound: List, Map, Array.
// ============================================================================

/// Extended AMQP value type. We avoid changes to
/// `crate::types::AmqpValue` because the type module is stable;
/// instead, a dedicated enum with all primitive and compound
/// variants and a From conversion to/from `AmqpValue` for the
/// subset.
#[derive(Debug, Clone, PartialEq)]
pub enum AmqpExtValue {
    /// Spec §1.6.1
    Null,
    /// Spec §1.6.2
    Boolean(bool),
    /// Spec §1.6.3
    Ubyte(u8),
    /// Spec §1.6.4
    Ushort(u16),
    /// Spec §1.6.5
    Uint(u32),
    /// Spec §1.6.6
    Ulong(u64),
    /// Spec §1.6.7
    Byte(i8),
    /// Spec §1.6.8
    Short(i16),
    /// Spec §1.6.9
    Int(i32),
    /// Spec §1.6.10
    Long(i64),
    /// Spec §1.6.11
    Float(f32),
    /// Spec §1.6.12
    Double(f64),
    /// Spec §1.6.16
    Char(char),
    /// Spec §1.6.17
    Timestamp(i64),
    /// Spec §1.6.18
    Uuid([u8; 16]),
    /// Spec §1.6.19
    Binary(Vec<u8>),
    /// Spec §1.6.20
    Str(String),
    /// Spec §1.6.21
    Symbol(String),
    /// Spec §1.6.22
    List(Vec<AmqpExtValue>),
    /// Spec §1.6.23
    Map(Vec<(AmqpExtValue, AmqpExtValue)>),
    /// Spec §1.6.24
    Array(Vec<AmqpExtValue>),
    /// Spec §1.3.4 — a **described type**: `0x00` + descriptor (ulong) + value.
    /// Used for composite types like `source`/`target`/message sections that
    /// appear nested inside performative lists.
    Described {
        /// The descriptor code (e.g. `0x29` for `target`, `0x28` for `source`).
        descriptor: u64,
        /// The described value (typically a `List`).
        value: alloc::boxed::Box<AmqpExtValue>,
    },
}

impl Eq for AmqpExtValue {}

impl AmqpExtValue {
    /// Encodes the value to wire format.
    ///
    /// # Errors
    /// see [`TypeError`].
    pub fn encode(&self) -> Result<Vec<u8>, TypeError> {
        self.encode_at(0)
    }

    fn encode_at(&self, depth: usize) -> Result<Vec<u8>, TypeError> {
        if depth > MAX_COMPOUND_DEPTH {
            return Err(TypeError::LengthTooLarge);
        }
        match self {
            Self::Null => Ok(alloc::vec![codes::NULL]),
            Self::Boolean(b) => Ok(if *b {
                alloc::vec![codes::BOOLEAN_TRUE]
            } else {
                alloc::vec![codes::BOOLEAN_FALSE]
            }),
            Self::Ubyte(v) => Ok(encode_ubyte(*v)),
            Self::Ushort(v) => Ok(encode_ushort(*v)),
            Self::Uint(v) => Ok(encode_uint(*v)),
            Self::Ulong(v) => Ok(crate::types::encode_ulong(*v)),
            Self::Byte(v) => Ok(encode_byte(*v)),
            Self::Short(v) => Ok(encode_short(*v)),
            Self::Int(v) => Ok(encode_int(*v)),
            Self::Long(v) => Ok(crate::types::encode_long(*v)),
            Self::Float(v) => Ok(encode_float(*v)),
            Self::Double(v) => Ok(encode_double(*v)),
            Self::Char(c) => encode_char(*c),
            Self::Timestamp(t) => Ok(encode_timestamp(*t)),
            Self::Uuid(u) => Ok(encode_uuid(*u)),
            Self::Binary(b) => crate::types::encode_binary(b),
            Self::Str(s) => crate::types::encode_string(s),
            Self::Symbol(s) => crate::types::encode_symbol(s),
            Self::List(items) => encode_list(items, depth),
            Self::Map(entries) => encode_map(entries, depth),
            Self::Array(items) => encode_array(items, depth),
            Self::Described { descriptor, value } => {
                // 0x00 + ulong(descriptor) + encoded value.
                let mut out = alloc::vec![0x00u8];
                out.extend_from_slice(&crate::types::encode_ulong(*descriptor));
                out.extend_from_slice(&value.encode_at(depth + 1)?);
                Ok(out)
            }
        }
    }

    /// The encoding of the widest constructor of the value's type, the one
    /// an array element takes when the compact constructors of its
    /// elements differ (Spec §1.6.24): `uint` 0x70, `ulong` 0x80, `int`
    /// 0x71, `long` 0x81, `boolean` 0x56, `vbin32`, `str32-utf8`, `sym32`,
    /// `list32`, `map32`, `array32`. Every other type has one encoding.
    fn encode_wide(&self, depth: usize) -> Result<Vec<u8>, TypeError> {
        fn fixed(code: u8, octets: &[u8]) -> Vec<u8> {
            let mut out = alloc::vec![code];
            out.extend_from_slice(octets);
            out
        }
        fn variable(code: u8, octets: &[u8]) -> Result<Vec<u8>, TypeError> {
            let len = u32::try_from(octets.len()).map_err(|_| TypeError::LengthTooLarge)?;
            let mut out = fixed(code, &len.to_be_bytes());
            out.extend_from_slice(octets);
            Ok(out)
        }
        fn compound32(code: u8, encoded: &[u8]) -> Result<Vec<u8>, TypeError> {
            // `encoded` is the 8-bit form: code, size, count, items; or
            // list0 for the empty list.
            let (count, items) = match encoded {
                [_, _, count, items @ ..] => (u32::from(*count), items),
                _ => (0, &[][..]),
            };
            let size = u32::try_from(items.len() + 4).map_err(|_| TypeError::LengthTooLarge)?;
            let mut out = fixed(code, &size.to_be_bytes());
            out.extend_from_slice(&count.to_be_bytes());
            out.extend_from_slice(items);
            Ok(out)
        }
        match self {
            Self::Uint(v) => Ok(fixed(codes::UINT, &v.to_be_bytes())),
            Self::Ulong(v) => Ok(fixed(codes::ULONG, &v.to_be_bytes())),
            Self::Int(v) => Ok(fixed(codes::INT, &v.to_be_bytes())),
            Self::Long(v) => Ok(fixed(codes::LONG, &v.to_be_bytes())),
            Self::Boolean(b) => Ok(fixed(codes::BOOLEAN, &[u8::from(*b)])),
            Self::Binary(b) => variable(codes::VBIN32, b),
            Self::Str(s) => variable(codes::STR32, s.as_bytes()),
            Self::Symbol(s) => {
                if !s.is_ascii() {
                    return Err(TypeError::NonAsciiSymbol);
                }
                variable(codes::SYM32, s.as_bytes())
            }
            Self::List(_) | Self::Map(_) | Self::Array(_) => {
                let encoded = self.encode_at(depth)?;
                match encoded.first() {
                    Some(&codes::LIST0 | &codes::LIST8) => compound32(codes::LIST32, &encoded),
                    Some(&codes::MAP8) => compound32(codes::MAP32, &encoded),
                    Some(&codes::ARRAY8) => compound32(codes::ARRAY32, &encoded),
                    _ => Ok(encoded),
                }
            }
            _ => self.encode_at(depth),
        }
    }

    /// Decode a single value.
    ///
    /// # Errors
    /// see [`TypeError`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), TypeError> {
        Self::decode_at(bytes, 0)
    }

    fn decode_at(bytes: &[u8], depth: usize) -> Result<(Self, usize), TypeError> {
        if depth > MAX_COMPOUND_DEPTH {
            return Err(TypeError::LengthTooLarge);
        }
        if bytes.is_empty() {
            return Err(TypeError::Truncated);
        }
        let code = bytes[0];
        match code {
            // Described type (§1.3.4): 0x00 + descriptor(ulong) + value.
            0x00 => {
                let (desc, dn) = crate::types::decode_value(&bytes[1..])?;
                let descriptor = match desc {
                    AmqpValue::Ulong(u) => u,
                    _ => return Err(TypeError::UnsupportedFormatCode(code)),
                };
                let (value, vn) = Self::decode_at(&bytes[1 + dn..], depth + 1)?;
                Ok((
                    Self::Described {
                        descriptor,
                        value: alloc::boxed::Box::new(value),
                    },
                    1 + dn + vn,
                ))
            }
            codes::NULL => Ok((Self::Null, 1)),
            codes::BOOLEAN_TRUE => Ok((Self::Boolean(true), 1)),
            codes::BOOLEAN_FALSE => Ok((Self::Boolean(false), 1)),
            // Spec §1.6.2: `boolean` 0x56 with one octet, 0x00 false, 0x01
            // true; the encoding of a boolean array element.
            codes::BOOLEAN => match bytes.get(1) {
                Some(0x00) => Ok((Self::Boolean(false), 2)),
                Some(0x01) => Ok((Self::Boolean(true), 2)),
                Some(_) => Err(TypeError::UnsupportedFormatCode(codes::BOOLEAN)),
                None => Err(TypeError::Truncated),
            },
            codes::UBYTE => decode_ubyte(bytes).map(|(v, n)| (Self::Ubyte(v), n)),
            codes::USHORT => decode_ushort(bytes).map(|(v, n)| (Self::Ushort(v), n)),
            codes::UINT0 | codes::SMALLUINT | codes::UINT => {
                decode_uint(bytes).map(|(v, n)| (Self::Uint(v), n))
            }
            codes::ULONG0 | codes::SMALLULONG | codes::ULONG => {
                let (v, n) = crate::types::decode_value(bytes)?;
                if let AmqpValue::Ulong(u) = v {
                    Ok((Self::Ulong(u), n))
                } else {
                    Err(TypeError::UnsupportedFormatCode(code))
                }
            }
            codes::BYTE => decode_byte(bytes).map(|(v, n)| (Self::Byte(v), n)),
            codes::SHORT => decode_short(bytes).map(|(v, n)| (Self::Short(v), n)),
            codes::SMALLINT | codes::INT => decode_int(bytes).map(|(v, n)| (Self::Int(v), n)),
            codes::SMALLLONG | codes::LONG => {
                let (v, n) = crate::types::decode_value(bytes)?;
                if let AmqpValue::Long(l) = v {
                    Ok((Self::Long(l), n))
                } else {
                    Err(TypeError::UnsupportedFormatCode(code))
                }
            }
            codes::FLOAT => decode_float(bytes).map(|(v, n)| (Self::Float(v), n)),
            codes::DOUBLE => decode_double(bytes).map(|(v, n)| (Self::Double(v), n)),
            codes::CHAR => decode_char(bytes).map(|(v, n)| (Self::Char(v), n)),
            codes::TIMESTAMP => decode_timestamp(bytes).map(|(v, n)| (Self::Timestamp(v), n)),
            codes::UUID => decode_uuid(bytes).map(|(v, n)| (Self::Uuid(v), n)),
            codes::VBIN8 | codes::VBIN32 => {
                let (v, n) = crate::types::decode_value(bytes)?;
                if let AmqpValue::Binary(b) = v {
                    Ok((Self::Binary(b), n))
                } else {
                    Err(TypeError::UnsupportedFormatCode(code))
                }
            }
            codes::STR8 | codes::STR32 => {
                let (v, n) = crate::types::decode_value(bytes)?;
                if let AmqpValue::String(s) = v {
                    Ok((Self::Str(s), n))
                } else {
                    Err(TypeError::UnsupportedFormatCode(code))
                }
            }
            codes::SYM8 | codes::SYM32 => {
                let (v, n) = crate::types::decode_value(bytes)?;
                if let AmqpValue::Symbol(s) = v {
                    Ok((Self::Symbol(s), n))
                } else {
                    Err(TypeError::UnsupportedFormatCode(code))
                }
            }
            codes::LIST0 => Ok((Self::List(Vec::new()), 1)),
            codes::LIST8 | codes::LIST32 => decode_list(bytes, depth),
            codes::MAP8 | codes::MAP32 => decode_map(bytes, depth),
            codes::ARRAY8 | codes::ARRAY32 => decode_array(bytes, depth),
            other => Err(TypeError::UnsupportedFormatCode(other)),
        }
    }
}

impl fmt::Display for AmqpExtValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

// ----- list -----

fn encode_list(items: &[AmqpExtValue], depth: usize) -> Result<Vec<u8>, TypeError> {
    if items.is_empty() {
        return Ok(alloc::vec![codes::LIST0]);
    }
    let mut body = Vec::new();
    for it in items {
        body.extend_from_slice(&it.encode_at(depth + 1)?);
    }
    let count = items.len();
    let size = body.len() + 1; // +1 = count byte for list8 / 4 bytes for list32
    let use_short = body.len() <= 254 && count <= u8::MAX as usize;
    if use_short {
        let mut out = alloc::vec![codes::LIST8];
        out.push((size) as u8);
        out.push(count as u8);
        out.extend_from_slice(&body);
        Ok(out)
    } else {
        let mut out = alloc::vec![codes::LIST32];
        let total_size = body.len() + 4;
        out.extend_from_slice(
            &u32::try_from(total_size)
                .map_err(|_| TypeError::LengthTooLarge)?
                .to_be_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(count)
                .map_err(|_| TypeError::LengthTooLarge)?
                .to_be_bytes(),
        );
        out.extend_from_slice(&body);
        Ok(out)
    }
}

fn decode_list(bytes: &[u8], depth: usize) -> Result<(AmqpExtValue, usize), TypeError> {
    if bytes.is_empty() {
        return Err(TypeError::Truncated);
    }
    let (header, count, body_start, total) = match bytes[0] {
        codes::LIST8 => {
            if bytes.len() < 3 {
                return Err(TypeError::Truncated);
            }
            let size = usize::from(bytes[1]);
            let count = usize::from(bytes[2]);
            (3, count, 3, 2 + size)
        }
        codes::LIST32 => {
            if bytes.len() < 9 {
                return Err(TypeError::Truncated);
            }
            let size = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
            let count = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as usize;
            (9, count, 9, 5 + size)
        }
        c => return Err(TypeError::UnsupportedFormatCode(c)),
    };
    let _ = header;
    if bytes.len() < total {
        return Err(TypeError::Truncated);
    }
    // `count` is a wire-supplied element count (LIST8/LIST32), not yet
    // validated against the bytes actually available for the body. Reject
    // before allocating: every element needs >= 1 body byte, so a count
    // above the available body bytes cannot be genuine (mirrors
    // `crates/cdr/src/composite.rs`'s `len > reader.remaining()` guard).
    if count > total.saturating_sub(body_start) {
        return Err(TypeError::Truncated);
    }
    let mut items = Vec::with_capacity(count);
    let mut cur = body_start;
    for _ in 0..count {
        let (v, n) = AmqpExtValue::decode_at(&bytes[cur..total], depth + 1)?;
        items.push(v);
        cur += n;
    }
    Ok((AmqpExtValue::List(items), total))
}

// ----- map -----

fn encode_map(
    entries: &[(AmqpExtValue, AmqpExtValue)],
    depth: usize,
) -> Result<Vec<u8>, TypeError> {
    let mut body = Vec::new();
    for (k, v) in entries {
        body.extend_from_slice(&k.encode_at(depth + 1)?);
        body.extend_from_slice(&v.encode_at(depth + 1)?);
    }
    let count = entries.len() * 2;
    let use_short = body.len() <= 254 && count <= u8::MAX as usize;
    if use_short {
        let mut out = alloc::vec![codes::MAP8];
        out.push((body.len() + 1) as u8);
        out.push(count as u8);
        out.extend_from_slice(&body);
        Ok(out)
    } else {
        let mut out = alloc::vec![codes::MAP32];
        let total_size = body.len() + 4;
        out.extend_from_slice(
            &u32::try_from(total_size)
                .map_err(|_| TypeError::LengthTooLarge)?
                .to_be_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(count)
                .map_err(|_| TypeError::LengthTooLarge)?
                .to_be_bytes(),
        );
        out.extend_from_slice(&body);
        Ok(out)
    }
}

fn decode_map(bytes: &[u8], depth: usize) -> Result<(AmqpExtValue, usize), TypeError> {
    if bytes.is_empty() {
        return Err(TypeError::Truncated);
    }
    let (count, body_start, total) = match bytes[0] {
        codes::MAP8 => {
            if bytes.len() < 3 {
                return Err(TypeError::Truncated);
            }
            (usize::from(bytes[2]), 3, 2 + usize::from(bytes[1]))
        }
        codes::MAP32 => {
            if bytes.len() < 9 {
                return Err(TypeError::Truncated);
            }
            let size = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
            let count = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as usize;
            (count, 9, 5 + size)
        }
        c => return Err(TypeError::UnsupportedFormatCode(c)),
    };
    if bytes.len() < total || count % 2 != 0 {
        return Err(TypeError::Truncated);
    }
    // `count` is a wire-supplied element count (MAP8/MAP32, key+value
    // together), not yet validated against the bytes actually available
    // for the body. Reject before allocating: every element needs >= 1
    // body byte (mirrors `crates/cdr/src/composite.rs`'s
    // `len > reader.remaining()` guard).
    if count > total.saturating_sub(body_start) {
        return Err(TypeError::Truncated);
    }
    let mut entries = Vec::with_capacity(count / 2);
    let mut cur = body_start;
    for _ in 0..count / 2 {
        let (k, kn) = AmqpExtValue::decode_at(&bytes[cur..total], depth + 1)?;
        cur += kn;
        let (v, vn) = AmqpExtValue::decode_at(&bytes[cur..total], depth + 1)?;
        cur += vn;
        entries.push((k, v));
    }
    Ok((AmqpExtValue::Map(entries), total))
}

// ----- array -----

fn encode_array(items: &[AmqpExtValue], depth: usize) -> Result<Vec<u8>, TypeError> {
    if items.is_empty() {
        return Err(TypeError::LengthTooLarge); // array MUST have ≥1 element to know constructor
    }
    // Spec §1.6.24: one constructor for every element. The compact
    // encodings of one type differ with the value (uint0, smalluint and
    // uint for a uint; str8 and str32 for a string), so elements whose
    // compact constructors differ are written in the wide encoding of
    // their type; only elements of different types are refused.
    let mut encoded = items
        .iter()
        .map(|it| it.encode_at(depth + 1))
        .collect::<Result<Vec<_>, _>>()?;
    // A constructor without octets (true, false, uint0, ulong0, list0)
    // would leave the elements no octets of their own to tell them apart.
    let zero_width = matches!(encoded[0].first(), Some(0x41..=0x45));
    if zero_width || encoded.iter().any(|e| e.first() != encoded[0].first()) {
        encoded = items
            .iter()
            .map(|it| it.encode_wide(depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
    }
    let Some(&constructor_byte) = encoded[0].first() else {
        return Err(TypeError::LengthTooLarge);
    };
    let mut body = Vec::new();
    body.push(constructor_byte);
    for enc in &encoded {
        // Skip the constructor byte for subsequent elements (Spec §1.2.4).
        if enc.first() != Some(&constructor_byte) {
            return Err(TypeError::UnsupportedFormatCode(constructor_byte));
        }
        body.extend_from_slice(&enc[1..]);
    }
    let count = items.len();
    let use_short = body.len() <= 254 && count <= u8::MAX as usize;
    if use_short {
        let mut out = alloc::vec![codes::ARRAY8];
        out.push((body.len() + 1) as u8);
        out.push(count as u8);
        out.extend_from_slice(&body);
        Ok(out)
    } else {
        let mut out = alloc::vec![codes::ARRAY32];
        let total_size = body.len() + 4;
        out.extend_from_slice(
            &u32::try_from(total_size)
                .map_err(|_| TypeError::LengthTooLarge)?
                .to_be_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(count)
                .map_err(|_| TypeError::LengthTooLarge)?
                .to_be_bytes(),
        );
        out.extend_from_slice(&body);
        Ok(out)
    }
}

fn decode_array(bytes: &[u8], depth: usize) -> Result<(AmqpExtValue, usize), TypeError> {
    if bytes.is_empty() {
        return Err(TypeError::Truncated);
    }
    let (count, body_start, total) = match bytes[0] {
        codes::ARRAY8 => {
            if bytes.len() < 3 {
                return Err(TypeError::Truncated);
            }
            (usize::from(bytes[2]), 3, 2 + usize::from(bytes[1]))
        }
        codes::ARRAY32 => {
            if bytes.len() < 9 {
                return Err(TypeError::Truncated);
            }
            let size = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
            let count = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as usize;
            (count, 9, 5 + size)
        }
        c => return Err(TypeError::UnsupportedFormatCode(c)),
    };
    if bytes.len() < total || body_start >= total {
        return Err(TypeError::Truncated);
    }
    let constructor_byte = bytes[body_start];
    // `count` is a wire-supplied element count (ARRAY8/ARRAY32), not yet
    // validated against the bytes actually available for the body. Reject
    // before allocating: every element needs >= 1 body byte beyond the
    // shared constructor (mirrors `crates/cdr/src/composite.rs`'s
    // `len > reader.remaining()` guard).
    if count > total.saturating_sub(body_start + 1) {
        return Err(TypeError::Truncated);
    }
    // Reconstruct each element by prepending the constructor byte to its
    // payload before passing to decode_at.
    let mut items = Vec::with_capacity(count);
    let mut cur = body_start + 1;
    for _ in 0..count {
        let mut elem = alloc::vec![constructor_byte];
        elem.extend_from_slice(&bytes[cur..total]);
        let (v, n) = AmqpExtValue::decode_at(&elem, depth + 1)?;
        items.push(v);
        cur += n - 1; // -1 for the prepended constructor byte
    }
    Ok((AmqpExtValue::Array(items), total))
}

// ============================================================================
//  Tests.
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // Expected octets from OASIS AMQP 1.0 Part 1 (`amqp-1.0-types`): the
    // format code of each encoding (§1.6), the width of its octets, the
    // layout of the compound and array encodings (§1.2, Figure 1.4 to 1.6).

    /// `value` encodes to `octets`, and `octets` decode to `value`.
    fn wire(value: AmqpExtValue, octets: &[u8]) {
        assert_eq!(value.encode().expect("encode"), octets, "{value:?}");
        assert_eq!(
            AmqpExtValue::decode(octets).expect("decode"),
            (value, octets.len())
        );
    }

    #[test]
    fn amqp10_1_6_3_a_ubyte_is_0x50_and_one_octet() {
        wire(AmqpExtValue::Ubyte(0), &[0x50, 0x00]);
        wire(AmqpExtValue::Ubyte(255), &[0x50, 0xff]);
    }

    #[test]
    fn amqp10_1_6_4_a_ushort_is_0x60_and_two_octets() {
        wire(AmqpExtValue::Ushort(0), &[0x60, 0x00, 0x00]);
        wire(AmqpExtValue::Ushort(0xabcd), &[0x60, 0xab, 0xcd]);
    }

    /// §1.6.5: `uint0` 0x43, `smalluint` 0x52 and one octet, `uint` 0x70
    /// and four octets.
    #[test]
    fn amqp10_1_6_5_a_uint_is_uint0_smalluint_or_four_octets() {
        wire(AmqpExtValue::Uint(0), &[0x43]);
        wire(AmqpExtValue::Uint(42), &[0x52, 0x2a]);
        wire(
            AmqpExtValue::Uint(0x1234_5678),
            &[0x70, 0x12, 0x34, 0x56, 0x78],
        );
    }

    #[test]
    fn amqp10_1_6_7_a_byte_is_0x51_and_one_signed_octet() {
        wire(AmqpExtValue::Byte(-1), &[0x51, 0xff]);
        wire(AmqpExtValue::Byte(127), &[0x51, 0x7f]);
        wire(AmqpExtValue::Byte(-128), &[0x51, 0x80]);
    }

    #[test]
    fn amqp10_1_6_8_a_short_is_0x61_and_two_octets() {
        wire(AmqpExtValue::Short(-1), &[0x61, 0xff, 0xff]);
        wire(AmqpExtValue::Short(i16::MAX), &[0x61, 0x7f, 0xff]);
        wire(AmqpExtValue::Short(i16::MIN), &[0x61, 0x80, 0x00]);
    }

    /// §1.6.9: `smallint` 0x54 and one signed octet, `int` 0x71 and four.
    #[test]
    fn amqp10_1_6_9_an_int_is_smallint_or_four_octets() {
        wire(AmqpExtValue::Int(42), &[0x54, 0x2a]);
        wire(AmqpExtValue::Int(-1), &[0x54, 0xff]);
        wire(AmqpExtValue::Int(i32::MAX), &[0x71, 0x7f, 0xff, 0xff, 0xff]);
        wire(AmqpExtValue::Int(i32::MIN), &[0x71, 0x80, 0x00, 0x00, 0x00]);
    }

    /// §1.6.11: IEEE 754-2008 binary32 after 0x72.
    #[test]
    fn amqp10_1_6_11_a_float_is_0x72_and_binary32() {
        wire(AmqpExtValue::Float(0.0), &[0x72, 0, 0, 0, 0]);
        wire(
            AmqpExtValue::Float(core::f32::consts::PI),
            &[0x72, 0x40, 0x49, 0x0f, 0xdb],
        );
        wire(AmqpExtValue::Float(-1.5), &[0x72, 0xbf, 0xc0, 0x00, 0x00]);
    }

    /// §1.6.12: IEEE 754-2008 binary64 after 0x82.
    #[test]
    fn amqp10_1_6_12_a_double_is_0x82_and_binary64() {
        wire(
            AmqpExtValue::Double(core::f64::consts::E),
            &[0x82, 0x40, 0x05, 0xbf, 0x0a, 0x8b, 0x14, 0x57, 0x69],
        );
    }

    /// §1.6.13 to §1.6.15: 0x74, 0x84 and 0x94 with 4, 8 and 16 octets.
    #[test]
    fn amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets() {
        assert_eq!(encode_decimal32([1, 2, 3, 4]), [0x74, 1, 2, 3, 4]);
        let d64 = encode_decimal64([7; 8]);
        assert_eq!((d64[0], d64.len()), (0x84, 9));
        let d128 = encode_decimal128([9; 16]);
        assert_eq!((d128[0], d128.len()), (0x94, 17));
    }

    /// §1.6.16: a UTF-32BE code point after 0x73.
    #[test]
    fn amqp10_1_6_16_a_char_is_0x73_and_a_utf_32be_code_point() {
        wire(AmqpExtValue::Char('A'), &[0x73, 0x00, 0x00, 0x00, 0x41]);
        wire(AmqpExtValue::Char('ä'), &[0x73, 0x00, 0x00, 0x00, 0xe4]);
        wire(AmqpExtValue::Char('🎉'), &[0x73, 0x00, 0x01, 0xf3, 0x89]);
    }

    /// Spec `dds-amqp-1.0` §7.1.4.2: an IDL `wchar` maps to the AMQP
    /// `char`, "a 4-octet UCS-4 code point", the format code and four
    /// big-endian octets for every code point up to U+10FFFF.
    #[test]
    fn ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian() {
        for c in ['A', 'ä', '🎉', '\u{10FFFF}'] {
            let bytes = AmqpExtValue::Char(c).encode().expect("encode");
            assert_eq!(bytes.len(), 5, "1 format code + 4 octets for {c:?}");
            assert_eq!(bytes[0], 0x73);
            assert_eq!(&bytes[1..], &u32::from(c).to_be_bytes(), "{c:?}");
        }
    }

    /// Spec `dds-amqp-1.0` §7.1.3: "all AMQP multi-byte values are
    /// big-endian", while the XCDR2 source data is little-endian.
    #[test]
    fn ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire() {
        let cases: [(AmqpExtValue, &[u8]); 7] = [
            (AmqpExtValue::Ushort(0x1234), &[0x60, 0x12, 0x34]),
            (
                AmqpExtValue::Uint(0x1122_3344),
                &[0x70, 0x11, 0x22, 0x33, 0x44],
            ),
            (AmqpExtValue::Short(-2), &[0x61, 0xff, 0xfe]),
            (AmqpExtValue::Int(i32::MIN), &[0x71, 0x80, 0, 0, 0]),
            (AmqpExtValue::Float(-1.5), &[0x72, 0xbf, 0xc0, 0, 0]),
            (
                AmqpExtValue::Double(-1.5),
                &[0x82, 0xbf, 0xf8, 0, 0, 0, 0, 0, 0],
            ),
            (
                AmqpExtValue::Timestamp(0x0102_0304_0506_0708),
                &[0x83, 1, 2, 3, 4, 5, 6, 7, 8],
            ),
        ];
        for (value, octets) in cases {
            wire(value, octets);
        }
    }

    /// §1.6.17: milliseconds since the Unix epoch as a signed 64-bit
    /// integer after 0x83.
    #[test]
    fn amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets() {
        wire(
            AmqpExtValue::Timestamp(-1),
            &[0x83, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        );
        wire(
            AmqpExtValue::Timestamp(1_700_000_000_000),
            &[0x83, 0x00, 0x00, 0x01, 0x8b, 0xcf, 0xe5, 0x68, 0x00],
        );
    }

    /// §1.6.18: sixteen octets after 0x98.
    #[test]
    fn amqp10_1_6_18_a_uuid_is_0x98_and_sixteen_octets() {
        let mut u = [0u8; 16];
        for (i, b) in u.iter_mut().enumerate() {
            *b = i as u8;
        }
        let mut octets = alloc::vec![0x98];
        octets.extend_from_slice(&u);
        wire(AmqpExtValue::Uuid(u), &octets);
    }

    /// §1.6.22: the empty list is `list0` 0x45.
    #[test]
    fn amqp10_1_6_22_the_empty_list_is_list0() {
        wire(AmqpExtValue::List(Vec::new()), &[0x45]);
    }

    /// §1.6.22 and §1.2: `list8` 0xc0, the size (count and items), the
    /// count, the items each with its constructor.
    #[test]
    fn amqp10_1_6_22_a_list_is_list8_size_count_and_items() {
        wire(
            AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Boolean(true),
                AmqpExtValue::Int(42),
                AmqpExtValue::Str("hello".to_string())
            ]),
            &[
                0xc0, 0x0b, 0x03, 0x41, 0x54, 0x2a, 0xa1, 0x05, b'h', b'e', b'l', b'l', b'o',
            ],
        );
        let inner = AmqpExtValue::List(alloc::vec![AmqpExtValue::Int(1), AmqpExtValue::Int(2)]);
        wire(
            AmqpExtValue::List(alloc::vec![inner.clone(), inner]),
            &[
                0xc0, 0x0f, 0x02, 0xc0, 0x05, 0x02, 0x54, 0x01, 0x54, 0x02, 0xc0, 0x05, 0x02, 0x54,
                0x01, 0x54, 0x02,
            ],
        );
    }

    /// §1.6.22: decoding stops at the nesting bound of this codec
    /// (`MAX_COMPOUND_DEPTH`, a guard against a stack exhausted by a
    /// sender), on encode and on decode.
    #[test]
    fn amqp10_1_6_22_a_list_nested_past_the_bound_is_refused() {
        let mut current = AmqpExtValue::Int(0);
        for _ in 0..(MAX_COMPOUND_DEPTH + 5) {
            current = AmqpExtValue::List(alloc::vec![current]);
        }
        assert!(current.encode().is_err());
        let bytes = AmqpExtValue::Ubyte(7).encode().unwrap();
        assert!(AmqpExtValue::decode_at(&bytes, MAX_COMPOUND_DEPTH).is_ok());
        assert!(matches!(
            AmqpExtValue::decode_at(&bytes, MAX_COMPOUND_DEPTH + 1),
            Err(TypeError::LengthTooLarge)
        ));
    }

    /// §1.6.23: `map8` 0xc1, the size, the count of keys and values, the
    /// pairs.
    #[test]
    fn amqp10_1_6_23_a_map_is_map8_size_count_and_pairs() {
        wire(
            AmqpExtValue::Map(alloc::vec![
                (AmqpExtValue::Str("k1".to_string()), AmqpExtValue::Int(1)),
                (
                    AmqpExtValue::Str("k2".to_string()),
                    AmqpExtValue::Boolean(true)
                ),
            ]),
            &[
                0xc1, 0x0c, 0x04, 0xa1, 0x02, b'k', b'1', 0x54, 0x01, 0xa1, 0x02, b'k', b'2', 0x41,
            ],
        );
    }

    /// §1.6.23: more than 255 keys and values is `map32` 0xd1 with a
    /// four-octet size and count.
    #[test]
    fn amqp10_1_6_23_a_map_of_more_than_255_elements_is_map32() {
        let entries: Vec<_> = (0..130u32)
            .map(|i| (AmqpExtValue::Uint(i), AmqpExtValue::Uint(i + 1)))
            .collect();
        let bytes = AmqpExtValue::Map(entries.clone()).encode().unwrap();
        assert_eq!(bytes[0], 0xd1);
        let size = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
        assert_eq!(size, bytes.len() - 5, "the size counts what follows it");
        assert_eq!(&bytes[5..9], &[0x00, 0x00, 0x01, 0x04], "260 elements");
        assert_eq!(
            AmqpExtValue::decode(&bytes).unwrap(),
            (AmqpExtValue::Map(entries), bytes.len())
        );
    }

    /// §1.6.24: `array8` 0xe0, the size, the count, one constructor, then
    /// the elements without one.
    #[test]
    fn amqp10_1_6_24_an_array_is_one_constructor_then_the_values() {
        wire(
            AmqpExtValue::Array(alloc::vec![
                AmqpExtValue::Short(10),
                AmqpExtValue::Short(20),
                AmqpExtValue::Short(30)
            ]),
            &[0xe0, 0x08, 0x03, 0x61, 0x00, 0x0a, 0x00, 0x14, 0x00, 0x1e],
        );
        wire(
            AmqpExtValue::Array(alloc::vec![AmqpExtValue::Ubyte(0xab)]),
            &[0xe0, 0x03, 0x01, 0x50, 0xab],
        );
    }

    /// §1.6.24: the elements of an array share one constructor, so uints
    /// whose compact encodings differ (uint0, smalluint, uint) take the
    /// four-octet `uint`, and booleans the one-octet `boolean` 0x56.
    #[test]
    fn amqp10_1_6_24_elements_whose_compact_encodings_differ_share_the_wide_one() {
        wire(
            AmqpExtValue::Array(alloc::vec![
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(1),
                AmqpExtValue::Uint(300)
            ]),
            &[
                0xe0, 0x0e, 0x03, 0x70, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0x01, 0x2c,
            ],
        );
        wire(
            AmqpExtValue::Array(alloc::vec![
                AmqpExtValue::Boolean(true),
                AmqpExtValue::Boolean(false)
            ]),
            &[0xe0, 0x04, 0x02, 0x56, 0x01, 0x00],
        );
        wire(
            AmqpExtValue::Array(alloc::vec![
                AmqpExtValue::Symbol("PLAIN".into()),
                AmqpExtValue::Symbol("ANONYMOUS".into())
            ]),
            &[
                0xe0, 0x12, 0x02, 0xa3, 0x05, b'P', b'L', b'A', b'I', b'N', 0x09, b'A', b'N', b'O',
                b'N', b'Y', b'M', b'O', b'U', b'S',
            ],
        );
    }

    /// §1.6.24: elements of different types are no array.
    #[test]
    fn amqp10_1_6_24_elements_of_different_types_are_refused() {
        let r = AmqpExtValue::Array(alloc::vec![
            AmqpExtValue::Int(1),
            AmqpExtValue::Boolean(true)
        ])
        .encode();
        assert!(r.is_err());
    }

    /// §1.6.24: more than 255 elements is `array32` 0xf0.
    #[test]
    fn amqp10_1_6_24_an_array_of_more_than_255_elements_is_array32() {
        let items: Vec<AmqpExtValue> = (0..300u32)
            .map(|i| AmqpExtValue::Ubyte((i & 0xff) as u8))
            .collect();
        let bytes = AmqpExtValue::Array(items.clone()).encode().unwrap();
        assert_eq!(
            &bytes[..10],
            &[0xf0, 0, 0, 0x01, 0x31, 0, 0, 0x01, 0x2c, 0x50]
        );
        assert_eq!(bytes.len(), 9 + 1 + 300);
        assert_eq!(
            AmqpExtValue::decode(&bytes).unwrap(),
            (AmqpExtValue::Array(items), bytes.len())
        );
    }

    /// Spec `dds-amqp-1.0` §7.1.7 and §7.1.8: a sequence is a list of its
    /// elements, a map a map of its pairs, a nested one inside its parent.
    #[test]
    fn ddsamqp_7_1_7_lists_maps_and_arrays_nest_with_their_own_sizes() {
        wire(
            AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Ubyte(1),
                AmqpExtValue::Ushort(2),
                AmqpExtValue::Uint(3)
            ]),
            &[0xc0, 0x08, 0x03, 0x50, 0x01, 0x60, 0x00, 0x02, 0x52, 0x03],
        );
        wire(
            AmqpExtValue::Map(alloc::vec![
                (AmqpExtValue::Str("a".into()), AmqpExtValue::Uint(1)),
                (AmqpExtValue::Str("b".into()), AmqpExtValue::Uint(2)),
            ]),
            &[
                0xc1, 0x0b, 0x04, 0xa1, 0x01, b'a', 0x52, 0x01, 0xa1, 0x01, b'b', 0x52, 0x02,
            ],
        );
        wire(
            AmqpExtValue::List(alloc::vec![AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Ubyte(42)
            ])]),
            &[0xc0, 0x06, 0x01, 0xc0, 0x03, 0x01, 0x50, 0x2a],
        );
    }

    // -------------------------------------------------------------
    // Wire-supplied element counts are refused when they cannot fit
    // the octets present, before any allocation (a 9-octet packet must
    // not ask for gigabytes).
    // -------------------------------------------------------------

    #[test]
    fn decode_list32_with_huge_count_and_tiny_body_is_rejected_cleanly() {
        let mut bytes = alloc::vec![codes::LIST32];
        bytes.extend_from_slice(&4u32.to_be_bytes());
        bytes.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(
            AmqpExtValue::decode(&bytes),
            Err(TypeError::Truncated)
        ));
    }

    #[test]
    fn decode_map32_with_huge_count_and_tiny_body_is_rejected_cleanly() {
        let mut bytes = alloc::vec![codes::MAP32];
        bytes.extend_from_slice(&4u32.to_be_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFEu32.to_be_bytes());
        assert!(matches!(
            AmqpExtValue::decode(&bytes),
            Err(TypeError::Truncated)
        ));
    }

    #[test]
    fn decode_array32_with_huge_count_and_tiny_body_is_rejected_cleanly() {
        let mut bytes = alloc::vec![codes::ARRAY32];
        bytes.extend_from_slice(&5u32.to_be_bytes());
        bytes.extend_from_slice(&u32::MAX.to_be_bytes());
        bytes.push(codes::SMALLUINT);
        assert!(matches!(
            AmqpExtValue::decode(&bytes),
            Err(TypeError::Truncated)
        ));
    }
}
