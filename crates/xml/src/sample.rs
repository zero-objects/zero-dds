// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.7 building block "Data Samples" — XML codec.
//!
//! Represents concrete sample values of individual registered types as
//! XML. A sample is a member-value map: element names correspond to
//! member names, children are recursively encoded values. Sequences/arrays
//! use `<item>` as the element name (Spec §7.3.7.3.2).
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
        /// Name of the active case's member, which is the element that
        /// wraps the value in the XML (Spec §7.3.7.4.2 shows
        /// `<aUnion><aLong>0</aLong></aUnion>`).
        member: String,
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
/// element of the data type directly (the examples of §7.3.7.4 write
/// the type's name as the root).
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
        TypeDef::Enum(e) => {
            // §7.1.4 Tab.7.1: an enum value is a string and the legal
            // values are the ones defined in the IDL. A literal that the
            // enum does not declare is an error, not a symbol.
            let literal = el.text.trim();
            if e.enumerators.iter().any(|en| en.name == literal) {
                Ok(SampleValue::EnumLiteral(literal.to_string()))
            } else {
                Err(XmlError::BadEnum(literal.to_string()))
            }
        }
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
        TypeDef::Annotation(a) => Err(XmlError::UnresolvedReference(format!(
            "annotation `{}` is no type and cannot be serialized as sample value",
            a.name
        ))),
    }
}

/// Inheritance depth beyond which a `baseType` chain is taken for a cycle.
const MAX_BASE_DEPTH: usize = 32;

/// The members of `s` preceded by the members of its `baseType` chain,
/// base first (XTypes 1.3 §7.3.2.5: a derived structure carries the
/// members of its base).
fn all_members(s: &StructType, type_lib: &[TypeLibrary]) -> Result<Vec<StructMember>, XmlError> {
    let mut chain: Vec<StructType> = Vec::new();
    let mut next = s.base_type.clone();
    while let Some(base_name) = next {
        if chain.len() >= MAX_BASE_DEPTH {
            return Err(XmlError::CircularInheritance(format!(
                "struct `{}`: baseType chain deeper than {MAX_BASE_DEPTH}",
                s.name
            )));
        }
        match resolve_named_type(&base_name, type_lib) {
            Some(TypeDef::Struct(b)) => {
                next = b.base_type.clone();
                chain.push(b);
            }
            _ => {
                return Err(XmlError::UnresolvedReference(format!(
                    "baseType `{base_name}` of struct `{}` is not a struct in the libraries",
                    s.name
                )));
            }
        }
    }
    let mut members: Vec<StructMember> = Vec::new();
    for b in chain.iter().rev() {
        members.extend(b.members.iter().cloned());
    }
    members.extend(s.members.iter().cloned());
    Ok(members)
}

fn parse_struct_value(
    el: &XmlElement,
    s: &StructType,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    let members = all_members(s, type_lib)?;
    // §7.2.3: the fields of a structure are unordered elements, one per
    // field. An element that is no field, or a field given twice, would
    // be read as nothing or as its first occurrence without a word.
    let mut seen: alloc::collections::BTreeSet<&str> = alloc::collections::BTreeSet::new();
    for child in &el.children {
        if !members.iter().any(|m| m.name == child.name) {
            return Err(XmlError::UnknownElement(format!(
                "{}/{}: not a member of struct `{}`",
                el.name, child.name, s.name
            )));
        }
        if !seen.insert(child.name.as_str()) {
            return Err(XmlError::InvalidXml(format!(
                "<{}> carries member <{}> more than once",
                el.name, child.name
            )));
        }
    }
    let mut map = BTreeMap::new();
    for member in &members {
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
    // §7.3.7.4.2's example writes the active case without a
    // `<discriminator>` element (`<aUnion><aLong>0</aLong></aUnion>`);
    // the case is then identified by the member element that is present.
    let case = match el.child("discriminator") {
        Some(disc_el) => {
            let disc = disc_el.text.trim().to_string();
            u.cases
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
                })?
        }
        None => u
            .cases
            .iter()
            .find(|c| el.child(&c.member.name).is_some())
            .ok_or_else(|| {
                XmlError::MissingRequiredElement(format!(
                    "union `{}`: neither a <discriminator> nor an active case member",
                    u.name
                ))
            })?,
    };
    let val_el = el.child(&case.member.name).ok_or_else(|| {
        XmlError::MissingRequiredElement(format!("union member `{}`", case.member.name))
    })?;
    let value = parse_member_value(val_el, &case.member, type_lib)?;
    // The written discriminator, or the case's first literal when the
    // document named the case only through its member element.
    let discriminator = match el.child("discriminator") {
        Some(d) => d.text.trim().to_string(),
        None => case
            .discriminators
            .iter()
            .find_map(|d| match d {
                UnionDiscriminator::Value(v) => Some(v.clone()),
                UnionDiscriminator::Default => None,
            })
            .unwrap_or_default(),
    };
    Ok(SampleValue::Union {
        discriminator,
        member: case.member.name.clone(),
        value: alloc::boxed::Box::new(value),
    })
}

