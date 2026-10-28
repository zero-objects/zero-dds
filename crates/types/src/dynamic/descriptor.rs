// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a type is, as §7.5.2.4 and Annex C name it.
//!
//! [`TypeKind`] is the enumeration of Annex C's `TypeKind` constants and [`TypeDescriptor`] is
//! §7.5.2.4's record. Both are **read off the model**: a
//! [`TypeDescriptor`] is computed from a [`zerodds_ast::TypeNode`] when
//! it is asked for, and is not a second place a type's shape is stored.
//!
//! ## Why the descriptor holds handles and not descriptors
//!
//! §7.5.2.4 gives `TypeDescriptor` the fields `base_type`,
//! `discriminator_type`, `element_type` and `key_element_type`, and each
//! of them is a `DynamicType` — not a nested descriptor. The module this
//! one replaces boxed a whole descriptor tree into each of those fields,
//! which made a type's shape exist twice: once in the tree the builder
//! had produced and once in the descriptor a caller read back. The two
//! could disagree, and nothing would say so.
//!
//! Here they are [`super::DynamicType`] handles, which are a pointer into
//! the model and a node id. Reading one costs nothing and cannot drift.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_ast::{Extensibility, PrimitiveKind as AstPrimitive, TryConstruct};

use super::type_::DynamicType;

/// Whether `text` is an IDL identifier: a letter or an underscore, then
/// letters, digits and underscores.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A member id. §7.3.1.1 — 32 bits.
pub type MemberId = u32;

/// The member id §7.5.2.1 gives a member whose id is not known.
///
/// `0x0FFF_FFFF`, which is `MEMBER_ID_INVALID` in the IDL of §7.5.2.5.
pub const MEMBER_ID_INVALID: MemberId = 0x0FFF_FFFF;

/// Annex C `TypeKind`: `typedef octet TypeKind`.
pub type TypeKindValue = u8;

/// Annex C `TK_NONE`.
pub const TK_NONE: TypeKindValue = 0x00;
/// Annex C `TK_BOOLEAN`.
pub const TK_BOOLEAN: TypeKindValue = 0x01;
/// Annex C `TK_BYTE`.
pub const TK_BYTE: TypeKindValue = 0x02;
/// Annex C `TK_INT16`.
pub const TK_INT16: TypeKindValue = 0x03;
/// Annex C `TK_INT32`.
pub const TK_INT32: TypeKindValue = 0x04;
/// Annex C `TK_INT64`.
pub const TK_INT64: TypeKindValue = 0x05;
/// Annex C `TK_UINT16`.
pub const TK_UINT16: TypeKindValue = 0x06;
/// Annex C `TK_UINT32`.
pub const TK_UINT32: TypeKindValue = 0x07;
/// Annex C `TK_UINT64`.
pub const TK_UINT64: TypeKindValue = 0x08;
/// Annex C `TK_FLOAT32`.
pub const TK_FLOAT32: TypeKindValue = 0x09;
/// Annex C `TK_FLOAT64`.
pub const TK_FLOAT64: TypeKindValue = 0x0A;
/// Annex C `TK_FLOAT128`.
pub const TK_FLOAT128: TypeKindValue = 0x0B;
/// Annex C `TK_INT8`.
pub const TK_INT8: TypeKindValue = 0x0C;
/// Annex C `TK_UINT8`.
pub const TK_UINT8: TypeKindValue = 0x0D;
/// Annex C `TK_CHAR8`.
pub const TK_CHAR8: TypeKindValue = 0x10;
/// Annex C `TK_CHAR16`.
pub const TK_CHAR16: TypeKindValue = 0x11;
/// Annex C `TK_STRING8`.
pub const TK_STRING8: TypeKindValue = 0x20;
/// Annex C `TK_STRING16`.
pub const TK_STRING16: TypeKindValue = 0x21;
/// Annex C `TK_ALIAS`.
pub const TK_ALIAS: TypeKindValue = 0x30;
/// Annex C `TK_ENUM`.
pub const TK_ENUM: TypeKindValue = 0x40;
/// Annex C `TK_BITMASK`.
pub const TK_BITMASK: TypeKindValue = 0x41;
/// Annex C `TK_ANNOTATION`.
pub const TK_ANNOTATION: TypeKindValue = 0x50;
/// Annex C `TK_STRUCTURE`.
pub const TK_STRUCTURE: TypeKindValue = 0x51;
/// Annex C `TK_UNION`.
pub const TK_UNION: TypeKindValue = 0x52;
/// Annex C `TK_BITSET`.
pub const TK_BITSET: TypeKindValue = 0x53;
/// Annex C `TK_SEQUENCE`.
pub const TK_SEQUENCE: TypeKindValue = 0x60;
/// Annex C `TK_ARRAY`.
pub const TK_ARRAY: TypeKindValue = 0x61;
/// Annex C `TK_MAP`.
pub const TK_MAP: TypeKindValue = 0x62;

