// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA `TypeCode` + its CDR wire form (§15.3.5) — the self-describing part
//! of an `any`. A `TypeCode` is a `TCKind` (`unsigned long`) plus parameters:
//!
//! * **Empty-param kinds** (null/void/short/long/…/boolean/char/octet/wchar/
//!   any/TypeCode/longlong/…): just the TCKind.
//! * **Simple-param** (`tk_string`/`tk_wstring`): TCKind + `bound` (ulong),
//!   inline (NOT encapsulated).
//! * **Complex-param** (`tk_objref`/`tk_struct`/`tk_enum`/`tk_sequence`/
//!   `tk_array`/`tk_alias`/`tk_except`/`tk_union`): TCKind + a CDR
//!   encapsulation (`sequence<octet>` = length + byte-order octet +
//!   parameters, alignment relative to the byte-order octet).
//!
//! Indirection (TCKind `0xffffffff` + negative offset, §15.3.5.1) is
//! **resolved** during decoding: a position cache `(stream offset → TypeCode)`
//! makes it possible to clone *repeated* TypeCodes and to represent
//! *recursive* ones (e.g. `struct Node { sequence<Node> kids; }`) as a
//! [`TypeCode::Recursive`] marker. Encode-side indirection (offset
//! backpatching) is a separate feature; for cross-ORB interop, decode
//! acceptance is sufficient.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::buffer::{BufferReader, BufferWriter};
use crate::endianness::Endianness;
use crate::error::{DecodeError, EncodeError};

/// `TCKind` discriminators (§15.3.5.1, OMG wire values). The constant names
/// map 1:1 to the OMG `tk_*` identifiers.
#[allow(missing_docs)]
pub mod tckind {
    pub const TK_NULL: u32 = 0;
    pub const TK_VOID: u32 = 1;
    pub const TK_SHORT: u32 = 2;
    pub const TK_LONG: u32 = 3;
    pub const TK_USHORT: u32 = 4;
    pub const TK_ULONG: u32 = 5;
    pub const TK_FLOAT: u32 = 6;
    pub const TK_DOUBLE: u32 = 7;
    pub const TK_BOOLEAN: u32 = 8;
    pub const TK_CHAR: u32 = 9;
    pub const TK_OCTET: u32 = 10;
    pub const TK_ANY: u32 = 11;
    pub const TK_TYPECODE: u32 = 12;
    pub const TK_OBJREF: u32 = 14;
    pub const TK_STRUCT: u32 = 15;
    pub const TK_ENUM: u32 = 17;
    pub const TK_STRING: u32 = 18;
    pub const TK_SEQUENCE: u32 = 19;
    pub const TK_ALIAS: u32 = 21;
    pub const TK_EXCEPT: u32 = 22;
    pub const TK_LONGLONG: u32 = 23;
    pub const TK_ULONGLONG: u32 = 24;
    pub const TK_WCHAR: u32 = 26;
    pub const TK_WSTRING: u32 = 27;
    pub const INDIRECTION: u32 = 0xffff_ffff;
}

/// CORBA `TypeCode` (subset: all scalar kinds + string/wstring + sequence +
/// struct + enum + alias + objref — the common structured `any` contents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeCode {
    /// `tk_null`.
    Null,
    /// `tk_void`.
    Void,
    /// `tk_short`.
    Short,
    /// `tk_long`.
    Long,
    /// `tk_ushort`.
    UShort,
    /// `tk_ulong`.
    ULong,
    /// `tk_longlong`.
    LongLong,
    /// `tk_ulonglong`.
    ULongLong,
    /// `tk_float`.
    Float,
    /// `tk_double`.
    Double,
    /// `tk_boolean`.
    Boolean,
    /// `tk_char`.
    Char,
    /// `tk_octet`.
    Octet,
    /// `tk_wchar`.
    WChar,
    /// `tk_any`.
    Any,
    /// `tk_TypeCode`.
    TypeCodeTc,
    /// `tk_string` with `bound` (0 = unbounded).
    String(u32),
    /// `tk_wstring` with `bound`.
    WString(u32),
    /// `tk_sequence`: element type + `bound`.
    Sequence {
        /// Element TypeCode.
        element: Box<TypeCode>,
        /// Bound (0 = unbounded).
        bound: u32,
    },
    /// `tk_struct` (and `tk_except`): RepositoryId, name, ordered members.
    Struct {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// Struct name.
        name: String,
        /// `(member_name, member_type)` in declaration order.
        members: Vec<(String, TypeCode)>,
        /// `true` ⇒ `tk_except` instead of `tk_struct` (same wire form).
        is_except: bool,
    },
    /// `tk_enum`: RepositoryId, name, enumerator names (index = value).
    Enum {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// Enum name.
        name: String,
        /// Enumerator names.
        members: Vec<String>,
    },
    /// `tk_alias` (typedef): RepositoryId, name, resolved content.
    Alias {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// Alias name.
        name: String,
        /// Resolved content TypeCode.
        content: Box<TypeCode>,
    },
    /// `tk_objref`: interface RepositoryId + name.
    ObjRef {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// Interface name.
        name: String,
    },
    /// Recursive reference (§15.3.5.1 indirection): points via `repo_id` to an
    /// enclosing TypeCode that is still being decoded (e.g.
    /// `struct Node { sequence<Node> kids; }`). Decode-only marker — breaks the
    /// otherwise infinite type graph.
    Recursive {
        /// RepositoryId of the recursively referenced type.
        repo_id: String,
    },
}

