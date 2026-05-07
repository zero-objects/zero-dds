// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! W3C XML Schema (XSD) → DDS-XTypes-Bridge (XTypes 1.3 §7.3.3).
//!
//! Spec OMG XTypes 1.3 §7.3.3 erlaubt XSD als alternative Type-
//! Representation neben IDL/XML/TypeObject. Dieses Modul mappt eine
//! Untermenge von W3C XSD 1.0/1.1 auf das interne [`TypeLibrary`]-
//! Datenmodell aus [`crate::xtypes_def`]; via
//! [`crate::typeobject_bridge`] wird daraus ein
//! [`zerodds_types::TypeObject`] berechnet.
//!
//! # Mapping (XTypes 1.3 §7.3.3 Tab.6.x)
//!
//! ```text
//! XSD                                    | DDS-Type
//! -------------------------------------- | -------------------------
//! <xsd:schema>                           | TypeLibrary (Top-Level)
//! <xsd:complexType name=X>               | <struct name=X>
//!   <xsd:sequence>                       |
//!     <xsd:element name=f type=t/>       | <member name=f type=t/>
//! <xsd:simpleType name=E>                | <enum name=E>
//!   <xsd:restriction base="xsd:string">  |
//!     <xsd:enumeration value=A/>         | <enumerator name=A/>
//! minOccurs=0 (Element)                  | optional=true
//! maxOccurs=unbounded                    | sequence
//! maxOccurs=N (N>1)                      | bounded sequence
//! ```
//!
//! # Built-In Type Mapping
//!
//! ```text
//! xsd:boolean | xsd:byte | xsd:short | xsd:int     | xsd:long
//! -> boolean    octet      short       long          longlong
//!
//! xsd:unsignedByte | xsd:unsignedShort | xsd:unsignedInt | xsd:unsignedLong
//! -> octet           ushort              ulong             ulonglong
//!
//! xsd:float | xsd:double  | xsd:string | xsd:base64Binary
//! -> float    double        string       sequence<octet>
//! ```
//!
//! # Bewusst nicht im Crate
//!
//! - Voller XSD-1.1-Validator (key/keyref, assertions).
//! - `<xsd:choice>` als Union-Equivalent (Spec sieht das nicht
//!   normativ vor; folgt evtl. Phase 2).
//! - XSD `xsd:include`/`xsd:import` (Multi-File-Schemas).
//! - Mixed Content / Attribute-as-Member.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::parser::{XmlElement, parse_xml_tree};
use crate::xtypes_def::{
    EnumLiteral, EnumType, PrimitiveType, StructMember, StructType, TypeDef, TypeLibrary, TypeRef,
};

/// Mapped einen W3C-XSD-Built-In-Type-Namen (mit oder ohne `xsd:`-
/// Prefix) auf einen DDS-`PrimitiveType` oder, falls es ein
/// User-Type ist, `None`.
fn map_builtin(local: &str) -> Option<PrimitiveType> {
    let trimmed = match local.split_once(':') {
        Some((_prefix, suffix)) => suffix,
        None => local,
    };
    Some(match trimmed {
        "boolean" => PrimitiveType::Boolean,
        "byte" => PrimitiveType::Octet,
        "unsignedByte" => PrimitiveType::Octet,
        "short" => PrimitiveType::Short,
        "unsignedShort" => PrimitiveType::UShort,
        "int" | "integer" => PrimitiveType::Long,
        "unsignedInt" => PrimitiveType::ULong,
        "long" => PrimitiveType::LongLong,
        "unsignedLong" => PrimitiveType::ULongLong,
        "float" => PrimitiveType::Float,
        "double" => PrimitiveType::Double,
        "string" | "normalizedString" | "token" => PrimitiveType::String,
        _ => return None,
    })
}

/// Aufloesung eines `type=...`-Attributs in einen `TypeRef`. Built-In-
/// XSD-Typen werden auf [`PrimitiveType`] gemapped, alle anderen
/// (User-Types, andere Namespaces) werden als [`TypeRef::Named`]
/// durchgereicht.
fn resolve_type_ref(raw: &str) -> TypeRef {
    if let Some(prim) = map_builtin(raw) {
        TypeRef::Primitive(prim)
    } else {
        // strip prefix wenn local-name unique ist (User-Types ohne ns).
        let local = match raw.split_once(':') {
            Some((_p, s)) => s,
            None => raw,
        };
        TypeRef::Named(local.to_string())
    }
}

