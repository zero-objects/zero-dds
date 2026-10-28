// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Node (JavaScript) expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-zig-gen/src/fill.rs`, and
//! simpler than it in two ways that are the language's and not a
//! shortcut taken here:
//!
//! - **No allocator to route anything through.** Zig's filler calls
//!   `zdwire.zdSlice` for every sequence and `zdwire.zdPtr` for every
//!   `@external` member because Zig has to be told where the storage
//!   comes from; a JavaScript array literal and a plain object literal
//!   already own theirs, so [`emit`] never reaches for either helper.
//! - **No result-location typing.** Zig's `zdwire.zdPtr(zd_heap, @as(T, …))`
//!   names `T` because an anonymous `.{ … }` literal handed to a
//!   function has no type to infer; nothing here is ever handed to a
//!   function that needs telling.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out, a map is an array of two-element arrays.
//!
//! | IDL | Node | what this writes |
//! |---|---|---|
//! | `boolean` | `boolean` | `true` / `false` |
//! | `char` / `wchar` | one-character `string` | the character literal — checked at the write call, not here |
//! | integers to 32 bits | `number` | the bare number |
//! | the two 64-bit integers | `bigint` | the number with a trailing `n` |
//! | `float`, `double` | `number` | a literal, or `NaN`/`Infinity` |
//! | `long double` | `{ octets }` | `{ octets: zdwire.float128FromDecimalString("…") }` |
//! | `fixed` | `string` | the decimal, quoted |
//! | `string`, `wstring` | `string` | a quoted literal |
//! | `sequence<T>` | `Array` | `[ …, … ]` |
//! | `T[N]` | `Array` | nested `[ …, … ]` literals |
//! | `map<K,V>` | `Array` | an array of `{ key: …, value: … }` |
//! | `enum` | the flattened frozen object | `Mode.IDLE` |
//! | `bitmask` | `number` or `bigint` | the or of the bits |
//! | `struct`, `bitset`, `valuetype` | plain object | `{ a: …, … }` |
//! | `union` | plain object | `{ _d: …, one: … }`, unnamed branches left out |
//! | `@optional T` | the value, or the key left out | — |
//! | `@external T` | the value, inline | JavaScript already holds it by reference |

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Ident, Literal};
use zerodds_lang_cst::node::Expr;
use zerodds_lang_rules::adapter::node::{field_name, node_type_name};

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// The one expression that builds `values` as a value of the root's
/// type.
///
/// # Errors
/// A message naming the path in the value.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<Expr, String> {
    emit(ast, type_, values, "sample", 0)
}

