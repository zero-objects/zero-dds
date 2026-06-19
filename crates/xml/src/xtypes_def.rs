// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.3 building block "Types" — data model.
//!
//! Descriptive data model of the XML view of XTypes 1.3 type definitions.
//!
//! `zerodds-xml` deliberately keeps a *standalone* descriptive layer and
//! makes **no** cross-crate edge to `zerodds-types::DynamicType`. Higher-level
//! adapters can later translate the structures held here into
//! their DCPS data models.
//!
//! Spec source: OMG DDS-XML 1.0 §7.3.3 (Types Building Block).
//!
//! # XML → Rust type mapping
//!
//! ```text
//! <types>                          | Vec<TypeLibrary> (multiple <types> allowed)
//! <module name=…>                  | TypeLibrary / TypeDef::Module(ModuleEntry)
//! <struct name=… extensibility=…   |
//!         baseType=…>              | TypeDef::Struct(StructType)
//! <member name=… type=… key=…      |
//!         optional=… id=…          |
//!         arrayDimensions=…        |
//!         sequenceMaxLength=…      |
//!         stringMaxLength=…>       | StructMember
//! <enum name=…>                    | TypeDef::Enum(EnumType)
//! <enumerator name=… value=…>      | EnumLiteral
//! <union name=… discriminator=…>   | TypeDef::Union(UnionType)
//! <case><caseDiscriminator/>       |
//!       <member …/></case>         | UnionCase
//! <typedef name=… type=… …>        | TypeDef::Typedef(TypedefType)
//! <bitmask name=… bitBound=…>      | TypeDef::Bitmask(BitmaskType)
//! <bit_value name=… position=…>    | BitValue
//! <bitset name=…>                  | TypeDef::Bitset(BitsetType)
//! <bitfield name=… type=… mask=…>  | BitField
//! ```

use alloc::string::String;
use alloc::vec::Vec;

/// Container for 1+ type definitions from a `<types>` block.
///
/// Spec §7.3.3.4: a DDS-XML document may have multiple `<types>` top-level
/// elements. Each block is modeled as a `TypeLibrary` with an
/// optional name (annotation, not spec-mandatory).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeLibrary {
    /// Optional library name (e.g. `<types name="Lib1">`); the spec allows
    /// the attribute without making it mandatory.
    pub name: String,
    /// Type definitions in document order. Modules are embedded as
    /// `TypeDef::Module(ModuleEntry)` (nested).
    pub types: Vec<TypeDef>,
}

impl TypeLibrary {
    /// Look up a top-level type by its local name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.name() == name)
    }
}

/// A single type entry (Spec §7.3.3.4 — struct/enum/union/typedef/
/// bitmask/bitset or a nested module).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDef {
    /// `<struct>` — XTypes aggregated type.
    Struct(StructType),
    /// `<enum>` — XTypes enumerated type.
    Enum(EnumType),
    /// `<union>` — XTypes union type.
    Union(UnionType),
    /// `<typedef>` — type alias with optional array/sequence modifiers.
    Typedef(TypedefType),
    /// `<bitmask>` — XTypes bitmask type.
    Bitmask(BitmaskType),
    /// `<bitset>` — XTypes bitset type.
    Bitset(BitsetType),
    /// `<module>` — namespacing container; further types nested inside.
    Module(ModuleEntry),
    /// `<include>` — pull in an external XML file (DDS-XML 1.0 §7.3.3.4 +
    /// XTypes 1.3 §7.3.2). Captured as a marker during parse; a resolver
    /// can resolve it later for composition.
    Include(IncludeEntry),
    /// `<forward_dcl>` — forward decl without members (XTypes 1.3 §7.3.2).
    /// Allows mutually recursive type refs.
    ForwardDcl(ForwardDeclEntry),
    /// `<const>` — constant definition (XTypes 1.3 §7.3.2 / IDL 4.2
    /// §7.4.1.4.4). Value as a string; the caller converts.
    Const(ConstEntry),
}

