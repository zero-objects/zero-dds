// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Composite-type encoder/decoder (W2).
//!
//! XCDR2 wire-format conventions (OMG XTypes 1.3 §7.4):
//!
//! - **String** (§7.4.4): `uint32` length in bytes **including the null
//!   terminator** + UTF-8 bytes + `\0`.
//! - **Sequence** (§7.4.4.2): `uint32` element count + elements
//!   (each element after its own alignment).
//! - **Array** (§7.4.4.3): `N` elements without a length prefix.
//! - **Optional** (§7.4.5.1.4): `uint8` present flag (0/1) + value
//!   if present.

// Module is only compiled under the `alloc` feature (the re-export in
// lib.rs has the `cfg`); this file depends on `Vec`/`String`.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::type_code::TypeCode;

use crate::buffer::{BufferReader, BufferWriter, XCDR2_MAX_ALIGNMENT};
use crate::encode::{CdrDecode, CdrEncode};
use crate::error::{DecodeError, EncodeError};

// ============================================================================
// XCDR2 DHEADER for collections with non-primitive elements
// ============================================================================
//
// OMG XTypes 1.3 §7.4.3.5: a `sequence<T>` or an array `T[N]` gets a
// DHEADER (uint32 = byte length of the following content) PREPENDED
// under XCDR2 (PLAIN_CDR2) when `T` is NON-primitive (string, struct,
// union, enum, sequence, array, map). Primitive elements
// (int/float/bool/char/octet) get NO DHEADER. Verified against Cyclone
// DDS (V-5 seq<long> without, V-6 seq<string>/seq<struct>/seq<enum>
// with DHEADER). Under XCDR1 (max_alignment == 8) there is never a
// DHEADER — hence the gate on `max_alignment == XCDR2_MAX_ALIGNMENT`.

/// `true` when, under XCDR2, a collection DHEADER is needed for an
/// element type with `elem_is_primitive`.
#[inline]
fn needs_collection_dheader(writer_max_alignment: usize, elem_is_primitive: bool) -> bool {
    !elem_is_primitive && writer_max_alignment == XCDR2_MAX_ALIGNMENT
}

/// Serializes `body` into a sub-writer (same endianness + alignment
/// cap), prepends a uint32 DHEADER (= body byte length) and writes both
/// to `writer`. Alignment is equivalent because the DHEADER content
/// always starts 4-aligned and XCDR2 caps at 4.
fn write_with_dheader<F>(writer: &mut BufferWriter, body: F) -> Result<(), EncodeError>
where
    F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
{
    let mut sub = BufferWriter::new(writer.endianness()).with_max_alignment(writer.max_alignment());
    body(&mut sub)?;
    let bytes = sub.into_bytes();
    let dheader = u32::try_from(bytes.len()).map_err(|_| EncodeError::ValueOutOfRange {
        message: "collection DHEADER exceeds u32::MAX",
    })?;
    writer.write_u32(dheader)?;
    writer.write_bytes(&bytes)
}

// ============================================================================
// String / &str
// ============================================================================

impl CdrEncode for str {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        let bytes = self.as_bytes();
        // Length in bytes including the null terminator.
        let len_with_nul = bytes
            .len()
            .checked_add(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(EncodeError::ValueOutOfRange {
                message: "string length exceeds u32::MAX",
            })?;
        writer.write_u32(len_with_nul)?;
        writer.write_bytes(bytes)?;
        writer.write_u8(0)?;
        Ok(())
    }
}

impl CdrEncode for String {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        self.as_str().encode(writer)
    }
}

impl CdrDecode for String {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len_with_nul = reader.read_u32()? as usize;
        if len_with_nul == 0 {
            return Err(DecodeError::LengthExceeded {
                announced: 0,
                remaining: reader.remaining(),
                offset: reader.position(),
            });
        }
        if len_with_nul > reader.remaining() {
            return Err(DecodeError::LengthExceeded {
                announced: len_with_nul,
                remaining: reader.remaining(),
                offset: reader.position(),
            });
        }
        // Last byte must be the null terminator.
        let payload_len = len_with_nul - 1;
        let offset = reader.position();
        let bytes = reader.read_bytes(payload_len)?;
        let s = core::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8 { offset })?;
        let owned = String::from(s);
        // Consume the null terminator.
        let nul = reader.read_u8()?;
        if nul != 0 {
            return Err(DecodeError::InvalidUtf8 { offset });
        }
        Ok(owned)
    }
}

