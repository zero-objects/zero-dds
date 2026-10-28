// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Elixir that turns one decoded sample back into JSON terms.
//!
//! The counterpart of `wireprobe-zerodds-python-gen/src/dump.rs`. The
//! dialect is the shared one of `crates/wire-probe/src/lib.rs`, and its
//! two unrecoverable rules are kept here: a 64-bit integer is a
//! **string**, and an absent `@optional` member is a key **left out**.
//! Not canonical form — the caller canonicalises in Rust.
//!
//! One function per type, keyed the way the codec is keyed — with one
//! Elixir-only difference from the Python column: a `typedef` and its
//! target share a dumper *name* (the tag resolves the alias), and where
//! Python quietly rebinds the second `def` over the first, two Elixir
//! definitions of one name are clauses of one function and the first
//! would shadow the second into infinite recursion. So aliases are
//! resolved during collection and never get a function of their own.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_cst::elixir::atom_name;
use zerodds_lang_render::elixir::string_literal;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::elixir::Elixir;

/// The name of the function that prints a value of `id` — the
/// decoder's name with the verb changed, so the two are keyed
/// identically.
///
/// **Aliases are resolved away first**, which is what keys a call site
/// to a definition. [`collect`] resolves them before it emits, because
/// an alias gets no function of its own; a call site that named the
/// alias took `decoder_name`'s answer for it instead, and the two
/// answers are not the same string. Measured, `sequence-of-typedef`
/// (`typedef long Metres; typedef Metres Distance;
/// sequence<Distance>`): the element call read `zd_json_long` and the
/// definition was `zd_json_p_long`, so the generated module did not
/// compile — `undefined function zd_json_long/2`. Five corpus cases
/// answered exit 7 on that, all of them a typedef chain over a
/// primitive.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Elixir>(ast, resolve_alias(ast, id, 0));
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// The type an alias chain ends at, or `id` itself.
///
/// The same walk [`collect`] does, and bounded the same way: an
/// unresolvable link or a chain past [`MAX_DEPTH`] answers with what it
/// has reached, so the caller gets a name rather than a hang.
///
/// zerodds-lint: recursion-depth 64 (typedef chain; bounded by
/// `MAX_DEPTH`)
fn resolve_alias(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Id<TypeNode> {
    if depth > MAX_DEPTH {
        return id;
    }
    let Ok(node) = ast.type_(id) else {
        return id;
    };
    match &node.kind {
        TypeKind::Alias { aliased } => aliased
            .resolved()
            .map_or(id, |target| resolve_alias(ast, target, depth + 1)),
        _ => id,
    }
}

/// How deep the reachability walk follows collections and typedefs.
const MAX_DEPTH: usize = 64;

/// The Elixir source of one dumper per type reachable from `root`,
/// indented for the inside of the codec module.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n  # One dumper per type, in the JSON dialect\n\
         \x20 # crates/wire-probe/src/lib.rs documents. Not canonical form: the\n\
         \x20 # caller canonicalises in Rust.\n",
    );
    for id in reachable {
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
}

