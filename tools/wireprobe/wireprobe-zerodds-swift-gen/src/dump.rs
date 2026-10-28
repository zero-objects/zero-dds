// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Swift that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-d-gen/src/dump.rs`: one
//! function per type, keyed the way the decoder is keyed, answering
//! the JSON **text** directly — Swift's own `JSONSerialization` orders
//! nothing and buys nothing here. The dialect is the shared one — a
//! 64-bit integer is a string, an absent `@optional` member is a key
//! left out — and it is not canonical form: the caller canonicalises
//! in Rust.
//!
//! A typedef's tag follows the alias, so for most targets the target's
//! own body answers to the typedef's name; a typedef of a primitive is
//! the one case the two names differ — the primitive's tag carries
//! `p_` — and gets a forwarding body. A refusal — a float with no JSON
//! form, a wide character that is no code point — throws a
//! `ZdWireError`, the sample's own refusal; the harness this file's
//! caller writes catches it and is the one that turns it into exit 1.
//! Every dumper is `throws` for the same reason every generated codec
//! function is: it may call another dumper that refuses.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::swift::{property_name, swift_type_name, swift_value_type};
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::swift::Swift;

use crate::fill::swift_string_literal;

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Swift>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The Swift source of one dumper per type reachable from `root`.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n// One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         // documents. Not canonical form: the caller canonicalises in Rust.\n",
    );
    let mut emitted: Vec<String> = Vec::new();
    for id in reachable {
        let name = dumper_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        if let Ok(TypeKind::Alias { aliased }) = ast.type_(id).map(|node| node.kind.clone()) {
            let Some(target) = aliased.resolved() else {
                return Err(format!("{name}: the typedef target is unresolved"));
            };
            let target_name = dumper_name(ast, target);
            if target_name == name {
                continue;
            }
            let parameter = parameter_type(ast, id, &name)?;
            emitted.push(name.clone());
            let _ = write!(
                out,
                "\nfunc {name}(_ v: {parameter}, _ path: String) throws -> String {{\n    \
                 return try {target_name}(v, path)\n}}\n"
            );
            continue;
        }
        emitted.push(name);
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
}

/// The Swift spelling of the dumper's parameter.
fn parameter_type(ast: &Ast, id: Id<TypeNode>, name: &str) -> Result<String, String> {
    match swift_value_type(ast, id) {
        Ok(type_) => Ok(zerodds_lang_render::swift::type_(&type_)),
        Err(error) => Err(format!("{name}: the type has no Swift form: {error:?}")),
    }
}

/// Every type a value of `id` can hold, `id` included.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_DEPTH`)
fn collect(ast: &Ast, id: Id<TypeNode>, out: &mut Vec<Id<TypeNode>>, depth: usize) {
    if depth > MAX_DEPTH || out.contains(&id) {
        return;
    }
    let Ok(node) = ast.type_(id) else {
        return;
    };
    out.push(id);
    let inner = |half: Option<Id<TypeNode>>, out: &mut Vec<Id<TypeNode>>| {
        if let Some(target) = half {
            collect(ast, target, out, depth + 1);
        }
    };
    match &node.kind {
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            inner(element.resolved(), out);
        }
        TypeKind::Map { key, value, .. } => {
            inner(key.resolved(), out);
            inner(value.resolved(), out);
        }
        TypeKind::Alias { aliased } => inner(aliased.resolved(), out),
        TypeKind::ValueBox { boxed } => inner(boxed.resolved(), out),
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            for member in ast.all_members_of(id) {
                if let Ok(member) = ast.member(member) {
                    inner(member.type_.resolved(), out);
                }
            }
        }
        TypeKind::Union { discriminator, .. } => {
            inner(discriminator.resolved(), out);
            for case in ast.cases_of(id) {
                let Ok(case) = ast.case(*case) else {
                    continue;
                };
                if let Ok(member) = ast.member(case.member) {
                    inner(member.type_.resolved(), out);
                }
            }
        }
        _ => {}
    }
}

/// The Swift string literal whose contents are the JSON key `name`
/// followed by the colon: `"\"a\":"`.
fn json_key(name: &str) -> String {
    swift_string_literal(&format!("\"{name}\":"))
}