// ============================================================================
// WString — IDL `wstring` (CORBA-GIOP-1.2-Wire, §9.3.2.7 / §15.3.2.7)
// ============================================================================

/// IDL `wstring` wrapper. Holds the text as a Rust `String` (Unicode), but the
/// **wire format** differs from `string`: GIOP 1.2 encodes a `wstring` as a
/// `uint32` length **in octets** (NOT characters, NOT incl. terminator)
/// followed by the UTF-16 code units in the message byte order — **without** a
/// null terminator. This makes `wstring` distinct from `string` (UTF-8) and
/// interop-capable with ORBs whose transmission codeset is UTF-16 (the default
/// for omniORB/TAO/JacORB).
#[derive(Debug, Clone, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
pub struct WString(pub String);

impl WString {
    /// Borrows the inner text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for WString {
    fn from(s: &str) -> Self {
        Self(String::from(s))
    }
}

impl From<String> for WString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Byte-order mark for UTF-16 (§15.3.1.6): `0xFEFF`. A reader with the
/// reverse byte order sees the mirrored `0xFFFE` and swaps.
const UTF16_BOM: u16 = 0xFEFF;

impl CdrEncode for WString {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        // GIOP 1.2 wstring (§15.3.2.7): uint32 length **in octets**, then the
        // UTF-16 code units, no null terminator. Per §15.3.1.6 a byte-order
        // mark (0xFEFF) in message byte order is prepended — exactly as
        // omniORB/TAO send; the BOM makes the units endianness-unambiguous
        // (a reader of the other order detects 0xFFFE and swaps). The length
        // octets include the BOM. An empty wstring = length 0 (no BOM),
        // as conventioned by all ORBs.
        let units = self.0.encode_utf16().count();
        if units == 0 {
            writer.write_u32(0)?;
            return Ok(());
        }
        let total_units = units.saturating_add(1); // + BOM
        let octets = u32::try_from(total_units.saturating_mul(2)).map_err(|_| {
            EncodeError::ValueOutOfRange {
                message: "CDR wstring length exceeds u32::MAX",
            }
        })?;
        writer.write_u32(octets)?;
        // write_u16 respects endianness; align(2) is a no-op here, since the
        // position after the uint32 is 4-aligned (and thus 2-aligned).
        writer.write_u16(UTF16_BOM)?;
        for unit in self.0.encode_utf16() {
            writer.write_u16(unit)?;
        }
        Ok(())
    }
}

impl CdrDecode for WString {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let octets = reader.read_u32()? as usize;
        if octets % 2 != 0 || octets > reader.remaining() {
            return Err(DecodeError::LengthExceeded {
                announced: octets,
                remaining: reader.remaining(),
                offset: reader.position(),
            });
        }
        if octets == 0 {
            return Ok(Self(String::new()));
        }
        let offset = reader.position();
        // Read the raw octets and interpret per §15.3.1.6: a leading BOM
        // determines the byte order of the units; if absent (e.g. JacORB),
        // big-endian applies. This way ZeroDDS reads both omniORB/TAO (BOM)
        // and JacORB (BE without BOM), independent of the message byte order.
        let bytes = reader.read_bytes(octets)?;
        let (start, big_endian) = match (bytes[0], bytes[1]) {
            (0xFE, 0xFF) => (2, true),  // BOM big-endian
            (0xFF, 0xFE) => (2, false), // BOM little-endian
            _ => (0, true),             // no BOM -> big-endian default
        };
        let mut units = Vec::with_capacity((octets - start) / 2);
        let mut idx = start;
        while idx + 1 < octets {
            let pair = [bytes[idx], bytes[idx + 1]];
            units.push(if big_endian {
                u16::from_be_bytes(pair)
            } else {
                u16::from_le_bytes(pair)
            });
            idx += 2;
        }
        let s = String::from_utf16(&units).map_err(|_| DecodeError::InvalidUtf8 { offset })?;
        Ok(Self(s))
    }
}

// ============================================================================
// CorbaAny — IDL `any` (CORBA-GIOP-Wire, §15.3.7: TypeCode + Value)
// ============================================================================

