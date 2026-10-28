// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The XML Data Representation of a sample on the wire. DDS-XTypes 1.3
//! §7.4.4, Table 39 and Table 60 (encapsulation identifier `{0x00, 0x04}`).
//!
//! A sample is one XML document whose structure follows the XSD Type
//! Representation of its type (§7.4.4): the root element is the type's
//! simple name, a structure's members are elements of their own name, the
//! elements of a sequence or an array are `<item>`, a map entry is an
//! `<item>` with a `<key>` and a `<value>`, an optional member that has no
//! value is left out, and "samples of mutable types are serialized no
//! differently than samples of final or appendable types".
//!
//! Two things the clause leaves to the type mapping are fixed here, in the
//! form `zerodds-xml`'s sample codec already uses: a union is its
//! `<discriminator>` and the element of the selected member, and a
//! bitmask is the names of its set flags separated by blanks.
//!
//! The wire form is the **Well-formed** variant (§7.4.4.2): a receiver is
//! assumed to infer the namespace, which keeps a sample small. A reader
//! accepts a document with a namespace as well, and ignores prefixes.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::PrimitiveKind as P;
use zerodds_float128::Float128;
use zerodds_types::dynamic::{DynamicData, DynamicType, DynamicTypeMember, TypeKind};

use crate::serialized_type::SerializedTypeSupport;

/// How deep a value is followed before the document is refused.
const MAX_DEPTH: usize = 64;

/// The XML document of a sample.
///
/// # Errors
/// The value does not fit the type.
pub fn to_xml(support: &SerializedTypeSupport, value: &DynamicData) -> Result<String, String> {
    let ty = support.get_type()?;
    let name = ty.name().to_string();
    let mut out = String::new();
    out.push('<');
    out.push_str(&name);
    out.push('>');
    write_value(&mut out, &ty, value, 0)?;
    out.push_str("</");
    out.push_str(&name);
    out.push('>');
    Ok(out)
}

/// The value an XML document holds, for the type of the support.
///
/// # Errors
/// The document is not well-formed, its root is not the type's name, or an
/// element is none of the type's.
pub fn from_xml(support: &SerializedTypeSupport, xml: &str) -> Result<DynamicData, String> {
    let ty = support.get_type()?;
    let root = parse_document(xml)?;
    if root.name != ty.name() {
        return Err(format!(
            "the root element is <{}>, the XML Data Representation of `{}` has <{}>",
            root.name,
            ty.name(),
            ty.name()
        ));
    }
    read_value(&ty, &root, 0)
}

/// Whether `payload` starts with the XML encapsulation header: `{0x00,
/// 0x04}` (DDS-XTypes 1.3 Table 60, DDSI-RTPS 2.5 Table 10.3) or `{0x01,
/// 0x00}` (Table 39, §7.4.3.4).
#[must_use]
pub fn is_xml_payload(payload: &[u8]) -> bool {
    payload.len() >= 4 && matches!(payload[..2], [0x00, 0x04] | [0x01, 0x00])
}

/// The XML document of an instance key: the `@key` members of the value
/// and no other, which is what an XML `serializedKey` carries.
///
/// # Errors
/// The value does not fit the type.
pub fn to_xml_key(support: &SerializedTypeSupport, value: &DynamicData) -> Result<String, String> {
    let ty = support.get_type()?;
    let name = ty.name().to_string();
    let entries = value.members().ok_or("a key holder is not an aggregate")?;
    let mut out = String::new();
    element(&mut out, &name, |out| {
        for member in members_of(&ty) {
            if !member.descriptor().map_err(|e| e.to_string())?.is_key {
                continue;
            }
            if let Some((_, inner)) = entries.iter().find(|(n, _)| n == member.name()) {
                let member_ty = member.dynamic_type().map_err(|e| e.to_string())?;
                element(out, member.name(), |out| {
                    write_value(out, &member_ty, inner, 1)
                })?;
            }
        }
        Ok(())
    })?;
    Ok(out)
}

