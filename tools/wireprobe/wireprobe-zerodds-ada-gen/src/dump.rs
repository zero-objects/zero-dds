// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-d-gen/src/dump.rs`: one
//! function per type, keyed the way the decoder is keyed, answering an
//! `Unbounded_String` of JSON text. The dialect is the shared one — a
//! 64-bit integer is a string, an absent `@optional` member is a key
//! left out — and it is not canonical form: the caller canonicalises in
//! Rust.
//!
//! Every profile is declared ahead of every body, because Ada resolves
//! nothing forward and the dumpers call each other. A refusal — a float
//! with no JSON form, a bit no bitmask declares — raises `Wire_Error`,
//! and the harness turns it into the sample's refusal.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::ada::{
    DISCRIMINANT, PRESENT, VALUE, ada_value_path, component_name,
};
use zerodds_lang_rules::emit::ada::Ada;
use zerodds_lang_rules::emit::decode::decoder_name;

use crate::fill::ada_string_literal;

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Ada>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The Ada source of one dumper per type reachable from `root` — the
/// profiles first, then the bodies — un-indented; the caller indents
/// into the probe's declarative part.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut specs = String::from(
        "\n--  One dumper per type, in the JSON dialect crates/wire-probe\n\
         --  documents. Not canonical form: the caller canonicalises in Rust.\n",
    );
    let mut bodies = String::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in &reachable {
        let id = *id;
        let name = dumper_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let parameter = parameter_type(ast, id, &name)?;
        if let Ok(TypeKind::Alias { aliased }) = ast.type_(id).map(|node| node.kind.clone()) {
            let target = match aliased.resolved() {
                Some(target) => target,
                None => return Err(format!("{name}: the typedef target is unresolved")),
            };
            let target_name = dumper_name(ast, target);
            if target_name == name {
                continue;
            }
            emitted.push(name.clone());
            let _ = writeln!(specs, "\n{}", profile(&name, &parameter));
            let _ = write!(
                bodies,
                "\n{} is\nbegin\n   return {target_name} (v, zd_path);\nend {name};\n",
                head(&name, &parameter)
            );
            continue;
        }
        emitted.push(name.clone());
        let _ = writeln!(specs, "\n{}", profile(&name, &parameter));
        bodies.push_str(&function(ast, id, union_label, &parameter)?);
    }
    specs.push_str(&bodies);
    Ok(specs)
}

/// The Ada spelling of the dumper's parameter.
fn parameter_type(ast: &Ast, id: Id<TypeNode>, name: &str) -> Result<String, String> {
    match ada_value_path(ast, id) {
        Ok(path) => Ok(path
            .segments()
            .iter()
            .map(zerodds_lang_cst::common::Ident::as_str)
            .collect::<Vec<&str>>()
            .join(".")),
        Err(error) => Err(format!("{name}: the type has no Ada form: {error:?}")),
    }
}

fn head(name: &str, parameter: &str) -> String {
    format!(
        "function {name} (v : in {parameter}; zd_path : in String)\n  return Ada.Strings.Unbounded.Unbounded_String"
    )
}