/// What kind of type this is. Annex C.
///
/// Exhaustive against [`zerodds_ast::TypeKind`] for the kinds XTypes
/// defines. A model kind XTypes has no row for — `fixed`, `valuetype`, a
/// value box, `native`, `any`, `ValueBase`, an object reference, an
/// annotation declaration — maps to [`Self::NoType`], which is `TK_NONE`,
/// and every operation that needs a real kind refuses it by name rather
/// than treating it as one of the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TypeKind {
    /// `TK_NONE` — no type. Also what a construct XTypes does not
    /// tabulate answers.
    NoType,
    /// `boolean`.
    Boolean,
    /// `octet` / `byte`.
    Byte,
    /// `int8`.
    Int8,
    /// `uint8`.
    UInt8,
    /// `int16`.
    Int16,
    /// `uint16`.
    UInt16,
    /// `int32`.
    Int32,
    /// `uint32`.
    UInt32,
    /// `int64`.
    Int64,
    /// `uint64`.
    UInt64,
    /// `float32`.
    Float32,
    /// `float64`.
    Float64,
    /// `float128` — `long double`.
    Float128,
    /// `char`.
    Char8,
    /// `wchar`.
    Char16,
    /// `string<N>`.
    String8,
    /// `wstring<N>`.
    String16,
    /// An enumeration.
    Enumeration,
    /// A bitmask.
    Bitmask,
    /// A `typedef`.
    Alias,
    /// An array.
    Array,
    /// A `sequence`.
    Sequence,
    /// A `map`.
    Map,
    /// A `struct`.
    Structure,
    /// A `union`.
    Union,
    /// A `bitset`.
    Bitset,
    /// An annotation declaration.
    Annotation,
}

impl TypeKind {
    /// Every kind, in Annex C order.
    ///
    /// So that a conformance test can assert the set is complete rather
    /// than take the enumeration's word for it.
    pub const ALL: [Self; 28] = [
        Self::NoType,
        Self::Boolean,
        Self::Byte,
        Self::Int8,
        Self::UInt8,
        Self::Int16,
        Self::UInt16,
        Self::Int32,
        Self::UInt32,
        Self::Int64,
        Self::UInt64,
        Self::Float32,
        Self::Float64,
        Self::Float128,
        Self::Char8,
        Self::Char16,
        Self::String8,
        Self::String16,
        Self::Enumeration,
        Self::Bitmask,
        Self::Alias,
        Self::Array,
        Self::Sequence,
        Self::Map,
        Self::Structure,
        Self::Union,
        Self::Bitset,
        Self::Annotation,
    ];

    /// The kind of a model primitive.
    ///
    /// `octet` and `uint8` are two rows: §7.3.4.2 gives them the
    /// distinct type kinds `TK_BYTE` and `TK_UINT8`, which is what
    /// [`crate::from_ast`] announces, so the `TypeKind` keeps them
    /// apart too.
    #[must_use]
    pub const fn of_primitive(primitive: AstPrimitive) -> Self {
        match primitive {
            AstPrimitive::Boolean => Self::Boolean,
            AstPrimitive::Octet => Self::Byte,
            AstPrimitive::UInt8 => Self::UInt8,
            AstPrimitive::Char => Self::Char8,
            AstPrimitive::WChar => Self::Char16,
            AstPrimitive::Int8 => Self::Int8,
            AstPrimitive::Int16 => Self::Int16,
            AstPrimitive::UInt16 => Self::UInt16,
            AstPrimitive::Int32 => Self::Int32,
            AstPrimitive::UInt32 => Self::UInt32,
            AstPrimitive::Int64 => Self::Int64,
            AstPrimitive::UInt64 => Self::UInt64,
            AstPrimitive::Float32 => Self::Float32,
            AstPrimitive::Float64 => Self::Float64,
            AstPrimitive::Float128 => Self::Float128,
        }
    }

