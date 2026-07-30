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
use crate::endianness::Endianness;
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
    // Backpatched DHEADER: placeholder, body straight into `writer`, patch the
    // length — no throwaway sub-writer Vec and no copy of its bytes back. Same
    // alignment argument as `struct_enc::encode_appendable`: the body aligns
    // relative to its own start and XCDR2 caps alignment at 4, so this is
    // byte-identical whenever `body_start` is aligned (always, on the XCDR2 path
    // that is the only one to use a DHEADER). Fall back to the sub-writer
    // otherwise so the bytes are always correct.
    writer.write_u32(0)?;
    let body_start = writer.position();
    if body_start % writer.max_alignment() == 0 {
        body(writer)?;
        let dheader = u32::try_from(writer.position() - body_start).map_err(|_| {
            EncodeError::ValueOutOfRange {
                message: "collection DHEADER exceeds u32::MAX",
            }
        })?;
        writer.patch_u32_at(body_start - 4, dheader)
    } else {
        let mut sub =
            BufferWriter::new(writer.endianness()).with_max_alignment(writer.max_alignment());
        body(&mut sub)?;
        let bytes = sub.into_bytes();
        let dheader = u32::try_from(bytes.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "collection DHEADER exceeds u32::MAX",
        })?;
        writer.patch_u32_at(body_start - 4, dheader)?;
        writer.write_bytes(&bytes)
    }
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

    /// Encodes the GIOP 1.2 wstring with an **explicit** BOM choice,
    /// bypassing the global [`corba_wstring_bom`] policy. `with_bom = true`
    /// = omniORB/TAO form (a `0xFEFF` byte-order mark prepended, counted in
    /// the octet length); `false` = JacORB form (no BOM, units in message
    /// byte order). An empty wstring is length 0 either way.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the octet length exceeds `u32::MAX`.
    pub fn encode_with_bom(
        &self,
        writer: &mut BufferWriter,
        with_bom: bool,
    ) -> Result<(), EncodeError> {
        let units = self.0.encode_utf16().count();
        if units == 0 {
            writer.write_u32(0)?;
            return Ok(());
        }
        let total_units = if with_bom {
            units.saturating_add(1)
        } else {
            units
        };
        let octets = u32::try_from(total_units.saturating_mul(2)).map_err(|_| {
            EncodeError::ValueOutOfRange {
                message: "CDR wstring length exceeds u32::MAX",
            }
        })?;
        writer.write_u32(octets)?;
        // write_u16 respects endianness; align(2) is a no-op here, since the
        // position after the uint32 is 4-aligned (and thus 2-aligned).
        if with_bom {
            writer.write_u16(UTF16_BOM)?;
        }
        for unit in self.0.encode_utf16() {
            writer.write_u16(unit)?;
        }
        Ok(())
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

/// Process-global policy: does the GIOP `wstring` encoder prepend a UTF-16
/// byte-order mark? `true` (default) = omniORB/TAO form (BOM, counted in the
/// octet length); `false` = JacORB form (no BOM, units in message byte
/// order). Underspecified by §15.3.1.6 (the BOM is optional) and real ORBs
/// disagree, so this is a configuration switch rather than a fixed choice.
///
/// `AtomicBool` (not a thread-local) because `zerodds-cdr` is `no_std`; the
/// wstring wire form is a deployment-wide interop setting, so a single
/// process-global is the right granularity. The **decoder** always accepts
/// both forms (see [`WString::decode`]), so this only affects what ZeroDDS
/// *emits* — set it to match the peer ORB.
static CORBA_WSTRING_BOM: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(true);

/// Sets the GIOP `wstring` BOM policy (see [`CORBA_WSTRING_BOM`]). Call once
/// at startup to match the peer ORB: leave the default (`true`) for
/// omniORB/TAO, pass `false` for JacORB-style (no BOM).
pub fn set_corba_wstring_bom(with_bom: bool) {
    CORBA_WSTRING_BOM.store(with_bom, core::sync::atomic::Ordering::Relaxed);
}

/// Current GIOP `wstring` BOM policy (default `true`).
#[must_use]
pub fn corba_wstring_bom() -> bool {
    CORBA_WSTRING_BOM.load(core::sync::atomic::Ordering::Relaxed)
}

impl CdrEncode for WString {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        // GIOP 1.2 wstring (§15.3.2.7): uint32 length **in octets**, then the
        // UTF-16 code units, no null terminator. Per §15.3.1.6 a byte-order
        // mark (0xFEFF) MAY be prepended; whether it is is governed by the
        // process-global [`corba_wstring_bom`] policy (default `true` =
        // omniORB/TAO; set `false` for JacORB).
        self.encode_with_bom(writer, corba_wstring_bom())
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
        // §15.3.1.6: a leading BOM, if present, fixes the units' byte order;
        // if absent, the message byte order applies (so a no-BOM wstring in a
        // LE message reads LE, and JacORB's no-BOM units in a BE message read
        // BE). This reads both omniORB/TAO (BOM) and JacORB (no BOM) and is
        // self-consistent with the no-BOM form `encode_with_bom` emits.
        let msg_big_endian = matches!(reader.endianness(), Endianness::Big);
        let bytes = reader.read_bytes(octets)?;
        let (start, big_endian) = match (bytes[0], bytes[1]) {
            (0xFE, 0xFF) => (2, true),  // BOM big-endian
            (0xFF, 0xFE) => (2, false), // BOM little-endian
            _ => (0, msg_big_endian),   // no BOM -> message byte order
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
// WChar — IDL `wchar` (CORBA-GIOP-1.2-Wire, §15.3.1.6)
// ============================================================================

/// IDL `wchar` wrapper for the **CORBA/GIOP 1.2** wire form. Holds a single
/// Unicode scalar (`char`), but unlike the DDS XCDR2 `wchar` (a bare 2-byte
/// UTF-16 code unit, no prefix) the GIOP 1.2 form is a **1-octet length**
/// (the octet count of the wchar in the transmission code set — 2 for a BMP
/// character in UTF-16) followed by the UTF-16 code unit(s).
///
/// **Byte order:** the code units are emitted **big-endian** regardless of the
/// message byte order. This is the consensus oracle-confirmed behaviour of
/// both omniORB 4.3 (writes `02 00 41` for `'A'` even inside a little-endian
/// encapsulation) and JacORB 3.9 (`02 00 41`, big-endian message). The default
/// UTF-16 transmission wide code set has no per-`wchar` BOM, so network
/// (big-endian) order is used. `wchar 'A'` (U+0041) = `02 0041`.
///
/// A CORBA `wchar` is a single transmission unit: real ORBs (omniORB
/// `CORBA::WChar`, Java `char`) are 16-bit, so only BMP characters occur
/// (length 2, one unit). A non-BMP scalar would need a UTF-16 surrogate pair
/// (length 4, two units); the encoder handles that defensively even though the
/// ORBs cannot produce it through `write_wchar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
pub struct WChar(pub char);

impl WChar {
    /// The inner Unicode scalar.
    #[must_use]
    pub fn as_char(&self) -> char {
        self.0
    }
}

impl From<char> for WChar {
    fn from(c: char) -> Self {
        Self(c)
    }
}

impl CdrEncode for WChar {
    fn encode(&self, writer: &mut BufferWriter) -> Result<(), EncodeError> {
        // GIOP 1.2 wchar (§15.3.1.6): a 1-octet length (octet count in the
        // transmission code set) followed by the UTF-16 code unit(s) in
        // BIG-ENDIAN (network) order — independent of the message byte order
        // (oracle-confirmed against omniORB 4.3 + JacORB 3.9).
        let mut buf = [0u16; 2];
        let units = self.0.encode_utf16(&mut buf);
        let octets = units.len().saturating_mul(2);
        let octets = u8::try_from(octets).map_err(|_| EncodeError::ValueOutOfRange {
            message: "CDR wchar octet length exceeds u8::MAX",
        })?;
        writer.write_u8(octets)?;
        for unit in units {
            writer.write_bytes(&unit.to_be_bytes())?;
        }
        Ok(())
    }
}

impl CdrDecode for WChar {
    fn decode(reader: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let offset = reader.position();
        let octets = reader.read_u8()? as usize;
        if octets == 0 || octets % 2 != 0 || octets > 4 || octets > reader.remaining() {
            return Err(DecodeError::LengthExceeded {
                announced: octets,
                remaining: reader.remaining(),
                offset,
            });
        }
        let bytes = reader.read_bytes(octets)?;
        let mut units = [0u16; 2];
        let mut count = 0usize;
        let mut idx = 0usize;
        while idx + 1 < octets {
            units[count] = u16::from_be_bytes([bytes[idx], bytes[idx + 1]]);
            count += 1;
            idx += 2;
        }
        // One scalar from the (1 or 2) UTF-16 units; a lone surrogate or an
        // invalid pair is rejected.
        let mut iter = char::decode_utf16(units[..count].iter().copied());
        let c = iter
            .next()
            .and_then(Result::ok)
            .filter(|_| iter.next().is_none())
            .ok_or(DecodeError::InvalidUtf8 { offset })?;
        Ok(Self(c))
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
    // XTypes 1.3 §7.4.3.5 rule (8) PARRAY_TYPE: an array of PRIMITIVE elements
    // carries NO collection DHEADER, regardless of dimensionality. Propagating
    // `T::IS_PRIMITIVE` makes a multi-dim primitive array (`[[i32; 3]; 2]`,
    // T = `[i32; 3]`) report primitive → no DHEADER, while an array of structs
    // (`[Pt; 2]`) stays non-primitive → DHEADER (rule (9) ARRAY_TYPE).
    const IS_PRIMITIVE: bool = T::IS_PRIMITIVE;

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
    // FINDING (4th, decode/encode asymmetry): must mirror `CdrEncode for
    // [T; N]`'s `IS_PRIMITIVE = T::IS_PRIMITIVE` (line ~692). Without it a
    // multi-dim PRIMITIVE array (`[[i32; 3]; 2]`, T = `[i32; 3]`) decoded as a
    // `[T; N]` element of an OUTER array reported the trait-default `false`,
    // so the outer decode read a phantom collection DHEADER that the encoder
    // never wrote (it correctly saw the inner array as primitive) → desync /
    // `UnexpectedEof`. Propagating it keeps the DHEADER decision symmetric.
    const IS_PRIMITIVE: bool = T::IS_PRIMITIVE;

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
        // XCDR2 §7.4.3.5: a map carries a collection DHEADER only when its
        // key/value *pair* is non-primitive (a struct/string value), exactly
        // like a sequence keys on its element type. A `map<primitive,primitive>`
        // is fully descriptive and gets NO DHEADER. Verified byte-identical to
        // Fast DDS + OpenDDS (which both omit it for primitive maps and emit it
        // for map-of-struct). The pair is primitive iff both K and V are.
        if needs_collection_dheader(w.max_alignment(), K::IS_PRIMITIVE && V::IS_PRIMITIVE) {
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
        // Symmetric to encode: only a non-primitive-pair map carries a DHEADER.
        if needs_collection_dheader(r.max_alignment(), K::IS_PRIMITIVE && V::IS_PRIMITIVE) {
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
    fn map_primitive_pair_omits_dheader_xcdr2() {
        // map<i32,i32> = {7:42}: key+value both primitive -> the map is fully
        // descriptive, so NO collection DHEADER under XCDR2. Byte-identical to
        // Fast DDS + OpenDDS (both omit it). Regression for the over-emitted
        // DHEADER that the map-of-struct golden (which keeps a DHEADER) hid.
        let mut m = BTreeMap::new();
        m.insert(7i32, 42i32);
        let mut w = BufferWriter::new(Endianness::Little).with_max_alignment(XCDR2_MAX_ALIGNMENT);
        m.encode(&mut w).unwrap();
        assert_eq!(
            w.into_bytes(),
            vec![1, 0, 0, 0, 7, 0, 0, 0, 42, 0, 0, 0],
            "primitive map must be count+key+value with NO leading DHEADER"
        );
        // and it round-trips
        let bytes = vec![1u8, 0, 0, 0, 7, 0, 0, 0, 42, 0, 0, 0];
        let mut r =
            BufferReader::new(&bytes, Endianness::Little).with_max_alignment(XCDR2_MAX_ALIGNMENT);
        let back: BTreeMap<i32, i32> = BTreeMap::decode(&mut r).unwrap();
        assert_eq!(back.get(&7), Some(&42));
    }

    #[test]
    fn map_non_primitive_value_keeps_dheader_xcdr2() {
        // map<i32,String>: value is non-primitive -> the map DOES carry a
        // DHEADER (same rule as a sequence-of-non-primitive, and as the
        // byte-anchored map-of-struct in the corpus). The first word is the
        // DHEADER (byte length), not the entry count.
        let mut m = BTreeMap::new();
        m.insert(1i32, "x".to_string());
        let mut w = BufferWriter::new(Endianness::Little).with_max_alignment(XCDR2_MAX_ALIGNMENT);
        m.encode(&mut w).unwrap();
        let b = w.into_bytes();
        assert_ne!(
            &b[0..4],
            &[1, 0, 0, 0],
            "non-primitive map must lead with a DHEADER"
        );
        let mut r =
            BufferReader::new(&b, Endianness::Little).with_max_alignment(XCDR2_MAX_ALIGNMENT);
        let back: BTreeMap<i32, alloc::string::String> = BTreeMap::decode(&mut r).unwrap();
        assert_eq!(back.get(&1).map(|s| s.as_str()), Some("x"));
    }

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
    fn wchar_giop12_wire_format_pinned_bytes() {
        // wchar 'A' (U+0041) = 1-octet length (2) + UTF-16 unit big-endian.
        // Pinned native bytes per omniORB 4.3 + JacORB 3.9: 02 0041.
        let wc = WChar('A');
        let mut w = BufferWriter::new(Endianness::Big);
        wc.encode(&mut w).unwrap();
        assert_eq!(w.into_bytes(), vec![0x02, 0x00, 0x41]);
    }

    #[test]
    fn wchar_units_are_big_endian_even_in_le_message() {
        // The UTF-16 unit is ALWAYS big-endian, independent of message order
        // (omniORB emits 02 00 41 inside a little-endian encapsulation).
        let wc = WChar('A');
        let mut w = BufferWriter::new(Endianness::Little);
        wc.encode(&mut w).unwrap();
        assert_eq!(w.into_bytes(), vec![0x02, 0x00, 0x41]);
    }

    #[test]
    fn wchar_non_ascii_bmp() {
        // 'ü' U+00FC -> 02 00fc ; '€' U+20AC -> 02 20ac (oracle-confirmed).
        for (c, want) in [('ü', vec![0x02, 0x00, 0xFC]), ('€', vec![0x02, 0x20, 0xAC])] {
            let mut w = BufferWriter::new(Endianness::Big);
            WChar(c).encode(&mut w).unwrap();
            assert_eq!(w.into_bytes(), want, "wchar {c:?}");
        }
    }

    #[test]
    fn wchar_roundtrip_both_orders() {
        for c in ['A', 'ü', '€', '\u{4E2D}'] {
            for e in [Endianness::Big, Endianness::Little] {
                let wc = WChar(c);
                let mut w = BufferWriter::new(e);
                wc.encode(&mut w).unwrap();
                let bytes = w.into_bytes();
                let mut r = BufferReader::new(&bytes, e);
                assert_eq!(WChar::decode(&mut r).unwrap(), wc, "{c:?}/{e:?}");
                assert_eq!(r.remaining(), 0);
            }
        }
    }

    #[test]
    fn wchar_astral_surrogate_pair_length_4() {
        // A non-BMP scalar (U+1F310 🌐) needs a UTF-16 surrogate pair: the
        // length octet is 4 and two big-endian units follow. Real ORBs cannot
        // produce this via write_wchar (16-bit WChar), but the codec handles
        // it self-consistently.
        let wc = WChar('\u{1F310}');
        let mut w = BufferWriter::new(Endianness::Big);
        wc.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes[0], 0x04, "length octet = 4 (two units)");
        assert_eq!(&bytes[1..], &[0xD8, 0x3C, 0xDF, 0x10]); // UTF-16BE surrogate pair
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(WChar::decode(&mut r).unwrap(), wc);
    }

    #[test]
    fn wchar_decode_rejects_odd_or_oversized_length() {
        // odd length
        let mut r = BufferReader::new(&[0x01, 0x00], Endianness::Big);
        assert!(matches!(
            WChar::decode(&mut r),
            Err(DecodeError::LengthExceeded { .. })
        ));
        // zero length
        let mut r = BufferReader::new(&[0x00], Endianness::Big);
        assert!(matches!(
            WChar::decode(&mut r),
            Err(DecodeError::LengthExceeded { .. })
        ));
        // lone high surrogate -> invalid scalar
        let mut r = BufferReader::new(&[0x02, 0xD8, 0x3C], Endianness::Big);
        assert!(matches!(
            WChar::decode(&mut r),
            Err(DecodeError::InvalidUtf8 { .. })
        ));
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

    #[test]
    fn wstring_bom_policy_default_is_omniorb_form() {
        // Default policy = BOM present (omniORB/TAO). Read-only check — does
        // NOT flip the global, so it cannot race the byte-asserting tests.
        assert!(corba_wstring_bom(), "default GIOP wstring policy is BOM-on");
    }

    #[test]
    fn wstring_encode_with_bom_omniorb_form() {
        // BOM form: octet length counts the BOM (units+1)*2; the 2 bytes
        // after the uint32 length are the 0xFEFF mark in message byte order.
        let ws = WString::from("AB");
        let mut w = BufferWriter::new(Endianness::Little);
        ws.encode_with_bom(&mut w, true).unwrap();
        let bytes = w.into_bytes();
        // octets = (2 units + BOM) * 2 = 6; LE length word then BOM 0xFEFF.
        assert_eq!(&bytes[0..4], &[6, 0, 0, 0]);
        assert_eq!(&bytes[4..6], &[0xFF, 0xFE]); // 0xFEFF little-endian
        assert_eq!(bytes.len(), 4 + 6);
        // Decoder (BOM-tolerant) round-trips it.
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(WString::decode(&mut r).unwrap(), ws);
    }

    #[test]
    fn wstring_encode_with_bom_jacorb_form() {
        // No-BOM form (JacORB): octet length = units*2, no 0xFEFF prefix.
        let ws = WString::from("AB");
        let mut w = BufferWriter::new(Endianness::Little);
        ws.encode_with_bom(&mut w, false).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[0..4], &[4, 0, 0, 0]); // octets = 2*2 = 4
        assert_eq!(&bytes[4..8], &[0x41, 0x00, 0x42, 0x00]); // 'A','B' LE, no BOM
        assert_eq!(bytes.len(), 4 + 4);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(WString::decode(&mut r).unwrap(), ws);
    }

    #[test]
    fn wstring_empty_both_bom_policies_are_length_zero() {
        let ws = WString::from("");
        for with_bom in [true, false] {
            let mut w = BufferWriter::new(Endianness::Little);
            ws.encode_with_bom(&mut w, with_bom).unwrap();
            assert_eq!(w.into_bytes(), vec![0, 0, 0, 0]);
        }
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