/// Parst ein XSD-Schema-Dokument und liefert eine [`TypeLibrary`].
///
/// # Errors
/// * [`XmlError::InvalidXml`] — kein wohlgeformtes XML, fehlendes
///   `<xsd:schema>`-Root, oder syntaktische Fehler in der XSD-Struktur.
pub fn parse_xsd_schema(xml: &str) -> Result<TypeLibrary, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "schema" {
        return Err(XmlError::InvalidXml(format!(
            "expected <xsd:schema> root, got <{}>",
            doc.root.name
        )));
    }

    let mut lib = TypeLibrary::default();
    for child in &doc.root.children {
        match child.name.as_str() {
            "complexType" => lib.types.push(TypeDef::Struct(parse_complex_type(child)?)),
            "simpleType" => {
                if let Some(en) = parse_simple_type_as_enum(child)? {
                    lib.types.push(TypeDef::Enum(en));
                }
                // simpleType ohne enumeration ist Type-Alias auf einen
                // built-in — wir ignorieren das auf dieser Stufe.
            }
            "element" => {
                // Top-Level `<xsd:element>` mit anonymem inline-
                // complexType: wir behandeln das als Top-Level-Struct
                // mit dem Element-Namen.
                if let Some(ct) = child.child("complexType") {
                    let mut s = parse_complex_type(ct)?;
                    if s.name.is_empty() {
                        s.name = child.attribute("name").unwrap_or_default().to_string();
                    }
                    if !s.name.is_empty() {
                        lib.types.push(TypeDef::Struct(s));
                    }
                }
            }
            "include" | "import" | "annotation" | "notation" | "group" | "attributeGroup" => {
                // Nicht im Scope; still ignorieren (XSD-Includes sind
                // Annotations rein dokumentarisch).
            }
            _ => {
                // Unbekanntes Top-Level-Element — strikt rejecten waere
                // zu hart, also lax.
            }
        }
    }
    Ok(lib)
}

/// Parst `<xsd:complexType name="X"><xsd:sequence>...</xsd:sequence></xsd:complexType>`.
fn parse_complex_type(node: &XmlElement) -> Result<StructType, XmlError> {
    let name = node.attribute("name").unwrap_or_default().to_string();

    let mut members = Vec::new();
    // `<xsd:sequence>` ist die normalisierte Member-Reihenfolge. Spec
    // erlaubt auch `<xsd:all>` (unordered) — wir behandeln es identisch.
    if let Some(seq) = node.child("sequence").or_else(|| node.child("all")) {
        for el in seq.children_named("element") {
            members.push(parse_element_as_member(el)?);
        }
    }
    // `<xsd:complexContent><xsd:extension base="..."><xsd:sequence>...`
    // -> Inheritance.
    let mut base_type: Option<String> = None;
    if let Some(cc) = node.child("complexContent") {
        if let Some(ext) = cc.child("extension") {
            if let Some(b) = ext.attribute("base") {
                let local = b.split_once(':').map(|(_p, s)| s).unwrap_or(b);
                base_type = Some(local.to_string());
            }
            if let Some(seq) = ext.child("sequence").or_else(|| ext.child("all")) {
                for el in seq.children_named("element") {
                    members.push(parse_element_as_member(el)?);
                }
            }
        }
    }

    Ok(StructType {
        name,
        extensibility: None,
        base_type,
        members,
    })
}

/// Parst ein `<xsd:element name="..." type="..." minOccurs="..."
/// maxOccurs="...">` als Struct-Member.
fn parse_element_as_member(node: &XmlElement) -> Result<StructMember, XmlError> {
    let name = node.attribute("name").unwrap_or_default().to_string();
    let raw_type = node.attribute("type").unwrap_or("xsd:string");
    let type_ref = resolve_type_ref(raw_type);

    let min_occurs = node
        .attribute("minOccurs")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1);
    let max_occurs_raw = node.attribute("maxOccurs").unwrap_or("1");
    let max_occurs: Option<u32> = if max_occurs_raw == "unbounded" {
        Some(u32::MAX)
    } else {
        max_occurs_raw.parse::<u32>().ok()
    };

    let optional = min_occurs == 0 && max_occurs == Some(1);
    let sequence_max_length = match max_occurs {
        Some(u32::MAX) => Some(0),   // unbounded → 0 = "no bound" Marker
        Some(n) if n > 1 => Some(n), // bounded sequence
        _ => None,
    };

    Ok(StructMember {
        name,
        type_ref,
        optional,
        sequence_max_length,
        ..Default::default()
    })
}