/// Value variants a [`CorbaAny`] can carry: all scalar IDL types +
/// string/wstring **and** structured types (sequence/struct/enum + nested
/// any). The structured variants carry enough type info that the full
/// [`TypeCode`] (§15.3.5) can be derived from them (e.g. the element
/// TypeCode even for an empty sequence).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AnyValue {
    /// `tk_null`.
    #[default]
    Null,
    /// `tk_boolean`.
    Boolean(bool),
    /// `tk_octet`.
    Octet(u8),
    /// `tk_char`.
    Char(u8),
    /// `tk_short`.
    Short(i16),
    /// `tk_ushort`.
    UShort(u16),
    /// `tk_long`.
    Long(i32),
    /// `tk_ulong`.
    ULong(u32),
    /// `tk_longlong`.
    LongLong(i64),
    /// `tk_ulonglong`.
    ULongLong(u64),
    /// `tk_float`.
    Float(f32),
    /// `tk_double`.
    Double(f64),
    /// `tk_wchar`.
    WChar(u16),
    /// `tk_string`.
    Str(String),
    /// `tk_wstring`.
    WStr(WString),
    /// `tk_sequence`: element TypeCode (needed even for an empty sequence) + items.
    Seq {
        /// TypeCode of the elements.
        element: TypeCode,
        /// Element values.
        items: Vec<AnyValue>,
    },
    /// `tk_struct`: RepositoryId + name + ordered `(member_name, value)`.
    Struct {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// Struct name.
        name: String,
        /// Members in declaration order.
        members: Vec<(String, AnyValue)>,
    },
    /// `tk_enum`: RepositoryId + name + ordinal value + enumerator names.
    Enum {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// Enum name.
        name: String,
        /// Ordinal value (index into `members`).
        value: u32,
        /// Enumerator names.
        members: Vec<String>,
    },
    /// `tk_any`: nested `any`.
    Any(Box<CorbaAny>),
}

impl AnyValue {
    /// Derives the full [`TypeCode`] (§15.3.5) of this value.
    #[must_use]
    pub fn type_code(&self) -> TypeCode {
        match self {
            Self::Null => TypeCode::Null,
            Self::Boolean(_) => TypeCode::Boolean,
            Self::Octet(_) => TypeCode::Octet,
            Self::Char(_) => TypeCode::Char,
            Self::Short(_) => TypeCode::Short,
            Self::UShort(_) => TypeCode::UShort,
            Self::Long(_) => TypeCode::Long,
            Self::ULong(_) => TypeCode::ULong,
            Self::LongLong(_) => TypeCode::LongLong,
            Self::ULongLong(_) => TypeCode::ULongLong,
            Self::Float(_) => TypeCode::Float,
            Self::Double(_) => TypeCode::Double,
            Self::WChar(_) => TypeCode::WChar,
            Self::Str(_) => TypeCode::String(0),
            Self::WStr(_) => TypeCode::WString(0),
            Self::Seq { element, .. } => TypeCode::Sequence {
                element: Box::new(element.clone()),
                bound: 0,
            },
            Self::Struct {
                repo_id,
                name,
                members,
            } => TypeCode::Struct {
                repo_id: repo_id.clone(),
                name: name.clone(),
                members: members
                    .iter()
                    .map(|(n, v)| (n.clone(), v.type_code()))
                    .collect(),
                is_except: false,
            },
            Self::Enum {
                repo_id,
                name,
                members,
                ..
            } => TypeCode::Enum {
                repo_id: repo_id.clone(),
                name: name.clone(),
                members: members.clone(),
            },
            Self::Any(_) => TypeCode::Any,
        }
    }

    /// Writes **only the value** (without the TypeCode), in its CDR representation.
    fn encode_value(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        match self {
            Self::Null => Ok(()),
            Self::Boolean(v) => v.encode(w),
            Self::Octet(v) => v.encode(w),
            Self::Char(v) => v.encode(w),
            Self::Short(v) => v.encode(w),
            Self::UShort(v) => v.encode(w),
            Self::Long(v) => v.encode(w),
            Self::ULong(v) => v.encode(w),
            Self::LongLong(v) => v.encode(w),
            Self::ULongLong(v) => v.encode(w),
            Self::Float(v) => v.encode(w),
            Self::Double(v) => v.encode(w),
            Self::WChar(v) => v.encode(w),
            Self::Str(s) => s.encode(w),
            Self::WStr(s) => s.encode(w),
            Self::Seq { items, .. } => {
                let len = u32::try_from(items.len()).map_err(|_| EncodeError::ValueOutOfRange {
                    message: "any sequence length exceeds u32",
                })?;
                w.write_u32(len)?;
                for it in items {
                    it.encode_value(w)?;
                }
                Ok(())
            }
            Self::Struct { members, .. } => {
                for (_, v) in members {
                    v.encode_value(w)?;
                }
                Ok(())
            }
            Self::Enum { value, .. } => w.write_u32(*value),
            Self::Any(inner) => inner.encode(w),
        }
    }

