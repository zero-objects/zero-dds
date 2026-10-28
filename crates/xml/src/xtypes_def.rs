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
//! <flag name=… position=…>    | BitValue
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
    /// `<annotation>` — declaration of an annotation type (Annex A
    /// `annotationTypeDecl`).
    Annotation(AnnotationTypeDef),
}

/// The use of an annotation, `<annotate name="…">` with its `<member>`
/// values (Annex A `annotationDecl`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Annotation {
    /// Name of the annotation type.
    pub name: String,
    /// The `<member name="…" value="…"/>` children, in document order.
    pub members: Vec<AnnotationValue>,
}

/// One `<member>` of an [`Annotation`] (Annex A `annotationMemberValueDecl`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationValue {
    /// Member name.
    pub name: String,
    /// The `value` attribute; the expression as written.
    pub value: Option<String>,
}

/// A `<verbatim>` child (Annex A `verbatimDecl`): text for a target
/// language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verbatim {
    /// `language` attribute, `*` (every language) when absent.
    pub language: String,
    /// `placement` attribute, `before-declaration` when absent.
    pub placement: String,
    /// The `<text>` child.
    pub text: String,
}

/// The declaration of an annotation type, `<annotation name="…">` (Annex A
/// `annotationTypeDecl`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationTypeDef {
    /// Annotation name.
    pub name: String,
    /// `baseType` attribute.
    pub base_type: Option<String>,
    /// The members the annotation takes.
    pub members: Vec<AnnotationMemberDecl>,
}

/// A `<member>` of an [`AnnotationTypeDef`] (Annex A `simpleMemberDecl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationMemberDecl {
    /// Member name.
    pub name: String,
    /// Member type.
    pub type_ref: TypeRef,
    /// The default value as written. The XML Type Representation has none
    /// (Annex A `simpleMemberDecl`); the XSD one has (`default` of an
    /// `xsd:attribute`, §7.3.3.1.1).
    pub default_value: Option<String>,
}

/// `tryConstruct` of a member (Annex A `tryConstructKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryConstruct {
    /// `discard`.
    Discard,
    /// `use_default`.
    UseDefault,
    /// `trim`.
    Trim,
}

/// `autoid` of a type or module (Annex A `autoIdKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoId {
    /// `hash`.
    Hash,
    /// `sequencial` (the spelling of the schema).
    Sequential,
}

/// `<include file="..."/>` — XML composition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IncludeEntry {
    /// File path relative to the including XML.
    pub file: String,
}

/// `<forward_dcl name="T" kind="enum|struct|union|bitmask"/>`.
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
    /// The `stringMaxLength` attribute of a string constant.
    pub string_max_length: Option<String>,
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
            Self::Annotation(a) => &a.name,
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
    /// `autoid` attribute.
    pub auto_id: Option<AutoId>,
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
    /// IDL `octet` (unsigned 8-bit); XTypes `byte`.
    Octet,
    /// XTypes `int8` (signed 8-bit integer).
    Int8,
    /// XTypes `uint8` (unsigned 8-bit integer).
    UInt8,
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
            "int8" => Some(Self::Int8),
            "uint8" => Some(Self::UInt8),
            "char" | "char8" => Some(Self::Char),
            "wchar" | "char16" => Some(Self::WChar),
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
            Self::Int8 => "int8",
            Self::UInt8 => "uint8",
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
    /// `nested` attribute.
    pub nested: Option<bool>,
    /// `autoid` attribute.
    pub auto_id: Option<AutoId>,
    /// `<annotate>` children of the struct.
    pub annotations: Vec<Annotation>,
    /// `<verbatim>` children of the struct.
    pub verbatims: Vec<Verbatim>,
    /// `<const>` children of the struct, scoped to it (`Struct::NAME`).
    pub constants: Vec<ConstEntry>,
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
    /// `sequenceMaxLength` attribute: `Some` marks a sequence, `Some(0)`
    /// is the unbounded sequence (`-1` in the document), `None` a member
    /// that is no sequence.
    pub sequence_max_length: Option<u32>,
    /// `mapMaxLength` attribute (XTypes 1.3 §7.3.2.4.3): the bound of the
    /// map, `None` for the unbounded map (`-1` in the document). Whether
    /// the member is a map at all is [`Self::is_map`].
    pub map_max_length: Option<u32>,
    /// `mapKeyType`/`mapKeyNonBasicTypeName`: the map key's type.
    /// `Some` exactly when the member is a map.
    pub map_key: Option<TypeRef>,
    /// `mapKeyStringMaxLength`: the bound of a string key, if any.
    pub map_key_string_max_length: Option<u32>,
    /// `arrayDimensions` attribute, parsed from `"3,4"` -> `vec![3,4]`.
    /// Empty if not an array.
    pub array_dimensions: Vec<u32>,
    /// `external` attribute.
    pub external: Option<bool>,
    /// `tryConstruct` attribute.
    pub try_construct: Option<TryConstruct>,
    /// `nonSerialized` attribute.
    pub non_serialized: Option<bool>,
    /// `<annotate>` children of the member.
    pub annotations: Vec<Annotation>,
}

