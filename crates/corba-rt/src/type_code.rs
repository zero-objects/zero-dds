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
//! Every `TCKind` of CORBA 3.3 Part 2 Table 9.2 is modelled, with its
//! parameter list: the empty kinds (including `tk_Principal` and
//! `tk_longdouble`), the simple ones (`tk_string`, `tk_wstring`,
//! `tk_fixed`) and the complex ones (`tk_objref`, `tk_struct`,
//! `tk_union`, `tk_enum`, `tk_sequence`, `tk_array`, `tk_alias`,
//! `tk_except`, `tk_value`, `tk_value_box`, `tk_native`,
//! `tk_abstract_interface`, `tk_local_interface`, `tk_component`,
//! `tk_home`, `tk_event`).
//!
//! Indirection (`TCKind` `0xffffffff` and a negative offset, §9.3.5.1.6)
//! is **resolved** while decoding: a position cache from stream offset to
//! `TypeCode` lets a *repeated* `TypeCode` be cloned and a *recursive* one
//! — `struct Node { sequence<Node> kids; }` — be represented as
//! [`TypeCode::Recursive`]. On the way out a [`TypeCode::Recursive`] is
//! written as an indirection to the `TCKind` of the enclosing `TypeCode`
//! with that repository id, the offset counted across the encapsulations
//! in between.
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
    pub const TK_PRINCIPAL: u32 = 13;
    pub const TK_OBJREF: u32 = 14;
    pub const TK_STRUCT: u32 = 15;
    pub const TK_UNION: u32 = 16;
    pub const TK_ENUM: u32 = 17;
    pub const TK_STRING: u32 = 18;
    pub const TK_SEQUENCE: u32 = 19;
    pub const TK_ARRAY: u32 = 20;
    pub const TK_ALIAS: u32 = 21;
    pub const TK_EXCEPT: u32 = 22;
    pub const TK_LONGLONG: u32 = 23;
    pub const TK_ULONGLONG: u32 = 24;
    pub const TK_LONGDOUBLE: u32 = 25;
    pub const TK_WCHAR: u32 = 26;
    pub const TK_WSTRING: u32 = 27;
    pub const TK_FIXED: u32 = 28;
    pub const TK_VALUE: u32 = 29;
    pub const TK_VALUE_BOX: u32 = 30;
    pub const TK_NATIVE: u32 = 31;
    pub const TK_ABSTRACT_INTERFACE: u32 = 32;
    pub const TK_LOCAL_INTERFACE: u32 = 33;
    pub const TK_COMPONENT: u32 = 34;
    pub const TK_HOME: u32 = 35;
    pub const TK_EVENT: u32 = 36;
    pub const INDIRECTION: u32 = 0xffff_ffff;
}

/// `CORBA::ValueModifier` values of a `tk_value` / `tk_event` (Part 1
/// §8.11).
pub mod value_modifier {
    /// `VM_NONE`.
    pub const VM_NONE: i16 = 0;
    /// `VM_CUSTOM`.
    pub const VM_CUSTOM: i16 = 1;
    /// `VM_ABSTRACT`.
    pub const VM_ABSTRACT: i16 = 2;
    /// `VM_TRUNCATABLE`.
    pub const VM_TRUNCATABLE: i16 = 3;
}

/// `CORBA::Visibility` of a value member.
pub mod visibility {
    /// `PRIVATE_MEMBER`.
    pub const PRIVATE_MEMBER: i16 = 0;
    /// `PUBLIC_MEMBER`.
    pub const PUBLIC_MEMBER: i16 = 1;
}

/// A union case label (Part 2 Table 9.2, footnote a): a value of the
/// discriminant type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnionLabel {
    /// `short` discriminant.
    Short(i16),
    /// `long` discriminant.
    Long(i32),
    /// `long long` discriminant.
    LongLong(i64),
    /// `unsigned short` discriminant.
    UShort(u16),
    /// `unsigned long` discriminant.
    ULong(u32),
    /// `unsigned long long` discriminant.
    ULongLong(u64),
    /// `char` discriminant.
    Char(u8),
    /// `wchar` discriminant, as the transmission code unit.
    WChar(u16),
    /// `boolean` discriminant.
    Boolean(bool),
    /// An enum discriminant, by ordinal.
    Enum(u32),
}

impl UnionLabel {
    /// Writes the label in the representation of its type.
    pub fn encode(self, w: &mut Writer) {
        match self {
            Self::Short(v) => w.write_i16(v),
            Self::Long(v) => w.write_i32(v),
            Self::LongLong(v) => w.write_i64(v),
            Self::UShort(v) | Self::WChar(v) => w.write_u16(v),
            Self::ULong(v) | Self::Enum(v) => w.write_u32(v),
            Self::ULongLong(v) => w.write_u64(v),
            Self::Char(v) => w.write_u8(v),
            Self::Boolean(v) => w.write_bool(v),
        }
    }

    /// Reads a label of the discriminant type `discriminator` (aliases
    /// resolved).
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a type no union can be discriminated
    /// by, and whatever the reader raises.
    pub fn decode(discriminator: &TypeCode, r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(match discriminator.resolved() {
            TypeCode::Short => Self::Short(r.read_i16()?),
            TypeCode::Long => Self::Long(r.read_i32()?),
            TypeCode::LongLong => Self::LongLong(r.read_i64()?),
            TypeCode::UShort => Self::UShort(r.read_u16()?),
            TypeCode::ULong => Self::ULong(r.read_u32()?),
            TypeCode::ULongLong => Self::ULongLong(r.read_u64()?),
            TypeCode::Char => Self::Char(r.read_u8()?),
            TypeCode::WChar => Self::WChar(r.read_u16()?),
            TypeCode::Boolean => Self::Boolean(r.read_bool()?),
            TypeCode::Enum { .. } => Self::Enum(r.read_u32()?),
            other => {
                return Err(WireError::OutOfRange {
                    at: r.position(),
                    expected: format!(
                        "a union discriminant of an integer, char, wchar, boolean or enum \
                         type, not TCKind {}",
                        other.tckind()
                    ),
                });
            }
        })
    }