fn parse_member_value(
    el: &XmlElement,
    member: &StructMember,
    type_lib: &[TypeLibrary],
) -> Result<SampleValue, XmlError> {
    // Array — §7.3.7.3.2 maps an array like a sequence, with one `<item>`
    // per element. The declared dimensions fix how many items there are,
    // nested for more than one dimension.
    if !member.array_dimensions.is_empty() {
        if member.array_dimensions.len() > MAX_ARRAY_DIMENSIONS {
            return Err(XmlError::LimitExceeded(format!(
                "member `{}` declares {} array dimensions, over {MAX_ARRAY_DIMENSIONS}",
                member.name,
                member.array_dimensions.len()
            )));
        }
        let items = parse_array_items(el, &member.array_dimensions, &member.type_ref, type_lib)?;
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
        // §7.3.7.3.2 / XTypes: a bounded sequence may not carry more
        // elements than its `sequenceMaxLength`.
        // `Some(0)` is the unbounded sequence (`sequenceMaxLength="-1"`).
        if let Some(max) = member.sequence_max_length.filter(|m| *m > 0) {
            if items.len() > max as usize {
                return Err(XmlError::ValueOutOfRange(format!(
                    "sequence `{}` has {} items, over sequenceMaxLength {}",
                    member.name,
                    items.len(),
                    max
                )));
            }
        }
        return Ok(SampleValue::Sequence(items));
    }
    parse_inner_value(el, &member.type_ref, type_lib)
}

/// Most array dimensions a member may declare; the nesting of `<item>` the
/// reader follows is bounded by it.
const MAX_ARRAY_DIMENSIONS: usize = 16;

/// Reads one or more array dimensions of `<item>` nesting.
///
/// zerodds-lint: recursion-depth = `dims.len()`, at most
/// [`MAX_ARRAY_DIMENSIONS`] (checked by the caller).
///
/// The outermost `<item>` count must equal the first declared dimension;
/// each further dimension nests inside its parent item (§7.3.7.3.2 maps an
/// array like a sequence, one `<item>` per element).
fn parse_array_items(
    el: &XmlElement,
    dims: &[u32],
    type_ref: &TypeRef,
    type_lib: &[TypeLibrary],
) -> Result<Vec<SampleValue>, XmlError> {
    let (first, rest) = dims
        .split_first()
        .ok_or_else(|| XmlError::ValueOutOfRange("array with no dimensions".to_string()))?;
    let items: Vec<&XmlElement> = el.children_named("item").collect();
    if items.len() != *first as usize {
        return Err(XmlError::ValueOutOfRange(format!(
            "array `{}` has {} items, expected {}",
            el.name,
            items.len(),
            first
        )));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if rest.is_empty() {
            out.push(parse_inner_value(item, type_ref, type_lib)?);
        } else {
            out.push(SampleValue::Array(parse_array_items(
                item, rest, type_ref, type_lib,
            )?));
        }
    }
    Ok(out)
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
        // §7.1.4 Tab.7.1 boolean, case sensitive — the Data Samples
        // building block reads element values under the same rule as the
        // QoS building block does.
        PrimitiveType::Boolean => Ok(PrimitiveValue::Bool(crate::qos_parser::parse_bool_strict(
            t,
        )?)),
        PrimitiveType::Octet | PrimitiveType::UInt8 => parse_uint::<u8>(t).map(PrimitiveValue::U8),
        PrimitiveType::Int8 => parse_i8(t).map(PrimitiveValue::I8),
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
        PrimitiveType::Short => parse_i16(t).map(PrimitiveValue::I16),
        PrimitiveType::UShort => parse_uint::<u16>(t).map(PrimitiveValue::U16),
        PrimitiveType::Long => parse_long_sample(t).map(PrimitiveValue::I32),
        PrimitiveType::ULong => parse_ulong_sample(t).map(PrimitiveValue::U32),
        PrimitiveType::LongLong => parse_i64(t).map(PrimitiveValue::I64),
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

/// Signed integer value from a sample, accepting the `0x…` bit-pattern
/// spelling of §7.1.4 Tab.7.1 next to decimal.
fn parse_i8(s: &str) -> Result<i8, XmlError> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16)
            .map(|v| v as i8)
            .map_err(|e| XmlError::ValueOutOfRange(format!("int8 hex `{s}`: {e}")));
    }
    s.parse::<i8>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("int8 `{s}`: {e}")))
}

