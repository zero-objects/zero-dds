// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Julia that turns one decoded sample back into JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into
//! the generated Julia. This takes one back out, after
//! `runtime/julia/zerodds_wire.jl` and the generated decoder have
//! filled it from the bytes.
//!
//! The dialect is the shared one (`crates/wire-probe/src/lib.rs`): a
//! 64-bit integer is a **string**, an absent `@optional` member is a
//! key that is **left out**, a map an array of two-element arrays. It
//! is not canonical form and does not try to be — the caller
//! canonicalises in Rust, so two columns can only part over a value.
//!
//! ## The JSON is a string the functions assemble
//!
//! Julia ships no JSON writer and this probe takes no package
//! dependency, so each generated `zd_json_*` returns the JSON **text**
//! of its value and an aggregate joins its members' texts. The escapes
//! live in the runtime (`zd_jtext`, `zd_jchar`,
//! `zd_jfloat`), where every column's dumpers share one spelling.
//!
//! ## One function per type
//!
//! Keyed the way the codec is keyed — the decoder's name with the verb
//! changed — so `octet` and `unsigned long long` stay apart although
//! Julia's `string()` would print both. A recursive type needs nothing
//! special: Julia resolves a global at the call.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_cst::julia::escape_field;
use zerodds_lang_render::julia::string_literal;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::julia::Julia;

/// The name of the function that prints a value of `id`.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Julia>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The primitive a type is, with any chain of typedefs followed.
///
/// `None` where the chain ends at something that is not a primitive, or
/// where it runs past [`MAX_DEPTH`] — a bound rather than a recursion,
/// because a typedef chain is written by hand and a cyclic one would
/// otherwise not return.
fn unaliased_kind(ast: &Ast, id: Id<TypeNode>) -> Option<PrimitiveKind> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).ok()?.kind {
            TypeKind::Primitive(kind) => return Some(kind),
            TypeKind::Alias { ref aliased } => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// The Julia source of one dumper per type reachable from `root`.
///
/// # Errors
/// A message naming the path in the type, in the same shape the fill
/// side reports one.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n# One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         # documents. Not canonical form: the caller canonicalises in Rust.\n",
    );
    let mut emitted: Vec<String> = Vec::new();
    for id in reachable {
        let name = dumper_name(ast, id);
        // An alias whose dumper name sees through to its target's must
        // leave the name to the target: its own body would be a call to
        // that very name — the reading half of the walk's
        // `alias_collapses_onto_target`, and a stack overflow on the
        // first value without it (`fixed-basic`, where the typedef
        // precedes the struct that uses it).
        if let Ok(TypeKind::Alias { aliased }) = ast.type_(id).map(|node| node.kind.clone()) {
            if aliased
                .resolved()
                .is_some_and(|target| dumper_name(ast, target) == name)
            {
                continue;
            }
        }
        if emitted.contains(&name) {
            continue;
        }
        emitted.push(name);
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
}

/// Every type a value of `id` can hold, `id` included — the **type**
/// graph, because a dumper has to exist for a sequence's element
/// whether the sample held one or none.
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

/// A JSON string literal, as the Julia string literal that holds it —
/// for the keys and enumerator names the generator knows at build time.
fn json_text(text: &str) -> String {
    let mut json = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(json, "\\u{:04x}", c as u32);
            }
            c => json.push(c),
        }
    }
    json.push('"');
    string_literal(&json)
}