    /// Whether the label is a value of `discriminator` (aliases
    /// resolved).
    #[must_use]
    pub fn fits(self, discriminator: &TypeCode) -> bool {
        matches!(
            (self, discriminator.resolved()),
            (Self::Short(_), TypeCode::Short)
                | (Self::Long(_), TypeCode::Long)
                | (Self::LongLong(_), TypeCode::LongLong)
                | (Self::UShort(_), TypeCode::UShort)
                | (Self::ULong(_), TypeCode::ULong)
                | (Self::ULongLong(_), TypeCode::ULongLong)
                | (Self::Char(_), TypeCode::Char)
                | (Self::WChar(_), TypeCode::WChar)
                | (Self::Boolean(_), TypeCode::Boolean)
                | (Self::Enum(_), TypeCode::Enum { .. })
        )
    }
}

/// One case of a `tk_union`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionMember {
    /// The label; for the default case any value of the discriminant
    /// type (§9.3.5.1.4).
    pub label: UnionLabel,
    /// The member name.
    pub name: String,
    /// The member type.
    pub member_type: TypeCode,
}

/// One state member of a `tk_value` / `tk_event`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueMember {
    /// The member name.
    pub name: String,
    /// The member type.
    pub member_type: TypeCode,
    /// [`visibility::PRIVATE_MEMBER`] or [`visibility::PUBLIC_MEMBER`].
    pub visibility: i16,
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

/// How many times the octets of its own encoding one `TypeCode` may read
/// again while it is decoded, through indirections to repeated
/// `TypeCode`s (Part 2 §9.3.5.1: an indirection may point to any
/// `TypeCode` read before in the same top-level `TypeCode`, not only to
/// an enclosing one).
///
/// A repeated `TypeCode` is a copy of its target at the place of the
/// indirection: the decoder reads the target's encapsulation again. The
/// copies nest — a struct whose two members are a nested struct and an
/// indirection to it is twice the nested struct, and a chain of such
/// structs doubles the decoded tree at every level — so a `TypeCode` of a
/// few hundred octets would otherwise decode to more nodes than memory
/// holds. What one decode reads again is bounded by the octets of the
/// `TypeCode` instead; a `TypeCode` that needs more is refused with a
/// [`WireError`]. A top-level `TypeCode` that is an indirection to an
/// earlier one of the same message ([`TypeCode::decode_top_level`]) is a
/// copy as well: such copies together are bounded by the same multiple of
/// the octets of the message read up to them.
pub const MAX_TYPECODE_REREAD_FACTOR: usize = 32;

