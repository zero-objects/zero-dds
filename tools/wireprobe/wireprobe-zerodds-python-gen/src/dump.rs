// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Python that turns one decoded sample back into JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into the
//! generated Python. This takes one back out, after `runtime/python` and
//! the generated decoder have filled it from the bytes.
//!
//! ## The dialect is the shared one
//!
//! `crates/wire-probe/src/lib.rs` documents what a value document means
//! and [`crate::fill`] reads the same table. Two of its rules cannot be
//! recovered by any reader and so have to be right here:
//!
//! - a 64-bit integer is a **string**, because a JSON number is a double
//!   and loses the low bits past 2^53;
//! - an absent `@optional` member is a key that is **left out**, never a
//!   `null`, which the sample dialect refuses on the way in.
//!
//! It is **not** canonical form and does not try to be. Key order, float
//! spelling and map entry order are left to whatever Python produces,
//! because canonicalising here would be a second implementation of the
//! canonical form and therefore a second chance for two columns to part
//! over a spelling rather than over a value.
//!
//! ## One function per type, where the C# column writes one straight run
//!
//! `wireprobe-zerodds-csharp-gen/src/dump.rs` walks the type and emits
//! straight-line C#, and refuses a type that reaches itself by name —
//! `struct Node { sequence<Node> }` is finite in every sample and
//! infinite as a shape. This column has no such limit and needs none: a
//! Python function resolves a global at the call and not at the `def`,
//! so one function per type is enough for a recursive type as well, and
//! it is also the shape `crates/lang-python`'s `_tj_` functions take.
//! The functions are keyed the way the codec is keyed, so a name that
//! does not resolve is the keying's fault and not a spelling's.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_cst::python::escape_name;
use zerodds_lang_render::python::string_literal;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::python::Python;

/// The name of the function that prints a value of `id`.
///
/// The decoder's name with the verb changed, so the two are keyed
/// identically: `emit::decode::decoder_name` builds from the IDL kind
/// and never from the Python spelling, which is what keeps `octet` and
/// `unsigned long long` — one Python `int` between them — apart.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Python>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs. The same bound `emit/python.rs` uses.
const MAX_DEPTH: usize = 64;

/// The Python source of one dumper per type reachable from `root`.
///
/// **Deduplicated by [`dumper_name`], not by `id`.** Two distinct `id`s
/// reach the same name whenever a typedef sits between them and
/// whatever they name — `dumper_name` is keyed off `decoder_name`, and
/// `adapter::python::type_tag`'s own `Alias` arm resolves an alias
/// straight through to its target, on purpose, so that the rest of a
/// generated module can use a typedef's underlying representation
/// without knowing it is one. A second `def` under a name Python has
/// already bound at module scope does not fail to compile — `def`
/// simply rebinds the name — so the second one silently replaces the
/// first, and whichever `id` [`collect`] happened to reach last is the
/// one that wins. `emitted` is what
/// `crate::emit::decode::decoder_over` already keeps for the same
/// reason, over the same walk's decoder half.
///
/// [`function`]'s own `TypeKind::Alias` arm handles the other half of
/// this: a typedef chain three deep over a primitive gives every
/// alias in the chain the *same* `dumper_name` as the one after it, so
/// even the **first** `id` reached would emit `def zd_json_long(v,
/// path): return zd_json_long(v, path)` — a forwarder to itself,
/// `RecursionError` on the very next sample — were it not for the
/// `own_name == callee` check there, mirroring
/// `crate::emit::decode::body_of`'s own comment on exactly this shape.
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
        if emitted.contains(&name) {
            continue;
        }
        let Some(text) = function(ast, id, &name, union_label)? else {
            // The alias's own body would have forwarded to itself under
            // this same name — `function`'s `TypeKind::Alias` arm — so
            // nothing is emitted and the name stays unclaimed for
            // whichever `id` further down the chain answers it for
            // real.
            continue;
        };
        emitted.push(name);
        out.push_str(&text);
    }
    Ok(out)
}