    /// The model primitive this kind stands for, where it stands for one.
    ///
    /// The exact inverse of [`Self::of_primitive`]: `TK_BYTE` is
    /// `octet` and `TK_UINT8` is `uint8`, the two kinds §7.3.4.2
    /// separates.
    #[must_use]
    pub const fn as_primitive(self) -> Option<AstPrimitive> {
        Some(match self {
            Self::Boolean => AstPrimitive::Boolean,
            Self::Byte => AstPrimitive::Octet,
            Self::UInt8 => AstPrimitive::UInt8,
            Self::Char8 => AstPrimitive::Char,
            Self::Char16 => AstPrimitive::WChar,
            Self::Int8 => AstPrimitive::Int8,
            Self::Int16 => AstPrimitive::Int16,
            Self::UInt16 => AstPrimitive::UInt16,
            Self::Int32 => AstPrimitive::Int32,
            Self::UInt32 => AstPrimitive::UInt32,
            Self::Int64 => AstPrimitive::Int64,
            Self::UInt64 => AstPrimitive::UInt64,
            Self::Float32 => AstPrimitive::Float32,
            Self::Float64 => AstPrimitive::Float64,
            Self::Float128 => AstPrimitive::Float128,
            _ => return None,
        })
    }

    /// Whether this is an atomic type — not a collection, not an
    /// aggregate, not an alias. §7.5.2.4.
    #[must_use]
    pub const fn is_primitive(self) -> bool {
        self.as_primitive().is_some()
    }

    /// Whether a type of this kind carries members a walk can enumerate.
    ///
    /// A `struct`, a `union` and an annotation declaration carry members;
    /// an enumeration carries enumerators, a bitmask carries bits and a
    /// bitset carries bitfields, and §7.5.2.8's `get_member_by_index`
    /// walks all six the same way.
    #[must_use]
    pub const fn is_aggregable(self) -> bool {
        matches!(
            self,
            Self::Structure
                | Self::Union
                | Self::Annotation
                | Self::Bitset
                | Self::Bitmask
                | Self::Enumeration
        )
    }

    /// Whether a type of this kind carries an element type.
    #[must_use]
    pub const fn is_collection(self) -> bool {
        matches!(self, Self::Array | Self::Sequence | Self::Map)
    }

    /// The number Annex C gives this kind: `TK_NONE = 0x00`,
    /// `TK_BOOLEAN = 0x01`, `TK_BYTE = 0x02`, `TK_INT16 = 0x03` and so on
    /// to `TK_MAP = 0x62` (`typedef octet TypeKind`).
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::NoType => TK_NONE,
            Self::Boolean => TK_BOOLEAN,
            Self::Byte => TK_BYTE,
            Self::Int16 => TK_INT16,
            Self::Int32 => TK_INT32,
            Self::Int64 => TK_INT64,
            Self::UInt16 => TK_UINT16,
            Self::UInt32 => TK_UINT32,
            Self::UInt64 => TK_UINT64,
            Self::Float32 => TK_FLOAT32,
            Self::Float64 => TK_FLOAT64,
            Self::Float128 => TK_FLOAT128,
            Self::Int8 => TK_INT8,
            Self::UInt8 => TK_UINT8,
            Self::Char8 => TK_CHAR8,
            Self::Char16 => TK_CHAR16,
            Self::String8 => TK_STRING8,
            Self::String16 => TK_STRING16,
            Self::Alias => TK_ALIAS,
            Self::Enumeration => TK_ENUM,
            Self::Bitmask => TK_BITMASK,
            Self::Annotation => TK_ANNOTATION,
            Self::Structure => TK_STRUCTURE,
            Self::Union => TK_UNION,
            Self::Bitset => TK_BITSET,
            Self::Sequence => TK_SEQUENCE,
            Self::Array => TK_ARRAY,
            Self::Map => TK_MAP,
        }
    }

    /// The kind a number of Annex C stands for.
    ///
    /// `None` for a number Annex C gives no constant. `TK_NONE` is
    /// [`Self::NoType`], a kind and an answer.
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        Some(match value {
            TK_NONE => Self::NoType,
            TK_BOOLEAN => Self::Boolean,
            TK_BYTE => Self::Byte,
            TK_INT16 => Self::Int16,
            TK_INT32 => Self::Int32,
            TK_INT64 => Self::Int64,
            TK_UINT16 => Self::UInt16,
            TK_UINT32 => Self::UInt32,
            TK_UINT64 => Self::UInt64,
            TK_FLOAT32 => Self::Float32,
            TK_FLOAT64 => Self::Float64,
            TK_FLOAT128 => Self::Float128,
            TK_INT8 => Self::Int8,
            TK_UINT8 => Self::UInt8,
            TK_CHAR8 => Self::Char8,
            TK_CHAR16 => Self::Char16,
            TK_STRING8 => Self::String8,
            TK_STRING16 => Self::String16,
            TK_ALIAS => Self::Alias,
            TK_ENUM => Self::Enumeration,
            TK_BITMASK => Self::Bitmask,
            TK_ANNOTATION => Self::Annotation,
            TK_STRUCTURE => Self::Structure,
            TK_UNION => Self::Union,
            TK_BITSET => Self::Bitset,
            TK_SEQUENCE => Self::Sequence,
            TK_ARRAY => Self::Array,
            TK_MAP => Self::Map,
            _ => return None,
        })
    }

    /// A name for diagnostics, in the spelling Annex C uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoType => "no type",
            Self::Boolean => "boolean",
            Self::Byte => "octet",
            Self::Int8 => "int8",
            Self::UInt8 => "uint8",
            Self::Int16 => "int16",
            Self::UInt16 => "uint16",
            Self::Int32 => "int32",
            Self::UInt32 => "uint32",
            Self::Int64 => "int64",
            Self::UInt64 => "uint64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::Float128 => "float128",
            Self::Char8 => "char8",
            Self::Char16 => "char16",
            Self::String8 => "string8",
            Self::String16 => "string16",
            Self::Enumeration => "an enumeration",
            Self::Bitmask => "a bitmask",
            Self::Alias => "an alias",
            Self::Array => "an array",
            Self::Sequence => "a sequence",
            Self::Map => "a map",
            Self::Structure => "a structure",
            Self::Union => "a union",
            Self::Bitset => "a bitset",
            Self::Annotation => "an annotation",
        }
    }
}

