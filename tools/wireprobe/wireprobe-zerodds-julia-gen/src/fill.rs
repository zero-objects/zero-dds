// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Julia expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-python-gen/src/fill.rs`. The
//! JSON dialect is the one `crates/wire-probe/src/lib.rs` documents: a
//! 64-bit integer is a string, an absent `@optional` member is a key
//! left out rather than a `null`, and a map is an array of two-element
//! arrays.
//!
//! ## Why the sample is a scratch and one `zd_freeze`
//!
//! The declared Julia types are immutable structs with positional
//! constructors, and a positional call would transpose two same-typed
//! members the day the member order moved. The sample is therefore
//! written as the same `ZdScratch` shape the decoder assembles —
//! `zd_scratch_of("x", …)`, keyed by the escaped field names — and one
//! `zd_freeze(T, …)` at the top builds the declared value. The freeze
//! converts into every declared width on the way, so a sample outside
//! the type fails there, before any octet, with the value named.
//!
//! ## What each construct becomes
//!
//! | IDL | this writes |
//! |---|---|
//! | `boolean` | `true` / `false` |
//! | `char`, `wchar` | `UInt8(...)` / `UInt16(...)` of the code unit |
//! | every integer width | the number |
//! | `float`, `double` | a number, `NaN`, `Inf` or `-Inf` |
//! | `long double` | the decimal, as a string literal |
//! | `string`, `wstring`, `fixed` | a string literal |
//! | `sequence<T>`, `T[N]` | `Any[…]` per dimension |
//! | `map<K,V>` | `Any[zd_scratch_of("key", …, "value", …), …]` |
//! | `enum` | the flattened enumerator constant |
//! | `bitmask` | `Flags(bits)` through the declared alias |
//! | `bitset`, `struct`, `union` | `zd_scratch_of(…)` |
//! | `@optional T` | the value, or the slot left out |
//!
//! An enumerator is written as the module-scope constant the
//! materialiser bound — `a_b_RED` — through the same
//! `julia_module_prefix` and `escape_name` the adapter uses, so a
//! renamed declaration cannot part the two.

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::julia::{escape_field, escape_name};
use zerodds_lang_render::julia::string_literal;
use zerodds_lang_rules::adapter::julia::{julia_module_prefix, julia_type_name};

