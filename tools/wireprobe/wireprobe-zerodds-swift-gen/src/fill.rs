// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Swift expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-d-gen/src/fill.rs`, with the
//! Go column's shape rather than the D one's: **one expression**, not a
//! run of statements. Swift forces it the other way round from D — a
//! generated struct's only initialiser is the memberwise one the
//! adapter writes out, so there is no default-initialised temporary to
//! assign into, and the memberwise call *is* the sample.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out, a map is an array of two-element arrays.
//!
//! | IDL | Swift | what this writes |
//! |---|---|---|
//! | `boolean` | `Bool` | `true` / `false` |
//! | `char` / `wchar` | `UInt8` / `UInt16` | `zdCheckedChar/Wchar(<code>)` |
//! | integers | their widths | the bare number; the parameter types it |
//! | `float`, `double` | `Float`/`Double` | a literal, or `Float.nan` and kin |
//! | `long double`, `fixed` | `String` | the decimal, quoted |
//! | `string`, `wstring` | `String` | a quoted literal |
//! | `sequence<T>`, `T[N]` | `[T]` | array literals |
//! | `map<K,V>` | entry structs | `[ZdMapEntry_..(key:, value:)]` |
//! | `enum` | `enum Name` | the case, `Mode.IDLE` |
//! | `bitmask` | `OptionSet` | `Name(rawValue: <or of the bits>)` |
//! | `struct`, `union`, `bitset` | structs | one memberwise call |
//! | `@optional T` | `T?` | the value; `nil` where the key is absent |
//! | `@external T` | `ZdExternal<T>` | `ZdExternal(<value>)` |

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::swift::{property_name, swift_map_entry_name, swift_type_name};

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// The Swift expression that builds `values` as a value of the root's
/// type. The caller writes it into a typed `let`.
///
/// # Errors
/// A message naming the path in the value.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, String> {
    emit(ast, type_, values, "sample", 0)
}

/// The Swift expression `value` is under `type_`.
#[allow(clippy::too_many_lines)]
/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_DEPTH`)
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
            Ok(swift_string_literal(text))
        }

        // The case by name, in the enum's own scope: readable, and
        // refused loudly where the name is not declared.
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
                        swift_type_name(ast, type_),
                        property_name(name)
                    ));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, through the raw-value initialiser
        // the `OptionSet` declares.
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
            let mask = u64::try_from(bits)
                .map_err(|_| format!("{path}: the mask does not fit sixty-four bits"))?;
            Ok(format!("{}(rawValue: {mask})", swift_type_name(ast, type_)))
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
            let mut args = Vec::new();
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
                args.push(format!("{}: {number}", property_name(name)));
            }
            Ok(format!(
                "{}({})",
                swift_type_name(ast, type_),
                args.join(", ")
            ))
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
            let mut args = Vec::new();
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
                args.push(format!("{}: {expr}", property_name("parent")));
            }
            let members: Vec<_> = members
                .into_iter()
                .filter(|(name, _, _, _)| {
                    zerodds_wireprobe_gen::fill::writes_member(inherited.as_ref(), name)
                })
                .collect();
            for (name, member_type, optional, external) in members {
                let member_path = format!("{path}.{name}");
                let label = property_name(&name);
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(format!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        let expr = emit(ast, member_type, member_value, &member_path, depth + 1)?;
                        let wrapped = if external {
                            format!("ZdExternal({expr})")
                        } else {
                            expr
                        };
                        args.push(format!("{label}: {wrapped}"));
                    }
                    // Absence is the key left out; the memberwise call
                    // still names the property, with `nil`.
                    None if optional => args.push(format!("{label}: nil")),
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(format!(
                "{}({})",
                swift_type_name(ast, type_),
                args.join(", ")
            ))
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
            let mut branches: Vec<(String, Id<TypeNode>, bool)> = Vec::new();
            for case in ast.cases_of(type_) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let Some(member_type) = member.type_.resolved() else {
                    return Err(format!(
                        "{path}.{}: the case type does not resolve",
                        member.name
                    ));
                };
                branches.push((
                    member.name.clone(),
                    member_type,
                    zerodds_lang_rules::adapter::held_externally(ast, member),
                ));
            }
            for name in object.keys() {
                if name != "_d" && !branches.iter().any(|(branch, _, _)| branch == name) {
                    return Err(format!("{path}.{name}: not a case of this union"));
                }
            }
            let disc = emit(ast, discriminator, given, &format!("{path}._d"), depth + 1)?;
            let mut args = vec![format!("_d: {disc}")];
            for (name, member_type, external) in branches {
                let label = property_name(&name);
                match object.get(&name) {
                    Some(member_value) => {
                        let expr = emit(
                            ast,
                            member_type,
                            member_value,
                            &format!("{path}.{name}"),
                            depth + 1,
                        )?;
                        let wrapped = if external {
                            format!("ZdExternal({expr})")
                        } else {
                            expr
                        };
                        args.push(format!("{label}: {wrapped}"));
                    }
                    None => args.push(format!("{label}: nil")),
                }
            }
            Ok(format!(
                "{}({})",
                swift_type_name(ast, type_),
                args.join(", ")
            ))
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
            Ok(format!("[{}]", parts.join(", ")))
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
            // `swift_map_entry_name` answers with a `Result` since the bound became
            // part of the hoisted routine's name: a map nested past
            // `MAX_TYPE_DEPTH` has no name to give.
            let entry_name =
                swift_map_entry_name(ast, type_).map_err(|error| format!("{path}: {error:?}"))?;
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
                    "{entry_name}(key: {key_text}, value: {value_text})"
                ));
            }
            Ok(format!("[{}]", parts.join(", ")))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no Swift form this filler can build"
        )),
    }
}

/// One dimension of an IDL array as an array literal.
/// zerodds-lint: recursion-depth 64 (array dimensions; bounded by `MAX_DEPTH`)
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
    Ok(format!("[{}]", parts.join(", ")))
}

/// One primitive value, as a Swift expression.
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
        // The bare number: a Swift integer literal is arbitrary
        // precision and the initialiser parameter types it.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => format!("{}", number(value)?),
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
                "zdCheckedChar"
            } else {
                "zdCheckedWchar"
            };
            format!("{checker}({})", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            swift_float(f64::from(given as f32), "Float")
        }
        PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            swift_float(given, "Double")
        }
        // The decimal text; `runtime/swift`'s writeF128 parses it
        // exactly.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            swift_string_literal(text)
        }
    })
}

/// A Swift floating-point spelling: a literal, or the type's own
/// properties for the three values with no literal. `-0.0` is a
/// literal in Swift and keeps its sign.
fn swift_float(number: f64, type_name: &str) -> String {
    if number.is_nan() {
        return format!("{type_name}.nan");
    }
    if number.is_infinite() {
        return if number > 0.0 {
            format!("{type_name}.infinity")
        } else {
            format!("-{type_name}.infinity")
        };
    }
    // `{:?}` is Rust's shortest round-tripping form; every spelling it
    // produces is a Swift floating literal too, and always carries a
    // `.` or an `e`, so the constant is floating and not integral.
    format!("{number:?}")
}

/// A Swift string literal; a control character goes out as `\u{..}` so
/// the file stays printable.
pub fn swift_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 || other as u32 == 0x7F => {
                out.push_str(&format!("\\u{{{:x}}}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