/// CORBA `TypeCode`: every `TCKind` of Part 2 Table 9.2.
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
    /// `tk_Principal`.
    Principal,
    /// `tk_longdouble`.
    LongDouble,
    /// `tk_fixed`: digits and scale.
    Fixed {
        /// Total number of decimal digits (1 to 31).
        digits: u16,
        /// Digits after the decimal point.
        scale: i16,
    },
    /// `tk_array`: one dimension; a multi-dimensional array nests one
    /// `tk_array` per dimension, the outermost for the leftmost index
    /// (§9.3.5.1.5).
    Array {
        /// The element's `TypeCode`.
        element: Box<TypeCode>,
        /// The number of elements.
        length: u32,
    },
    /// `tk_union`: repository id, name, discriminant type, the index of
    /// the default case (negative: none), the cases in declaration order.
    Union {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The union's name.
        name: String,
        /// The discriminant type.
        discriminator: Box<TypeCode>,
        /// The index of the default case in `members`, or negative.
        default_index: i32,
        /// The cases.
        members: Vec<UnionMember>,
    },
    /// `tk_value`, and `tk_event` which has the same parameters:
    /// repository id, name, value modifier, concrete base (`tk_null` for
    /// none), state members.
    Value {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The value type's name (may be empty).
        name: String,
        /// A [`value_modifier`] value.
        modifier: i16,
        /// The concrete base, or [`TypeCode::Null`].
        concrete_base: Box<TypeCode>,
        /// The state members of this type (not those of the base).
        members: Vec<ValueMember>,
        /// `true` makes this `tk_event` rather than `tk_value`.
        is_event: bool,
    },
    /// `tk_value_box`: repository id, name, the boxed type.
    ValueBox {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The box's name.
        name: String,
        /// The boxed type.
        content: Box<TypeCode>,
    },
    /// `tk_native`: repository id and name.
    Native {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The name.
        name: String,
    },
    /// `tk_abstract_interface`: repository id and name.
    AbstractInterface {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The name.
        name: String,
    },
    /// `tk_local_interface`: repository id and name.
    LocalInterface {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The name.
        name: String,
    },
    /// `tk_component`: repository id and name.
    Component {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The name.
        name: String,
    },
    /// `tk_home`: repository id and name.
    Home {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The name.
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
            Self::Principal => TK_PRINCIPAL,
            Self::LongDouble => TK_LONGDOUBLE,
            Self::String(_) => TK_STRING,
            Self::WString(_) => TK_WSTRING,
            Self::Fixed { .. } => TK_FIXED,
            Self::Sequence { .. } => TK_SEQUENCE,
            Self::Array { .. } => TK_ARRAY,
            Self::Struct {
                is_except: true, ..
            } => TK_EXCEPT,
            Self::Struct { .. } => TK_STRUCT,
            Self::Union { .. } => TK_UNION,
            Self::Enum { .. } => TK_ENUM,
            Self::Alias { .. } => TK_ALIAS,
            Self::ObjRef { .. } => TK_OBJREF,
            Self::Value { is_event: true, .. } => TK_EVENT,
            Self::Value { .. } => TK_VALUE,
            Self::ValueBox { .. } => TK_VALUE_BOX,
            Self::Native { .. } => TK_NATIVE,
            Self::AbstractInterface { .. } => TK_ABSTRACT_INTERFACE,
            Self::LocalInterface { .. } => TK_LOCAL_INTERFACE,
            Self::Component { .. } => TK_COMPONENT,
            Self::Home { .. } => TK_HOME,
            // The wire carries the indirection.
            Self::Recursive { .. } => INDIRECTION,
        }
    }

    /// The `TypeCode` with every `tk_alias` around it removed.
    #[must_use]
    pub fn resolved(&self) -> &Self {
        let mut tc = self;
        while let Self::Alias { content, .. } = tc {
            tc = content;
        }
        tc
    }

    /// The repository id of a `TypeCode` that has one.
    #[must_use]
    pub fn repo_id(&self) -> Option<&str> {
        match self {
            Self::Struct { repo_id, .. }
            | Self::Union { repo_id, .. }
            | Self::Enum { repo_id, .. }
            | Self::Alias { repo_id, .. }
            | Self::ObjRef { repo_id, .. }
            | Self::Value { repo_id, .. }
            | Self::ValueBox { repo_id, .. }
            | Self::Native { repo_id, .. }
            | Self::AbstractInterface { repo_id, .. }
            | Self::LocalInterface { repo_id, .. }
            | Self::Component { repo_id, .. }
            | Self::Home { repo_id, .. }
            | Self::Recursive { repo_id } => Some(repo_id),
            _ => None,
        }
    }

    /// Writes the `TypeCode` (§9.3.5.1). A [`TypeCode::Recursive`] inside
    /// it is written as an indirection (§9.3.5.1.6) to the enclosing
    /// `TypeCode` with that repository id.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a length that exceeds `u32`, a union
    /// label that is not a value of the discriminant type, and a
    /// [`TypeCode::Recursive`] that no enclosing `TypeCode` matches (a
    /// top-level one included: an indirection is never freestanding).
    pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
        let mut open = Vec::new();
        self.encode_ctx(w, 0, &mut open)
    }

    /// Position-tracking encode. `base` is the absolute offset of `w`'s
    /// byte 0 in the top-level `TypeCode`'s stream; `open` holds the
    /// repository id and absolute `TCKind` offset of every complex
    /// `TypeCode` being written around this one.
    fn encode_ctx(
        &self,
        w: &mut Writer,
        base: usize,
        open: &mut Vec<(String, usize)>,
    ) -> Result<(), WireError> {
        w.align_to(4);
        let tckind_pos = base + w.position();
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
            | Self::TypeCodeTc
            | Self::Principal
            | Self::LongDouble => Ok(()),
            // Simple-parameter: inline.
            Self::String(b) | Self::WString(b) => {
                w.write_u32(*b);
                Ok(())
            }
            Self::Fixed { digits, scale } => {
                w.write_u16(*digits);
                w.write_i16(*scale);
                Ok(())
            }
            // Complex-parameter: an encapsulation.
            Self::Sequence { element, bound } => encap_ctx(w, base, open, |e, b, open| {
                element.encode_ctx(e, b, open)?;
                e.write_u32(*bound);
                Ok(())
            }),
            Self::Array { element, length } => encap_ctx(w, base, open, |e, b, open| {
                element.encode_ctx(e, b, open)?;
                e.write_u32(*length);
                Ok(())
            }),
            Self::Struct {
                repo_id,
                name,
                members,
                ..
            } => encap_ctx(w, base, open, |e, b, open| {
                e.write_string(repo_id);
                e.write_string(name);
                e.write_u32(count_u32(
                    members.len(),
                    "TypeCode struct members",
                    e.position(),
                )?);
                open.push((repo_id.clone(), tckind_pos));
                for (member_name, member_type) in members {
                    e.write_string(member_name);
                    member_type.encode_ctx(e, b, open)?;
                }
                open.pop();
                Ok(())
            }),
            Self::Union {
                repo_id,
                name,
                discriminator,
                default_index,
                members,
            } => encap_ctx(w, base, open, |e, b, open| {
                e.write_string(repo_id);
                e.write_string(name);
                open.push((repo_id.clone(), tckind_pos));
                discriminator.encode_ctx(e, b, open)?;
                e.write_i32(*default_index);
                e.write_u32(count_u32(
                    members.len(),
                    "TypeCode union members",
                    e.position(),
                )?);
                for m in members {
                    if !m.label.fits(discriminator) {
                        return Err(WireError::OutOfRange {
                            at: e.position(),
                            expected: format!(
                                "a union label of the discriminant type, not {:?}",
                                m.label
                            ),
                        });
                    }
                    m.label.encode(e);
                    e.write_string(&m.name);
                    m.member_type.encode_ctx(e, b, open)?;
                }
                open.pop();
                Ok(())
            }),
            Self::Enum {
                repo_id,
                name,
                members,
            } => encap_ctx(w, base, open, |e, _b, _open| {
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
            }),
            Self::Alias {
                repo_id,
                name,
                content,
            }
            | Self::ValueBox {
                repo_id,
                name,
                content,
            } => encap_ctx(w, base, open, |e, b, open| {
                e.write_string(repo_id);
                e.write_string(name);
                open.push((repo_id.clone(), tckind_pos));
                content.encode_ctx(e, b, open)?;
                open.pop();
                Ok(())
            }),
            Self::Value {
                repo_id,
                name,
                modifier,
                concrete_base,
                members,
                ..
            } => encap_ctx(w, base, open, |e, b, open| {
                e.write_string(repo_id);
                e.write_string(name);
                e.write_i16(*modifier);
                open.push((repo_id.clone(), tckind_pos));
                concrete_base.encode_ctx(e, b, open)?;
                e.write_u32(count_u32(
                    members.len(),
                    "TypeCode value members",
                    e.position(),
                )?);
                for m in members {
                    e.write_string(&m.name);
                    m.member_type.encode_ctx(e, b, open)?;
                    e.write_i16(m.visibility);
                }
                open.pop();
                Ok(())
            }),
            Self::ObjRef { repo_id, name }
            | Self::Native { repo_id, name }
            | Self::AbstractInterface { repo_id, name }
            | Self::LocalInterface { repo_id, name }
            | Self::Component { repo_id, name }
            | Self::Home { repo_id, name } => encap_ctx(w, base, open, |e, _b, _open| {
                e.write_string(repo_id);
                e.write_string(name);
                Ok(())
            }),
            // §9.3.5.1.6: 0xffffffff (written above) and the offset from
            // the offset field back to the TCKind of the enclosing
            // TypeCode.
            Self::Recursive { repo_id } => {
                let Some(&(_, target)) = open.iter().rev().find(|(id, _)| id == repo_id) else {
                    return Err(WireError::OutOfRange {
                        at: w.position(),
                        expected: format!(
                            "a recursive TypeCode inside the TypeCode {repo_id} it refers to"
                        ),
                    });
                };
                let field = base + w.position();
                let offset = i64::try_from(target)
                    .ok()
                    .zip(i64::try_from(field).ok())
                    .and_then(|(t, f)| i32::try_from(t - f).ok())
                    .ok_or_else(|| WireError::OutOfRange {
                        at: w.position(),
                        expected: String::from("a TypeCode indirection offset that fits a long"),
                    })?;
                w.write_i32(offset);
                Ok(())
            }
        }
    }

    /// Reads a `TypeCode` (§15.3.5) including **indirection** (§15.3.5.1):
    /// a recursive or repeated `TypeCode` (`0xffffffff` and a negative
    /// offset) is resolved — a repeated one by reading its encoding again
    /// at the place of the indirection, a recursive one as
    /// [`TypeCode::Recursive`].
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for an unknown `TCKind`, an indirection
    /// that resolves to nothing, a `TypeCode` nested deeper than
    /// [`MAX_TYPECODE_DEPTH`] and one whose repeated `TypeCode`s read more
    /// than [`MAX_TYPECODE_REREAD_FACTOR`] times its own octets again, and
    /// whatever the reader raises for a short or malformed stream.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
        // What the indirections to repeated `TypeCode`s may read again:
        // a multiple of the octets of this one.
        let mut cache = TcCache::new(encoded_octets(r).saturating_mul(MAX_TYPECODE_REREAD_FACTOR));
        // Base 0: cache positions are this reader's own offsets.
        Self::decode_ctx(r, 0, &mut cache, 0)
    }

    /// Reads a top-level `TypeCode` of a message whose earlier top-level
    /// `TypeCode`s `tops` holds (CORBA 3.3 Part 2 §9.3.5.1.6): under GIOP
    /// 1.3 and above a top-level `TypeCode` may be an indirection to an
    /// earlier top-level one of the same message, and is then a copy of
    /// it; below 1.3 it may not. An indirection nested in this `TypeCode`
    /// stays inside it, as in [`Self::decode`]. `r` is one reader over the
    /// message (or over a body whose positions are those of the message),
    /// so that the offsets of the indirections count across the values in
    /// between.
    ///
    /// # Errors
    /// As [`Self::decode`]; [`WireError::OutOfRange`] for a top-level
    /// indirection when `tops` does not allow one, and for one whose
    /// target is not the start of an earlier top-level `TypeCode` ("cannot
    /// reference TypeCodes nested within some other top-level TypeCode").
    pub fn decode_top_level(
        r: &mut Reader<'_>,
        tops: &mut TopLevelTypeCodes,
    ) -> Result<Self, WireError> {
        use tckind::INDIRECTION;
        r.align_to(4)?;
        let position = r.position();
        let mut peek = r.clone();
        if peek.read_u32()? != INDIRECTION {
            let tc = Self::decode(r)?;
            let octets = r.position() - position;
            tops.by_position.insert(position, (tc.clone(), octets));
            return Ok(tc);
        }
        if !tops.allow_indirection {
            return Err(WireError::OutOfRange {
                at: position,
                expected: String::from(
                    "a top-level TypeCode, not an indirection: below GIOP 1.3 an \
                     indirection stays inside one top-level TypeCode",
                ),
            });
        }
        r.read_u32()?;
        let off_field_pos = r.position();
        #[allow(clippy::cast_possible_wrap)]
        let offset = r.read_u32()? as i32;
        let target = i64::try_from(off_field_pos)
            .ok()
            .map(|p| p + i64::from(offset))
            .and_then(|t| usize::try_from(t).ok())
            .filter(|_| offset < 0);
        match target.and_then(|t| tops.by_position.get(&t)) {
            Some((tc, octets)) => {
                // A copy of the earlier one: the copies of a message
                // together within a multiple of the octets read so far.
                let copied = tops.copied.saturating_add(*octets);
                if copied > (off_field_pos + 4).saturating_mul(MAX_TYPECODE_REREAD_FACTOR) {
                    return Err(WireError::OutOfRange {
                        at: off_field_pos,
                        expected: format!(
                            "top-level TypeCode indirections that copy at most \
                             {MAX_TYPECODE_REREAD_FACTOR} times the octets of the message \
                             read up to them"
                        ),
                    });
                }
                tops.copied = copied;
                Ok(tc.clone())
            }
            None => Err(WireError::OutOfRange {
                at: off_field_pos,
                expected: String::from(
                    "a top-level TypeCode indirection to the TCKind of an earlier top-level \
                     TypeCode of the message",
                ),
            }),
        }
    }

    /// Position-tracking decode. `base` is the absolute stream offset of
    /// the reader's byte 0; `cache` maps `base + position` of a `TCKind`
    /// to the `TypeCode` decoded there, which is what makes a backward
    /// indirection resolvable. `depth` is the current nesting depth,
    /// capped by [`MAX_TYPECODE_DEPTH`].
    fn decode_ctx<'b>(
        r: &mut Reader<'b>,
        base: usize,
        cache: &mut TcCache<'b>,
        depth: u32,
    ) -> Result<Self, WireError> {
        use tckind::*;
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
            TK_PRINCIPAL => Ok(Self::Principal),
            TK_LONGDOUBLE => Ok(Self::LongDouble),
            TK_STRING => Ok(Self::String(r.read_u32()?)),
            TK_WSTRING => Ok(Self::WString(r.read_u32()?)),
            TK_FIXED => {
                let digits = r.read_u16()?;
                let scale = r.read_i16()?;
                Ok(Self::Fixed { digits, scale })
            }
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
                match cache.entries.get(&target) {
                    // A repeated `TypeCode`: its encapsulation read again
                    // here, at this depth, within what the decode may
                    // read again.
                    Some(TcCacheEntry::Done(encoded)) => {
                        let encoded = *encoded;
                        cache.read_again(encoded.bytes.len(), off_field_pos)?;
                        Self::decode_encoded(encoded, cache, depth)
                    }
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
            TK_SEQUENCE
            | TK_STRUCT
            | TK_EXCEPT
            | TK_ENUM
            | TK_ALIAS
            | TK_OBJREF
            | TK_NATIVE
            | TK_ABSTRACT_INTERFACE
            | TK_LOCAL_INTERFACE
            | TK_COMPONENT
            | TK_HOME
            | TK_ARRAY
            | TK_UNION
            | TK_VALUE
            | TK_EVENT
            | TK_VALUE_BOX => {
                let encoded = read_encapsulation(r, base, kind, tckind_pos)?;
                Self::decode_encoded(encoded, cache, depth)
            }
            other => Err(WireError::OutOfRange {
                at: tckind_pos,
                expected: format!("a TCKind this decoder knows, not {other}"),
            }),
        }
    }

    /// The `TypeCode` of a complex kind from its encapsulation
    /// (§9.3.5.1): the parameters in the byte order the encapsulation
    /// names, with alignment counted from its byte-order octet. Once read,
    /// an indirection to its `TCKind` reads the encapsulation again.
    fn decode_encoded<'b>(
        encoded: Encoded<'b>,
        cache: &mut TcCache<'b>,
        depth: u32,
    ) -> Result<Self, WireError> {
        // A reader over the WHOLE encapsulation, so alignment counts from
        // the byte-order octet; skip the octet and read the body in its
        // order.
        let mut e = Reader::new(encoded.bytes, encoded.endianness);
        let _bo = e.read_u8()?;
        let tc = Self::decode_params(&encoded, &mut e, cache, depth)?;
        cache
            .entries
            .insert(encoded.tckind_pos, TcCacheEntry::Done(encoded));
        Ok(tc)
    }

    /// The parameters of the complex `TypeCode` `encoded` names, read
    /// from `e`, the reader over its encapsulation after the byte-order
    /// octet.
    fn decode_params<'b>(
        encoded: &Encoded<'b>,
        e: &mut Reader<'b>,
        cache: &mut TcCache<'b>,
        depth: u32,
    ) -> Result<Self, WireError> {
        use tckind::*;
        let kind = encoded.kind;
        let child_base = encoded.child_base;
        let tckind_pos = encoded.tckind_pos;
        match kind {
            TK_SEQUENCE => {
                let element = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                let bound = e.read_u32()?;
                Ok(Self::Sequence { element, bound })
            }
            TK_STRUCT | TK_EXCEPT => {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                // The placeholder goes in *before* the members: a member
                // that indirects back to this struct resolves to
                // Recursive rather than to nothing.
                cache
                    .entries
                    .insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let count = e.read_u32()? as usize;
                let mut members = Vec::with_capacity(count.min(256));
                for _ in 0..count {
                    let member_name = e.read_string()?;
                    let member_type = Self::decode_ctx(e, child_base, cache, depth + 1)?;
                    members.push((member_name, member_type));
                }
                Ok(Self::Struct {
                    repo_id,
                    name,
                    members,
                    is_except: kind == TK_EXCEPT,
                })
            }
            TK_ENUM => {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                let count = e.read_u32()? as usize;
                let mut members = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    members.push(e.read_string()?);
                }
                Ok(Self::Enum {
                    repo_id,
                    name,
                    members,
                })
            }
            TK_ALIAS => {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                cache
                    .entries
                    .insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let content = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                Ok(Self::Alias {
                    repo_id,
                    name,
                    content,
                })
            }
            TK_OBJREF
            | TK_NATIVE
            | TK_ABSTRACT_INTERFACE
            | TK_LOCAL_INTERFACE
            | TK_COMPONENT
            | TK_HOME => {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                Ok(match kind {
                    TK_OBJREF => Self::ObjRef { repo_id, name },
                    TK_NATIVE => Self::Native { repo_id, name },
                    TK_ABSTRACT_INTERFACE => Self::AbstractInterface { repo_id, name },
                    TK_LOCAL_INTERFACE => Self::LocalInterface { repo_id, name },
                    TK_COMPONENT => Self::Component { repo_id, name },
                    _ => Self::Home { repo_id, name },
                })
            }
            TK_ARRAY => {
                let element = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                let length = e.read_u32()?;
                Ok(Self::Array { element, length })
            }
            TK_UNION => {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                cache
                    .entries
                    .insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let discriminator = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                let default_index = e.read_i32()?;
                let count = bounded_count(e, 1)?;
                let mut members = Vec::with_capacity(count.min(256));
                for _ in 0..count {
                    let label = UnionLabel::decode(&discriminator, e)?;
                    let member_name = e.read_string()?;
                    let member_type = Self::decode_ctx(e, child_base, cache, depth + 1)?;
                    members.push(UnionMember {
                        label,
                        name: member_name,
                        member_type,
                    });
                }
                if usize::try_from(default_index).is_ok_and(|i| i >= members.len()) {
                    return Err(WireError::OutOfRange {
                        at: e.position(),
                        expected: format!(
                            "a union default index below the case count {}, not {default_index}",
                            members.len()
                        ),
                    });
                }
                Ok(Self::Union {
                    repo_id,
                    name,
                    discriminator,
                    default_index,
                    members,
                })
            }
            TK_VALUE | TK_EVENT => {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                let modifier = e.read_i16()?;
                cache
                    .entries
                    .insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let concrete_base = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                let count = bounded_count(e, 4)?;
                let mut members = Vec::with_capacity(count.min(256));
                for _ in 0..count {
                    let member_name = e.read_string()?;
                    let member_type = Self::decode_ctx(e, child_base, cache, depth + 1)?;
                    let visibility = e.read_i16()?;
                    members.push(ValueMember {
                        name: member_name,
                        member_type,
                        visibility,
                    });
                }
                Ok(Self::Value {
                    repo_id,
                    name,
                    modifier,
                    concrete_base,
                    members,
                    is_event: kind == TK_EVENT,
                })
            }
            TK_VALUE_BOX => {
                let repo_id = e.read_string()?;
                let name = e.read_string()?;
                cache
                    .entries
                    .insert(tckind_pos, TcCacheEntry::InProgress(repo_id.clone()));
                let content = Box::new(Self::decode_ctx(e, child_base, cache, depth + 1)?);
                Ok(Self::ValueBox {
                    repo_id,
                    name,
                    content,
                })
            }
            other => Err(WireError::OutOfRange {
                at: tckind_pos,
                expected: format!("a TCKind with an encapsulation, not {other}"),
            }),
        }
    }
}

