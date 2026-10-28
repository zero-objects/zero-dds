// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The D that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-go-gen/src/dump.rs`: one
//! function per type, keyed the way the decoder is keyed, answering a
//! `std.json` `JSONValue`. The dialect is the shared one — a 64-bit
//! integer is a string, an absent `@optional` member is a key left out
//! — and it is not canonical form: the caller canonicalises in Rust.
//!
//! A typedef's tag follows the alias, so for most targets the target's
//! own body answers to the typedef's name; a typedef of a primitive is
//! the one case the two names differ — the primitive's tag carries
//! `p_` — and gets a forwarding body. A refusal — a float with no JSON
//! form, an enumerator value with no name — throws, and the harness
//! turns it into the sample's refusal.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::d::field_name;
use zerodds_lang_rules::emit::d::D;
use zerodds_lang_rules::emit::decode::decoder_name;

use crate::fill::d_string_literal;

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<D>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The D source of one dumper per type reachable from `root`.
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
            let target = match aliased.resolved() {
                Some(target) => target,
                None => return Err(format!("{name}: the typedef target is unresolved")),
            };
            let target_name = dumper_name(ast, target);
            if target_name == name {
                continue;
            }
            let parameter = parameter_type(ast, id, &name)?;
            emitted.push(name.clone());
            let _ = write!(
                out,
                "\nJSONValue {name}({parameter} v, string path)\n{{\n    return {target_name}(v, path);\n}}\n"
            );
            continue;
        }
        emitted.push(name);
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
}

