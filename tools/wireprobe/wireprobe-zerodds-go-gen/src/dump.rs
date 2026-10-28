// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Go that turns one decoded sample back into JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into
//! the generated Go. This takes one back out, after `runtime/go` and
//! the generated decoder have filled it from the bytes.
//!
//! The dialect is the shared one `crates/wire-probe/src/lib.rs`
//! documents, and two of its rules cannot be recovered by any reader:
//! a 64-bit integer is a **string**, and an absent `@optional` member
//! is a key that is **left out**, never a `null`. It is not canonical
//! form — the caller canonicalises in Rust, so a second canonicaliser
//! cannot part from the first over a spelling.
//!
//! One function per type, keyed the way the decoder is keyed, and Go
//! resolves a package-level name wherever it is declared, so a
//! recursive type needs nothing extra. A typedef's tag follows the
//! alias, so for most targets the target's own body already answers to
//! the typedef's name; a typedef of a primitive is the one case the two
//! names differ — the primitive's tag carries `p_` — and gets a
//! forwarding body. The struct-key maps a Go `map[string]any` would
//! randomise are marshalled by `encoding/json`, which sorts map keys,
//! so the document is stable.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::go::field_name;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::go::Go;

use crate::fill::go_string_literal;

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed, so the two are keyed identically.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Go>(ast, id);
    // The prefix through the same transform `decoder_name` published
    // the name under, rather than written out: the Go column carries
    // its visibility in the first letter of the name, so a literal
    // `"zd_dec_"` here would stop matching the day that letter moved
    // and the dumper would be called `zd_json_Zd_dec_T`.
    let prefix = zerodds_lang_rules::emit::go::exported_name("zd_dec_");
    match decoder.strip_prefix(&prefix) {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The Go source of one dumper per type reachable from `root`.
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
        // Two structurally equal collections share one tag: the first
        // body emitted under a name is the body, and the bodies a
        // shared tag describes are identical by construction.
        let name = dumper_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        // A typedef's own dumper name follows the alias chain, so for a
        // typedef of an aggregate or a collection the target's body
        // already answers to it and a second one would be a duplicate
        // function. A typedef of a *primitive* is the one case the two
        // names differ — the primitive's own tag carries `p_` — so a
        // forwarding body carries the alias's name to the target's.
        // Measured: `typedef-chain-primitive` compiled to a call on a
        // `zd_json_long` nothing declared.
        if let Ok(TypeKind::Alias { aliased }) = ast.type_(id).map(|node| node.kind.clone()) {
            let target = match aliased.resolved() {
                Some(target) => target,
                None => return Err(format!("{name}: the typedef target is unresolved")),
            };
            let target_name = dumper_name(ast, target);
            if target_name == name {
                continue;
            }
            let parameter = match zerodds_lang_rules::adapter::go::go_value_type(ast, id) {
                Ok(type_) => zerodds_lang_render::go::type_(&type_),
                Err(error) => {
                    return Err(format!("{name}: the typedef has no Go form: {error:?}"));
                }
            };
            emitted.push(name.clone());
            out.push_str(&format!(
                "\nfunc {name}(v {parameter}, path string) any {{\n\treturn {target_name}(v, path)\n}}\n"
            ));
            continue;
        }
        emitted.push(name);
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
}

/// Every type a value of `id` can hold, `id` included — the type graph
/// and not the value, because a dumper has to exist for a sequence's
/// element whether the sample held one or none.
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