/// The Node expression `value` is under `type_`.
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
) -> Result<Expr, String> {
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
            Ok(Expr::string(text))
        }

        // The enumerator by name, off the flattened frozen object:
        // readable, and refused loudly where the name is not declared.
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
                    return Ok(Expr::member(
                        Expr::name(&node_type_name(ast, type_)),
                        &field_name(name),
                    ));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, as the holder its bit bound calls
        // for: a `number` to 32 bits and a `bigint` beyond it, exactly
        // the boundary `crates/lang-rules/src/emit/node.rs`'s
        // `unsigned_holder` draws.
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
            if ast.effective_bit_bound(type_) > 32 {
                Ok(Expr::Literal(Literal::Int(format!("{bits}n"))))
            } else {
                Ok(Expr::int(i128::try_from(bits).unwrap_or(i128::MAX)))
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
            let mut entries = Vec::new();
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
                entries.push((Ident::new(&field_name(name)), Expr::int(i128::from(number))));
            }
            Ok(Expr::Object(entries))
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object"))?;
            let members: Vec<_> =
                ast.all_members_of(type_)
                    .iter()
                    .filter_map(|id| ast.member(*id).ok())
                    .filter_map(|member| {
                        member.type_.resolved().map(|target| {
                            (member.name.clone(), target, member.facets.is_optional())
                        })
                    })
                    .collect();
            let declared: Vec<&str> = members.iter().map(|(name, _, _)| name.as_str()).collect();
            zerodds_wireprobe_gen::fill::refuse_undeclared(object, &declared, path)?;
            let mut entries = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3): it takes the
            // keys of its own members, and this structure the others.
            let inherited = zerodds_wireprobe_gen::fill::Inherited::of(ast, type_, object);
            if let Some(inherited) = &inherited {
                let expr = emit(
                    ast,
                    inherited.base,
                    &inherited.part,
                    &format!("{path}.parent"),
                    depth + 1,
                )?;
                entries.push((Ident::new("parent"), expr));
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
                        let expr = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                        entries.push((Ident::new(&field_name(&name)), expr));
                    }
                    // Absence is the key left out.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(Expr::Object(entries))
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
            let disc = emit(ast, discriminator, given, &format!("{path}._d"), depth + 1)?;
            let mut entries = vec![(Ident::new("_d"), disc)];
            for (name, member) in ast
                .cases_of(type_)
                .iter()
                .filter_map(|case| ast.case(*case).ok())
                .filter_map(|case| ast.member(case.member).ok().map(|m| (m.name.clone(), m)))
            {
                if let Some(member_value) = object.get(&name) {
                    let member_type = member
                        .type_
                        .resolved()
                        .ok_or_else(|| format!("{path}.{name}: the case type does not resolve"))?;
                    let member_path = format!("{path}.{name}");
                    let expr = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                    entries.push((Ident::new(&field_name(&name)), expr));
                }
                // A branch the sample does not name is never read: the
                // encoder writes the selected one and the decoder never
                // fills the rest, so leaving the key out says the same
                // thing a `undefined` placeholder would, without
                // claiming the value is there.
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
            Ok(Expr::Object(entries))
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
            Ok(Expr::Array(parts))
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
                let key_expr = emit(
                    ast,
                    key_type,
                    &pair[0],
                    &format!("{path}[{index}].key"),
                    depth + 1,
                )?;
                let value_expr = emit(
                    ast,
                    value_type,
                    &pair[1],
                    &format!("{path}[{index}].value"),
                    depth + 1,
                )?;
                parts.push(Expr::Object(vec![
                    (Ident::new("key"), key_expr),
                    (Ident::new("value"), value_expr),
                ]));
            }
            Ok(Expr::Array(parts))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no Node form this filler can build"
        )),
    }
}

/// One dimension of an IDL array as a nested `Array` literal; the outer
/// dimension is written outermost, matching the walk's own nested-loop
/// order.
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
) -> Result<Expr, String> {
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
    Ok(Expr::Array(parts))
}

/// One primitive value, as a Node expression.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<Expr, String> {
    let number = |value: &Value| -> Result<i128, String> {
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
        PrimitiveKind::Boolean => Expr::bool(
            value
                .as_bool()
                .ok_or_else(|| format!("{path}: expected a boolean"))?,
        ),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32 => Expr::int(number(value)?),
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => {
            Expr::Literal(Literal::Int(format!("{}n", number(value)?)))
        }
        // The character literal, unchecked here: the range check is the
        // sample's own refusal and happens inside `writeChar`/`writeWchar`
        // when the codec runs, which is where the Rust probe's own
        // refusal happens too.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            Expr::Literal(Literal::Char(one))
        }
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            node_float(given, kind == PrimitiveKind::Float32)
        }
        // The decimal text, converted at run time by the runtime's own
        // exact-rational reading — a JavaScript `number` literal would
        // round to a binary64 before the value reached the member.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            Expr::Object(vec![(
                Ident::new("octets"),
                Expr::Call {
                    callee: Box::new(Expr::member(
                        Expr::name("zdwire"),
                        "float128FromDecimalString",
                    )),
                    args: vec![Expr::string(text)],
                },
            )])
        }
    })
}

/// A JavaScript floating-point spelling: a literal for a finite value,
/// `NaN`/`Infinity` for the three with none. `narrow` says the member is
/// a `float`: a JavaScript `number` is always a binary64, so a `float32`
/// sample is rounded once here the same way `runtime/node/zdwire.js`'s
/// `writeF32` rounds it, keeping the literal in the file the number the
/// sample actually carries.
fn node_float(number: f64, narrow: bool) -> Expr {
    #[allow(clippy::cast_possible_truncation)]
    let value = if narrow {
        f64::from(number as f32)
    } else {
        number
    };
    if value.is_nan() {
        return Expr::name("NaN");
    }
    if value.is_infinite() {
        return if value > 0.0 {
            Expr::name("Infinity")
        } else {
            Expr::Unary {
                op: "-",
                value: Box::new(Expr::name("Infinity")),
            }
        };
    }
    // `{:?}` is Rust's shortest round-tripping form; JavaScript's
    // grammar accepts every spelling it produces.
    Expr::Literal(Literal::Float(format!("{value:?}")))
}
