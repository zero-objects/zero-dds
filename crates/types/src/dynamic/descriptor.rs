// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeDescriptor + MemberDescriptor (XTypes 1.3 §7.5.1, §7.5.2).
//!
//! A `TypeDescriptor` fully describes a DynamicType: kind +
//! name + bound + element type etc. It is the **constructive** entry point
//! to `DynamicTypeBuilderFactory::create_type` (Spec §7.5.5.1).
//!
//! A `MemberDescriptor` describes a member within a
//! composite type (struct/union/annotation). Spec §7.5.2 lists all
//! fields, which are mapped 1:1 here. The apply logic for
//! `try_construct` (DISCARD/USE_DEFAULT/TRIM) is added in C4.7.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// XTypes 1.3 TypeKind-Enum (§7.5.1 Table 10).
///
/// Covers the 24 kinds named in the spec. `NoType` corresponds to
/// `TK_NONE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TypeKind {
    /// No type — sentinel value.
    NoType,
    /// `boolean`.
    Boolean,
    /// `octet` / `byte` (8-bit unsigned).
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
    /// `float128` (long double).
    Float128,
    /// `char` (8-bit).
    Char8,
    /// `wchar` (16-bit).
    Char16,
    /// `string<N>`.
    String8,
    /// `wstring<N>`.
    String16,
    /// Enumeration.
    Enumeration,
    /// Bitmask.
    Bitmask,
    /// Alias / typedef.
    Alias,
    /// Array `T[D1,D2,...]`.
    Array,
    /// `sequence<T,N>`.
    Sequence,
    /// `map<K,V,N>`.
    Map,
    /// `struct`.
    Structure,
    /// `union`.
    Union,
    /// `bitset`.
    Bitset,
    /// `annotation`.
    Annotation,
}

impl TypeKind {
    /// `true` if the kind is a primitive, atomic type (not a
    /// composite, not a collection). Spec §7.5.1.
    #[must_use]
    pub const fn is_primitive(self) -> bool {
        matches!(
            self,
            Self::Boolean
                | Self::Byte
                | Self::Int8
                | Self::UInt8
                | Self::Int16
                | Self::UInt16
                | Self::Int32
                | Self::UInt32
                | Self::Int64
                | Self::UInt64
                | Self::Float32
                | Self::Float64
                | Self::Float128
                | Self::Char8
                | Self::Char16
        )
    }

    /// `true` if this kind can carry members (Struct/Union/
    /// Annotation/Bitset/Bitmask/Enum).
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
}

/// Extensibility kind (§7.2.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensibilityKind {
    /// `@final` — the type is closed.
    Final,
    /// `@appendable` — new fields at the end allowed (default).
    Appendable,
    /// `@mutable` — arbitrary evolution with `@id` bindings.
    Mutable,
}

impl Default for ExtensibilityKind {
    fn default() -> Self {
        Self::Appendable
    }
}

/// Try-Construct-Strategie (Spec §7.5.2 + §7.6.4).
///
/// The apply semantics (what happens on a decoder failure) is implemented
/// in C4.7 — here only the enum + member field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryConstructKind {
    /// Discard the sample.
    Discard,
    /// Set to the default value.
    UseDefault,
    /// Truncate to the bound (strings/sequences).
    Trim,
}

impl Default for TryConstructKind {
    fn default() -> Self {
        Self::Discard
    }
}

/// `MemberId` — consistent with XTypes 1.3 §7.3.1.1 (32 bits).
pub type MemberId = u32;

/// XTypes §7.5.1.2 TypeDescriptor.
///
/// Describes a DynamicType — for construction via
/// [`crate::dynamic::DynamicTypeBuilderFactory::create_type`] or as a
/// read-only view via [`crate::dynamic::DynamicType::descriptor`].
///
/// Fields that are irrelevant for a given kind can be left empty
/// (e.g. `bound` for a struct = `Vec::new()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDescriptor {
    /// TypeKind.
    pub kind: TypeKind,
    /// Fully qualified name, e.g. `"::sensors::Chatter"`.
    pub name: String,
    /// Base type for inheritance (struct/union).
    pub base_type: Option<Box<TypeDescriptor>>,
    /// Discriminator type for `kind == Union` (mandatory).
    pub discriminator_type: Option<Box<TypeDescriptor>>,
    /// Bound — array dimensions, or `[max]` for sequence/string/map.
    /// Empty for composite/primitive.
    pub bound: Vec<u32>,
    /// Element type for array/sequence/map.
    pub element_type: Option<Box<TypeDescriptor>>,
    /// Key type for map.
    pub key_element_type: Option<Box<TypeDescriptor>>,
    /// Extensibility kind (relevant for struct/union).
    pub extensibility_kind: ExtensibilityKind,
    /// `@nested` — the type is not intended as a top-level topic.
    pub is_nested: bool,
}