/// `<include file="..."/>` — XML composition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IncludeEntry {
    /// File path relative to the including XML.
    pub file: String,
}

/// `<forward_dcl name="T" kind="STRUCT|UNION"/>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ForwardDeclEntry {
    /// Type name.
    pub name: String,
    /// "STRUCT" or "UNION" — the spec allows only these two.
    pub kind: String,
}

/// `<const name="X" type="long" value="42"/>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstEntry {
    /// Constant name.
    pub name: String,
    /// Type (primitive string like "long", "string", etc.).
    pub type_name: String,
    /// Raw value as a string.
    pub value: String,
}

impl TypeDef {
    /// Local name of the type/module.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Struct(s) => &s.name,
            Self::Enum(e) => &e.name,
            Self::Union(u) => &u.name,
            Self::Typedef(t) => &t.name,
            Self::Bitmask(b) => &b.name,
            Self::Bitset(b) => &b.name,
            Self::Module(m) => &m.name,
            Self::Include(i) => &i.file,
            Self::ForwardDcl(f) => &f.name,
            Self::Const(c) => &c.name,
        }
    }
}

/// Nested module (`<module name="…">` — Spec §7.3.3.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleEntry {
    /// Module name.
    pub name: String,
    /// Nested type definitions.
    pub types: Vec<TypeDef>,
}

/// Extensibility-Annotation (`@final`, `@appendable`, `@mutable`) — Spec
/// §7.2.3.5 + §7.3.3.4.4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extensibility {
    /// `final` — no extending, byte-compatible with strict IDL templates.
    Final,
    /// `appendable` — spec default; only extensible at the end.
    Appendable,
    /// `mutable` — XCDR2 with member IDs, reordering allowed.
    Mutable,
}

impl Default for Extensibility {
    fn default() -> Self {
        Self::Appendable
    }
}

/// Primitive type symbol per Spec §7.2.1 + §7.3.3.4.4.2.
///
/// Strings/WStrings optionally carry the `stringMaxLength` attribute on the
/// member, but are represented in the TypeRef table as the unbounded variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    /// IDL `boolean`.
    Boolean,
    /// IDL `octet` (unsigned 8-bit).
    Octet,
    /// IDL `char` (8-bit).
    Char,
    /// IDL `wchar` (16-bit).
    WChar,
    /// IDL `short` (signed 16-bit).
    Short,
    /// IDL `unsigned short` / `ushort`.
    UShort,
    /// IDL `long` (signed 32-bit).
    Long,
    /// IDL `unsigned long` / `ulong`.
    ULong,
    /// IDL `long long` / `longlong`.
    LongLong,
    /// IDL `unsigned long long` / `ulonglong`.
    ULongLong,
    /// IDL `float` (32-bit IEEE 754).
    Float,
    /// IDL `double` (64-bit IEEE 754).
    Double,
    /// IDL `long double` (extended precision).
    LongDouble,
    /// IDL `string` (8-bit chars).
    String,
    /// IDL `wstring` (16-bit chars).
    WString,
}

impl PrimitiveType {
    /// Parses a primitive type symbol from the `type=…` attribute of the
    /// `<member>`/`<typedef>` elements.
    #[must_use]
    pub fn from_xml(s: &str) -> Option<Self> {
        match s {
            "boolean" => Some(Self::Boolean),
            "octet" | "byte" => Some(Self::Octet),
            "char" => Some(Self::Char),
            "wchar" => Some(Self::WChar),
            "short" | "int16" => Some(Self::Short),
            "ushort" | "uint16" => Some(Self::UShort),
            "long" | "int32" => Some(Self::Long),
            "ulong" | "uint32" => Some(Self::ULong),
            "longlong" | "int64" => Some(Self::LongLong),
            "ulonglong" | "uint64" => Some(Self::ULongLong),
            "float" | "float32" => Some(Self::Float),
            "double" | "float64" => Some(Self::Double),
            "longdouble" | "float128" => Some(Self::LongDouble),
            "string" => Some(Self::String),
            "wstring" => Some(Self::WString),
            _ => None,
        }
    }

