// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust expression that builds one sample of an HDDS-generated type.
//!
//! The C and C++ emitters beside this one write *statements*, because
//! their generated APIs are accessor pairs and `resize` calls that have
//! to run in order. `hddsgen` emits plain structs with `pub` fields, so
//! a whole sample is one expression and the probe template only has to
//! bind it.
//!
//! Every shape here was measured against `hddsgen gen rust` output
//! rather than assumed:
//!
//! | IDL | what `hddsgen` emits |
//! |---|---|
//! | `module outer { module inner {` | `pub mod outer { pub mod inner {` |
//! | `enum Color { RED }` | `pub enum Color { RED = 0 }` |
//! | `long arr[3]` | `pub arr: [i32; 3]` |
//! | `@optional long maybe` | `pub maybe: Option<i32>` |
//! | `wstring wide` | `pub wide: String` |
//! | `char letter` | `pub letter: char` |
//!
//! The last two rows are worth keeping in view. A `wstring` becomes a
//! Rust `String`, and a one-octet IDL `char` becomes a four-octet Rust
//! `char`. This emitter does not correct for either — it builds what the
//! generated type takes, and the wire comparison is what reports the
//! difference. The `char` row was assumed to be `u8` when this file was
//! written and the compiler said otherwise; it is measured now.

use core::fmt::Write as _;

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_wire_codec::encode::ProbeError;

use crate::contract;
use crate::target_keywords::rust as rust_identifier;