impl TypeDescriptor {
    /// Constructs a primitive descriptor.
    #[must_use]
    pub fn primitive(kind: TypeKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            base_type: None,
            discriminator_type: None,
            bound: Vec::new(),
            element_type: None,
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs a struct descriptor.
    #[must_use]
    pub fn structure(name: impl Into<String>) -> Self {
        Self {
            kind: TypeKind::Structure,
            name: name.into(),
            base_type: None,
            discriminator_type: None,
            bound: Vec::new(),
            element_type: None,
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs a union descriptor.
    #[must_use]
    pub fn union(name: impl Into<String>, discriminator: TypeDescriptor) -> Self {
        Self {
            kind: TypeKind::Union,
            name: name.into(),
            base_type: None,
            discriminator_type: Some(Box::new(discriminator)),
            bound: Vec::new(),
            element_type: None,
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs a sequence descriptor.
    #[must_use]
    pub fn sequence(name: impl Into<String>, element: TypeDescriptor, max: u32) -> Self {
        Self {
            kind: TypeKind::Sequence,
            name: name.into(),
            base_type: None,
            discriminator_type: None,
            bound: alloc::vec![max],
            element_type: Some(Box::new(element)),
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs an array descriptor.
    #[must_use]
    pub fn array(name: impl Into<String>, element: TypeDescriptor, dims: Vec<u32>) -> Self {
        Self {
            kind: TypeKind::Array,
            name: name.into(),
            base_type: None,
            discriminator_type: None,
            bound: dims,
            element_type: Some(Box::new(element)),
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs a map descriptor.
    #[must_use]
    pub fn map(
        name: impl Into<String>,
        key: TypeDescriptor,
        element: TypeDescriptor,
        max: u32,
    ) -> Self {
        Self {
            kind: TypeKind::Map,
            name: name.into(),
            base_type: None,
            discriminator_type: None,
            bound: alloc::vec![max],
            element_type: Some(Box::new(element)),
            key_element_type: Some(Box::new(key)),
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs a string descriptor (`string<bound>`).
    #[must_use]
    pub fn string8(bound: u32) -> Self {
        Self {
            kind: TypeKind::String8,
            name: alloc::format!("string<{bound}>"),
            base_type: None,
            discriminator_type: None,
            bound: alloc::vec![bound],
            element_type: None,
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs a WString descriptor.
    #[must_use]
    pub fn string16(bound: u32) -> Self {
        Self {
            kind: TypeKind::String16,
            name: alloc::format!("wstring<{bound}>"),
            base_type: None,
            discriminator_type: None,
            bound: alloc::vec![bound],
            element_type: None,
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Constructs an enum descriptor.
    #[must_use]
    pub fn enumeration(name: impl Into<String>) -> Self {
        Self {
            kind: TypeKind::Enumeration,
            name: name.into(),
            base_type: None,
            discriminator_type: None,
            bound: Vec::new(),
            element_type: None,
            key_element_type: None,
            extensibility_kind: ExtensibilityKind::default(),
            is_nested: false,
        }
    }

    /// Validierung — Spec §7.5.1.4 `is_consistent()`.
    ///
    /// Checks the constraints defined in the spec for a
    /// descriptor object: a discriminator is mandatory for a union, a bound
    /// is mandatory for array/sequence/string/map etc.
    ///
    /// # Errors
    /// `String` with a human-readable error description.
    pub fn is_consistent(&self) -> Result<(), String> {
        // Cycle check: a descriptor may not reference itself as
        // base_type or element_type (detected by structural
        // equality — the builder performs the robust cycle check).
        if self.name.is_empty() && self.kind != TypeKind::NoType {
            return Err(String::from("descriptor without name"));
        }
        match self.kind {
            TypeKind::Union => {
                let Some(d) = &self.discriminator_type else {
                    return Err(String::from("union without discriminator_type"));
                };
                if !is_valid_discriminator(d.kind) {
                    return Err(alloc::format!(
                        "union discriminator must be int/enum/bool, got {:?}",
                        d.kind
                    ));
                }
            }
            TypeKind::Array => {
                if self.bound.is_empty() {
                    return Err(String::from("array without dimensions"));
                }
                if self.bound.contains(&0) {
                    return Err(String::from("array dimension must be > 0"));
                }
                if self.element_type.is_none() {
                    return Err(String::from("array without element_type"));
                }
            }
            TypeKind::Sequence | TypeKind::String8 | TypeKind::String16 => {
                if self.bound.len() != 1 {
                    return Err(String::from("sequence/string needs exactly 1 bound"));
                }
                if matches!(self.kind, TypeKind::Sequence) && self.element_type.is_none() {
                    return Err(String::from("sequence without element_type"));
                }
            }
            TypeKind::Map => {
                if self.bound.len() != 1 {
                    return Err(String::from("map needs exactly 1 bound"));
                }
                if self.element_type.is_none() {
                    return Err(String::from("map without value element_type"));
                }
                if self.key_element_type.is_none() {
                    return Err(String::from("map without key_element_type"));
                }
            }
            _ => {}
        }
        // Inheritance cycle check (1 level; deeper levels are checked in the
        // builder via `build()` against the final DynamicType).
        if let Some(b) = &self.base_type {
            if b.name == self.name && !self.name.is_empty() {
                return Err(String::from("inheritance cycle: base_type == self"));
            }
        }
        Ok(())
    }
}

/// XTypes §7.5.2.2 MemberDescriptor.
///
/// Describes a member within a composite type (struct,
/// union, annotation, bitset, bitmask). For a bitmask,
/// `member_type` is typically `Boolean` and `id` is the bit position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDescriptor {
    /// Member name (case-sensitive, unique within the composite).
    pub name: String,
    /// Member id (unique within the composite, for XCDR2).
    pub id: MemberId,
    /// Type of the member.
    pub member_type: Box<TypeDescriptor>,
    /// Default value in canonical IDL literal form.
    pub default_value: Option<String>,
    /// Order index (0-based) — for `member_by_index`.
    pub index: u32,
    /// Union case labels. Spec §7.5.2 — only populated for unions.
    pub label: Vec<i64>,
    /// Try-construct strategy (apply in C4.7).
    pub try_construct: TryConstructKind,
    /// `@key` — the member is part of the topic key.
    pub is_key: bool,
    /// `@optional`.
    pub is_optional: bool,
    /// `@must_understand`.
    pub is_must_understand: bool,
    /// `@external` — indirect storage (shared_ptr in C++).
    pub is_shared: bool,
    /// `default:` branch for a union.
    pub is_default_label: bool,
    /// Bitfield-Breite in Bits (1..=64) — nur fuer `Bitset`-Felder belegt;
    /// `id` traegt dabei die Bit-Startposition. Wird vom DynamicType →
    /// TypeObject-Bridge (XTypes §7.3.4.4 CompleteBitfield) ausgewertet.
    pub bit_bound: Option<u8>,
}

impl MemberDescriptor {
    /// Creates a MemberDescriptor with the most common defaults
    /// for struct members.
    #[must_use]
    pub fn new(name: impl Into<String>, id: MemberId, ty: TypeDescriptor) -> Self {
        Self {
            name: name.into(),
            id,
            member_type: Box::new(ty),
            default_value: None,
            index: 0,
            label: Vec::new(),
            try_construct: TryConstructKind::default(),
            is_key: false,
            is_optional: false,
            is_must_understand: false,
            is_shared: false,
            is_default_label: false,
            bit_bound: None,
        }
    }

    /// Validierung — Spec §7.5.2.4 `is_consistent()`.
    ///
    /// # Errors
    /// `String` with the error text.
    pub fn is_consistent(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err(String::from("member without name"));
        }
        self.member_type.is_consistent()?;
        if self.is_default_label && !self.label.is_empty() {
            return Err(String::from(
                "member with is_default_label must not have explicit labels",
            ));
        }
        Ok(())
    }
}

/// True if the TypeKind is a valid union discriminator
/// (Spec §7.4.1.4.4: integral or enum or boolean or char).
const fn is_valid_discriminator(kind: TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Boolean
            | TypeKind::Byte
            | TypeKind::Int8
            | TypeKind::UInt8
            | TypeKind::Int16
            | TypeKind::UInt16
            | TypeKind::Int32
            | TypeKind::UInt32
            | TypeKind::Int64
            | TypeKind::UInt64
            | TypeKind::Char8
            | TypeKind::Char16
            | TypeKind::Enumeration
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn type_kind_primitive_set_matches_spec_table_10() {
        for k in [
            TypeKind::Boolean,
            TypeKind::Byte,
            TypeKind::Int8,
            TypeKind::UInt8,
            TypeKind::Int16,
            TypeKind::UInt16,
            TypeKind::Int32,
            TypeKind::UInt32,
            TypeKind::Int64,
            TypeKind::UInt64,
            TypeKind::Float32,
            TypeKind::Float64,
            TypeKind::Float128,
            TypeKind::Char8,
            TypeKind::Char16,
        ] {
            assert!(k.is_primitive(), "{k:?} should be primitive");
        }
        for k in [
            TypeKind::Structure,
            TypeKind::Union,
            TypeKind::Sequence,
            TypeKind::Array,
            TypeKind::Map,
            TypeKind::String8,
            TypeKind::String16,
            TypeKind::Alias,
        ] {
            assert!(!k.is_primitive(), "{k:?} should not be primitive");
        }
    }

    #[test]
    fn type_kind_aggregable_set() {
        assert!(TypeKind::Structure.is_aggregable());
        assert!(TypeKind::Union.is_aggregable());
        assert!(TypeKind::Annotation.is_aggregable());
        assert!(TypeKind::Bitset.is_aggregable());
        assert!(TypeKind::Bitmask.is_aggregable());
        assert!(TypeKind::Enumeration.is_aggregable());
        assert!(!TypeKind::Int32.is_aggregable());
        assert!(!TypeKind::Sequence.is_aggregable());
    }

    #[test]
    fn descriptor_struct_passes_consistency() {
        let s = TypeDescriptor::structure("::Foo");
        assert!(s.is_consistent().is_ok());
    }

    #[test]
    fn descriptor_union_without_discriminator_fails() {
        let mut u = TypeDescriptor::structure("::U");
        u.kind = TypeKind::Union;
        let err = u.is_consistent().unwrap_err();
        assert!(err.contains("discriminator"));
    }

    #[test]
    fn descriptor_union_with_invalid_discriminator_fails() {
        let bad_disc = TypeDescriptor::structure("::S");
        let u = TypeDescriptor::union("::U", bad_disc);
        let err = u.is_consistent().unwrap_err();
        assert!(err.contains("discriminator"));
    }

    #[test]
    fn descriptor_array_without_dims_fails() {
        let mut a = TypeDescriptor::array(
            "::A",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            alloc::vec![3, 3],
        );
        a.bound.clear();
        let err = a.is_consistent().unwrap_err();
        assert!(err.contains("dimensions"));
    }

    #[test]
    fn descriptor_array_with_zero_dim_fails() {
        let a = TypeDescriptor::array(
            "::A",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            alloc::vec![3, 0, 4],
        );
        let err = a.is_consistent().unwrap_err();
        assert!(err.contains("> 0"));
    }

    #[test]
    fn descriptor_sequence_with_element_passes() {
        let s = TypeDescriptor::sequence(
            "::S",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            100,
        );
        assert!(s.is_consistent().is_ok());
    }

    #[test]
    fn descriptor_map_requires_both_key_and_value() {
        let mut m = TypeDescriptor::map(
            "::M",
            TypeDescriptor::string8(64),
            TypeDescriptor::primitive(TypeKind::Int64, "int64"),
            500,
        );
        assert!(m.is_consistent().is_ok());
        m.key_element_type = None;
        assert!(m.is_consistent().is_err());
    }

    #[test]
    fn descriptor_inheritance_cycle_self_reference_rejected() {
        let mut s = TypeDescriptor::structure("::Foo");
        let cycle = TypeDescriptor::structure("::Foo");
        s.base_type = Some(Box::new(cycle));
        let err = s.is_consistent().unwrap_err();
        assert!(err.contains("cycle"));
    }

    #[test]
    fn member_descriptor_default_label_with_labels_rejected() {
        let mut m =
            MemberDescriptor::new("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"));
        m.is_default_label = true;
        m.label = alloc::vec![0];
        let err = m.is_consistent().unwrap_err();
        assert!(err.contains("default_label"));
    }

    #[test]
    fn member_descriptor_empty_name_rejected() {
        let m = MemberDescriptor::new("", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"));
        assert!(m.is_consistent().is_err());
    }

    #[test]
    fn try_construct_default_is_discard() {
        assert_eq!(TryConstructKind::default(), TryConstructKind::Discard);
    }

    #[test]
    fn extensibility_default_is_appendable() {
        assert_eq!(ExtensibilityKind::default(), ExtensibilityKind::Appendable);
    }
}