/// The Julia expression `values` is, under `type_` — the scratch the
/// harness's one `zd_freeze` builds the sample from.
///
/// # Errors
/// A message naming the path in the value, in the shape the Rust probe
/// reports.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, String> {
    emit(ast, type_, values, "sample", 0)
}

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// zerodds-lint: recursion-depth 64 (member, case, collection and alias
/// nesting; bounded by [`MAX_DEPTH`], which refuses with the path in
/// the sample. The walk pairs a type with the part of the sample that
/// fills it, and a recursive IDL type ends only because the sample
/// ends — a termination argument, not a bound, so the bound is the
/// constant.)
#[allow(clippy::too_many_lines)]
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
    depth: usize,
) -> Result<String, String> {
    if depth > MAX_DEPTH {
        return Err(format!("{path}: the type nests past {MAX_DEPTH} levels"));
    }
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive_literal(*kind, value, path),

        // A `fixed` and a `long double` are decimal *strings* here, and
        // a `wstring` a `String` holding UTF-8: the adapter's shapes.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            Ok(string_literal(text))
        }

        TypeKind::Enumeration { .. } => {
            let name = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected an enumerator name, as a string"))?;
            let mut declared = Vec::new();
            for id in ast.enumerators_of(type_) {
                let Ok(enumerator) = ast.enumerator(*id) else {
                    continue;
                };
                declared.push(enumerator.name.clone());
                if enumerator.name == name {
                    // The module-scope constant the materialiser bound.
                    return Ok(format!(
                        "{}{}",
                        julia_module_prefix(ast, type_),
                        escape_name(name)
                    ));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, through the declared alias so the
        // value carries the bitmask's own width.
        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array of bit names"))?;
            let mut declared = Vec::new();
            for id in ast.bits_of(type_) {
                if let Ok(bit) = ast.bit_value(*id) {
                    declared.push(bit.name.clone());
                }
            }
            let mut bits: u128 = 0;
            for entry in names {
                let name = entry
                    .as_str()
                    .ok_or_else(|| format!("{path}: a bit name is a string"))?;
                let mut found = false;
                for id in ast.bits_of(type_) {
                    let Ok(bit) = ast.bit_value(*id) else {
                        continue;
                    };
                    if bit.name != name {
                        continue;
                    }
                    let position = ast.effective_bit_position(*id).ok_or_else(|| {
                        format!("{path}: `{name}` has no position the model can state")
                    })?;
                    if position >= 64 {
                        return Err(format!("{path}: `{name}` sits at bit {position}, past 64"));
                    }
                    bits |= 1u128 << position;
                    found = true;
                    break;
                }
                if !found {
                    return Err(format!("{path}: `{name}` is not one of {declared:?}"));
                }
            }
            Ok(format!("{}({bits})", julia_type_name(ast, type_)))
        }

        TypeKind::Bitset { .. } => {
            let fields = ast.all_bitfields_of(type_);
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object keyed by bitfield name"))?;
            let declared: Vec<String> = fields
                .iter()
                .filter_map(|id| ast.bitfield(*id).ok())
                .filter_map(|field| field.name.clone())
                .collect();
            for name in object.keys() {
                if !declared.contains(name) {
                    return Err(format!("{path}.{name}: not a bitfield; {declared:?}"));
                }
            }
            let mut parts = Vec::new();
            for id in &fields {
                let Ok(field) = ast.bitfield(*id) else {
                    continue;
                };
                // An anonymous bitfield takes no value: it shifts the
                // fields above it and never appears in a sample.
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let given = object.get(name).ok_or_else(|| {
                    format!("{path}.{name}: no value, and a bitfield is never optional")
                })?;
                let number = given
                    .as_u64()
                    .ok_or_else(|| format!("{path}.{name}: expected a number"))?;
                parts.push((escape_field(name), format!("{number}")));
            }
            Ok(scratch_of(&parts))
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object"))?;
            let members: Vec<(String, Id<TypeNode>, bool)> = ast
                .all_members_of(type_)
                .iter()
                .filter_map(|id| ast.member(*id).ok())
                .filter_map(|member| {
                    member
                        .type_
                        .resolved()
                        .map(|type_| (member.name.clone(), type_, member.facets.is_optional()))
                })
                .collect();
            let declared: Vec<&str> = members.iter().map(|(name, _, _)| name.as_str()).collect();
            zerodds_wireprobe_gen::fill::refuse_undeclared(object, &declared, path)?;
            let mut parts = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3): it takes the
            // keys of its own members, and this structure the others.
            let inherited = zerodds_wireprobe_gen::fill::Inherited::of(ast, type_, object);
            if let Some(inherited) = &inherited {
                let text = emit(
                    ast,
                    inherited.base,
                    &inherited.part,
                    &format!("{path}.parent"),
                    depth + 1,
                )?;
                parts.push((escape_field("parent"), text));
            }
            let members: Vec<_> = members
                .into_iter()
                .filter(|(name, _, _)| {
                    zerodds_wireprobe_gen::fill::writes_member(inherited.as_ref(), name)
                })
                .collect();
            for (name, member_type, optional) in members {
                let member_path = format!("{path}.{name}");
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(format!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        let text = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                        parts.push((escape_field(&name), text));
                    }
                    // Absence is the slot left out: the scratch answers
                    // `nothing` for it and the freeze keeps it.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(scratch_of(&parts))
        }

        TypeKind::Union { discriminator, .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object with `_d` and the member"))?;
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{path}: the discriminator does not resolve"))?;
            let given = object
                .get("_d")
                .ok_or_else(|| format!("{path}._d: no value"))?;
            let mut parts = vec![(
                "_d".to_string(),
                emit(ast, discriminator, given, &format!("{path}._d"), depth + 1)?,
            )];
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let member = ast
                    .cases_of(type_)
                    .iter()
                    .filter_map(|case| ast.case(*case).ok())
                    .filter_map(|case| ast.member(case.member).ok())
                    .find(|member| &member.name == name)
                    .ok_or_else(|| format!("{path}.{name}: not a case of this union"))?;
                let member_type = member
                    .type_
                    .resolved()
                    .ok_or_else(|| format!("{path}.{name}: the case type does not resolve"))?;
                let text = emit(
                    ast,
                    member_type,
                    member_value,
                    &format!("{path}.{name}"),
                    depth + 1,
                )?;
                parts.push((escape_field(name), text));
            }
            Ok(scratch_of(&parts))
        }

        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            let items = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array"))?;
            let mut parts = Vec::new();
            for (index, item) in items.iter().enumerate() {
                parts.push(emit(
                    ast,
                    element,
                    item,
                    &format!("{path}[{index}]"),
                    depth + 1,
                )?);
            }
            Ok(any_vector(&parts))
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            let lengths: Vec<i128> = dims
                .iter()
                .map(|dim| ast.eval_integral(*dim).unwrap_or(0))
                .collect();
            fill_array(ast, element, value, &lengths, path, depth)
        }

        TypeKind::Map { key, value: v, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{path}: the key type does not resolve"))?;
            let value_type = v
                .resolved()
                .ok_or_else(|| format!("{path}: the value type does not resolve"))?;
            let entries = value
                .as_array()
                .ok_or_else(|| format!("{path}: a map is an array of two-element arrays"))?;
            let mut parts = Vec::new();
            for (index, pair) in entries.iter().enumerate() {
                let pair = pair
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| format!("{path}[{index}]: expected a two-element array"))?;
                let key_text = emit(
                    ast,
                    key_type,
                    &pair[0],
                    &format!("{path}[{index}].key"),
                    depth + 1,
                )?;
                let value_text = emit(
                    ast,
                    value_type,
                    &pair[1],
                    &format!("{path}[{index}].value"),
                    depth + 1,
                )?;
                parts.push(scratch_of(&[
                    ("key".to_string(), key_text),
                    ("value".to_string(), value_text),
                ]));
            }
            Ok(any_vector(&parts))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no Julia form this filler can build"
        )),
    }
}

