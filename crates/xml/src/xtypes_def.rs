// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.3 Building Block "Types" — Datenmodell.
//!
//! Beschreibungs-Datenmodell der XML-Sicht auf XTypes 1.3 Type-Definitionen.
//!
//! `zerodds-xml` haelt bewusst eine *eigenstaendige* Beschreibungsschicht und
//! macht **keine** Cross-Crate-Edge zu `zerodds-types::DynamicType`. Hoeher
//! liegende Adapter koennen die hier gehaltenen Strukturen spaeter in
//! ihre DCPS-Datenmodelle uebersetzen.
//!
//! Spec-Quelle: OMG DDS-XML 1.0 §7.3.3 (Types Building Block).
//!
//! # XML → Rust-Type Mapping
//!
//! ```text
//! <types>                          | Vec<TypeLibrary> (multiple <types> erlaubt)
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

/// Container fuer 1+ Type-Definitionen aus einem `<types>`-Block.
///
/// Spec §7.3.3.4: ein DDS-XML-Dokument darf mehrere `<types>`-Top-Level-
/// Elemente fuehren. Jeder Block wird als eine `TypeLibrary` mit
/// optionalem Namen (Annotation, nicht Spec-Pflicht) modelliert.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeLibrary {
    /// Optionaler Library-Name (z.B. `<types name="Lib1">`); Spec laesst
    /// das Attribut zu, ohne es verpflichtend zu machen.
    pub name: String,
    /// Type-Definitionen in Dokument-Reihenfolge. Module sind als
    /// `TypeDef::Module(ModuleEntry)` eingebettet (nested).
    pub types: Vec<TypeDef>,
}

impl TypeLibrary {
    /// Lookup eines Top-Level-Types ueber seinen lokalen Namen.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.name() == name)
    }
}

/// Ein einzelner Type-Eintrag (Spec §7.3.3.4 — Struct/Enum/Union/Typedef/
/// Bitmask/Bitset oder geschachteltes Modul).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDef {
    /// `<struct>` — XTypes Aggregated Type.
    Struct(StructType),
    /// `<enum>` — XTypes Enumerated Type.
    Enum(EnumType),
    /// `<union>` — XTypes Union Type.
    Union(UnionType),
    /// `<typedef>` — Type-Alias mit optionalen Array/Sequence-Modifiern.
    Typedef(TypedefType),
    /// `<bitmask>` — XTypes Bitmask Type.
    Bitmask(BitmaskType),
    /// `<bitset>` — XTypes Bitset Type.
    Bitset(BitsetType),
    /// `<module>` — Namespacing-Container; weitere Types nested inside.
    Module(ModuleEntry),
    /// `<include>` — externe XML-Datei einziehen (DDS-XML 1.0 §7.3.3.4 +
    /// XTypes 1.3 §7.3.2). Wird beim parse als Marker erfasst; Resolver
    /// kann ihn nachgelagert zur Composition aufloesen.
    Include(IncludeEntry),
    /// `<forward_dcl>` — Forward-Decl ohne Members (XTypes 1.3 §7.3.2).
    /// Erlaubt mutual-recursive Type-Refs.
    ForwardDcl(ForwardDeclEntry),
    /// `<const>` — Konstanten-Definition (XTypes 1.3 §7.3.2 / IDL 4.2
    /// §7.4.1.4.4). Wert als String; Caller konvertiert.
    Const(ConstEntry),
}

/// `<include file="..."/>` — XML-Composition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IncludeEntry {
    /// Datei-Pfad relativ zur einschliessenden XML.
    pub file: String,
}

/// `<forward_dcl name="T" kind="STRUCT|UNION"/>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ForwardDeclEntry {
    /// Type-Name.
    pub name: String,
    /// "STRUCT" oder "UNION" — Spec laesst nur diese zwei zu.
    pub kind: String,
}

/// `<const name="X" type="long" value="42"/>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstEntry {
    /// Konstanten-Name.
    pub name: String,
    /// Type (Primitive-String wie "long", "string", etc.).
    pub type_name: String,
    /// Roher Wert als String.
    pub value: String,
}

impl TypeDef {
    /// Lokaler Name des Types/Moduls.
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

/// Geschachteltes Modul (`<module name="…">` — Spec §7.3.3.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleEntry {
    /// Modul-Name.
    pub name: String,
    /// Geschachtelte Type-Definitionen.
    pub types: Vec<TypeDef>,
}

/// Extensibility-Annotation (`@final`, `@appendable`, `@mutable`) — Spec
/// §7.2.3.5 + §7.3.3.4.4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extensibility {
    /// `final` — kein Erweitern, byte-kompatibel zu strikten IDL-Vorlagen.
    Final,
    /// `appendable` — Spec-Default; nur am Ende erweiterbar.
    Appendable,
    /// `mutable` — XCDR2 mit Member-IDs, Reordering erlaubt.
    Mutable,
}

impl Default for Extensibility {
    fn default() -> Self {
        Self::Appendable
    }
}