/// One dumper, as a whole `func`. The parameter type is the
/// materialiser's, so the signature and the decoder agree.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let parameter = match zerodds_lang_rules::adapter::go::go_value_type(ast, id) {
        Ok(type_) => zerodds_lang_render::go::type_(&type_),
        Err(error) => return Err(format!("{name}: the type has no Go form: {error:?}")),
    };
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        // A JSON number JSON can write, or a refusal naming the path.
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if !zdwire.JSONNumberOK(float64(v)) {".to_string());
            body.push("\tpanic(path + \": the value has no JSON form\")".to_string());
            body.push("}".to_string());
            body.push("return v".to_string());
        }
        // The exact decimal, which is what the member holds.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return v".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value: a form that changed shape at 2^53 would compare
        // unequal across the boundary.
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("return zdwire.FormatI64(v)".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::UInt64) => {
            body.push("return zdwire.FormatU64(v)".to_string());
        }
        // The one-character string the dialect writes for a character.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("return string(rune(v))".to_string());
        }
        TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return v".to_string());
        }
        // The enumerator's name, chosen by the value: a value with no
        // enumerator has no name, and handing the number back would put
        // an integer where every other column writes a string.
        TypeKind::Enumeration { .. } => {
            let mut pairs = Vec::new();
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let Some(value) = ast.effective_enumerator_value(*enumerator) else {
                    continue;
                };
                pairs.push(format!("{value}: {}", go_string_literal(&node.name)));
            }
            body.push(format!(
                "zd_names := map[int32]string{{{}}}",
                pairs.join(", ")
            ));
            body.push("zd_found, zd_ok := zd_names[int32(v)]".to_string());
            body.push("if !zd_ok {".to_string());
            body.push(
                "\tpanic(path + \": the value names no enumerator of this type\")".to_string(),
            );
            body.push("}".to_string());
            body.push("return zd_found".to_string());
        }
        // The set bits' names in declaration order; a bit set inside
        // the bound that the type does not declare is refused rather
        // than dropped.
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
            body.push("zd_bits := uint64(v)".to_string());
            body.push(format!("if zd_bits & ^uint64({accounted:#x}) != 0 {{"));
            body.push(
                "\tpanic(path + \": a bit this bitmask does not declare is set\")".to_string(),
            );
            body.push("}".to_string());
            body.push("zd_out := []any{}".to_string());
            for (bit, position) in &declared {
                body.push(format!("if zd_bits>>{position}&1 == 1 {{"));
                body.push(format!(
                    "\tzd_out = append(zd_out, {})",
                    go_string_literal(bit)
                ));
                body.push("}".to_string());
            }
            body.push("return zd_out".to_string());
        }
        // The whole list, base first: a derived bitset carries the
        // base's bitfields and the decoder fills them.
        TypeKind::Bitset { .. } => {
            let mut entries = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                entries.push(format!(
                    "{}: v.{}",
                    go_string_literal(name),
                    field_name(name)
                ));
            }
            body.push(format!("return map[string]any{{{}}}", entries.join(", ")));
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("zd_out := []any{}".to_string());
            body.push("for zd_i := 0; zd_i < len(v); zd_i++ {".to_string());
            body.push(format!(
                "\tzd_out = append(zd_out, {}(v[zd_i], path+\"[\"+zdwire.Itoa(zd_i)+\"]\"))",
                dumper_name(ast, element)
            ));
            body.push("}".to_string());
            body.push("return zd_out".to_string());
        }
        // One loop per dimension. §7.4.3.5 treats a multi-dimensional
        // array as one flat run with the first index varying most
        // slowly, and nested JSON arrays in written order are that. The
        // path is not indexed, as in the sibling dumpers: an IDL array
        // has a fixed length and every element is present.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let levels = dims.len();
            let mut access = String::from("v");
            for level in 0..levels {
                let indent = "\t".repeat(level);
                body.push(format!("{indent}zd_out{level} := []any{{}}"));
                body.push(format!(
                    "{indent}for zd_i{level} := 0; zd_i{level} < len({access}); zd_i{level}++ {{"
                ));
                access.push_str(&format!("[zd_i{level}]"));
            }
            let deepest = "\t".repeat(levels);
            let innermost = levels - 1;
            body.push(format!(
                "{deepest}zd_out{innermost} = append(zd_out{innermost}, {}({access}, path))",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "\t".repeat(level);
                body.push(format!("{indent}}}"));
                // A closed row is appended into its enclosing
                // dimension, in that dimension's own loop body.
                if level > 0 {
                    let outer = level - 1;
                    body.push(format!(
                        "{indent}zd_out{outer} = append(zd_out{outer}, zd_out{level})"
                    ));
                }
            }
            body.push("return zd_out0".to_string());
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
            body.push("zd_out := []any{}".to_string());
            body.push("for zd_i := 0; zd_i < len(v); zd_i++ {".to_string());
            body.push("\tzd_where := path + \"[\" + zdwire.Itoa(zd_i) + \"]\"".to_string());
            body.push(format!(
                "\tzd_out = append(zd_out, []any{{{}(v[zd_i].Key, zd_where+\".key\"), \
                 {}(v[zd_i].Value, zd_where+\".value\")}})",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
            body.push("}".to_string());
            body.push("return zd_out".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("zd_out := map[string]any{}".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `Parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::go::Go,
                >(ast, id, member.node_id, &member.name);
                let key = go_string_literal(&member.name);
                let where_ = format!("path+{}", go_string_literal(&format!(".{}", member.name)));
                if member.facets.is_optional() {
                    // An absent member is a key that is not printed.
                    body.push(format!("if v.{field} != nil {{"));
                    body.push(format!(
                        "\tzd_out[{key}] = {}(*v.{field}, {where_})",
                        dumper_name(ast, member_type)
                    ));
                    body.push("}".to_string());
                } else {
                    body.push(format!(
                        "zd_out[{key}] = {}(v.{field}, {where_})",
                        dumper_name(ast, member_type)
                    ));
                }
            }
            body.push("return zd_out".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("zd_out := map[string]any{}".to_string());
            body.push(format!(
                "zd_out[\"_d\"] = {}(v.D, path+\"._d\")",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch the value holds: the holder has one field per case
            // and the defaulter fills the default branch before the
            // decoder runs, so printing every non-zero field would
            // return the selected branch and the defaulted one.
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
                            tests.push(format!("v.D == {}", go_label(ast, discriminator, value)));
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
                          body: &mut Vec<String>,
                          closes: bool| {
                let field = field_name(member_name);
                let key = go_string_literal(member_name);
                let where_ = format!("path+{}", go_string_literal(&format!(".{member_name}")));
                let call = format!("{}(v.{field}, {where_})", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push(format!("\tzd_out[{key}] = {call}"));
                        if closes {
                            body.push("}".to_string());
                        }
                    }
                    None => body.push(format!("zd_out[{key}] = {call}")),
                }
            };
            let arm_count = arms.len();
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 {
                    format!("if {condition} {{")
                } else {
                    format!("}} else if {condition} {{")
                };
                let last = index + 1 == arm_count;
                branch(
                    Some(keyword),
                    member_name,
                    *member_type,
                    &mut body,
                    last && fallback.is_none(),
                );
            }
            if let Some((member_name, member_type)) = fallback {
                if arm_count == 0 {
                    // With no labelled arm ahead of it the default case
                    // is the only one there is, so it takes no test.
                    branch(None, &member_name, member_type, &mut body, false);
                } else {
                    branch(
                        Some("} else {".to_string()),
                        &member_name,
                        member_type,
                        &mut body,
                        true,
                    );
                }
            }
            body.push("return zd_out".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Go shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\nfunc {name}(v {parameter}, path string) any {{\n");
    for line in body {
        let _ = writeln!(out, "\t{line}");
    }
    out.push_str("}\n");
    Ok(out)
}

/// A union label in the discriminator's Go form: `true`/`false` for a
/// `boolean`, and the number for everything else — a `char` and a
/// `wchar` discriminator hold their code unit as an integer here, so
/// the number is already the form.
fn go_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_primitive(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => if value == 0 { "false" } else { "true" }.to_string(),
        _ => format!("{value}"),
    }
}

/// The primitive kind a discriminator is, once typedefs are seen
/// through.
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
