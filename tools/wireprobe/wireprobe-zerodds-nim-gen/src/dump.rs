// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Nim that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-d-gen/src/dump.rs`: one
//! procedure per type, keyed the way the decoder is keyed, answering a
//! `std/json` `JsonNode`. The dialect is the shared one — a 64-bit
//! integer is a string, an absent `@optional` member is a key left out
//! — and it is not canonical form: the caller canonicalises in Rust.
//!
//! The forward declarations come first, because Nim resolves a
//! module-scope name only after its declaration and a struct's dumper
//! calls its members' whatever order the reachability walk found them
//! in.
//!
//! A typedef's tag follows the alias, so for most targets the target's
//! own body answers to the typedef's name; a typedef of a primitive is
//! the one case the two names differ — the primitive's tag carries
//! `p_` — and gets a forwarding body. A refusal — a float with no JSON
//! form, an enumerator value with no name — raises, and the harness
//! turns it into the sample's refusal.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::nim::{field_name, nim_value_type};
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::nim::Nim;

use crate::fill::nim_string_literal;

/// The name of the procedure that prints a value of `id`: the decoder's
/// name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Nim>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The Nim source of one dumper per type reachable from `root`.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n# One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         # documents. Not canonical form: the caller canonicalises in Rust.\n",
    );
    let mut emitted: Vec<String> = Vec::new();
    let mut bodies = String::new();
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
            let _ = writeln!(out, "proc {name}(v: {parameter}, path: string): JsonNode");
            let _ = write!(
                bodies,
                "\nproc {name}(v: {parameter}, path: string): JsonNode =\n  \
                 return {target_name}(v, path)\n"
            );
            continue;
        }
        let parameter = parameter_type(ast, id, &name)?;
        let _ = writeln!(out, "proc {name}(v: {parameter}, path: string): JsonNode");
        emitted.push(name);
        bodies.push_str(&function(ast, id, union_label)?);
    }
    out.push_str(&bodies);
    Ok(out)
}

