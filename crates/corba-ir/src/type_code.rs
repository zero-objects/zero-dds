// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TypeCode — Spec §10.7.
//!
//! `TypeCode` is the runtime representation of an IDL type.
//! Spec §10.7.2 defines 32 `TCKind` values (`tk_null`..`tk_local_interface`).
//!
//! We model TCKind as an enum and the TypeCode as a struct with
//! `kind` plus optional body data. Complex types (struct/union/
//! enum/sequence/array/alias/except/interface/valuetype) carry
//! their IDL member lists.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// `TCKind` — Spec §10.7.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TcKind {
    /// `tk_null = 0`.
    Null = 0,
    /// `tk_void = 1`.
    Void = 1,
    /// `tk_short = 2`.
    Short = 2,
    /// `tk_long = 3`.
    Long = 3,
    /// `tk_ushort = 4`.
    UShort = 4,
    /// `tk_ulong = 5`.
    ULong = 5,
    /// `tk_float = 6`.
    Float = 6,
    /// `tk_double = 7`.
    Double = 7,
    /// `tk_boolean = 8`.
    Boolean = 8,
    /// `tk_char = 9`.
    Char = 9,
    /// `tk_octet = 10`.
    Octet = 10,
    /// `tk_any = 11`.
    Any = 11,
    /// `tk_TypeCode = 12`.
    TypeCode = 12,
    /// `tk_Principal = 13` (deprecated).
    Principal = 13,
    /// `tk_objref = 14`.
    ObjRef = 14,
    /// `tk_struct = 15`.
    Struct = 15,
    /// `tk_union = 16`.
    Union = 16,
    /// `tk_enum = 17`.
    Enum = 17,
    /// `tk_string = 18`.
    String = 18,
    /// `tk_sequence = 19`.
    Sequence = 19,
    /// `tk_array = 20`.
    Array = 20,
    /// `tk_alias = 21`.
    Alias = 21,
    /// `tk_except = 22`.
    Except = 22,
    /// `tk_longlong = 23`.
    LongLong = 23,
    /// `tk_ulonglong = 24`.
    ULongLong = 24,
    /// `tk_longdouble = 25`.
    LongDouble = 25,
    /// `tk_wchar = 26`.
    WChar = 26,
    /// `tk_wstring = 27`.
    WString = 27,
    /// `tk_fixed = 28`.
    Fixed = 28,
    /// `tk_value = 29`.
    Value = 29,
    /// `tk_value_box = 30`.
    ValueBox = 30,
    /// `tk_native = 31`.
    Native = 31,
    /// `tk_abstract_interface = 32`.
    AbstractInterface = 32,
    /// `tk_local_interface = 33`.
    LocalInterface = 33,
}

impl TcKind {
    /// Raw discriminant value (`unsigned long`).
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Union member for `tk_union`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionMember {
    /// Member label (CDR-encoded discriminator value).
    pub label_bytes: Vec<u8>,
    /// Member name.
    pub name: String,
    /// Member type.
    pub type_code: TypeCode,
}

/// Struct/except member.
#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    /// Field name.
    pub name: String,
    /// Field type.
    pub type_code: TypeCode,
}

/// `TypeCode` body — data depending on `kind`.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeCodeBody {
    /// Primitive types without a body.
    Primitive,
    /// `tk_string` with a max bound (0 = unbounded).
    String {
        /// Max length (0 = unbounded).
        bound: u32,
    },
    /// `tk_wstring` with a max bound.
    WString {
        /// Max length (0 = unbounded).
        bound: u32,
    },
    /// `tk_fixed` with digits and scale.
    Fixed {
        /// Total digits.
        digits: u16,
        /// Decimal scale (may be negative).
        scale: i16,
    },
    /// `tk_objref` / `tk_native` / `tk_abstract_interface` /
    /// `tk_local_interface`. RepositoryId + simple name.
    Interface {
        /// Repository ID.
        id: String,
        /// Simple name.
        name: String,
    },
    /// `tk_struct` / `tk_except`.
    Struct {
        /// Repository ID.
        id: String,
        /// Simple name.
        name: String,
        /// Members.
        members: Vec<StructMember>,
    },
    /// `tk_union`.
    Union {
        /// Repository ID.
        id: String,
        /// Simple name.
        name: String,
        /// Discriminator type.
        discriminator: Box<TypeCode>,
        /// Default index (`-1` = none).
        default_index: i32,
        /// Members.
        members: Vec<UnionMember>,
    },
    /// `tk_enum`.
    Enum {
        /// Repository ID.
        id: String,
        /// Simple name.
        name: String,
        /// Member names in order.
        members: Vec<String>,
    },
    /// `tk_sequence`.
    Sequence {
        /// Element TypeCode.
        element: Box<TypeCode>,
        /// Bound (0 = unbounded).
        bound: u32,
    },
    /// `tk_array`.
    Array {
        /// Element TypeCode.
        element: Box<TypeCode>,
        /// Length.
        length: u32,
    },
    /// `tk_alias`.
    Alias {
        /// Repository ID.
        id: String,
        /// Simple name.
        name: String,
        /// Original TypeCode.
        original: Box<TypeCode>,
    },
    /// `tk_value`.
    Value {
        /// Repository ID.
        id: String,
        /// Simple name.
        name: String,
        /// Value modifier (NONE/CUSTOM/ABSTRACT/TRUNCATABLE).
        modifier: i16,
        /// Concrete base type (`Null` TC = no base).
        concrete_base: Box<TypeCode>,
        /// Members.
        members: Vec<ValueMember>,
    },
    /// `tk_value_box`.
    ValueBox {
        /// Repository ID.
        id: String,
        /// Simple name.
        name: String,
        /// Boxed type.
        boxed: Box<TypeCode>,
    },
}

