// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.7 building block "Data Samples" — XML codec.
//!
//! Represents concrete sample values of individual registered types as
//! XML. A sample is a member-value map: element names correspond to
//! member names, children are recursively encoded values. Sequences/arrays
//! use `<item>` as the element name (Spec §7.3.7.4.4).
//!
//! # XML → Rust type mapping
//!
//! ```text
//! <sample type_ref="Mod::Type"> … </sample>  | SampleValue::Struct
//! <member-name>123</member-name>             | SampleValue::Primitive
//! <seq-name><item>…</item>…</seq-name>       | SampleValue::Sequence
//! <arr-name><item>…</item>…</arr-name>       | SampleValue::Array
//! <union-name><discriminator>…</discriminator>
//!             <case-name>…</case-name></union-name>
//!                                            | SampleValue::Union
//! ```

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::parser::{XmlElement, parse_xml_tree};
use crate::xtypes_def::{
    PrimitiveType, StructMember, StructType, TypeDef, TypeLibrary, TypeRef, UnionDiscriminator,
};

/// Value of a concrete sample member (recursive).
#[derive(Debug, Clone, PartialEq)]
pub enum SampleValue {
    /// Primitive value.
    Primitive(PrimitiveValue),
    /// Struct member map (member name -> value).
    Struct(BTreeMap<String, SampleValue>),
    /// Sequence values.
    Sequence(Vec<SampleValue>),
    /// Array values.
    Array(Vec<SampleValue>),
    /// Union with an active discriminator and the selected case value.
    Union {
        /// String form of the discriminator value.
        discriminator: String,
        /// Active case value.
        value: alloc::boxed::Box<SampleValue>,
    },
    /// Enum literal as a symbol.
    EnumLiteral(String),
}

/// Concrete primitive value (typed, with a range check on parse).
#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveValue {
    /// `boolean`.
    Bool(bool),
    /// `char` as 8-bit signed (IDL-conformant).
    I8(i8),
    /// `octet`.
    U8(u8),
    /// `short`.
    I16(i16),
    /// `ushort` / `wchar`.
    U16(u16),
    /// `long`.
    I32(i32),
    /// `ulong`.
    U32(u32),
    /// `longlong`.
    I64(i64),
    /// `ulonglong`.
    U64(u64),
    /// `float`.
    F32(f32),
    /// `double` / `longdouble` (longdouble falls back to f64).
    F64(f64),
    /// `string` / `wstring`.
    Str(String),
    /// 8-bit `char` as a Unicode code point.
    Char(char),
}

/// Parses a concrete `<sample>` element against a type definition.
///
/// `xml` may be either a `<sample>` root element or the root
/// element of the data type directly (Spec §7.3.7.4 allows both
/// representations).
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML not well-formed.
/// * [`XmlError::UnresolvedReference`] — referenced type not found in
///   `type_lib`.
/// * [`XmlError::ValueOutOfRange`] — primitive value out of range.
/// * [`XmlError::MissingRequiredElement`] — a required member is missing.
pub fn parse_sample(
    xml: &str,
    type_def: &TypeDef,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    let doc = parse_xml_tree(xml)?;
    parse_sample_element(&doc.root, type_def, type_lib)
}

/// Variant of [`parse_sample`] that takes an already-parsed
/// [`XmlElement`].
///
/// # Errors
/// As [`parse_sample`].
pub fn parse_sample_element(
    el: &XmlElement,
    type_def: &TypeDef,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    parse_value_for_typedef(el, type_def, type_lib)
}