/// Primitive-Type-Symbol gemaess Spec §7.2.1 + §7.3.3.4.4.2.
///
/// Strings/WStrings tragen optional `stringMaxLength`-Attribut am Member,
/// aber sind in der TypeRef-Tabelle als unbounded-Variante dargestellt.
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
    /// Parst ein Primitive-Type-Symbol aus dem `type=…`-Attribut der
    /// `<member>`/`<typedef>`-Elemente.
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

    /// Kanonisches Primitive-Type-Symbol (fuer Round-Trip-Serialisierung).
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

/// Type-Verweis aus einem `type=…`-Attribut (Member, Typedef, Bitfield-
/// Mask): entweder Primitive-Symbol oder Named-Reference auf einen
/// nutzer-definierten Type (`MyModule::State`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    /// Primitive (siehe [`PrimitiveType`]).
    Primitive(PrimitiveType),
    /// Named (z.B. `MyEnum`, `MyModule::State`).
    Named(String),
}

/// `<struct>`-Definition (Spec §7.3.3.4.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StructType {
    /// Struct-Name (Attribut `name`).
    pub name: String,
    /// Optionaler Extensibility-Mode.
    pub extensibility: Option<Extensibility>,
    /// Optionaler Base-Struct (Attribut `baseType` / `base_type`).
    pub base_type: Option<String>,
    /// Member in Dokument-Reihenfolge.
    pub members: Vec<StructMember>,
}

/// Einzelner Struct-Member (`<member …/>` — Spec §7.3.3.4.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructMember {
    /// Member-Name.
    pub name: String,
    /// Type-Verweis.
    pub type_ref: TypeRef,
    /// `@key`-Annotation (Attribut `key="true"`).
    pub key: bool,
    /// `@optional`-Annotation (Attribut `optional="true"`).
    pub optional: bool,
    /// `@must_understand`-Annotation.
    pub must_understand: bool,
    /// Optionaler Member-ID-Override fuer XCDR2 (Attribut `id`).
    pub id: Option<u32>,
    /// `stringMaxLength`-Attribut (Bounded-String/WString-Limit).
    pub string_max_length: Option<u32>,
    /// `sequenceMaxLength`-Attribut (Bounded-Sequence-Limit).
    pub sequence_max_length: Option<u32>,
    /// `arrayDimensions`-Attribut, parsed aus `"3,4"` -> `vec![3,4]`.
    /// Leer wenn kein Array.
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
    /// Enumerator-Eintraege.
    pub enumerators: Vec<EnumLiteral>,
}

/// Einzelner `<enumerator>`-Eintrag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnumLiteral {
    /// Symbolischer Name.
    pub name: String,
    /// Numerischer Wert; `None` bedeutet implicit auto-numbering
    /// (vorheriger + 1, beginnend bei 0).
    pub value: Option<i32>,
}

/// `<union>`-Definition (Spec §7.3.3.4.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionType {
    /// Union-Name.
    pub name: String,
    /// Discriminator-Type-Verweis (z.B. `long`, `short`, `MyEnum`).
    pub discriminator: TypeRef,
    /// Cases in Dokument-Reihenfolge.
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

/// `<case>`-Eintrag (Spec §7.3.3.4.6.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnionCase {
    /// Discriminator-Werte fuer diesen Case.
    /// Mehrere `<caseDiscriminator>`-Children moeglich.
    pub discriminators: Vec<UnionDiscriminator>,
    /// Member, der bei aktivem Discriminator gewaehlt ist.
    pub member: StructMember,
}

/// Discriminator-Wert eines Cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnionDiscriminator {
    /// Numerischer Literal (string-encoded vom XML).
    Value(String),
    /// `default`-Branch (Spec §7.3.3.4.6.1.2).
    Default,
}

/// `<typedef>`-Definition (Spec §7.3.3.4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedefType {
    /// Typedef-Name.
    pub name: String,
    /// Aliased Type.
    pub type_ref: TypeRef,
    /// `arrayDimensions` (leer wenn kein Array).
    pub array_dimensions: Vec<u32>,
    /// `sequenceMaxLength` falls Sequence-Alias.
    pub sequence_max_length: Option<u32>,
    /// `stringMaxLength` falls Bounded-String.
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
    /// `<bit_value>`-Eintraege.
    pub bit_values: Vec<BitValue>,
}

/// Einzelner `<bit_value>`-Eintrag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitValue {
    /// Symbolischer Name.
    pub name: String,
    /// Bit-Position (0-based). `None` -> implicit (vorheriger + 1).
    pub position: Option<u32>,
}

/// `<bitset>`-Definition (Spec §7.3.3.4.9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitsetType {
    /// Bitset-Name.
    pub name: String,
    /// `<bitfield>`-Eintraege.
    pub bit_fields: Vec<BitField>,
}

/// Einzelnes `<bitfield>`-Element eines Bitsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitField {
    /// Feldname.
    pub name: String,
    /// Underlying Type.
    pub type_ref: TypeRef,
    /// Bitmaske als String (z.B. `"0x06"`); wir bewahren das exakte Token,
    /// um Round-Trip stabil zu halten.
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
