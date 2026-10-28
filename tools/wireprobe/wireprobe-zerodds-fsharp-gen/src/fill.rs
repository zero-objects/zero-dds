// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The F# expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-zig-gen/src/fill.rs`, with this
//! column's own shape: an object-initialiser expression
//! ([`zerodds_lang_cst::fsharp::Expr::ObjectInit`]) for every aggregate,
//! because `crates/lang-cst/src/fsharp/mod.rs`'s module documentation is
//! where the class-not-record design that makes one expression build a
//! whole value is argued. A union's unselected branches are not left
//! `undefined` the way Zig's are — every field is named, and
//! [`default_fsharp::default_expr`] gives the ones the sample does not
//! select their own type's Table 9 value, exactly as
//! `crates/lang-rules/src/adapter/fsharp.rs` already does for the class
//! declaration's own field defaults.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out, a map is an array of two-element arrays.
//!
//! | IDL | F# | what this writes |
//! |---|---|---|
//! | `boolean` | `bool` | `true` / `false` |
//! | `char` / `wchar` | `byte` / `uint16` | `ZdWire.checkedChar/checkedWchar(<code>)` |
//! | narrow integers | their widths | the typed conversion — `byte(5)`, `uint16(5)`, … |
//! | `long`, `unsigned long` | `int32`/`uint32` | the bare number, or `uint32(n)` |
//! | `long long`, `unsigned long long` | `int64`/`uint64` | `int64(n)` / `uint64(n)` |
//! | `float`, `double` | `float32`/`float` | a literal, or `System.Single`/`System.Double`'s NaN/infinity |
//! | `long double` | `string` | the decimal, quoted — the runtime validates it when it writes |
//! | `fixed` | `string` | the decimal, quoted |
//! | `string`, `wstring` | `string` | a quoted literal |
//! | `sequence<T>` | `T []` | `[| … |]` |
//! | `T[N]` | `T [] …` | nested `[| … |]` |
//! | `map<K,V>` | `ZdMapEntry_.. []` | `[| ZdMapEntry_..(Key = …, Value = …); … |]` |
//! | `enum` | a CLI enum | `Mode.IDLE` |
//! | `bitmask` | a type abbreviation | the or of the bits, typed |
//! | `struct`, `bitset`, `valuetype`, `exception` | a class | `T(A = …, B = …)` |
//! | `union` | a class | `T(_d = …, a = …, b = <Table 9's value>)` |
//! | `@optional T` | `T option` | `Some(value)` |
//! | `@external T` | `T` | the bare value — no indirection needed |

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::Literal;
use zerodds_lang_cst::fsharp;
use zerodds_lang_rules::adapter::fsharp::{
    field_name, fsharp_map_entry_name, fsharp_type_name, fsharp_value_type, typed_int,
};
use zerodds_lang_rules::default_fsharp;

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// The one expression that builds `values` as a value of the root's
/// type.
///
/// # Errors
/// A message naming the path in the value.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<fsharp::Expr, String> {
    emit(ast, type_, values, "sample", 0)
}

/// The F# expression `value` is under `type_`.
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
) -> Result<fsharp::Expr, String> {
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
            Ok(fsharp::Expr::Literal(Literal::Str(text.to_string())))
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
                    return Ok(fsharp::Expr::dot(
                        fsharp::Expr::Name(zerodds_lang_cst::common::Ident::new(
                            &fsharp_type_name(ast, type_),
                        )),
                        &field_name(name),
                    ));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array of bit names"))?;
            let mut declared = Vec::new();
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
                    declared.push(bit.name.clone());
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
            let bound = ast.effective_bit_bound(type_);
            let holder = zerodds_lang_rules::adapter::fsharp::fsharp_holder(bound);
            Ok(typed_int(
                holder,
                fsharp::Expr::int(i128::try_from(bits).unwrap_or(i128::MAX)),
            ))
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
                let holder = zerodds_lang_rules::adapter::fsharp::fsharp_holder(field.bits);
                parts.push((
                    zerodds_lang_cst::common::Ident::new(&field_name(name)),
                    typed_int(holder, fsharp::Expr::int(i128::from(number))),
                ));
            }
            Ok(fsharp::Expr::ObjectInit {
                type_: zerodds_lang_cst::common::Ident::new(&fsharp_type_name(ast, type_)),
                fields: parts,
            })
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
            let mut parts = Vec::new();
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
                parts.push((
                    zerodds_lang_cst::common::Ident::new(&field_name("parent")),
                    expr,
                ));
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
                        let wrapped = if optional {
                            fsharp::Expr::Some_(Box::new(expr))
                        } else {
                            expr
                        };
                        parts.push((
                            zerodds_lang_cst::common::Ident::new(&field_name(&name)),
                            wrapped,
                        ));
                    }
                    // Absence is the key left out: the field's own
                    // `None` default is already the absence.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(fsharp::Expr::ObjectInit {
                type_: zerodds_lang_cst::common::Ident::new(&fsharp_type_name(ast, type_)),
                fields: parts,
            })
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
            let mut parts = vec![(zerodds_lang_cst::common::Ident::new("_d"), disc)];
            for (name, member) in ast
                .cases_of(type_)
                .iter()
                .filter_map(|case| ast.case(*case).ok())
                .filter_map(|case| ast.member(case.member).ok().map(|m| (m.name.clone(), m)))
            {
                let field = zerodds_lang_cst::common::Ident::new(&field_name(&name));
                match object.get(&name) {
                    Some(member_value) => {
                        let member_type = member.type_.resolved().ok_or_else(|| {
                            format!("{path}.{name}: the case type does not resolve")
                        })?;
                        let member_path = format!("{path}.{name}");
                        let expr = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                        parts.push((field, expr));
                    }
                    // A branch the sample does not name is never read:
                    // the encoder writes the selected one. Its own
                    // type's Table 9 value stands in for it — a real
                    // value, never an undefined one; the module
                    // documentation of `crates/lang-cst/src/fsharp/
                    // mod.rs` carries why.
                    None => {
                        let member_type = member.type_.resolved().ok_or_else(|| {
                            format!("{path}.{name}: the case type does not resolve")
                        })?;
                        let default = default_fsharp::default_expr(ast, member_type)
                            .map_err(|error| format!("{path}.{name}: {error}"))?;
                        parts.push((field, default));
                    }
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
            Ok(fsharp::Expr::ObjectInit {
                type_: zerodds_lang_cst::common::Ident::new(&fsharp_type_name(ast, type_)),
                fields: parts,
            })
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
            Ok(fsharp::Expr::ArrayLit(parts))
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the array element does not resolve"))?;
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
            // `fsharp_map_entry_name` answers with a `Result` since the bound became
            // part of the hoisted routine's name: a map nested past
            // `MAX_TYPE_DEPTH` has no name to give.
            let entry_name =
                fsharp_map_entry_name(ast, type_).map_err(|error| format!("{path}: {error:?}"))?;
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
                parts.push(fsharp::Expr::ObjectInit {
                    type_: zerodds_lang_cst::common::Ident::new(&entry_name),
                    fields: vec![
                        (zerodds_lang_cst::common::Ident::new("Key"), key_expr),
                        (zerodds_lang_cst::common::Ident::new("Value"), value_expr),
                    ],
                });
            }
            Ok(fsharp::Expr::ArrayLit(parts))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no F# form this filler can build"
        )),
    }
}

