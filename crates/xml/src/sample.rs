// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.7 Building Block "Data Samples" — XML-Codec.
//!
//! Repraesentiert konkrete Sample-Werte einzelner registrierter Types als
//! XML. Ein Sample ist Member-Wert-Map: Element-Namen entsprechen
//! Member-Namen, Children rekursiv kodierte Werte. Sequenzen/Arrays
//! verwenden `<item>` als Element-Name (Spec §7.3.7.4.4).
//!
//! # XML → Rust-Type Mapping
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

/// Wert eines konkreten Sample-Members (rekursiv).
#[derive(Debug, Clone, PartialEq)]
pub enum SampleValue {
    /// Primitiv-Wert.
    Primitive(PrimitiveValue),
    /// Struct-Member-Map (Member-Name -> Wert).
    Struct(BTreeMap<String, SampleValue>),
    /// Sequence-Werte.
    Sequence(Vec<SampleValue>),
    /// Array-Werte.
    Array(Vec<SampleValue>),
    /// Union mit aktivem Discriminator und ausgewaehltem Case-Wert.
    Union {
        /// String-Form des Discriminator-Wertes.
        discriminator: String,
        /// Aktiver Case-Wert.
        value: alloc::boxed::Box<SampleValue>,
    },
    /// Enum-Literal als Symbol.
    EnumLiteral(String),
}

/// Konkreter Primitiv-Wert (typed, mit Range-Check beim Parse).
#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveValue {
    /// `boolean`.
    Bool(bool),
    /// `char` als 8-bit signed (IDL-konform).
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
    /// `double` / `longdouble` (longdouble fallt auf f64 zurueck).
    F64(f64),
    /// `string` / `wstring`.
    Str(String),
    /// 8-bit `char` als Unicode-Codepoint.
    Char(char),
}

/// Parst ein konkretes `<sample>`-Element gegen eine Type-Definition.
///
/// `xml` darf entweder ein `<sample>`-Wurzel-Element oder das Wurzel-
/// Element des Datentyps direkt sein (Spec §7.3.7.4 erlaubt beide
/// Repraesentationen).
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML nicht wohlgeformt.
/// * [`XmlError::UnresolvedReference`] — referenzierter Type nicht im
///   `type_lib` auffindbar.
/// * [`XmlError::ValueOutOfRange`] — Primitiv-Wert ausserhalb Range.
/// * [`XmlError::MissingRequiredElement`] — Pflicht-Member fehlt.
pub fn parse_sample(
    xml: &str,
    type_def: &TypeDef,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    let doc = parse_xml_tree(xml)?;
    parse_sample_element(&doc.root, type_def, type_lib)
}

/// Variante von [`parse_sample`], die bereits ein geparstes
/// [`XmlElement`] entgegen nimmt.
///
/// # Errors
/// Wie [`parse_sample`].
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
    // Sequence — wenn Member als sequence deklariert ist (max-length
    // gesetzt) ODER der Wrapper-Knoten ausschliesslich `<item>`-Children
    // enthaelt. Spec §7.3.7.3.2: leere Sequence (`<seq></seq>`) ist
    // valide → bei `sequence_max_length.is_some()` auch empty zulassen.
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

/// zerodds-lint: recursion-depth = anzahl `::`-Segmente + Modul-Schachtelungstiefe.
/// Effektiv ≤ 16 durch DoS-Cap der XML-Foundation.
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

/// Serialisiert einen Sample-Wert als XML.
///
/// Wrapped das Ergebnis in `<sample type_ref="…">`. Member-Namen
/// entsprechen den Schluesseln in `value`, Sequenzen/Arrays nutzen
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
/// nesting). DoS-Cap der XML-Foundation begrenzt Tiefe ≤ 16.
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

    // ---- §7.3.7.3.2 Sequence/Array mit <item>-Tag ---------------------

    #[test]
    fn parse_sample_with_sequence_using_item_tag() {
        // Spec §7.3.7.3.2: "complexType contains zero or more elements
        // named **item**". Hier explizit Sequence<long> mit max=10.
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
        // Spec §7.3.7.3.2: Arrays nutzen denselben <item>-Tag.
        // array_dimensions[3] mit element_type long.
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
        // serialize_sample emittiert <item> fuer Sequence — Spec §7.3.7.3.2.
        // Wir kapseln die Sequence in einen Top-Level-Struct, damit
        // serialize_sample einen Type-Wrapper bekommt.
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
        // §7.3.7.1 Sample-Format fuer Union — discriminator + active case.
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
