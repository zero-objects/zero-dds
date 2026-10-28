// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Zig expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-d-gen/src/fill.rs`, with the
//! Go column's shape rather than the D one's: one literal, not a run of
//! statements. Zig makes the literal form the natural one — a struct
//! literal names its fields, an omitted `@optional` member takes the
//! `= null` its declaration carries, and a union's branches are plain
//! fields, so nothing needs a temporary. What D routes through
//! statements Zig routes through the result location: every nested
//! `.{ … }` takes its type from the field or element it initialises.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out, a map is an array of two-element arrays.
//!
//! | IDL | Zig | what this writes |
//! |---|---|---|
//! | `boolean` | `bool` | `true` / `false` |
//! | `char` / `wchar` | `u8` / `u16` | `try zdwire.checkedChar/Wchar(<code>)` |
//! | integers | their widths | the bare number — a `comptime_int` coerces |
//! | `float`, `double` | `f32`/`f64` | a literal, or `std.math.nan(..)` and kin |
//! | `long double` | `f128` | the decimal as a literal — `comptime_float` is 128 bits wide |
//! | `fixed` | `[]const u8` | the decimal, quoted |
//! | `string`, `wstring` | `[]const u8` | a quoted literal |
//! | `sequence<T>` | `[]T` | `try zdwire.zdSlice(zd_heap, T, &[_]T{ … })` |
//! | `T[N]` | `[N]T` | nested `.{ … }` list literals |
//! | `map<K,V>` | `[]ZdMapEntry_..` | the same slice helper over entry literals |
//! | `enum` | `enum(i32)` | the enumerator, `Mode.IDLE` |
//! | `bitmask` | an unsigned alias | the or of the bits, as a number |
//! | `struct`, `bitset` | structs | `.{ .a = …, … }` |
//! | `union` | struct with `_d` | the literal, unnamed branches `undefined` |
//! | `@optional T` | `?T` | the bare value — the coercion is Zig's |
//! | `@external T` | `*T` | `try zdwire.zdPtr(zd_heap, @as(T, …))` |

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::zig::{
    field_name, zig_map_entry_name, zig_type_name, zig_value_type,
};

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// The one expression that builds `values` as a value of the root's
/// type. The caller writes it into a typed local; `zd_heap` is the
/// allocator name in scope there.
///
/// # Errors
/// A message naming the path in the value.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, String> {
    emit(ast, type_, values, "sample", 0)
}

/// The Zig expression `value` is under `type_`.
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
            Ok(zig_string_literal(text))
        }

        // The enumerator by name, in the enum's own scope: readable,
        // and refused loudly where the name is not declared.
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
                    return Ok(format!(
                        "{}.{}",
                        zig_type_name(ast, type_),
                        field_name(name)
                    ));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits; the member is an alias to an
        // unsigned integer, so the number is the value.
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
                parts.push(format!(".{} = {number}", field_name(name)));
            }
            Ok(anonymous(&parts))
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object"))?;
            let members: Vec<_> = ast
                .all_members_of(type_)
                .iter()
                .filter_map(|id| ast.member(*id).ok())
                .filter_map(|member| {
                    member.type_.resolved().map(|target| {
                        (
                            member.name.clone(),
                            target,
                            member.facets.is_optional(),
                            zerodds_lang_rules::adapter::held_externally(ast, member),
                        )
                    })
                })
                .collect();
            let declared: Vec<&str> = members
                .iter()
                .map(|(name, _, _, _)| name.as_str())
                .collect();
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
                parts.push(format!(".{} = {expr}", field_name("parent")));
            }
            let members: Vec<_> = members
                .into_iter()
                .filter(|(name, _, _, _)| {
                    zerodds_wireprobe_gen::fill::writes_member(inherited.as_ref(), name)
                })
                .collect();
            for (name, member_type, optional, external) in members {
                let member_path = format!("{path}.{name}");
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(format!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        let expr = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                        let wrapped = if external {
                            external_pointer(ast, member_type, &expr, &member_path)?
                        } else {
                            expr
                        };
                        parts.push(format!(".{} = {wrapped}", field_name(&name)));
                    }
                    // Absence is the key left out: the field's `= null`
                    // default is already the absence, and the literal
                    // simply does not name it.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(anonymous(&parts))
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
            let mut parts = vec![format!("._d = {disc}")];
            for (name, member) in ast
                .cases_of(type_)
                .iter()
                .filter_map(|case| ast.case(*case).ok())
                .filter_map(|case| ast.member(case.member).ok().map(|m| (m.name.clone(), m)))
            {
                match object.get(&name) {
                    Some(member_value) => {
                        let member_type = member.type_.resolved().ok_or_else(|| {
                            format!("{path}.{name}: the case type does not resolve")
                        })?;
                        let member_path = format!("{path}.{name}");
                        let expr = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                        // `@external` is a facet of a union's member as
                        // much as of a structure's, and the branch field
                        // is the same `*T`.
                        let wrapped = if zerodds_lang_rules::adapter::held_externally(ast, member) {
                            external_pointer(ast, member_type, &expr, &member_path)?
                        } else {
                            expr
                        };
                        parts.push(format!(".{} = {wrapped}", field_name(&name)));
                    }
                    // A branch the sample does not name is never read:
                    // the encoder writes the selected one, the dumper
                    // prints it, and `undefined` says the non-value out
                    // loud.
                    None => parts.push(format!(".{} = undefined", field_name(&name))),
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
            Ok(anonymous(&parts))
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
            slice_of(ast, element, &parts, path)
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            // -1 is a sentinel, never a length; the mismatch is
            // reported against the sample below.
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
            // `zig_map_entry_name` answers with a `Result` since the bound became
            // part of the hoisted routine's name: a map nested past
            // `MAX_TYPE_DEPTH` has no name to give.
            let entry_name =
                zig_map_entry_name(ast, type_).map_err(|error| format!("{path}: {error:?}"))?;
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
                parts.push(format!(".{{ .key = {key_text}, .value = {value_text} }}"));
            }
            Ok(format!(
                "try zdwire.zdSlice(zd_heap, {entry_name}, &[_]{entry_name}{{ {} }})",
                parts.join(", ")
            ))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no Zig form this filler can build"
        )),
    }
}