/// The D spelling of the dumper's parameter.
fn parameter_type(ast: &Ast, id: Id<TypeNode>, name: &str) -> Result<String, String> {
    match zerodds_lang_rules::adapter::d::d_value_type(ast, id) {
        Ok(type_) => Ok(zerodds_lang_render::d::type_(&type_)),
        Err(error) => Err(format!("{name}: the type has no D form: {error:?}")),
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

/// One dumper, as a whole function. Four spaces, Allman braces — the
/// generated file's own style.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let parameter = parameter_type(ast, id, &name)?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if (!zdwire.JSONNumberOK(cast(double) v))".to_string());
            body.push("{".to_string());
            body.push(
                "    throw new Exception(path ~ \": the value has no JSON form\");".to_string(),
            );
            body.push("}".to_string());
            body.push("return JSONValue(v);".to_string());
        }
        // The exact decimal, which is what the member holds.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return JSONValue(v);".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value.
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("return JSONValue(zdwire.FormatI64(v));".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::UInt64) => {
            body.push("return JSONValue(zdwire.FormatU64(v));".to_string());
        }
        // The one-character string the dialect writes for a character.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("return JSONValue(to!string(cast(dchar) v));".to_string());
        }
        TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return JSONValue(v);".to_string());
        }
        // The enumerator's name, chosen by the value; a value with no
        // enumerator has no name.
        TypeKind::Enumeration { .. } => {
            body.push("switch (cast(int) v)".to_string());
            body.push("{".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let Some(value) = ast.effective_enumerator_value(*enumerator) else {
                    continue;
                };
                body.push(format!("case {value}:"));
                body.push(format!(
                    "    return JSONValue({});",
                    d_string_literal(&node.name)
                ));
            }
            body.push("default:".to_string());
            body.push(
                "    throw new Exception(path ~ \": the value names no enumerator of this type\");"
                    .to_string(),
            );
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
            body.push("const ulong zd_bits = v;".to_string());
            body.push(format!("if ((zd_bits & ~{accounted:#x}UL) != 0)"));
            body.push("{".to_string());
            body.push(
                "    throw new Exception(path ~ \": a bit this bitmask does not declare is set\");"
                    .to_string(),
            );
            body.push("}".to_string());
            body.push("JSONValue[] zd_out;".to_string());
            for (bit, position) in &declared {
                body.push(format!("if ((zd_bits >> {position} & 1) == 1)"));
                body.push("{".to_string());
                body.push(format!(
                    "    zd_out ~= JSONValue({});",
                    d_string_literal(bit)
                ));
                body.push("}".to_string());
            }
            body.push("return JSONValue(zd_out);".to_string());
        }
        // The whole list, base first.
        TypeKind::Bitset { .. } => {
            body.push("JSONValue[string] zd_out;".to_string());
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                body.push(format!(
                    "zd_out[{}] = JSONValue(v.{});",
                    d_string_literal(name),
                    field_name(name)
                ));
            }
            body.push("return JSONValue(zd_out);".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("JSONValue[] zd_out;".to_string());
            body.push("foreach (zd_i; 0 .. v.length)".to_string());
            body.push("{".to_string());
            body.push(format!(
                "    zd_out ~= {}(v[zd_i], path ~ \"[\" ~ to!string(zd_i) ~ \"]\");",
                dumper_name(ast, element)
            ));
            body.push("}".to_string());
            body.push("return JSONValue(zd_out);".to_string());
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
                body.push(format!("{indent}JSONValue[] zd_out{level};"));
                body.push(format!(
                    "{indent}foreach (zd_i{level}; 0 .. {access}.length)"
                ));
                body.push(format!("{indent}{{"));
                access.push_str(&format!("[zd_i{level}]"));
            }
            let deepest = "    ".repeat(levels);
            let innermost = levels - 1;
            body.push(format!(
                "{deepest}zd_out{innermost} ~= {}({access}, path);",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}}}"));
                if level > 0 {
                    let outer = level - 1;
                    body.push(format!(
                        "{indent}zd_out{outer} ~= JSONValue(zd_out{level});"
                    ));
                }
            }
            body.push("return JSONValue(zd_out0);".to_string());
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
            body.push("JSONValue[] zd_out;".to_string());
            body.push("foreach (zd_i; 0 .. v.length)".to_string());
            body.push("{".to_string());
            body.push("    const zd_where = path ~ \"[\" ~ to!string(zd_i) ~ \"]\";".to_string());
            body.push(format!(
                "    zd_out ~= JSONValue([{}(v[zd_i].key, zd_where ~ \".key\"), \
                 {}(v[zd_i].value, zd_where ~ \".value\")]);",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
            body.push("}".to_string());
            body.push("return JSONValue(zd_out);".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("JSONValue[string] zd_out;".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<zerodds_lang_rules::emit::d::D>(
                    ast,
                    id,
                    member.node_id,
                    &member.name,
                );
                let key = d_string_literal(&member.name);
                let where_ = format!("path ~ {}", d_string_literal(&format!(".{}", member.name)));
                let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                if external {
                    // An absent key only where the member may be
                    // absent; a non-optional external pointer is always
                    // set by Table 9 or by the decoder.
                    body.push(format!("if (v.{field} !is null)"));
                    body.push("{".to_string());
                    body.push(format!(
                        "    zd_out[{key}] = {}(*v.{field}, {where_});",
                        dumper_name(ast, member_type)
                    ));
                    body.push("}".to_string());
                } else if member.facets.is_optional() {
                    body.push(format!("if (!v.{field}.isNull)"));
                    body.push("{".to_string());
                    body.push(format!(
                        "    zd_out[{key}] = {}(v.{field}.get, {where_});",
                        dumper_name(ast, member_type)
                    ));
                    body.push("}".to_string());
                } else {
                    body.push(format!(
                        "zd_out[{key}] = {}(v.{field}, {where_});",
                        dumper_name(ast, member_type)
                    ));
                }
            }
            body.push("return JSONValue(zd_out);".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("JSONValue[string] zd_out;".to_string());
            body.push(format!(
                "zd_out[\"_d\"] = {}(v._d, path ~ \"._d\");",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch the value holds: the branches overlay.
            let suppress_negative = union_negative_matches_nothing(ast, id, union_label);
            let mut arms: Vec<(String, String, Id<TypeNode>)> = Vec::new();
            let mut fallback: Option<(String, Id<TypeNode>)> = None;
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
                            tests.push(format!("v._d == {}", d_label(ast, discriminator, value)));
                        }
                    }
                }
                if is_default {
                    fallback = Some((member.name.clone(), member_type));
                }
                if !tests.is_empty() {
                    arms.push((tests.join(" || "), member.name.clone(), member_type));
                }
            }
            let branch = |head: Option<String>,
                          member_name: &str,
                          member_type: Id<TypeNode>,
                          body: &mut Vec<String>| {
                let field = field_name(member_name);
                let key = d_string_literal(member_name);
                let where_ = format!("path ~ {}", d_string_literal(&format!(".{member_name}")));
                // An `@external` branch is a `T*`, dereferenced as an
                // `@external` struct member is: the selected branch is
                // always set by the decoder. Passing the pointer did not
                // compile ("cannot pass argument `v.nested` of type
                // `Inner*` to parameter `Inner v`").
                let external = ast
                    .cases_of(id)
                    .iter()
                    .filter_map(|case| ast.case(*case).ok())
                    .filter_map(|case| ast.member(case.member).ok())
                    .find(|member| member.name == member_name)
                    .is_some_and(|member| {
                        zerodds_lang_rules::adapter::held_externally(ast, member)
                    });
                let value = if external {
                    format!("*v.{field}")
                } else {
                    format!("v.{field}")
                };
                let call = format!("{}({value}, {where_})", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push("{".to_string());
                        body.push(format!("    zd_out[{key}] = {call};"));
                        body.push("}".to_string());
                    }
                    None => body.push(format!("zd_out[{key}] = {call};")),
                }
            };
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 {
                    format!("if ({condition})")
                } else {
                    format!("else if ({condition})")
                };
                branch(Some(keyword), member_name, *member_type, &mut body);
            }
            if let Some((member_name, member_type)) = fallback {
                if arms.is_empty() {
                    branch(None, &member_name, member_type, &mut body);
                } else {
                    branch(
                        Some("else".to_string()),
                        &member_name,
                        member_type,
                        &mut body,
                    );
                }
            }
            body.push("return JSONValue(zd_out);".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no D shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\nJSONValue {name}({parameter} v, string path)\n{{\n");
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    out.push_str("}\n");
    Ok(out)
}

/// A union label in the discriminator's D form: `true`/`false` for a
/// `boolean`, and the number for everything else — a `char` and a
/// `wchar` discriminator hold their code unit as an integer here.
fn d_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_primitive(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => if value == 0 { "false" } else { "true" }.to_string(),
        _ => format!("{value}"),
    }
}

/// The type `id` is once the typedefs are seen through.
///
/// zerodds-lint: recursion-depth 64 (typedef chains only —
/// every other kind answers without a descent, so the walk never
/// reaches an aggregate and cannot cycle; bounded by [`MAX_DEPTH`])
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