/// One dumper, as a whole function. Four spaces, the generated file's
/// own style.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let parameter = parameter_type(ast, id, &name)?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if !zdJsonNumberOK(Double(v)) {".to_string());
            body.push(
                "    throw ZdWireError.outOfRange(at: 0, expected: path + \": the value has \
                 no JSON form\")"
                    .to_string(),
            );
            body.push("}".to_string());
            body.push("return String(v)".to_string());
        }
        // The exact decimal, quoted: the member holds it as text.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return zdJsonString(v)".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value.
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            body.push("return zdJsonString(String(v))".to_string());
        }
        // The one-character string the dialect writes for a character.
        TypeKind::Primitive(PrimitiveKind::Char) => {
            body.push("return zdJsonString(String(UnicodeScalar(v)))".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::WChar) => {
            body.push("guard let zd_scalar = UnicodeScalar(v) else {".to_string());
            body.push(
                "    throw ZdWireError.outOfRange(at: 0, expected: path + \": the value is \
                 no code point\")"
                    .to_string(),
            );
            body.push("}".to_string());
            body.push("return zdJsonString(String(zd_scalar))".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("return v ? \"true\" : \"false\"".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("return String(v)".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return zdJsonString(v)".to_string());
        }
        // The case's name, chosen exhaustively: a Swift enum with a
        // raw type cannot hold an undeclared value at all.
        TypeKind::Enumeration { .. } => {
            body.push("switch v {".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                body.push(format!("case .{}:", property_name(&node.name)));
                body.push(format!(
                    "    return {}",
                    swift_string_literal(&format!("\"{}\"", node.name))
                ));
            }
            body.push("}".to_string());
        }
        // The set bits' names in declaration order; a bit set inside
        // the bound that the type does not declare is refused.
        TypeKind::Bitmask { .. } => {
            let bound = ast.effective_bit_bound(id);
            let mut accounted: u64 = 0;
            let mut declared: Vec<(String, u16)> = Vec::new();
            for bit in ast.bits_of(id) {
                let Ok(value) = ast.bit_value(*bit) else {
                    continue;
                };
                let Some(position) = ast.effective_bit_position(*bit) else {
                    continue;
                };
                if position >= bound {
                    continue;
                }
                let Some(mask) = 1u64.checked_shl(u32::from(position)) else {
                    continue;
                };
                accounted |= mask;
                declared.push((value.name.clone(), position));
            }
            body.push("let zd_bits = UInt64(v.rawValue)".to_string());
            body.push(format!("if (zd_bits & ~UInt64({accounted:#x})) != 0 {{"));
            body.push(
                "    throw ZdWireError.outOfRange(at: 0, expected: path + \": a bit this \
                 bitmask does not declare is set\")"
                    .to_string(),
            );
            body.push("}".to_string());
            body.push("var zd_out: [String] = []".to_string());
            for (bit, position) in &declared {
                body.push(format!("if (zd_bits >> {position}) & 1 == 1 {{"));
                body.push(format!(
                    "    zd_out.append({})",
                    swift_string_literal(&format!("\"{bit}\""))
                ));
                body.push("}".to_string());
            }
            body.push("return \"[\" + zd_out.joined(separator: \",\") + \"]\"".to_string());
        }
        // The whole list, base first.
        TypeKind::Bitset { .. } => {
            body.push("var zd_out: [String] = []".to_string());
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field.name.as_ref() else {
                    continue;
                };
                body.push(format!(
                    "zd_out.append({} + String(v.{}))",
                    json_key(field_name),
                    property_name(field_name)
                ));
            }
            body.push("return \"{\" + zd_out.joined(separator: \",\") + \"}\"".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("var zd_out: [String] = []".to_string());
            body.push("for zd_i in 0..<v.count {".to_string());
            body.push(format!(
                "    zd_out.append(try {}(v[zd_i], path + \"[\" + String(zd_i) + \"]\"))",
                dumper_name(ast, element)
            ));
            body.push("}".to_string());
            body.push("return \"[\" + zd_out.joined(separator: \",\") + \"]\"".to_string());
        }
        // One loop per dimension; the path is not indexed, as in the
        // sibling dumpers.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let levels = dims.len();
            let mut access = String::from("v");
            for level in 0..levels {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}var zd_out{level}: [String] = []"));
                body.push(format!("{indent}for zd_i{level} in 0..<{access}.count {{"));
                access.push_str(&format!("[zd_i{level}]"));
            }
            let deepest = "    ".repeat(levels);
            let innermost = levels - 1;
            body.push(format!(
                "{deepest}zd_out{innermost}.append(try {}({access}, path))",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}}}"));
                if level > 0 {
                    let outer = level - 1;
                    body.push(format!(
                        "{indent}zd_out{outer}.append(\"[\" + zd_out{level}.joined(separator: \
                         \",\") + \"]\")"
                    ));
                }
            }
            body.push("return \"[\" + zd_out0.joined(separator: \",\") + \"]\"".to_string());
        }
        // An array of two-element arrays, because a JSON object keys on
        // a string and a map key need not be one.
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push("var zd_out: [String] = []".to_string());
            body.push("for zd_i in 0..<v.count {".to_string());
            body.push("    let zd_where = path + \"[\" + String(zd_i) + \"]\"".to_string());
            body.push(format!(
                "    zd_out.append(try \"[\" + {}(v[zd_i].key, zd_where + \".key\") + \",\" + \
                 {}(v[zd_i].value, zd_where + \".value\") + \"]\")",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
            body.push("}".to_string());
            body.push("return \"[\" + zd_out.joined(separator: \",\") + \"]\"".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("var zd_out: [String] = []".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::swift::Swift,
                >(ast, id, member.node_id, &member.name);
                let key = json_key(&member.name);
                let where_ = format!(
                    "path + {}",
                    swift_string_literal(&format!(".{}", member.name))
                );
                let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                let optional = member.facets.is_optional();
                if optional {
                    body.push(format!("if let zd_m = v.{field} {{"));
                    let held = if external { "zd_m.value" } else { "zd_m" };
                    body.push(format!(
                        "    zd_out.append(try {key} + {}({held}, {where_}))",
                        dumper_name(ast, member_type)
                    ));
                    body.push("}".to_string());
                } else if external {
                    body.push(format!(
                        "zd_out.append(try {key} + {}(v.{field}.value, {where_}))",
                        dumper_name(ast, member_type)
                    ));
                } else {
                    body.push(format!(
                        "zd_out.append(try {key} + {}(v.{field}, {where_}))",
                        dumper_name(ast, member_type)
                    ));
                }
            }
            body.push("return \"{\" + zd_out.joined(separator: \",\") + \"}\"".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("var zd_out: [String] = []".to_string());
            body.push(format!(
                "zd_out.append(try {} + {}(v._d, path + \"._d\"))",
                json_key("_d"),
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch the value holds.
            let suppress_negative = union_negative_matches_nothing(ast, id, union_label);
            let mut arms: Vec<(String, String, Id<TypeNode>, bool)> = Vec::new();
            let mut fallback: Option<(String, Id<TypeNode>, bool)> = None;
            for case in ast.cases_of(id) {
                let Ok(case) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case.member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the case type is unresolved", member.name)
                })?;
                let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                let mut tests: Vec<String> = Vec::new();
                let mut is_default = false;
                for label in &case.labels {
                    match label.expr() {
                        None => is_default = true,
                        Some(value_expr) => {
                            let value = ast.eval_integral(value_expr).map_err(|error| {
                                format!("{name}: a case label does not evaluate: {error}")
                            })?;
                            if suppress_negative && value < 0 {
                                continue;
                            }
                            tests.push(format!(
                                "v._d == {}",
                                swift_label(ast, discriminator, value)?
                            ));
                        }
                    }
                }
                if is_default {
                    fallback = Some((member.name.clone(), member_type, external));
                }
                if !tests.is_empty() {
                    arms.push((
                        tests.join(" || "),
                        member.name.clone(),
                        member_type,
                        external,
                    ));
                }
            }
            let branch = |head: Option<String>,
                          member_name: &str,
                          member_type: Id<TypeNode>,
                          external: bool,
                          body: &mut Vec<String>| {
                let field = property_name(member_name);
                let key = json_key(member_name);
                let where_ = format!(
                    "path + {}",
                    swift_string_literal(&format!(".{member_name}"))
                );
                let held = if external {
                    format!("v.{field}!.value")
                } else {
                    format!("v.{field}!")
                };
                let call = format!("{}({held}, {where_})", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push(format!("    zd_out.append(try {key} + {call})"));
                        body.push("}".to_string());
                    }
                    None => body.push(format!("zd_out.append(try {key} + {call})")),
                }
            };
            for (index, (condition, member_name, member_type, external)) in arms.iter().enumerate()
            {
                let keyword = if index == 0 {
                    format!("if {condition} {{")
                } else {
                    format!("}} else if {condition} {{")
                };
                if index > 0 {
                    // The previous arm's close brace is replaced by the
                    // chained `} else if`.
                    body.pop();
                }
                branch(
                    Some(keyword),
                    member_name,
                    *member_type,
                    *external,
                    &mut body,
                );
            }
            if let Some((member_name, member_type, external)) = fallback {
                if arms.is_empty() {
                    branch(None, &member_name, member_type, external, &mut body);
                } else {
                    body.pop();
                    branch(
                        Some("} else {".to_string()),
                        &member_name,
                        member_type,
                        external,
                        &mut body,
                    );
                }
            }
            body.push("return \"{\" + zd_out.joined(separator: \",\") + \"}\"".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Swift shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\nfunc {name}(_ v: {parameter}, _ path: String) throws -> String {{\n");
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    out.push_str("}\n");
    Ok(out)
}

/// A union label in the discriminator's Swift form: `true`/`false` for
/// a `boolean`, the enumerator for an enumeration — Swift has no
/// conversion between an integer and a raw enum to lean on — and the
/// bare number for everything else, typed by `v._d` beside it.
fn swift_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Result<String, String> {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        for enumerator in ast.enumerators_of(resolved) {
            let Ok(node) = ast.enumerator(*enumerator) else {
                continue;
            };
            if ast.effective_enumerator_value(*enumerator) == Some(value) {
                return Ok(format!(
                    "{}.{}",
                    swift_type_name(ast, resolved),
                    property_name(&node.name)
                ));
            }
        }
        return Err(format!(
            "a case label of {value} names no enumerator of the discriminator"
        ));
    }
    match resolve_primitive(ast, resolved, 0) {
        Some(PrimitiveKind::Boolean) => Ok(if value == 0 { "false" } else { "true" }.to_string()),
        _ => Ok(format!("{value}")),
    }
}

fn resolve_alias(ast: &Ast, id: Id<TypeNode>) -> Id<TypeNode> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(target) => current = target,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_DEPTH`)
fn resolve_primitive(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<PrimitiveKind> {
    if depth > MAX_DEPTH {
        return None;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => Some(kind),
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .and_then(|target| resolve_primitive(ast, target, depth + 1)),
        _ => None,
    }
}