    /// Reads **only the value**, guided by the TypeCode `tc`.
    fn decode_value(tc: &TypeCode, r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        match tc {
            TypeCode::Null | TypeCode::Void => Ok(Self::Null),
            TypeCode::Boolean => Ok(Self::Boolean(bool::decode(r)?)),
            TypeCode::Octet => Ok(Self::Octet(u8::decode(r)?)),
            TypeCode::Char => Ok(Self::Char(u8::decode(r)?)),
            TypeCode::Short => Ok(Self::Short(i16::decode(r)?)),
            TypeCode::UShort => Ok(Self::UShort(u16::decode(r)?)),
            TypeCode::Long => Ok(Self::Long(i32::decode(r)?)),
            TypeCode::ULong => Ok(Self::ULong(u32::decode(r)?)),
            TypeCode::LongLong => Ok(Self::LongLong(i64::decode(r)?)),
            TypeCode::ULongLong => Ok(Self::ULongLong(u64::decode(r)?)),
            TypeCode::Float => Ok(Self::Float(f32::decode(r)?)),
            TypeCode::Double => Ok(Self::Double(f64::decode(r)?)),
            TypeCode::WChar => Ok(Self::WChar(u16::decode(r)?)),
            TypeCode::String(_) => Ok(Self::Str(String::decode(r)?)),
            TypeCode::WString(_) => Ok(Self::WStr(WString::decode(r)?)),
            TypeCode::Sequence { element, .. } => {
                let count = r.read_u32()? as usize;
                let mut items = Vec::with_capacity(count.min(4096));
                for _ in 0..count {
                    items.push(Self::decode_value(element, r)?);
                }
                Ok(Self::Seq {
                    element: (**element).clone(),
                    items,
                })
            }
            TypeCode::Struct {
                repo_id,
                name,
                members,
                ..
            } => {
                let mut out = Vec::with_capacity(members.len());
                for (mn, mt) in members {
                    out.push((mn.clone(), Self::decode_value(mt, r)?));
                }
                Ok(Self::Struct {
                    repo_id: repo_id.clone(),
                    name: name.clone(),
                    members: out,
                })
            }
            TypeCode::Enum {
                repo_id,
                name,
                members,
            } => Ok(Self::Enum {
                repo_id: repo_id.clone(),
                name: name.clone(),
                value: r.read_u32()?,
                members: members.clone(),
            }),
            // typedef resolves transparently to the content.
            TypeCode::Alias { content, .. } => Self::decode_value(content, r),
            TypeCode::Any => Ok(Self::Any(Box::new(CorbaAny::decode(r)?))),
            // ObjRef/TypeCode value inside an any: not yet supported.
            TypeCode::ObjRef { .. } | TypeCode::TypeCodeTc => Err(DecodeError::InvalidEnum {
                kind: "any value (objref/TypeCode value unsupported)",
                value: tc.tckind(),
            }),
            // Recursive marker: a value can only be decoded against the
            // RESOLVED type (a recursive any-value is a separate feature).
            TypeCode::Recursive { .. } => Err(DecodeError::InvalidEnum {
                kind: "any value against recursive TypeCode marker unsupported",
                value: tc.tckind(),
            }),
        }
    }
}

/// IDL `any` (§15.3.7): self-describing = `TypeCode` + `Value`. On the wire,
/// the value in its representation follows the full [`TypeCode`] (§15.3.5) —
/// wire-compatible with omniORB/TAO/JacORB, also for structured content
/// (sequence/struct/enum/nested any).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CorbaAny(pub AnyValue);

impl CdrEncode for CorbaAny {
    fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.0.type_code().encode(w)?;
        self.0.encode_value(w)
    }
}

impl CdrDecode for CorbaAny {
    fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let tc = TypeCode::decode(r)?;
        Ok(Self(AnyValue::decode_value(&tc, r)?))
    }
}

// ============================================================================
// Sequence (Vec<T>)
// ============================================================================

