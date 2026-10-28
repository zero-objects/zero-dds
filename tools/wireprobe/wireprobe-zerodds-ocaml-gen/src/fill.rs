// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OCaml expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-go-gen/src/fill.rs`: an OCaml
//! record literal can hold a nested aggregate, so the sample is one
//! expression the harness declares once under the sample's type — and
//! that annotation is what disambiguates every record and constructor
//! inside it.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out rather than a `null`, and a map is an array
//! of two-element arrays.
//!
//! ## What the OCaml mapping makes of each construct
//!
//! Read off `adapter/ocaml.rs`'s memory-shape table, which is what the
//! codec walks against.
//!
//! | IDL | OCaml | what this writes |
//! |---|---|---|
//! | `boolean` | `bool` | `true` / `false` |
//! | `char` | `int` | `(Zdwire.checked_char <code point>)` |
//! | `wchar` | `int` | `(Zdwire.checked_wchar <code point>)` |
//! | up to 16 bits | `int` | the number |
//! | `long` | `int32` | `<n>l` |
//! | `unsigned long`, `long long` | `int64` | `<n>L` |
//! | `unsigned long long` | `string` | the decimal, quoted |
//! | `float`, `double` | `float` | a literal, or `nan`/`infinity`/`neg_infinity` |
//! | `long double`, `fixed` | `string` | the decimal, quoted |
//! | `string`, `wstring` | `string` | a quoted literal |
//! | `sequence<T>` | `t array` | `[| … |]` |
//! | `T[N][M]` | `t array array` | nested `[| … |]`, lengths checked |
//! | `map<K,V>` | entry-record array | `{ key = k; value = v }` per entry |
//! | `enum` | a variant | the constructor |
//! | `bitmask` | `int32`/`int64` | the or of the named bits, suffixed |
//! | `struct`, `union`, `bitset` | records | literals naming every field |
//! | `@optional T` | `t option` | `Some …`, or `None` for a key left out |
//!
//! The two characters go through the runtime's `checked_char` and
//! `checked_wchar`, which refuse a code point outside the member's
//! width **at run time**: that refusal is the sample's — exit 1, the
//! verdict the Rust probe answers for the same sample.

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::ocaml::lower_ident;
use zerodds_lang_rules::adapter::ocaml::{ocaml_enumerator_name, ocaml_holder};

/// The OCaml expression `values` is, under `type_`.
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

        // A `fixed` and both string widths are an OCaml `string`; the
        // decimal must not arrive as a JSON number, because a JSON
        // number is a double and `fixed<31,10>` is exactly what one
        // cannot carry.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            Ok(ocaml_string_literal(text))
        }

        // The constructor, in the adapter's spelling; a name that is
        // not an enumerator is refused here, naming the choices.
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
                    return Ok(ocaml_enumerator_name(ast, type_, name));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, in the holder's own width.
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
            // The pattern in the holder's width; bit 31 of an `int32`
            // holder reads back negative, which is the holder's cost
            // and not the sample's.
            if ast.effective_bit_bound(type_) <= 32 {
                #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
                Ok(format!("({}l)", bits as u32 as i32))
            } else {
                #[allow(clippy::cast_possible_wrap)]
                Ok(format!("({}L)", bits as i64))
            }
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
                let suffix = match ocaml_holder(field.bits) {
                    zerodds_lang_cst::ocaml::Primitive::Int32 => "l",
                    zerodds_lang_cst::ocaml::Primitive::Int64 => "L",
                    _ => "",
                };
                parts.push(format!("{} = {number}{suffix}", lower_ident(name)));
            }
            if parts.is_empty() {
                return Ok("()".to_string());
            }
            Ok(format!("{{ {} }}", parts.join("; ")))
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
                parts.push(format!("{} = {text}", lower_ident("parent")));
            }
            let members: Vec<_> = members
                .into_iter()
                .filter(|(name, _, _)| {
                    zerodds_wireprobe_gen::fill::writes_member(inherited.as_ref(), name)
                })
                .collect();
            for (name, member_type, optional) in members {
                let member_path = format!("{path}.{name}");
                let spelled = lower_ident(&name);
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(format!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        let filled = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                        if optional {
                            parts.push(format!("{spelled} = Some ({filled})"));
                        } else {
                            parts.push(format!("{spelled} = {filled}"));
                        }
                    }
                    // Absence is the key left out: the field is an
                    // `option` and `None` is the absence.
                    None if optional => parts.push(format!("{spelled} = None")),
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            if parts.is_empty() {
                // An aggregate with no members is `unit` in this column.
                return Ok("()".to_string());
            }
            Ok(format!("{{ {} }}", parts.join("; ")))
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
                "_d = {}",
                emit(ast, discriminator, given, &format!("{path}._d"), depth + 1)?
            )];
            // Every branch is a field of the record; the ones the
            // sample does not carry are `None`.
            for case in ast.cases_of(type_) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let spelled = lower_ident(&member.name);
                match object.get(&member.name) {
                    Some(member_value) => {
                        let member_type = member.type_.resolved().ok_or_else(|| {
                            format!("{path}.{}: the case type does not resolve", member.name)
                        })?;
                        let filled = emit(
                            ast,
                            member_type,
                            member_value,
                            &format!("{path}.{}", member.name),
                            depth + 1,
                        )?;
                        parts.push(format!("{spelled} = Some ({filled})"));
                    }
                    None => parts.push(format!("{spelled} = None")),
                }
            }
            for name in object.keys() {
                if name == "_d" {
                    continue;
                }
                let known = ast
                    .cases_of(type_)
                    .iter()
                    .filter_map(|case| ast.case(*case).ok())
                    .filter_map(|case| ast.member(case.member).ok())
                    .any(|member| &member.name == name);
                if !known {
                    return Err(format!("{path}.{name}: not a case of this union"));
                }
            }
            Ok(format!("{{ {} }}", parts.join("; ")))
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
            Ok(format!("[| {} |]", parts.join("; ")))
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            let lengths: Vec<i128> = dims
                .iter()
                .map(|dim| ast.eval_integral(*dim).unwrap_or(-1))
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
                parts.push(format!("{{ key = {key_text}; value = {value_text} }}"));
            }
            Ok(format!("[| {} |]", parts.join("; ")))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no OCaml form this filler can build"
        )),
    }
}