/// A type's extensibility. §7.2.4.4.
///
/// A rename of [`zerodds_ast::Extensibility`] and nothing more, so that
/// the reflective API can be used without naming the model's crate. The
/// two convert in both directions and neither carries a default: what an
/// aggregate whose source said nothing is taken to be is a **dialect**
/// answer, `zerodds_ast_layout::LayoutKey::default_extensibility`, and a
/// `Default` here would be a second answer to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExtensibilityKind {
    /// `@final`.
    Final,
    /// `@appendable`.
    Appendable,
    /// `@mutable`.
    Mutable,
}

impl ExtensibilityKind {
    /// Every extensibility XTypes defines.
    pub const ALL: [Self; 3] = [Self::Final, Self::Appendable, Self::Mutable];

    /// The source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::Appendable => "appendable",
            Self::Mutable => "mutable",
        }
    }
}

impl From<Extensibility> for ExtensibilityKind {
    fn from(value: Extensibility) -> Self {
        match value {
            Extensibility::Final => Self::Final,
            Extensibility::Appendable => Self::Appendable,
            Extensibility::Mutable => Self::Mutable,
        }
    }
}

impl From<ExtensibilityKind> for Extensibility {
    fn from(value: ExtensibilityKind) -> Self {
        match value {
            ExtensibilityKind::Final => Self::Final,
            ExtensibilityKind::Appendable => Self::Appendable,
            ExtensibilityKind::Mutable => Self::Mutable,
        }
    }
}

/// What a receiver does with a value it cannot construct. §7.2.2.7.
///
/// A rename of [`zerodds_ast::TryConstruct`], as [`ExtensibilityKind`] is
/// of [`zerodds_ast::Extensibility`].
///
/// **The behaviour is not applied here.** `zerodds_wire_codec::decode`
/// applies it at every position that has one — a string past its bound, a
/// sequence past its bound, an enumerator the receiver does not declare —
/// and this is the annotation as the model records it, so that a caller
/// reading a type can see what a peer will do. A second implementation of
/// the rule would be a second answer to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TryConstructKind {
    /// Discard the sample.
    Discard,
    /// Take the type's default value.
    UseDefault,
    /// Truncate to the receiver's bound.
    Trim,
}

impl TryConstructKind {
    /// Every behaviour XTypes defines.
    pub const ALL: [Self; 3] = [Self::Discard, Self::UseDefault, Self::Trim];

    /// The annotation-argument spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discard => "DISCARD",
            Self::UseDefault => "USE_DEFAULT",
            Self::Trim => "TRIM",
        }
    }
}

impl From<TryConstruct> for TryConstructKind {
    fn from(value: TryConstruct) -> Self {
        match value {
            TryConstruct::Discard => Self::Discard,
            TryConstruct::UseDefault => Self::UseDefault,
            TryConstruct::Trim => Self::Trim,
        }
    }
}

impl From<TryConstructKind> for TryConstruct {
    fn from(value: TryConstructKind) -> Self {
        match value {
            TryConstructKind::Discard => Self::Discard,
            TryConstructKind::UseDefault => Self::UseDefault,
            TryConstructKind::Trim => Self::Trim,
        }
    }
}