fn parse_i16(s: &str) -> Result<i16, XmlError> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u16::from_str_radix(hex, 16)
            .map(|v| v as i16)
            .map_err(|e| XmlError::ValueOutOfRange(format!("short hex `{s}`: {e}")));
    }
    s.parse::<i16>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("short `{s}`: {e}")))
}

fn parse_i64(s: &str) -> Result<i64, XmlError> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16)
            .map(|v| v as i64)
            .map_err(|e| XmlError::ValueOutOfRange(format!("longlong hex `{s}`: {e}")));
    }
    s.parse::<i64>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("longlong `{s}`: {e}")))
}

/// `long` value of a sample. §7.2.2 lets a member be initialized with the
/// name of a constant, and §7.1.4 Tab.7.1 gives `long` the hex spelling
/// and `LENGTH_UNLIMITED` next to the decimal range.
fn parse_long_sample(s: &str) -> Result<i32, XmlError> {
    match s {
        "LENGTH_UNLIMITED" | "TIME_INVALID_SEC" | "TIME_INVALID_NSEC" => {
            Ok(crate::types::LENGTH_UNLIMITED)
        }
        "DURATION_INFINITE_SEC" | "DURATION_INFINITE_NSEC" => {
            Ok(crate::types::DURATION_INFINITE_SEC)
        }
        "DURATION_ZERO_SEC" | "DURATION_ZERO_NSEC" => Ok(crate::types::DURATION_ZERO_SEC),
        _ => crate::types::parse_long(s),
    }
}

/// `unsigned long` value of a sample: decimal or `0x…`, or the name of
/// one of the §7.2.2.1 constants of that type (`nanosec` of `Duration_t`
/// and `Time_t`).
fn parse_ulong_sample(s: &str) -> Result<u32, XmlError> {
    match s {
        "DURATION_INFINITE_NSEC" => Ok(crate::types::DURATION_INFINITE_NSEC),
        "DURATION_ZERO_NSEC" => Ok(crate::types::DURATION_ZERO_NSEC),
        "TIME_INVALID_NSEC" => Ok(crate::types::TIME_INVALID_NSEC),
        _ => parse_uint::<u32>(s),
    }
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
/// Wraps the result in `<sample type_ref="…">`, the form of DDS-XML 1.0
/// §7.3.7. The XML Data Representation of DDS-XTypes 1.3 §7.4.4 has the
/// type's name as its root and is written by
/// [`serialize_data_representation`]. Member names
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

/// The two variants of DDS-XTypes 1.3 §7.4.4 "XML Data Representation".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRepresentationForm {
    /// §7.4.4.1: the document declares the namespace it may be validated
    /// against, the implied `ddstype://www.omg.org/<module path>` where
    /// the XSD names none.
    Valid,
    /// §7.4.4.2: the document declares no namespace, "well-formed but not
    /// valid".
    WellFormed,
}

/// The namespace §7.4.4.1 implies for a type: `ddstype://www.omg.org/`
/// followed by the enclosing modules, outermost first, separated by `/`.
/// A type outside every module gets `ddstype://www.omg.org`.
#[must_use]
pub fn implied_namespace(module_path: &[&str]) -> String {
    let mut out = String::from("ddstype://www.omg.org");
    for module in module_path {
        out.push('/');
        out.push_str(module);
    }
    out
}

