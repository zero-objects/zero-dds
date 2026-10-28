// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Kotlin expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-zig-gen/src/fill.rs` and
//! `wireprobe-zerodds-java-gen/src/fill.rs`. Kotlin's constructor call
//! takes the Zig column's shape rather than Java's statement-driven one:
//! every generated class is a `data class` whose primary constructor
//! names every property, so one **named-argument call** builds a whole
//! value, omitting an absent `@optional` member and letting its `null`
//! default stand.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional` member
//! is a key left out, a map is an array of two-element arrays.
//!
//! | IDL | Kotlin | what this writes |
//! |---|---|---|
//! | `boolean` | `Boolean` | `true` / `false` |
//! | `char` / `wchar` | `Char` / `Int` | `checkedChar(..).toChar()` / `checkedWchar(..)` |
//! | integers | their widths | the number, `L`-suffixed where `Long` |
//! | `float`, `double` | `Float`/`Double` | a literal, `f`-suffixed for `Float` |
//! | `long double` | `Float128` | `Float128.parse("..")` |
//! | `fixed` | `String` | the decimal, quoted |
//! | `string`, `wstring` | `String` | a quoted literal |
//! | `sequence<T>` | `Array<T>`/`TArray` | `arrayOf(..)`/`intArrayOf(..)` |
//! | `T[N]` | nested arrays | nested array-literal calls |
//! | `map<K,V>` | `Array<ZdEntry_..>` | `arrayOf(ZdEntry_..(key=.., value=..), ..)` |
//! | `enum` | `Int` | the constant, `Mode.IDLE` |
//! | `bitmask` | `Int`/`Long` | the or of the bits, as a number |
//! | `struct`, `bitset` | `data class` | `Type(a = .., b = ..)` |
//! | `union` | `data class` | the literal, unselected branches at their own default |
//! | `@optional T` | `T?` | the bare value, or the argument left out |
//! | `@external T` | `T` | the bare value — no indirection to build |

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::kotlin::{
    kotlin_map_entry_name, kotlin_type_name, kotlin_value_type, property_name, unsigned_holder,
};

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// The one expression that builds `values` as a value of the root's
/// type.
///
/// # Errors
/// A message naming the path in the value.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, String> {
    emit(ast, type_, values, "sample", 0)
}

/// The Kotlin expression `value` is under `type_`.
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
            Ok(kotlin_string_literal(text))
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
                    return Ok(format!(
                        "{}.{}",
                        kotlin_type_name(ast, type_),
                        property_name(name)
                    ));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits: a bitmask member is a plain `Int`
        // or `Long`, so the number is the whole of it.
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
            if unsigned_holder(bound) == zerodds_lang_cst::kotlin::Primitive::Long {
                // Bit 63 set makes the or larger than `Long.MAX_VALUE`,
                // and the holder is a `Long`: what is written is the
                // two's-complement pattern of the same sixty-four bits,
                // the way every other unsigned value this column spells.
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let pattern = bits as u64 as i64;
                Ok(zerodds_lang_render::kotlin::long_text(pattern))
            } else {
                // The same reading one width down: bit 31 set is past
                // `Int.MAX_VALUE`, and an `Int` holder takes the
                // pattern.
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let pattern = bits as u32 as i32;
                Ok(format!("{pattern}"))
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
                // `adapter/kotlin.rs::unsigned_holder` gives a wide
                // enough field a `Long`, and Kotlin takes no unsuffixed
                // literal there.
                let suffix =
                    if unsigned_holder(field.bits) == zerodds_lang_cst::kotlin::Primitive::Long {
                        "L"
                    } else {
                        ""
                    };
                parts.push(format!("{} = {number}{suffix}", property_name(name)));
            }
            Ok(constructor_call(ast, type_, &parts))
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
                parts.push(format!("{} = {expr}", property_name("parent")));
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
                        parts.push(format!("{} = {expr}", property_name(&name)));
                    }
                    // Absence is the argument left out: the property's
                    // own `= null` default is already the absence.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(constructor_call(ast, type_, &parts))
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
            let mut parts = vec![format!("_d = {disc}")];
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
                    parts.push(format!("{} = {expr}", property_name(&name)));
                }
                // A branch the sample does not name is left at its own
                // property default — never read: the encoder writes only
                // the branch the discriminator selects.
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
            Ok(constructor_call(ast, type_, &parts))
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
            array_of(ast, element, &parts, path)
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
            let entry_name = kotlin_map_entry_name(ast, type_);
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
                parts.push(format!(
                    "{entry_name}(key = {key_text}, value = {value_text})"
                ));
            }
            Ok(format!("arrayOf({})", parts.join(", ")))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no Kotlin form this filler can build"
        )),
    }
}