impl TypeCode {
    /// The `TCKind` wire value.
    #[must_use]
    pub const fn tckind(&self) -> u32 {
        use tckind::*;
        match self {
            Self::Null => TK_NULL,
            Self::Void => TK_VOID,
            Self::Short => TK_SHORT,
            Self::Long => TK_LONG,
            Self::UShort => TK_USHORT,
            Self::ULong => TK_ULONG,
            Self::LongLong => TK_LONGLONG,
            Self::ULongLong => TK_ULONGLONG,
            Self::Float => TK_FLOAT,
            Self::Double => TK_DOUBLE,
            Self::Boolean => TK_BOOLEAN,
            Self::Char => TK_CHAR,
            Self::Octet => TK_OCTET,
            Self::WChar => TK_WCHAR,
            Self::Any => TK_ANY,
            Self::TypeCodeTc => TK_TYPECODE,
            Self::String(_) => TK_STRING,
            Self::WString(_) => TK_WSTRING,
            Self::Sequence { .. } => TK_SEQUENCE,
            Self::Struct {
                is_except: true, ..
            } => TK_EXCEPT,
            Self::Struct { .. } => TK_STRUCT,
            Self::Enum { .. } => TK_ENUM,
            Self::Alias { .. } => TK_ALIAS,
            Self::ObjRef { .. } => TK_OBJREF,
            // Decode-only marker; the wire carries the indirection instead.
            Self::Recursive { .. } => INDIRECTION,
        }
    }