/// One dumper, as a whole `function … end`.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        // Through the runtime's one float spelling, which widens a
        // `Float32` first (Julia prints one with an `f0` marker no JSON
        // parser reads) and refuses NaN and the infinities by name.
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("return zd_jfloat(v)".to_string());
        }
        // The exact decimal the runtime handed back, as a string: a
        // JSON number is a double and 60 of a binary128's 113
        // significand bits do not fit one.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return zd_jtext(v)".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value: a form that changed shape at 2^53 would compare
        // unequal across the boundary.
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            body.push("return string(\"\\\"\", string(v), \"\\\"\")".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("return v ? \"true\" : \"false\"".to_string());
        }
        // The code unit as a one-character JSON string. Through the
        // runtime's escape, which spells a unit no `Char` holds as
        // `\\uXXXX` rather than constructing one.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("return zd_jchar(v)".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("return string(v)".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return zd_jtext(v)".to_string());
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{name}: the typedef target is unresolved"))?;
            body.push(format!("return {}(v, path)", dumper_name(ast, target)));
        }
        // The enumerator's name, chosen by the value; a value with no
        // enumerator has no name, and handing the number back would put
        // an integer where every other column writes a string.
        TypeKind::Enumeration { .. } => {
            body.push("zd_code = zd_i64(v)".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let Some(value) = ast.effective_enumerator_value(*enumerator) else {
                    continue;
                };
                body.push(format!(
                    "zd_code == {value} && return {}",
                    json_text(&node.name)
                ));
            }
            body.push(
                "throw(ZdOutOfRange(0, string(path, \": \", zd_code, \" names no enumerator of \
                 this type\")))"
                    .to_string(),
            );
        }
        // The set bits' names in declaration order; a bit set inside
        // the bound that the type does not declare is refused rather
        // than dropped. §7.4.1.1.3.2 puts the bits above the bound
        // outside the bitmask.
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
            body.push("zd_bits = UInt64(v)".to_string());
            body.push(format!("zd_outside = zd_bits & ~UInt64({accounted})"));
            body.push(
                "zd_outside != 0 && throw(ZdOutOfRange(0, string(path, \": \", zd_outside, \" is \
                 not a bit this bitmask declares\")))"
                    .to_string(),
            );
            body.push("zd_out = String[]".to_string());
            for (bit, position) in &declared {
                body.push(format!(
                    "(zd_bits >> {position}) & 0x01 != 0 && push!(zd_out, {})",
                    json_text(bit)
                ));
            }
            body.push("return string(\"[\", join(zd_out, \",\"), \"]\")".to_string());
        }
        // The whole list, base first: a derived bitset carries the
        // base's bitfields and the decoder fills them.
        TypeKind::Bitset { .. } => {
            body.push("zd_out = String[]".to_string());
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field.name.as_ref() else {
                    continue;
                };
                body.push(format!(
                    "push!(zd_out, string({}, \":\", string(v.{})))",
                    json_text(field_name),
                    escape_field(field_name)
                ));
            }
            body.push("return string(\"{\", join(zd_out, \",\"), \"}\")".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push(format!(
                "return string(\"[\", join([{}(zd_e, string(path, \"[\", zd_i - 1, \"]\")) for \
                 (zd_i, zd_e) in enumerate(v)], \",\"), \"]\")",
                dumper_name(ast, element)
            ));
        }
        // One comprehension per dimension: §7.4.3.5 treats a
        // multi-dimensional array as one flat run with the first index
        // varying most slowly, and nested JSON arrays in written order
        // are that. The path is not indexed: an IDL array has a fixed
        // length and every element is present.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            body.push(format!(
                "return {}",
                nested_comprehension(&dumper_name(ast, element), dims.len())
            ));
        }
        // An array of two-element arrays, because a JSON object keys on
        // a string and a map key need not be one. The entries stay in
        // the order the value holds them; the canonicaliser sorts.
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push("zd_out = String[]".to_string());
            body.push("for (zd_i, zd_e) in enumerate(v)".to_string());
            body.push("    zd_where = string(path, \"[\", zd_i - 1, \"]\")".to_string());
            body.push(format!(
                "    push!(zd_out, string(\"[\", {}(zd_e.key, string(zd_where, \".key\")), \
                 \",\", {}(zd_e.value, string(zd_where, \".value\")), \"]\"))",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
            body.push("end".to_string());
            body.push("return string(\"[\", join(zd_out, \",\"), \"]\")".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("zd_out = String[]".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::julia::Julia,
                >(ast, id, member.node_id, &member.name);
                let call = format!(
                    "{}(v.{field}, string(path, {}))",
                    dumper_name(ast, member_type),
                    string_literal(&format!(".{}", member.name))
                );
                let push = format!(
                    "push!(zd_out, string({}, \":\", {call}))",
                    json_text(&member.name)
                );
                if member.facets.is_optional() {
                    // An absent member is a key that is not printed:
                    // `null` would read as "present and empty" to one
                    // column and as "absent" to another.
                    body.push(format!("if v.{field} !== nothing"));
                    body.push(format!("    {push}"));
                    body.push("end".to_string());
                } else {
                    body.push(push);
                }
            }
            body.push("return string(\"{\", join(zd_out, \",\"), \"}\")".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("zd_out = String[]".to_string());
            body.push(format!(
                "push!(zd_out, string(\"\\\"_d\\\":\", {}(v._d, string(path, \"._d\"))))",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch the value holds: the holder is one field per case,
            // and the defaulter fills the default branch before the
            // decoder runs — printing every non-`nothing` field would
            // return the selected branch *and* the defaulted one.
            //
            // The comparisons run over `zd_i64(v._d)`: a Julia `@enum`
            // does not compare equal to a bare integer, and one numeric
            // seam covers the enum, character and boolean discriminators
            // alike.
            //
            // Except over `unsigned long long`. `zd_i64` of a `UInt64`
            // is `reinterpret`, which is the bit pattern the codec's
            // union overlay wants and the wrong number here: a label
            // above `typemax(Int64)` — DDS-XTypes puts no ceiling on
            // one — would arrive negative and select no branch, so the
            // dumper would print `_d` and drop the member. The value
            // stays unsigned and the label is a decimal literal Julia
            // widens to `Int128`, which compares by value.
            let disc_is_u64 = matches!(
                unaliased_kind(ast, discriminator),
                Some(PrimitiveKind::UInt64)
            );
            body.push(if disc_is_u64 {
                "zd_d = v._d".to_string()
            } else {
                "zd_d = zd_i64(v._d)".to_string()
            });
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
                            tests.push(format!("zd_d == {value}"));
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
            let branch = |member_name: &str, member_type: Id<TypeNode>| {
                let field = escape_field(member_name);
                format!(
                    "push!(zd_out, string({}, \":\", {}(v.{field}, string(path, {}))))",
                    json_text(member_name),
                    dumper_name(ast, member_type),
                    string_literal(&format!(".{member_name}"))
                )
            };
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 { "if" } else { "elseif" };
                body.push(format!("{keyword} {condition}"));
                body.push(format!("    {}", branch(member_name, *member_type)));
            }
            if let Some((member_name, member_type)) = fallback {
                if arms.is_empty() {
                    body.push(branch(&member_name, member_type));
                } else {
                    body.push("else".to_string());
                    body.push(format!("    {}", branch(&member_name, member_type)));
                    body.push("end".to_string());
                }
            } else if !arms.is_empty() {
                body.push("end".to_string());
            }
            body.push("return string(\"{\", join(zd_out, \",\"), \"}\")".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Julia shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\n\nfunction {name}(v, path)\n");
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    out.push_str("end\n");
    Ok(out)
}

/// `string("[", join([… for zd_a1 in v], ","), "]")`, one level per
/// dimension.
fn nested_comprehension(element: &str, dims: usize) -> String {
    if dims == 0 {
        return format!("{element}(v, path)");
    }
    let mut inner = format!("{element}(zd_a{dims}, path)");
    for level in (1..=dims).rev() {
        let source = if level == 1 {
            "v".to_string()
        } else {
            format!("zd_a{}", level - 1)
        };
        inner = format!("string(\"[\", join([{inner} for zd_a{level} in {source}], \",\"), \"]\")");
    }
    inner
}
