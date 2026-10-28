// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA `TypeCode` and its CDR wire form (CORBA 3.3 part 2 §15.3.5) — the
//! self-describing half of an `any`. A `TypeCode` is a `TCKind`
//! (`unsigned long`) followed by parameters:
//!
//! * **Empty-parameter kinds** (`tk_null`/`tk_void`/`tk_short`/…/
//!   `tk_boolean`/`tk_char`/`tk_octet`/`tk_wchar`/`tk_any`/`tk_TypeCode`/
//!   `tk_longlong`/…): the `TCKind` and nothing else.
//! * **Simple-parameter** (`tk_string`/`tk_wstring`): the `TCKind` and a
//!   `bound` (`unsigned long`), inline and *not* encapsulated.
//! * **Complex-parameter** (`tk_objref`/`tk_struct`/`tk_enum`/
//!   `tk_sequence`/`tk_alias`/`tk_except`): the `TCKind` and a CDR
//!   encapsulation — a `sequence<octet>` holding a byte-order octet and
//!   the parameters, with alignment counted from that octet.
//!
//! Indirection (`TCKind` `0xffffffff` and a negative offset, §15.3.5.1) is
//! **resolved** while decoding: a position cache from stream offset to
//! `TypeCode` lets a *repeated* `TypeCode` be cloned and a *recursive* one
//! — `struct Node { sequence<Node> kids; }` — be represented as
//! [`TypeCode::Recursive`]. Emitting an indirection is a separate feature;
//! accepting one is what cross-ORB interop needs.
//!
//! ## Where this lives
//!
//! In `zerodds-corba-rt` and not in `zerodds-wire`. A `TypeCode` is a
//! CORBA type, not a wire primitive: `zerodds-wire` reads and writes
//! bytes at the alignment a layout plan names, and a self-describing type
//! graph decoded out of an `any` is a model, not a framing rule.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// `TCKind` discriminators (§15.3.5.1, OMG wire values). The constant
/// names map one to one onto the OMG `tk_*` identifiers.
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

/// Hard cap on `TypeCode` nesting depth (sequence element, struct member,
/// alias content) while decoding.
///
/// Untrusted input — a `TypeCode` inside a GIOP `any` from any peer —
/// could otherwise nest without bound and exhaust the stack. That is a
/// network-reachable crash and not merely an allocation, so it is bounded
/// on its own axis rather than by the length checks that guard
/// allocation sizes.
pub const MAX_TYPECODE_DEPTH: u32 = 64;

/// CORBA `TypeCode`: every scalar kind, string and wstring, sequence,
/// struct, enum, alias and objref — the contents an `any` carries.
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
    /// `tk_wstring` with `bound` (0 = unbounded).
    WString(u32),
    /// `tk_sequence`: element type and `bound`.
    Sequence {
        /// The element's `TypeCode`.
        element: Box<TypeCode>,
        /// The bound (0 = unbounded).
        bound: u32,
    },
    /// `tk_struct`, and `tk_except` which has the same wire form:
    /// repository id, name, ordered members.
    Struct {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The struct's name.
        name: String,
        /// `(member_name, member_type)` in declaration order.
        members: Vec<(String, TypeCode)>,
        /// `true` makes this `tk_except` rather than `tk_struct`.
        is_except: bool,
    },
    /// `tk_enum`: repository id, name, enumerator names (index = value).
    Enum {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The enum's name.
        name: String,
        /// The enumerator names.
        members: Vec<String>,
    },
    /// `tk_alias` (a `typedef`): repository id, name, resolved content.
    Alias {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The alias's name.
        name: String,
        /// The content it resolves to.
        content: Box<TypeCode>,
    },
    /// `tk_objref`: the interface's repository id and name.
    ObjRef {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The interface's name.
        name: String,
    },
    /// A recursive reference (§15.3.5.1 indirection): points, by
    /// `repo_id`, at an enclosing `TypeCode` that is still being decoded —
    /// `struct Node { sequence<Node> kids; }`. A decode-side marker, and
    /// what keeps the type graph finite.
    Recursive {
        /// The repository id of the type referred back to.
        repo_id: String,
    },
}

