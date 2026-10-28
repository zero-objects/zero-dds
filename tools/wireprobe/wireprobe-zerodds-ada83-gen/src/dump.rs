// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada 83 that turns one decoded sample back into JSON.
//!
//! `wireprobe-zerodds-ada-gen/src/dump.rs` is the template: one
//! function per type, keyed the way the decoder is keyed, answering a
//! `Zd_Wire83.Text` of JSON text, in the shared dialect. What differs
//! is only the spelling — `Zd_Wire83.Json_*` in place of `Zd_Wire.Json_*`,
//! `Zd_Wire83.Text` string concatenation (`&`) in place of
//! `ASU.Append`/`Ada.Strings.Unbounded`, and `Long_Integer` wherever
//! `adapter::ada` reaches for an `Interfaces` width.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::ada83::{
    DISCRIMINANT, PRESENT, VALUE, ada83_pattern, ada83_value_path, component_name,
};
use zerodds_lang_rules::emit::ada83::Ada83;
use zerodds_lang_rules::emit::decode::decoder_name;

use crate::fill::ada_string_literal;

/// The name of the function that prints a value of `id`.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Ada83>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd83_json_{rest}"),
        None => format!("zd83_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The Ada 83 source of one dumper per type reachable from `root` — the
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

/// The Ada 83 spelling of the dumper's parameter.
fn parameter_type(ast: &Ast, id: Id<TypeNode>, name: &str) -> Result<String, String> {
    match ada83_value_path(ast, id) {
        Ok(path) => Ok(path
            .segments()
            .iter()
            .map(zerodds_lang_cst::common::Ident::as_str)
            .collect::<Vec<&str>>()
            .join(".")),
        Err(error) => Err(format!("{name}: the type has no Ada 83 form: {error:?}")),
    }
}

fn head(name: &str, parameter: &str) -> String {
    format!("function {name} (v : in {parameter}; zd_path : in String)\n  return Zd_Wire83.Text")
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
            body.push("return Zd_Wire83.Json_F32 (v, zd_path);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Float64) => {
            body.push("return Zd_Wire83.Json_F64 (v, zd_path);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return Zd_Wire83.Json_String (v);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::UInt64) => {
            body.push("return Zd_Wire83.Json_U64 (v);".to_string());
        }
        // Both 64-bit widths go out as a decimal *in a string*, which is
        // the dialect `crates/wire-probe` documents and what every other
        // column writes: a JSON number is a double and loses the low
        // bits past 2**53. `Json_Int`'s unquoted form would have made
        // this column the only one spelling a `long long` bare.
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("return Zd_Wire83.Json_I64 (v);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("return Zd_Wire83.Json_Bool (v);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("return Zd_Wire83.Json_Char (v);".to_string());
        }
        // Every narrower integer: `v` is already `Long_Integer`.
        TypeKind::Primitive(_) => {
            body.push("return Zd_Wire83.Json_Int (v);".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return Zd_Wire83.Json_String (v);".to_string());
        }
        // The literal's IDL name, chosen by the value: an `if`/`elsif`
        // chain, since this column names no enumeration type to
        // `case` over.
        TypeKind::Enumeration { .. } => {
            let mut first = true;
            for enumerator in ast.enumerators_of(id) {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let keyword = if first { "if" } else { "elsif" };
                first = false;
                body.push(format!(
                    "{keyword} v = {} then",
                    component_name(&enum_node.name)
                ));
                body.push(format!(
                    "   return Zd_Wire83.Json_String (Zd_Wire83.Text_Of ({}));",
                    ada_string_literal(&enum_node.name)
                ));
            }
            if !first {
                body.push("end if;".to_string());
            }
            body.push("raise Zd_Wire83.Malformed_Stream;".to_string());
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
            declarations.push("zd_out : Zd_Wire83.Text := Zd_Wire83.Text_Of (\"[\");".to_string());
            // A bit outside the declared set is refused rather than
            // printed silently. The mask is the 64-bit pattern: a
            // bitmask declaring `@position(63)` has no decimal below
            // `Long_Integer'Last`, and `Bits_Subset` reads both sides as
            // patterns for that reason.
            body.push(format!(
                "if not Zd_Wire83.Bits_Subset (v, {}) then",
                ada83_pattern(i128::from(accounted))
            ));
            body.push("   raise Zd_Wire83.Malformed_Stream;".to_string());
            body.push("end if;".to_string());
            for (bit, position) in &declared {
                // `Shift_Right` and not `v / 2**position`: RM83 4.5.5's
                // `/` truncates toward zero, which reads the wrong bit
                // out of a value whose bit 63 is set, and `2**63` has no
                // `Long_Integer` to divide by in the first place.
                body.push(format!(
                    "if Zd_Wire83.Shift_Right (v, {position}) mod 2 = 1 then"
                ));
                body.push(
                    "   if zd_out.all'Length > 1 then Zd_Wire83.Json_Append (zd_out, \",\"); end if;".to_string(),
                );
                body.push(format!(
                    "   Zd_Wire83.Json_Append (zd_out, Zd_Wire83.Json_String (Zd_Wire83.Text_Of ({})).all);",
                    ada_string_literal(bit)
                ));
                body.push("end if;".to_string());
            }
            body.push("Zd_Wire83.Json_Append (zd_out, \"]\");".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Bitset { .. } => {
            declarations.push("zd_out : Zd_Wire83.Text := Zd_Wire83.Text_Of (\"{\");".to_string());
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field.name.as_ref() else {
                    continue;
                };
                body.push(format!(
                    "Zd_Wire83.Json_Member (zd_out, {}, Zd_Wire83.Json_Int (v.{}));",
                    ada_string_literal(field_name),
                    component_name(field_name)
                ));
            }
            body.push("Zd_Wire83.Json_Append (zd_out, \"}\");".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            declarations.push("zd_out : Zd_Wire83.Text := Zd_Wire83.Text_Of (\"[\");".to_string());
            body.push("for zd_i in v.all'Range loop".to_string());
            body.push(
                "   if zd_out.all'Length > 1 then Zd_Wire83.Json_Append (zd_out, \",\"); end if;"
                    .to_string(),
            );
            body.push(format!(
                "   Zd_Wire83.Json_Append (zd_out, {} (v (zd_i), zd_path).all);",
                dumper_name(ast, element)
            ));
            body.push("end loop;".to_string());
            body.push("Zd_Wire83.Json_Append (zd_out, \"]\");".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let levels = dims.len();
            let mut access = String::from("v");
            for level in 0..levels {
                declarations.push(format!(
                    "zd_out{level} : Zd_Wire83.Text := Zd_Wire83.Empty_Text;"
                ));
            }
            body.push("zd_out0 := Zd_Wire83.Text_Of (\"[\");".to_string());
            for level in 0..levels {
                let indent = "   ".repeat(level);
                if level > 0 {
                    body.push(format!(
                        "{indent}zd_out{level} := Zd_Wire83.Text_Of (\"[\");"
                    ));
                }
                body.push(format!("{indent}for zd_i{level} in {access}'Range loop"));
                body.push(format!(
                    "{indent}   if zd_out{level}.all'Length > 1 then Zd_Wire83.Json_Append (zd_out{level}, \",\"); end if;"
                ));
                access.push_str(&format!(" (zd_i{level})"));
            }
            let deepest = "   ".repeat(levels);
            let innermost = levels - 1;
            body.push(format!(
                "{deepest}Zd_Wire83.Json_Append (zd_out{innermost}, {} ({access}, zd_path).all);",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "   ".repeat(level);
                body.push(format!("{indent}end loop;"));
                body.push(format!(
                    "{indent}Zd_Wire83.Json_Append (zd_out{level}, \"]\");"
                ));
                if level > 0 {
                    let outer = level - 1;
                    body.push(format!(
                        "{indent}Zd_Wire83.Json_Append (zd_out{outer}, zd_out{level}.all);"
                    ));
                }
            }
            body.push("return zd_out0;".to_string());
        }
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            declarations.push("zd_out : Zd_Wire83.Text := Zd_Wire83.Text_Of (\"[\");".to_string());
            body.push("for zd_i in v.all'Range loop".to_string());
            body.push(
                "   if zd_out.all'Length > 1 then Zd_Wire83.Json_Append (zd_out, \",\"); end if;"
                    .to_string(),
            );
            body.push("   Zd_Wire83.Json_Append (zd_out, \"[\");".to_string());
            body.push(format!(
                "   Zd_Wire83.Json_Append (zd_out, {} (v (zd_i).Key, zd_path).all);",
                dumper_name(ast, key_type)
            ));
            body.push("   Zd_Wire83.Json_Append (zd_out, \",\");".to_string());
            body.push(format!(
                "   Zd_Wire83.Json_Append (zd_out, {} (v (zd_i).{VALUE}, zd_path).all);",
                dumper_name(ast, value_type)
            ));
            body.push("   Zd_Wire83.Json_Append (zd_out, \"]\");".to_string());
            body.push("end loop;".to_string());
            body.push("Zd_Wire83.Json_Append (zd_out, \"]\");".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            declarations.push("zd_out : Zd_Wire83.Text := Zd_Wire83.Text_Of (\"{\");".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `Parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::ada83::Ada83,
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
                if zerodds_lang_rules::adapter::ada83::held_by_reference(
                    ast,
                    id,
                    member_type,
                    member.facets.is_optional(),
                    external,
                ) {
                    body.push(format!("if v.{field} /= null then"));
                    body.push(format!(
                        "   Zd_Wire83.Json_Member (zd_out, {key}, {} (v.{field}.all, {where_}));",
                        dumper_name(ast, member_type)
                    ));
                    body.push("end if;".to_string());
                } else if member.facets.is_optional() {
                    body.push(format!("if v.{field}.{PRESENT} then"));
                    body.push(format!(
                        "   Zd_Wire83.Json_Member (zd_out, {key}, {} (v.{field}.{VALUE}, {where_}));",
                        dumper_name(ast, member_type)
                    ));
                    body.push("end if;".to_string());
                } else {
                    body.push(format!(
                        "Zd_Wire83.Json_Member (zd_out, {key}, {} (v.{field}, {where_}));",
                        dumper_name(ast, member_type)
                    ));
                }
            }
            body.push("Zd_Wire83.Json_Append (zd_out, \"}\");".to_string());
            body.push("return zd_out;".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            declarations.push("zd_out : Zd_Wire83.Text := Zd_Wire83.Text_Of (\"{\");".to_string());
            body.push(format!(
                "Zd_Wire83.Json_Member (zd_out, \"_d\", {} (v.{DISCRIMINANT}, zd_path & \"._d\"));",
                dumper_name(ast, discriminator)
            ));
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
                        body.push(format!("   Zd_Wire83.Json_Member (zd_out, {key}, {call});"));
                    }
                    None => body.push(format!("Zd_Wire83.Json_Member (zd_out, {key}, {call});")),
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
            body.push("Zd_Wire83.Json_Append (zd_out, \"}\");".to_string());
            body.push("return zd_out;".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Ada 83 shape to print from",
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

/// A union label in the discriminator's Ada 83 form: `True`/`False`
/// for a `boolean`, and the number for everything else — an
/// enumeration discriminator included, since this column names no
/// literal to compare a named constant's value against here either.
fn ada_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match ast
        .type_(resolve_alias(ast, discriminator))
        .map(|node| node.kind.clone())
    {
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            if value == 0 { "False" } else { "True" }.to_string()
        }
        // The same fold the declaration and the filler make: an
        // `unsigned long long` label at or above 2**63 is the negative
        // `Long_Integer` whose bits it is.
        _ => format!("{}", ada83_pattern(value)),
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
