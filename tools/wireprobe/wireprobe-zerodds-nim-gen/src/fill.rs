// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Nim statements one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-d-gen/src/fill.rs`, and like it
//! a run of statements into a declared local, not one literal. Nim's
//! reason is typing rather than reach: a bare `@[1, 2]` is a
//! `seq[int]`, a bare `5` under `some(..)` fixes the generic at `int`,
//! and every member of a generated type is a fixed-width type — so the
//! filler declares a typed temporary per composite and writes every
//! primitive with its Nim type on it.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out, a map is an array of two-element arrays.
//!
//! | IDL | Nim | what this writes |
//! |---|---|---|
//! | `boolean` | `bool` | `true` / `false` |
//! | `char` / `wchar` | `uint8` / `uint16` | `zdwire.checkedChar/Wchar(<code>)` |
//! | integers | their widths | `int32(-5)`, `'i64`/`'u64` past 32 bits |
//! | `float`, `double` | `float32`/`float64` | a literal, `NaN`, `Inf`, `-Inf` |
//! | `long double`, `fixed` | `string` | the decimal, quoted |
//! | `string`, `wstring` | `string` | a quoted literal, ASCII-escaped |
//! | `sequence<T>` | `seq[T]` | a typed temp and one `add` per element |
//! | `T[N]` | `array[N, T]` | a typed temp and one assignment per slot |
//! | `map<K,V>` | `seq[Entry]` | `add(Entry(key: .., value: ..))` |
//! | `enum` | `enum` | the enumerator's constant |
//! | `bitmask` | an unsigned alias | `Flags(<the or of the bits>)` |
//! | `struct`, `union`, `bitset` | objects | a temp and assignments |
//! | `@optional T` | `Option[T]` | `some(<typed value>)` |
//! | `@external T` | `ref T` | `zdwire.zdRef(<typed value>)` |

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::nim::{
    field_name, nim_enumerator_constant, nim_map_entry_name, nim_type_name, nim_value_type,
};

/// A statement builder: the lines before, and the expression they end
/// in.
pub struct Filler {
    /// The statements, one line each, un-indented.
    pub lines: Vec<String>,
    counter: usize,
}

impl Filler {
    fn fresh(&mut self) -> String {
        self.counter += 1;
        format!("zdV{}", self.counter)
    }
}