/// The members of a structure, those of its bases first. The chain of bases
/// is walked in a loop of at most `MAX_DEPTH` steps.
fn members_of(ty: &DynamicType) -> Vec<DynamicTypeMember> {
    let mut chain = alloc::vec![ty.clone()];
    let mut current = ty.clone();
    for _ in 0..MAX_DEPTH {
        let Some(base) = current.base_type().and_then(|b| b.resolve_alias().ok()) else {
            break;
        };
        chain.push(base.clone());
        current = base;
    }
    chain.reverse();
    chain.iter().flat_map(DynamicType::members).collect()
}

fn dimensions(ty: &DynamicType) -> usize {
    ty.descriptor().map_or(1, |d| d.bound.len().max(1))
}

fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

fn element(
    out: &mut String,
    name: &str,
    body: impl FnOnce(&mut String) -> Result<(), String>,
) -> Result<(), String> {
    out.push('<');
    out.push_str(name);
    out.push('>');
    body(out)?;
    out.push_str("</");
    out.push_str(name);
    out.push('>');
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (`MAX_DEPTH`, checked first thing and
/// returned as an error)
fn write_value(
    out: &mut String,
    ty: &DynamicType,
    value: &DynamicData,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(String::from(
            "the value nests deeper than the document is written",
        ));
    }
    let ty = ty.resolve_alias().map_err(|e| e.to_string())?;
    match ty.kind() {
        TypeKind::Structure | TypeKind::Annotation => {
            let entries = value.members().ok_or("a structure is not an aggregate")?;
            for member in members_of(&ty) {
                if let Some((_, inner)) = entries.iter().find(|(n, _)| n == member.name()) {
                    let member_ty = member.dynamic_type().map_err(|e| e.to_string())?;
                    element(out, member.name(), |out| {
                        write_value(out, &member_ty, inner, depth + 1)
                    })?;
                }
            }
            Ok(())
        }
        TypeKind::Union => {
            let entries = value.members().ok_or("a union is not an aggregate")?;
            let disc_ty = ty
                .discriminator_type()
                .ok_or("a union without a discriminator")?;
            if let Some((_, d)) = entries.iter().find(|(n, _)| n == "_d") {
                element(out, "discriminator", |out| write_scalar(out, &disc_ty, d))?;
            }
            for (name, inner) in entries.iter().filter(|(n, _)| n != "_d") {
                let member = ty
                    .members()
                    .into_iter()
                    .find(|m| m.name() == name)
                    .ok_or_else(|| format!("`{name}` is no member of the union"))?;
                let member_ty = member.dynamic_type().map_err(|e| e.to_string())?;
                element(out, name, |out| {
                    write_value(out, &member_ty, inner, depth + 1)
                })?;
            }
            Ok(())
        }
        TypeKind::Sequence | TypeKind::Array => {
            let element_ty = ty
                .element_type()
                .ok_or("a collection without an element type")?;
            let dims = if ty.kind() == TypeKind::Array {
                dimensions(&ty)
            } else {
                1
            };
            write_items(out, &element_ty, value, dims, depth)
        }
        TypeKind::Map => {
            let key_ty = ty.key_element_type().ok_or("a map without a key type")?;
            let value_ty = ty.element_type().ok_or("a map without a value type")?;
            let entries = value.items().ok_or("a map is not a collection")?;
            for entry in entries {
                let pair = entry
                    .items()
                    .filter(|p| p.len() == 2)
                    .ok_or("a malformed map entry")?;
                element(out, "item", |out| {
                    element(out, "key", |out| {
                        write_value(out, &key_ty, &pair[0], depth + 1)
                    })?;
                    element(out, "value", |out| {
                        write_value(out, &value_ty, &pair[1], depth + 1)
                    })
                })?;
            }
            Ok(())
        }
        _ => write_scalar(out, &ty, value),
    }
}

/// zerodds-lint: recursion-depth 64 (`MAX_DEPTH`, checked first thing and
/// returned as an error)
fn write_items(
    out: &mut String,
    element_ty: &DynamicType,
    value: &DynamicData,
    dims: usize,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(String::from(
            "the value nests deeper than the document is written",
        ));
    }
    let items = value
        .items()
        .ok_or("a collection is not a collection value")?;
    for item in items {
        element(out, "item", |out| {
            if dims > 1 {
                write_items(out, element_ty, item, dims - 1, depth + 1)
            } else {
                write_value(out, element_ty, item, depth + 1)
            }
        })?;
    }
    Ok(())
}

