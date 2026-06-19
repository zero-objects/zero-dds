// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.3 Building Block "Types" — XML→AST-Parser.
//!
//! Reads `<types>` top-level blocks and produces a
//! [`Vec<TypeLibrary>`]. The module hierarchy is kept as nested
//! [`TypeDef::Module`] entries.
//!
//! Spec source: OMG DDS-XML 1.0 §7.3.3 (Types Building Block).

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::parser::{XmlElement, parse_xml_tree};
use crate::types::{parse_long, parse_ulong};
use crate::xtypes_def::{
    BitField, BitValue, BitmaskType, BitsetType, EnumLiteral, EnumType, Extensibility, ModuleEntry,
    PrimitiveType, StructMember, StructType, TypeDef, TypeLibrary, TypeRef, TypedefType, UnionCase,
    UnionDiscriminator, UnionType,
};

/// Parses all `<types>` top-level elements of a DDS-XML document.
///
/// The top-level wrapper `<dds>` is NOT required — `xml` may be directly
/// a `<types>` root element.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — not well-formed XML.
/// * [`XmlError::UnknownElement`] — unknown top-level element.
/// * [`XmlError::ValueOutOfRange`] — numeric attribute out of range.
pub fn parse_type_libraries(xml: &str) -> Result<Vec<TypeLibrary>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    let mut out = Vec::new();
    match doc.root.name.as_str() {
        "dds" => {
            for child in &doc.root.children {
                if child.name == "types" {
                    out.push(parse_types_element(child)?);
                }
            }
        }
        "types" => out.push(parse_types_element(&doc.root)?),
        other => {
            return Err(XmlError::InvalidXml(format!(
                "expected <dds> or <types> root, got <{other}>"
            )));
        }
    }
    Ok(out)
}

/// Parses a single `<types>` element into a `TypeLibrary`.
///
/// # Errors
/// As [`parse_type_libraries`].
pub fn parse_types_element(el: &XmlElement) -> Result<TypeLibrary, XmlError> {
    let name = el.attribute("name").unwrap_or("").to_string();
    let mut lib = TypeLibrary {
        name,
        types: Vec::new(),
    };
    for child in &el.children {
        lib.types.push(parse_type_def(child)?);
    }
    Ok(lib)
}

/// zerodds-lint: recursion-depth = module nesting depth (indirectly via
/// `parse_module`). The XML foundation's DoS cap limits the depth to ≤ 16.
fn parse_type_def(el: &XmlElement) -> Result<TypeDef, XmlError> {
    match el.name.as_str() {
        "module" => Ok(TypeDef::Module(parse_module(el)?)),
        "struct" => Ok(TypeDef::Struct(parse_struct(el)?)),
        "enum" => Ok(TypeDef::Enum(parse_enum(el)?)),
        "union" => Ok(TypeDef::Union(parse_union(el)?)),
        "typedef" => Ok(TypeDef::Typedef(parse_typedef(el)?)),
        "bitmask" => Ok(TypeDef::Bitmask(parse_bitmask(el)?)),
        "bitset" => Ok(TypeDef::Bitset(parse_bitset(el)?)),
        "include" => Ok(TypeDef::Include(parse_include(el)?)),
        "forward_dcl" | "forwardDcl" => Ok(TypeDef::ForwardDcl(parse_forward_dcl(el)?)),
        "const" => Ok(TypeDef::Const(parse_const(el)?)),
        other => Err(XmlError::UnknownElement(other.to_string())),
    }
}

fn parse_include(el: &XmlElement) -> Result<crate::xtypes_def::IncludeEntry, XmlError> {
    let file = require_attr(el, "file")?.to_string();
    Ok(crate::xtypes_def::IncludeEntry { file })
}

fn parse_forward_dcl(el: &XmlElement) -> Result<crate::xtypes_def::ForwardDeclEntry, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    // kind is optional; default "STRUCT".
    let kind = el.attribute("kind").unwrap_or("STRUCT").to_string();
    Ok(crate::xtypes_def::ForwardDeclEntry { name, kind })
}

fn parse_const(el: &XmlElement) -> Result<crate::xtypes_def::ConstEntry, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let type_name = require_attr(el, "type")?.to_string();
    let value = require_attr(el, "value")?.to_string();
    Ok(crate::xtypes_def::ConstEntry {
        name,
        type_name,
        value,
    })
}

/// zerodds-lint: recursion-depth = module nesting depth (indirectly via
/// `parse_type_def`). The XML foundation's DoS cap limits the depth to ≤ 16.
fn parse_module(el: &XmlElement) -> Result<ModuleEntry, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let mut types = Vec::new();
    for child in &el.children {
        types.push(parse_type_def(child)?);
    }
    Ok(ModuleEntry { name, types })
}

