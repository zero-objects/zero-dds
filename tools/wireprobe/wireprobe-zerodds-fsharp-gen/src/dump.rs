// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The F# that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-zig-gen/src/dump.rs`: one
//! function per type, keyed the way the decoder is keyed, appending
//! JSON text to one `StringBuilder`. The dialect is the shared one: a
//! 64-bit integer is a string, an absent `@optional` member is a key
//! left out, a map is an array of two-element arrays.
//!
//! A typedef's tag follows the alias, so for most types the target's
//! own body answers to the typedef's name; a typedef of a primitive is
//! the one case the two names differ and gets a forwarding body. A
//! refusal — a float with no JSON form, a code point no UTF-8 holds —
//! raises `ZdWire.WireError`, and the harness turns that into the
//! sample's own refusal.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::fsharp::field_name;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::fsharp::Fsharp;

use crate::fill::type_text;

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Fsharp>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The F# source of one dumper per type reachable from `root`, plus the
/// one JSON-escape helper they share.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n// One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         // documents. Not canonical form: the caller canonicalises in Rust.\n\n\
         let zdJsonText (zdOut: System.Text.StringBuilder) (text: string) : unit =\n\
         \x20   zdOut.Append('\"') |> ignore\n\
         \x20   for zdC in text do\n\
         \x20       match zdC with\n\
         \x20       | '\"' -> zdOut.Append(\"\\\\\\\"\") |> ignore\n\
         \x20       | '\\\\' -> zdOut.Append(\"\\\\\\\\\") |> ignore\n\
         \x20       | '\\n' -> zdOut.Append(\"\\\\n\") |> ignore\n\
         \x20       | '\\r' -> zdOut.Append(\"\\\\r\") |> ignore\n\
         \x20       | '\\t' -> zdOut.Append(\"\\\\t\") |> ignore\n\
         \x20       | c when int c < 0x20 -> zdOut.Append(sprintf \"\\\\u%04x\" (int c)) |> ignore\n\
         \x20       | c -> zdOut.Append(c) |> ignore\n\
         \x20   zdOut.Append('\"') |> ignore\n\n\
         let zdJsonPoint (zdOut: System.Text.StringBuilder) (point: int) : unit =\n\
         \x20   let text =\n\
         \x20       try System.Char.ConvertFromUtf32(point)\n\
         \x20       with _ -> raise (ZdWire.WireError(sprintf \"U+%04X is not a scalar value\" point))\n\
         \x20   zdJsonText zdOut text\n",
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
            let parameter = type_text(ast, id, &name)?;
            emitted.push(name.clone());
            let _ = write!(
                out,
                "\nlet {name} (zdOut: System.Text.StringBuilder) (v: {parameter}) : unit =\n\
                 \x20   {target_name} zdOut v\n"
            );
            continue;
        }
        emitted.push(name);
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
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