/// Everything §7.5.2.4 says about one type.
///
/// Read off the model when it is asked for. The four type-valued fields
/// are [`DynamicType`] handles into the same model, for the reason in the
/// module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDescriptor {
    /// §7.5.2.4 `kind`.
    pub kind: TypeKind,
    /// §7.5.2.4 `name` — the fully qualified name, without a leading
    /// `::`, which is the spelling `CompleteTypeDetail::type_name`
    /// carries and the one Cyclone, Fast DDS and RTI all write.
    pub name: String,
    /// §7.5.2.4.1 `base_type` — the type a `struct` or a `bitset` extends,
    /// and the type an alias names.
    pub base_type: Option<DynamicType>,
    /// §7.5.2.4 `discriminator_type` — a union's discriminator.
    pub discriminator_type: Option<DynamicType>,
    /// §7.5.2.4.2 `bound` — an array's dimensions, outermost first, or the
    /// one bound of a sequence, a string, a map or a bitmask.
    ///
    /// An unbounded sequence, string or map has the one bound
    /// [`super::LENGTH_UNLIMITED`], as Table 50 asks ("the length of the
    /// property value is one") and §7.5.2.2.3 to §7.5.2.2.5 name the
    /// value that makes a collection unbounded. It is not zero:
    /// §7.3.4.6 writes zero on the wire for "no bound", and a reader
    /// that saw `[0]` here could not tell that from a bound somebody
    /// declared as zero. A descriptor with an empty list for such a kind
    /// is read as unbounded too.
    pub bound: Vec<u32>,
    /// §7.5.2.4.5 `element_type` — the element of an array or a sequence,
    /// the value of a map, Char8 or Char16 for a string, Boolean for a
    /// bitmask.
    pub element_type: Option<DynamicType>,
    /// §7.5.2.4 `key_element_type` — a map's key.
    pub key_element_type: Option<DynamicType>,
    /// §7.5.2.4 `extensibility_kind`, for the kinds that have one.
    ///
    /// `None` for a type whose kind has no extensibility — a primitive, a
    /// collection, an alias — and never a substituted `Appendable`.
    pub extensibility_kind: Option<ExtensibilityKind>,
    /// §7.5.2.4 `is_nested` — `@nested`.
    pub is_nested: bool,
    /// The `@bit_bound` of an enumeration.
    ///
    /// Not one of §7.5.2.4's fields; the clause carries the bound in the
    /// `TypeObject` header instead. It is here because a caller walking an
    /// enumeration needs it to know which values exist, and reaching back
    /// into the model for it is the drift this record exists to avoid. A
    /// bitmask's bound is [`Self::bound`] (§7.5.2.4.2).
    pub bit_bound: Option<u16>,
}

impl TypeDescriptor {
    /// §7.5.2.4.3 `copy_from(other)`: overwrite this descriptor so that
    /// `equals(other)` holds. `other` is not changed.
    pub fn copy_from(&mut self, other: &Self) {
        self.clone_from(other);
    }

