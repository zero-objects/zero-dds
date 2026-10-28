// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-WEB §8.1/§8.2 XML data representation, converted through the
//! type's own [`DynamicType`] descriptor — never an opaque byte guess.
//!
//! §8.1 makes `application/dds-web+xml` the Mandatory format;
//! nothing else is Required, so only XML is implemented here (see the
//! crate README's "Open points" for the optional-format gap).
//!
//! The wire tags this module writes/reads are the sample's own member
//! names, recursively — the same shape `zerodds_xml::sample`'s module
//! doc documents for DDS-XML 1.0's "Data Samples" building block
//! (`<member-name>value</member-name>`, `<item>` for sequence/array
//! elements, `<discriminator>`+the active case's tag for a union). That
//! crate's `SampleValue` is a different type model (`TypeLibrary`/
//! `TypeDef`, not `zerodds_types::dynamic`), so this module talks
//! directly to [`DynamicData`]/[`DynamicType`] instead of bridging the
//! two — see the crate README for why.

// `DynamicData::{integer,float,character}` take the AST's own
// `PrimitiveKind` (`zerodds_ast::PrimitiveKind`) — a distinctly-named
// enum from `zerodds_types::type_identifier::PrimitiveKind` (the
// TypeIdentifier-wire one), spelled `Octet`/`Char`/`WChar` instead of
// `Byte`/`Char8`/`Char16`.
use zerodds_ast::PrimitiveKind;
use zerodds_types::dynamic::descriptor::TypeKind;
use zerodds_types::dynamic::{DynamicData, DynamicType};
use zerodds_xml_wire::emitter::XmlEmitter;
use zerodds_xml_wire::parser::{Event, XmlParser};

/// Conversion error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynXmlError {
    /// The XML text did not parse, or did not have the expected shape
    /// (a member/element the type descriptor asked for was missing).
    Malformed(String),
    /// A primitive's text content did not parse as its kind.
    BadLiteral {
        /// The member/element path (dotted).
        at: String,
        /// The raw text that failed to parse.
        text: String,
    },
    /// A `TypeKind` this converter does not (yet) handle — currently
    /// `long double` (Float128; blocked pending Rust `f128`, same
    /// project-wide boundary as `idl-4.2`'s long-double bucket) and
    /// `map` (DDS-WEB's own XML samples in the wild are struct/
    /// sequence/enum/union; a map member is rare enough that adding
    /// support is deferred rather than guessed at).
    Unsupported(&'static str),
    /// The `DynamicType` descriptor itself refused to answer (a stale
    /// handle, or an inconsistent model).
    Model(String),
    /// The document or the value nests deeper than `MAX_DEPTH` — the
    /// element named, or the path of the value.
    TooDeep(String),
}

/// Nesting bound of the XML body and of the value walked through it. A
/// body arrives over HTTP; its element tree is dropped recursively and a
/// recursive type (a struct with a sequence of itself) follows it down.
const MAX_DEPTH: usize = 64;

impl std::fmt::Display for DynXmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(m) => write!(f, "malformed XML body: {m}"),
            Self::BadLiteral { at, text } => write!(f, "bad literal at `{at}`: `{text}`"),
            Self::Unsupported(k) => write!(f, "XML conversion not implemented for {k}"),
            Self::Model(m) => write!(f, "type model: {m}"),
            Self::TooDeep(at) => write!(f, "`{at}`: nesting exceeds {MAX_DEPTH} levels"),
        }
    }
}

impl std::error::Error for DynXmlError {}

/// A parsed element tree — the minimal DOM this module needs, built
/// from `zerodds_xml_wire::parser`'s event stream.
#[derive(Debug, Clone, Default)]
struct XmlNode {
    name: String,
    children: Vec<XmlNode>,
    text: String,
}