/// Serializes a sample as one XML document of the XML Data Representation
/// (DDS-XTypes 1.3 §7.4.4).
///
/// The root element is named after the type, as the XSD Type
/// Representation names its document root (§7.3.3.3) and as the
/// clause's example `<MyStructure>` does; members are elements of their
/// own name and the elements of a collection are `<item>`. For
/// [`DataRepresentationForm::Valid`] the root declares the implied
/// namespace of `module_path` as the default namespace — the form the
/// clause's non-normative note names, `xmlns="ddstype://www.omg.org/…"` —
/// so that no element needs a prefix. For
/// [`DataRepresentationForm::WellFormed`] no namespace is declared.
///
/// `serialize_sample` is the `<sample type_ref="…">` form of DDS-XML 1.0
/// §7.3.7 and is a different document.
#[must_use]
pub fn serialize_data_representation(
    value: &SampleValue,
    type_def: &TypeDef,
    module_path: &[&str],
    form: DataRepresentationForm,
) -> String {
    let name = type_def.name();
    let mut out = String::new();
    out.push('<');
    out.push_str(name);
    if form == DataRepresentationForm::Valid {
        out.push_str(" xmlns=\"");
        out.push_str(&implied_namespace(module_path));
        out.push('"');
    }
    out.push('>');
    serialize_value_body(value, &mut out);
    out.push_str("</");
    out.push_str(name);
    out.push('>');
    out
}