/// Every type a value of `id` can hold, aliases resolved away —
/// see the module documentation for why an alias gets no function.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by
/// `MAX_DEPTH`)
fn collect(ast: &Ast, id: Id<TypeNode>, out: &mut Vec<Id<TypeNode>>, depth: usize) {
    if depth > MAX_DEPTH || out.contains(&id) {
        return;
    }
    let Ok(node) = ast.type_(id) else {
        return;
    };
    if let TypeKind::Alias { aliased } = &node.kind {
        if let Some(target) = aliased.resolved() {
            collect(ast, target, out, depth + 1);
        }
        return;
    }
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

/// One dumper, as a whole `def` at module indent.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        // A JSON number JSON can write, or a refusal naming the path:
        // the non-finite values travel as atoms here and have no JSON
        // form at all.
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if v == :nan or v == :infinity or v == :neg_infinity do".to_string());
            body.push(
                "  raise ArgumentError, \"#{path}: #{inspect(v)} has no JSON form\"".to_string(),
            );
            body.push("end".to_string());
            body.push("v * 1.0".to_string());
        }
        // The exact decimal, as a string: a JSON number is a double and
        // 60 of a binary128's 113 significand bits do not fit one.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("_ = path".to_string());
            body.push("ZeroddsWire.Float128.to_exact_decimal(v)".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value: a form that changed shape at 2^53 would compare
        // unequal across the boundary.
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            body.push("_ = path".to_string());
            body.push("Integer.to_string(v)".to_string());
        }
        // A boolean, a one-character binary, an integer inside a
        // double, a string and a fixed: each is already the JSON term.
        TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("_ = path".to_string());
            body.push("v".to_string());
        }
        // The enumerator's name, chosen by the value; a value with no
        // enumerator has no name and is refused.
        TypeKind::Enumeration { .. } => {
            body.push("case v do".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let Some(value) = ast.effective_enumerator_value(*enumerator) else {
                    continue;
                };
                body.push(format!("  {value} -> {}", string_literal(&node.name)));
            }
            body.push(
                "  other -> raise ArgumentError, \
                 \"#{path}: #{other} names no enumerator of this type\""
                    .to_string(),
            );
            body.push("end".to_string());
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
            body.push(format!(
                "zd_outside = Bitwise.band(v, Bitwise.bnot({accounted}))"
            ));
            body.push("if zd_outside != 0 do".to_string());
            body.push(
                "  raise ArgumentError, \
                 \"#{path}: #{zd_outside} is not a bit this bitmask declares\""
                    .to_string(),
            );
            body.push("end".to_string());
            body.push("zd_out = []".to_string());
            // Reversed build then reversed back, so the list is in
            // declaration order without an append per bit.
            for (bit, position) in declared.iter().rev() {
                body.push(format!(
                    "zd_out = if Bitwise.band(Bitwise.bsr(v, {position}), 1) == 1, \
                     do: [{} | zd_out], else: zd_out",
                    string_literal(bit)
                ));
            }
            body.push("zd_out".to_string());
        }
        // The whole list, base first: a derived bitset carries the
        // base's bitfields.
        TypeKind::Bitset { .. } => {
            let mut entries = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field.name.as_ref() else {
                    continue;
                };
                entries.push(format!(
                    "{} => v.{}",
                    string_literal(field_name),
                    atom_name(field_name)
                ));
            }
            body.push("_ = path".to_string());
            body.push(format!("%{{{}}}", entries.join(", ")));
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("v".to_string());
            body.push("|> Enum.with_index()".to_string());
            body.push(format!(
                "|> Enum.map(fn {{zd_e, zd_i}} -> {}(zd_e, \"#{{path}}[#{{zd_i}}]\") end)",
                dumper_name(ast, element)
            ));
        }
        // One `Enum.map` per dimension over the tuple's elements; an IDL
        // array is fixed-length and every element is present, so the path
        // is not indexed.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            body.push(nested_map(&dumper_name(ast, element), dims.len()));
        }
        // An array of two-element arrays, because a JSON object keys on
        // a string and a map key need not be one; the entries stay in
        // the order the value holds them.
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push("v".to_string());
            body.push("|> Enum.with_index()".to_string());
            body.push("|> Enum.map(fn {zd_e, zd_i} ->".to_string());
            body.push("  zd_where = \"#{path}[#{zd_i}]\"".to_string());
            body.push(format!(
                "  [{}(ZeroddsWire.entry_half(zd_e, 0), zd_where <> \".key\"), \
                 {}(ZeroddsWire.entry_half(zd_e, 1), zd_where <> \".value\")]",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
            body.push("end)".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("zd_out = %{}".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::elixir::Elixir,
                >(ast, id, member.node_id, &member.name);
                let key = string_literal(&member.name);
                let call = format!(
                    "{}(v.{field}, path <> {})",
                    dumper_name(ast, member_type),
                    string_literal(&format!(".{}", member.name))
                );
                if member.facets.is_optional() {
                    // An absent member is a key that is not printed;
                    // `null` would read as "present and empty" to one
                    // column and as "absent" to another.
                    body.push(format!(
                        "zd_out = if v.{field} != nil, \
                         do: Map.put(zd_out, {key}, {call}), else: zd_out"
                    ));
                } else {
                    body.push(format!("zd_out = Map.put(zd_out, {key}, {call})"));
                }
            }
            body.push("zd_out".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push(format!(
                "zd_out = %{{\"_d\" => {}(v._d, path <> \"._d\")}}",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch the value holds: the holder is one struct with a
            // field per case, and the defaulter fills the default
            // branch before the decoder runs.
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
                                "v._d == {}",
                                elixir_label(ast, discriminator, value)
                            ));
                        }
                    }
                }
                if is_default {
                    fallback = Some((member.name.clone(), member_type));
                }
                if !tests.is_empty() {
                    arms.push((tests.join(" or "), member.name.clone(), member_type));
                }
            }
            let selected = |member_name: &str, member_type: Id<TypeNode>| {
                format!(
                    "Map.put(zd_out, {}, {}(v.{}, path <> {}))",
                    string_literal(member_name),
                    dumper_name(ast, member_type),
                    atom_name(member_name),
                    string_literal(&format!(".{member_name}"))
                )
            };
            if arms.is_empty() {
                match fallback {
                    Some((member_name, member_type)) => {
                        body.push(selected(&member_name, member_type));
                    }
                    None => body.push("zd_out".to_string()),
                }
            } else {
                body.push("cond do".to_string());
                for (condition, member_name, member_type) in &arms {
                    body.push(format!("  {condition} ->"));
                    body.push(format!("    {}", selected(member_name, *member_type)));
                }
                match fallback {
                    Some((member_name, member_type)) => {
                        body.push("  true ->".to_string());
                        body.push(format!("    {}", selected(&member_name, member_type)));
                    }
                    None => {
                        body.push("  true ->".to_string());
                        body.push("    zd_out".to_string());
                    }
                }
                body.push("end".to_string());
            }
        }
        other => {
            return Err(format!(
                "{name}: {other:?} has no Elixir shape to print from"
            ));
        }
    }

    let mut out = format!("\n  def {name}(v, path) do\n");
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    out.push_str("  end\n");
    Ok(out)
}