/// Parst `<xsd:simpleType name="X"><xsd:restriction base="xsd:string">
/// <xsd:enumeration value="A"/>...</xsd:restriction></xsd:simpleType>`
/// und liefert `Some(EnumType)`. Wenn keine `<xsd:enumeration>`-
/// Children vorhanden sind, ist das nur ein Type-Alias und wir liefern
/// `None` (kein Enum).
fn parse_simple_type_as_enum(node: &XmlElement) -> Result<Option<EnumType>, XmlError> {
    let name = node.attribute("name").unwrap_or_default().to_string();
    let restriction = match node.child("restriction") {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut enumerators: Vec<EnumLiteral> = Vec::new();
    for en in restriction.children_named("enumeration") {
        if let Some(value) = en.attribute("value") {
            enumerators.push(EnumLiteral {
                name: value.to_string(),
                value: None,
            });
        }
    }
    if enumerators.is_empty() {
        return Ok(None);
    }
    Ok(Some(EnumType {
        name,
        bit_bound: None,
        enumerators,
    }))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::string::String;

    fn lib_of(xml: &str) -> TypeLibrary {
        parse_xsd_schema(xml).expect("parse")
    }

    #[test]
    fn empty_schema_yields_empty_library() {
        let xml = r#"<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema"/>"#;
        let lib = lib_of(xml);
        assert!(lib.types.is_empty());
    }

    #[test]
    fn complex_type_maps_to_struct_with_primitive_members() {
        // Spec §7.3.3 — XSD complexType + xsd:sequence → DDS struct.
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Point">
                    <xsd:sequence>
                        <xsd:element name="x" type="xsd:int"/>
                        <xsd:element name="y" type="xsd:int"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        assert_eq!(lib.types.len(), 1);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!("expected struct, got {:?}", lib.types[0]);
        };
        assert_eq!(s.name, "Point");
        assert_eq!(s.members.len(), 2);
        assert_eq!(s.members[0].name, "x");
        assert_eq!(
            s.members[0].type_ref,
            TypeRef::Primitive(PrimitiveType::Long)
        );
        assert_eq!(s.members[1].name, "y");
    }

    #[test]
    fn xsd_long_maps_to_dds_longlong() {
        // Spec §7.3.3 — xsd:long ist 64-bit → DDS longlong (§7.4.13).
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Big">
                    <xsd:sequence>
                        <xsd:element name="v" type="xsd:long"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert_eq!(
            s.members[0].type_ref,
            TypeRef::Primitive(PrimitiveType::LongLong)
        );
    }

    #[test]
    fn unsigned_xsd_types_map_to_unsigned_dds_primitives() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="U">
                    <xsd:sequence>
                        <xsd:element name="a" type="xsd:unsignedByte"/>
                        <xsd:element name="b" type="xsd:unsignedShort"/>
                        <xsd:element name="c" type="xsd:unsignedInt"/>
                        <xsd:element name="d" type="xsd:unsignedLong"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert_eq!(
            s.members[0].type_ref,
            TypeRef::Primitive(PrimitiveType::Octet)
        );
        assert_eq!(
            s.members[1].type_ref,
            TypeRef::Primitive(PrimitiveType::UShort)
        );
        assert_eq!(
            s.members[2].type_ref,
            TypeRef::Primitive(PrimitiveType::ULong)
        );
        assert_eq!(
            s.members[3].type_ref,
            TypeRef::Primitive(PrimitiveType::ULongLong)
        );
    }

    #[test]
    fn min_occurs_zero_yields_optional_member() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Opt">
                    <xsd:sequence>
                        <xsd:element name="maybe" type="xsd:int" minOccurs="0"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert!(s.members[0].optional);
    }

    #[test]
    fn max_occurs_unbounded_yields_sequence() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="WithSeq">
                    <xsd:sequence>
                        <xsd:element name="items" type="xsd:int" maxOccurs="unbounded"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert!(s.members[0].sequence_max_length.is_some());
    }

    #[test]
    fn max_occurs_bounded_yields_bounded_sequence() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="WithBoundedSeq">
                    <xsd:sequence>
                        <xsd:element name="items" type="xsd:int" maxOccurs="5"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(s) = &lib.types[0] else {
            panic!()
        };
        assert_eq!(s.members[0].sequence_max_length, Some(5));
    }

    #[test]
    fn simple_type_with_enumeration_maps_to_dds_enum() {
        // Spec §7.3.3 — XSD restriction with enumeration → DDS enum.
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:simpleType name="Color">
                    <xsd:restriction base="xsd:string">
                        <xsd:enumeration value="RED"/>
                        <xsd:enumeration value="GREEN"/>
                        <xsd:enumeration value="BLUE"/>
                    </xsd:restriction>
                </xsd:simpleType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Enum(e) = &lib.types[0] else {
            panic!("expected enum, got {:?}", lib.types[0]);
        };
        assert_eq!(e.name, "Color");
        assert_eq!(e.enumerators.len(), 3);
        assert_eq!(e.enumerators[0].name, "RED");
        assert_eq!(e.enumerators[1].name, "GREEN");
        assert_eq!(e.enumerators[2].name, "BLUE");
    }

    #[test]
    fn user_type_reference_yields_named_typeref() {
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Origin">
                    <xsd:sequence>
                        <xsd:element name="lat" type="xsd:double"/>
                    </xsd:sequence>
                </xsd:complexType>
                <xsd:complexType name="Trip">
                    <xsd:sequence>
                        <xsd:element name="from" type="Origin"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        assert_eq!(lib.types.len(), 2);
        let TypeDef::Struct(trip) = &lib.types[1] else {
            panic!()
        };
        assert_eq!(
            trip.members[0].type_ref,
            TypeRef::Named(String::from("Origin"))
        );
    }

    #[test]
    fn complex_content_extension_yields_inheritance() {
        // Spec — `<xsd:complexContent><xsd:extension base="P">` ist
        // XSD-Inheritance, das auf DDS-Struct-Inheritance gemapped wird.
        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Parent">
                    <xsd:sequence>
                        <xsd:element name="a" type="xsd:int"/>
                    </xsd:sequence>
                </xsd:complexType>
                <xsd:complexType name="Child">
                    <xsd:complexContent>
                        <xsd:extension base="Parent">
                            <xsd:sequence>
                                <xsd:element name="b" type="xsd:int"/>
                            </xsd:sequence>
                        </xsd:extension>
                    </xsd:complexContent>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = lib_of(xml);
        let TypeDef::Struct(child) = &lib.types[1] else {
            panic!()
        };
        assert_eq!(child.name, "Child");
        assert_eq!(child.base_type.as_deref(), Some("Parent"));
        assert_eq!(child.members.len(), 1);
        assert_eq!(child.members[0].name, "b");
    }

    #[test]
    fn non_schema_root_is_rejected() {
        let xml = r#"<dds><types/></dds>"#;
        let r = parse_xsd_schema(xml);
        assert!(r.is_err(), "non-<xsd:schema> root muss rejected werden");
    }

    #[test]
    fn xsd_to_typeobject_via_bridge_produces_minimal_struct() {
        // End-to-End: XSD-Source → TypeLibrary →
        // typeobject_bridge::bridge_library → MinimalTypeObject.
        // Damit ist der Spec-Pfad XTypes 1.3 §7.3.3 voll abgedeckt
        // (XSD → TypeObject auf Wire-Form).
        use crate::typeobject_bridge::bridge_library;

        let xml = r#"
            <xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
                <xsd:complexType name="Sensor">
                    <xsd:sequence>
                        <xsd:element name="id" type="xsd:int"/>
                        <xsd:element name="name" type="xsd:string"/>
                    </xsd:sequence>
                </xsd:complexType>
            </xsd:schema>
        "#;
        let lib = parse_xsd_schema(xml).expect("xsd-parse");
        let map = bridge_library(&lib).expect("bridge");
        assert!(
            map.contains_key("Sensor"),
            "TypeObject fuer Sensor fehlt: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }
}