/// Parses a document of the XML Data Representation (§7.4.4) and says
/// which variant it is.
///
/// The root element has to be named after the type. A declared namespace
/// has to be the one [`implied_namespace`] gives for `module_path`; a
/// document that declares another is neither variant of the clause for
/// this type.
///
/// # Errors
/// [`XmlError::InvalidXml`] for a root of another name or another
/// namespace; otherwise as [`parse_sample`].
pub fn parse_data_representation(
    xml: &str,
    type_def: &TypeDef,
    module_path: &[&str],
    type_lib: &[TypeLibrary],
) -> Result<(SampleValue, DataRepresentationForm), XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != type_def.name() {
        return Err(XmlError::InvalidXml(format!(
            "the root element is <{}>, the XML Data Representation of `{}` has <{}> (DDS-XTypes 1.3 §7.4.4)",
            doc.root.name,
            type_def.name(),
            type_def.name()
        )));
    }
    let form = match doc.root.namespace.as_deref() {
        None => DataRepresentationForm::WellFormed,
        Some(declared) if declared == implied_namespace(module_path) => {
            DataRepresentationForm::Valid
        }
        Some(declared) => {
            return Err(XmlError::InvalidXml(format!(
                "the document declares the namespace `{declared}`, the implied one of `{}` is `{}` (DDS-XTypes 1.3 §7.4.4.1)",
                type_def.name(),
                implied_namespace(module_path)
            )));
        }
    };
    let value = parse_sample_element(&doc.root, type_def, type_lib)?;
    Ok((value, form))
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
            discriminator: _,
            member,
            value,
        } => {
            // §7.3.7.4.2: the active case is written as the member element
            // holding the value, without a `<discriminator>` wrapper.
            out.push('<');
            out.push_str(member);
            out.push('>');
            serialize_value_body(value, out);
            out.push_str("</");
            out.push_str(member);
            out.push('>');
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
            // §7.1.4 Table 7.1: the reserved characters are escaped; any
            // other non-ASCII character is written as a numeric reference,
            // as the example of §7.3.7.4.5 does.
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

    fn point() -> TypeDef {
        TypeDef::Struct(StructType {
            name: "MyStructure".into(),
            members: alloc::vec![
                StructMember {
                    name: "my_integer".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Long),
                    ..Default::default()
                },
                StructMember {
                    name: "my_sequence_of_doubles".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Double),
                    sequence_max_length: Some(8),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
    }

    fn point_value() -> SampleValue {
        let mut m = BTreeMap::new();
        m.insert(
            "my_integer".to_string(),
            SampleValue::Primitive(PrimitiveValue::I32(5)),
        );
        m.insert(
            "my_sequence_of_doubles".to_string(),
            SampleValue::Sequence(alloc::vec![
                SampleValue::Primitive(PrimitiveValue::F64(10.0)),
                SampleValue::Primitive(PrimitiveValue::F64(20.0)),
            ]),
        );
        SampleValue::Struct(m)
    }

    /// §7.4.4.2: the root is the type's name and no namespace is declared.
    #[test]
    fn the_well_formed_form_has_the_type_as_root_and_no_namespace() {
        let xml = serialize_data_representation(
            &point_value(),
            &point(),
            &["MyModule1", "MyModule2"],
            DataRepresentationForm::WellFormed,
        );
        assert!(xml.starts_with("<MyStructure>"), "{xml}");
        assert!(!xml.contains("xmlns"), "{xml}");
        assert!(xml.ends_with("</MyStructure>"), "{xml}");
        assert!(xml.contains("<my_integer>5</my_integer>"), "{xml}");
        assert!(xml.contains("<item>10</item>"), "{xml}");
    }

    /// §7.4.4.1: the implied namespace is
    /// `ddstype://www.omg.org/<module path>`.
    #[test]
    fn the_valid_form_declares_the_implied_namespace() {
        let xml = serialize_data_representation(
            &point_value(),
            &point(),
            &["MyModule1", "MyModule2"],
            DataRepresentationForm::Valid,
        );
        assert!(
            xml.starts_with("<MyStructure xmlns=\"ddstype://www.omg.org/MyModule1/MyModule2\">"),
            "{xml}"
        );
        assert_eq!(implied_namespace(&[]), "ddstype://www.omg.org");
    }

    /// Both forms read back to the value, and say which form they were.
    #[test]
    fn both_forms_round_trip_and_are_told_apart() {
        for form in [
            DataRepresentationForm::Valid,
            DataRepresentationForm::WellFormed,
        ] {
            let xml = serialize_data_representation(&point_value(), &point(), &["M"], form);
            let (value, found) =
                parse_data_representation(&xml, &point(), &["M"], &[]).expect("parse");
            assert_eq!(found, form);
            let SampleValue::Struct(m) = value else {
                panic!("not a struct")
            };
            assert!(matches!(
                m.get("my_integer"),
                Some(SampleValue::Primitive(PrimitiveValue::I32(5)))
            ));
        }
    }

    /// A root of another name and a namespace of another module path are
    /// refused.
    #[test]
    fn another_root_or_another_namespace_is_refused() {
        let wrong_root = "<sample type_ref=\"MyStructure\"><my_integer>5</my_integer></sample>";
        assert!(parse_data_representation(wrong_root, &point(), &[], &[]).is_err());
        let xml = serialize_data_representation(
            &point_value(),
            &point(),
            &["Other"],
            DataRepresentationForm::Valid,
        );
        assert!(parse_data_representation(&xml, &point(), &["M"], &[]).is_err());
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

    /// §7.3.7.3.2: the declared array dimensions fix the item count.
    #[test]
    fn parse_sample_array_length_is_checked() {
        let td = TypeDef::Struct(StructType {
            name: "A".into(),
            members: alloc::vec![StructMember {
                name: "coords".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                array_dimensions: alloc::vec![3],
                ..Default::default()
            }],
            ..Default::default()
        });
        let too_many = r#"<sample type_ref="A"><coords><item>1</item><item>2</item><item>3</item><item>4</item></coords></sample>"#;
        assert!(matches!(
            parse_sample(too_many, &td, &[]),
            Err(XmlError::ValueOutOfRange(_))
        ));
        let too_few =
            r#"<sample type_ref="A"><coords><item>1</item><item>2</item></coords></sample>"#;
        assert!(matches!(
            parse_sample(too_few, &td, &[]),
            Err(XmlError::ValueOutOfRange(_))
        ));
    }

    #[test]
    fn parse_sample_multidimensional_array_is_nested() {
        // long[2][2] is two items, each holding two items.
        let td = TypeDef::Struct(StructType {
            name: "A".into(),
            members: alloc::vec![StructMember {
                name: "grid".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                array_dimensions: alloc::vec![2, 2],
                ..Default::default()
            }],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="A"><grid>
            <item><item>1</item><item>2</item></item>
            <item><item>3</item><item>4</item></item>
        </grid></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("nested");
        let SampleValue::Struct(m) = v else { panic!() };
        let SampleValue::Array(rows) = m.get("grid").expect("grid") else {
            panic!("expected outer array")
        };
        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[0], SampleValue::Array(inner) if inner.len() == 2));
    }

    /// §7.3.7.3.2 / XTypes: a bounded sequence may not exceed its bound.
    #[test]
    fn parse_sample_sequence_over_bound_rejected() {
        let td = TypeDef::Struct(StructType {
            name: "S".into(),
            members: alloc::vec![StructMember {
                name: "ids".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                sequence_max_length: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="S"><ids><item>1</item><item>2</item><item>3</item></ids></sample>"#;
        assert!(matches!(
            parse_sample(xml, &td, &[]),
            Err(XmlError::ValueOutOfRange(_))
        ));
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
            ..Default::default()
        });
        let xml = r#"<sample type_ref="U"><discriminator>1</discriminator><a>42</a></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse union");
        let SampleValue::Union {
            discriminator,
            member,
            value,
        } = v
        else {
            panic!("expected Union")
        };
        assert_eq!(discriminator, "1");
        assert_eq!(member, "a");
        assert!(matches!(
            *value,
            SampleValue::Primitive(PrimitiveValue::I32(42))
        ));
    }

    /// §7.3.7.4.2's example writes the union without a `<discriminator>`
    /// element; the active case is the member element that is present.
    #[test]
    fn parse_sample_union_without_discriminator() {
        use crate::xtypes_def::{UnionCase, UnionType};
        let td = TypeDef::Union(UnionType {
            name: "U".into(),
            discriminator: TypeRef::Primitive(PrimitiveType::Long),
            cases: alloc::vec![
                UnionCase {
                    discriminators: alloc::vec![UnionDiscriminator::Value("0".into())],
                    member: StructMember {
                        name: "aLong".into(),
                        type_ref: TypeRef::Primitive(PrimitiveType::Long),
                        ..Default::default()
                    },
                },
                UnionCase {
                    discriminators: alloc::vec![UnionDiscriminator::Value("1".into())],
                    member: StructMember {
                        name: "other".into(),
                        type_ref: TypeRef::Primitive(PrimitiveType::Long),
                        ..Default::default()
                    },
                },
            ],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="U"><aLong>0</aLong></sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse union");
        let SampleValue::Union {
            discriminator,
            member,
            value,
        } = v
        else {
            panic!("expected Union")
        };
        assert_eq!(member, "aLong");
        assert_eq!(discriminator, "0");
        assert!(matches!(
            *value,
            SampleValue::Primitive(PrimitiveValue::I32(0))
        ));
    }

    #[test]
    fn parse_sample_long_hex_and_constant_names() {
        let td = TypeDef::Struct(StructType {
            name: "X".into(),
            members: alloc::vec![
                StructMember {
                    name: "hex".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Long),
                    ..Default::default()
                },
                StructMember {
                    name: "unlimited".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Long),
                    ..Default::default()
                },
                StructMember {
                    name: "infinite".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Long),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="X">
            <hex>0x10</hex>
            <unlimited>LENGTH_UNLIMITED</unlimited>
            <infinite>DURATION_INFINITE_SEC</infinite>
        </sample>"#;
        let v = parse_sample(xml, &td, &[]).expect("parse");
        let SampleValue::Struct(m) = v else { panic!() };
        assert!(matches!(
            m.get("hex"),
            Some(SampleValue::Primitive(PrimitiveValue::I32(0x10)))
        ));
        assert!(matches!(
            m.get("unlimited"),
            Some(SampleValue::Primitive(PrimitiveValue::I32(-1)))
        ));
        assert!(matches!(
            m.get("infinite"),
            Some(SampleValue::Primitive(PrimitiveValue::I32(2147483647)))
        ));
    }

    #[test]
    fn parse_sample_enum_rejects_unknown_literal() {
        use crate::xtypes_def::{EnumLiteral, EnumType};
        let td = TypeDef::Enum(EnumType {
            name: "E".into(),
            enumerators: alloc::vec![EnumLiteral {
                name: "ACE".into(),
                value: None,
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(parse_sample(r#"<sample type_ref="E">ACE</sample>"#, &td, &[]).is_ok());
        let err =
            parse_sample(r#"<sample type_ref="E">JOKER</sample>"#, &td, &[]).expect_err("bad");
        assert!(matches!(err, XmlError::BadEnum(_)));
    }

    #[test]
    fn serialize_union_round_trips() {
        use crate::xtypes_def::{UnionCase, UnionType};
        let td = TypeDef::Union(UnionType {
            name: "U".into(),
            discriminator: TypeRef::Primitive(PrimitiveType::Long),
            cases: alloc::vec![UnionCase {
                discriminators: alloc::vec![UnionDiscriminator::Value("0".into())],
                member: StructMember {
                    name: "aLong".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Long),
                    ..Default::default()
                },
            }],
            ..Default::default()
        });
        let value = SampleValue::Union {
            discriminator: "0".into(),
            member: "aLong".into(),
            value: alloc::boxed::Box::new(SampleValue::Primitive(PrimitiveValue::I32(0))),
        };
        let out = serialize_sample(&value, &td, &[]);
        assert!(out.contains("<aLong>0</aLong>"), "{out}");
        let reparsed = parse_sample(&out, &td, &[]).expect("reparse");
        assert_eq!(reparsed, value);
    }

    fn long_member(name: &str) -> StructMember {
        StructMember {
            name: name.into(),
            type_ref: TypeRef::Primitive(PrimitiveType::Long),
            ..Default::default()
        }
    }

    /// §7.2.3: a structure's fields are its elements; an element that is
    /// no field is an error and not read as nothing.
    #[test]
    fn parse_sample_unknown_member_rejected() {
        let td = TypeDef::Struct(StructType {
            name: "X".into(),
            members: alloc::vec![long_member("id")],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="X"><id>1</id><idd>2</idd></sample>"#;
        let err = parse_sample(xml, &td, &[]).expect_err("unknown");
        assert!(matches!(err, XmlError::UnknownElement(_)), "got {err:?}");
    }

    /// §7.2.3: a field is a single element; a second occurrence is
    /// reported and not dropped.
    #[test]
    fn parse_sample_duplicate_member_rejected() {
        let td = TypeDef::Struct(StructType {
            name: "X".into(),
            members: alloc::vec![long_member("id")],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="X"><id>1</id><id>2</id></sample>"#;
        let err = parse_sample(xml, &td, &[]).expect_err("duplicate");
        assert!(matches!(err, XmlError::InvalidXml(_)), "got {err:?}");
    }

    /// Fields in any order (§7.2.3 "unordered elements").
    #[test]
    fn parse_sample_members_in_any_order() {
        let td = TypeDef::Struct(StructType {
            name: "X".into(),
            members: alloc::vec![long_member("a"), long_member("b")],
            ..Default::default()
        });
        let v = parse_sample(
            r#"<sample type_ref="X"><b>2</b><a>1</a></sample>"#,
            &td,
            &[],
        )
        .expect("parse");
        let SampleValue::Struct(m) = v else { panic!() };
        assert!(matches!(
            m.get("a"),
            Some(SampleValue::Primitive(PrimitiveValue::I32(1)))
        ));
        assert!(matches!(
            m.get("b"),
            Some(SampleValue::Primitive(PrimitiveValue::I32(2)))
        ));
    }

    /// XTypes 1.3 §7.3.2.5: a derived structure carries the members of
    /// its base; a sample of it has them as elements.
    #[test]
    fn parse_sample_reads_inherited_members() {
        let libs = crate::xtypes_parser::parse_type_libraries(
            r#"<types>
                <struct name="Base"><member name="a" type="int32"/></struct>
                <struct name="Derived" baseType="Base"><member name="b" type="int32"/></struct>
            </types>"#,
        )
        .expect("types");
        let derived = libs[0].find("Derived").expect("Derived").clone();
        let v = parse_sample(
            r#"<sample type_ref="Derived"><a>1</a><b>2</b></sample>"#,
            &derived,
            &libs,
        )
        .expect("parse");
        let SampleValue::Struct(m) = v else { panic!() };
        assert!(m.contains_key("a"));
        assert!(m.contains_key("b"));
        // The inherited member is required like any other.
        let err = parse_sample(
            r#"<sample type_ref="Derived"><b>2</b></sample>"#,
            &derived,
            &libs,
        )
        .expect_err("missing base member");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
        // A base that is not defined is reported.
        let err = parse_sample(
            r#"<sample type_ref="Derived"><a>1</a><b>2</b></sample>"#,
            &derived,
            &[],
        )
        .expect_err("no base");
        assert!(matches!(err, XmlError::UnresolvedReference(_)));
    }

    /// XTypes 1.3 §7.3.2.4.2: `sequenceMaxLength="-1"` is the unbounded
    /// sequence, which takes any number of items, none included.
    #[test]
    fn parse_sample_unbounded_sequence() {
        let libs = crate::xtypes_parser::parse_type_libraries(
            r#"<types><struct name="S">
                <member name="ids" type="int32" sequenceMaxLength="-1"/>
            </struct></types>"#,
        )
        .expect("types");
        let s = libs[0].find("S").expect("S").clone();
        let v = parse_sample(
            r#"<sample type_ref="S"><ids><item>1</item><item>2</item><item>3</item></ids></sample>"#,
            &s,
            &libs,
        )
        .expect("items");
        let SampleValue::Struct(m) = v else { panic!() };
        assert!(matches!(m.get("ids"), Some(SampleValue::Sequence(i)) if i.len() == 3));
        let v = parse_sample(r#"<sample type_ref="S"><ids/></sample>"#, &s, &libs).expect("none");
        let SampleValue::Struct(m) = v else { panic!() };
        assert!(matches!(m.get("ids"), Some(SampleValue::Sequence(i)) if i.is_empty()));
    }

    /// §7.2.2.1: `nanosec` is an `unsigned long`; the constants of that
    /// type are legal values next to the numbers.
    #[test]
    fn parse_sample_ulong_constant_names() {
        let td = TypeDef::Struct(StructType {
            name: "D".into(),
            members: alloc::vec![
                StructMember {
                    name: "inf".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::ULong),
                    ..Default::default()
                },
                StructMember {
                    name: "zero".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::ULong),
                    ..Default::default()
                },
                StructMember {
                    name: "invalid".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::ULong),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        let xml = r#"<sample type_ref="D"><inf>DURATION_INFINITE_NSEC</inf><zero>DURATION_ZERO_NSEC</zero><invalid>TIME_INVALID_NSEC</invalid></sample>"#;
        let SampleValue::Struct(m) = parse_sample(xml, &td, &[]).expect("parse") else {
            panic!()
        };
        assert!(matches!(
            m.get("inf"),
            Some(SampleValue::Primitive(PrimitiveValue::U32(0x7FFF_FFFF)))
        ));
        assert!(matches!(
            m.get("zero"),
            Some(SampleValue::Primitive(PrimitiveValue::U32(0)))
        ));
        assert!(matches!(
            m.get("invalid"),
            Some(SampleValue::Primitive(PrimitiveValue::U32(0xFFFF_FFFF)))
        ));
    }

    /// XTypes 1.3 Annex A: `int8` is a signed and `uint8` an unsigned 8-bit
    /// integer; each takes exactly its range.
    #[test]
    fn parse_sample_int8_and_uint8_ranges() {
        let td = TypeDef::Struct(StructType {
            name: "B".into(),
            members: alloc::vec![
                StructMember {
                    name: "s".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::Int8),
                    ..Default::default()
                },
                StructMember {
                    name: "u".into(),
                    type_ref: TypeRef::Primitive(PrimitiveType::UInt8),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        let ok = parse_sample(
            r#"<sample type_ref="B"><s>-128</s><u>255</u></sample>"#,
            &td,
            &[],
        )
        .expect("range ends");
        let SampleValue::Struct(m) = ok else { panic!() };
        assert!(matches!(
            m.get("s"),
            Some(SampleValue::Primitive(PrimitiveValue::I8(-128)))
        ));
        assert!(matches!(
            m.get("u"),
            Some(SampleValue::Primitive(PrimitiveValue::U8(255)))
        ));
        for bad in [
            r#"<sample type_ref="B"><s>128</s><u>0</u></sample>"#,
            r#"<sample type_ref="B"><s>0</s><u>256</u></sample>"#,
            r#"<sample type_ref="B"><s>0</s><u>-1</u></sample>"#,
        ] {
            assert!(
                matches!(
                    parse_sample(bad, &td, &[]),
                    Err(XmlError::ValueOutOfRange(_))
                ),
                "{bad}"
            );
        }
    }
}