/// A union label as an Elixir literal in the discriminator's own type —
/// the same rewrites `emit/elixir.rs`'s `cast` makes on the codec's
/// side.
fn elixir_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_primitive(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => if value == 0 { "false" } else { "true" }.to_string(),
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => u32::try_from(value)
            .ok()
            .and_then(char::from_u32)
            .map_or_else(
                // A surrogate has no Elixir binary; written as the
                // number the arm never fires on, exactly as the codec's
                // own comparison never fires on its atom.
                || format!("{value}"),
                |point| string_literal(&point.to_string()),
            ),
        _ => format!("{value}"),
    }
}

/// The primitive kind a discriminator is, once typedefs are seen
/// through.
///
/// zerodds-lint: recursion-depth 64 (typedef chain; bounded by
/// `MAX_DEPTH`)
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

/// `Enum.map(Tuple.to_list(v), fn zd_a1 -> … end)`, one level per
/// dimension. The decoder answers an array as nested tuples
/// (`ZeroddsWire.array_to_tuples`, since `fb65ea29e50`), and `Enum` takes
/// no tuple: every array cell of the matrix refresh of 2026-10-02 failed its
/// read with "protocol Enumerable not implemented for type Tuple".
fn nested_map(element: &str, dims: usize) -> String {
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
        inner = format!("Enum.map(Tuple.to_list({source}), fn zd_a{level} -> {inner} end)");
    }
    inner
}
