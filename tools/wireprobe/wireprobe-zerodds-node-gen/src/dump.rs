// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The JavaScript that turns one decoded sample back into JSON.
//!
//! The counterpart of `wireprobe-zerodds-zig-gen/src/dump.rs`, and
//! simpler in the way native `JSON.stringify` makes it: Zig's dumper
//! writes escaped text into a buffer octet by octet because Zig's
//! standard library has no value tree worth the detour, while every
//! function generated here builds a plain JavaScript value — a
//! `number`, a `string`, an `Array`, an object — and the one call at the
//! bottom of the harness, `JSON.stringify`, is the only place any of it
//! is turned into text. The dialect is the shared one:
//! `crates/wire-probe/src/lib.rs`'s — a 64-bit integer is a string, an
//! absent `@optional` member is a key left out, a map is an array of
//! two-element arrays.
//!
//! As in the Zig column, a typedef's tag follows the alias, so for most
//! targets the target's own body answers to the typedef's name; a
//! typedef of a primitive is the one case the two names differ and gets
//! a forwarding body. A refusal — a float with no JSON form, a bitmask
//! bit the declaration does not have — throws
//! `zdwire.OutOfRange`, and the harness turns it into the sample's
//! refusal, exit 1, exactly as the Rust probe answers for the same
//! case.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::node::field_name;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::node::Node;