fn parse_struct(el: &XmlElement) -> Result<StructType, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let extensibility = el
        .attribute("extensibility")
        .map(parse_extensibility)
        .transpose()?;
    let base_type = el
        .attribute("baseType")
        .or_else(|| el.attribute("base_type"))
        .map(ToString::to_string);
    let mut members = Vec::new();
    for child in &el.children {
        if child.name == "member" {
            members.push(parse_member(child)?);
        }
    }
    Ok(StructType {
        name,
        extensibility,
        base_type,
        members,
    })
}

fn parse_extensibility(s: &str) -> Result<Extensibility, XmlError> {
    match s {
        "final" | "FINAL" => Ok(Extensibility::Final),
        "appendable" | "APPENDABLE" => Ok(Extensibility::Appendable),
        "mutable" | "MUTABLE" => Ok(Extensibility::Mutable),
        other => Err(XmlError::BadEnum(other.to_string())),
    }
}

fn parse_member(el: &XmlElement) -> Result<StructMember, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let type_str = require_attr(el, "type")?;
    let type_ref = parse_type_ref(type_str);
    let key = parse_attr_bool(el, "key")?;
    let optional = parse_attr_bool(el, "optional")?;
    let must_understand =
        parse_attr_bool(el, "mustUnderstand")? || parse_attr_bool(el, "must_understand")?;
    let id = el.attribute("id").map(parse_ulong).transpose()?;
    let string_max_length = el
        .attribute("stringMaxLength")
        .map(parse_ulong)
        .transpose()?;
    let sequence_max_length = el
        .attribute("sequenceMaxLength")
        .map(parse_ulong)
        .transpose()?;
    let array_dimensions = el
        .attribute("arrayDimensions")
        .map(parse_dimensions)
        .transpose()?
        .unwrap_or_default();
    Ok(StructMember {
        name,
        type_ref,
        key,
        optional,
        must_understand,
        id,
        string_max_length,
        sequence_max_length,
        array_dimensions,
    })
}

fn parse_type_ref(s: &str) -> TypeRef {
    if let Some(p) = PrimitiveType::from_xml(s) {
        TypeRef::Primitive(p)
    } else {
        TypeRef::Named(s.to_string())
    }
}

fn parse_attr_bool(el: &XmlElement, key: &str) -> Result<bool, XmlError> {
    match el.attribute(key) {
        None => Ok(false),
        Some(v) => crate::types::parse_bool(v),
    }
}

fn parse_dimensions(s: &str) -> Result<Vec<u32>, XmlError> {
    let mut out = Vec::new();
    for piece in s.split(',') {
        let t = piece.trim();
        if t.is_empty() {
            continue;
        }
        let v = parse_ulong(t)?;
        out.push(v);
    }
    Ok(out)
}

fn parse_enum(el: &XmlElement) -> Result<EnumType, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let bit_bound = el.attribute("bitBound").map(parse_ulong).transpose()?;
    let mut enumerators = Vec::new();
    for child in &el.children {
        if child.name == "enumerator" {
            let n = require_attr(child, "name")?.to_string();
            let value = child.attribute("value").map(parse_long).transpose()?;
            enumerators.push(EnumLiteral { name: n, value });
        }
    }
    Ok(EnumType {
        name,
        bit_bound,
        enumerators,
    })
}

fn parse_union(el: &XmlElement) -> Result<UnionType, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let disc_str = require_attr(el, "discriminator")?;
    let discriminator = parse_type_ref(disc_str);
    let mut cases = Vec::new();
    for child in el.children_named("case") {
        cases.push(parse_union_case(child)?);
    }
    Ok(UnionType {
        name,
        discriminator,
        cases,
    })
}

fn parse_union_case(el: &XmlElement) -> Result<UnionCase, XmlError> {
    let mut discriminators = Vec::new();
    let mut member: Option<StructMember> = None;
    for child in &el.children {
        match child.name.as_str() {
            "caseDiscriminator" => {
                let v = require_attr(child, "value")?;
                if v == "default" {
                    discriminators.push(UnionDiscriminator::Default);
                } else {
                    discriminators.push(UnionDiscriminator::Value(v.to_string()));
                }
            }
            "member" => {
                member = Some(parse_member(child)?);
            }
            _ => {}
        }
    }
    let member = member.ok_or_else(|| XmlError::MissingRequiredElement("member".to_string()))?;
    Ok(UnionCase {
        discriminators,
        member,
    })
}

fn parse_typedef(el: &XmlElement) -> Result<TypedefType, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let type_str = require_attr(el, "type")?;
    let type_ref = parse_type_ref(type_str);
    let array_dimensions = el
        .attribute("arrayDimensions")
        .map(parse_dimensions)
        .transpose()?
        .unwrap_or_default();
    let sequence_max_length = el
        .attribute("sequenceMaxLength")
        .map(parse_ulong)
        .transpose()?;
    let string_max_length = el
        .attribute("stringMaxLength")
        .map(parse_ulong)
        .transpose()?;
    Ok(TypedefType {
        name,
        type_ref,
        array_dimensions,
        sequence_max_length,
        string_max_length,
    })
}