    /// Canonical primitive type symbol (for round-trip serialization).
    #[must_use]
    pub fn as_xml(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Octet => "octet",
            Self::Char => "char",
            Self::WChar => "wchar",
            Self::Short => "short",
            Self::UShort => "ushort",
            Self::Long => "long",
            Self::ULong => "ulong",
            Self::LongLong => "longlong",
            Self::ULongLong => "ulonglong",
            Self::Float => "float",
            Self::Double => "double",
            Self::LongDouble => "longdouble",
            Self::String => "string",
            Self::WString => "wstring",
        }
    }
}

/// Type reference from a `type=…` attribute (member, typedef, bitfield
/// mask): either a primitive symbol or a named reference to a
/// user-defined type (`MyModule::State`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    /// Primitive (see [`PrimitiveType`]).
    Primitive(PrimitiveType),
    /// Named (e.g. `MyEnum`, `MyModule::State`).
    Named(String),
}

/// `<struct>` definition (Spec §7.3.3.4.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StructType {
    /// Struct name (attribute `name`).
    pub name: String,
    /// Optional extensibility mode.
    pub extensibility: Option<Extensibility>,
    /// Optional base struct (attribute `baseType` / `base_type`).
    pub base_type: Option<String>,
    /// Members in document order.
    pub members: Vec<StructMember>,
}

/// A single struct member (`<member …/>` — Spec §7.3.3.4.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructMember {
    /// Member name.
    pub name: String,
    /// Type reference.
    pub type_ref: TypeRef,
    /// `@key` annotation (attribute `key="true"`).
    pub key: bool,
    /// `@optional` annotation (attribute `optional="true"`).
    pub optional: bool,
    /// `@must_understand` annotation.
    pub must_understand: bool,
    /// Optional member-ID override for XCDR2 (attribute `id`).
    pub id: Option<u32>,
    /// `stringMaxLength` attribute (bounded string/WString limit).
    pub string_max_length: Option<u32>,
    /// `sequenceMaxLength` attribute (bounded sequence limit).
    pub sequence_max_length: Option<u32>,
    /// `arrayDimensions` attribute, parsed from `"3,4"` -> `vec![3,4]`.
    /// Empty if not an array.
    pub array_dimensions: Vec<u32>,
}

impl Default for StructMember {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_ref: TypeRef::Primitive(PrimitiveType::Long),
            key: false,
            optional: false,
            must_understand: false,
            id: None,
            string_max_length: None,
            sequence_max_length: None,
            array_dimensions: Vec::new(),
        }
    }
}

/// `<enum>`-Definition (Spec §7.3.3.4.5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnumType {
    /// Enum-Name.
    pub name: String,
    /// Optionales `bitBound`-Attribut (default 32).
    pub bit_bound: Option<u32>,
    /// Enumerator entries.
    pub enumerators: Vec<EnumLiteral>,
}

/// Single `<enumerator>` entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnumLiteral {
    /// Symbolischer Name.
    pub name: String,
    /// Numeric value; `None` means implicit auto-numbering
    /// (previous + 1, starting at 0).
    pub value: Option<i32>,
}

/// `<union>`-Definition (Spec §7.3.3.4.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionType {
    /// Union-Name.
    pub name: String,
    /// Discriminator type reference (e.g. `long`, `short`, `MyEnum`).
    pub discriminator: TypeRef,
    /// Cases in document order.
    pub cases: Vec<UnionCase>,
}

impl Default for UnionType {
    fn default() -> Self {
        Self {
            name: String::new(),
            discriminator: TypeRef::Primitive(PrimitiveType::Long),
            cases: Vec::new(),
        }
    }
}