/// The Rust expression that constructs `type_` from `values`.
///
/// # Errors
/// [`ProbeError::Mismatch`] when a value does not fit the type it is
/// against, [`ProbeError::Missing`] for a member with no value,
/// [`ProbeError::Unknown`] for a key naming no member, and
/// [`ProbeError::Unsupported`] for a construct `hddsgen` does not
/// generate a Rust shape for.
pub fn fill_hdds(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, ProbeError> {
    // Before the first hop; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    expression(ast, type_, values, "")
}

/// The path a generated type is reached by: `outer::inner::Sample`.
///
/// `hddsgen` nests one `pub mod` per IDL module and does not flatten or
/// prefix, so the scoped name is the Rust path with no transformation.
pub fn rust_path(ast: &Ast, type_: Id<TypeNode>) -> String {
    ast.any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .map_or_else(
            || String::from("UNKNOWN"),
            |scoped| scoped.segments().join("::"),
        )
}

/// zerodds-lint: recursion-depth 128 — the sample's own nesting. Every arm
/// but the typedef one descends into a strictly smaller part of the value,
/// so the walk ends because the sample is finite: a recursive type has
/// finite values, because the sequence that closes its cycle is eventually
/// empty. The typedef arm keeps the value and is finite because
/// `contract::reject_alias_cycle` at the entry point has refused a chain
/// that returns to itself. A value file is `serde_json`'s to parse and its
/// `RECURSION_LIMIT` is 128.
fn expression(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<String, ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive(*kind, value, path),
        TypeKind::String { width, .. } => {
            let text = contract::text(value, path)?;
            // Both widths land in `String`; see the module note.
            let _ = width;
            Ok(format!("String::from({})", rust_string(text)))
        }
        TypeKind::Enumeration { .. } => {
            let (name, _) = contract::enumerator(ast, type_, value, path)?;
            Ok(format!("{}::{name}", rust_path(ast, type_)))
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            expression(ast, target, value, path)
        }
        TypeKind::Structure { .. } => structure(ast, type_, value, path),
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let items = contract::bounded_items(ast, value, *bound, "elements", path)?;
            let mut out = String::from("vec![");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                let item_path = contract::join(path, &index.to_string());
                out.push_str(&expression(ast, element, item, &item_path)?);
            }
            out.push(']');
            Ok(out)
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            array(ast, element, value, path, &counts)
        }
        // Measured against the cells' own `generated.rs`: the current
        // hddsgen emits a data-carrying Rust enum for a union — variants
        // named UpperCamel after the members, one payload each, the
        // discriminator baked into each variant's encoder — so the fill
        // constructs exactly that. (The earlier refusal here claimed
        // hddsgen "does not parse" unions; the cells' idlc.log shows
        // exit 0 and the enum in the output, so the claim was stale.)
        TypeKind::Union { .. } => {
            let object = contract::object(value, path)?;
            let _ = contract::discriminator(object, path)?;
            let mut picked: Option<(String, Id<TypeNode>, &Value)> = None;
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let Some(member_type) = union_member_type(ast, type_, name) else {
                    return Err(ProbeError::Unknown {
                        path: contract::join(path, name),
                        declared: union_member_names(ast, type_),
                    });
                };
                // Refused, not overwritten. The C and Fast DDS fills
                // emit a statement for every member key the sample
                // names and pin `_d` after them; this backend builds
                // one enum variant and can carry exactly one payload,
                // so the assignment that stood here kept the last key
                // of the object and dropped every earlier one — a
                // sample that named two branches was recorded as
                // though it had named one, with nothing said.
                if let Some((first, _, _)) = &picked {
                    return Err(ProbeError::Mismatch {
                        path: path.to_string(),
                        expected: "one union member".to_string(),
                        found: format!("`{first}` and `{name}`"),
                    });
                }
                picked = Some((name.to_string(), member_type, member_value));
            }
            match picked {
                Some((name, member_type, member_value)) => {
                    let payload =
                        expression(ast, member_type, member_value, &contract::join(path, &name))?;
                    Ok(format!(
                        "{}::{}({payload})",
                        rust_path(ast, type_),
                        upper_camel(&name)
                    ))
                }
                // hddsgen's enum has no unselected state to spell.
                None => Err(unsupported(
                    path,
                    "a union selecting no member, which hddsgen's enum cannot hold",
                )),
            }
        }
        // Measured against the cells' own `generated.rs`: hddsgen
        // parses a bitset and emits `pub struct <Name> { pub bits: u64 }`
        // with a `set_<field>` accessor per bitfield (SHIFT/MASK
        // constants of its own), and its encoder writes the holder as a
        // full 8-byte `u64` whatever the declared widths sum to — a
        // wire divergence the cells exist to record, not to hide. The
        // fill goes through the generated setters so the packing stays
        // hddsgen's. (The earlier refusal here claimed hddsgen "does
        // not parse" bitsets; the cells' idlc.log shows exit 0 and the
        // struct in the output, so the claim was stale.)
        TypeKind::Bitset { .. } => {
            let object = contract::object(value, path)?;
            let declared = contract::bitfield_names(ast, &ast.all_bitfields_of(type_));
            contract::reject_unknown_keys(object, &declared, path)?;
            let mut out = format!("{{ let mut zd_b = {} {{ bits: 0 }};", rust_path(ast, type_));
            for name in &declared {
                let field_path = contract::join(path, name);
                let Some(field_value) = object.get(name) else {
                    return Err(ProbeError::Missing { path: field_path });
                };
                let raw = contract::integer(field_value, &field_path)?;
                let _ = write!(out, " zd_b.set_{name}({raw}u64);");
            }
            out.push_str(" zd_b }");
            Ok(out)
        }
        // Measured, not assumed: `hddsgen gen rust` takes
        // `bitmask-bitbound-32` and writes Rust for it, so this is
        // where the gap is and not in the generator. It sat here as a
        // bare `"a bitmask"` beside three neighbours that each name
        // whose limit they are.
        // Measured against the cells' own `generated.rs`: hddsgen maps
        // a bitmask to `pub type <Name> = u64;` with one
        // `<NAME>_<FLAG>` constant per flag (`1u64 << position`), so
        // the fill ORs the named constants and spells no positions of
        // its own. An empty selection is the literal zero.
        TypeKind::Bitmask { .. } => {
            let names = contract::bit_names(ast, type_, value, path)?;
            if names.is_empty() {
                return Ok("0u64".to_string());
            }
            let scoped = rust_path(ast, type_);
            let (prefix, type_name) = match scoped.rsplit_once("::") {
                Some((module, name)) => (format!("{module}::"), name.to_string()),
                None => (String::new(), scoped),
            };
            let constant_stem = type_name.to_uppercase();
            let flags: Vec<String> = names
                .iter()
                .map(|(name, _)| format!("{prefix}{constant_stem}_{}", name.to_uppercase()))
                .collect();
            Ok(format!("({})", flags.join(" | ")))
        }
        // Measured in the cells' `generated.rs`: hddsgen maps `map<K,V>`
        // to `std::collections::HashMap<K, V>` and emits an encoder for
        // it, so the fill builds the literal. (The earlier "shape is
        // unmeasured" refusal predated that output.)
        TypeKind::Map {
            key,
            value: entry,
            bound,
        } => {
            let key_type = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map key type is unresolved"),
            })?;
            let value_type = entry.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map value type is unresolved"),
            })?;
            let entries = contract::bounded_items(ast, value, *bound, "entries", path)?;
            let mut out = String::from("std::collections::HashMap::from([");
            for (index, pair) in entries.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                let entry_path = format!("{path}[{index}]");
                let (entry_key, entry_value) = contract::map_entry(pair, &entry_path)?;
                let key_expr = expression(
                    ast,
                    key_type,
                    entry_key,
                    &contract::join(&entry_path, "key"),
                )?;
                let value_expr = expression(
                    ast,
                    value_type,
                    entry_value,
                    &contract::join(&entry_path, "value"),
                )?;
                let _ = write!(out, "({key_expr}, {value_expr})");
            }
            out.push_str("])");
            Ok(out)
        }
        _ => Err(unsupported(
            path,
            "a construct with no measured hddsgen shape",
        )),
    }
}