/// Parses `xml` and returns its single root element (skipping a leading
/// `<?xml ...?>` declaration).
///
/// # Errors
/// [`DynXmlError::Malformed`] on invalid XML or a document with no root
/// element.
fn parse_root(xml: &str) -> Result<XmlNode, DynXmlError> {
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    for event in XmlParser::new(xml) {
        let event = event.map_err(|e| DynXmlError::Malformed(e.to_string()))?;
        match event {
            Event::Declaration(_) | Event::CData(_) => {}
            Event::StartElement { name, .. } => {
                // The root plus one element per value level.
                if stack.len() > MAX_DEPTH {
                    return Err(DynXmlError::TooDeep(name));
                }
                stack.push(XmlNode {
                    name,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            Event::Text(text) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(text.trim());
                }
            }
            Event::EndElement(name) => {
                let node = stack.pop().ok_or_else(|| {
                    DynXmlError::Malformed(format!("unmatched close tag `{name}`"))
                })?;
                if node.name != name {
                    return Err(DynXmlError::Malformed(format!(
                        "close tag `{name}` does not match open tag `{}`",
                        node.name
                    )));
                }
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
        }
    }
    root.ok_or_else(|| DynXmlError::Malformed("no root element".into()))
}

/// Encodes `value` (of type `ty`) as the XML element `tag`, appended to
/// `emitter`.
///
/// # Errors
/// See [`DynXmlError`].
pub fn encode_value(
    emitter: &mut XmlEmitter,
    tag: &str,
    value: &DynamicData,
    ty: &DynamicType,
) -> Result<(), DynXmlError> {
    encode_at(emitter, tag, value, ty, 0)
}

/// zerodds-lint: recursion-depth 64 (one level per nested member, case,
/// element or alias; refused past `MAX_DEPTH`)
fn encode_at(
    emitter: &mut XmlEmitter,
    tag: &str,
    value: &DynamicData,
    ty: &DynamicType,
    depth: usize,
) -> Result<(), DynXmlError> {
    if depth > MAX_DEPTH {
        return Err(DynXmlError::TooDeep(tag.to_string()));
    }
    let kind = ty.kind();
    match kind {
        TypeKind::Alias => {
            let target = ty
                .resolve_alias()
                .map_err(|e| DynXmlError::Model(e.to_string()))?;
            return encode_at(emitter, tag, value, &target, depth + 1);
        }
        TypeKind::Structure => {
            emitter
                .start_element(tag, &[])
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
            for member in ty.members() {
                let member_ty = member
                    .dynamic_type()
                    .map_err(|e| DynXmlError::Model(e.to_string()))?;
                let member_value = value.member(member.name()).ok_or_else(|| {
                    DynXmlError::Malformed(format!("missing member `{}`", member.name()))
                })?;
                encode_at(emitter, member.name(), member_value, &member_ty, depth + 1)?;
            }
            emitter
                .end_element()
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
        }
        TypeKind::Union => {
            let disc_ty = ty
                .discriminator_type()
                .ok_or_else(|| DynXmlError::Model("union with no discriminator".into()))?;
            let (disc_entry, active) = value
                .members()
                .and_then(|m| m.first())
                .zip(value.members().and_then(|m| m.get(1)))
                .ok_or_else(|| DynXmlError::Malformed("union value has no case".into()))?;
            emitter
                .start_element(tag, &[])
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
            encode_at(emitter, "discriminator", &disc_entry.1, &disc_ty, depth + 1)?;
            let case_name = &active.0;
            let case_member = ty.member_by_name(case_name).ok_or_else(|| {
                DynXmlError::Malformed(format!("union has no case `{case_name}`"))
            })?;
            let case_ty = case_member
                .dynamic_type()
                .map_err(|e| DynXmlError::Model(e.to_string()))?;
            encode_at(emitter, case_name, &active.1, &case_ty, depth + 1)?;
            emitter
                .end_element()
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
        }
        TypeKind::Sequence | TypeKind::Array => {
            let element_ty = ty
                .element_type()
                .ok_or_else(|| DynXmlError::Model("collection with no element type".into()))?;
            emitter
                .start_element(tag, &[])
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
            for item in value.items().unwrap_or(&[]) {
                encode_at(emitter, "item", item, &element_ty, depth + 1)?;
            }
            emitter
                .end_element()
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
        }
        TypeKind::Map => return Err(DynXmlError::Unsupported("map")),
        TypeKind::Float128 => return Err(DynXmlError::Unsupported("long double")),
        TypeKind::Bitmask | TypeKind::Bitset => {
            emitter
                .start_element(tag, &[])
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
            for item in value.items().into_iter().flatten() {
                if let DynamicData::Text(name) = item {
                    emitter
                        .start_element("item", &[])
                        .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
                    emitter.text(name);
                    emitter
                        .end_element()
                        .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
                }
            }
            for (name, bit_value) in value.members().into_iter().flatten() {
                if matches!(bit_value, DynamicData::Boolean(true)) {
                    emitter
                        .start_element("item", &[])
                        .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
                    emitter.text(name);
                    emitter
                        .end_element()
                        .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
                }
            }
            emitter
                .end_element()
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
        }
        _ => {
            let text = primitive_to_text(value)?;
            emitter
                .start_element(tag, &[])
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
            emitter.text(&text);
            emitter
                .end_element()
                .map_err(|e| DynXmlError::Malformed(e.to_string()))?;
        }
    }
    Ok(())
}

/// Top-level: `value` as a complete XML document (`<?xml?>` + the
/// element named `root_tag`).
///
/// # Errors
/// See [`DynXmlError`].
pub fn to_xml_document(
    root_tag: &str,
    value: &DynamicData,
    ty: &DynamicType,
) -> Result<String, DynXmlError> {
    let mut emitter = XmlEmitter::new();
    emitter.declaration();
    encode_value(&mut emitter, root_tag, value, ty)?;
    Ok(emitter.finish())
}

/// Parses `xml` as a value of type `ty` (the root element's tag is not
/// checked against a name — the caller already knows what it asked
/// for).
///
/// # Errors
/// See [`DynXmlError`].
pub fn from_xml_document(xml: &str, ty: &DynamicType) -> Result<DynamicData, DynXmlError> {
    let root = parse_root(xml)?;
    decode_value(&root, ty, &root.name, 0)
}

/// zerodds-lint: recursion-depth 64 (one level per nested member, case,
/// element or alias; refused past `MAX_DEPTH`, and `parse_root` refuses a
/// deeper document before)
fn decode_value(
    node: &XmlNode,
    ty: &DynamicType,
    path: &str,
    depth: usize,
) -> Result<DynamicData, DynXmlError> {
    if depth > MAX_DEPTH {
        return Err(DynXmlError::TooDeep(path.to_string()));
    }
    let kind = ty.kind();
    match kind {
        TypeKind::Alias => {
            let target = ty
                .resolve_alias()
                .map_err(|e| DynXmlError::Model(e.to_string()))?;
            decode_value(node, &target, path, depth + 1)
        }
        TypeKind::Structure => {
            let mut entries = Vec::new();
            for member in ty.members() {
                let member_ty = member
                    .dynamic_type()
                    .map_err(|e| DynXmlError::Model(e.to_string()))?;
                let child = node
                    .children
                    .iter()
                    .find(|c| c.name == member.name())
                    .ok_or_else(|| {
                        DynXmlError::Malformed(format!(
                            "{path}: missing member `{}`",
                            member.name()
                        ))
                    })?;
                let value = decode_value(
                    child,
                    &member_ty,
                    &format!("{path}.{}", member.name()),
                    depth + 1,
                )?;
                entries.push((member.name().to_string(), value));
            }
            Ok(DynamicData::Aggregate(entries))
        }
        TypeKind::Union => {
            let disc_ty = ty
                .discriminator_type()
                .ok_or_else(|| DynXmlError::Model("union with no discriminator".into()))?;
            let disc_node = node
                .children
                .iter()
                .find(|c| c.name == "discriminator")
                .ok_or_else(|| DynXmlError::Malformed(format!("{path}: missing discriminator")))?;
            let disc_value = decode_value(
                disc_node,
                &disc_ty,
                &format!("{path}.discriminator"),
                depth + 1,
            )?;
            let case_node = node
                .children
                .iter()
                .find(|c| c.name != "discriminator")
                .ok_or_else(|| DynXmlError::Malformed(format!("{path}: missing active case")))?;
            let case_member = ty.member_by_name(&case_node.name).ok_or_else(|| {
                DynXmlError::Malformed(format!("{path}: unknown case `{}`", case_node.name))
            })?;
            let case_ty = case_member
                .dynamic_type()
                .map_err(|e| DynXmlError::Model(e.to_string()))?;
            let case_value = decode_value(
                case_node,
                &case_ty,
                &format!("{path}.{}", case_node.name),
                depth + 1,
            )?;
            Ok(DynamicData::Aggregate(vec![
                ("discriminator".to_string(), disc_value),
                (case_node.name.clone(), case_value),
            ]))
        }
        TypeKind::Sequence | TypeKind::Array => {
            let element_ty = ty
                .element_type()
                .ok_or_else(|| DynXmlError::Model("collection with no element type".into()))?;
            let mut items = Vec::with_capacity(node.children.len());
            for (i, child) in node.children.iter().enumerate() {
                items.push(decode_value(
                    child,
                    &element_ty,
                    &format!("{path}[{i}]"),
                    depth + 1,
                )?);
            }
            Ok(DynamicData::Collection(items))
        }
        TypeKind::Map => Err(DynXmlError::Unsupported("map")),
        TypeKind::Float128 => Err(DynXmlError::Unsupported("long double")),
        TypeKind::Bitmask => {
            let bits = node
                .children
                .iter()
                .map(|c| DynamicData::Text(c.text.clone()))
                .collect();
            Ok(DynamicData::Bits(bits))
        }
        _ => text_to_primitive(kind, &node.text, path),
    }
}

/// Text content of the wire tags the wire codec understands for a
/// primitive `DynamicData`.
fn primitive_to_text(value: &DynamicData) -> Result<String, DynXmlError> {
    Ok(match value {
        DynamicData::Boolean(b) => b.to_string(),
        DynamicData::Integer { value, .. } => value.to_string(),
        DynamicData::Float { value, .. } => value.to_string(),
        DynamicData::Character { value, .. } => value.clone(),
        DynamicData::Text(s) => s.clone(),
        DynamicData::Enumerator(name) => name.clone(),
        DynamicData::Fixed(s) => s.clone(),
        DynamicData::LongDouble(_) => return Err(DynXmlError::Unsupported("long double")),
        _ => {
            return Err(DynXmlError::Malformed(
                "value is not a primitive/enum".into(),
            ));
        }
    })
}

/// The [`PrimitiveKind`] a [`TypeKind`] maps to, for every kind that is
/// a primitive (not string/enum/collection/aggregate).
const fn primitive_kind_of(kind: TypeKind) -> Option<PrimitiveKind> {
    Some(match kind {
        TypeKind::Boolean => PrimitiveKind::Boolean,
        TypeKind::Byte => PrimitiveKind::Octet,
        TypeKind::Int8 => PrimitiveKind::Int8,
        TypeKind::UInt8 => PrimitiveKind::UInt8,
        TypeKind::Int16 => PrimitiveKind::Int16,
        TypeKind::UInt16 => PrimitiveKind::UInt16,
        TypeKind::Int32 => PrimitiveKind::Int32,
        TypeKind::UInt32 => PrimitiveKind::UInt32,
        TypeKind::Int64 => PrimitiveKind::Int64,
        TypeKind::UInt64 => PrimitiveKind::UInt64,
        TypeKind::Float32 => PrimitiveKind::Float32,
        TypeKind::Float64 => PrimitiveKind::Float64,
        TypeKind::Char8 => PrimitiveKind::Char,
        TypeKind::Char16 => PrimitiveKind::WChar,
        _ => return None,
    })
}

fn text_to_primitive(kind: TypeKind, text: &str, path: &str) -> Result<DynamicData, DynXmlError> {
    let bad = || DynXmlError::BadLiteral {
        at: path.to_string(),
        text: text.to_string(),
    };
    match kind {
        TypeKind::Boolean => Ok(DynamicData::Boolean(match text {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err(bad()),
        })),
        TypeKind::String8 | TypeKind::String16 => Ok(DynamicData::Text(text.to_string())),
        TypeKind::Enumeration => Ok(DynamicData::Enumerator(text.to_string())),
        TypeKind::Char8 | TypeKind::Char16 => {
            let ch = text.chars().next().ok_or_else(bad)?;
            let pk = primitive_kind_of(kind).ok_or_else(bad)?;
            Ok(DynamicData::character(pk, ch))
        }
        TypeKind::Float32 | TypeKind::Float64 => {
            let v: f64 = text.trim().parse().map_err(|_| bad())?;
            let pk = primitive_kind_of(kind).ok_or_else(bad)?;
            Ok(DynamicData::float(pk, v))
        }
        _ => {
            let pk = primitive_kind_of(kind).ok_or(DynXmlError::Unsupported("this TypeKind"))?;
            let v: i128 = text.trim().parse().map_err(|_| bad())?;
            Ok(DynamicData::integer(pk, v))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::types_registry::TypeRegistry;

    fn resolved(idl: &str, type_name: &str) -> crate::types_registry::ResolvedType {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "zd-dynxml-{}-{n}-{}",
            std::process::id(),
            type_name.replace("::", "_")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.idl");
        std::fs::write(&path, idl).unwrap();
        let reg = TypeRegistry::new();
        let r = reg.load_idl_file(&path, type_name).expect("resolved");
        let _ = std::fs::remove_dir_all(&dir);
        r
    }

    #[test]
    fn struct_roundtrip_through_xml() {
        let r = resolved(
            "module M { @final struct P { @key long id; string name; boolean active; }; };",
            "M::P",
        );
        let value = DynamicData::Aggregate(vec![
            (
                "id".to_string(),
                DynamicData::integer(PrimitiveKind::Int32, 7),
            ),
            ("name".to_string(), DynamicData::text("alice".to_string())),
            ("active".to_string(), DynamicData::Boolean(true)),
        ]);
        let xml = to_xml_document("data", &value, &r.dynamic_type).expect("encode");
        assert!(xml.contains("<id>7</id>"));
        assert!(xml.contains("<name>alice</name>"));
        assert!(xml.contains("<active>true</active>"));
        let back = from_xml_document(&xml, &r.dynamic_type).expect("decode");
        assert_eq!(
            back.member("id"),
            Some(&DynamicData::integer(PrimitiveKind::Int32, 7))
        );
        assert_eq!(
            back.member("name"),
            Some(&DynamicData::text("alice".to_string()))
        );
        assert_eq!(back.member("active"), Some(&DynamicData::Boolean(true)));
    }

    #[test]
    fn sequence_of_structs_roundtrip() {
        let r = resolved(
            "module M { @final struct Item { long v; }; @final struct Bag { sequence<Item> items; }; };",
            "M::Bag",
        );
        let value = DynamicData::Aggregate(vec![(
            "items".to_string(),
            DynamicData::Collection(vec![
                DynamicData::Aggregate(vec![(
                    "v".to_string(),
                    DynamicData::integer(PrimitiveKind::Int32, 1),
                )]),
                DynamicData::Aggregate(vec![(
                    "v".to_string(),
                    DynamicData::integer(PrimitiveKind::Int32, 2),
                )]),
            ]),
        )]);
        let xml = to_xml_document("data", &value, &r.dynamic_type).expect("encode");
        assert!(xml.contains("<item>"));
        let back = from_xml_document(&xml, &r.dynamic_type).expect("decode");
        let items = back.member("items").unwrap().items().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn a_body_nested_past_the_bound_is_refused_before_it_is_walked() {
        let r = resolved("module M { @final struct P { long x; }; };", "M::P");
        let levels = MAX_DEPTH + 2;
        let xml = format!(
            "<data>{}{}</data>",
            "<x>".repeat(levels),
            "</x>".repeat(levels)
        );
        let err = from_xml_document(&xml, &r.dynamic_type).unwrap_err();
        assert!(matches!(err, DynXmlError::TooDeep(_)), "{err:?}");
    }

    #[test]
    fn missing_member_is_reported() {
        let r = resolved("module M { @final struct P { long x; long y; }; };", "M::P");
        let value = DynamicData::Aggregate(vec![(
            "x".to_string(),
            DynamicData::integer(PrimitiveKind::Int32, 1),
        )]);
        let err = to_xml_document("data", &value, &r.dynamic_type).unwrap_err();
        assert!(matches!(err, DynXmlError::Malformed(_)));
    }

    #[test]
    fn bad_literal_is_reported_not_silently_defaulted() {
        let r = resolved("module M { @final struct P { long x; }; };", "M::P");
        let xml = "<data><x>not-a-number</x></data>";
        let err = from_xml_document(xml, &r.dynamic_type).unwrap_err();
        assert!(matches!(err, DynXmlError::BadLiteral { .. }));
    }

    #[test]
    fn enum_roundtrip() {
        let r = resolved(
            "module M { enum Color { RED, GREEN, BLUE }; @final struct P { Color c; }; };",
            "M::P",
        );
        let value = DynamicData::Aggregate(vec![(
            "c".to_string(),
            DynamicData::Enumerator("GREEN".to_string()),
        )]);
        let xml = to_xml_document("data", &value, &r.dynamic_type).expect("encode");
        assert!(xml.contains("<c>GREEN</c>"));
        let back = from_xml_document(&xml, &r.dynamic_type).expect("decode");
        assert_eq!(
            back.member("c"),
            Some(&DynamicData::Enumerator("GREEN".to_string()))
        );
    }
}
