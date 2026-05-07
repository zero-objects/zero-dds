// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeDescriptor + MemberDescriptor (XTypes 1.3 §7.5.1, §7.5.2).
//!
//! Ein `TypeDescriptor` beschreibt einen DynamicType vollstaendig: kind +
//! Name + Bound + Element-Type usw. Er ist der **konstruktive** Eingang
//! zu `DynamicTypeBuilderFactory::create_type` (Spec §7.5.5.1).
//!
//! Ein `MemberDescriptor` beschreibt einen Member innerhalb eines
//! Composite-Types (Struct/Union/Annotation). Spec §7.5.2 listet alle
//! Felder, die hier 1:1 abgebildet sind. Apply-Logik fuer
//! `try_construct` (DISCARD/USE_DEFAULT/TRIM) wird in C4.7 ergaenzt.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// XTypes 1.3 TypeKind-Enum (§7.5.1 Table 10).
///
/// Deckt die 24 in der Spec genannten Kinds. `NoType` entspricht
/// `TK_NONE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TypeKind {
    /// Kein Typ — Sentinel-Wert.
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
    /// `true` wenn der Kind ein primitiver, atomarer Typ ist (kein
    /// Composite, keine Collection). Spec §7.5.1.
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

    /// `true` wenn dieser Kind Members tragen kann (Struct/Union/
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

/// Extensibility-Kind (§7.2.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensibilityKind {
    /// `@final` — Typ ist abgeschlossen.
    Final,
    /// `@appendable` — neue Felder am Ende erlaubt (Default).
    Appendable,
    /// `@mutable` — beliebige Evolution mit `@id`-Bindings.
    Mutable,
}

impl Default for ExtensibilityKind {
    fn default() -> Self {
        Self::Appendable
    }
}

/// Try-Construct-Strategie (Spec §7.5.2 + §7.6.4).
///
/// Apply-Semantik (was passiert bei Decoder-Fehlschlag) wird in C4.7
/// implementiert — hier ist nur das Enum + Member-Feld.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryConstructKind {
    /// Werfe das Sample weg.
    Discard,
    /// Setze auf den Default-Wert.
    UseDefault,
    /// Truncate auf Bound (Strings/Sequences).
    Trim,
}

impl Default for TryConstructKind {
    fn default() -> Self {
        Self::Discard
    }
}

/// `MemberId` — konsistent mit XTypes 1.3 §7.3.1.1 (32 bits).
pub type MemberId = u32;

/// XTypes §7.5.1.2 TypeDescriptor.
///
/// Beschreibt einen DynamicType — zur Konstruktion via
/// [`crate::dynamic::DynamicTypeBuilderFactory::create_type`] oder als
/// Read-only-Sicht via [`crate::dynamic::DynamicType::descriptor`].
///
/// Felder, die fuer einen gegebenen Kind irrelevant sind, koennen leer
/// bleiben (z.B. `bound` fuer Struct = `Vec::new()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDescriptor {
    /// TypeKind.
    pub kind: TypeKind,
    /// Voll qualifizierter Name, z.B. `"::sensors::Chatter"`.
    pub name: String,
    /// Base-Type fuer Inheritance (Struct/Union).
    pub base_type: Option<Box<TypeDescriptor>>,
    /// Discriminator-Type fuer `kind == Union` (Pflicht).
    pub discriminator_type: Option<Box<TypeDescriptor>>,
    /// Bound — Array-Dimensionen, oder `[max]` fuer Sequence/String/Map.
    /// Leer fuer Composite/Primitive.
    pub bound: Vec<u32>,
    /// Element-Type fuer Array/Sequence/Map.
    pub element_type: Option<Box<TypeDescriptor>>,
    /// Key-Type fuer Map.
    pub key_element_type: Option<Box<TypeDescriptor>>,
    /// Extensibility-Kind (relevant fuer Struct/Union).
    pub extensibility_kind: ExtensibilityKind,
    /// `@nested` — Type ist nicht als Top-Level-Topic gedacht.
    pub is_nested: bool,
}

impl TypeDescriptor {
    /// Konstruiert einen primitiven Descriptor.
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

    /// Konstruiert einen Struct-Descriptor.
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

    /// Konstruiert einen Union-Descriptor.
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

    /// Konstruiert einen Sequence-Descriptor.
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

    /// Konstruiert einen Array-Descriptor.
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

    /// Konstruiert einen Map-Descriptor.
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

    /// Konstruiert einen String-Descriptor (`string<bound>`).
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

    /// Konstruiert einen WString-Descriptor.
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

    /// Konstruiert einen Enum-Descriptor.
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
    /// Prueft die in der Spec festgelegten Constraints fuer ein
    /// Descriptor-Object: Discriminator-Pflicht bei Union, Bound-Pflicht
    /// bei Array/Sequence/String/Map etc.
    ///
    /// # Errors
    /// `String` mit menschen-lesbarer Fehlerbeschreibung.
    pub fn is_consistent(&self) -> Result<(), String> {
        // Cycle-Check: ein Descriptor darf sich nicht selbst als
        // base_type oder element_type referenzieren (durch struktur-
        // gleichheit erkannt — robusten Cycle-Check macht der Builder).
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
        // Inheritance-Cycle-Check (1 Ebene; tiefere Ebenen werden im
        // Builder via `build()` gegen die finale DynamicType gecheckt).
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
/// Beschreibt einen Member innerhalb eines Composite-Types (Struct,
/// Union, Annotation, Bitset, Bitmask). Fuer Bitmask repraesentiert
/// `member_type` typisch `Boolean`, `id` ist die Bit-Position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDescriptor {
    /// Member-Name (case-sensitive, eindeutig im Composite).
    pub name: String,
    /// Member-Id (eindeutig im Composite, fuer XCDR2).
    pub id: MemberId,
    /// Type des Members.
    pub member_type: Box<TypeDescriptor>,
    /// Default-Wert in canonical IDL-Literal-Form.
    pub default_value: Option<String>,
    /// Reihenfolgen-Index (0-basiert) — fuer `member_by_index`.
    pub index: u32,
    /// Union-Case-Labels. Spec §7.5.2 — nur fuer Union belegt.
    pub label: Vec<i64>,
    /// Try-Construct-Strategie (Apply in C4.7).
    pub try_construct: TryConstructKind,
    /// `@key` — Member ist Teil des Topic-Keys.
    pub is_key: bool,
    /// `@optional`.
    pub is_optional: bool,
    /// `@must_understand`.
    pub is_must_understand: bool,
    /// `@external` — indirect storage (shared_ptr in C++).
    pub is_shared: bool,
    /// `default:` Branch fuer Union.
    pub is_default_label: bool,
}

impl MemberDescriptor {
    /// Erstellt einen MemberDescriptor mit den verbreitetsten Defaults
    /// fuer Struct-Members.
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
        }
    }

    /// Validierung — Spec §7.5.2.4 `is_consistent()`.
    ///
    /// # Errors
    /// `String` mit Fehlertext.
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

/// True, wenn der TypeKind ein gueltiger Union-Discriminator ist
/// (Spec §7.4.1.4.4: integral oder enum oder boolean oder char).
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