/// The Nim spelling of the dumper's parameter.
fn parameter_type(ast: &Ast, id: Id<TypeNode>, name: &str) -> Result<String, String> {
    match nim_value_type(ast, id) {
        Ok(type_) => Ok(zerodds_lang_render::nim::type_(&type_)),
        Err(error) => Err(format!("{name}: the type has no Nim form: {error:?}")),
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

/// One dumper, as a whole procedure. Two-space indentation — the
/// generated file's own style.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let parameter = parameter_type(ast, id, &name)?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if not zdwire.jsonNumberOk(float64(v)):".to_string());
            body.push(
                "  raise newException(ValueError, path & \": the value has no JSON form\")"
                    .to_string(),
            );
            body.push("return %float64(v)".to_string());
        }
        // The exact decimal, which is what the member holds.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return %v".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value.
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("return %zdwire.formatI64(v)".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::UInt64) => {
            body.push("return %zdwire.formatU64(v)".to_string());
        }
        // The one-character string the dialect writes for a character.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("return %($Rune(int(v)))".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("return %v".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("return %int64(v)".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return %v".to_string());
        }
        // The enumerator's name, chosen by the value; a value with no
        // enumerator has no name.
        TypeKind::Enumeration { .. } => {
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let Some(value) = ast.effective_enumerator_value(*enumerator) else {
                    continue;
                };
                body.push(format!("if zdwire.zdI64(v) == {value}'i64:"));
                body.push(format!("  return %{}", nim_string_literal(&node.name)));
            }
            body.push(
                "raise newException(ValueError, path & \": the value names no enumerator of \
                 this type\")"
                    .to_string(),
            );
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
            body.push("let zdBits = uint64(v)".to_string());
            body.push(format!("if (zdBits and not {accounted:#x}'u64) != 0'u64:"));
            body.push(
                "  raise newException(ValueError, path & \": a bit this bitmask does not \
                 declare is set\")"
                    .to_string(),
            );
            body.push("var zdOut = newJArray()".to_string());
            for (bit, position) in &declared {
                body.push(format!("if ((zdBits shr {position}) and 1'u64) == 1'u64:"));
                body.push(format!("  zdOut.add(%{})", nim_string_literal(bit)));
            }
            body.push("return zdOut".to_string());
        }
        // The whole list, base first. A holder past int64 would raise
        // in the conversion, which is the loud failure.
        TypeKind::Bitset { .. } => {
            body.push("var zdOut = newJObject()".to_string());
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                body.push(format!(
                    "zdOut[{}] = %int64(v.{})",
                    nim_string_literal(name),
                    field_name(name)
                ));
            }
            body.push("return zdOut".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("var zdOut = newJArray()".to_string());
            body.push("for zdI in 0 ..< v.len:".to_string());
            body.push(format!(
                "  zdOut.add({}(v[zdI], path & \"[\" & $zdI & \"]\"))",
                dumper_name(ast, element)
            ));
            body.push("return zdOut".to_string());
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
                let indent = "  ".repeat(level);
                body.push(format!("{indent}var zdOut{level} = newJArray()"));
                body.push(format!("{indent}for zdI{level} in 0 ..< {access}.len:"));
                access.push_str(&format!("[zdI{level}]"));
            }
            let deepest = "  ".repeat(levels);
            let innermost = levels - 1;
            body.push(format!(
                "{deepest}zdOut{innermost}.add({}({access}, path))",
                dumper_name(ast, element)
            ));
            for level in (1..levels).rev() {
                let outer = level - 1;
                let indent = "  ".repeat(level);
                body.push(format!("{indent}zdOut{outer}.add(zdOut{level})"));
            }
            body.push("return zdOut0".to_string());
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
            body.push("var zdOut = newJArray()".to_string());
            body.push("for zdI in 0 ..< v.len:".to_string());
            body.push("  let zdWhere = path & \"[\" & $zdI & \"]\"".to_string());
            body.push("  var zdPair = newJArray()".to_string());
            body.push(format!(
                "  zdPair.add({}(v[zdI].key, zdWhere & \".key\"))",
                dumper_name(ast, key_type)
            ));
            body.push(format!(
                "  zdPair.add({}(v[zdI].value, zdWhere & \".value\"))",
                dumper_name(ast, value_type)
            ));
            body.push("  zdOut.add(zdPair)".to_string());
            body.push("return zdOut".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("var zdOut = newJObject()".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::nim::Nim,
                >(ast, id, member.node_id, &member.name);
                let key = nim_string_literal(&member.name);
                let where_ = format!(
                    "path & {}",
                    nim_string_literal(&format!(".{}", member.name))
                );
                let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                let optional = member.facets.is_optional();
                match (optional, external) {
                    (true, true) => {
                        body.push(format!("if v.{field}.isSome:"));
                        body.push(format!(
                            "  zdOut[{key}] = {}(v.{field}.get[], {where_})",
                            dumper_name(ast, member_type)
                        ));
                    }
                    (true, false) => {
                        body.push(format!("if v.{field}.isSome:"));
                        body.push(format!(
                            "  zdOut[{key}] = {}(v.{field}.get, {where_})",
                            dumper_name(ast, member_type)
                        ));
                    }
                    // A non-optional external reference is always set
                    // by Table 9 or by the decoder.
                    (false, true) => {
                        body.push(format!(
                            "zdOut[{key}] = {}(v.{field}[], {where_})",
                            dumper_name(ast, member_type)
                        ));
                    }
                    (false, false) => {
                        body.push(format!(
                            "zdOut[{key}] = {}(v.{field}, {where_})",
                            dumper_name(ast, member_type)
                        ));
                    }
                }
            }
            body.push("return zdOut".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("var zdOut = newJObject()".to_string());
            body.push(format!(
                "zdOut[\"_d\"] = {}(v.d, path & \"._d\")",
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
                            tests.push(nim_label_test(ast, discriminator, value));
                        }
                    }
                }
                if is_default {
                    fallback = Some((member.name.clone(), member_type, external));
                }
                if !tests.is_empty() {
                    arms.push((
                        tests.join(" or "),
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
                let field = field_name(member_name);
                let key = nim_string_literal(member_name);
                let where_ = format!("path & {}", nim_string_literal(&format!(".{member_name}")));
                // The selected branch is the `isSome` one; `get`
                // reaches the value the codec put there, behind its
                // `ref` where the IDL held it externally.
                let accessor = if external {
                    format!("v.{field}.get[]")
                } else {
                    format!("v.{field}.get")
                };
                let call = format!("{}({accessor}, {where_})", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push(format!("  zdOut[{key}] = {call}"));
                    }
                    None => body.push(format!("zdOut[{key}] = {call}")),
                }
            };
            for (index, (condition, member_name, member_type, external)) in arms.iter().enumerate()
            {
                let keyword = if index == 0 {
                    format!("if {condition}:")
                } else {
                    format!("elif {condition}:")
                };
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
                    branch(
                        Some("else:".to_string()),
                        &member_name,
                        member_type,
                        external,
                        &mut body,
                    );
                }
            }
            body.push("return zdOut".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Nim shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\nproc {name}(v: {parameter}, path: string): JsonNode =\n");
    for line in body {
        let _ = writeln!(out, "  {line}");
    }
    Ok(out)
}

/// A union label test in the discriminator's Nim form: the boolean
/// literal for a `boolean`, the discriminator's own width for an
/// `unsigned long long`, and the 64-bit widening for everything else —
/// an enumeration compares through `zdwire.zdI64`, because a Nim enum
/// does not compare against a bare integer.
///
/// The `unsigned long long` arm is not a convenience. A label above
/// `int64` — `case 16045690984833335998:` is one the corpus has — has
/// no `'i64` literal at all: Nim rejects the token before any
/// conversion, so widening to `int64` would put a source file through
/// the compiler that cannot be one. `emit/nim.rs`'s `wide_int` makes
/// the same distinction for the codec itself.
fn nim_label_test(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_primitive(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => {
            format!("v.d == {}", if value == 0 { "false" } else { "true" })
        }
        Some(PrimitiveKind::UInt64) => format!("v.d == {value}'u64"),
        _ => format!("zdwire.zdI64(v.d) == {value}'i64"),
    }
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