fn structure(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<String, ProbeError> {
    let object = contract::object(value, path)?;
    // Checked against every member the type has, a base's included,
    // because the sample is flat: `{ "a": 1, "b": 2, "c": 3 }` for a
    // three-deep hierarchy. A key naming no member anywhere is an error
    // and not a skip — it would otherwise leave a field at its default
    // and still produce bytes.
    let declared: Vec<String> = contract::members(ast, type_)
        .iter()
        .map(|member| member.name.clone())
        .collect();
    contract::reject_unknown_keys(object, &declared, path)?;
    literal(ast, type_, value, path)
}

/// One struct literal, with an inherited base nested in a `base` field.
///
/// `hddsgen` does not flatten inheritance: `struct Mid : Base` becomes
/// `pub struct Mid { pub base: Base, pub b: i32 }`. The sample stays
/// flat, so each level reads the keys its own members name and hands the
/// same object down.
fn literal(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<String, ProbeError> {
    let object = contract::object(value, path)?;
    let mut out = format!("{} {{ ", rust_path(ast, type_));
    let mut written = 0usize;

    if let Ok(node) = ast.type_(type_) {
        if let TypeKind::Structure {
            base: Some(base), ..
        } = &node.kind
        {
            let base = base.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the base type is unresolved"),
            })?;
            let _ = write!(out, "base: {}", literal(ast, base, value, path)?);
            written += 1;
        }
    }

    let members: Vec<contract::Member> = contract::members(ast, type_)
        .into_iter()
        .filter(|member| {
            ast.members_of(type_)
                .iter()
                .any(|own| ast.member(*own).is_ok_and(|m| m.node_id == member.node))
        })
        .collect();

    for member in &members {
        if written > 0 {
            out.push_str(", ");
        }
        written += 1;
        let member_path = contract::join(path, &member.name);
        // Rust has no partial struct literal, so every field is written
        // whether or not the sample carries it. An absent `@optional`
        // is `None` — which is the value, per XTypes 1.3 §7.4.1.2.3,
        // and not a stand-in for one.
        let rendered = match contract::member_value(object, member, &member_path)? {
            Some(member_value) => {
                let inner = expression(ast, member.type_, member_value, &member_path)?;
                if member.optional {
                    format!("Some({inner})")
                } else {
                    inner
                }
            }
            None => String::from("None"),
        };
        // The field name as hddsgen spells it: a Rust keyword is a raw
        // identifier there, and the bare word does not parse.
        let _ = write!(out, "{}: {rendered}", rust_identifier(&member.name));
    }
    out.push_str(" }");
    Ok(out)
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `expression`, which is bounded by the sample.
fn array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    path: &str,
    counts: &[usize],
) -> Result<String, ProbeError> {
    let Some((count, rest)) = counts.split_first() else {
        return expression(ast, element, value, path);
    };
    let items = contract::array(value, path)?;
    if items.len() != *count {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("{count} elements"),
            found: format!("{}", items.len()),
        });
    }
    let mut out = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        let item_path = contract::join(path, &index.to_string());
        out.push_str(&array(ast, element, item, &item_path, rest)?);
    }
    out.push(']');
    Ok(out)
}