/// Valuetype member.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueMember {
    /// Field name.
    pub name: String,
    /// Field type.
    pub type_code: TypeCode,
    /// Visibility (`PRIVATE_MEMBER = 0`, `PUBLIC_MEMBER = 1`).
    pub access: i16,
}

/// TypeCode.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeCode {
    /// `kind`.
    pub kind: TcKind,
    /// Body data depending on `kind`.
    pub body: TypeCodeBody,
}

impl TypeCode {
    /// Constructor for primitive types.
    #[must_use]
    pub const fn primitive(kind: TcKind) -> Self {
        Self {
            kind,
            body: TypeCodeBody::Primitive,
        }
    }

    /// `tk_null` constructor.
    #[must_use]
    pub const fn null() -> Self {
        Self::primitive(TcKind::Null)
    }

    /// `tk_void` constructor.
    #[must_use]
    pub const fn void() -> Self {
        Self::primitive(TcKind::Void)
    }

    /// `tk_string` with a bound.
    #[must_use]
    pub const fn string(bound: u32) -> Self {
        Self {
            kind: TcKind::String,
            body: TypeCodeBody::String { bound },
        }
    }

    /// `tk_sequence`.
    #[must_use]
    pub fn sequence(element: TypeCode, bound: u32) -> Self {
        Self {
            kind: TcKind::Sequence,
            body: TypeCodeBody::Sequence {
                element: Box::new(element),
                bound,
            },
        }
    }

    /// `tk_struct`.
    #[must_use]
    pub fn r#struct(id: String, name: String, members: Vec<StructMember>) -> Self {
        Self {
            kind: TcKind::Struct,
            body: TypeCodeBody::Struct { id, name, members },
        }
    }

    /// `tk_enum`.
    #[must_use]
    pub fn r#enum(id: String, name: String, members: Vec<String>) -> Self {
        Self {
            kind: TcKind::Enum,
            body: TypeCodeBody::Enum { id, name, members },
        }
    }

    /// Returns the repository ID if present (Spec §10.7.1.1).
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match &self.body {
            TypeCodeBody::Interface { id, .. }
            | TypeCodeBody::Struct { id, .. }
            | TypeCodeBody::Union { id, .. }
            | TypeCodeBody::Enum { id, .. }
            | TypeCodeBody::Alias { id, .. }
            | TypeCodeBody::Value { id, .. }
            | TypeCodeBody::ValueBox { id, .. } => Some(id),
            _ => None,
        }
    }

    /// Returns the simple name if present.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match &self.body {
            TypeCodeBody::Interface { name, .. }
            | TypeCodeBody::Struct { name, .. }
            | TypeCodeBody::Union { name, .. }
            | TypeCodeBody::Enum { name, .. }
            | TypeCodeBody::Alias { name, .. }
            | TypeCodeBody::Value { name, .. }
            | TypeCodeBody::ValueBox { name, .. } => Some(name),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn well_known_tc_kind_values_match_spec() {
        // Spec §10.7.2 Table 10-2.
        assert_eq!(TcKind::Null.as_u32(), 0);
        assert_eq!(TcKind::Void.as_u32(), 1);
        assert_eq!(TcKind::Long.as_u32(), 3);
        assert_eq!(TcKind::Struct.as_u32(), 15);
        assert_eq!(TcKind::Union.as_u32(), 16);
        assert_eq!(TcKind::Enum.as_u32(), 17);
        assert_eq!(TcKind::Sequence.as_u32(), 19);
        assert_eq!(TcKind::Alias.as_u32(), 21);
        assert_eq!(TcKind::Value.as_u32(), 29);
        assert_eq!(TcKind::ValueBox.as_u32(), 30);
        assert_eq!(TcKind::AbstractInterface.as_u32(), 32);
        assert_eq!(TcKind::LocalInterface.as_u32(), 33);
    }

    #[test]
    fn primitive_tc_has_no_body() {
        let tc = TypeCode::primitive(TcKind::Long);
        assert_eq!(tc.kind, TcKind::Long);
        assert!(tc.id().is_none());
        assert!(tc.name().is_none());
    }

    #[test]
    fn struct_tc_carries_id_and_members() {
        let tc = TypeCode::r#struct(
            "IDL:demo/Point:1.0".into(),
            "Point".into(),
            alloc::vec![
                StructMember {
                    name: "x".into(),
                    type_code: TypeCode::primitive(TcKind::Long),
                },
                StructMember {
                    name: "y".into(),
                    type_code: TypeCode::primitive(TcKind::Long),
                },
            ],
        );
        assert_eq!(tc.kind, TcKind::Struct);
        assert_eq!(tc.id(), Some("IDL:demo/Point:1.0"));
        assert_eq!(tc.name(), Some("Point"));
    }

    #[test]
    fn sequence_tc_holds_element() {
        let inner = TypeCode::primitive(TcKind::Octet);
        let seq = TypeCode::sequence(inner.clone(), 100);
        match &seq.body {
            TypeCodeBody::Sequence { element, bound } => {
                assert_eq!(**element, inner);
                assert_eq!(*bound, 100);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn enum_tc_holds_member_names() {
        let tc = TypeCode::r#enum(
            "IDL:demo/Color:1.0".into(),
            "Color".into(),
            alloc::vec!["RED".into(), "GREEN".into(), "BLUE".into()],
        );
        match &tc.body {
            TypeCodeBody::Enum { members, .. } => assert_eq!(members.len(), 3),
            _ => panic!(),
        }
    }
}