/// One dimension of an IDL array as a nested array literal; the
/// enclosing array type gives it its shape.
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
) -> Result<fsharp::Expr, String> {
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
    Ok(fsharp::Expr::ArrayLit(parts))
}

/// One primitive value, as an F# expression.
fn primitive_literal(
    kind: PrimitiveKind,
    value: &Value,
    path: &str,
) -> Result<fsharp::Expr, String> {
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
        PrimitiveKind::Boolean => fsharp::Expr::Literal(Literal::Bool(
            value
                .as_bool()
                .ok_or_else(|| format!("{path}: expected a boolean"))?,
        )),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => {
            let v = number(value)?;
            let fsharp::Type::Primitive(primitive) =
                zerodds_lang_rules::adapter::fsharp::fsharp_primitive_type(kind)
            else {
                return Err(format!("{path}: no F# primitive for {kind:?}"));
            };
            typed_int(primitive, fsharp::Expr::int(v))
        }
        // The code point, checked at run time by the runtime's own
        // `checkedChar`/`checkedWchar`: the refusal is the sample's
        // and not this generator's.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            let checker = if kind == PrimitiveKind::Char {
                "checkedChar"
            } else {
                "checkedWchar"
            };
            fsharp::Expr::call(
                &format!("ZdWire.{checker}"),
                vec![fsharp::Expr::int(i128::from(u32::from(one)))],
            )
        }
        PrimitiveKind::Float32 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = given as f32;
            fsharp_float32(narrowed)
        }
        PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            fsharp_float64(given)
        }
        // The decimal as quoted text: this column holds `long double`
        // as a `string`, and `ZdWire.WriteF128` is the one place that
        // decides whether it is a decimal this runtime accepts.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            fsharp::Expr::Literal(Literal::Str(text.to_string()))
        }
    })
}

fn fsharp_float64(number: f64) -> fsharp::Expr {
    if number.is_nan() {
        return fsharp::Expr::Name(zerodds_lang_cst::common::Ident::new("System.Double.NaN"));
    }
    if number.is_infinite() {
        let name = if number > 0.0 {
            "System.Double.PositiveInfinity"
        } else {
            "System.Double.NegativeInfinity"
        };
        return fsharp::Expr::Name(zerodds_lang_cst::common::Ident::new(name));
    }
    // Rust's `{:?}` is the shortest round-tripping form, and it always
    // carries a `.` or an `e`, so the constant reads as `float` and not
    // as an integer needing a suffix.
    fsharp::Expr::Literal(Literal::Float(format!("{number:?}")))
}

fn fsharp_float32(number: f32) -> fsharp::Expr {
    if number.is_nan() {
        return fsharp::Expr::Name(zerodds_lang_cst::common::Ident::new("System.Single.NaN"));
    }
    if number.is_infinite() {
        let name = if number > 0.0 {
            "System.Single.PositiveInfinity"
        } else {
            "System.Single.NegativeInfinity"
        };
        return fsharp::Expr::Name(zerodds_lang_cst::common::Ident::new(name));
    }
    fsharp::Expr::call(
        "float32",
        vec![fsharp::Expr::Literal(Literal::Float(format!("{number:?}")))],
    )
}

/// The F# type of `id`, spelled by the materialiser's own function.
///
/// # Errors
/// A message naming why the type has no F# form.
pub fn type_text(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, String> {
    match fsharp_value_type(ast, id, path) {
        Ok(type_) => Ok(zerodds_lang_render::fsharp::type_(&type_)),
        Err(error) => Err(format!("{path}: the type has no F# form: {error:?}")),
    }
}

/// An F# string literal.
#[must_use]
pub fn fsharp_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&zerodds_lang_render::fsharp::string_text(text));
    out.push('"');
    out
}