    /// Encodes the TypeCode (§15.3.5) into the buffer.
    ///
    /// # Errors
    /// Buffer write error or length overflow.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.tckind())?;
        match self {
            // Empty-param kinds: just the TCKind.
            Self::Null
            | Self::Void
            | Self::Short
            | Self::Long
            | Self::UShort
            | Self::ULong
            | Self::LongLong
            | Self::ULongLong
            | Self::Float
            | Self::Double
            | Self::Boolean
            | Self::Char
            | Self::Octet
            | Self::WChar
            | Self::Any
            | Self::TypeCodeTc => Ok(()),
            // Simple-param: bound inline.
            Self::String(b) | Self::WString(b) => w.write_u32(*b),
            // Complex-param: encapsulation.
            Self::Sequence { element, bound } => {
                let encap = build_encap(w.endianness(), |e| {
                    element.encode(e)?;
                    e.write_u32(*bound)
                })?;
                write_encap(w, &encap)
            }
            Self::Struct {
                repo_id,
                name,
                members,
                ..
            } => {
                let encap = build_encap(w.endianness(), |e| {
                    e.write_string(repo_id)?;
                    e.write_string(name)?;
                    e.write_u32(u32::try_from(members.len()).map_err(|_| {
                        EncodeError::ValueOutOfRange {
                            message: "TypeCode struct member count exceeds u32",
                        }
                    })?)?;
                    for (mn, mt) in members {
                        e.write_string(mn)?;
                        mt.encode(e)?;
                    }
                    Ok(())
                })?;
                write_encap(w, &encap)
            }
            Self::Enum {
                repo_id,
                name,
                members,
            } => {
                let encap = build_encap(w.endianness(), |e| {
                    e.write_string(repo_id)?;
                    e.write_string(name)?;
                    e.write_u32(u32::try_from(members.len()).map_err(|_| {
                        EncodeError::ValueOutOfRange {
                            message: "TypeCode enum member count exceeds u32",
                        }
                    })?)?;
                    for mn in members {
                        e.write_string(mn)?;
                    }
                    Ok(())
                })?;
                write_encap(w, &encap)
            }
            Self::Alias {
                repo_id,
                name,
                content,
            } => {
                let encap = build_encap(w.endianness(), |e| {
                    e.write_string(repo_id)?;
                    e.write_string(name)?;
                    content.encode(e)
                })?;
                write_encap(w, &encap)
            }
            Self::ObjRef { repo_id, name } => {
                let encap = build_encap(w.endianness(), |e| {
                    e.write_string(repo_id)?;
                    e.write_string(name)
                })?;
                write_encap(w, &encap)
            }
            // Encode-side indirection (a backpatch offset table) is a
            // separate feature; decode acceptance is enough for cross-ORB interop.
            Self::Recursive { .. } => Err(EncodeError::ValueOutOfRange {
                message: "TypeCode::Recursive encode (indirection emit) not yet supported",
            }),
        }
    }

    /// Decodes a TypeCode (§15.3.5) including **indirection** (§15.3.5.1):
    /// recursive and repeated TypeCodes (`0xffffffff` + negative offset)
    /// are resolved — repeated ones cloned via a position cache, recursive
    /// ones as a [`TypeCode::Recursive`] marker.
    ///
    /// # Errors
    /// `InvalidEnum` on an unknown TCKind / unresolvable indirection;
    /// buffer read error.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let mut cache = TcCache::new();
        // base 0: cache positions are reader-absolute (`r.position()`).
        Self::decode_ctx(r, 0, &mut cache)
    }

    /// Position-tracking decode. `base` = absolute stream offset of the
    /// reader's byte 0; `cache` maps `(base + position)` of a TCKind to the
    /// TypeCode decoded there — a precondition for resolving indirections
    /// (which point backward by byte offset).
    fn decode_ctx(
        r: &mut BufferReader<'_>,
        base: usize,
        cache: &mut TcCache,
    ) -> Result<Self, DecodeError> {
        use tckind::*;
        // TCKind is a 4-aligned `unsigned long`. Apply the alignment BEFORE
        // recording the position (read_u32 would otherwise do it internally
        // first) — otherwise the cache position points at the padding instead
        // of the TCKind, and indirections to nested TypeCodes fail to resolve.
        r.align(4)?;
        let tckind_pos = base + r.position();
        let kind = r.read_u32()?;
        match kind {
            TK_NULL => Ok(Self::Null),
            TK_VOID => Ok(Self::Void),
            TK_SHORT => Ok(Self::Short),
            TK_LONG => Ok(Self::Long),
            TK_USHORT => Ok(Self::UShort),
            TK_ULONG => Ok(Self::ULong),
            TK_LONGLONG => Ok(Self::LongLong),
            TK_ULONGLONG => Ok(Self::ULongLong),
            TK_FLOAT => Ok(Self::Float),
            TK_DOUBLE => Ok(Self::Double),
            TK_BOOLEAN => Ok(Self::Boolean),
            TK_CHAR => Ok(Self::Char),
            TK_OCTET => Ok(Self::Octet),
            TK_WCHAR => Ok(Self::WChar),
            TK_ANY => Ok(Self::Any),
            TK_TYPECODE => Ok(Self::TypeCodeTc),
            TK_STRING => Ok(Self::String(r.read_u32()?)),
            TK_WSTRING => Ok(Self::WString(r.read_u32()?)),
            INDIRECTION => {
                // 0xffffffff followed by a signed long: offset relative to the
                // position of the offset field, backward to the target TCKind (§15.3.5.1).
                let off_field_pos = base + r.position();
                let offset = r.read_u32()? as i32;
                if offset >= 0 {
                    return Err(DecodeError::InvalidEnum {
                        kind: "TypeCode indirection: offset must be negative",
                        value: INDIRECTION,
                    });
                }
                let target =
                    usize::try_from(off_field_pos as i64 + i64::from(offset)).map_err(|_| {
                        DecodeError::InvalidEnum {
                            kind: "TypeCode indirection: target before stream start",
                            value: INDIRECTION,
                        }
                    })?;
                match cache.get(&target) {
                    Some(TcCacheEntry::Done(tc)) => Ok(tc.clone()),
                    Some(TcCacheEntry::InProgress(repo_id)) => Ok(Self::Recursive {
                        repo_id: repo_id.clone(),
                    }),
                    None => Err(DecodeError::InvalidEnum {
                        kind: "TypeCode indirection: unresolved target",
                        value: INDIRECTION,
                    }),
                }
            }
            TK_SEQUENCE => decode_encap_ctx(r, base, cache, |e, cb, cache| {
                let element = Box::new(Self::decode_ctx(e, cb, cache)?);
                let bound = e.read_u32()?;
                let tc = Self::Sequence { element, bound };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            TK_STRUCT | TK_EXCEPT => decode_encap_ctx(r, base, cache, |e, cb, cache| {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                // Placeholder BEFORE the members: a member that indirects
                // recursively back to this struct resolves to Recursive.
                cache.insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let count = e.read_u32()? as usize;
                let mut members = Vec::with_capacity(count.min(256));
                for _ in 0..count {
                    let mn = e.read_string()?;
                    let mt = Self::decode_ctx(e, cb, cache)?;
                    members.push((mn, mt));
                }
                let tc = Self::Struct {
                    repo_id,
                    name,
                    members,
                    is_except: kind == TK_EXCEPT,
                };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            TK_ENUM => decode_encap_ctx(r, base, cache, |e, _cb, cache| {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                let count = e.read_u32()? as usize;
                let mut members = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    members.push(e.read_string()?);
                }
                let tc = Self::Enum {
                    repo_id,
                    name,
                    members,
                };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            TK_ALIAS => decode_encap_ctx(r, base, cache, |e, cb, cache| {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                cache.insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let content = Box::new(Self::decode_ctx(e, cb, cache)?);
                let tc = Self::Alias {
                    repo_id,
                    name,
                    content,
                };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            TK_OBJREF => decode_encap_ctx(r, base, cache, |e, _cb, cache| {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                let tc = Self::ObjRef { repo_id, name };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            other => Err(DecodeError::InvalidEnum {
                kind: "TypeCode TCKind (unsupported)",
                value: other,
            }),
        }
    }
}

/// Cache entry for indirection resolution (§15.3.5.1).
enum TcCacheEntry {
    /// Complex TypeCode still being decoded — an indirection to it is
    /// recursive and yields [`TypeCode::Recursive`].
    InProgress(String),
    /// Fully decoded — an indirection to it is a repeated TypeCode.
    Done(TypeCode),
}

/// Position cache: `(stream offset of the TCKind) → entry`.
type TcCache = alloc::collections::BTreeMap<usize, TcCacheEntry>;

/// Builds a CDR encapsulation: byte-order octet + body (alignment from the
/// byte-order octet — natural alignment in the fresh sub-buffer).
fn build_encap<F>(endianness: Endianness, body: F) -> Result<Vec<u8>, EncodeError>
where
    F: FnOnce(&mut BufferWriter) -> Result<(), EncodeError>,
{
    let mut e = BufferWriter::new(endianness);
    e.write_u8(match endianness {
        Endianness::Big => 0,
        Endianness::Little => 1,
    })?;
    body(&mut e)?;
    Ok(e.into_bytes())
}

/// Writes an encapsulation as `sequence<octet>` (length + bytes).
fn write_encap(w: &mut BufferWriter, encap: &[u8]) -> Result<(), EncodeError> {
    let len = u32::try_from(encap.len()).map_err(|_| EncodeError::ValueOutOfRange {
        message: "TypeCode encapsulation exceeds u32",
    })?;
    w.write_u32(len)?;
    w.write_bytes(encap)
}

/// Reads an encapsulation (`sequence<octet>`) and decodes its body in the order
/// given by the byte-order octet (origin = byte-order octet). Passes the
/// **absolute stream offset** of the encapsulation's byte 0 (`base + position at
/// `read_bytes`) as `child_base` to the body — needed so that indirections
/// inside the encapsulation resolve to TCKinds outside (recursively).
fn decode_encap_ctx<F>(
    r: &mut BufferReader<'_>,
    base: usize,
    cache: &mut TcCache,
    body: F,
) -> Result<TypeCode, DecodeError>
where
    F: FnOnce(&mut BufferReader<'_>, usize, &mut TcCache) -> Result<TypeCode, DecodeError>,
{
    let offset = r.position();
    let len = r.read_u32()? as usize;
    // The stream position from which read_bytes copies = the absolute offset of
    // byte 0 of the sub-buffer.
    let child_base = base + r.position();
    let bytes = r.read_bytes(len)?;
    if bytes.is_empty() {
        return Err(DecodeError::InvalidString {
            offset,
            reason: "empty TypeCode encapsulation",
        });
    }
    let endianness = match bytes[0] {
        0 => Endianness::Big,
        1 => Endianness::Little,
        _ => {
            return Err(DecodeError::InvalidString {
                offset,
                reason: "invalid TypeCode encapsulation byte-order",
            });
        }
    };
    // Reader over the WHOLE encapsulation (origin = byte-order octet); skip the
    // octet, then read the body in its order.
    let mut e = BufferReader::new(bytes, endianness);
    let _bo = e.read_u8()?;
    body(&mut e, child_base, cache)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;

    fn rt(tc: &TypeCode, e: Endianness) {
        let mut w = BufferWriter::new(e);
        tc.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, e);
        assert_eq!(&TypeCode::decode(&mut r).unwrap(), tc, "{tc:?} / {e:?}");
    }

    #[test]
    fn simple_kinds_roundtrip() {
        for tc in [
            TypeCode::Null,
            TypeCode::Long,
            TypeCode::Double,
            TypeCode::Boolean,
            TypeCode::Octet,
            TypeCode::WChar,
            TypeCode::Any,
            TypeCode::String(0),
            TypeCode::String(255),
            TypeCode::WString(64),
        ] {
            rt(&tc, Endianness::Big);
            rt(&tc, Endianness::Little);
        }
    }

    #[test]
    fn sequence_of_long_roundtrip() {
        let tc = TypeCode::Sequence {
            element: Box::new(TypeCode::Long),
            bound: 0,
        };
        rt(&tc, Endianness::Big);
        rt(&tc, Endianness::Little);
    }

    /// Indirection (§15.3.5.1) — **recursive** struct `Node { Node n; }`: one
    /// member indirects to the enclosing struct that is still being decoded
    /// → [`TypeCode::Recursive`]. Hand-built frame (the encoder emits no
    /// indirection). Top-level struct: the encap body starts at abs offset 8,
    /// the indirection's offset field sits at 12+i_pos → offset = -(12+i_pos)
    /// points at the Node TCKind at abs 0.
    #[test]
    fn indirection_recursive_struct() {
        use tckind::{INDIRECTION, TK_STRUCT};
        let mut eb = BufferWriter::new(Endianness::Big);
        eb.write_u8(0).unwrap(); // BO octet (big)
        eb.write_string("IDL:Node:1.0").unwrap();
        eb.write_string("Node").unwrap();
        eb.write_u32(1).unwrap(); // 1 member
        eb.write_string("n").unwrap();
        eb.align(4);
        let i_pos = eb.position();
        eb.write_u32(INDIRECTION).unwrap();
        let offset: i32 = -(12 + i_pos as i32);
        eb.write_u32(offset as u32).unwrap();
        let body = eb.into_bytes();

        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u32(TK_STRUCT).unwrap();
        w.write_u32(body.len() as u32).unwrap();
        w.write_bytes(&body).unwrap();
        let bytes = w.into_bytes();

        let decoded = TypeCode::decode(&mut BufferReader::new(&bytes, Endianness::Big)).unwrap();
        assert_eq!(
            decoded,
            TypeCode::Struct {
                repo_id: "IDL:Node:1.0".into(),
                name: "Node".into(),
                members: vec![(
                    "n".into(),
                    TypeCode::Recursive {
                        repo_id: "IDL:Node:1.0".into()
                    }
                )],
                is_except: false,
            }
        );
    }

    /// Indirection with an unresolvable target (no previously decoded TCKind at
    /// the target position) → error instead of panic/infinite loop.
    #[test]
    fn indirection_unresolved_target_is_error() {
        use tckind::{INDIRECTION, TK_STRUCT};
        // struct with one member whose type is an indirection with offset -4
        // (points into the middle of the length/repo-ID field, no TCKind there).
        let mut eb = BufferWriter::new(Endianness::Big);
        eb.write_u8(0).unwrap();
        eb.write_string("IDL:X:1.0").unwrap();
        eb.write_string("X").unwrap();
        eb.write_u32(1).unwrap();
        eb.write_string("m").unwrap();
        eb.align(4);
        eb.write_u32(INDIRECTION).unwrap();
        eb.write_u32((-4_i32) as u32).unwrap(); // target = offset field - 4, no TCKind
        let body = eb.into_bytes();
        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u32(TK_STRUCT).unwrap();
        w.write_u32(body.len() as u32).unwrap();
        w.write_bytes(&body).unwrap();
        let bytes = w.into_bytes();
        assert!(TypeCode::decode(&mut BufferReader::new(&bytes, Endianness::Big)).is_err());
    }

    /// Indirection (§15.3.5.1) — **repeated** TypeCode: `struct Outer { S a;
    /// S b; }`, where `b`'s type is an indirection to `a`'s (already decoded)
    /// `S` TypeCode → cloned from the cache. Both members are the same `S`.
    /// Same encap: the abs offset 8 cancels out, so
    /// offset = `s_subpos - (i_pos + 4)`.
    #[test]
    fn indirection_repeated_typecode() {
        use tckind::{INDIRECTION, TK_STRUCT};
        let s = TypeCode::Struct {
            repo_id: "IDL:S:1.0".into(),
            name: "S".into(),
            members: vec![("x".into(), TypeCode::Long)],
            is_except: false,
        };
        let mut eb = BufferWriter::new(Endianness::Big);
        eb.write_u8(0).unwrap(); // BO
        eb.write_string("IDL:Outer:1.0").unwrap();
        eb.write_string("Outer").unwrap();
        eb.write_u32(2).unwrap(); // 2 members
        eb.write_string("a").unwrap();
        eb.align(4);
        let s_subpos = eb.position();
        s.encode(&mut eb).unwrap(); // S at s_subpos
        eb.write_string("b").unwrap();
        eb.align(4);
        let i_pos = eb.position();
        eb.write_u32(INDIRECTION).unwrap();
        let offset: i32 = s_subpos as i32 - (i_pos as i32 + 4);
        eb.write_u32(offset as u32).unwrap();
        let body = eb.into_bytes();

        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u32(TK_STRUCT).unwrap();
        w.write_u32(body.len() as u32).unwrap();
        w.write_bytes(&body).unwrap();
        let bytes = w.into_bytes();

        let decoded = TypeCode::decode(&mut BufferReader::new(&bytes, Endianness::Big)).unwrap();
        assert_eq!(
            decoded,
            TypeCode::Struct {
                repo_id: "IDL:Outer:1.0".into(),
                name: "Outer".into(),
                members: vec![("a".into(), s.clone()), ("b".into(), s.clone())],
                is_except: false,
            }
        );
    }

    /// A positive offset (indirection points forward) is invalid → error.
    #[test]
    fn indirection_forward_offset_rejected() {
        use tckind::INDIRECTION;
        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u32(INDIRECTION).unwrap();
        w.write_u32(4_u32).unwrap(); // positive offset
        let bytes = w.into_bytes();
        assert!(TypeCode::decode(&mut BufferReader::new(&bytes, Endianness::Big)).is_err());
    }

    #[test]
    fn struct_roundtrip_both_orders() {
        let tc = TypeCode::Struct {
            repo_id: "IDL:Point:1.0".into(),
            name: "Point".into(),
            members: vec![
                ("x".into(), TypeCode::Long),
                ("y".into(), TypeCode::Long),
                ("label".into(), TypeCode::String(0)),
            ],
            is_except: false,
        };
        rt(&tc, Endianness::Big);
        rt(&tc, Endianness::Little);
    }

    #[test]
    fn enum_and_nested_sequence_of_struct() {
        rt(
            &TypeCode::Enum {
                repo_id: "IDL:Color:1.0".into(),
                name: "Color".into(),
                members: vec!["RED".into(), "GREEN".into(), "BLUE".into()],
            },
            Endianness::Big,
        );
        // sequence<Point> — complex element in a complex TC (nested encaps).
        let point = TypeCode::Struct {
            repo_id: "IDL:Point:1.0".into(),
            name: "Point".into(),
            members: vec![("x".into(), TypeCode::Long), ("y".into(), TypeCode::Long)],
            is_except: false,
        };
        let seq = TypeCode::Sequence {
            element: Box::new(point),
            bound: 10,
        };
        rt(&seq, Endianness::Big);
        rt(&seq, Endianness::Little);
    }

    #[test]
    fn struct_wire_layout_be() {
        // tk_struct=15, then encap (len + bo=0 + repo_id + name + count + members).
        let tc = TypeCode::Struct {
            repo_id: "X".into(),
            name: "X".into(),
            members: vec![],
            is_except: false,
        };
        let mut w = BufferWriter::new(Endianness::Big);
        tc.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[0..4], &[0, 0, 0, 15], "TCKind tk_struct");
        // then uint32 encap length, then byte-order octet 0.
        let encap_len = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        assert_eq!(bytes.len(), 8 + encap_len);
        assert_eq!(bytes[8], 0, "encap byte-order = big");
    }
}
