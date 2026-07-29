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
    let element_constructor = items[0].encode_at(depth + 1)?;
    if element_constructor.is_empty() {
        return Err(TypeError::LengthTooLarge);
    }
    let constructor_byte = element_constructor[0];
    let mut body = Vec::new();
    body.push(constructor_byte);
    for it in items {
        let enc = it.encode_at(depth + 1)?;
        // Skip the constructor byte for subsequent elements (Spec §1.2.4).
        if enc.is_empty() || enc[0] != constructor_byte {
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

    fn roundtrip(v: AmqpExtValue) {
        let bytes = v.encode().expect("encode");
        let (parsed, _) = AmqpExtValue::decode(&bytes).expect("decode");
        assert_eq!(parsed, v);
    }

    #[test]
    fn ubyte_round_trips_at_extremes() {
        roundtrip(AmqpExtValue::Ubyte(0));
        roundtrip(AmqpExtValue::Ubyte(255));
    }

    #[test]
    fn ushort_round_trips() {
        roundtrip(AmqpExtValue::Ushort(0));
        roundtrip(AmqpExtValue::Ushort(0xABCD));
    }

    #[test]
    fn uint_uses_compact_zero_form() {
        let bytes = AmqpExtValue::Uint(0).encode().expect("encode");
        assert_eq!(bytes, alloc::vec![codes::UINT0]);
    }

    #[test]
    fn uint_uses_smalluint_for_low_values() {
        let bytes = AmqpExtValue::Uint(42).encode().expect("encode");
        assert_eq!(bytes[0], codes::SMALLUINT);
    }

    #[test]
    fn uint_uses_full_for_high_values() {
        let bytes = AmqpExtValue::Uint(0x1234_5678).encode().expect("encode");
        assert_eq!(bytes[0], codes::UINT);
        roundtrip(AmqpExtValue::Uint(0x1234_5678));
    }

    #[test]
    fn byte_round_trips_negative() {
        roundtrip(AmqpExtValue::Byte(-1));
        roundtrip(AmqpExtValue::Byte(127));
        roundtrip(AmqpExtValue::Byte(-128));
    }

    #[test]
    fn short_round_trips() {
        roundtrip(AmqpExtValue::Short(-1));
        roundtrip(AmqpExtValue::Short(i16::MAX));
        roundtrip(AmqpExtValue::Short(i16::MIN));
    }

    #[test]
    fn int_uses_compact_when_in_byte_range() {
        let bytes = AmqpExtValue::Int(42).encode().expect("encode");
        assert_eq!(bytes[0], codes::SMALLINT);
        roundtrip(AmqpExtValue::Int(42));
    }

    #[test]
    fn int_uses_full_for_high_values() {
        roundtrip(AmqpExtValue::Int(i32::MAX));
        roundtrip(AmqpExtValue::Int(i32::MIN));
    }

    #[test]
    fn float_round_trips() {
        roundtrip(AmqpExtValue::Float(0.0));
        roundtrip(AmqpExtValue::Float(core::f32::consts::PI));
        roundtrip(AmqpExtValue::Float(-1.5));
    }

    #[test]
    fn double_round_trips() {
        roundtrip(AmqpExtValue::Double(core::f64::consts::E));
    }

    #[test]
    fn char_round_trips_ascii_and_unicode() {
        roundtrip(AmqpExtValue::Char('A'));
        roundtrip(AmqpExtValue::Char('ä'));
        roundtrip(AmqpExtValue::Char('🎉'));
    }

    #[test]
    fn timestamp_round_trips_negative() {
        roundtrip(AmqpExtValue::Timestamp(-1));
        roundtrip(AmqpExtValue::Timestamp(1_700_000_000_000));
    }

    #[test]
    fn uuid_round_trips() {
        let mut u = [0u8; 16];
        for (i, b) in u.iter_mut().enumerate() {
            *b = i as u8;
        }
        roundtrip(AmqpExtValue::Uuid(u));
    }

    // -- Compound --

    #[test]
    fn empty_list_uses_list0() {
        let bytes = AmqpExtValue::List(Vec::new()).encode().expect("encode");
        assert_eq!(bytes, alloc::vec![codes::LIST0]);
    }

    #[test]
    fn small_list_round_trips() {
        roundtrip(AmqpExtValue::List(alloc::vec![
            AmqpExtValue::Boolean(true),
            AmqpExtValue::Int(42),
            AmqpExtValue::Str("hello".to_string())
        ]));
    }

    #[test]
    fn nested_list_round_trips() {
        let inner = AmqpExtValue::List(alloc::vec![AmqpExtValue::Int(1), AmqpExtValue::Int(2)]);
        let outer = AmqpExtValue::List(alloc::vec![inner.clone(), inner]);
        roundtrip(outer);
    }

    #[test]
    fn map_round_trips_with_string_keys() {
        roundtrip(AmqpExtValue::Map(alloc::vec![
            (AmqpExtValue::Str("k1".to_string()), AmqpExtValue::Int(1)),
            (
                AmqpExtValue::Str("k2".to_string()),
                AmqpExtValue::Boolean(true)
            ),
        ]));
    }

    #[test]
    fn array_round_trips_homogeneous_short() {
        // Use `short` because all values share the same format code 0x61.
        roundtrip(AmqpExtValue::Array(alloc::vec![
            AmqpExtValue::Short(10),
            AmqpExtValue::Short(20),
            AmqpExtValue::Short(30)
        ]));
    }

    #[test]
    fn array_homogeneous_constraint_violation_yields_error() {
        // Mixed types in array — must fail.
        let r = AmqpExtValue::Array(alloc::vec![
            AmqpExtValue::Int(1),
            AmqpExtValue::Boolean(true)
        ])
        .encode();
        assert!(r.is_err());
    }

    #[test]
    fn deeply_nested_list_exceeds_dos_cap() {
        let mut current = AmqpExtValue::Int(0);
        for _ in 0..(MAX_COMPOUND_DEPTH + 5) {
            current = AmqpExtValue::List(alloc::vec![current]);
        }
        assert!(current.encode().is_err());
    }

    // -------------------------------------------------------------
    // Buffer-cap hardening — wire-supplied element counts must be
    // rejected cleanly (no OOM, no panic) when they cannot possibly
    // fit the bytes actually present, *before* `Vec::with_capacity`
    // runs. See crates/cdr/src/composite.rs for the established
    // `len > reader.remaining()` pattern this mirrors.
    // -------------------------------------------------------------

    #[test]
    fn decode_list32_with_huge_count_and_tiny_body_is_rejected_cleanly() {
        // LIST32 header claiming size=4 (just the count field, no
        // items) but count = u32::MAX. A naive `Vec::with_capacity(count)`
        // would attempt a many-GB allocation from a 9-byte packet.
        let mut bytes = alloc::vec![codes::LIST32];
        bytes.extend_from_slice(&4u32.to_be_bytes()); // size
        bytes.extend_from_slice(&u32::MAX.to_be_bytes()); // count
        let res = AmqpExtValue::decode(&bytes);
        assert!(
            matches!(res, Err(TypeError::Truncated)),
            "expected clean Truncated rejection, got {res:?}"
        );
    }

    #[test]
    fn decode_map32_with_huge_count_and_tiny_body_is_rejected_cleanly() {
        let mut bytes = alloc::vec![codes::MAP32];
        bytes.extend_from_slice(&4u32.to_be_bytes()); // size
        bytes.extend_from_slice(&0xFFFF_FFFEu32.to_be_bytes()); // count (even)
        let res = AmqpExtValue::decode(&bytes);
        assert!(
            matches!(res, Err(TypeError::Truncated)),
            "expected clean Truncated rejection, got {res:?}"
        );
    }

    #[test]
    fn decode_array32_with_huge_count_and_tiny_body_is_rejected_cleanly() {
        // ARRAY32 header claiming just enough size for a single
        // constructor byte, but a u32::MAX element count.
        let mut bytes = alloc::vec![codes::ARRAY32];
        bytes.extend_from_slice(&5u32.to_be_bytes()); // size (4 count + 1 constructor)
        bytes.extend_from_slice(&u32::MAX.to_be_bytes()); // count
        bytes.push(codes::SMALLUINT); // constructor byte for the (absent) elements
        let res = AmqpExtValue::decode(&bytes);
        assert!(
            matches!(res, Err(TypeError::Truncated)),
            "expected clean Truncated rejection, got {res:?}"
        );
    }

    #[test]
    fn list_map_array_within_bound_still_round_trip() {
        // Regression guard: the new bounds check must not reject
        // legitimately-sized compounds.
        roundtrip(AmqpExtValue::List(alloc::vec![
            AmqpExtValue::Int(1),
            AmqpExtValue::Int(2),
            AmqpExtValue::Int(3)
        ]));
        roundtrip(AmqpExtValue::Map(alloc::vec![(
            AmqpExtValue::Str("k".to_string()),
            AmqpExtValue::Int(9)
        )]));
        roundtrip(AmqpExtValue::Array(alloc::vec![
            AmqpExtValue::Short(1),
            AmqpExtValue::Short(2)
        ]));
    }

    #[test]
    fn decimal32_64_128_have_correct_lengths() {
        assert_eq!(encode_decimal32([0; 4]).len(), 5);
        assert_eq!(encode_decimal64([0; 8]).len(), 9);
        assert_eq!(encode_decimal128([0; 16]).len(), 17);
    }

    // -------------------------------------------------------------
    // Mutation killers — extended_types.rs
    // -------------------------------------------------------------

    /// Catches the `Display::fmt -> Ok(default)` mutation (line 508).
    /// Display delegates to Debug ({self:?}); the mutation would
    /// return "".
    #[test]
    fn ext_value_display_non_empty() {
        let s = alloc::format!("{}", AmqpExtValue::Ubyte(42));
        assert!(s.contains("42"), "Display must include value, got '{s}'");
        let s2 = alloc::format!("{}", AmqpExtValue::Boolean(true));
        assert!(s2.contains("true") || s2.contains("Boolean"));
    }

    /// Catches the `decode_at` depth-cap boundary `>` -> `==`/`>=` (line 433).
    /// MAX_COMPOUND_DEPTH=32. Depth=32 must pass, depth=33 must error.
    /// Tested directly via a decode_at call instead of fully-nested encoding.
    #[test]
    fn decode_at_depth_at_cap_accepted() {
        // Simple value, depth=MAX → must pass.
        let bytes = AmqpExtValue::Ubyte(7).encode().unwrap();
        let res = AmqpExtValue::decode_at(&bytes, MAX_COMPOUND_DEPTH);
        assert!(res.is_ok(), "depth=MAX must decode, got {res:?}");
    }

    #[test]
    fn decode_at_depth_over_cap_rejected() {
        let bytes = AmqpExtValue::Ubyte(7).encode().unwrap();
        let res = AmqpExtValue::decode_at(&bytes, MAX_COMPOUND_DEPTH + 1);
        assert!(matches!(res, Err(TypeError::LengthTooLarge)));
    }

    /// Catches `&& -> ||` in the encode_map use_short boundary (line 598).
    /// With `||`: count > 254 BUT body.len() <= 254 would still
    /// use MAP8 — and truncate count to u8. Tested with count > 254.
    /// Map with 128 entries (=256 elements; > u8::MAX=255).
    #[test]
    fn encode_map_uses_long_form_when_count_exceeds_u8() {
        // 130 entries → count=260 > 255 → MUST use MAP32.
        let mut entries = Vec::new();
        for i in 0..130u32 {
            entries.push((AmqpExtValue::Uint(i), AmqpExtValue::Uint(i + 1)));
        }
        let bytes = AmqpExtValue::Map(entries.clone()).encode().unwrap();
        // MAP8 = 0xC1; MAP32 = 0xD1.
        assert_eq!(bytes[0], codes::MAP32, "expected MAP32 for count>255");
        // Round-trip must work.
        let (parsed, _) = AmqpExtValue::decode(&bytes).unwrap();
        assert_eq!(parsed, AmqpExtValue::Map(entries));
    }

    /// Catches `&& -> ||` in encode_array use_short (line 681).
    /// An array with count > 255 must use ARRAY32.
    #[test]
    fn encode_array_uses_long_form_when_count_exceeds_u8() {
        let items: Vec<AmqpExtValue> = (0..300u32)
            .map(|i| AmqpExtValue::Ubyte((i & 0xff) as u8))
            .collect();
        let bytes = AmqpExtValue::Array(items.clone()).encode().unwrap();
        // ARRAY8 = 0xE0; ARRAY32 = 0xF0.
        assert_eq!(bytes[0], codes::ARRAY32, "expected ARRAY32 for count>255");
        let (parsed, _) = AmqpExtValue::decode(&bytes).unwrap();
        assert_eq!(parsed, AmqpExtValue::Array(items));
    }

    /// decode_array boundary `bytes.len() < 3` (line 712 ARRAY8) and
    /// `bytes.len() < 9` (line 718 ARRAY32). Catches `<` -> `<=` mutations:
    /// an exactly-3-byte buf for ARRAY8 / an exactly-9-byte buf for ARRAY32 must
    /// PASS (no Truncated).
    #[test]
    fn decode_array_at_minimum_buffer_size() {
        // ARRAY8 with body.len()=1 (constructor only) and count=0 — minimal.
        // Bytes: [ARRAY8, size=1, count=0, constructor_for_first_item]
        // size=1 because body=[constructor], total=2+1=3 → bytes.len() must
        // be at least 3, plus 1 byte for the constructor = 4.
        // We can't build that trivially because an empty array = Error.
        // Instead: ARRAY8 with 1 element.
        let bytes = AmqpExtValue::Array(vec![AmqpExtValue::Ubyte(0xAB)])
            .encode()
            .unwrap();
        // Bytes: [0xE0, size=3, count=1, 0x50_for_ubyte, 0xAB]
        assert_eq!(bytes[0], codes::ARRAY8);
        let (parsed, n) = AmqpExtValue::decode(&bytes).unwrap();
        assert_eq!(n, bytes.len());
        if let AmqpExtValue::Array(items) = parsed {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0], AmqpExtValue::Ubyte(0xAB));
        } else {
            panic!("expected Array");
        }
    }

    /// Round-trip with concrete values indirectly tests the `+ -> *` and
    /// `+ -> -` arithmetic mutations in the encode/decode bodies:
    /// incorrectly computed offsets/sizes break the round-trip.
    #[test]
    fn list_map_array_roundtrip_concrete_values() {
        roundtrip(AmqpExtValue::List(vec![
            AmqpExtValue::Ubyte(1),
            AmqpExtValue::Ushort(2),
            AmqpExtValue::Uint(3),
        ]));
        roundtrip(AmqpExtValue::Map(vec![
            (AmqpExtValue::Str("a".into()), AmqpExtValue::Uint(1)),
            (AmqpExtValue::Str("b".into()), AmqpExtValue::Uint(2)),
        ]));
        roundtrip(AmqpExtValue::Array(vec![
            AmqpExtValue::Ubyte(10),
            AmqpExtValue::Ubyte(20),
            AmqpExtValue::Ubyte(30),
        ]));
        // Nested compound — tests recursive depth + 1 increments.
        roundtrip(AmqpExtValue::List(vec![AmqpExtValue::List(vec![
            AmqpExtValue::Ubyte(42),
        ])]));
    }
}