fn write_scalar(out: &mut String, ty: &DynamicType, value: &DynamicData) -> Result<(), String> {
    match value {
        DynamicData::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        DynamicData::Integer { value, .. } => out.push_str(&value.to_string()),
        DynamicData::Float { value, .. } => {
            if value.is_nan() {
                out.push_str("NaN");
            } else if value.is_infinite() {
                out.push_str(if *value > 0.0 { "INF" } else { "-INF" });
            } else {
                out.push_str(&format!("{value:?}"));
            }
        }
        DynamicData::LongDouble(v) => out.push_str(&v.to_shortest_decimal()),
        DynamicData::Character { value, .. }
        | DynamicData::Text(value)
        | DynamicData::Fixed(value)
        | DynamicData::Enumerator(value) => {
            escape(value, out);
        }
        DynamicData::Bits(flags) => {
            let names: Vec<String> = flags
                .iter()
                .filter_map(|f| match f {
                    DynamicData::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .collect();
            escape(&names.join(" "), out);
        }
        other => {
            return Err(format!(
                "`{}` is {}, which has no scalar form",
                ty.name(),
                zerodds_wire_codec::Source::kind(other)
            ));
        }
    }
    Ok(())
}

// -- reading ------------------------------------------------------------

#[derive(Debug, Default)]
struct Node {
    name: String,
    text: String,
    children: Vec<Node>,
}

fn unescape(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let end = tail.find(';').ok_or("an entity without its semicolon")?;
        match &tail[..end] {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            number if number.starts_with('#') => {
                let code = if let Some(hex) = number.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16)
                } else {
                    number[1..].parse::<u32>()
                };
                out.push(
                    code.ok()
                        .and_then(char::from_u32)
                        .ok_or("a character reference that is no character")?,
                );
            }
            other => return Err(format!("`&{other};` is no entity of XML")),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn parse_document(xml: &str) -> Result<Node, String> {
    let mut chars = xml.trim_start_matches('\u{feff}');
    // The prolog: declaration, comments, processing instructions.
    loop {
        chars = chars.trim_start();
        if let Some(rest) = chars.strip_prefix("<?") {
            chars = &rest[rest
                .find("?>")
                .ok_or("an unterminated processing instruction")?
                + 2..];
        } else if let Some(rest) = chars.strip_prefix("<!--") {
            chars = &rest[rest.find("-->").ok_or("an unterminated comment")? + 3..];
        } else {
            break;
        }
    }
    let (node, rest) = parse_element(chars, 0)?;
    let mut rest = rest;
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = &r[r.find("-->").ok_or("an unterminated comment")? + 3..];
        } else {
            break;
        }
    }
    if !rest.is_empty() {
        return Err(String::from("content after the root element"));
    }
    Ok(node)
}