/// The top-level `TypeCode`s read so far in one GIOP message, by the
/// position of their `TCKind` (CORBA 3.3 Part 2 §9.3.5.1.6), for
/// [`TypeCode::decode_top_level`].
#[derive(Debug, Clone, Default)]
pub struct TopLevelTypeCodes {
    allow_indirection: bool,
    /// Each top-level `TypeCode` and the octets of its encoding.
    by_position: BTreeMap<usize, (TypeCode, usize)>,
    /// The octets of the `TypeCode`s the top-level indirections copied.
    copied: usize,
}

impl TopLevelTypeCodes {
    /// An empty set for a message whose GIOP version allows an
    /// indirection from one top-level `TypeCode` to another
    /// (`allow_indirection`, GIOP 1.3 and above,
    /// `zerodds_corba_giop::Version::allows_indirection_between_top_level_typecodes`)
    /// or does not.
    #[must_use]
    pub fn new(allow_indirection: bool) -> Self {
        Self {
            allow_indirection,
            by_position: BTreeMap::new(),
            copied: 0,
        }
    }
}

/// A cache entry for indirection resolution (§15.3.5.1).
enum TcCacheEntry<'b> {
    /// A complex `TypeCode` still being decoded — an indirection to it is
    /// recursive and yields [`TypeCode::Recursive`].
    InProgress(String),
    /// Fully decoded — an indirection to it is a repeated `TypeCode`,
    /// read again from its encapsulation.
    Done(Encoded<'b>),
}