/// `Type(a = x, b = y)`, or `Type()` for nothing — a data class's
/// primary constructor, called with named arguments so an omitted
/// `@optional` argument takes its own `null` default.
fn constructor_call(ast: &Ast, type_: Id<TypeNode>, parts: &[String]) -> String {
    format!("{}({})", kotlin_type_name(ast, type_), parts.join(", "))
}

/// `arrayOf(a, b)`/`intArrayOf(a, b)`, chosen the same way
/// `zerodds_lang_render::kotlin::array_of` chooses for a specialised
/// element.
fn array_of(
    ast: &Ast,
    element: Id<TypeNode>,
    parts: &[String],
    path: &str,
) -> Result<String, String> {
    let element_type = kotlin_value_type(ast, element, path)
        .map_err(|error| format!("{path}: the element has no Kotlin form: {error:?}"))?;
    Ok(match &element_type {
        zerodds_lang_cst::kotlin::Type::Primitive(primitive) => {
            format!(
                "{}ArrayOf({})",
                lower_first(primitive.as_str()),
                parts.join(", ")
            )
        }
        _ => format!(
            "arrayOf<{}>({})",
            zerodds_lang_render::kotlin::type_(&element_type),
            parts.join(", ")
        ),
    })
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// One dimension of an IDL array as a nested array-literal call; the
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
    // The element of this level is itself an array unless this is the
    // innermost dimension.
    if rest.is_empty() {
        array_of(ast, element, &parts, path)
    } else {
        Ok(format!("arrayOf({})", parts.join(", ")))
    }
}

/// One primitive value, as a Kotlin expression.
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
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32 => {
            format!("{}", number(value)?)
        }
        // A `Long` holds all three, but an `unsigned long long` past
        // `Long.MAX_VALUE` has no Kotlin literal of its own value: the
        // JVM has no unsigned integer, so what is written is the
        // two's-complement pattern the same 64 bits stand for, which is
        // what `crates/lang-rules/src/emit/kotlin.rs::wide_int` writes at
        // every other site and what `runtime/kotlin/ZdWire.kt` puts on
        // the wire unchanged.
        PrimitiveKind::UInt32 | PrimitiveKind::Int64 | PrimitiveKind::UInt64 => {
            let given = number(value)?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let pattern = if given > i128::from(i64::MAX) {
                u64::try_from(given)
                    .map_err(|_| format!("{path}: `{given}` does not fit 64 bits"))?
                    as i64
            } else {
                i64::try_from(given)
                    .map_err(|_| format!("{path}: `{given}` does not fit 64 bits"))?
            };
            zerodds_lang_render::kotlin::long_text(pattern)
        }
        // The code point, checked at run time — the refusal is the
        // sample's and not this generator's.
        PrimitiveKind::Char => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            format!("checkedChar({}, 0).toChar()", u32::from(one))
        }
        // `wchar` is `Int` here, so the checked point is the value
        // itself — no `.toChar()` to follow it with.
        PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            format!("checkedWchar({}, 0)", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = f64::from(given as f32);
            kotlin_float(narrowed, true)
        }
        PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            kotlin_float(given, false)
        }
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            format!("Float128.parse({})", kotlin_string_literal(text))
        }
    })
}

/// A Kotlin floating-point spelling: a literal, or the stdlib's own
/// named constants for the three values with none.
fn kotlin_float(number: f64, narrow: bool) -> String {
    let suffix = if narrow { "f" } else { "" };
    let type_name = if narrow { "Float" } else { "Double" };
    if number.is_nan() {
        return format!("{type_name}.NaN");
    }
    if number.is_infinite() {
        return if number > 0.0 {
            format!("{type_name}.POSITIVE_INFINITY")
        } else {
            format!("{type_name}.NEGATIVE_INFINITY")
        };
    }
    format!("{number:?}{suffix}")
}

/// The Kotlin type of `id`, spelled by the materialiser's own function.
pub fn type_text(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, String> {
    match kotlin_value_type(ast, id, path) {
        Ok(type_) => Ok(zerodds_lang_render::kotlin::type_(&type_)),
        Err(error) => Err(format!("{path}: the type has no Kotlin form: {error:?}")),
    }
}

/// A Kotlin string literal, in the renderer's own escaping.
pub fn kotlin_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&zerodds_lang_render::kotlin::string_text(text));
    out.push('"');
    out
}