impl<T: CdrEncode> CdrEncode for Vec<T> {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        let len = u32::try_from(self.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "sequence length exceeds u32::MAX",
        })?;
        if needs_collection_dheader(writer.max_alignment(), T::IS_PRIMITIVE) {
            // XCDR2 §7.4.3.5: DHEADER covers [count + elements].
            write_with_dheader(writer, |sub| {
                sub.write_u32(len)?;
                for item in self {
                    item.encode(sub)?;
                }
                Ok(())
            })
        } else {
            writer.write_u32(len)?;
            for item in self {
                item.encode(writer)?;
            }
            Ok(())
        }
    }
}

impl<T: CdrDecode> CdrDecode for Vec<T> {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        if needs_collection_dheader(reader.max_alignment(), T::IS_PRIMITIVE) {
            // XCDR2 §7.4.3.5: skip the DHEADER before [count + elements].
            let _dheader = reader.read_u32()?;
        }
        let len = reader.read_u32()? as usize;
        // Defensive sanity check: cannot have more elements than
        // remaining bytes (at least 1 byte per element).
        if len > reader.remaining() {
            return Err(DecodeError::LengthExceeded {
                announced: len,
                remaining: reader.remaining(),
                offset: reader.position(),
            });
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(T::decode(reader)?);
        }
        Ok(out)
    }
}

// ============================================================================
// Array [T; N]
// ============================================================================

impl<T: CdrEncode, const N: usize> CdrEncode for [T; N] {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        if needs_collection_dheader(writer.max_alignment(), T::IS_PRIMITIVE) {
            // XCDR2 §7.4.3.5: array without count, DHEADER covers only elements.
            write_with_dheader(writer, |sub| {
                for item in self {
                    item.encode(sub)?;
                }
                Ok(())
            })
        } else {
            for item in self {
                item.encode(writer)?;
            }
            Ok(())
        }
    }
}

impl<T: CdrDecode + Default + Copy, const N: usize> CdrDecode for [T; N] {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        if needs_collection_dheader(reader.max_alignment(), T::IS_PRIMITIVE) {
            // XCDR2 §7.4.3.5: DHEADER (uint32) before the array elements.
            let _dheader = reader.read_u32()?;
        }
        let mut out = [T::default(); N];
        for slot in &mut out {
            *slot = T::decode(reader)?;
        }
        Ok(out)
    }
}

// ============================================================================
// Optional<T>
// ============================================================================

impl<T: CdrEncode> CdrEncode for Option<T> {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        match self {
            None => writer.write_u8(0),
            Some(value) => {
                writer.write_u8(1)?;
                value.encode(writer)
            }
        }
    }
}

impl<T: CdrDecode> CdrDecode for Option<T> {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let offset = reader.position();
        let flag = reader.read_u8()?;
        match flag {
            0 => Ok(None),
            1 => Ok(Some(T::decode(reader)?)),
            // Other values are forbidden by the XCDR spec — we use
            // InvalidBool as a pragmatic match (boolean semantics).
            other => Err(DecodeError::InvalidBool {
                value: other,
                offset,
            }),
        }
    }
}

// ============================================================================
// Map<K, V> — XCDR2 §7.4.4.6
// ============================================================================
//
// Wire format: 4-byte u32 entry count + N × (K, V) pairs. We
// serialize entries in BTreeMap iteration order (which is key-sorted,
// hence reproducible). Decode rebuilds a BTreeMap.

use alloc::collections::BTreeMap;

impl<K, V> CdrEncode for BTreeMap<K, V>
where
    K: CdrEncode + Ord,
    V: CdrEncode,
{
    fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        let len = u32::try_from(self.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "map: entry-count > u32::MAX",
        })?;
        // A map is a non-primitive collection -> DHEADER under XCDR2
        // (§7.4.3.5/§7.4.4.6). Rule-derived (not Cyclone-captured).
        if w.max_alignment() == XCDR2_MAX_ALIGNMENT {
            write_with_dheader(w, |sub| {
                sub.write_u32(len)?;
                for (k, v) in self {
                    k.encode(sub)?;
                    v.encode(sub)?;
                }
                Ok(())
            })
        } else {
            w.write_u32(len)?;
            for (k, v) in self {
                k.encode(w)?;
                v.encode(w)?;
            }
            Ok(())
        }
    }
}