fn parse_value_for_typedef(
    el: &XmlElement,
    type_def: &TypeDef,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    match type_def {
        TypeDef::Struct(s) => parse_struct_value(el, s, type_lib),
        TypeDef::Enum(_) => Ok(SampleValue::EnumLiteral(el.text.clone())),
        TypeDef::Union(u) => parse_union_value(el, u, type_lib),
        TypeDef::Typedef(td) => {
            let synthetic = StructMember {
                name: td.name.clone(),
                type_ref: td.type_ref.clone(),
                array_dimensions: td.array_dimensions.clone(),
                sequence_max_length: td.sequence_max_length,
                string_max_length: td.string_max_length,
                ..Default::default()
            };
            parse_member_value(el, &synthetic, type_lib)
        }
        TypeDef::Bitmask(_) => Ok(SampleValue::Primitive(PrimitiveValue::Str(el.text.clone()))),
        TypeDef::Bitset(_) => Ok(SampleValue::Primitive(PrimitiveValue::Str(el.text.clone()))),
        TypeDef::Module(m) => Err(XmlError::UnresolvedReference(format!(
            "module `{}` is not directly serializable",
            m.name
        ))),
        TypeDef::Include(i) => Err(XmlError::UnresolvedReference(format!(
            "include `{}` cannot be serialized as sample value",
            i.file
        ))),
        TypeDef::ForwardDcl(f) => Err(XmlError::UnresolvedReference(format!(
            "forward declaration `{}` cannot be serialized — body missing",
            f.name
        ))),
        TypeDef::Const(c) => Ok(SampleValue::Primitive(PrimitiveValue::Str(c.value.clone()))),
    }
}

fn parse_struct_value(
    el: &XmlElement,
    s: &StructType,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    let mut map = BTreeMap::new();
    for member in &s.members {
        if let Some(child) = el.child(&member.name) {
            let v = parse_member_value(child, member, type_lib)?;
            map.insert(member.name.clone(), v);
        } else if !member.optional {
            return Err(XmlError::MissingRequiredElement(member.name.clone()));
        }
    }
    Ok(SampleValue::Struct(map))
}

fn parse_union_value(
    el: &XmlElement,
    u: &crate::xtypes_def::UnionType,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    let disc_el = el
        .child("discriminator")
        .ok_or_else(|| XmlError::MissingRequiredElement("discriminator".into()))?;
    let disc = disc_el.text.clone();

    // Active case lookup
    let case = u
        .cases
        .iter()
        .find(|c| {
            c.discriminators.iter().any(|d| match d {
                UnionDiscriminator::Value(v) => v == &disc,
                UnionDiscriminator::Default => false,
            })
        })
        .or_else(|| {
            u.cases.iter().find(|c| {
                c.discriminators
                    .iter()
                    .any(|d| matches!(d, UnionDiscriminator::Default))
            })
        })
        .ok_or_else(|| {
            XmlError::UnresolvedReference(format!("union case for discriminator `{disc}`"))
        })?;
    let val_el = el.child(&case.member.name).ok_or_else(|| {
        XmlError::MissingRequiredElement(format!("union member `{}`", case.member.name))
    })?;
    let value = parse_member_value(val_el, &case.member, type_lib)?;
    Ok(SampleValue::Union {
        discriminator: disc,
        value: alloc::boxed::Box::new(value),
    })
}

fn parse_member_value(
    el: &XmlElement,
    member: &StructMember,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    // Array (multi-dim collapsed to flat sequence of items at top level).
    if !member.array_dimensions.is_empty() {
        let mut items = Vec::new();
        for child in el.children_named("item") {
            let inner = parse_inner_value(child, &member.type_ref, type_lib)?;
            items.push(inner);
        }
        return Ok(SampleValue::Array(items));
    }
    // Sequence — when the member is declared as a sequence (max-length
    // set) OR the wrapper node contains exclusively `<item>` children.
    // Spec §7.3.7.3.2: an empty sequence (`<seq></seq>`) is
    // valid → with `sequence_max_length.is_some()` also allow empty.
    let is_seq = member.sequence_max_length.is_some() || has_only_item_children(el);
    if is_seq
        && (has_item_children(el)
            || (member.sequence_max_length.is_some() && el.children.is_empty()))
    {
        let mut items = Vec::new();
        for child in el.children_named("item") {
            let inner = parse_inner_value(child, &member.type_ref, type_lib)?;
            items.push(inner);
        }
        return Ok(SampleValue::Sequence(items));
    }
    parse_inner_value(el, &member.type_ref, type_lib)
}

fn has_item_children(el: &XmlElement) -> bool {
    el.children.iter().any(|c| c.name == "item")
}

fn has_only_item_children(el: &XmlElement) -> bool {
    !el.children.is_empty() && el.children.iter().all(|c| c.name == "item")
}