/// The name of the function that turns a value of `id` back into JSON:
/// the decoder's name with the verb changed.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Node>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The JavaScript source of one dumper per type reachable from `root`.
///
/// `union_label` reaches the union arm, which has to ask the same
/// question the generated decoder asked — a negative label the setting
/// says selects nothing gets no test here either, so the printed member
/// is the one the decoder actually filled.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n// One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         // documents. Not canonical form: the caller canonicalises in Rust.\n\
         // JSON.stringify writes a negative zero as 0; a float dumper returns\n\
         // this placeholder instead and the harness puts \"-0\" in its place.\n\
         // A decoded string cannot hold U+0000, so no value can be mistaken\n\
         // for it.\n\
         const ZD_NEGATIVE_ZERO = \"\\u0000-0\";\n",
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
            emitted.push(name.clone());
            let _ = write!(
                out,
                "\nfunction {name}(v) {{\n  return {target_name}(v);\n}}\n"
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
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if (!Number.isFinite(v)) {".to_string());
            body.push("  // NaN and the infinities are not JSON.".to_string());
            body.push("  throw new zdwire.OutOfRange(0, \"a finite number\");".to_string());
            body.push("}".to_string());
            body.push("if (Object.is(v, -0)) {".to_string());
            body.push("  return ZD_NEGATIVE_ZERO;".to_string());
            body.push("}".to_string());
            body.push("return v;".to_string());
        }
        // The decimal in a string — the form the value dialect keeps a
        // `long double` in, because a JSON number is read back through
        // an `f64` and thirty-four significant digits do not survive
        // that.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("const zd_text = zdwire.float128ToDecimalString(v.octets);".to_string());
            body.push(
                "if (zd_text === \"inf\" || zd_text === \"-inf\" || zd_text === \"nan\" || \
                 zd_text === \"-nan\") {"
                    .to_string(),
            );
            body.push("  throw new zdwire.OutOfRange(0, \"a finite number\");".to_string());
            body.push("}".to_string());
            body.push("return zd_text;".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value.
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            body.push("return v.toString();".to_string());
        }
        // Every other primitive — `boolean`, the narrower integers, and
        // a `char`/`wchar`'s one-character string — is already a
        // JavaScript value `JSON.stringify` writes correctly on its own.
        TypeKind::Primitive(_) => {
            body.push("return v;".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return v;".to_string());
        }
        // The enumerator's name, chosen by the value: nothing here
        // trusts an out-of-range holder to have named one.
        TypeKind::Enumeration { .. } => {
            body.push("switch (v) {".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let value = ast
                    .effective_enumerator_value(*enumerator)
                    .ok_or_else(|| format!("{name}: an enumerator value does not evaluate"))?;
                body.push(format!(
                    "  case {value}: return {};",
                    json_string_literal(&enum_node.name)
                ));
            }
            body.push("  default:".to_string());
            body.push(
                "    throw new zdwire.OutOfRange(0, \"a value naming a declared enumerator\");"
                    .to_string(),
            );
            body.push("}".to_string());
        }
        TypeKind::Bitmask { .. } => {
            let bound = ast.effective_bit_bound(id);
            let wide = bound > 32;
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
            let suffix = if wide { "n" } else { "" };
            let zero = if wide { "0n" } else { "0" };
            body.push(format!("const zd_accounted = {accounted}{suffix};"));
            body.push(format!("if ((v & ~zd_accounted) !== {zero}) {{"));
            body.push("  // A bit this bitmask does not declare is set.".to_string());
            body.push(
                "  throw new zdwire.OutOfRange(0, \"no bit outside the declared set\");"
                    .to_string(),
            );
            body.push("}".to_string());
            body.push("const zd_out = [];".to_string());
            for (bit, position) in &declared {
                body.push(format!(
                    "if ((v & (1{suffix} << {position}{suffix})) !== {zero}) {{"
                ));
                body.push(format!("  zd_out.push({});", json_string_literal(bit)));
                body.push("}".to_string());
            }
            body.push("return zd_out;".to_string());
        }
        TypeKind::Bitset { .. } => {
            let mut parts = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_ident) = field.name.as_ref() else {
                    continue;
                };
                parts.push(format!(
                    "{}: v.{}",
                    json_string_literal(field_ident),
                    field_name(field_ident)
                ));
            }
            body.push(format!("return {{ {} }};", parts.join(", ")));
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push(format!(
                "return v.map((zd_e) => {}(zd_e));",
                dumper_name(ast, element)
            ));
        }
        // One nested `.map` per dimension.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            // Built inside-out: the innermost call reaches the element,
            // and each dimension outside it wraps one more `.map`,
            // outermost last so `v.map` is always the top level.
            let levels = dims.len().max(1);
            let mut call = format!("{}(zd_e{})", dumper_name(ast, element), levels - 1);
            for level in (1..levels).rev() {
                call = format!("zd_e{}.map((zd_e{level}) => {call})", level - 1);
            }
            body.push(format!("return v.map((zd_e0) => {call});"));
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
            body.push(format!(
                "return v.map((zd_e) => [{}(zd_e.key), {}(zd_e.value)]);",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("const zd_out = {};".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::node::Node,
                >(ast, id, member.node_id, &member.name);
                let key = json_string_literal(&member.name);
                let call = format!("{}(v.{field})", dumper_name(ast, member_type));
                if member.facets.is_optional() {
                    body.push(format!("if (v.{field} !== undefined) {{"));
                    body.push(format!("  zd_out[{key}] = {call};"));
                    body.push("}".to_string());
                } else {
                    body.push(format!("zd_out[{key}] = {call};"));
                }
            }
            body.push("return zd_out;".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push(format!(
                "const zd_out = {{ _d: {}(v._d) }};",
                dumper_name(ast, discriminator)
            ));
            // An if-chain over the labels and not a `switch`, the same
            // shape `wireprobe-zerodds-c-gen` and
            // `wireprobe-zerodds-typescript-gen` carry. A `switch` here
            // had no arm for `default:` — a `CaseLabel::Default` names
            // no value, so the case that carries it collected no
            // `case` label at all and its assignment became
            // unreachable text behind the previous arm's `break`. The
            // decoder had already read the branch into the sample and
            // the printer dropped it without a word: `array-1d-union`,
            // whose second element selects the `default:` arm with
            // discriminator 9, printed `{"_d":9}` while the octets
            // `c004000000000000` for `-2.5` sat on the wire and every
            // other column printed `{"_d":9,"fraction":-2.5}`.
            //
            // The one crossing `UnionLabelMatch` has, asked of the same
            // discriminator the generated decoder asked it of.
            let suppress_negative = union_negative_matches_nothing(ast, id, union_label);
            let mut arms: Vec<(String, String)> = Vec::new();
            let mut fallback: Option<String> = None;
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let member_type = member
                    .type_
                    .resolved()
                    .ok_or_else(|| format!("{name}: a union case type is unresolved"))?;
                let print = format!(
                    "zd_out[{}] = {}(v.{});",
                    json_string_literal(&member.name),
                    dumper_name(ast, member_type),
                    field_name(&member.name)
                );
                let mut condition = String::new();
                let mut is_default = false;
                for label in &case_node.labels {
                    match label.expr() {
                        None => is_default = true,
                        Some(value_expr) => {
                            let value = ast
                                .eval_integral(value_expr)
                                .map_err(|_| format!("{name}: a case label does not evaluate"))?;
                            // A label the setting says selects nothing
                            // gets no test, so the chain falls to the
                            // default where the union has one and
                            // prints only `_d` where it has none —
                            // which is what the generated decoder left
                            // in the sample.
                            if suppress_negative && value < 0 {
                                continue;
                            }
                            if !condition.is_empty() {
                                condition.push_str(" || ");
                            }
                            let _ = write!(
                                condition,
                                "v._d === {}",
                                discriminant_case_literal(ast, discriminator, value)
                            );
                        }
                    }
                }
                if is_default {
                    fallback = Some(print.clone());
                }
                if !condition.is_empty() {
                    arms.push((condition, print));
                }
            }
            for (index, (condition, print)) in arms.iter().enumerate() {
                let head = if index == 0 { "if" } else { "} else if" };
                body.push(format!("{head} ({condition}) {{"));
                body.push(format!("  {print}"));
            }
            match fallback {
                // Every label the setting suppressed: the `default:`
                // arm is the only one left and needs no test.
                Some(print) if arms.is_empty() => body.push(print),
                Some(print) => {
                    body.push("} else {".to_string());
                    body.push(format!("  {print}"));
                    body.push("}".to_string());
                }
                // A union with no `default:` and a discriminator
                // matching no label prints `_d` alone, which is what
                // the decoder left in the sample.
                None if arms.is_empty() => {}
                None => body.push("}".to_string()),
            }
            body.push("return zd_out;".to_string());
        }
        TypeKind::Alias { .. } => {
            // Reached only when the alias's own name coincides with its
            // target's tag, in which case the caller already skipped
            // emitting a forwarder; nothing calls this arm.
            body.push("return v;".to_string());
        }
        other => return Err(format!("{name}: {other:?} has no Node JSON form")),
    }
    Ok(format!("\nfunction {name}(v) {{\n{}\n}}\n", indent(&body)))
}

