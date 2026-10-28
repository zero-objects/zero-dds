// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Kotlin that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-zig-gen/src/dump.rs`: one
//! top-level function per type, keyed the way the decoder is keyed,
//! appending JSON text to one `StringBuilder`. The dialect is the shared
//! one: a 64-bit integer is a string, an absent `@optional` member is a
//! key left out, a map is an array of two-element arrays.
//!
//! A discriminator's label test needs no enumeration seam here, unlike
//! Zig's `@intFromEnum`: `adapter/kotlin.rs` gives an enumeration's
//! value the plain type `Int`, so every discriminator but a `Boolean`
//! one already compares against the label's own integer.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::kotlin::{property_name, unsigned_holder};
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::kotlin::Kotlin;

use crate::fill::{kotlin_string_literal, type_text};

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Kotlin>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

const MAX_DEPTH: usize = 64;

/// The Kotlin source of one dumper per type reachable from `root`, plus
/// the JSON-escape helper they share.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n// One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         // documents. Not canonical form: the caller canonicalises in Rust.\n\n\
         fun zdJsonText(out: StringBuilder, text: String) {\n\
         \x20   out.append('\"')\n\
         \x20   for (c in text) {\n\
         \x20       when (c) {\n\
         \x20           '\"' -> out.append(\"\\\\\\\"\")\n\
         \x20           '\\\\' -> out.append(\"\\\\\\\\\")\n\
         \x20           '\\n' -> out.append(\"\\\\n\")\n\
         \x20           '\\r' -> out.append(\"\\\\r\")\n\
         \x20           '\\t' -> out.append(\"\\\\t\")\n\
         \x20           else -> if (c.code < 0x20) {\n\
         \x20               out.append(\"\\\\u%04x\".format(c.code))\n\
         \x20           } else {\n\
         \x20               out.append(c)\n\
         \x20           }\n\
         \x20       }\n\
         \x20   }\n\
         \x20   out.append('\"')\n\
         }\n",
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
                "\nfun {name}(out: StringBuilder, v: {parameter}) {{\n\
                 \x20   {target_name}(out, v)\n\
                 }}\n"
            );
            continue;
        }
        emitted.push(name);
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
}

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
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if (v.isNaN() || v.isInfinite()) {".to_string());
            body.push(
                "    throw IllegalArgumentException(\"NaN and the infinities are not JSON\")"
                    .to_string(),
            );
            body.push("}".to_string());
            body.push("out.append(v.toString())".to_string());
        }
        // The binary128, as a decimal in a string: a JSON number is read
        // back through a `Double` and thirty-four significant digits do
        // not survive that.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("if (v.isNaN() || v.isInfinite()) {".to_string());
            body.push(
                "    throw IllegalArgumentException(\"NaN and the infinities are not JSON\")"
                    .to_string(),
            );
            body.push("}".to_string());
            body.push("zdJsonText(out, v.toString())".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("zdJsonText(out, v.toString())".to_string());
        }
        // The JVM has no unsigned integer, so an `unsigned long long`
        // past `Long.MAX_VALUE` sits in the `Long` as the pattern of the
        // same 64 bits and `toString` would print the negative number
        // that pattern reads as signed. `Long.toUnsignedString` is the
        // value the sample gave, and the spelling `tools/wireprobe/Zdj.java`
        // prints the Java column's through.
        TypeKind::Primitive(PrimitiveKind::UInt64) => {
            body.push("zdJsonText(out, java.lang.Long.toUnsignedString(v))".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Char) => {
            body.push("zdJsonText(out, v.toString())".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::WChar) => {
            body.push("zdJsonText(out, String(Character.toChars(v)))".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("out.append(if (v) \"true\" else \"false\")".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("out.append(v.toString())".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("zdJsonText(out, v)".to_string());
        }
        // An enumeration's value is a plain `Int`; the name is chosen by
        // matching it against every declared enumerator.
        TypeKind::Enumeration { .. } => {
            body.push("when (v) {".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let value = ast.effective_enumerator_value(*enumerator).unwrap_or(0);
                body.push(format!(
                    "    {value} -> zdJsonText(out, {})",
                    kotlin_string_literal(&node.name)
                ));
            }
            body.push(
                "    else -> throw IllegalArgumentException(\"$v is not a declared enumerator\")"
                    .to_string(),
            );
            body.push("}".to_string());
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
            // The comparison runs in a `Long` whatever the holder is,
            // so every literal beside it carries the suffix. The
            // conversion is written only where the holder is narrower
            // than that: `adapter/kotlin.rs::unsigned_holder` gives a
            // bound past thirty-two a `Long` already, and `kotlinc
            // -Werror` refuses `.toLong()` on one as a redundant call.
            if unsigned_holder(bound) == zerodds_lang_cst::kotlin::Primitive::Long {
                body.push("val bits: Long = v".to_string());
            } else {
                body.push("val bits: Long = v.toLong()".to_string());
            }
            #[allow(clippy::cast_possible_wrap)]
            let accounted_pattern = accounted as i64;
            // The mask in parentheses: Kotlin has no negative literal,
            // only a minus in front of one, and `.inv()` binds tighter
            // than the minus — `-3L.inv()` is `-(3L.inv())` and a
            // different mask.
            body.push(format!(
                "if ((bits and ({}).inv()) != 0L) {{",
                zerodds_lang_render::kotlin::long_text(accounted_pattern)
            ));
            body.push("    throw IllegalArgumentException(\"a bit this bitmask does not declare is set\")".to_string());
            body.push("}".to_string());
            body.push("out.append('[')".to_string());
            body.push("var first = true".to_string());
            for (bit, position) in &declared {
                body.push(format!("if (((bits ushr {position}) and 1L) == 1L) {{"));
                body.push("    if (!first) out.append(',')".to_string());
                body.push("    first = false".to_string());
                body.push(format!(
                    "    out.append({})",
                    kotlin_string_literal(&format!("\"{bit}\""))
                ));
                body.push("}".to_string());
            }
            body.push("out.append(']')".to_string());
        }
        TypeKind::Bitset { .. } => {
            body.push("out.append('{')".to_string());
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
                    "out.append(\"{comma}\\\"{}\\\":\")",
                    escape_key(name)
                ));
                body.push(format!("out.append(v.{}.toString())", property_name(name)));
            }
            body.push("out.append('}')".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("out.append('[')".to_string());
            body.push("for ((i, e) in v.withIndex()) {".to_string());
            body.push("    if (i != 0) out.append(',')".to_string());
            body.push(format!("    {}(out, e)", dumper_name(ast, element)));
            body.push("}".to_string());
            body.push("out.append(']')".to_string());
        }
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let levels = dims.len();
            let mut access = String::from("v");
            for level in 0..levels {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}out.append('[')"));
                body.push(format!(
                    "{indent}for ((i{level}, e{level}) in {access}.withIndex()) {{"
                ));
                body.push(format!("{indent}    if (i{level} != 0) out.append(',')"));
                access = format!("e{level}");
            }
            let deepest = "    ".repeat(levels);
            body.push(format!(
                "{deepest}{}(out, {access})",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}}}"));
                body.push(format!("{indent}out.append(']')"));
            }
        }
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push("out.append('[')".to_string());
            body.push("for ((i, e) in v.withIndex()) {".to_string());
            body.push("    if (i != 0) out.append(',')".to_string());
            body.push("    out.append('[')".to_string());
            body.push(format!("    {}(out, e.key)", dumper_name(ast, key_type)));
            body.push("    out.append(',')".to_string());
            body.push(format!(
                "    {}(out, e.value)",
                dumper_name(ast, value_type)
            ));
            body.push("    out.append(']')".to_string());
            body.push("}".to_string());
            body.push("out.append(']')".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("out.append('{')".to_string());
            let members = ast.all_members_of(id);
            let mut any_optional = false;
            for member in &members {
                if let Ok(member) = ast.member(*member) {
                    if member.facets.is_optional() {
                        any_optional = true;
                    }
                }
            }
            if any_optional && !members.is_empty() {
                body.push("var first = true".to_string());
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
                    zerodds_lang_rules::emit::kotlin::Kotlin,
                >(ast, id, member.node_id, &member.name);
                let key = escape_key(&member.name);
                let optional = member.facets.is_optional();
                let comma = |body: &mut Vec<String>, indent: &str| {
                    if any_optional {
                        body.push(format!("{indent}if (!first) out.append(',')"));
                        body.push(format!("{indent}first = false"));
                    } else if !first {
                        body.push(format!("{indent}out.append(',')"));
                    }
                };
                if optional {
                    body.push(format!("if (v.{field} != null) {{"));
                    comma(&mut body, "    ");
                    body.push(format!("    out.append(\"\\\"{key}\\\":\")"));
                    body.push(format!(
                        "    {}(out, v.{field}!!)",
                        dumper_name(ast, member_type)
                    ));
                    body.push("}".to_string());
                } else {
                    comma(&mut body, "");
                    body.push(format!("out.append(\"\\\"{key}\\\":\")"));
                    body.push(format!("{}(out, v.{field})", dumper_name(ast, member_type)));
                }
                first = false;
            }
            body.push("out.append('}')".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("out.append(\"{\\\"_d\\\":\")".to_string());
            body.push(format!("{}(out, v._d)", dumper_name(ast, discriminator)));
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
                            tests.push(kotlin_label_test(ast, discriminator, value));
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
                let field = property_name(member_name);
                let key = escape_key(member_name);
                let write_key = format!("out.append(\",\\\"{key}\\\":\")");
                let call = format!("{}(out, v.{field})", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push(format!("    {write_key}"));
                        body.push(format!("    {call}"));
                        body.push("}".to_string());
                    }
                    None => {
                        body.push(write_key);
                        body.push(call);
                    }
                }
            };
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 {
                    format!("if ({condition}) {{")
                } else {
                    format!("}} else if ({condition}) {{")
                };
                if index > 0 {
                    let _ = body.pop();
                }
                branch(Some(keyword), member_name, *member_type, &mut body);
            }
            if let Some((member_name, member_type)) = fallback {
                if arms.is_empty() {
                    branch(None, &member_name, member_type, &mut body);
                } else {
                    let _ = body.pop();
                    branch(
                        Some("} else {".to_string()),
                        &member_name,
                        member_type,
                        &mut body,
                    );
                }
            }
            body.push("out.append('}')".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Kotlin shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\nfun {name}(out: StringBuilder, v: {parameter}) {{\n");
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    out.push_str("}\n");
    Ok(out)
}

/// A union label test in the discriminator's Kotlin form.
///
/// Kotlin compares only values of one type, and the discriminator field
/// has the type `adapter/kotlin.rs` gives it: `Boolean`, `Char` for
/// `char`/`wchar`, `Byte` for `int8`, `Short` for `short`, `Long` for
/// `long long` and both wider unsigned kinds, `Int` for the rest and for
/// an enumeration. A bare label is an `Int` literal, so `v._d == 1` on a
/// `Long` or a `Short` does not compile — what every
/// `union-discriminator-*` read cell of the Kotlin column hit. A `Char`
/// is compared by its code, a `Byte` and a `Short` widened, and a
/// `Long` gets an `L` literal; an `unsigned long long` label above
/// `Long.MAX_VALUE` is the two's-complement pattern the field holds, and
/// `Long.MIN_VALUE` is named because its literal does not parse.
fn kotlin_label_test(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_kind(ast, discriminator, 0) {
        Some(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            format!("v._d == {}", if value == 0 { "false" } else { "true" })
        }
        Some(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
            format!("v._d.code == {value}")
        }
        Some(TypeKind::Primitive(PrimitiveKind::Int8 | PrimitiveKind::Int16)) => {
            format!("v._d.toInt() == {value}")
        }
        Some(TypeKind::Primitive(
            PrimitiveKind::Int64 | PrimitiveKind::UInt32 | PrimitiveKind::UInt64,
        )) => format!("v._d == {}", kotlin_long_literal(value)),
        _ => format!("v._d == {value}"),
    }
}

/// `value` as the `Long` the field holds: an `unsigned long long` above
/// `Long.MAX_VALUE` as its two's-complement pattern.
fn kotlin_long_literal(value: i128) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let bits = value as i64;
    if bits == i64::MIN {
        "Long.MIN_VALUE".to_string()
    } else {
        format!("{bits}L")
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

fn escape_key(name: &str) -> String {
    debug_assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    name.to_string()
}