impl<K, V> CdrDecode for BTreeMap<K, V>
where
    K: CdrDecode + Ord,
    V: CdrDecode,
{
    fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        if r.max_alignment() == XCDR2_MAX_ALIGNMENT {
            let _dheader = r.read_u32()?;
        }
        let len = r.read_u32()? as usize;
        let mut map = BTreeMap::new();
        for _ in 0..len {
            let k = K::decode(r)?;
            let v = V::decode(r)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::Endianness;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn wstring_giop12_wire_format_and_roundtrip() {
        // "Aü€" -> UTF-16: 0x0041, 0x00FC, 0x20AC -> 3 units + BOM = 4 units = 8 octets.
        let ws = WString::from("Aü€");
        let mut w = BufferWriter::new(Endianness::Big);
        ws.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        // Wire: uint32 length-in-octets (8, BE) + BOM(0xFEFF) + 3×u16 (BE), NO terminator.
        assert_eq!(
            &bytes[0..4],
            &[0, 0, 0, 8],
            "length in octets incl. BOM, not characters"
        );
        assert_eq!(
            &bytes[4..],
            &[0xFE, 0xFF, 0x00, 0x41, 0x00, 0xFC, 0x20, 0xAC]
        );
        assert_eq!(bytes.len(), 12, "no null terminator");

        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(WString::decode(&mut r).unwrap(), ws);
    }

    #[test]
    fn wstring_decodes_foreign_byte_order_via_bom() {
        // BE message, but the UTF-16 units carry an LE BOM (0xFFFE) and
        // are little-endian encoded (permitted per §15.3.1.6 — the BOM
        // controls the unit order independent of the message order). The
        // length prefix is message order (BE). "Aü€" = 0x0041,0x00FC,0x20AC + BOM = 8 octets.
        let mut bytes = vec![0, 0, 0, 8]; // length BE, incl. BOM
        bytes.extend_from_slice(&[0xFF, 0xFE]); // BOM little-endian
        bytes.extend_from_slice(&[0x41, 0x00, 0xFC, 0x00, 0xAC, 0x20]); // units LE
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(WString::decode(&mut r).unwrap(), WString::from("Aü€"));
    }

    #[test]
    fn wstring_decodes_jacorb_style_no_bom_big_endian() {
        // JacORB sends UTF-16 big-endian WITHOUT a BOM in a BE message. The
        // default BE path must apply. "Aü€" = 6 octets (no BOM).
        let mut bytes = vec![0, 0, 0, 6];
        bytes.extend_from_slice(&[0x00, 0x41, 0x00, 0xFC, 0x20, 0xAC]); // "Aü€" BE
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(WString::decode(&mut r).unwrap(), WString::from("Aü€"));
    }

    #[test]
    fn wstring_little_endian_roundtrips() {
        let ws = WString::from("hello wörld 🌍");
        let mut w = BufferWriter::new(Endianness::Little);
        ws.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(WString::decode(&mut r).unwrap(), ws);
    }

    #[test]
    fn corba_any_simple_types_roundtrip() {
        for (label, v) in [
            ("long", AnyValue::Long(-123456)),
            ("ulong", AnyValue::ULong(4_000_000_000)),
            ("double", AnyValue::Double(2.5)),
            ("boolean", AnyValue::Boolean(true)),
            ("octet", AnyValue::Octet(0xAB)),
            ("longlong", AnyValue::LongLong(-1_000_000_000_000)),
            ("short", AnyValue::Short(-7)),
            ("char", AnyValue::Char(b'Q')),
        ] {
            for e in [Endianness::Big, Endianness::Little] {
                let any = CorbaAny(v.clone());
                let mut w = BufferWriter::new(e);
                any.encode(&mut w).unwrap();
                let bytes = w.into_bytes();
                let mut r = BufferReader::new(&bytes, e);
                assert_eq!(CorbaAny::decode(&mut r).unwrap(), any, "{label}/{e:?}");
            }
        }
    }

    #[test]
    fn corba_any_long_wire_format() {
        // tk_long (3) + i32 value. BE: [0,0,0,3][0,0,0,42].
        let any = CorbaAny(AnyValue::Long(42));
        let mut w = BufferWriter::new(Endianness::Big);
        any.encode(&mut w).unwrap();
        assert_eq!(w.into_bytes(), vec![0, 0, 0, 3, 0, 0, 0, 42]);
    }

    #[test]
    fn corba_any_string_roundtrip() {
        let any = CorbaAny(AnyValue::Str("héllo".to_string()));
        let mut w = BufferWriter::new(Endianness::Little);
        any.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        // tk_string (18) + bound (0) + CDR string.
        assert_eq!(&bytes[0..8], &[18, 0, 0, 0, 0, 0, 0, 0]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(CorbaAny::decode(&mut r).unwrap(), any);
    }

    fn any_rt(v: AnyValue) {
        for e in [Endianness::Big, Endianness::Little] {
            let any = CorbaAny(v.clone());
            let mut w = BufferWriter::new(e);
            any.encode(&mut w).unwrap();
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, e);
            assert_eq!(CorbaAny::decode(&mut r).unwrap(), any, "{v:?} / {e:?}");
        }
    }

    #[test]
    fn corba_any_sequence_of_long() {
        any_rt(AnyValue::Seq {
            element: TypeCode::Long,
            items: vec![AnyValue::Long(1), AnyValue::Long(-2), AnyValue::Long(3)],
        });
        // Empty sequence — element TypeCode is preserved.
        any_rt(AnyValue::Seq {
            element: TypeCode::Double,
            items: vec![],
        });
    }

    #[test]
    fn corba_any_struct_mixed_members() {
        any_rt(AnyValue::Struct {
            repo_id: "IDL:Point:1.0".to_string(),
            name: "Point".to_string(),
            members: vec![
                ("x".to_string(), AnyValue::Long(10)),
                ("y".to_string(), AnyValue::Long(-20)),
                ("label".to_string(), AnyValue::Str("p1".to_string())),
                ("active".to_string(), AnyValue::Boolean(true)),
            ],
        });
    }

    #[test]
    fn corba_any_enum_and_nested_any_and_seq_of_struct() {
        any_rt(AnyValue::Enum {
            repo_id: "IDL:Color:1.0".to_string(),
            name: "Color".to_string(),
            value: 2,
            members: vec!["RED".to_string(), "GREEN".to_string(), "BLUE".to_string()],
        });
        // any-in-any.
        any_rt(AnyValue::Any(Box::new(CorbaAny(AnyValue::Double(2.5)))));
        // sequence<struct> — complex element, nested encaps + values.
        let mk = |x: i32| AnyValue::Struct {
            repo_id: "IDL:Pair:1.0".to_string(),
            name: "Pair".to_string(),
            members: vec![
                ("k".to_string(), AnyValue::Long(x)),
                ("v".to_string(), AnyValue::Str(alloc::format!("v{x}"))),
            ],
        };
        any_rt(AnyValue::Seq {
            element: mk(0).type_code(),
            items: vec![mk(1), mk(2)],
        });
    }

    #[test]
    fn corba_any_wstring_roundtrip() {
        let any = CorbaAny(AnyValue::WStr(WString::from("wíde€")));
        let mut w = BufferWriter::new(Endianness::Big);
        any.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(CorbaAny::decode(&mut r).unwrap(), any);
    }

    #[test]
    fn wstring_empty() {
        let ws = WString::from("");
        let mut w = BufferWriter::new(Endianness::Big);
        ws.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(
            bytes,
            vec![0, 0, 0, 0],
            "empty wstring = length 0, no bytes"
        );
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(WString::decode(&mut r).unwrap(), ws);
    }

    fn rt_le<T>(value: T)
    where
        T: CdrEncode + CdrDecode + PartialEq + core::fmt::Debug,
    {
        let mut w = BufferWriter::new(Endianness::Little);
        value.encode(&mut w).expect("encode");
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = T::decode(&mut r).expect("decode");
        assert_eq!(decoded, value);
        assert_eq!(r.remaining(), 0);
    }

    // ---- String ----

    #[test]
    fn string_roundtrip_ascii() {
        rt_le(String::from("hello"));
    }

    #[test]
    fn string_roundtrip_unicode() {
        rt_le(String::from("Hällo, 🌍 Welt"));
    }

    #[test]
    fn string_roundtrip_empty() {
        rt_le(String::new());
    }

    #[test]
    fn string_wire_format_includes_null_terminator() {
        let mut w = BufferWriter::new(Endianness::Little);
        "ab".encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        // u32 len = 3 (ab + null) | 'a' 'b' null
        assert_eq!(bytes, vec![3, 0, 0, 0, b'a', b'b', 0]);
    }

    #[test]
    fn string_decode_rejects_zero_length() {
        let bytes = [0u8, 0, 0, 0]; // u32 len = 0 — no null terminator present
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = String::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    #[test]
    fn string_decode_rejects_announced_overrun() {
        let bytes = [100u8, 0, 0, 0, b'x'];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = String::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    #[test]
    fn string_decode_rejects_missing_null_terminator() {
        // Length 3 (a, b, x) — last byte is 'x' instead of 0.
        let bytes = [3u8, 0, 0, 0, b'a', b'b', b'x'];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = String::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::InvalidUtf8 { .. })));
    }

    // ---- Sequence (Vec<T>) ----

    #[test]
    fn sequence_u8_roundtrip() {
        rt_le::<Vec<u8>>(vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn sequence_u32_roundtrip() {
        rt_le::<Vec<u32>>(vec![0xDEAD, 0xBEEF, 0x1234]);
    }

    #[test]
    fn sequence_empty_roundtrip() {
        rt_le::<Vec<u32>>(vec![]);
    }

    #[test]
    fn sequence_string_roundtrip() {
        rt_le::<Vec<String>>(vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn sequence_decode_rejects_overrun_length() {
        // Length 999 in 4 bytes Vec<u8>
        let bytes = [0xE7u8, 0x03, 0, 0, b'x']; // 999 announced, 1 byte data
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = Vec::<u8>::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::LengthExceeded { .. })));
    }

    #[test]
    fn sequence_alignment_4_byte_prefix() {
        // u8 + Vec<u8> → u8 + 3 pad + u32 len + bytes
        let mut w = BufferWriter::new(Endianness::Little);
        1u8.encode(&mut w).unwrap();
        vec![10u8, 20, 30].encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes[0], 1); // u8
        assert_eq!(&bytes[1..4], &[0, 0, 0]); // padding
        assert_eq!(&bytes[4..8], &[3, 0, 0, 0]); // u32 length
        assert_eq!(&bytes[8..11], &[10, 20, 30]); // payload
    }

    // ---- Array ----

    #[test]
    fn array_u8_roundtrip() {
        rt_le::<[u8; 4]>([1, 2, 3, 4]);
    }

    #[test]
    fn array_u32_roundtrip() {
        rt_le::<[u32; 3]>([100, 200, 300]);
    }

    #[test]
    fn array_no_length_prefix() {
        let mut w = BufferWriter::new(Endianness::Little);
        [1u8, 2, 3].encode(&mut w).unwrap();
        // No u32 length — only elements.
        assert_eq!(w.into_bytes(), vec![1, 2, 3]);
    }

    #[test]
    fn array_zero_size() {
        let arr: [u32; 0] = [];
        let mut w = BufferWriter::new(Endianness::Little);
        arr.encode(&mut w).unwrap();
        assert!(w.into_bytes().is_empty());
    }

    // ---- Optional ----

    #[test]
    fn optional_none_roundtrip() {
        rt_le::<Option<u32>>(None);
    }

    #[test]
    fn optional_some_roundtrip() {
        rt_le::<Option<u32>>(Some(42));
    }

    #[test]
    fn optional_some_string_roundtrip() {
        rt_le::<Option<String>>(Some("hi".to_string()));
    }

    #[test]
    fn optional_wire_format_none_is_zero_byte() {
        let mut w = BufferWriter::new(Endianness::Little);
        Option::<u32>::None.encode(&mut w).unwrap();
        assert_eq!(w.into_bytes(), vec![0]);
    }

    #[test]
    fn optional_wire_format_some_is_one_then_value() {
        let mut w = BufferWriter::new(Endianness::Little);
        Some(0xABCDu32).encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes[0], 1); // present-flag
        // 3 byte padding + 4 byte u32
        assert_eq!(&bytes[1..4], &[0, 0, 0]);
        assert_eq!(&bytes[4..8], &[0xCD, 0xAB, 0, 0]);
    }

    #[test]
    fn optional_decode_rejects_invalid_flag() {
        let bytes = [0xFFu8];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = Option::<u32>::decode(&mut r);
        assert!(matches!(res, Err(DecodeError::InvalidBool { .. })));
    }

    // ---- Mixed nested ----

    #[test]
    fn nested_optional_sequence_string() {
        let value: Option<Vec<String>> = Some(vec!["a".to_string(), "bb".to_string()]);
        rt_le(value);
    }

    #[test]
    fn nested_array_of_optionals() {
        let value: [Option<u32>; 3] = [Some(1), None, Some(3)];
        rt_le(value);
    }
}