/// zerodds-lint: recursion-depth 64 (`MAX_DEPTH`, checked first thing and
/// returned as an error)
fn parse_element(input: &str, depth: usize) -> Result<(Node, &str), String> {
    if depth > MAX_DEPTH {
        return Err(String::from("the document nests deeper than it is read"));
    }
    let rest = input.strip_prefix('<').ok_or("an element is expected")?;
    let name_end = rest
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .ok_or("an unterminated tag")?;
    let name = local_name(&rest[..name_end]).to_string();
    let mut rest = &rest[name_end..];
    // Attributes are skipped (namespace declarations among them).
    let mut quote: Option<char> = None;
    let mut at = 0;
    for (index, c) in rest.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => {
                at = index;
                break;
            }
            None => {}
        }
        if index + c.len_utf8() == rest.len() {
            return Err(String::from("an unterminated tag"));
        }
    }
    let self_closing = rest[..at].ends_with('/');
    rest = &rest[at + 1..];
    let mut node = Node {
        name,
        ..Node::default()
    };
    if self_closing {
        return Ok((node, rest));
    }
    loop {
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = &r[r.find("-->").ok_or("an unterminated comment")? + 3..];
        } else if let Some(r) = rest.strip_prefix("</") {
            let end = r.find('>').ok_or("an unterminated end tag")?;
            if local_name(r[..end].trim()) != node.name {
                return Err(format!("`</{}>` closes <{}>", r[..end].trim(), node.name));
            }
            node.text = unescape(&node.text)?;
            return Ok((node, &r[end + 1..]));
        } else if rest.starts_with('<') {
            let (child, r) = parse_element(rest, depth + 1)?;
            node.children.push(child);
            rest = r;
        } else {
            let end = rest.find('<').ok_or("an unterminated element")?;
            node.text.push_str(&rest[..end]);
            rest = &rest[end..];
        }
    }
}

/// zerodds-lint: recursion-depth 64 (`MAX_DEPTH`, checked first thing and
/// returned as an error)
fn read_value(ty: &DynamicType, node: &Node, depth: usize) -> Result<DynamicData, String> {
    if depth > MAX_DEPTH {
        return Err(String::from("the value nests deeper than it is read"));
    }
    let ty = ty.resolve_alias().map_err(|e| e.to_string())?;
    match ty.kind() {
        TypeKind::Structure | TypeKind::Annotation => {
            let members = members_of(&ty);
            let mut entries = Vec::new();
            for child in &node.children {
                let member = members
                    .iter()
                    .find(|m| m.name() == child.name)
                    .ok_or_else(|| format!("<{}> is no member of `{}`", child.name, ty.name()))?;
                let member_ty = member.dynamic_type().map_err(|e| e.to_string())?;
                if entries
                    .iter()
                    .any(|(n, _): &(String, DynamicData)| n == member.name())
                {
                    return Err(format!("<{}> appears twice", child.name));
                }
                entries.push((
                    member.name().to_string(),
                    read_value(&member_ty, child, depth + 1)?,
                ));
            }
            // Members in declaration order, as the encoder expects them.
            entries.sort_by_key(|(n, _)| members.iter().position(|m| m.name() == n));
            Ok(DynamicData::Aggregate(entries))
        }
        TypeKind::Union => {
            let disc_ty = ty
                .discriminator_type()
                .ok_or("a union without a discriminator")?;
            let mut entries = Vec::new();
            for child in &node.children {
                if child.name == "discriminator" {
                    entries.insert(
                        0,
                        (
                            String::from("_d"),
                            read_scalar(
                                &disc_ty.resolve_alias().map_err(|e| e.to_string())?,
                                &child.text,
                            )?,
                        ),
                    );
                    continue;
                }
                let member = ty
                    .members()
                    .into_iter()
                    .find(|m| m.name() == child.name)
                    .ok_or_else(|| format!("<{}> is no member of `{}`", child.name, ty.name()))?;
                let member_ty = member.dynamic_type().map_err(|e| e.to_string())?;
                entries.push((
                    member.name().to_string(),
                    read_value(&member_ty, child, depth + 1)?,
                ));
            }
            if !entries.iter().any(|(n, _)| n == "_d") {
                return Err(String::from("a union needs its <discriminator>"));
            }
            Ok(DynamicData::Aggregate(entries))
        }
        TypeKind::Sequence | TypeKind::Array => {
            let element_ty = ty
                .element_type()
                .ok_or("a collection without an element type")?;
            let dims = if ty.kind() == TypeKind::Array {
                dimensions(&ty)
            } else {
                1
            };
            read_items(&element_ty, node, dims, depth)
        }
        TypeKind::Map => {
            let key_ty = ty.key_element_type().ok_or("a map without a key type")?;
            let value_ty = ty.element_type().ok_or("a map without a value type")?;
            let mut entries = Vec::new();
            for item in &node.children {
                if item.name != "item" {
                    return Err(format!("<{}> in a map; <item> is due", item.name));
                }
                let key = item
                    .children
                    .iter()
                    .find(|c| c.name == "key")
                    .ok_or("a map entry without <key>")?;
                let value = item
                    .children
                    .iter()
                    .find(|c| c.name == "value")
                    .ok_or("a map entry without <value>")?;
                entries.push(DynamicData::Collection(alloc::vec![
                    read_value(&key_ty, key, depth + 1)?,
                    read_value(&value_ty, value, depth + 1)?,
                ]));
            }
            Ok(DynamicData::Collection(entries))
        }
        _ => {
            if let Some(child) = node.children.first() {
                return Err(format!(
                    "<{}> inside a value of `{}`, which has no members",
                    child.name,
                    ty.name()
                ));
            }
            read_scalar(&ty, &node.text)
        }
    }
}