/// One dimension of an IDL array, recursively: nested `Any[…]` with the
/// length checked against the declaration rather than taken from the
/// sample. `zd_freeze` turns the run into the declared `NTuple`.
///
/// zerodds-lint: recursion-depth 64 (one level per array dimension,
/// then the element through [`emit`]; bounded by [`MAX_DEPTH`])
fn fill_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lengths: &[i128],
    path: &str,
    depth: usize,
) -> Result<String, String> {
    let Some((length, rest)) = lengths.split_first() else {
        return emit(ast, element, value, path, depth + 1);
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("{path}: expected an array of {length}"))?;
    if i128::try_from(items.len()).unwrap_or(-1) != *length {
        return Err(format!(
            "{path}: the array holds {length} and the sample gave {}",
            items.len()
        ));
    }
    let mut parts = Vec::new();
    for (index, item) in items.iter().enumerate() {
        parts.push(fill_array(
            ast,
            element,
            item,
            rest,
            &format!("{path}[{index}]"),
            depth + 1,
        )?);
    }
    Ok(any_vector(&parts))
}

/// One primitive value, as a Julia literal in the declared width.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, String> {
    let number = |value: &Value| -> Result<i128, String> {
        // A 64-bit value arrives as a string: JSON's number is a double
        // and a double cannot hold every 64-bit integer.
        match value {
            Value::String(text) => text
                .parse::<i128>()
                .map_err(|_| format!("{path}: `{text}` is not an integer")),
            Value::Number(number) => number
                .as_i64()
                .map(i128::from)
                .ok_or_else(|| format!("{path}: not an integer")),
            other => Err(format!("{path}: expected a number, got {other}")),
        }
    };
    Ok(match kind {
        PrimitiveKind::Boolean => {
            if value
                .as_bool()
                .ok_or_else(|| format!("{path}: expected a boolean"))?
            {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // The bare number: `zd_freeze` converts it into the declared
        // width and refuses one outside it, which is the same trade the
        // runtime's own range checks make — loud, at the value.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => format!("{}", number(value)?),
        // The code unit, in the holder the adapter declared. A `char`
        // above one octet or a `wchar` outside the BMP fails at the
        // conversion — before any octet, with the value named — which
        // is the refusal the runtime would make at the write.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            let holder = if kind == PrimitiveKind::Char {
                "UInt8"
            } else {
                "UInt16"
            };
            format!("{holder}({})", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            julia_float(f64::from(number as f32))
        }
        PrimitiveKind::Float64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            julia_float(number)
        }
        // The decimal string, exactly as the corpus wrote it: the
        // member holds the text and `runtime/julia` converts to the
        // sixteen octets at the write, once.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            string_literal(text)
        }
    })
}

/// A Julia floating-point literal. `NaN`, `Inf` and `-Inf` are the
/// language's own spellings; everything else is the shortest decimal
/// that reads back as the same binary64, which is what `{:?}` prints.
fn julia_float(number: f64) -> String {
    if number.is_nan() {
        return "NaN".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 { "Inf" } else { "-Inf" }.to_string();
    }
    format!("{number:?}")
}

/// `zd_scratch_of("x", …, "y", …)`.
fn scratch_of(parts: &[(String, String)]) -> String {
    let args: Vec<String> = parts
        .iter()
        .map(|(name, value)| format!("{}, {value}", string_literal(name)))
        .collect();
    format!("zd_scratch_of({})", args.join(", "))
}

fn any_vector(parts: &[String]) -> String {
    format!("Any[{}]", parts.join(", "))
}