/// Every type a value of `id` can hold, `id` included.
///
/// The **type** graph and not the value: a sample decides how long a
/// sequence is, and a dumper has to exist for its element whether the
/// sample held one or none. A type already collected is not descended
/// into again, which is what makes a recursive type terminate here.
///
/// An alias **is** pushed, under its own `id`, the same as every other
/// kind: [`dump`] is what decides which of the (possibly several) ids
/// that share one [`dumper_name`] actually gets a `def`, the same
/// division of labour `crate::emit::decode::decoder_over` and
/// `crate::emit::decode::body_of`'s own `TypeKind::Alias` arm already
/// keep in the shared decoder walk. This function only has to answer
/// "what is reachable", not "what is worth a second definition" — a
/// typedef chain three deep over a primitive is reachable through three
/// distinct alias `id`s that all resolve to the same `dumper_name`, and
/// `dump`'s own doc has the reason a walk that special-cased aliases out
/// of reachability here got that one wrong.
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

/// One dumper, as a whole `def` under `name` — [`dump`]'s own, already
/// computed, so this and the caller's dedup check read the identical
/// string.
///
/// `Ok(None)` is not a failure: it is what the `TypeKind::Alias` arm
/// answers when this `id`'s name and the name it would forward to are
/// the same, which [`dump`]'s own doc explains.
///
/// # Errors
/// A message naming the path in the type, in the same shape the fill
/// side reports one.
#[allow(clippy::too_many_lines)]
fn function(
    ast: &Ast,
    id: Id<TypeNode>,
    name: &str,
    union_label: UnionLabelMatch,
) -> Result<Option<String>, String> {
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        // A JSON number JSON can write, or a refusal naming the path.
        // `float("nan")` and the two infinities are not JSON at all —
        // `json.dumps` writes the bare tokens `NaN` and `Infinity`,
        // which no conforming parser reads — so a document carrying one
        // would fail in the caller's canonicaliser with the path lost.
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("if v != v or v in (float(\"inf\"), float(\"-inf\")):".to_string());
            body.push("    raise ValueError(f\"{path}: {v} has no JSON form\")".to_string());
            body.push("return v".to_string());
        }
        // The exact decimal and not the shortest one, as a string: a
        // JSON number is a double and 60 of a binary128's 113
        // significand bits do not fit one.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return v.to_exact_decimal()".to_string());
        }
        // A JSON number loses the low bits past 2^53, so both 64-bit
        // widths go out as decimal in a string — for every value and not
        // only the large ones, because a form that changed shape at 2^53
        // would compare unequal across the boundary.
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            body.push("return str(v)".to_string());
        }
        // A `bool`, a one-character `str`, an `int` inside a double, a
        // `string` and a `fixed`: each is already the JSON the table
        // calls for.
        TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return v".to_string());
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{name}: the typedef target is unresolved"))?;
            let callee = dumper_name(ast, target);
            // Unless the alias and what it forwards to are one dumper —
            // `adapter::python::type_tag`'s `Alias` arm resolves fully
            // through a chain, so every alias in `Range -> Distance ->
            // Metres -> long` answers `dumper_name` with the same
            // string, and the first of them to reach this arm would
            // otherwise write `def zd_json_long(v, path): return
            // zd_json_long(v, path)` — a forwarder to itself. `None`
            // leaves the name unclaimed for whichever `id` further down
            // the chain has a genuinely different one to forward to; see
            // `crate::emit::decode::body_of`'s identical check over the
            // decoder half of this same walk.
            if callee == name {
                return Ok(None);
            }
            body.push(format!("return {callee}(v, path)"));
        }
        // The enumerator's name, chosen by the value. A value with no
        // enumerator has no name, and handing the number back would put
        // an integer where every other column writes a string — which
        // compares as a difference in the *value* and would send the
        // next reader of the report looking at the wire.
        TypeKind::Enumeration { .. } => {
            let mut pairs = Vec::new();
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let Some(value) = ast.effective_enumerator_value(*enumerator) else {
                    continue;
                };
                pairs.push(format!("{value}: {}", string_literal(&node.name)));
            }
            body.push(format!("zd_names = {{{}}}", pairs.join(", ")));
            body.push("zd_found = zd_names.get(int(v))".to_string());
            body.push("if zd_found is None:".to_string());
            body.push(
                "    raise ValueError(f\"{path}: {int(v)} names no enumerator of this type\")"
                    .to_string(),
            );
            body.push("return zd_found".to_string());
        }
        // The set bits' names in declaration order. §7.4.1.1.3.2 puts
        // the bits above the bound outside the bitmask, so one declared
        // there is not part of the value and is not named here either. A
        // bit set inside the bound that the type does not declare is
        // refused rather than dropped: dropping it would answer with a
        // plausible list missing the one thing that was wrong.
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
                // Table 33 caps a bitmask at sixty-four bits, so a
                // position past that is a model the holder cannot carry.
                // Left unaccounted rather than shifted, and refused
                // below by name.
                let Some(mask) = 1u64.checked_shl(u32::from(position)) else {
                    continue;
                };
                accounted |= mask;
                declared.push((value.name.clone(), position));
            }
            body.push(format!("zd_outside = int(v) & ~{accounted:#x}"));
            body.push("if zd_outside:".to_string());
            body.push(
                "    raise ValueError(f\"{path}: {hex(zd_outside)} is not a bit this bitmask \
                 declares\")"
                    .to_string(),
            );
            body.push("zd_out = []".to_string());
            for (bit, position) in &declared {
                body.push(format!("if int(v) >> {position} & 1:"));
                body.push(format!("    zd_out.append({})", string_literal(bit)));
            }
            body.push("return zd_out".to_string());
        }
        // The whole list, base first: a derived `bitset` carries the
        // base's bitfields and the decoder fills them, so printing only
        // the derived type's own would return a value with members
        // missing and say nothing about it.
        TypeKind::Bitset { .. } => {
            let mut entries = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding: it moves the next
                // field along and carries no value, so no key.
                let Some(field_name) = field.name.as_ref() else {
                    continue;
                };
                entries.push(format!(
                    "{}: v.{}",
                    string_literal(field_name),
                    escape_name(field_name)
                ));
            }
            body.push(format!("return {{{}}}", entries.join(", ")));
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push(format!(
                "return [{}(zd_e, f\"{{path}}[{{zd_i}}]\") for zd_i, zd_e in enumerate(v)]",
                dumper_name(ast, element)
            ));
        }
        // One comprehension per dimension. §7.4.3.5 treats a
        // multi-dimensional array as one flat run with the first index
        // varying most slowly, and nested JSON arrays in written order
        // are that.
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
        // a string and a map key need not be one. The entries are left
        // in the order the value holds them; `canonical.rs` sorts, and
        // sorting a second time here would be a second answer to what
        // order means.
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push("zd_out = []".to_string());
            body.push("for zd_i, zd_e in enumerate(v):".to_string());
            body.push("    zd_where = f\"{path}[{zd_i}]\"".to_string());
            body.push(format!(
                "    zd_out.append([{}(zd_e[0], zd_where + \".key\"), \
                 {}(zd_e[1], zd_where + \".value\")])",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
            body.push("return zd_out".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("zd_out = {}".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let field = zerodds_lang_rules::emit::member_field::<
                    zerodds_lang_rules::emit::python::Python,
                >(ast, id, member.node_id, &member.name);
                let key = string_literal(&member.name);
                let where_ = format!("path + {}", string_literal(&format!(".{}", member.name)));
                let call = format!("{}(v.{field}, {where_})", dumper_name(ast, member_type));
                if member.facets.is_optional() {
                    // An absent member is a key that is not printed.
                    // `null` would read as "present and empty" to one
                    // column and as "absent" to another, and the sample
                    // dialect refuses it on the way in.
                    body.push(format!("if v.{field} is not None:"));
                    body.push(format!("    zd_out[{key}] = {call}"));
                } else {
                    body.push(format!("zd_out[{key}] = {call}"));
                }
            }
            body.push("return zd_out".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("zd_out = {}".to_string());
            body.push(format!(
                "zd_out[\"_d\"] = {}(v._d, path + \"._d\")",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every branch
            // the value holds.
            //
            // Not a choice between two readings of the clause — it is
            // what this column's *holder* forces. `adapter/python.rs`
            // declares one `typing.Optional` field per case where C has
            // one C `union`, so a write into `whole` does not clear
            // `fraction`; and `default_python`'s Table 9 defaulter fills
            // the default branch before the decoder runs. Printing every
            // field that is not `None` therefore returned the selected
            // branch *and* the defaulted one — measured on `union-final`
            // and fourteen more, where the reading differed from the
            // corpus sample in a way the octets never did.
            //
            // The same shape `wireprobe-zerodds-csharp-gen`'s
            // `emit_union` writes, and for the same reason: C# has one
            // field per case too. A union whose `_d` selects no case
            // prints `_d` alone — `union-default-unselected`.
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
                            // A label the setting says selects nothing
                            // gets no test, so the chain falls to the
                            // default where the union has one and prints
                            // `_d` alone where it has none — which is
                            // what the generated decoder left in the
                            // sample.
                            if suppress_negative && value < 0 {
                                continue;
                            }
                            tests.push(format!(
                                "v._d == {}",
                                python_label(ast, discriminator, value)
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
            let branch = |head: Option<String>,
                          member_name: &str,
                          member_type: Id<TypeNode>,
                          body: &mut Vec<String>| {
                let field = escape_name(member_name);
                let key = string_literal(member_name);
                let where_ = format!("path + {}", string_literal(&format!(".{member_name}")));
                let call = format!("{}(v.{field}, {where_})", dumper_name(ast, member_type));
                match head {
                    Some(head) => {
                        body.push(head);
                        body.push(format!("    zd_out[{key}] = {call}"));
                    }
                    None => body.push(format!("zd_out[{key}] = {call}")),
                }
            };
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 { "if" } else { "elif" };
                branch(
                    Some(format!("{keyword} {condition}:")),
                    member_name,
                    *member_type,
                    &mut body,
                );
            }
            if let Some((member_name, member_type)) = fallback {
                // With no labelled arm ahead of it the default case is
                // the only one there is, so it takes no test: an `if
                // True:` would say a question had been asked.
                let head = (!arms.is_empty()).then(|| "else:".to_string());
                branch(head, &member_name, member_type, &mut body);
            }
            body.push("return zd_out".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Python shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\n\ndef {name}(v, path):\n");
    for line in body {
        let _ = writeln!(out, "    {line}");
    }
    Ok(Some(out))
}

/// A union label as a Python literal in the discriminator's own type.
///
/// The same three rewrites `emit/python.rs`'s [`CodecTarget::cast`] does
/// on the codec's side, and for the same reason: the walk hands every
/// label over as an integer, and two of the discriminator types hold
/// something else.
///
/// - a `char` or `wchar` discriminator holds a `str`, so `v._d == 97` is
///   `False` for every sample and the label becomes the character;
/// - a `boolean` discriminator holds a `bool`. `v._d == 1` happens to be
///   true in Python — `True == 1` — but `True`/`False` is what says so.
///
/// An enumerated discriminator needs neither: `adapter/python.rs`
/// declares an `enum.IntEnum`, whose members *are* `int`s.
///
/// [`CodecTarget::cast`]: zerodds_lang_rules::emit::CodecTarget::cast
fn python_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_primitive(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => if value == 0 { "False" } else { "True" }.to_string(),
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => u32::try_from(value)
            .ok()
            .and_then(char::from_u32)
            .map_or_else(
                // A code point no `char` names cannot be compared
                // against a `str` at all. Written as the number, so the
                // arm never fires and the reading says which branch was
                // missing rather than which character was wrong.
                || format!("{value}"),
                |point| string_literal(&point.to_string()),
            ),
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

/// `[[f(b, path) for b in a] for a in v]`, one level per dimension.
///
/// The path is not indexed here, unlike the sequence case: an IDL array
/// has a fixed length and every element of it is present, so a message
/// naming the element adds nothing a reader of the sample cannot see.
fn nested_comprehension(element: &str, dims: usize) -> String {
    // No dimension the model could state: the array is its element, and
    // an empty comprehension would iterate over a value that is not a
    // sequence.
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
        inner = format!("[{inner} for zd_a{level} in {source}]");
    }
    inner
}