/// The statements that build `values` into a local named `into` of the
/// root's type. The caller declares the local.
///
/// # Errors
/// A message naming the path in the value.
pub fn fill(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    into: &str,
) -> Result<Vec<String>, String> {
    let mut filler = Filler {
        lines: Vec::new(),
        counter: 0,
    };
    let expr = emit(&mut filler, ast, type_, values, "sample", 0)?;
    filler.lines.push(format!("{into} = {expr}"));
    Ok(filler.lines)
}

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// The Nim type of `id`, spelled by the materialiser's own function.
fn type_text(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, String> {
    match nim_value_type(ast, id) {
        Ok(type_) => Ok(zerodds_lang_render::nim::type_(&type_)),
        Err(error) => Err(format!("{path}: the type has no Nim form: {error:?}")),
    }
}

/// The Nim expression `value` is under `type_`, with any statements it
/// needs appended first. Every answer is typed — a literal carries a
/// conversion or a suffix — so the caller may put it under `some(..)`
/// or `zdwire.zdRef(..)` without moving the type.
///
/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_DEPTH`)
#[allow(clippy::too_many_lines)]
fn emit(
    filler: &mut Filler,
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
            Ok(nim_string_literal(text))
        }

        // The enumerator by name, as the constant the materialiser
        // declared: readable, and refused loudly where the name is not
        // declared.
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
                    return Ok(nim_enumerator_constant(ast, type_, name));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, in the declared alias's own type —
        // the conversion is what keeps the value typed under `add` and
        // `some(..)`.
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
            let literal = if bits > u64::from(u32::MAX) {
                format!("{bits}'u64")
            } else {
                format!("{bits}")
            };
            Ok(format!("{}({literal})", nim_type_name(ast, type_)))
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
            let temp = filler.fresh();
            filler
                .lines
                .push(format!("var {temp}: {}", nim_type_name(ast, type_)));
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
                let literal = if number > u64::from(u32::MAX) {
                    format!("{number}'u64")
                } else {
                    format!("{number}")
                };
                filler
                    .lines
                    .push(format!("{temp}.{} = {literal}", field_name(name)));
            }
            Ok(temp)
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
            let temp = filler.fresh();
            filler
                .lines
                .push(format!("var {temp}: {}", nim_type_name(ast, type_)));
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3): it takes the
            // keys of its own members, and this structure the others.
            let inherited = zerodds_wireprobe_gen::fill::Inherited::of(ast, type_, object);
            if let Some(inherited) = &inherited {
                let expr = emit(
                    filler,
                    ast,
                    inherited.base,
                    &inherited.part,
                    &format!("{path}.parent"),
                    depth + 1,
                )?;
                filler
                    .lines
                    .push(format!("{temp}.{} = {expr}", field_name("parent")));
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
                        let expr = emit(
                            filler,
                            ast,
                            member_type,
                            member_value,
                            &member_path,
                            depth + 1,
                        )?;
                        let wrapped = match (optional, external) {
                            (true, true) => format!("some(zdwire.zdRef({expr}))"),
                            (false, true) => format!("zdwire.zdRef({expr})"),
                            (true, false) => format!("some({expr})"),
                            (false, false) => expr,
                        };
                        filler
                            .lines
                            .push(format!("{temp}.{} = {wrapped}", field_name(&name)));
                    }
                    // Absence is the key left out: the default-initialised
                    // field is already the absence.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(temp)
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
            let temp = filler.fresh();
            filler
                .lines
                .push(format!("var {temp}: {}", nim_type_name(ast, type_)));
            let disc = emit(
                filler,
                ast,
                discriminator,
                given,
                &format!("{path}._d"),
                depth + 1,
            )?;
            filler.lines.push(format!("{temp}.d = {disc}"));
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
                let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                let expr = emit(
                    filler,
                    ast,
                    member_type,
                    member_value,
                    &format!("{path}.{name}"),
                    depth + 1,
                )?;
                // The branch is this column's `Option[T]` marker for
                // "this is the branch that was written", `ref`-held
                // where the IDL said `@external`.
                let wrapped = if external {
                    format!("some(zdwire.zdRef({expr}))")
                } else {
                    format!("some({expr})")
                };
                filler
                    .lines
                    .push(format!("{temp}.{} = {wrapped}", field_name(name)));
            }
            Ok(temp)
        }

        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            let items = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array"))?;
            let temp = filler.fresh();
            filler
                .lines
                .push(format!("var {temp}: {}", type_text(ast, type_, path)?));
            for (index, item) in items.iter().enumerate() {
                let expr = emit(
                    filler,
                    ast,
                    element,
                    item,
                    &format!("{path}[{index}]"),
                    depth + 1,
                )?;
                filler.lines.push(format!("{temp}.add({expr})"));
            }
            Ok(temp)
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            // -1 is a sentinel, never a length; the mismatch is reported
            // against the sample below.
            let lengths: Vec<i128> = dims
                .iter()
                .map(|dim| ast.eval_integral(*dim).unwrap_or(-1))
                .collect();
            let temp = filler.fresh();
            filler
                .lines
                .push(format!("var {temp}: {}", type_text(ast, type_, path)?));
            fill_array(
                filler, ast, element, value, &lengths, &temp, "", path, depth,
            )?;
            Ok(temp)
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
            // `nim_map_entry_name` answers with a `Result` since the
            // bound became part of the hoisted routine's name: a map
            // nested past `MAX_TYPE_DEPTH` has no name to give. The
            // filler says so with the path it was walking.
            let entry_name =
                nim_map_entry_name(ast, type_).map_err(|error| format!("{path}: {error:?}"))?;
            let temp = filler.fresh();
            filler.lines.push(format!("var {temp}: seq[{entry_name}]"));
            for (index, pair) in entries.iter().enumerate() {
                let pair = pair
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| format!("{path}[{index}]: expected a two-element array"))?;
                let key_text = emit(
                    filler,
                    ast,
                    key_type,
                    &pair[0],
                    &format!("{path}[{index}].key"),
                    depth + 1,
                )?;
                let value_text = emit(
                    filler,
                    ast,
                    value_type,
                    &pair[1],
                    &format!("{path}[{index}].value"),
                    depth + 1,
                )?;
                filler.lines.push(format!(
                    "{temp}.add({entry_name}(key: {key_text}, value: {value_text}))"
                ));
            }
            Ok(temp)
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(filler, ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no Nim form this filler can build"
        )),
    }
}

/// One dimension of an IDL array, as one assignment per slot: nested
/// literals would need the outer type restated, and an index chain
/// does not.
///
/// zerodds-lint: recursion-depth 64 (one level per dimension; bounded by `MAX_DEPTH`)
#[allow(clippy::too_many_arguments)]
fn fill_array(
    filler: &mut Filler,
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lengths: &[i128],
    temp: &str,
    indices: &str,
    path: &str,
    depth: usize,
) -> Result<(), String> {
    let Some((length, rest)) = lengths.split_first() else {
        let expr = emit(filler, ast, element, value, path, depth + 1)?;
        filler.lines.push(format!("{temp}{indices} = {expr}"));
        return Ok(());
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
    for (index, item) in items.iter().enumerate() {
        fill_array(
            filler,
            ast,
            element,
            item,
            rest,
            temp,
            &format!("{indices}[{index}]"),
            &format!("{path}[{index}]"),
            depth + 1,
        )?;
    }
    Ok(())
}

/// One primitive value, as a typed Nim expression.
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
    let converted = |type_name: &str, value: &Value| -> Result<String, String> {
        Ok(format!("{type_name}({})", number(value)?))
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
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => converted("uint8", value)?,
        PrimitiveKind::Int8 => converted("int8", value)?,
        PrimitiveKind::Int16 => converted("int16", value)?,
        PrimitiveKind::UInt16 => converted("uint16", value)?,
        PrimitiveKind::Int32 => converted("int32", value)?,
        PrimitiveKind::UInt32 => converted("uint32", value)?,
        // The suffix is the type: a bare decimal past int32 would still
        // be an `int` literal, and one past int64 would not lex.
        PrimitiveKind::Int64 => format!("{}'i64", number(value)?),
        PrimitiveKind::UInt64 => format!("{}'u64", number(value)?),
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
            format!("zdwire.{checker}({})", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = f64::from(given as f32);
            format!("float32({})", nim_float(narrowed))
        }
        PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            format!("float64({})", nim_float(given))
        }
        // The decimal text; runtime/nim's writeF128 parses it exactly.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            nim_string_literal(text)
        }
    })
}

/// A Nim floating-point spelling: a literal, or the system constants
/// for the three values with no literal. `-0.0` is a literal in Nim and
/// keeps its sign.
fn nim_float(number: f64) -> String {
    if number.is_nan() {
        return "NaN".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 { "Inf" } else { "-Inf" }.to_string();
    }
    // `{:?}` is Rust's shortest round-tripping form; every spelling it
    // produces is a Nim floating literal too, and always carries a `.`
    // or an `e`, so the constant is floating and not integral.
    format!("{number:?}")
}

/// A Nim string literal, pure ASCII — the renderer's own escape, so a
/// filled string and a declared one read identically.
#[must_use]
pub fn nim_string_literal(text: &str) -> String {
    zerodds_lang_render::nim::string_literal(text)
}