/// zerodds-lint: recursion-depth 64 (`MAX_DEPTH`, checked first thing and
/// returned as an error)
fn read_items(
    element_ty: &DynamicType,
    node: &Node,
    dims: usize,
    depth: usize,
) -> Result<DynamicData, String> {
    if depth > MAX_DEPTH {
        return Err(String::from("the document nests deeper than it is read"));
    }
    let mut items = Vec::new();
    for item in &node.children {
        if item.name != "item" {
            return Err(format!("<{}> in a collection; <item> is due", item.name));
        }
        items.push(if dims > 1 {
            read_items(element_ty, item, dims - 1, depth + 1)?
        } else {
            read_value(element_ty, item, depth + 1)?
        });
    }
    Ok(DynamicData::Collection(items))
}

fn read_scalar(ty: &DynamicType, text: &str) -> Result<DynamicData, String> {
    let t = text.trim();
    let bad = || format!("`{t}` is no value of `{}`", ty.name());
    match ty.kind() {
        TypeKind::Boolean => match t {
            "true" | "1" => Ok(DynamicData::Boolean(true)),
            "false" | "0" => Ok(DynamicData::Boolean(false)),
            _ => Err(bad()),
        },
        TypeKind::Float32 | TypeKind::Float64 => {
            let v = match t {
                "NaN" => f64::NAN,
                "INF" => f64::INFINITY,
                "-INF" => f64::NEG_INFINITY,
                other => other.parse::<f64>().map_err(|_| bad())?,
            };
            Ok(DynamicData::float(
                if ty.kind() == TypeKind::Float32 {
                    P::Float32
                } else {
                    P::Float64
                },
                v,
            ))
        }
        TypeKind::Float128 => t
            .parse::<f64>()
            .map(|v| DynamicData::LongDouble(Float128::from_f64(v)))
            .map_err(|_| bad()),
        TypeKind::Char8 | TypeKind::Char16 => {
            let c = text.chars().next().ok_or_else(bad)?;
            Ok(DynamicData::character(
                if ty.kind() == TypeKind::Char8 {
                    P::Char
                } else {
                    P::WChar
                },
                c,
            ))
        }
        TypeKind::String8 | TypeKind::String16 => Ok(DynamicData::Text(text.to_string())),
        TypeKind::Enumeration => ty
            .members()
            .iter()
            .any(|m| m.name() == t)
            .then(|| DynamicData::Enumerator(t.to_string()))
            .ok_or_else(bad),
        TypeKind::Bitmask => {
            let known = ty.members();
            let mut flags = Vec::new();
            for name in t.split_whitespace() {
                if !known.iter().any(|m| m.name() == name) {
                    return Err(format!("`{name}` is no flag of `{}`", ty.name()));
                }
                flags.push(DynamicData::Text(name.to_string()));
            }
            flags.sort_by_key(|f| match f {
                DynamicData::Text(n) => known.iter().position(|m| m.name() == n),
                _ => None,
            });
            Ok(DynamicData::Bits(flags))
        }
        _ => read_integer(ty, text),
    }
}