fn parse_bitmask(el: &XmlElement) -> Result<BitmaskType, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let bit_bound = el.attribute("bitBound").map(parse_ulong).transpose()?;
    let mut bit_values = Vec::new();
    for child in el.children_named("bit_value") {
        let n = require_attr(child, "name")?.to_string();
        let position = child.attribute("position").map(parse_ulong).transpose()?;
        bit_values.push(BitValue { name: n, position });
    }
    Ok(BitmaskType {
        name,
        bit_bound,
        bit_values,
    })
}

fn parse_bitset(el: &XmlElement) -> Result<BitsetType, XmlError> {
    let name = require_attr(el, "name")?.to_string();
    let mut bit_fields = Vec::new();
    for child in el.children_named("bitfield") {
        let n = require_attr(child, "name")?.to_string();
        let type_str = require_attr(child, "type")?;
        let mask = require_attr(child, "mask")?.to_string();
        bit_fields.push(BitField {
            name: n,
            type_ref: parse_type_ref(type_str),
            mask,
        });
    }
    Ok(BitsetType { name, bit_fields })
}

fn require_attr<'a>(el: &'a XmlElement, key: &str) -> Result<&'a str, XmlError> {
    el.attribute(key)
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("@{key} on <{}>", el.name)))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_struct() {
        let xml = r#"<types>
            <struct name="State">
                <member name="id" type="long" key="true"/>
            </struct>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs.len(), 1);
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!("expected struct");
        };
        assert_eq!(s.name, "State");
        assert_eq!(s.members.len(), 1);
        assert!(s.members[0].key);
    }

    #[test]
    fn parse_module_nested() {
        let xml = r#"<types>
            <module name="Outer">
                <module name="Inner">
                    <struct name="X"/>
                </module>
            </module>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::Module(m) = &libs[0].types[0] else {
            panic!()
        };
        assert_eq!(m.name, "Outer");
        let TypeDef::Module(inner) = &m.types[0] else {
            panic!()
        };
        assert_eq!(inner.name, "Inner");
    }

    #[test]
    fn dds_root_with_multiple_types_blocks() {
        let xml = r#"<dds>
            <types><struct name="A"/></types>
            <types><struct name="B"/></types>
        </dds>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs.len(), 2);
    }

    #[test]
    fn unknown_root_rejected() {
        let xml = r#"<other/>"#;
        let err = parse_type_libraries(xml).expect_err("err");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    // ---- §7.3.2 XML type repr extended constructs ----

    #[test]
    fn parse_include_element() {
        let xml = r#"<types>
            <include file="shared/common.xml"/>
            <struct name="Local"/>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs[0].types.len(), 2);
        let TypeDef::Include(inc) = &libs[0].types[0] else {
            panic!("expected Include");
        };
        assert_eq!(inc.file, "shared/common.xml");
    }

    #[test]
    fn parse_forward_dcl_element() {
        let xml = r#"<types>
            <forward_dcl name="Node" kind="STRUCT"/>
            <struct name="Node">
                <member name="payload" type="long"/>
            </struct>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::ForwardDcl(fwd) = &libs[0].types[0] else {
            panic!("expected ForwardDcl");
        };
        assert_eq!(fwd.name, "Node");
        assert_eq!(fwd.kind, "STRUCT");
    }

    #[test]
    fn parse_forward_dcl_default_kind_struct() {
        let xml = r#"<types>
            <forward_dcl name="X"/>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        let TypeDef::ForwardDcl(fwd) = &libs[0].types[0] else {
            panic!("expected ForwardDcl");
        };
        assert_eq!(fwd.kind, "STRUCT");
    }

    #[test]
    fn parse_const_element() {
        let xml = r#"<types>
            <const name="MaxSize" type="long" value="1024"/>
            <const name="Greeting" type="string" value="hello"/>
        </types>"#;
        let libs = parse_type_libraries(xml).expect("parse");
        assert_eq!(libs[0].types.len(), 2);
        let TypeDef::Const(c1) = &libs[0].types[0] else {
            panic!("expected Const");
        };
        assert_eq!(c1.name, "MaxSize");
        assert_eq!(c1.type_name, "long");
        assert_eq!(c1.value, "1024");
        let TypeDef::Const(c2) = &libs[0].types[1] else {
            panic!("expected Const");
        };
        assert_eq!(c2.value, "hello");
    }

    #[test]
    fn parse_const_missing_value_attribute_rejected() {
        let xml = r#"<types>
            <const name="X" type="long"/>
        </types>"#;
        let res = parse_type_libraries(xml);
        assert!(res.is_err());
    }

    #[test]
    fn parse_include_missing_file_attribute_rejected() {
        let xml = r#"<types>
            <include/>
        </types>"#;
        let res = parse_type_libraries(xml);
        assert!(res.is_err());
    }
}