fn parse_inner_value(
    el: &XmlElement,
    type_ref: &TypeRef,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    match type_ref {
        TypeRef::Primitive(p) => parse_primitive_value(&el.text, *p).map(SampleValue::Primitive),
        TypeRef::Named(name) => {
            let td = resolve_named_type(name, type_lib).ok_or_else(|| {
                XmlError::UnresolvedReference(format!("type `{name}` not in libraries"))
            })?;
            parse_value_for_typedef(el, &td, type_lib)
        }
    }
}

fn resolve_named_type(name: &str, type_lib: &[TypeLibrary]) -> Option<TypeDef> {
    let parts: Vec<&str> = name.split("::").collect();
    for lib in type_lib {
        if let Some(td) = walk(&lib.types, &parts) {
            return Some(td);
        }
    }
    None
}

/// zerodds-lint: recursion-depth = number of `::` segments + module nesting depth.
/// Effectively ≤ 16 due to the XML foundation's DoS cap.
fn walk(types: &[TypeDef], parts: &[&str]) -> Option<TypeDef> {
    if parts.is_empty() {
        return None;
    }
    let head = parts[0];
    for t in types {
        if t.name() == head {
            if parts.len() == 1 {
                return Some(t.clone());
            }
            if let TypeDef::Module(m) = t {
                return walk(&m.types, &parts[1..]);
            }
        }
    }
    // Fallback: recursive search through modules at any depth (bare name).
    if parts.len() == 1 {
        for t in types {
            if let TypeDef::Module(m) = t {
                if let Some(found) = walk(&m.types, parts) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn parse_primitive_value(s: &str, p: PrimitiveType) -> Result<PrimitiveValue, XmlError> {
    let t = s.trim();
    match p {
        PrimitiveType::Boolean => Ok(PrimitiveValue::Bool(crate::types::parse_bool(t)?)),
        PrimitiveType::Octet => parse_uint::<u8>(t).map(PrimitiveValue::U8),
        PrimitiveType::Char => {
            let mut chars = t.chars();
            let c = chars
                .next()
                .ok_or_else(|| XmlError::ValueOutOfRange("empty char".into()))?;
            if chars.next().is_some() {
                return Err(XmlError::ValueOutOfRange(format!(
                    "char `{t}` is multi-char"
                )));
            }
            Ok(PrimitiveValue::Char(c))
        }
        PrimitiveType::WChar => {
            let mut chars = t.chars();
            let c = chars
                .next()
                .ok_or_else(|| XmlError::ValueOutOfRange("empty wchar".into()))?;
            Ok(PrimitiveValue::Char(c))
        }
        PrimitiveType::Short => parse_int::<i16>(t).map(PrimitiveValue::I16),
        PrimitiveType::UShort => parse_uint::<u16>(t).map(PrimitiveValue::U16),
        PrimitiveType::Long => parse_int::<i32>(t).map(PrimitiveValue::I32),
        PrimitiveType::ULong => parse_uint::<u32>(t).map(PrimitiveValue::U32),
        PrimitiveType::LongLong => parse_int::<i64>(t).map(PrimitiveValue::I64),
        PrimitiveType::ULongLong => parse_uint::<u64>(t).map(PrimitiveValue::U64),
        PrimitiveType::Float => t
            .parse::<f32>()
            .map(PrimitiveValue::F32)
            .map_err(|e| XmlError::ValueOutOfRange(format!("float `{t}`: {e}"))),
        PrimitiveType::Double | PrimitiveType::LongDouble => t
            .parse::<f64>()
            .map(PrimitiveValue::F64)
            .map_err(|e| XmlError::ValueOutOfRange(format!("double `{t}`: {e}"))),
        PrimitiveType::String | PrimitiveType::WString => Ok(PrimitiveValue::Str(s.to_string())),
    }
}

fn parse_int<T>(s: &str) -> Result<T, XmlError>
where
    T: core::str::FromStr,
    T::Err: core::fmt::Display,
{
    s.parse::<T>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("int `{s}`: {e}")))
}

fn parse_uint<T>(s: &str) -> Result<T, XmlError>
where
    T: core::str::FromStr,
    T::Err: core::fmt::Display,
{
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        // Parse as u64, then narrow.
        let v = u64::from_str_radix(hex, 16)
            .map_err(|e| XmlError::ValueOutOfRange(format!("hex `{s}`: {e}")))?;
        let s2 = alloc::format!("{v}");
        return s2
            .parse::<T>()
            .map_err(|e| XmlError::ValueOutOfRange(format!("uint `{s}`: {e}")));
    }
    s.parse::<T>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("uint `{s}`: {e}")))
}

