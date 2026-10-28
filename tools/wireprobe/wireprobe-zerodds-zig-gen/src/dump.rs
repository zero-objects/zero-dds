// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Zig that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-d-gen/src/dump.rs`: one
//! function per type, keyed the way the decoder is keyed, appending
//! JSON text to one buffer — Zig's standard library builds no value
//! tree worth the detour, and the caller canonicalises in Rust either
//! way. The dialect is the shared one: a 64-bit integer is a string, an
//! absent `@optional` member is a key left out, a map is an array of
//! two-element arrays.
//!
//! A typedef's tag follows the alias, so for most targets the target's
//! own body answers to the typedef's name; a typedef of a primitive is
//! the one case the two names differ — the primitive's tag carries
//! `p_` — and gets a forwarding body. A refusal — a float with no JSON
//! form, a code point no UTF-8 holds — answers `WireError.OutOfRange`,
//! and the harness turns it into the sample's refusal.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::zig::field_name;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::zig::Zig;

use crate::fill::{type_text, zig_string_literal};

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Zig>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The Zig source of one dumper per type reachable from `root`, plus
/// the one JSON-escape helper they share.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n// One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         // documents. Not canonical form: the caller canonicalises in Rust.\n\n\
         fn zd_json_text(zd_heap: std.mem.Allocator, zd_out: *std.ArrayList(u8), text: []const u8) zdwire.WireError!void {\n\
         \x20   try zd_out.append(zd_heap, '\"');\n\
         \x20   for (text) |zd_c| {\n\
         \x20       switch (zd_c) {\n\
         \x20           '\"' => try zd_out.appendSlice(zd_heap, \"\\\\\\\"\"),\n\
         \x20           '\\\\' => try zd_out.appendSlice(zd_heap, \"\\\\\\\\\"),\n\
         \x20           '\\n' => try zd_out.appendSlice(zd_heap, \"\\\\n\"),\n\
         \x20           '\\r' => try zd_out.appendSlice(zd_heap, \"\\\\r\"),\n\
         \x20           '\\t' => try zd_out.appendSlice(zd_heap, \"\\\\t\"),\n\
         \x20           else => {\n\
         \x20               if (zd_c < 0x20) {\n\
         \x20                   try zd_out.print(zd_heap, \"\\\\u{x:0>4}\", .{zd_c});\n\
         \x20               } else {\n\
         \x20                   try zd_out.append(zd_heap, zd_c);\n\
         \x20               }\n\
         \x20           },\n\
         \x20       }\n\
         \x20   }\n\
         \x20   try zd_out.append(zd_heap, '\"');\n\
         }\n\n\
         fn zd_json_point(zd_heap: std.mem.Allocator, zd_out: *std.ArrayList(u8), point: u21) zdwire.WireError!void {\n\
         \x20   var zd_buf: [4]u8 = undefined;\n\
         \x20   const zd_n = std.unicode.utf8Encode(point, &zd_buf) catch\n\
         \x20       return zdwire.WireError.OutOfRange;\n\
         \x20   try zd_json_text(zd_heap, zd_out, zd_buf[0..zd_n]);\n\
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
                "\nfn {name}(zd_heap: std.mem.Allocator, zd_out: *std.ArrayList(u8), v: {parameter}) zdwire.WireError!void {{\n\
                 \x20   try {target_name}(zd_heap, zd_out, v);\n\
                 }}\n"
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

/// One dumper, as a whole function. Four spaces, the generated file's
/// own style.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let parameter = type_text(ast, id, &name)?;
    let mut body: Vec<String> = Vec::new();
    // `v` is always read below — every arm names it — so no discard is
    // ever needed; `zd_out` likewise.
    match &node.kind {
        // `{e}` and not `{d}`: Zig's `{d}` writes a float in positional
        // notation whatever its exponent, and `f64`'s largest finite
        // value comes out as 309 digits — `serde_json` answers `number
        // out of range` on it, measured on `sequence-double-and-
        // longdouble`. `{e}` is the same shortest round-tripping
        // decimal with an exponent, and JSON takes it as a number.
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if (std.math.isNan(v) or std.math.isInf(v)) {".to_string());
            body.push("    // NaN and the infinities are not JSON.".to_string());
            body.push("    return zdwire.WireError.OutOfRange;".to_string());
            body.push("}".to_string());
            body.push("try zd_out.print(zd_heap, \"{e}\", .{v});".to_string());
        }
        // The binary128, as a decimal in a string — the form the value
        // dialect keeps a `long double` in, because a JSON number is
        // read back through an `f64` and thirty-four significant digits
        // do not survive that.
        //
        // `{e}` and not `{d}`, for the reason above and one more:
        // measured on Zig 0.14.1, `{d}` on an `f128` prints the literal
        // text `(float)` once the decimal exponent leaves its fixed
        // buffer — `smallest_normal` (3.36…e-4932) and `largest_finite`
        // (1.18…e+4932) of `primitive-longdouble` both come out that
        // way, and the JSON does not even parse. `{e}` carries all
        // thirty-four digits at every magnitude.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("if (std.math.isNan(v) or std.math.isInf(v)) {".to_string());
            body.push("    return zdwire.WireError.OutOfRange;".to_string());
            body.push("}".to_string());
            body.push("try zd_out.print(zd_heap, \"\\\"{e}\\\"\", .{v});".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value.
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            body.push("try zd_out.print(zd_heap, \"\\\"{d}\\\"\", .{v});".to_string());
        }
        // The one-character string the dialect writes for a character.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("try zd_json_point(zd_heap, zd_out, v);".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push(
                "try zd_out.appendSlice(zd_heap, if (v) \"true\" else \"false\");".to_string(),
            );
        }
        TypeKind::Primitive(_) => {
            body.push("try zd_out.print(zd_heap, \"{d}\", .{v});".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("try zd_json_text(zd_heap, zd_out, v);".to_string());
        }
        // The enumerator's name, chosen by the variant: a Zig enum
        // holds no value outside its variants, so the switch is
        // exhaustive with nothing left over.
        TypeKind::Enumeration { .. } => {
            body.push("switch (v) {".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                body.push(format!(
                    "    .{} => try zd_out.appendSlice(zd_heap, {}),",
                    field_name(&node.name),
                    zig_string_literal(&format!("\"{}\"", node.name))
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
            body.push("const zd_bits: u64 = v;".to_string());
            body.push(format!(
                "if ((zd_bits & ~@as(u64, {accounted:#x})) != 0) {{"
            ));
            body.push("    // A bit this bitmask does not declare is set.".to_string());
            body.push("    return zdwire.WireError.OutOfRange;".to_string());
            body.push("}".to_string());
            body.push("try zd_out.append(zd_heap, '[');".to_string());
            body.push("var zd_first = true;".to_string());
            for (bit, position) in &declared {
                body.push(format!("if ((zd_bits >> {position}) & 1 == 1) {{"));
                body.push("    if (!zd_first) try zd_out.append(zd_heap, ',');".to_string());
                body.push("    zd_first = false;".to_string());
                body.push(format!(
                    "    try zd_out.appendSlice(zd_heap, {});",
                    zig_string_literal(&format!("\"{bit}\""))
                ));
                body.push("}".to_string());
            }
            if declared.is_empty() {
                body.push("_ = zd_first;".to_string());
            }
            body.push("try zd_out.append(zd_heap, ']');".to_string());
        }
        // The whole list, base first.
        TypeKind::Bitset { .. } => {
            body.push("try zd_out.append(zd_heap, '{');".to_string());
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
                    "try zd_out.appendSlice(zd_heap, \"{comma}\\\"{}\\\":\");",
                    escape_key(name)
                ));
                body.push(format!(
                    "try zd_out.print(zd_heap, \"{{d}}\", .{{v.{}}});",
                    field_name(name)
                ));
            }
            body.push("try zd_out.append(zd_heap, '}');".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("try zd_out.append(zd_heap, '[');".to_string());
            body.push("for (v, 0..) |zd_e, zd_i| {".to_string());
            body.push("    if (zd_i != 0) try zd_out.append(zd_heap, ',');".to_string());
            body.push(format!(
                "    try {}(zd_heap, zd_out, zd_e);",
                dumper_name(ast, element)
            ));
            body.push("}".to_string());
            body.push("try zd_out.append(zd_heap, ']');".to_string());
        }
        // One loop per dimension.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let levels = dims.len();
            let mut access = String::from("v");
            for level in 0..levels {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}try zd_out.append(zd_heap, '[');"));
                body.push(format!(
                    "{indent}for ({access}, 0..) |zd_e{level}, zd_i{level}| {{"
                ));
                body.push(format!(
                    "{indent}    if (zd_i{level} != 0) try zd_out.append(zd_heap, ',');"
                ));
                access = format!("zd_e{level}");
            }
            let deepest = "    ".repeat(levels);
            body.push(format!(
                "{deepest}try {}(zd_heap, zd_out, {access});",
                dumper_name(ast, element)
            ));
            for level in (0..levels).rev() {
                let indent = "    ".repeat(level);
                body.push(format!("{indent}}}"));
                body.push(format!("{indent}try zd_out.append(zd_heap, ']');"));
            }
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
            body.push("try zd_out.append(zd_heap, '[');".to_string());
            body.push("for (v, 0..) |zd_e, zd_i| {".to_string());
            body.push("    if (zd_i != 0) try zd_out.append(zd_heap, ',');".to_string());
            body.push("    try zd_out.append(zd_heap, '[');".to_string());
            body.push(format!(
                "    try {}(zd_heap, zd_out, zd_e.key);",
                dumper_name(ast, key_type)
            ));
            body.push("    try zd_out.append(zd_heap, ',');".to_string());
            body.push(format!(
                "    try {}(zd_heap, zd_out, zd_e.value);",
                dumper_name(ast, value_type)
            ));
            body.push("    try zd_out.append(zd_heap, ']');".to_string());
            body.push("}".to_string());
            body.push("try zd_out.append(zd_heap, ']');".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("try zd_out.append(zd_heap, '{');".to_string());
            let members = ast.all_members_of(id);
            let mut any_optional = false;
            for member in &members {
                if let Ok(member) = ast.member(*member) {
                    let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                    if member.facets.is_optional() || external {
                        any_optional = true;
                    }
                }
            }
            // A comma latch only where a key can be left out; with
            // every key always present the commas are compile-time.
            if any_optional && !members.is_empty() {
                body.push("var zd_first = true;".to_string());
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
                    zerodds_lang_rules::emit::zig::Zig,
                >(ast, id, member.node_id, &member.name);
                let key = escape_key(&member.name);
                let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                let optional = member.facets.is_optional();
                let comma = |body: &mut Vec<String>, indent: &str| {
                    if any_optional {
                        body.push(format!(
                            "{indent}if (!zd_first) try zd_out.append(zd_heap, ',');"
                        ));
                        body.push(format!("{indent}zd_first = false;"));
                    } else if !first {
                        body.push(format!("{indent}try zd_out.append(zd_heap, ',');"));
                    }
                };
                if optional {
                    body.push(format!("if (v.{field}) |zd_p| {{"));
                    comma(&mut body, "    ");
                    body.push(format!(
                        "    try zd_out.appendSlice(zd_heap, \"\\\"{key}\\\":\");"
                    ));
                    let held = if external { "zd_p.*" } else { "zd_p" };
                    body.push(format!(
                        "    try {}(zd_heap, zd_out, {held});",
                        dumper_name(ast, member_type)
                    ));
                    body.push("}".to_string());
                } else {
                    comma(&mut body, "");
                    body.push(format!(
                        "try zd_out.appendSlice(zd_heap, \"\\\"{key}\\\":\");"
                    ));
                    let held = if external {
                        format!("v.{field}.*")
                    } else {
                        format!("v.{field}")
                    };
                    body.push(format!(
                        "try {}(zd_heap, zd_out, {held});",
                        dumper_name(ast, member_type)
                    ));
                }
                first = false;
            }
            body.push("try zd_out.append(zd_heap, '}');".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("try zd_out.appendSlice(zd_heap, \"{\\\"_d\\\":\");".to_string());
            body.push(format!(
                "try {}(zd_heap, zd_out, v._d);",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch the value holds: the branches are alternatives.
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
                            tests.push(zig_label_test(ast, discriminator, value));
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
                let key = escape_key(member_name);
                let held = if external {
                    format!("v.{field}.*")
                } else {
                    format!("v.{field}")
                };
                let write_key = format!("try zd_out.appendSlice(zd_heap, \",\\\"{key}\\\":\");");
                let call = format!(
                    "try {}(zd_heap, zd_out, {held});",
                    dumper_name(ast, member_type)
                );
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
            for (index, (condition, member_name, member_type, external)) in arms.iter().enumerate()
            {
                let keyword = if index == 0 {
                    format!("if ({condition}) {{")
                } else {
                    format!("}} else if ({condition}) {{")
                };
                // Re-open the previous arm's close for the chain form.
                if index > 0 {
                    let _ = body.pop();
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
                    let _ = body.pop();
                    branch(
                        Some("} else {".to_string()),
                        &member_name,
                        member_type,
                        external,
                        &mut body,
                    );
                }
            }
            body.push("try zd_out.append(zd_heap, '}');".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Zig shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!(
        "\nfn {name}(zd_heap: std.mem.Allocator, zd_out: *std.ArrayList(u8), v: {parameter}) zdwire.WireError!void {{\n"
    );
    // A structure with no members prints `{}` and never names `v`, and
    // Zig refuses an unused function parameter outright — `empty-struct`
    // and `empty-struct-in-sequence` are the corpus cases. The same
    // assembly pass the codec emitter runs for its own locals: the
    // discard is kept where the body mentions the name nowhere, and
    // nowhere else, because Zig refuses a pointless discard too.
    if !mentions(&body, "v") {
        out.push_str("    _ = v;\n");
    }
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    out.push_str("}\n");
    Ok(out)
}

/// Whether any line of `body` names `name` as an identifier rather
/// than as a run of letters inside a longer one — `v` is mentioned by
/// `v.x` and by `zd_json_m_S(zd_heap, zd_out, v)`, and not by `zd_value`.
fn mentions(body: &[String], name: &str) -> bool {
    let ident = |c: char| c == '_' || c.is_ascii_alphanumeric();
    body.iter().any(|line| {
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back().is_none_or(|c| !ident(c));
            let after = line[at + name.len()..]
                .chars()
                .next()
                .is_none_or(|c| !ident(c));
            before && after
        })
    })
}

/// A union label test in the discriminator's Zig form: the boolean
/// literal for a `boolean`, `@intFromEnum` for an enumeration — the
/// label is the enumerator's declared value — and the number for
/// everything else.
fn zig_label_test(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_kind(ast, discriminator, 0) {
        Some(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            format!("v._d == {}", if value == 0 { "false" } else { "true" })
        }
        Some(TypeKind::Enumeration { .. }) => format!("@intFromEnum(v._d) == {value}"),
        _ => format!("v._d == {value}"),
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

/// A member name as it appears inside a generated `"…":` key — the
/// characters JSON and the Zig string literal both need escaped do not
/// occur in an IDL identifier, so this is the identity with the claim
/// written down.
fn escape_key(name: &str) -> String {
    debug_assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    name.to_string()
}