fn read_integer(ty: &DynamicType, text: &str) -> Result<DynamicData, String> {
    let t = text.trim();
    let bad = || format!("`{t}` is no value of `{}`", ty.name());
    let primitive = ty.kind().as_primitive().ok_or_else(bad)?;
    let v = if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i128::from_str_radix(hex, 16).map_err(|_| bad())?
    } else {
        t.parse::<i128>().map_err(|_| bad())?
    };
    Ok(DynamicData::integer(primitive, v))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    const RICH_IDL: &str = "
        module m {
          enum Color { RED, GREEN };
          bitmask Flags { A, B, C };
          struct Inner { long n; string<8> tag; };
          union U switch (long) { case 1: long one; case 2: string two; };
          @final struct Rich {
            @key long id;
            string name;
            Color shade;
            Flags permissions;
            sequence<Inner> inners;
            long grid[2][2];
            map<string, long> table;
            U choice;
            @optional long maybe;
            boolean ok;
            double ratio;
            char c;
          };
        };";

    fn rich_support() -> alloc::sync::Arc<SerializedTypeSupport> {
        let ast = alloc::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(RICH_IDL, "rich.idl").unwrap(),
        );
        let ty = DynamicType::find(&ast, "m::Rich").unwrap();
        let object = zerodds_types::TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap();
        let dependencies: Vec<Vec<u8>> =
            zerodds_types::from_ast::dependency_type_objects(&ast, ty.id())
                .unwrap()
                .into_iter()
                .map(|object| {
                    zerodds_types::TypeObject::Complete(object)
                        .to_bytes_le()
                        .unwrap()
                })
                .collect();
        SerializedTypeSupport::from_type_objects("m::Rich", &object, &dependencies, true, 0)
            .unwrap()
    }

    fn rich_value() -> DynamicData {
        let inner = |n: i128, tag: &str| {
            DynamicData::Aggregate(alloc::vec![
                ("n".into(), DynamicData::integer(P::Int32, n)),
                ("tag".into(), DynamicData::Text(tag.into())),
            ])
        };
        let int = |v: i128| DynamicData::integer(P::Int32, v);
        DynamicData::Aggregate(alloc::vec![
            ("id".into(), int(7)),
            ("name".into(), DynamicData::Text("a <b> & c".into())),
            ("shade".into(), DynamicData::Enumerator("GREEN".into())),
            (
                "permissions".into(),
                DynamicData::Bits(alloc::vec![
                    DynamicData::Text("A".into()),
                    DynamicData::Text("C".into())
                ])
            ),
            (
                "inners".into(),
                DynamicData::Collection(alloc::vec![inner(1, "x"), inner(2, "y")])
            ),
            (
                "grid".into(),
                DynamicData::Collection(alloc::vec![
                    DynamicData::Collection(alloc::vec![int(1), int(2)]),
                    DynamicData::Collection(alloc::vec![int(3), int(4)]),
                ])
            ),
            (
                "table".into(),
                DynamicData::Collection(alloc::vec![DynamicData::Collection(alloc::vec![
                    DynamicData::Text("k".into()),
                    int(9)
                ])])
            ),
            (
                "choice".into(),
                DynamicData::Aggregate(alloc::vec![
                    ("_d".into(), int(2)),
                    ("two".into(), DynamicData::Text("deux".into())),
                ])
            ),
            ("maybe".into(), int(5)),
            ("ok".into(), DynamicData::Boolean(true)),
            ("ratio".into(), DynamicData::float(P::Float64, 0.5)),
            ("c".into(), DynamicData::character(P::Char, 'z')),
        ])
    }

    /// Every construct of the data model has its form in the document, and
    /// the document reads back as the value it was written from.
    #[test]
    fn a_value_is_written_as_its_document_and_read_back() {
        let support = rich_support();
        let value = rich_value();
        let xml = to_xml(&support, &value).unwrap();
        assert_eq!(
            xml,
            "<Rich><id>7</id><name>a &lt;b&gt; &amp; c</name><shade>GREEN</shade>\
             <permissions>A C</permissions>\
             <inners><item><n>1</n><tag>x</tag></item><item><n>2</n><tag>y</tag></item></inners>\
             <grid><item><item>1</item><item>2</item></item><item><item>3</item><item>4</item></item></grid>\
             <table><item><key>k</key><value>9</value></item></table>\
             <choice><discriminator>2</discriminator><two>deux</two></choice>\
             <maybe>5</maybe><ok>true</ok><ratio>0.5</ratio><c>z</c></Rich>"
        );
        assert_eq!(from_xml(&support, &xml).unwrap(), value);
        // The Valid variant: the implied namespace, spelled out, with a prefix.
        let namespaced = xml
            .replacen("<Rich>", "<m:Rich xmlns:m=\"ddstype://www.omg.org/m\">", 1)
            .replacen("</Rich>", "</m:Rich>", 1);
        assert_eq!(from_xml(&support, &namespaced).unwrap(), value);
    }

    /// An optional member without a value is left out, and the document
    /// without it is a document of the type.
    #[test]
    fn an_absent_optional_member_is_not_in_the_document() {
        let support = rich_support();
        let mut value = rich_value();
        if let DynamicData::Aggregate(entries) = &mut value {
            entries.retain(|(n, _)| n != "maybe");
        }
        let xml = to_xml(&support, &value).unwrap();
        assert!(!xml.contains("maybe"));
        assert_eq!(from_xml(&support, &xml).unwrap(), value);
    }

    /// The path of a sample on the wire: the CDR body decodes to a value,
    /// the value is the document, and the document is the CDR body again.
    #[test]
    fn the_document_of_a_cdr_body_is_that_body_again() {
        let support = rich_support();
        let sample = support.from_dynamic_data(rich_value()).unwrap();
        let xml = to_xml(&support, sample.data()).unwrap();
        let back = support
            .from_dynamic_data(from_xml(&support, &xml).unwrap())
            .unwrap();
        assert_eq!(back.body(), sample.body());
    }

    /// An instance key is the document of the `@key` members.
    #[test]
    fn the_key_document_holds_the_key_members_only() {
        let support = rich_support();
        assert_eq!(
            to_xml_key(&support, &rich_value()).unwrap(),
            "<Rich><id>7</id></Rich>"
        );
    }

    #[test]
    fn a_document_that_is_none_of_the_types_is_refused() {
        let support = rich_support();
        for xml in [
            "<Other><id>1</id></Other>",
            "<Rich><id>x</id></Rich>",
            "<Rich><nope>1</nope></Rich>",
            "<Rich><id>1</id><id>2</id></Rich>",
            "<Rich><shade>BLUE</shade></Rich>",
            "<Rich><permissions>Z</permissions></Rich>",
            "<Rich><inners><x/></inners></Rich>",
            "<Rich><choice><two>a</two></choice></Rich>",
            "<Rich><ok>maybe</ok></Rich>",
        ] {
            assert!(from_xml(&support, xml).is_err(), "`{xml}` must be refused");
        }
    }

    /// A document from the wire cannot nest deeper than the reader follows.
    #[test]
    fn a_document_nested_past_the_limit_is_refused() {
        let deep = format!(
            "{}{}",
            "<a>".repeat(MAX_DEPTH + 8),
            "</a>".repeat(MAX_DEPTH + 8)
        );
        assert!(parse_document(&deep).is_err());
        let fine = format!("{}{}", "<a>".repeat(8), "</a>".repeat(8));
        assert!(parse_document(&fine).is_ok());
    }

    #[test]
    fn entities_and_prefixes_are_read() {
        let node = parse_document("<?xml version=\"1.0\"?><my:A xmlns:my=\"ddstype://www.omg.org/M\"><my:b>x &amp; &lt;y&gt; &#65;</my:b><!-- c --></my:A>").unwrap();
        assert_eq!(node.name, "A");
        assert_eq!(node.children[0].name, "b");
        assert_eq!(node.children[0].text, "x & <y> A");
        assert!(parse_document("<a><b></a>").is_err());
        assert!(parse_document("<a/>x").is_err());
        assert!(parse_document("<a>&nope;</a>").is_err());
    }
}
