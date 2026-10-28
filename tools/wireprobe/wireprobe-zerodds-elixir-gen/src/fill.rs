// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Elixir expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-python-gen/src/fill.rs`: an
//! Elixir literal can hold a nested aggregate, so the sample is one
//! expression the harness binds once. The JSON dialect is the one
//! `crates/wire-probe/src/lib.rs` documents: a 64-bit integer is a
//! string, an absent `@optional` member is a key left out rather than a
//! `null`, and a map is an array of two-element arrays.
//!
//! ## What the Elixir mapping makes of each construct
//!
//! Read off `emit/elixir.rs`'s `value_type_of` — what the codec walks
//! against — and `adapter/elixir.rs`, which declares it. Three rows
//! differ from the declaration on purpose, and the codec, this filler
//! and the dumper agree on all three:
//!
//! - an **array** is nested lists, not the declared tuples — the reader
//!   builds it element by element and a tuple cannot be written into;
//! - a **map entry** is a two-element list, not the declared two-tuple,
//!   for the same reason at the entry's two halves;
//! - a **non-finite float** is one of the atoms `:nan`, `:infinity`,
//!   `:neg_infinity` — an Erlang float cannot hold the value itself.
//!
//! An enumerator becomes its **value**: the binding holds enumerations
//! as integers, and `ast.effective_enumerator_value` is the same
//! function the materialiser writes the constant from.

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::elixir::atom_name;
use zerodds_lang_render::elixir::string_literal;
use zerodds_lang_rules::adapter::elixir::type_module_path;

/// The Elixir expression `values` is, under `type_`.
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

/// zerodds-lint: recursion-depth 64 (member and collection nesting;
/// bounded by `MAX_DEPTH`)
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

        // A `fixed` is a binary holding the decimal text: the digit
        // count is a measured axis and a holder that normalised leading
        // zeros away would have answered it.
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
                    let number = ast.effective_enumerator_value(*id).ok_or_else(|| {
                        format!("{path}: `{name}` has no value the model can state")
                    })?;
                    return Ok(format!("{number}"));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, as one integer — the binding holds
        // a bitmask as a plain integer at every bound.
        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array of bit names"))?;
            let mut declared = Vec::new();
            for id in ast.bits_of(type_) {
                let Ok(bit) = ast.bit_value(*id) else {
                    continue;
                };
                declared.push(bit.name.clone());
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
            Ok(format!("{bits}"))
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
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let given = object.get(name).ok_or_else(|| {
                    format!("{path}.{name}: no value, and a bitfield is never optional")
                })?;
                let number = given
                    .as_u64()
                    .ok_or_else(|| format!("{path}.{name}: expected a number"))?;
                parts.push(format!("{}: {number}", atom_name(name)));
            }
            Ok(construct(ast, type_, &parts))
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
                parts.push(format!("{}: {text}", atom_name("parent")));
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
                        parts.push(format!("{}: {text}", atom_name(&name)));
                    }
                    // Absence is the key left out: the `defstruct`
                    // default is already `nil`.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(construct(ast, type_, &parts))
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
            let mut parts = vec![format!(
                "_d: {}",
                emit(ast, discriminator, given, &format!("{path}._d"), depth + 1)?
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
                parts.push(format!("{}: {text}", atom_name(name)));
            }
            Ok(construct(ast, type_, &parts))
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
            Ok(list_literal(&parts))
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
                // A two-element **list**: the codec's own entry shape,
                // whose halves the read side writes one at a time.
                parts.push(format!("[{key_text}, {value_text}]"));
            }
            Ok(list_literal(&parts))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no Elixir form this filler can build"
        )),
    }
}

/// One dimension of an IDL array, recursively — nested lists, one level
/// per dimension, the length checked against the declaration.
///
/// zerodds-lint: recursion-depth 64 (array dimensions; bounded by
/// `MAX_DEPTH`)
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
    Ok(list_literal(&parts))
}

/// One primitive value, as an Elixir literal.
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
        // Every integer width is one unbounded integer; the runtime
        // range-checks at the octet.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => format!("{}", number(value)?),
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            // The character whole; `Writer.write_char` does the
            // refusing, which is where it belongs.
            string_literal(&one.to_string())
        }
        // The value is rounded to single precision here, once, so the
        // literal in the file is what the wire will carry — the same
        // decision the Python filler writes down.
        PrimitiveKind::Float32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            elixir_float(f64::from(number as f32))
        }
        PrimitiveKind::Float64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            elixir_float(number)
        }
        // Parsed by the runtime from the decimal the corpus gave:
        // Rust's `f64` cannot hold a binary128, so a generator that
        // computed the octets here would be the Rust column answering
        // for this one.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            format!(
                "ZeroddsWire.Float128.from_decimal({})",
                string_literal(text)
            )
        }
    })
}

/// An Elixir floating-point literal — or the atom a value an Erlang
/// float cannot hold travels as.
fn elixir_float(number: f64) -> String {
    if number.is_nan() {
        return ":nan".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 {
            ":infinity"
        } else {
            ":neg_infinity"
        }
        .to_string();
    }
    // `{:?}` is Rust's shortest round-tripping form; it always carries
    // a `.` or an `e`, and Elixir's float grammar wants a digit on both
    // sides of the point, which `1e10`-shaped output would break — so
    // the exponent form is normalised.
    let text = format!("{number:?}");
    if let Some(at) = text.find(['e', 'E']) {
        let (mantissa, exponent) = text.split_at(at);
        let mantissa = if mantissa.contains('.') {
            mantissa.to_string()
        } else {
            format!("{mantissa}.0")
        };
        format!("{mantissa}{exponent}")
    } else {
        text
    }
}

/// A struct literal, its module path the materialiser's own.
fn construct(ast: &Ast, type_: Id<TypeNode>, parts: &[String]) -> String {
    let module = type_module_path(ast, type_)
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_string())
        .collect::<Vec<String>>()
        .join(".");
    format!("%{module}{{{}}}", parts.join(", "))
}

fn list_literal(parts: &[String]) -> String {
    format!("[{}]", parts.join(", "))
}