/// One dumper, as a whole function.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let parameter = type_text(ast, id, &name)?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        // .NET's `%e` format is a scientific-notation round trip that
        // JSON reads as a number; `%A`/`ToString()` on a `float`/
        // `float32` can print `NaN`/`Infinity`, which JSON does not
        // parse, so those two are refused ahead of the print.
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push(
                "if System.Double.IsNaN(float v) || System.Double.IsInfinity(float v) then"
                    .to_string(),
            );
            body.push(
                "    raise (ZdWire.WireError \"NaN and the infinities are not JSON\")".to_string(),
            );
            body.push("zdOut.Append(sprintf \"%.17g\" (float v)) |> ignore".to_string());
        }
        // The decimal text, quoted — this column already holds `long
        // double` as the same decimal `ZdWire.WriteF128` writes, so
        // there is nothing to convert; `nan`/`inf`/`-inf` are refused
        // here for the same reason the narrower widths are.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("let zdLower = v.Trim().ToLowerInvariant()".to_string());
            body.push("if zdLower.Contains(\"nan\") || zdLower.Contains(\"inf\") then".to_string());
            body.push(
                "    raise (ZdWire.WireError \"NaN and the infinities are not JSON\")".to_string(),
            );
            body.push("zdJsonText zdOut v".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            body.push("zdOut.Append(sprintf \"\\\"%d\\\"\" v) |> ignore".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("zdJsonPoint zdOut (int v)".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("zdOut.Append(if v then \"true\" else \"false\") |> ignore".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("zdOut.Append(sprintf \"%d\" v) |> ignore".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("zdJsonText zdOut v".to_string());
        }
        TypeKind::Enumeration { .. } => {
            body.push("match v with".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                body.push(format!(
                    "| {}.{} -> zdOut.Append({}) |> ignore",
                    crate::fill::type_text(ast, id, &name)?,
                    field_name(&node.name),
                    crate::fill::fsharp_string_literal(&format!("\"{}\"", node.name))
                ));
            }
            body.push(format!(
                "| _ -> raise (ZdWire.WireError \"{name}: an enumerator this dumper does not know\")"
            ));
        }
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
            body.push("let zdBits : uint64 = uint64 v".to_string());
            body.push(format!(
                "if (zdBits &&& ~~~(uint64 {accounted}uL)) <> 0uL then"
            ));
            body.push(
                "    raise (ZdWire.WireError \"a bit this bitmask does not declare is set\")"
                    .to_string(),
            );
            body.push("zdOut.Append('[') |> ignore".to_string());
            body.push("let mutable zdFirst = true".to_string());
            for (bit, position) in &declared {
                body.push(format!("if (zdBits >>> {position}) &&& 1uL = 1uL then"));
                body.push("    if not zdFirst then zdOut.Append(',') |> ignore".to_string());
                body.push("    zdFirst <- false".to_string());
                body.push(format!(
                    "    zdOut.Append({}) |> ignore",
                    crate::fill::fsharp_string_literal(&format!("\"{bit}\""))
                ));
            }
            body.push("zdOut.Append(']') |> ignore".to_string());
        }
        TypeKind::Bitset { .. } => {
            body.push("zdOut.Append('{') |> ignore".to_string());
            let mut first = true;
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let comma = if first { "" } else { "," };
                first = false;
                body.push(format!(
                    "zdOut.Append(\"{comma}\\\"{}\\\":\") |> ignore",
                    escape_key(name)
                ));
                body.push(format!(
                    "zdOut.Append(sprintf \"%d\" v.{}) |> ignore",
                    field_name(name)
                ));
            }
            body.push("zdOut.Append('}') |> ignore".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("zdOut.Append('[') |> ignore".to_string());
            body.push("for zdI in 0 .. v.Length - 1 do".to_string());
            body.push("    if zdI <> 0 then zdOut.Append(',') |> ignore".to_string());
            body.push(format!("    {} zdOut v.[zdI]", dumper_name(ast, element)));
            body.push("zdOut.Append(']') |> ignore".to_string());
        }
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let levels = dims.len();
            let mut access = "v".to_string();
            for level in 0..levels {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}zdOut.Append('[') |> ignore"));
                body.push(format!(
                    "{indent}for zdI{level} in 0 .. {access}.Length - 1 do"
                ));
                body.push(format!(
                    "{indent}    if zdI{level} <> 0 then zdOut.Append(',') |> ignore"
                ));
                access = format!("{access}.[zdI{level}]");
            }
            let deepest = "    ".repeat(levels);
            body.push(format!(
                "{deepest}{} zdOut {access}",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}zdOut.Append(']') |> ignore"));
            }
        }
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push("zdOut.Append('[') |> ignore".to_string());
            body.push("for zdI in 0 .. v.Length - 1 do".to_string());
            body.push("    if zdI <> 0 then zdOut.Append(',') |> ignore".to_string());
            body.push("    zdOut.Append('[') |> ignore".to_string());
            body.push(format!(
                "    {} zdOut v.[zdI].Key",
                dumper_name(ast, key_type)
            ));
            body.push("    zdOut.Append(',') |> ignore".to_string());
            body.push(format!(
                "    {} zdOut v.[zdI].Value",
                dumper_name(ast, value_type)
            ));
            body.push("    zdOut.Append(']') |> ignore".to_string());
            body.push("zdOut.Append(']') |> ignore".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("zdOut.Append('{') |> ignore".to_string());
            let members = ast.all_members_of(id);
            let any_optional = members.iter().any(|member| {
                ast.member(*member)
                    .is_ok_and(|member| member.facets.is_optional())
            });
            if any_optional && !members.is_empty() {
                body.push("let mutable zdFirst = true".to_string());
            }
            let mut first = true;
            for member in members {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::fsharp::Fsharp,
                >(ast, id, member.node_id, &member.name);
                let key = escape_key(&member.name);
                let optional = member.facets.is_optional();
                let comma = |body: &mut Vec<String>, indent: &str| {
                    if any_optional {
                        body.push(format!(
                            "{indent}if not zdFirst then zdOut.Append(',') |> ignore"
                        ));
                        body.push(format!("{indent}zdFirst <- false"));
                    } else if !first {
                        body.push(format!("{indent}zdOut.Append(',') |> ignore"));
                    }
                };
                if optional {
                    body.push(format!("match v.{field} with"));
                    body.push("| Some zdP ->".to_string());
                    comma(&mut body, "    ");
                    body.push(format!("    zdOut.Append(\"\\\"{key}\\\":\") |> ignore"));
                    body.push(format!("    {} zdOut zdP", dumper_name(ast, member_type)));
                    body.push("| None -> ()".to_string());
                } else {
                    comma(&mut body, "");
                    body.push(format!("zdOut.Append(\"\\\"{key}\\\":\") |> ignore"));
                    body.push(format!("{} zdOut v.{field}", dumper_name(ast, member_type)));
                }
                first = false;
            }
            body.push("zdOut.Append('}') |> ignore".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("zdOut.Append(\"{\\\"_d\\\":\") |> ignore".to_string());
            body.push(format!("{} zdOut v._d", dumper_name(ast, discriminator)));
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
                            tests.push(fsharp_label_test(ast, discriminator, value));
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
                let key = escape_key(member_name);
                let write_key = format!("    zdOut.Append(\",\\\"{key}\\\":\") |> ignore");
                let call = format!("    {} zdOut v.{field}", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push(write_key);
                        body.push(call);
                    }
                    None => {
                        body.push(write_key.trim_start().to_string());
                        body.push(call.trim_start().to_string());
                    }
                }
            };
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 {
                    format!("if {condition} then")
                } else {
                    format!("elif {condition} then")
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
            body.push("zdOut.Append('}') |> ignore".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no F# shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out =
        format!("\nlet {name} (zdOut: System.Text.StringBuilder) (v: {parameter}) : unit =\n");
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    Ok(out)
}

/// A union label test in the discriminator's F# form: the boolean
/// literal for a `boolean`, `LanguagePrimitives.EnumToValue` for an
/// enumeration, the number for everything else.
///
/// Everything else is compared as `int64`, and an `unsigned long long`
/// label above `Int64.MaxValue` is written as the two's-complement value
/// `int64` turns the field into: `16045690984833335998L` is out of an
/// `int64` literal's range (FS1149), and it was the one form the
/// `union-discriminator-ulonglong` cells could not compile.
fn fsharp_label_test(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_kind(ast, discriminator, 0) {
        Some(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            format!("v._d = {}", if value == 0 { "false" } else { "true" })
        }
        Some(TypeKind::Enumeration { .. }) => format!("int v._d = {value}"),
        _ => {
            #[allow(clippy::cast_possible_truncation)]
            let wrapped = value as i64;
            format!("int64 v._d = {wrapped}L")
        }
    }
}

/// The type `id` is once the typedefs are seen through.
///
/// zerodds-lint: recursion-depth 64 (typedef chains only —
/// every other kind answers without a descent, so the walk never
/// reaches an aggregate and cannot cycle; bounded by [`MAX_DEPTH`])
fn resolve_kind(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<TypeKind> {
    if depth > MAX_DEPTH {
        return None;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .and_then(|target| resolve_kind(ast, target, depth + 1)),
        Ok(kind) => Some(kind),
        Err(_) => None,
    }
}

/// A member name as it appears inside a generated `"…":` key.
fn escape_key(name: &str) -> String {
    debug_assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    name.to_string()
}