impl TypeCode {
    /// The `TCKind` wire value.
    #[must_use]
    pub const fn tckind(&self) -> u32 {
        use tckind::{
            INDIRECTION, TK_ALIAS, TK_ANY, TK_BOOLEAN, TK_CHAR, TK_DOUBLE, TK_ENUM, TK_EXCEPT,
            TK_FLOAT, TK_LONG, TK_LONGLONG, TK_NULL, TK_OBJREF, TK_OCTET, TK_SEQUENCE, TK_SHORT,
            TK_STRING, TK_STRUCT, TK_TYPECODE, TK_ULONG, TK_ULONGLONG, TK_USHORT, TK_VOID,
            TK_WCHAR, TK_WSTRING,
        };
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
            // A decode-side marker; the wire carries the indirection.
            Self::Recursive { .. } => INDIRECTION,
        }
    }

    /// Writes the `TypeCode` (§15.3.5).
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a length that exceeds `u32`, and for
    /// [`TypeCode::Recursive`], which has no encode-side form.
    pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u32(self.tckind());
        match self {
            // Empty-parameter kinds: the TCKind and nothing else.
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
            // Simple-parameter: the bound inline.
            Self::String(b) | Self::WString(b) => {
                w.write_u32(*b);
                Ok(())
            }
            // Complex-parameter: an encapsulation.
            Self::Sequence { element, bound } => {
                let encap = build_encap(w.endianness(), |e| {
                    element.encode(e)?;
                    e.write_u32(*bound);
                    Ok(())
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
                    e.write_string(repo_id);
                    e.write_string(name);
                    e.write_u32(count_u32(
                        members.len(),
                        "TypeCode struct members",
                        e.position(),
                    )?);
                    for (member_name, member_type) in members {
                        e.write_string(member_name);
                        member_type.encode(e)?;
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
                    e.write_string(repo_id);
                    e.write_string(name);
                    e.write_u32(count_u32(
                        members.len(),
                        "TypeCode enum members",
                        e.position(),
                    )?);
                    for member_name in members {
                        e.write_string(member_name);
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
                    e.write_string(repo_id);
                    e.write_string(name);
                    content.encode(e)
                })?;
                write_encap(w, &encap)
            }
            Self::ObjRef { repo_id, name } => {
                let encap = build_encap(w.endianness(), |e| {
                    e.write_string(repo_id);
                    e.write_string(name);
                    Ok(())
                })?;
                write_encap(w, &encap)
            }
            // Emitting an indirection needs a backpatched offset table and
            // is a separate feature; accepting one is what interop needs.
            Self::Recursive { .. } => Err(WireError::OutOfRange {
                at: w.position(),
                expected: String::from(
                    "a TypeCode this writer can emit — TypeCode::Recursive needs indirection emit",
                ),
            }),
        }
    }

    /// Reads a `TypeCode` (§15.3.5) including **indirection** (§15.3.5.1):
    /// a recursive or repeated `TypeCode` (`0xffffffff` and a negative
    /// offset) is resolved — a repeated one cloned out of the position
    /// cache, a recursive one as [`TypeCode::Recursive`].
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for an unknown `TCKind` or an
    /// indirection that resolves to nothing, and whatever the reader
    /// raises for a short or malformed stream.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let mut cache = TcCache::new();
        // Base 0: cache positions are this reader's own offsets.
        Self::decode_ctx(r, 0, &mut cache, 0)
    }

    /// Position-tracking decode. `base` is the absolute stream offset of
    /// the reader's byte 0; `cache` maps `base + position` of a `TCKind`
    /// to the `TypeCode` decoded there, which is what makes a backward
    /// indirection resolvable. `depth` is the current nesting depth,
    /// capped by [`MAX_TYPECODE_DEPTH`].
    fn decode_ctx(
        r: &mut Reader<'_>,
        base: usize,
        cache: &mut TcCache,
        depth: u32,
    ) -> Result<Self, WireError> {
        use tckind::{
            INDIRECTION, TK_ALIAS, TK_ANY, TK_BOOLEAN, TK_CHAR, TK_DOUBLE, TK_ENUM, TK_EXCEPT,
            TK_FLOAT, TK_LONG, TK_LONGLONG, TK_NULL, TK_OBJREF, TK_OCTET, TK_SEQUENCE, TK_SHORT,
            TK_STRING, TK_STRUCT, TK_TYPECODE, TK_ULONG, TK_ULONGLONG, TK_USHORT, TK_VOID,
            TK_WCHAR, TK_WSTRING,
        };
        if depth > MAX_TYPECODE_DEPTH {
            return Err(WireError::OutOfRange {
                at: r.position(),
                expected: format!("a TypeCode nested at most {MAX_TYPECODE_DEPTH} deep"),
            });
        }
        // The TCKind is a four-aligned `unsigned long`. Align *before*
        // recording the position — `read_u32` would align internally and
        // the cache entry would then point at the padding rather than at
        // the TCKind, and every indirection into a nested TypeCode would
        // fail to resolve.
        r.align_to(4)?;
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
                // 0xffffffff and then a signed long: the offset from the
                // position of the offset field, backward, to the target
                // TCKind (§15.3.5.1).
                let off_field_pos = base + r.position();
                #[allow(clippy::cast_possible_wrap)]
                let offset = r.read_u32()? as i32;
                if offset >= 0 {
                    return Err(WireError::OutOfRange {
                        at: off_field_pos,
                        expected: String::from("a negative TypeCode indirection offset"),
                    });
                }
                let target = i64::try_from(off_field_pos)
                    .ok()
                    .map(|p| p + i64::from(offset))
                    .and_then(|t| usize::try_from(t).ok())
                    .ok_or_else(|| WireError::OutOfRange {
                        at: off_field_pos,
                        expected: String::from(
                            "a TypeCode indirection target inside the stream, not before its start",
                        ),
                    })?;
                match cache.get(&target) {
                    Some(TcCacheEntry::Done(tc)) => Ok(tc.clone()),
                    Some(TcCacheEntry::InProgress(repo_id)) => Ok(Self::Recursive {
                        repo_id: repo_id.clone(),
                    }),
                    None => Err(WireError::OutOfRange {
                        at: off_field_pos,
                        expected: format!(
                            "a TypeCode indirection whose target is a TCKind already read — \
                             nothing was decoded at {target}"
                        ),
                    }),
                }
            }
            TK_SEQUENCE => decode_encap_ctx(r, base, cache, |e, child_base, cache| {
                let element = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                let bound = e.read_u32()?;
                let tc = Self::Sequence { element, bound };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            TK_STRUCT | TK_EXCEPT => decode_encap_ctx(r, base, cache, |e, child_base, cache| {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                // The placeholder goes in *before* the members: a member
                // that indirects back to this struct resolves to
                // Recursive rather than to nothing.
                cache.insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let count = e.read_u32()? as usize;
                let mut members = Vec::with_capacity(count.min(256));
                for _ in 0..count {
                    let member_name = e.read_string()?;
                    let member_type = Self::decode_ctx(e, child_base, cache, depth + 1)?;
                    members.push((member_name, member_type));
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
            TK_ENUM => decode_encap_ctx(r, base, cache, |e, _child_base, cache| {
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
            TK_ALIAS => decode_encap_ctx(r, base, cache, |e, child_base, cache| {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                cache.insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let content = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                let tc = Self::Alias {
                    repo_id,
                    name,
                    content,
                };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            TK_OBJREF => decode_encap_ctx(r, base, cache, |e, _child_base, cache| {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                let tc = Self::ObjRef { repo_id, name };
                cache.insert(tckind_pos, TcCacheEntry::Done(tc.clone()));
                Ok(tc)
            }),
            other => Err(WireError::OutOfRange {
                at: tckind_pos,
                expected: format!("a TCKind this decoder knows, not {other}"),
            }),
        }
    }
}

/// A cache entry for indirection resolution (§15.3.5.1).
enum TcCacheEntry {
    /// A complex `TypeCode` still being decoded — an indirection to it is
    /// recursive and yields [`TypeCode::Recursive`].
    InProgress(String),
    /// Fully decoded — an indirection to it is a repeated `TypeCode`.
    Done(TypeCode),
}

/// The position cache: stream offset of a `TCKind` to its entry.
type TcCache = BTreeMap<usize, TcCacheEntry>;

/// A length as a `u32`, or a refusal that names the field.
fn count_u32(len: usize, what: &str, at: usize) -> Result<u32, WireError> {
    u32::try_from(len).map_err(|_| WireError::OutOfRange {
        at,
        expected: format!("at most u32::MAX {what}, not {len}"),
    })
}

/// Builds a CDR encapsulation: the byte-order octet and then the body,
/// with alignment counted from that octet — which a fresh writer gives
/// naturally.
fn build_encap<F>(endianness: Endianness, body: F) -> Result<Vec<u8>, WireError>
where
    F: FnOnce(&mut Writer) -> Result<(), WireError>,
{
    let mut e = Writer::new(endianness);
    e.write_u8(match endianness {
        Endianness::Big => 0,
        Endianness::Little => 1,
    });
    body(&mut e)?;
    Ok(e.finish())
}

/// Writes an encapsulation as a `sequence<octet>`: its length, then its
/// bytes.
fn write_encap(w: &mut Writer, encap: &[u8]) -> Result<(), WireError> {
    let len = count_u32(
        encap.len(),
        "bytes in a TypeCode encapsulation",
        w.position(),
    )?;
    w.write_u32(len);
    w.write_bytes(encap);
    Ok(())
}

/// Reads an encapsulation (`sequence<octet>`) and decodes its body in the
/// order its byte-order octet names, with alignment counted from that
/// octet.
///
/// The body is handed the **absolute stream offset** of the
/// encapsulation's byte 0, so that an indirection inside it can resolve
/// to a `TCKind` outside it.
fn decode_encap_ctx<F>(
    r: &mut Reader<'_>,
    base: usize,
    cache: &mut TcCache,
    body: F,
) -> Result<TypeCode, WireError>
where
    F: FnOnce(&mut Reader<'_>, usize, &mut TcCache) -> Result<TypeCode, WireError>,
{
    let at = r.position();
    let len = r.read_u32()? as usize;
    // The stream position read_bytes copies from is the absolute offset
    // of byte 0 of the sub-buffer.
    let child_base = base + r.position();
    let bytes = r.read_bytes(len)?;
    let Some(&bo) = bytes.first() else {
        return Err(WireError::OutOfRange {
            at,
            expected: String::from("a TypeCode encapsulation with at least a byte-order octet"),
        });
    };
    let endianness = match bo {
        0 => Endianness::Big,
        1 => Endianness::Little,
        other => {
            return Err(WireError::OutOfRange {
                at: child_base,
                expected: format!("a byte-order octet of 0 or 1, not {other}"),
            });
        }
    };
    // A reader over the WHOLE encapsulation, so alignment counts from the
    // byte-order octet; skip the octet and read the body in its order.
    let mut e = Reader::new(bytes, endianness);
    let _bo = e.read_u8()?;
    body(&mut e, child_base, cache)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;

    fn rt(tc: &TypeCode, e: Endianness) {
        let mut w = Writer::new(e);
        tc.encode(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, e);
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

    // A `TypeCode` nested — through a sequence element, a struct member
    // or an alias content — deeper than `MAX_TYPECODE_DEPTH` has to be
    // refused cleanly rather than recursing without bound. A GIOP peer
    // controls this nesting through any decoded `any`.

    #[test]
    fn nested_sequence_beyond_max_depth_rejected_cleanly() {
        let mut tc = TypeCode::Octet;
        for _ in 0..(MAX_TYPECODE_DEPTH + 10) {
            tc = TypeCode::Sequence {
                element: Box::new(tc),
                bound: 0,
            };
        }
        let mut w = Writer::new(Endianness::Big);
        tc.encode(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let res = TypeCode::decode(&mut r);
        assert!(
            matches!(res, Err(WireError::OutOfRange { .. })),
            "expected a clean refusal, got {res:?}"
        );
    }

    #[test]
    fn nested_sequence_within_max_depth_still_round_trips() {
        let mut tc = TypeCode::Octet;
        for _ in 0..10 {
            tc = TypeCode::Sequence {
                element: Box::new(tc),
                bound: 0,
            };
        }
        rt(&tc, Endianness::Big);
    }

    /// Indirection (§15.3.5.1) — a **recursive** `struct Node { Node n; }`:
    /// the member indirects to the enclosing struct that is still being
    /// decoded, and resolves to [`TypeCode::Recursive`]. Hand-built,
    /// because the encoder emits no indirection. The encapsulation body
    /// starts at absolute offset 8, so the offset field at `12 + i_pos`
    /// needs `-(12 + i_pos)` to reach the Node TCKind at absolute 0.
    #[test]
    fn indirection_recursive_struct() {
        use tckind::{INDIRECTION, TK_STRUCT};
        let mut eb = Writer::new(Endianness::Big);
        eb.write_u8(0); // byte-order octet, big
        eb.write_string("IDL:Node:1.0");
        eb.align_to(4);
        eb.write_string("Node");
        eb.align_to(4);
        eb.write_u32(1); // one member
        eb.write_string("n");
        eb.align_to(4);
        let i_pos = eb.position();
        eb.write_u32(INDIRECTION);
        #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
        let offset: i32 = -(12 + i_pos as i32);
        #[allow(clippy::cast_sign_loss)]
        eb.write_u32(offset as u32);
        let body = eb.finish();

        let mut w = Writer::new(Endianness::Big);
        w.write_u32(TK_STRUCT);
        #[allow(clippy::cast_possible_truncation)]
        w.write_u32(body.len() as u32);
        w.write_bytes(&body);
        let bytes = w.finish();

        let decoded = TypeCode::decode(&mut Reader::new(&bytes, Endianness::Big)).unwrap();
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

    /// An indirection whose target holds no previously decoded `TCKind`
    /// is an error rather than a panic or a loop.
    #[test]
    fn indirection_unresolved_target_is_error() {
        use tckind::{INDIRECTION, TK_STRUCT};
        let mut eb = Writer::new(Endianness::Big);
        eb.write_u8(0);
        eb.write_string("IDL:X:1.0");
        eb.align_to(4);
        eb.write_string("X");
        eb.align_to(4);
        eb.write_u32(1);
        eb.write_string("m");
        eb.align_to(4);
        eb.write_u32(INDIRECTION);
        #[allow(clippy::cast_sign_loss)]
        eb.write_u32((-4_i32) as u32); // target = offset field - 4, no TCKind there
        let body = eb.finish();
        let mut w = Writer::new(Endianness::Big);
        w.write_u32(TK_STRUCT);
        #[allow(clippy::cast_possible_truncation)]
        w.write_u32(body.len() as u32);
        w.write_bytes(&body);
        let bytes = w.finish();
        assert!(TypeCode::decode(&mut Reader::new(&bytes, Endianness::Big)).is_err());
    }

    /// Indirection (§15.3.5.1) — a **repeated** `TypeCode`:
    /// `struct Outer { S a; S b; }`, where `b`'s type indirects to `a`'s
    /// already-decoded `S` and is cloned out of the cache. Same
    /// encapsulation, so the absolute offset 8 cancels and the offset is
    /// `s_subpos - (i_pos + 4)`.
    #[test]
    fn indirection_repeated_typecode() {
        use tckind::{INDIRECTION, TK_STRUCT};
        let s = TypeCode::Struct {
            repo_id: "IDL:S:1.0".into(),
            name: "S".into(),
            members: vec![("x".into(), TypeCode::Long)],
            is_except: false,
        };
        let mut eb = Writer::new(Endianness::Big);
        eb.write_u8(0); // byte-order octet
        eb.write_string("IDL:Outer:1.0");
        eb.align_to(4);
        eb.write_string("Outer");
        eb.align_to(4);
        eb.write_u32(2); // two members
        eb.write_string("a");
        eb.align_to(4);
        let s_subpos = eb.position();
        s.encode(&mut eb).unwrap(); // S at s_subpos
        eb.write_string("b");
        eb.align_to(4);
        let i_pos = eb.position();
        eb.write_u32(INDIRECTION);
        #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
        let offset: i32 = s_subpos as i32 - (i_pos as i32 + 4);
        #[allow(clippy::cast_sign_loss)]
        eb.write_u32(offset as u32);
        let body = eb.finish();

        let mut w = Writer::new(Endianness::Big);
        w.write_u32(TK_STRUCT);
        #[allow(clippy::cast_possible_truncation)]
        w.write_u32(body.len() as u32);
        w.write_bytes(&body);
        let bytes = w.finish();

        let decoded = TypeCode::decode(&mut Reader::new(&bytes, Endianness::Big)).unwrap();
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

    /// A forward (positive) indirection offset is invalid.
    #[test]
    fn indirection_forward_offset_rejected() {
        use tckind::INDIRECTION;
        let mut w = Writer::new(Endianness::Big);
        w.write_u32(INDIRECTION);
        w.write_u32(4_u32); // a positive offset
        let bytes = w.finish();
        assert!(TypeCode::decode(&mut Reader::new(&bytes, Endianness::Big)).is_err());
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
    fn except_keeps_its_own_tckind() {
        let tc = TypeCode::Struct {
            repo_id: "IDL:Bad:1.0".into(),
            name: "Bad".into(),
            members: vec![("why".into(), TypeCode::String(0))],
            is_except: true,
        };
        assert_eq!(tc.tckind(), tckind::TK_EXCEPT);
        rt(&tc, Endianness::Big);
        rt(&tc, Endianness::Little);
    }

    #[test]
    fn alias_and_objref_roundtrip() {
        rt(
            &TypeCode::Alias {
                repo_id: "IDL:Meters:1.0".into(),
                name: "Meters".into(),
                content: Box::new(TypeCode::Double),
            },
            Endianness::Big,
        );
        rt(
            &TypeCode::ObjRef {
                repo_id: "IDL:Bank/Account:1.0".into(),
                name: "Account".into(),
            },
            Endianness::Little,
        );
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
        // sequence<Point> — a complex element inside a complex TypeCode,
        // so nested encapsulations.
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
        // tk_struct = 15, then the encapsulation: length, byte-order
        // octet, repo_id, name, member count, members.
        let tc = TypeCode::Struct {
            repo_id: "X".into(),
            name: "X".into(),
            members: vec![],
            is_except: false,
        };
        let mut w = Writer::new(Endianness::Big);
        tc.encode(&mut w).unwrap();
        let bytes = w.finish();
        assert_eq!(&bytes[0..4], &[0, 0, 0, 15], "TCKind tk_struct");
        let encap_len = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        assert_eq!(bytes.len(), 8 + encap_len);
        assert_eq!(bytes[8], 0, "the encapsulation's byte order is big");
    }

    /// The refusal [`TypeCode::Recursive`] gets on the encode side is a
    /// refusal and not a silently truncated stream.
    #[test]
    fn recursive_marker_refuses_to_encode() {
        let tc = TypeCode::Recursive {
            repo_id: "IDL:Node:1.0".into(),
        };
        let mut w = Writer::new(Endianness::Big);
        assert!(matches!(
            tc.encode(&mut w),
            Err(WireError::OutOfRange { .. })
        ));
    }
}