/// The encapsulation of a complex `TypeCode` (§9.3.5.1) and where it
/// stands in the stream.
#[derive(Clone, Copy)]
struct Encoded<'b> {
    /// The `TCKind`.
    kind: u32,
    /// The encapsulation, its byte-order octet first.
    bytes: &'b [u8],
    /// The byte order the byte-order octet names.
    endianness: Endianness,
    /// The absolute stream offset of the byte-order octet.
    child_base: usize,
    /// The absolute stream offset of the `TCKind`.
    tckind_pos: usize,
}

/// The state of one `TypeCode` decode: the position cache (stream offset
/// of a `TCKind` to its entry) and the octets the indirections to
/// repeated `TypeCode`s may still read again.
struct TcCache<'b> {
    entries: BTreeMap<usize, TcCacheEntry<'b>>,
    read_again_left: usize,
}

impl TcCache<'_> {
    /// An empty cache whose repeated `TypeCode`s may read `budget`
    /// octets again.
    fn new(budget: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            read_again_left: budget,
        }
    }

    /// Takes `octets` from what the decode may read again, for the
    /// indirection whose offset field is at `at`.
    fn read_again(&mut self, octets: usize, at: usize) -> Result<(), WireError> {
        self.read_again_left =
            self.read_again_left
                .checked_sub(octets)
                .ok_or_else(|| WireError::OutOfRange {
                    at,
                    expected: format!(
                        "repeated TypeCodes that read at most {MAX_TYPECODE_REREAD_FACTOR} times \
                     the octets of the TypeCode again"
                    ),
                })?;
        Ok(())
    }
}