/// One dimension of an IDL array, lengths checked against the sample.
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
    if i128::try_from(items.len()).map_err(|_| format!("{path}: length overflow"))? != *length {
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
    Ok(format!("[| {} |]", parts.join("; ")))
}

/// One primitive value, as an OCaml expression.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, String> {
    let number = |value: &Value| -> Result<i128, String> {
        // A 64-bit value arrives as a string: JSON's number is a double
        // and a double cannot hold every 64-bit integer.
        match value {
            Value::String(text) => text
                .parse::<i128>()
                .map_err(|_| format!("{path}: `{text}` is not an integer")),
            Value::Number(number) => match number.as_i64() {
                Some(v) => Ok(i128::from(v)),
                None => match number.as_u64() {
                    Some(v) => Ok(i128::from(v)),
                    None => Err(format!("{path}: not an integer")),
                },
            },
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
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16 => {
            let number = number(value)?;
            if number < 0 {
                format!("({number})")
            } else {
                format!("{number}")
            }
        }
        PrimitiveKind::Int32 => format!("({}l)", number(value)?),
        PrimitiveKind::UInt32 | PrimitiveKind::Int64 => format!("({}L)", number(value)?),
        // The decimal text: the member holds it, and `write_u64` parses
        // the full unsigned range through the `0u` prefix.
        PrimitiveKind::UInt64 => {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                other => return Err(format!("{path}: expected a number, got {other}")),
            };
            ocaml_string_literal(&text)
        }
        // The code point, checked at run time: the refusal is the
        // sample's — exit 1 — and not this generator's.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            let checker = if kind == PrimitiveKind::Char {
                "checked_char"
            } else {
                "checked_wchar"
            };
            format!("(Zdwire.{checker} {})", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let value = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = f64::from(value as f32);
            ocaml_float(narrowed)
        }
        PrimitiveKind::Float64 => {
            let value = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            ocaml_float(value)
        }
        // The decimal text; `write_f128` parses it exactly.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            ocaml_string_literal(text)
        }
    })
}

/// An OCaml floating-point spelling: a literal, or the three named
/// values with no literal. `-0.0` is a literal in OCaml and keeps its
/// sign.
fn ocaml_float(number: f64) -> String {
    if number.is_nan() {
        return "nan".to_string();
    }
    if number.is_infinite() {
        return (if number > 0.0 {
            "infinity"
        } else {
            "neg_infinity"
        })
        .to_string();
    }
    // `{:?}` is Rust's shortest round-tripping form; every spelling it
    // produces — `1.5`, `1e-45`, `5e-324`, `-0.0` — is an OCaml float
    // literal too, and always carries a `.` or an `e`, so the constant
    // is a float and not an int.
    format!("({number:?})")
}

/// An OCaml string literal of `text`'s bytes. Every byte outside
/// printable ASCII is a `\xHH` escape: a raw byte pushed as a Rust
/// `char` would be re-encoded as UTF-8 on the way into the generated
/// source and arrive doubly encoded — `string-adjacent-widths` measured
/// exactly that, a 10-byte "été" where the wire holds 6.
pub fn ocaml_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            byte if !(0x20..0x7F).contains(&byte) => out.push_str(&format!("\\x{byte:02x}")),
            byte => out.push(byte as char),
        }
    }
    out.push('"');
    out
}