/// Serializes a sample value as XML.
///
/// Wraps the result in `<sample type_ref="…">`. Member names
/// correspond to the keys in `value`, sequences/arrays use
/// `<item>`.
#[must_use]
pub fn serialize_sample(
    value: &SampleValue,
    type_def: &TypeDef,
    _type_lib: &[TypeLibrary],
) -> String {
    let mut out = String::new();
    out.push_str("<sample type_ref=\"");
    out.push_str(type_def.name());
    out.push_str("\">");
    serialize_value_body(value, &mut out);
    out.push_str("</sample>");
    out
}

/// zerodds-lint: recursion-depth = sample-tree-depth (struct/sequence/array
/// nesting). The XML foundation's DoS cap limits the depth to ≤ 16.
fn serialize_value_body(value: &SampleValue, out: &mut String) {
    match value {
        SampleValue::Primitive(p) => out.push_str(&serialize_primitive(p)),
        SampleValue::EnumLiteral(s) => out.push_str(&xml_escape(s)),
        SampleValue::Struct(map) => {
            for (k, v) in map {
                out.push('<');
                out.push_str(k);
                out.push('>');
                serialize_value_body(v, out);
                out.push_str("</");
                out.push_str(k);
                out.push('>');
            }
        }
        SampleValue::Sequence(items) | SampleValue::Array(items) => {
            for it in items {
                out.push_str("<item>");
                serialize_value_body(it, out);
                out.push_str("</item>");
            }
        }
        SampleValue::Union {
            discriminator,
            value,
        } => {
            out.push_str("<discriminator>");
            out.push_str(&xml_escape(discriminator));
            out.push_str("</discriminator>");
            // The active case member name is unknown here; we emit
            // a generic <value> wrapper.
            out.push_str("<value>");
            serialize_value_body(value, out);
            out.push_str("</value>");
        }
    }
}