/// The octets of the `TypeCode` that starts at `r`, as far as its
/// `TCKind` and the length of its encapsulation tell: the `TCKind`, the
/// length and the encapsulation of a complex kind, capped by what the
/// stream holds; 0 for any other kind (it has no indirection inside) and
/// for a stream too short to tell.
fn encoded_octets(r: &Reader<'_>) -> usize {
    use tckind::*;
    let mut peek = r.clone();
    if peek.align_to(4).is_err() {
        return 0;
    }
    match peek.read_u32() {
        Ok(
            TK_SEQUENCE
            | TK_STRUCT
            | TK_EXCEPT
            | TK_ENUM
            | TK_ALIAS
            | TK_OBJREF
            | TK_NATIVE
            | TK_ABSTRACT_INTERFACE
            | TK_LOCAL_INTERFACE
            | TK_COMPONENT
            | TK_HOME
            | TK_ARRAY
            | TK_UNION
            | TK_VALUE
            | TK_EVENT
            | TK_VALUE_BOX,
        ) => peek
            .read_u32()
            .map(|len| (len as usize).min(peek.remaining()).saturating_add(8))
            .unwrap_or(0),
        _ => 0,
    }
}

/// A tuple count that the rest of the encapsulation can hold, each tuple
/// taking at least `min_octets`.
fn bounded_count(r: &mut Reader<'_>, min_octets: usize) -> Result<usize, WireError> {
    let at = r.position();
    let count = r.read_u32()? as usize;
    if count > r.remaining() / min_octets.max(1) {
        return Err(WireError::ImpossibleLength {
            at,
            claimed: count,
            available: r.remaining(),
        });
    }
    Ok(count)
}

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

