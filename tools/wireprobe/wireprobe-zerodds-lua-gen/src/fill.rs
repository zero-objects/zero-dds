// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Lua expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-python-gen/src/fill.rs`: a Lua
//! table constructor holds a nested aggregate, so the sample is one
//! expression the harness binds once. The JSON dialect is the one
//! `crates/wire-probe/src/lib.rs` documents: a 64-bit integer is a
//! string, an absent `@optional` member is a key left out, a map is an
//! array of two-element arrays, an enumerator is its name.
//!
//! ## What the Lua mapping makes of each construct
//!
//! Read off `crates/lang-rules/src/adapter/lua.rs`, which is the
//! authority, and `emit/lua.rs`, which the codec walks against:
//!
//! | IDL | Lua | what this writes |
//! |---|---|---|
//! | `boolean` | boolean | `true` / `false` |
//! | `char`, `wchar` | integer | the code point, as a number |
//! | integers to 64 bits | integer | the number; above 2^63 − 1 a hex literal, which wraps to the exact bit pattern |
//! | `float`, `double` | float | a literal `math.type` calls `float` |
//! | `long double` | string | the decimal text, converted at the octet |
//! | `string`, `wstring`, `fixed` | string | a quoted literal |
//! | `sequence<T>`, `T[N]` | 1-based table | a table constructor per dimension |
//! | `map<K,V>` | array of pairs | `{ { key = k, value = v }, … }` |
//! | `enum` | integer | the enumerator's **value** |
//! | `bitmask` | integer | the or of the named bits |
//! | `bitset` | table | one key per bitfield |
//! | `struct`, `union` | table | keyed entries, a keyword key in the bracket form |
//! | `@optional T` | the key, or no key | the value, or the entry left out |
//!
//! ## Why an enumerator becomes its value and not a name
//!
//! The declared path is the materialiser's to compute and a rule
//! application may rename it; the **value** is the model's and needs no
//! naming — `ast.effective_enumerator_value` is the same function the
//! materialiser writes the descriptor from, so a wrong constant shows
//! up as a mismatch in the octets rather than as a name that does not
//! resolve.

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::lua::is_identifier;
use zerodds_lang_render::lua::escape_string;

/// The Lua expression `values` is, under `type_`.
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

/// A Lua string literal.
fn quoted(text: &str) -> String {
    format!("\"{}\"", escape_string(text))
}

/// A table entry's key, in whichever of the two forms Lua accepts:
/// `name = …` for an identifier that is not a keyword, `["name"] = …`
/// otherwise. The key is the same string either way.
fn keyed(name: &str, value: &str) -> String {
    if is_identifier(name) {
        return format!("{name} = {value}");
    }
    format!("[{}] = {value}", quoted(name))
}

/// A whole number as the literal that keeps it an integer: decimal
/// inside a signed 64-bit range, hexadecimal above it — a decimal
/// numeral past 2^63 − 1 denotes a *float* (Lua 5.4 reference §3.1)
/// and a hexadecimal one wraps to the exact bit pattern. −2^63 itself
/// takes the hex form too: `-9223372036854775808` is unary minus over
/// a numeral that is already a float before the minus applies.
fn integer_literal(value: i128, path: &str) -> Result<String, String> {
    if value == i128::from(i64::MIN) {
        return Ok("0x8000000000000000".to_string());
    }
    if i64::try_from(value).is_ok() {
        return Ok(format!("{value}"));
    }
    if let Ok(unsigned) = u64::try_from(value) {
        return Ok(format!("0x{unsigned:016X}"));
    }
    Err(format!("{path}: {value} is outside a 64-bit Lua integer"))
}

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

        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            Ok(quoted(text))
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
                    return integer_literal(number, path);
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, as one integer — a 64-bit Lua
        // integer carries a `@bit_bound(64)` mask exactly, as the
        // pattern.
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
            let mut bits: u64 = 0;
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
                    bits |= 1u64 << position;
                    found = true;
                    break;
                }
                if !found {
                    return Err(format!("{path}: `{name}` is not one of {declared:?}"));
                }
            }
            integer_literal(i128::from(bits), path)
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
                // An anonymous bitfield is padding and takes no value.
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let given = object.get(name).ok_or_else(|| {
                    format!("{path}.{name}: no value, and a bitfield is never optional")
                })?;
                let number = given
                    .as_u64()
                    .ok_or_else(|| format!("{path}.{name}: expected a number"))?;
                parts.push(keyed(name, &format!("{number}")));
            }
            Ok(format!("{{ {} }}", parts.join(", ")))
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
                parts.push(keyed("parent", &text));
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
                        parts.push(keyed(&name, &text));
                    }
                    // In Lua the absence of a value is the absence of
                    // the key, so the entry is simply not written.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(format!("{{ {} }}", parts.join(", ")))
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
            let mut parts = vec![keyed(
                "_d",
                &emit(ast, discriminator, given, &format!("{path}._d"), depth + 1)?,
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
                parts.push(keyed(name, &text));
            }
            Ok(format!("{{ {} }}", parts.join(", ")))
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
                parts.push(format!("{{ key = {key_text}, value = {value_text} }}"));
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
            "{path}: {other:?} has no Lua form this filler can build"
        )),
    }
}

/// One dimension of an IDL array, recursively: nested 1-based tables,
/// the length checked against the declaration rather than taken from
/// the sample.
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
    Ok(list_literal(&parts))
}

/// One primitive value, as a Lua literal.
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
                .or_else(|| number.as_u64().map(i128::from))
                .ok_or_else(|| format!("{path}: not an integer")),
            other => Err(format!("{path}: expected a number, got {other}")),
        }
    };
    match kind {
        PrimitiveKind::Boolean => Ok(
            if value
                .as_bool()
                .ok_or_else(|| format!("{path}: expected a boolean"))?
            {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => integer_literal(number(value)?, path),
        // The code point, as the integer the member holds. The refusal
        // of one outside the type's range is `runtime/lua`'s, at the
        // octet, where it belongs.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            Ok(format!("{}", u32::from(one)))
        }
        PrimitiveKind::Float32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            Ok(lua_float(f64::from(number as f32)))
        }
        PrimitiveKind::Float64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            Ok(lua_float(number))
        }
        // The decimal text, parsed by `runtime/lua` exactly: a Lua
        // float is a binary64 and cannot carry a binary128, and a
        // generator that computed the octets here would be the Rust
        // column answering for this one.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            Ok(quoted(text))
        }
    }
}

/// A Lua floating-point literal that `math.type` calls a float.
///
/// `{:?}` is the shortest round-tripping decimal; a form with no radix
/// point and no exponent would denote an *integer* (reference §3.1),
/// so `.0` is appended where needed. The three values with no literal
/// are spelled as the expressions that produce them.
fn lua_float(number: f64) -> String {
    if number.is_nan() {
        return "(0 / 0)".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 {
            "math.huge".to_string()
        } else {
            "-math.huge".to_string()
        };
    }
    let text = format!("{number:?}");
    if text.contains('.') || text.contains('e') || text.contains('E') || text.contains("inf") {
        return text;
    }
    format!("{text}.0")
}

fn list_literal(parts: &[String]) -> String {
    if parts.is_empty() {
        return "{}".to_string();
    }
    format!("{{ {} }}", parts.join(", "))
}