    /// §7.5.2.4.6 `equals(other)`: "the values of all of the properties
    /// identified in Table 50 above are equal in each of them". The
    /// type-valued properties are compared as types
    /// ([`DynamicType::equals`]).
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        self == other
    }

    /// Whether the record contradicts itself. §7.5.2.4's consistency
    /// rules.
    ///
    /// # Errors
    /// [`super::DynamicError::Inconsistent`], naming what is missing.
    pub fn is_consistent(&self) -> Result<(), super::DynamicError> {
        use super::DynamicError;
        use alloc::format;
        let kind = self.kind;
        // §7.5.2.4.10: "To be consistent, this name must be a valid
        // identifier for the given type kind". A named type has a name of
        // identifiers separated by `::`; an anonymous collection or string
        // has none that matters, and a bitmask made by `create_bitmask_type`
        // has none until it is given one.
        let named_kind = matches!(
            kind,
            TypeKind::Structure
                | TypeKind::Union
                | TypeKind::Enumeration
                | TypeKind::Alias
                | TypeKind::Bitset
                | TypeKind::Annotation
        );
        let well_formed = self.name.split("::").all(is_identifier);
        if (named_kind && !well_formed)
            || (!self.name.is_empty() && kind == TypeKind::Bitmask && !well_formed)
        {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is no name of {}",
                self.name,
                kind.as_str()
            )));
        }
        // §7.5.2.4: for each property, the kinds that carry it; for every
        // other kind "a consistent descriptor shall have a nil value".
        if self.discriminator_type.is_some() != matches!(kind, TypeKind::Union) {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is {} {} a discriminator type",
                self.name,
                kind.as_str(),
                if matches!(kind, TypeKind::Union) {
                    "with no"
                } else {
                    "and has"
                }
            )));
        }
        if self.base_type.is_none() && matches!(kind, TypeKind::Alias) {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is an alias with no base type",
                self.name
            )));
        }
        if self.base_type.is_some()
            && !matches!(
                kind,
                TypeKind::Structure | TypeKind::Alias | TypeKind::Bitset
            )
        {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is {} and has a base type",
                self.name,
                kind.as_str()
            )));
        }
        let has_element = matches!(
            kind,
            TypeKind::Sequence
                | TypeKind::Array
                | TypeKind::Map
                | TypeKind::String8
                | TypeKind::String16
                | TypeKind::Bitmask
        );
        if self.element_type.is_some() != has_element {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is {} {} an element type",
                self.name,
                kind.as_str(),
                if has_element { "with no" } else { "and has" }
            )));
        }
        if self.key_element_type.is_some() != matches!(kind, TypeKind::Map) {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is {} {} a key type",
                self.name,
                kind.as_str(),
                if matches!(kind, TypeKind::Map) {
                    "with no"
                } else {
                    "and has"
                }
            )));
        }
        match kind {
            TypeKind::Array if self.bound.is_empty() => {
                return Err(DynamicError::inconsistent(format!(
                    "`{}` is an array with no dimensions",
                    self.name
                )));
            }
            TypeKind::Array => {}
            // One bound, or none for an unbounded collection.
            TypeKind::Sequence
            | TypeKind::Map
            | TypeKind::Bitmask
            | TypeKind::String8
            | TypeKind::String16
                if self.bound.len() <= 1 => {}
            _ if self.bound.is_empty() => {}
            _ => {
                return Err(DynamicError::inconsistent(format!(
                    "`{}` is {} with a bound it does not take",
                    self.name,
                    kind.as_str()
                )));
            }
        }
        Ok(())
    }
}

/// Everything §7.5.2 says about one member of an aggregate.
///
/// Read off the model when it is asked for, as [`TypeDescriptor`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDescriptor {
    /// §7.5.2.1 `name`.
    pub name: String,
    /// §7.5.2.1 `id` — the **effective** member id, which is
    /// `zerodds_ast::MemberIdAssignment::effective`: `@id`, `@hashid`,
    /// `@autoid` or the previous member plus one, whichever the frontend
    /// decided. Not decided again here.
    pub id: MemberId,
    /// §7.5.2.1 `type` — the member's own type.
    pub type_: DynamicType,
    /// §7.5.2.1 `index` — the member's position in declaration order.
    pub index: u32,
    /// §7.5.2.7.8 `label` — the case labels that select this member, for a
    /// union member (`Int64 [*]`, Table 53).
    ///
    /// Empty for a member of a `struct`, and for a union member reached
    /// only through `default:` — [`Self::is_default_label`] is what says
    /// that apart from a case with no labels at all.
    pub label: Vec<i64>,
    /// §7.5.2.1 `default_label` — whether `default:` selects this member.
    pub is_default_label: bool,
    /// §7.5.2.1 `try_construct_kind`.
    pub try_construct_kind: TryConstructKind,
    /// §7.5.2.1 `is_key` — `@key`.
    pub is_key: bool,
    /// §7.5.2.1 `is_optional` — `@optional`.
    pub is_optional: bool,
    /// §7.5.2.1 `is_must_understand` — `@must_understand`.
    pub is_must_understand: bool,
    /// §7.5.2.1 `is_shared` — `@external`.
    pub is_shared: bool,
    /// `@default`, as the model's expression handle.
    ///
    /// Not evaluated here: `zerodds_wire_codec::default::member` is what
    /// turns it into a value, against the member's own type, and a second
    /// evaluation would be a second answer.
    pub has_default_value: bool,
    /// §7.5.2.7.3 `default_value` — the member's `@default` in string
    /// form: "a valid IDL literal of the corresponding type" with string
    /// and character literals unquoted and every expression evaluated. An
    /// empty string means "the 'default default' value for its type".
    ///
    /// For a literal of an enumeration it is the literal's value and for
    /// a flag of a bitmask its position, as text: the descriptor of such
    /// a member has no other place for them (its id is
    /// `MEMBER_ID_INVALID`, §7.5.2.7.5), and `TypeBuilder::add_member`
    /// reads them from here.
    pub default_value: String,
}

impl MemberDescriptor {
    /// §7.5.2.7.1 `copy_from(other)`: overwrite this descriptor so that
    /// `equals(other)` holds. `other` is not changed.
    pub fn copy_from(&mut self, other: &Self) {
        self.clone_from(other);
    }