/// `try zdwire.zdPtr(zd_heap, @as(T, …))` — the storage an `@external`
/// member's `*T` points at, out of the harness's arena.
///
/// The `@as` is what makes it compile. Everywhere else in this filler a
/// literal takes its type from the field it initialises; here the field
/// is a `*T` and the literal is an argument to a call, so it has no
/// result location and `zdwire.zdPtr` infers `*@TypeOf(v)` from what it
/// was handed. Measured on Zig 0.14.1: an anonymous `.{ … }` gives
/// `*main.zdMain__struct_1234` where the field wants `*main.Inner`, a
/// string literal gives `**const [5:0]u8` where it wants
/// `*[]const u8`, and a bare number is `error: unable to resolve
/// comptime value`. Naming the type turns all three into the field's
/// own pointer.
fn external_pointer(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &str,
    path: &str,
) -> Result<String, String> {
    let text = type_text(ast, type_, path)?;
    Ok(format!("try zdwire.zdPtr(zd_heap, @as({text}, {value}))"))
}

/// `try zdwire.zdSlice(zd_heap, T, &[_]T{ … })` — the mutable slice a
/// sequence member is, out of the harness's arena; the element type is
/// written so the array literal's own type never has to be inferred.
fn slice_of(
    ast: &Ast,
    element: Id<TypeNode>,
    parts: &[String],
    path: &str,
) -> Result<String, String> {
    let element_type = type_text(ast, element, path)?;
    Ok(format!(
        "try zdwire.zdSlice(zd_heap, {element_type}, &[_]{element_type}{{ {} }})",
        parts.join(", ")
    ))
}

/// One dimension of an IDL array as an anonymous list literal; the
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
    Ok(anonymous(&parts))
}

/// `.{ a, b }`, or `.{}` for nothing.
fn anonymous(parts: &[String]) -> String {
    if parts.is_empty() {
        return ".{}".to_string();
    }
    format!(".{{ {} }}", parts.join(", "))
}

/// One primitive value, as a Zig expression.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, String> {
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
        // The bare number: a `comptime_int` coerces to the member's
        // width, and one that does not fit is a compile error rather
        // than a wrap.
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
            format!("{v}")
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
                "checkedChar"
            } else {
                "checkedWchar"
            };
            format!("try zdwire.{checker}({})", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = f64::from(given as f32);
            zig_float(narrowed, "f32")
        }
        PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            zig_float(given, "f64")
        }
        // The decimal as a literal: Zig's `comptime_float` is 128 bits
        // wide, so the number in the file is the number the sample
        // wrote.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            f128_literal(text, path)?
        }
    })
}

/// A `long double` decimal as a Zig `f128` expression: the literal for
/// a finite value, `std.math` for the three with no literal.
fn f128_literal(text: &str, path: &str) -> Result<String, String> {
    let lowered = text.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "inf" | "+inf" | "infinity" | "+infinity" => return Ok("std.math.inf(f128)".to_string()),
        "-inf" | "-infinity" => return Ok("-std.math.inf(f128)".to_string()),
        "nan" | "+nan" => return Ok("std.math.nan(f128)".to_string()),
        "-nan" => return Ok("-std.math.nan(f128)".to_string()),
        _ => {}
    }
    // A malformed decimal is refused here by shape: every character has
    // to be one a Zig float literal admits, or the file would not
    // compile and the refusal would land on the wrong exit code.
    if !text
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        || text.is_empty()
    {
        return Err(format!("{path}: `{text}` is not a decimal"));
    }
    // An integer-shaped literal coerces as a `comptime_int`; either way
    // the ascription pins the width.
    Ok(format!("@as(f128, {text})"))
}

/// A Zig floating-point spelling: a literal, or `std.math` for the
/// three values with no literal. `-0.0` is a literal in Zig and keeps
/// its sign.
fn zig_float(number: f64, type_name: &str) -> String {
    if number.is_nan() {
        return format!("std.math.nan({type_name})");
    }
    if number.is_infinite() {
        return if number > 0.0 {
            format!("std.math.inf({type_name})")
        } else {
            format!("-std.math.inf({type_name})")
        };
    }
    // `{:?}` is Rust's shortest round-tripping form; every spelling it
    // produces is a Zig floating literal too, and always carries a `.`
    // or an `e`, so the constant is floating and not integral.
    format!("{number:?}")
}

/// The Zig type of `id`, spelled by the materialiser's own function.
pub fn type_text(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, String> {
    match zig_value_type(ast, id, path) {
        Ok(type_) => Ok(zerodds_lang_render::zig::type_(&type_)),
        Err(error) => Err(format!("{path}: the type has no Zig form: {error:?}")),
    }
}

/// A Zig string literal, in the renderer's own escaping: Zig has no
/// `\0`, so a NUL goes out as `\x00`.
pub fn zig_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&zerodds_lang_render::zig::string_text(text));
    out.push('"');
    out
}