fn primitive(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, ProbeError> {
    match kind {
        PrimitiveKind::Boolean => {
            let flag = contract::boolean(value, path)?;
            Ok(if flag {
                String::from("true")
            } else {
                String::from("false")
            })
        }
        // Measured: `hddsgen` renders both `char` and `wchar` as a Rust
        // `char`, so a one-octet IDL `char` is a four-octet Rust scalar
        // in the generated struct. What that means on the wire is the
        // comparison's business, not this emitter's — it writes what the
        // generated field takes.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let character = contract::character_for(kind, value, path)?;
            Ok(format!("'\\u{{{:x}}}'", u32::from(character)))
        }
        PrimitiveKind::Float32 => {
            let number = contract::float(value, path)?;
            Ok(format!("{number:?}f32"))
        }
        PrimitiveKind::Float64 => {
            let number = contract::float(value, path)?;
            Ok(format!("{number:?}f64"))
        }
        PrimitiveKind::Float128 => Err(unsupported(
            path,
            "a long double, which has no Rust primitive",
        )),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => integer_literal(kind, value, path, "u8"),
        PrimitiveKind::Int8 => integer_literal(kind, value, path, "i8"),
        PrimitiveKind::Int16 => integer_literal(kind, value, path, "i16"),
        PrimitiveKind::UInt16 => integer_literal(kind, value, path, "u16"),
        PrimitiveKind::Int32 => integer_literal(kind, value, path, "i32"),
        PrimitiveKind::UInt32 => integer_literal(kind, value, path, "u32"),
        PrimitiveKind::Int64 => integer_literal(kind, value, path, "i64"),
        PrimitiveKind::UInt64 => integer_literal(kind, value, path, "u64"),
        // No catch-all: a primitive kind added later is a compile error
        // here rather than a refusal that reads as an HDDS limit.
    }
}

fn integer_literal(
    kind: PrimitiveKind,
    value: &Value,
    path: &str,
    suffix: &str,
) -> Result<String, ProbeError> {
    let number = contract::integer_for(kind, value, path)?;
    Ok(format!("{number}{suffix}"))
}

/// A Rust string literal. Escaped rather than pasted: a sample may hold
/// a quote, a backslash or a control character, and a literal built by
/// concatenation would stop compiling instead of producing bytes.
fn rust_string(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                let _ = write!(out, "\\u{{{:x}}}", other as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn unsupported(path: &str, kind: &'static str) -> ProbeError {
    ProbeError::Unsupported {
        path: path.to_string(),
        kind,
    }
}

/// The UpperCamel spelling hddsgen gives a union member's variant —
/// measured: `whole` becomes `Whole`, `fraction` becomes `Fraction`.
///
/// Shared with [`crate::dump_rust`]: the two halves name the same
/// variant, and a dump that guessed the spelling would not compile
/// against the type the fill built.
pub(crate) fn upper_camel(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The member's type in a union, by its IDL name.
fn union_member_type(ast: &Ast, union: Id<TypeNode>, name: &str) -> Option<Id<TypeNode>> {
    ast.cases_of(union)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .filter_map(|case| ast.member(case.member).ok())
        .find(|member| member.name == name)
        .and_then(|member| member.type_.resolved())
}

/// Every member name a union declares, for the unknown-key error.
fn union_member_names(ast: &Ast, union: Id<TypeNode>) -> Vec<String> {
    ast.cases_of(union)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .filter_map(|case| ast.member(case.member).ok())
        .map(|member| member.name.clone())
        .collect()
}