    /// §7.5.2.7.4 `equals(other)`: "the values of all of the properties
    /// identified in Table 53 above are equal in each of them".
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        self == other
    }

    /// §7.5.2.7.7 `is_consistent()` for a member of a type of kind
    /// `owner`: [`Self::is_consistent`] and the rules that need the
    /// owner.
    ///
    /// - §7.5.2.7.5: a member that does not belong to an aggregated type
    ///   has `MEMBER_ID_INVALID`, and one that does has a legal id (below
    ///   `0x1000_0000`).
    /// - §7.5.2.7.2 and §7.5.2.7.8: only a union member has labels or is
    ///   the default one, and a union member that is not the default has
    ///   at least one label.
    /// - §7.5.2.7.9: the name is not empty.
    ///
    /// # Errors
    /// [`super::DynamicError::Inconsistent`].
    pub fn is_consistent_in(&self, owner: TypeKind) -> Result<(), super::DynamicError> {
        self.check_in(owner, false)
    }

    /// [`Self::is_consistent_in`] for a descriptor that is used to add a
    /// member: §7.5.2.7.5 lets its `id` be `MEMBER_ID_INVALID`, "in that
    /// case, the implementation shall select an ID for the new member that
    /// is one more than the current maximum member ID in the type".
    ///
    /// # Errors
    /// [`super::DynamicError::Inconsistent`].
    pub fn is_consistent_for_add(&self, owner: TypeKind) -> Result<(), super::DynamicError> {
        self.check_in(owner, true)
    }

    fn check_in(&self, owner: TypeKind, adding: bool) -> Result<(), super::DynamicError> {
        use super::DynamicError;
        use alloc::format;
        self.is_consistent()?;
        // §7.5.2.7.9: "The value must be a well-formed member name."
        if !is_identifier(&self.name) {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is no member name",
                self.name
            )));
        }
        let aggregated = matches!(
            owner,
            TypeKind::Structure | TypeKind::Union | TypeKind::Annotation | TypeKind::Bitset
        );
        if aggregated && self.id >= MEMBER_ID_INVALID && !(adding && self.id == MEMBER_ID_INVALID) {
            return Err(DynamicError::inconsistent(format!(
                "`{}` has the id {:#x}, outside the legal range of §7.5.2.5",
                self.name, self.id
            )));
        }
        if !aggregated && self.id != MEMBER_ID_INVALID {
            return Err(DynamicError::inconsistent(format!(
                "`{}` belongs to {}, which is no aggregated type, and has the id {}; \
                 §7.5.2.7.5 requires MEMBER_ID_INVALID",
                self.name,
                owner.as_str(),
                self.id
            )));
        }
        if matches!(owner, TypeKind::Union) {
            if !self.is_default_label && self.label.is_empty() {
                return Err(DynamicError::inconsistent(format!(
                    "`{}` is a union member with no label that is not the default one",
                    self.name
                )));
            }
        } else if !self.label.is_empty() || self.is_default_label {
            return Err(DynamicError::inconsistent(format!(
                "`{}` has case labels and does not belong to a union",
                self.name
            )));
        }
        Ok(())
    }

    /// Whether the record contradicts itself.
    ///
    /// # Errors
    /// [`super::DynamicError::Inconsistent`].
    pub fn is_consistent(&self) -> Result<(), super::DynamicError> {
        use super::DynamicError;
        use alloc::format;
        // §7.2.2.4.4.4.9: "Optional members are not allowed to associate
        // an explicit default value for the member."
        if self.is_optional && self.has_default_value {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is @optional and carries a @default, which §7.2.2.4.4.4.9 does not allow",
                self.name
            )));
        }
        // §7.2.2.4.4.4.8: a key member's must-understand property "shall
        // always be true". Reported and not repaired: repairing it here
        // would make the descriptor disagree with the model it was read
        // from, and `crate::from_ast::member_bits` is where the wire form
        // honours the clause.
        if self.is_key && !self.is_must_understand {
            return Err(DynamicError::inconsistent(format!(
                "`{}` is @key and not @must_understand, which §7.2.2.4.4.4.8 requires",
                self.name
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    /// Every primitive of the model has a row in Annex C, and every row
    /// that names a primitive maps back to the same primitive.
    ///
    /// The two directions are a bijection, `octet` and `uint8`
    /// included: §7.3.4.2 gives them `TK_BYTE` and `TK_UINT8`, two
    /// type kinds, so neither loses its identity here.
    #[test]
    fn every_model_primitive_has_a_kind_and_comes_back() {
        for primitive in AstPrimitive::ALL {
            let kind = TypeKind::of_primitive(primitive);
            assert!(kind.is_primitive(), "{primitive:?} is not a primitive kind");
            let back = kind.as_primitive().expect("a primitive kind maps back");
            assert_eq!(back, primitive);
        }
    }

    /// `octet` and `uint8` reach two `TypeKind`s, not one.
    ///
    /// XTypes 1.3 §7.3.4.2 gives `TK_BYTE` (0x02) and `TK_UINT8`
    /// (0x0D). Merging them announced `TK_BYTE` for a `uint8` member —
    /// wire-visible in the TypeIdentifier octet and in every hash
    /// derived from it — and made the two mutually assignable under
    /// §7.2.4.4.4.4 Table 15.
    #[test]
    fn octet_and_uint8_are_two_kinds() {
        assert_eq!(TypeKind::of_primitive(AstPrimitive::Octet), TypeKind::Byte);
        assert_eq!(TypeKind::of_primitive(AstPrimitive::UInt8), TypeKind::UInt8);
        assert_ne!(
            TypeKind::of_primitive(AstPrimitive::Octet),
            TypeKind::of_primitive(AstPrimitive::UInt8)
        );
    }

    /// The three classifications do not overlap, and `NoType` is in none
    /// of them.
    #[test]
    fn the_classifications_are_disjoint() {
        for kind in TypeKind::ALL {
            let classes = usize::from(kind.is_primitive())
                + usize::from(kind.is_aggregable())
                + usize::from(kind.is_collection());
            assert!(classes <= 1, "{kind:?} is in {classes} classes");
        }
        assert!(!TypeKind::NoType.is_primitive());
        assert!(!TypeKind::NoType.is_aggregable());
        assert!(!TypeKind::NoType.is_collection());
        assert_eq!(TypeKind::NoType.as_primitive(), None);
    }

    /// Annex C (S. 291–292) names a number for each kind. The numbers
    /// are the ones in the IDL, kind by kind, and the mapping is
    /// one-to-one.
    #[test]
    fn the_kinds_carry_the_numbers_of_annex_c() {
        let expected: [(TypeKind, u8); 28] = [
            (TypeKind::NoType, 0x00),
            (TypeKind::Boolean, 0x01),
            (TypeKind::Byte, 0x02),
            (TypeKind::Int16, 0x03),
            (TypeKind::Int32, 0x04),
            (TypeKind::Int64, 0x05),
            (TypeKind::UInt16, 0x06),
            (TypeKind::UInt32, 0x07),
            (TypeKind::UInt64, 0x08),
            (TypeKind::Float32, 0x09),
            (TypeKind::Float64, 0x0A),
            (TypeKind::Float128, 0x0B),
            (TypeKind::Int8, 0x0C),
            (TypeKind::UInt8, 0x0D),
            (TypeKind::Char8, 0x10),
            (TypeKind::Char16, 0x11),
            (TypeKind::String8, 0x20),
            (TypeKind::String16, 0x21),
            (TypeKind::Alias, 0x30),
            (TypeKind::Enumeration, 0x40),
            (TypeKind::Bitmask, 0x41),
            (TypeKind::Annotation, 0x50),
            (TypeKind::Structure, 0x51),
            (TypeKind::Union, 0x52),
            (TypeKind::Bitset, 0x53),
            (TypeKind::Sequence, 0x60),
            (TypeKind::Array, 0x61),
            (TypeKind::Map, 0x62),
        ];
        for (kind, number) in expected {
            assert_eq!(kind.value(), number, "{kind:?}");
            assert_eq!(TypeKind::from_value(number), Some(kind), "{number:#x}");
        }
        // Every kind is in the table, and a number outside it is no kind.
        assert_eq!(expected.len(), TypeKind::ALL.len());
        for kind in TypeKind::ALL {
            assert!(expected.iter().any(|(k, _)| *k == kind), "{kind:?}");
        }
        for number in [0x0E_u8, 0x0F, 0x12, 0x22, 0x31, 0x42, 0x54, 0x63, 0xFF] {
            assert_eq!(TypeKind::from_value(number), None, "{number:#x}");
        }
    }

    #[test]
    fn the_two_renames_round_trip() {
        for extensibility in ExtensibilityKind::ALL {
            let there: Extensibility = extensibility.into();
            assert_eq!(ExtensibilityKind::from(there), extensibility);
        }
        for behaviour in TryConstructKind::ALL {
            let there: TryConstruct = behaviour.into();
            assert_eq!(TryConstructKind::from(there), behaviour);
        }
    }
}