/// A `switch` case label for a discriminator's value, in the JavaScript
/// form the runtime value actually holds — a boolean literal, a
/// character literal or a `bigint` — so a strict `switch` comparison
/// matches it. The same table `default_node.rs`'s `discriminant_literal`
/// answers for the write direction, kept separate because the two are
/// compared against each other and a shared helper would let them agree
/// about a mistake.
fn discriminant_case_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    let resolved = resolve_alias(ast, discriminator);
    if let Ok(TypeKind::Enumeration { .. }) = ast.type_(resolved).map(|node| node.kind.clone()) {
        return value.to_string();
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => (value != 0).to_string(),
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            match u32::try_from(value).ok().and_then(char::from_u32) {
                Some(ch) => json_char_literal(ch),
                None => value.to_string(),
            }
        }
        Some(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => format!("{value}n"),
        _ => value.to_string(),
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

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_DEPTH`)
fn resolved_primitive(ast: &Ast, id: Id<TypeNode>) -> Option<PrimitiveKind> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Primitive(kind)) => return Some(kind),
            Ok(TypeKind::Alias { aliased }) => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// Indent every line of `body` by two spaces.
fn indent(body: &[String]) -> String {
    body.iter()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A JSON/JavaScript double-quoted string literal for a name known to
/// hold no character below `0x20`: an IDL identifier and an IDL
/// enumerator name are both `[A-Za-z_][A-Za-z0-9_]*` (IDL 4.2 §7.2.3),
/// so the escaping every other string in this tree needs never applies
/// here.
fn json_string_literal(text: &str) -> String {
    format!("\"{text}\"")
}

/// A one-character JavaScript string literal for a discriminator's
/// character label.
fn json_char_literal(ch: char) -> String {
    match ch {
        '\'' => "'\\''".to_string(),
        '\\' => "'\\\\'".to_string(),
        '\n' => "'\\n'".to_string(),
        '\r' => "'\\r'".to_string(),
        '\t' => "'\\t'".to_string(),
        c if (c as u32) < 0x20 => format!("'\\x{:02x}'", c as u32),
        other => format!("'{other}'"),
    }
}