fn serialize_primitive(p: &PrimitiveValue) -> String {
    match p {
        PrimitiveValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        PrimitiveValue::I8(v) => alloc::format!("{v}"),
        PrimitiveValue::U8(v) => alloc::format!("{v}"),
        PrimitiveValue::I16(v) => alloc::format!("{v}"),
        PrimitiveValue::U16(v) => alloc::format!("{v}"),
        PrimitiveValue::I32(v) => alloc::format!("{v}"),
        PrimitiveValue::U32(v) => alloc::format!("{v}"),
        PrimitiveValue::I64(v) => alloc::format!("{v}"),
        PrimitiveValue::U64(v) => alloc::format!("{v}"),
        PrimitiveValue::F32(v) => alloc::format!("{v}"),
        PrimitiveValue::F64(v) => alloc::format!("{v}"),
        PrimitiveValue::Str(s) => xml_escape(s),
        PrimitiveValue::Char(c) => xml_escape(&alloc::format!("{c}")),
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Spec §7.3.7.4.5: Non-ASCII via numeric entity.
            c if (c as u32) > 0x7F => {
                let code = c as u32;
                out.push_str(&alloc::format!("&#x{code:x};"));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::xtypes_def::{StructMember, StructType, TypeRef};

    #[test]
    fn parse_simple_struct_sample() {
        let td = TypeDef::Struct(StructType {
            name: "X".into(),
            members: alloc::vec![StructMember {
                name: "id".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                ..Default::default()
            }],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="X"><id>42</id></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse");
        let SampleValue::Struct(m) = v else { panic!() };
        assert!(matches!(
            m.get("id"),
            Some(SampleValue::Primitive(PrimitiveValue::I32(42)))
        ));
    }

    #[test]
    fn missing_member_rejected() {
        let td = TypeDef::Struct(StructType {
            name: "X".into(),
            members: alloc::vec![StructMember {
                name: "id".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                ..Default::default()
            }],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="X"></sample>"#;
        let err = parse_sample(xml, &td, &[]).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    // ---- §7.3.7.3.2 Sequence/Array with <item> tag --------------------

    #[test]
    fn parse_sample_with_sequence_using_item_tag() {
        // Spec §7.3.7.3.2: "complexType contains zero or more elements
        // named **item**". Here explicitly a Sequence<long> with max=10.
        let td = TypeDef::Struct(StructType {
            name: "WithSeq".into(),
            members: alloc::vec![StructMember {
                name: "ids".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                sequence_max_length: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="WithSeq"><ids><item>1</item><item>2</item><item>3</item></ids></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse");
        let SampleValue::Struct(m) = v else { panic!() };
        let ids = m.get("ids").expect("ids");
        let SampleValue::Sequence(items) = ids else {
            panic!("expected Sequence, got {ids:?}")
        };
        assert_eq!(items.len(), 3);
        assert!(matches!(
            items[0],
            SampleValue::Primitive(PrimitiveValue::I32(1))
        ));
        assert!(matches!(
            items[2],
            SampleValue::Primitive(PrimitiveValue::I32(3))
        ));
    }

    #[test]
    fn parse_sample_with_array_using_item_tag() {
        // Spec §7.3.7.3.2: arrays use the same <item> tag.
        // array_dimensions[3] with element_type long.
        let td = TypeDef::Struct(StructType {
            name: "WithArr".into(),
            members: alloc::vec![StructMember {
                name: "coords".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                array_dimensions: alloc::vec![3],
                ..Default::default()
            }],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="WithArr"><coords><item>10</item><item>20</item><item>30</item></coords></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse");
        let SampleValue::Struct(m) = v else { panic!() };
        let coords = m.get("coords").expect("coords");
        let SampleValue::Array(items) = coords else {
            panic!("expected Array, got {coords:?}")
        };
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn parse_sample_with_empty_sequence() {
        let td = TypeDef::Struct(StructType {
            name: "X".into(),
            members: alloc::vec![StructMember {
                name: "ids".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                sequence_max_length: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="X"><ids></ids></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse empty");
        let SampleValue::Struct(m) = v else { panic!() };
        let ids = m.get("ids").expect("ids");
        let SampleValue::Sequence(items) = ids else {
            panic!("expected Sequence")
        };
        assert!(items.is_empty());
    }

    #[test]
    fn serialize_sample_uses_item_tag_for_sequence() {
        // serialize_sample emits <item> for a sequence — Spec §7.3.7.3.2.
        // We wrap the sequence in a top-level struct so that
        // serialize_sample gets a type wrapper.
        let value = SampleValue::Struct(BTreeMap::from([(
            "ids".to_string(),
            SampleValue::Sequence(alloc::vec![
                SampleValue::Primitive(PrimitiveValue::I32(7)),
                SampleValue::Primitive(PrimitiveValue::I32(8)),
            ]),
        )]));
        let td = TypeDef::Struct(StructType {
            name: "Wrap".into(),
            members: alloc::vec![StructMember {
                name: "ids".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                sequence_max_length: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        });
        let out = serialize_sample(&value, &td, &[]);
        assert!(out.contains("<item>7</item>"));
        assert!(out.contains("<item>8</item>"));
    }

    #[test]
    fn parse_sample_with_union() {
        // §7.3.7.1 sample format for a union — discriminator + active case.
        use crate::xtypes_def::{UnionCase, UnionType};
        let td = TypeDef::Union(UnionType {
            name: "U".into(),
            discriminator: TypeRef::Primitive(PrimitiveType::Long),
            cases: alloc::vec![UnionCase {
                discriminators: alloc::vec![UnionDiscriminator::Value("1".into())],
                member: StructMember {
                    name: "a".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Long),
                    ..Default::default()
                },
            }],
        });
        let xml = r#"<sample type_ref="U"><discriminator>1</discriminator><a>42</a></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse union");
        let SampleValue::Union {
            discriminator,
            value,
        } = v
        else {
            panic!("expected Union")
        };
        assert_eq!(discriminator, "1");
        assert!(matches!(
            *value,
            SampleValue::Primitive(PrimitiveValue::I32(42))
        ));
    }
}