fn profile(name: &str, parameter: &str) -> String {
    format!("{};", head(name, parameter))
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

/// One dumper, as a whole function body. Three spaces, the generated
/// file's own style.
#[allow(clippy::too_many_lines)]
fn function(
    ast: &Ast,
    id: Id<TypeNode>,
    union_label: UnionLabelMatch,
    parameter: &str,
) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let mut body: Vec<String> = Vec::new();
    let mut declarations: Vec<String> = Vec::new();
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float32) => {
            body.push("return Zd_Wire.Json_F32 (v, zd_path);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Float64) => {
            body.push("return Zd_Wire.Json_F64 (v, zd_path);".to_string());
        }
        // The exact decimal text, which is what the member holds.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return Zd_Wire.Json_String (v);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("return Zd_Wire.Json_I64 (v);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::UInt64) => {
            body.push("return Zd_Wire.Json_U64 (Interfaces.Unsigned_64 (v));".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("return Zd_Wire.Json_Bool (v);".to_string());
        }
        // The one-character string the dialect writes for a character.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("return Zd_Wire.Json_Char (Interfaces.Unsigned_32 (v));".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("return Zd_Wire.Json_Int (Interfaces.Integer_64 (v));".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return Zd_Wire.Json_String (v);".to_string());
        }
        // The literal's IDL name, chosen by the value; an Ada
        // enumeration admits no value outside its literals, so the case
        // is exhaustive with no other arm.
        TypeKind::Enumeration { .. } => {
            body.push("case v is".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                body.push(format!("   when {} =>", component_name(&node.name)));
                body.push(format!(
                    "      return Zd_Wire.Json_String (Zd_Wire.To_Text ({}));",
                    ada_string_literal(&node.name)
                ));
            }
            body.push("end case;".to_string());
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
            declarations.push(
                "zd_bits : constant Interfaces.Unsigned_64 := Interfaces.Unsigned_64 (v);"
                    .to_string(),
            );
            declarations
                .push("zd_out : ASU.Unbounded_String := Zd_Wire.To_Text (\"[\");".to_string());
            body.push(format!(
                "if (zd_bits and not Interfaces.Unsigned_64'({accounted})) /= 0 then"
            ));
            body.push(
                "   raise Zd_Wire.Wire_Error with zd_path & \": a bit this bitmask does not declare is set\";"
                    .to_string(),
            );
            body.push("end if;".to_string());
            for (bit, position) in &declared {
                body.push(format!(
                    "if (Interfaces.Shift_Right (zd_bits, {position}) and 1) = 1 then"
                ));
                body.push(
                    "   if ASU.Length (zd_out) > 1 then ASU.Append (zd_out, ','); end if;"
                        .to_string(),
                );
                body.push(format!(
                    "   ASU.Append (zd_out, Zd_Wire.Json_String (Zd_Wire.To_Text ({})));",
                    ada_string_literal(bit)
                ));
                body.push("end if;".to_string());
            }
            body.push("ASU.Append (zd_out, ']');".to_string());
            body.push("return zd_out;".to_string());
        }
        // The whole list, base first.
        TypeKind::Bitset { .. } => {
            declarations
                .push("zd_out : ASU.Unbounded_String := Zd_Wire.To_Text (\"{\");".to_string());
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field.name.as_ref() else {
                    continue;
                };
                body.push(format!(
                    "Zd_Wire.Json_Member (zd_out, {}, Zd_Wire.Json_Int (Interfaces.Integer_64 (v.{})));",
                    ada_string_literal(field_name),
                    component_name(field_name)
                ));
            }
            body.push("ASU.Append (zd_out, '}');".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            declarations
                .push("zd_out : ASU.Unbounded_String := Zd_Wire.To_Text (\"[\");".to_string());
            body.push("for zd_i in v.all'Range loop".to_string());
            body.push(
                "   if ASU.Length (zd_out) > 1 then ASU.Append (zd_out, ','); end if;".to_string(),
            );
            body.push(format!(
                "   ASU.Append (zd_out, {} (v (zd_i), zd_path));",
                dumper_name(ast, element)
            ));
            body.push("end loop;".to_string());
            body.push("ASU.Append (zd_out, ']');".to_string());
            body.push("return zd_out;".to_string());
        }
        // One loop per dimension, arrays built inside out.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let levels = dims.len();
            let mut access = String::from("v");
            for level in 0..levels {
                declarations.push(format!(
                    "zd_out{level} : ASU.Unbounded_String := ASU.Null_Unbounded_String;"
                ));
            }
            body.push("zd_out0 := Zd_Wire.To_Text (\"[\");".to_string());
            for level in 0..levels {
                let indent = "   ".repeat(level);
                if level > 0 {
                    body.push(format!("{indent}zd_out{level} := Zd_Wire.To_Text (\"[\");"));
                }
                body.push(format!("{indent}for zd_i{level} in {access}'Range loop"));
                body.push(format!(
                    "{indent}   if ASU.Length (zd_out{level}) > 1 then ASU.Append (zd_out{level}, ','); end if;"
                ));
                access.push_str(&format!(" (zd_i{level})"));
            }
            let deepest = "   ".repeat(levels);
            let innermost = levels - 1;
            body.push(format!(
                "{deepest}ASU.Append (zd_out{innermost}, {} ({access}, zd_path));",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "   ".repeat(level);
                body.push(format!("{indent}end loop;"));
                body.push(format!("{indent}ASU.Append (zd_out{level}, ']');"));
                if level > 0 {
                    let outer = level - 1;
                    body.push(format!(
                        "{indent}ASU.Append (zd_out{outer}, zd_out{level});"
                    ));
                }
            }
            body.push("return zd_out0;".to_string());
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
            declarations
                .push("zd_out : ASU.Unbounded_String := Zd_Wire.To_Text (\"[\");".to_string());
            body.push("for zd_i in v.all'Range loop".to_string());
            body.push(
                "   if ASU.Length (zd_out) > 1 then ASU.Append (zd_out, ','); end if;".to_string(),
            );
            body.push("   ASU.Append (zd_out, '[');".to_string());
            body.push(format!(
                "   ASU.Append (zd_out, {} (v (zd_i).Key, zd_path));",
                dumper_name(ast, key_type)
            ));
            body.push("   ASU.Append (zd_out, ',');".to_string());
            body.push(format!(
                "   ASU.Append (zd_out, {} (v (zd_i).{VALUE}, zd_path));",
                dumper_name(ast, value_type)
            ));
            body.push("   ASU.Append (zd_out, ']');".to_string());
            body.push("end loop;".to_string());
            body.push("ASU.Append (zd_out, ']');".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            declarations
                .push("zd_out : ASU.Unbounded_String := Zd_Wire.To_Text (\"{\");".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `Parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::ada::Ada,
                >(ast, id, member.node_id, &member.name);
                let key = ada_string_literal(&member.name);
                let where_ = format!(
                    "zd_path & {}",
                    ada_string_literal(&format!(".{}", member.name))
                );
                let external = member
                    .facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|xtypes| xtypes.is_external);
                // An access type where the adapter declares one: present
                // is `/= null`, the value is `.all`.
                if zerodds_lang_rules::adapter::ada::held_by_reference(
                    ast,
                    id,
                    member_type,
                    member.facets.is_optional(),
                    external,
                ) {
                    body.push(format!("if v.{field} /= null then"));
                    body.push(format!(
                        "   Zd_Wire.Json_Member (zd_out, {key}, {} (v.{field}.all, {where_}));",
                        dumper_name(ast, member_type)
                    ));
                    body.push("end if;".to_string());
                } else if member.facets.is_optional() {
                    body.push(format!("if v.{field}.{PRESENT} then"));
                    body.push(format!(
                        "   Zd_Wire.Json_Member (zd_out, {key}, {} (v.{field}.{VALUE}, {where_}));",
                        dumper_name(ast, member_type)
                    ));
                    body.push("end if;".to_string());
                } else {
                    body.push(format!(
                        "Zd_Wire.Json_Member (zd_out, {key}, {} (v.{field}, {where_}));",
                        dumper_name(ast, member_type)
                    ));
                }
            }
            body.push("ASU.Append (zd_out, '}');".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            declarations
                .push("zd_out : ASU.Unbounded_String := Zd_Wire.To_Text (\"{\");".to_string());
            body.push(format!(
                "Zd_Wire.Json_Member (zd_out, \"_d\", {} (v.{DISCRIMINANT}, zd_path & \"._d\"));",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch: only the selected one exists in the record.
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
                            tests.push(format!(
                                "v.{DISCRIMINANT} = {}",
                                ada_label(ast, discriminator, value)
                            ));
                        }
                    }
                }
                if is_default {
                    fallback = Some((member.name.clone(), member_type));
                }
                if !tests.is_empty() {
                    arms.push((tests.join(" or else "), member.name.clone(), member_type));
                }
            }
            let branch = |head: Option<String>,
                          member_name: &str,
                          member_type: Id<TypeNode>,
                          body: &mut Vec<String>| {
                let field = component_name(member_name);
                let key = ada_string_literal(member_name);
                let where_ = format!(
                    "zd_path & {}",
                    ada_string_literal(&format!(".{member_name}"))
                );
                let call = format!("{} (v.{field}, {where_})", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push(format!("   Zd_Wire.Json_Member (zd_out, {key}, {call});"));
                    }
                    None => body.push(format!("Zd_Wire.Json_Member (zd_out, {key}, {call});")),
                }
            };
            let had_arms = !arms.is_empty();
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 {
                    format!("if {condition} then")
                } else {
                    format!("elsif {condition} then")
                };
                branch(Some(keyword), member_name, *member_type, &mut body);
            }
            if let Some((member_name, member_type)) = fallback {
                if had_arms {
                    branch(
                        Some("else".to_string()),
                        &member_name,
                        member_type,
                        &mut body,
                    );
                } else {
                    branch(None, &member_name, member_type, &mut body);
                }
            }
            if had_arms {
                body.push("end if;".to_string());
            }
            body.push("ASU.Append (zd_out, '}');".to_string());
            body.push("return zd_out;".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Ada shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\n{} is\n", head(&name, parameter));
    for line in declarations {
        let _ = writeln!(out, "   {line}");
    }
    out.push_str("begin\n");
    for line in body {
        let _ = writeln!(out, "   {line}");
    }
    let _ = writeln!(out, "end {name};");
    Ok(out)
}

/// A union label in the discriminator's Ada form: the enumeration
/// literal, `True`/`False` for a `boolean`, and the number for
/// everything else.
fn ada_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match ast
        .type_(resolve_alias(ast, discriminator))
        .map(|node| node.kind.clone())
    {
        Ok(TypeKind::Enumeration { enumerators }) => {
            for enumerator in &enumerators {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ast.effective_enumerator_value(*enumerator) == Some(value) {
                    return component_name(&node.name);
                }
            }
            format!("{value}")
        }
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            if value == 0 { "False" } else { "True" }.to_string()
        }
        _ => format!("{value}"),
    }
}

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_DEPTH`)
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