/// `<case>` entry (Spec §7.3.3.4.6.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnionCase {
    /// Discriminator values for this case.
    /// Multiple `<caseDiscriminator>` children possible.
    pub discriminators: Vec<UnionDiscriminator>,
    /// Member selected when the discriminator is active.
    pub member: StructMember,
}

/// Discriminator value of a case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnionDiscriminator {
    /// Numeric literal (string-encoded from the XML).
    Value(String),
    /// `default` branch (Spec §7.3.3.4.6.1.2).
    Default,
}

/// `<typedef>` definition (Spec §7.3.3.4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedefType {
    /// Typedef name.
    pub name: String,
    /// Aliased type.
    pub type_ref: TypeRef,
    /// `arrayDimensions` (empty if not an array).
    pub array_dimensions: Vec<u32>,
    /// `sequenceMaxLength` if it is a sequence alias.
    pub sequence_max_length: Option<u32>,
    /// `stringMaxLength` if it is a bounded string.
    pub string_max_length: Option<u32>,
}

impl Default for TypedefType {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_ref: TypeRef::Primitive(PrimitiveType::Long),
            array_dimensions: Vec::new(),
            sequence_max_length: None,
            string_max_length: None,
        }
    }
}

/// `<bitmask>`-Definition (Spec §7.3.3.4.8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitmaskType {
    /// Bitmask-Name.
    pub name: String,
    /// `bitBound`-Attribut (default 32).
    pub bit_bound: Option<u32>,
    /// `<bit_value>` entries.
    pub bit_values: Vec<BitValue>,
}

/// Single `<bit_value>` entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitValue {
    /// Symbolischer Name.
    pub name: String,
    /// Bit position (0-based). `None` -> implicit (previous + 1).
    pub position: Option<u32>,
}

/// `<bitset>`-Definition (Spec §7.3.3.4.9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitsetType {
    /// Bitset-Name.
    pub name: String,
    /// `<bitfield>` entries.
    pub bit_fields: Vec<BitField>,
}

/// Single `<bitfield>` element of a bitset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitField {
    /// Feldname.
    pub name: String,
    /// Underlying Type.
    pub type_ref: TypeRef,
    /// Bitmask as a string (e.g. `"0x06"`); we preserve the exact token
    /// to keep round-trips stable.
    pub mask: String,
}

impl Default for BitField {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_ref: TypeRef::Primitive(PrimitiveType::ULong),
            mask: String::new(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn primitive_type_roundtrip() {
        for p in [
            PrimitiveType::Boolean,
            PrimitiveType::Octet,
            PrimitiveType::Char,
            PrimitiveType::WChar,
            PrimitiveType::Short,
            PrimitiveType::UShort,
            PrimitiveType::Long,
            PrimitiveType::ULong,
            PrimitiveType::LongLong,
            PrimitiveType::ULongLong,
            PrimitiveType::Float,
            PrimitiveType::Double,
            PrimitiveType::LongDouble,
            PrimitiveType::String,
            PrimitiveType::WString,
        ] {
            let s = p.as_xml();
            assert_eq!(PrimitiveType::from_xml(s), Some(p));
        }
    }

    #[test]
    fn primitive_aliases() {
        assert_eq!(PrimitiveType::from_xml("int32"), Some(PrimitiveType::Long));
        assert_eq!(PrimitiveType::from_xml("byte"), Some(PrimitiveType::Octet));
        assert_eq!(
            PrimitiveType::from_xml("uint16"),
            Some(PrimitiveType::UShort)
        );
    }

    #[test]
    fn extensibility_default_appendable() {
        assert_eq!(Extensibility::default(), Extensibility::Appendable);
    }

    #[test]
    fn typedef_name_lookup() {
        let lib = TypeLibrary {
            name: "L".into(),
            types: alloc::vec![TypeDef::Typedef(TypedefType {
                name: "Velocity".into(),
                ..Default::default()
            })],
        };
        assert!(lib.find("Velocity").is_some());
        assert!(lib.find("Missing").is_none());
    }
}