impl StructMember {
    /// `true` if the member is a map (XTypes 1.3 §7.3.2.4.3: the
    /// `mapKeyType` attribute is required for every map).
    #[must_use]
    pub fn is_map(&self) -> bool {
        self.map_key.is_some()
    }
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
            map_max_length: None,
            map_key: None,
            map_key_string_max_length: None,
            array_dimensions: Vec::new(),
            external: None,
            try_construct: None,
            non_serialized: None,
            annotations: Vec::new(),
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
    /// Optional `extensibility` attribute (`final` or `appendable`;
    /// default `appendable`).
    pub extensibility: Option<Extensibility>,
    /// Enumerator entries.
    pub enumerators: Vec<EnumLiteral>,
    /// `<annotate>` children of the enum.
    pub annotations: Vec<Annotation>,
    /// `<verbatim>` children of the enum.
    pub verbatims: Vec<Verbatim>,
}

/// Single `<enumerator>` entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnumLiteral {
    /// Symbolischer Name.
    pub name: String,
    /// Numeric value; `None` means implicit auto-numbering
    /// (previous + 1, starting at 0).
    pub value: Option<i32>,
    /// `defaultLiteral` attribute.
    pub default_literal: Option<bool>,
    /// `<annotate>` children of the enumerator.
    pub annotations: Vec<Annotation>,
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
    /// `extensibility` attribute.
    pub extensibility: Option<Extensibility>,
    /// `nested` attribute.
    pub nested: Option<bool>,
    /// `autoid` attribute.
    pub auto_id: Option<AutoId>,
    /// `key` attribute of the `<discriminator>`.
    pub discriminator_key: bool,
    /// `<annotate>` children of the `<discriminator>`.
    pub discriminator_annotations: Vec<Annotation>,
    /// `<annotate>` children of the union.
    pub annotations: Vec<Annotation>,
    /// `<verbatim>` children of the union.
    pub verbatims: Vec<Verbatim>,
}

impl Default for UnionType {
    fn default() -> Self {
        Self {
            name: String::new(),
            discriminator: TypeRef::Primitive(PrimitiveType::Long),
            cases: Vec::new(),
            extensibility: None,
            nested: None,
            auto_id: None,
            discriminator_key: false,
            discriminator_annotations: Vec::new(),
            annotations: Vec::new(),
            verbatims: Vec::new(),
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
    /// `sequenceMaxLength` if it is a sequence alias (`Some(0)`: unbounded).
    pub sequence_max_length: Option<u32>,
    /// `stringMaxLength` if it is a bounded string.
    pub string_max_length: Option<u32>,
    /// `mapMaxLength` of a map alias (`None`: unbounded, `-1` in the
    /// document). Whether the alias is a map is [`Self::is_map`].
    pub map_max_length: Option<u32>,
    /// `mapKeyType`/`mapKeyNonBasicTypeName` of a map alias.
    pub map_key: Option<TypeRef>,
    /// `mapKeyStringMaxLength`: the bound of a string key, if any.
    pub map_key_string_max_length: Option<u32>,
    /// `external` attribute.
    pub external: Option<bool>,
}

impl TypedefType {
    /// `true` if the alias is a map (XTypes 1.3 §7.3.2.4.3: `mapKeyType`
    /// is required for every map).
    #[must_use]
    pub fn is_map(&self) -> bool {
        self.map_key.is_some()
    }
}

impl Default for TypedefType {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_ref: TypeRef::Primitive(PrimitiveType::Long),
            array_dimensions: Vec::new(),
            sequence_max_length: None,
            string_max_length: None,
            map_max_length: None,
            map_key: None,
            map_key_string_max_length: None,
            external: None,
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
    /// `<flag>` entries.
    pub bit_values: Vec<BitValue>,
    /// `<annotate>` children of the bitmask.
    pub annotations: Vec<Annotation>,
}

/// Single `<flag>` entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitValue {
    /// Symbolischer Name.
    pub name: String,
    /// Bit position (0-based). `None` -> implicit (previous + 1).
    pub position: Option<u32>,
    /// `<annotate>` children of the flag.
    pub annotations: Vec<Annotation>,
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