/// Writes the encapsulation of a complex `TypeCode`'s parameters. The
/// length goes at the next four-aligned position of `w`, the
/// encapsulation's byte 0 right after it; `body` gets that byte's
/// absolute offset so that an indirection inside it can be counted
/// across the encapsulation boundary.
fn encap_ctx<F>(
    w: &mut Writer,
    base: usize,
    open: &mut Vec<(String, usize)>,
    body: F,
) -> Result<(), WireError>
where
    F: FnOnce(&mut Writer, usize, &mut Vec<(String, usize)>) -> Result<(), WireError>,
{
    w.align_to(4);
    let child_base = base + w.position() + 4;
    let encap = build_encap(w.endianness(), |e| body(e, child_base, open))?;
    write_encap(w, &encap)
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

/// Reads the encapsulation (`sequence<octet>`) of a complex `TypeCode`
/// whose `TCKind` `kind` stood at `tckind_pos`, and the byte order its
/// byte-order octet names.
///
/// The encapsulation keeps the **absolute stream offset** of its byte 0
/// (`child_base`), so that an indirection inside it can resolve to a
/// `TCKind` outside it.
fn read_encapsulation<'b>(
    r: &mut Reader<'b>,
    base: usize,
    kind: u32,
    tckind_pos: usize,
) -> Result<Encoded<'b>, WireError> {
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
    Ok(Encoded {
        kind,
        bytes,
        endianness,
        child_base,
        tckind_pos,
    })
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

    fn sample_union() -> TypeCode {
        TypeCode::Union {
            repo_id: "IDL:U:1.0".into(),
            name: "U".into(),
            discriminator: Box::new(TypeCode::Short),
            default_index: 2,
            members: vec![
                UnionMember {
                    label: UnionLabel::Short(1),
                    name: "a".into(),
                    member_type: TypeCode::Long,
                },
                UnionMember {
                    label: UnionLabel::Short(2),
                    name: "b".into(),
                    member_type: TypeCode::String(0),
                },
                UnionMember {
                    label: UnionLabel::Short(0),
                    name: "c".into(),
                    member_type: TypeCode::Double,
                },
            ],
        }
    }

    fn sample_value() -> TypeCode {
        TypeCode::Value {
            repo_id: "IDL:Horse:1.0".into(),
            name: "Horse".into(),
            modifier: value_modifier::VM_TRUNCATABLE,
            concrete_base: Box::new(TypeCode::Value {
                repo_id: "IDL:Animal:1.0".into(),
                name: "Animal".into(),
                modifier: value_modifier::VM_NONE,
                concrete_base: Box::new(TypeCode::Null),
                members: vec![ValueMember {
                    name: "legs".into(),
                    member_type: TypeCode::Short,
                    visibility: visibility::PUBLIC_MEMBER,
                }],
                is_event: false,
            }),
            members: vec![ValueMember {
                name: "mane".into(),
                member_type: TypeCode::String(0),
                visibility: visibility::PRIVATE_MEMBER,
            }],
            is_event: false,
        }
    }

    #[test]
    fn corba_p2_9_3_5_every_tckind_round_trips() {
        let named = |repo: &str| (String::from(repo), String::from("N"));
        let (r, n) = named("IDL:I:1.0");
        let all = [
            TypeCode::Null,
            TypeCode::Void,
            TypeCode::Short,
            TypeCode::Long,
            TypeCode::UShort,
            TypeCode::ULong,
            TypeCode::Float,
            TypeCode::Double,
            TypeCode::Boolean,
            TypeCode::Char,
            TypeCode::Octet,
            TypeCode::Any,
            TypeCode::TypeCodeTc,
            TypeCode::Principal,
            TypeCode::ObjRef {
                repo_id: r.clone(),
                name: n.clone(),
            },
            TypeCode::Struct {
                repo_id: "IDL:S:1.0".into(),
                name: "S".into(),
                members: vec![("x".into(), TypeCode::LongLong)],
                is_except: false,
            },
            sample_union(),
            TypeCode::Enum {
                repo_id: "IDL:E:1.0".into(),
                name: "E".into(),
                members: vec!["A".into(), "B".into()],
            },
            TypeCode::String(8),
            TypeCode::Sequence {
                element: Box::new(TypeCode::Octet),
                bound: 4,
            },
            TypeCode::Array {
                element: Box::new(TypeCode::Array {
                    element: Box::new(TypeCode::Float),
                    length: 3,
                }),
                length: 2,
            },
            TypeCode::Alias {
                repo_id: "IDL:A:1.0".into(),
                name: "A".into(),
                content: Box::new(TypeCode::ULongLong),
            },
            TypeCode::Struct {
                repo_id: "IDL:X:1.0".into(),
                name: "X".into(),
                members: vec![],
                is_except: true,
            },
            TypeCode::LongLong,
            TypeCode::ULongLong,
            TypeCode::LongDouble,
            TypeCode::WChar,
            TypeCode::WString(0),
            TypeCode::Fixed {
                digits: 10,
                scale: 2,
            },
            sample_value(),
            TypeCode::ValueBox {
                repo_id: "IDL:B:1.0".into(),
                name: "B".into(),
                content: Box::new(TypeCode::WString(0)),
            },
            TypeCode::Native {
                repo_id: r.clone(),
                name: n.clone(),
            },
            TypeCode::AbstractInterface {
                repo_id: r.clone(),
                name: n.clone(),
            },
            TypeCode::LocalInterface {
                repo_id: r.clone(),
                name: n.clone(),
            },
            TypeCode::Component {
                repo_id: r.clone(),
                name: n.clone(),
            },
            TypeCode::Home {
                repo_id: r.clone(),
                name: n.clone(),
            },
            TypeCode::Value {
                repo_id: "IDL:Ev:1.0".into(),
                name: "Ev".into(),
                modifier: value_modifier::VM_NONE,
                concrete_base: Box::new(TypeCode::Null),
                members: vec![],
                is_event: true,
            },
        ];
        // Table 9.2: TCKind 0 to 36 in order.
        let kinds: Vec<u32> = all.iter().map(TypeCode::tckind).collect();
        assert_eq!(kinds, (0..=36).collect::<Vec<u32>>());
        for tc in &all {
            rt(tc, Endianness::Big);
            rt(tc, Endianness::Little);
        }
    }

    #[test]
    fn corba_p2_9_3_5_1_simple_parameters_are_inline() {
        let mut w = Writer::new(Endianness::Big);
        TypeCode::Fixed {
            digits: 5,
            scale: -1,
        }
        .encode(&mut w)
        .unwrap();
        assert_eq!(w.finish(), [0, 0, 0, 28, 0, 5, 0xff, 0xff]);
        let mut w = Writer::new(Endianness::Little);
        TypeCode::Principal.encode(&mut w).unwrap();
        TypeCode::LongDouble.encode(&mut w).unwrap();
        assert_eq!(w.finish(), [13, 0, 0, 0, 25, 0, 0, 0]);
    }

    #[test]
    fn corba_p2_9_3_5_1_union_parameters_follow_table_9_2() {
        let tc = TypeCode::Union {
            repo_id: "IDL:U:1.0".into(),
            name: "U".into(),
            discriminator: Box::new(TypeCode::Char),
            default_index: -1,
            members: vec![UnionMember {
                label: UnionLabel::Char(b'x'),
                name: "m".into(),
                member_type: TypeCode::Octet,
            }],
        };
        let mut w = Writer::new(Endianness::Big);
        tc.encode(&mut w).unwrap();
        let bytes = w.finish();
        let mut expected = vec![0, 0, 0, 16];
        let mut e = Writer::new(Endianness::Big);
        e.write_u8(0);
        e.write_string("IDL:U:1.0");
        e.write_string("U");
        e.write_u32(9); // discriminant type tk_char
        e.write_i32(-1); // no default case
        e.write_u32(1);
        e.write_u8(b'x'); // the label as a char
        e.write_string("m");
        e.write_u32(10); // tk_octet
        let encap = e.finish();
        expected.extend_from_slice(&u32::try_from(encap.len()).unwrap().to_be_bytes());
        expected.extend_from_slice(&encap);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn corba_p2_9_3_5_1_a_union_label_of_another_type_is_refused() {
        let tc = TypeCode::Union {
            repo_id: "IDL:U:1.0".into(),
            name: "U".into(),
            discriminator: Box::new(TypeCode::Long),
            default_index: -1,
            members: vec![UnionMember {
                label: UnionLabel::Short(1),
                name: "m".into(),
                member_type: TypeCode::Octet,
            }],
        };
        let mut w = Writer::new(Endianness::Big);
        assert!(matches!(
            tc.encode(&mut w),
            Err(WireError::OutOfRange { .. })
        ));
        // A discriminant type no union can have.
        let mut w = Writer::new(Endianness::Big);
        let mut bad = Writer::new(Endianness::Big);
        bad.write_u8(0);
        bad.write_string("IDL:U:1.0");
        bad.write_string("U");
        bad.write_u32(tckind::TK_DOUBLE);
        bad.write_i32(-1);
        bad.write_u32(1);
        bad.write_f64(1.0);
        let encap = bad.finish();
        w.write_u32(tckind::TK_UNION);
        w.write_u32(u32::try_from(encap.len()).unwrap());
        w.write_bytes(&encap);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert!(matches!(
            TypeCode::decode(&mut r),
            Err(WireError::OutOfRange { .. })
        ));
    }

    /// Part 2 §9.3.5.1.6: "For GIOP 1.3 and above, the indirection applies
    /// only to TypeCodes nested within some top-level TypeCode, or from one
    /// top-level TypeCode to another"; "Indirected top-level TypeCodes can
    /// reference other top-level TypeCodes but cannot reference TypeCodes
    /// nested within some other top-level TypeCode". Two top-level
    /// TypeCodes in one stream, the second an indirection.
    #[test]
    fn corba_p2_9_3_5_indirection_between_top_level_typecodes_under_1_3() {
        let point = TypeCode::Struct {
            repo_id: String::from("IDL:demo/Point:1.0"),
            name: String::from("Point"),
            members: alloc::vec![
                (String::from("x"), TypeCode::Long),
                (String::from("label"), TypeCode::String(0)),
            ],
            is_except: false,
        };
        // `target` is the position the indirection points to.
        let stream = |target: usize| {
            let mut w = Writer::new(Endianness::Big);
            point.encode(&mut w).unwrap();
            w.write_u8(7); // a value between the two TypeCodes
            w.align_to(4);
            w.write_u32(tckind::INDIRECTION);
            let field = w.position();
            #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
            w.write_u32((target as i64 - field as i64) as i32 as u32);
            w.finish()
        };
        let read_both = |bytes: &[u8], allow: bool| {
            let mut r = Reader::new(bytes, Endianness::Big);
            let mut tops = TopLevelTypeCodes::new(allow);
            let first = TypeCode::decode_top_level(&mut r, &mut tops)?;
            r.read_u8()?;
            let second = TypeCode::decode_top_level(&mut r, &mut tops)?;
            Ok::<_, WireError>((first, second))
        };
        // GIOP 1.3 and above: the second is a copy of the first.
        let (first, second) = read_both(&stream(0), true).unwrap();
        assert_eq!(first, point);
        assert_eq!(second, point);
        // Below 1.3 a top-level indirection is refused.
        assert!(read_both(&stream(0), false).is_err());
        // The TCKind of the nested `long` member is no top-level TypeCode.
        let mut w = Writer::new(Endianness::Big);
        point.encode(&mut w).unwrap();
        let bytes = w.finish();
        let nested = (4..bytes.len() - 3)
            .step_by(4)
            .find(|&i| bytes[i..i + 4] == tckind::TK_LONG.to_be_bytes())
            .unwrap();
        assert!(read_both(&stream(nested), true).is_err());
    }

    #[test]
    fn corba_p2_9_3_5_1_a_recursive_typecode_is_written_with_an_indirection() {
        // struct Node { sequence<Node> kids; }
        let node = TypeCode::Struct {
            repo_id: "IDL:Node:1.0".into(),
            name: "Node".into(),
            members: vec![(
                "kids".into(),
                TypeCode::Sequence {
                    element: Box::new(TypeCode::Recursive {
                        repo_id: "IDL:Node:1.0".into(),
                    }),
                    bound: 0,
                },
            )],
            is_except: false,
        };
        let mut w = Writer::new(Endianness::Big);
        node.encode(&mut w).unwrap();
        let bytes = w.finish();
        // TCKind at 0, struct encapsulation from 8; the sequence TCKind at
        // 60, its encapsulation from 68; the element at 72 is the
        // indirection, its offset field at 76 points back to 0.
        assert_eq!(&bytes[60..64], &[0, 0, 0, 19]);
        assert_eq!(&bytes[72..76], &[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(&bytes[76..80], &(-76_i32).to_be_bytes());
        for e in [Endianness::Big, Endianness::Little] {
            rt(&node, e);
        }
        // Recursion through a union and a value type as well.
        let list = TypeCode::Value {
            repo_id: "IDL:List:1.0".into(),
            name: "List".into(),
            modifier: value_modifier::VM_NONE,
            concrete_base: Box::new(TypeCode::Null),
            members: vec![ValueMember {
                name: "next".into(),
                member_type: TypeCode::Recursive {
                    repo_id: "IDL:List:1.0".into(),
                },
                visibility: visibility::PUBLIC_MEMBER,
            }],
            is_event: false,
        };
        let tree = TypeCode::Union {
            repo_id: "IDL:Tree:1.0".into(),
            name: "Tree".into(),
            discriminator: Box::new(TypeCode::Boolean),
            default_index: -1,
            members: vec![UnionMember {
                label: UnionLabel::Boolean(true),
                name: "children".into(),
                member_type: TypeCode::Sequence {
                    element: Box::new(TypeCode::Recursive {
                        repo_id: "IDL:Tree:1.0".into(),
                    }),
                    bound: 0,
                },
            }],
        };
        for tc in [list, tree] {
            rt(&tc, Endianness::Little);
            rt(&tc, Endianness::Big);
        }
    }
}
