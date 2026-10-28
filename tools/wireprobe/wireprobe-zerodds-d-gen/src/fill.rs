// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The D statements one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-go-gen/src/fill.rs`, with the
//! Java column's shape rather than the Go one's: a run of statements
//! into a declared local, not one literal. Two D facts force it. A
//! union's branches live in an anonymous `union`, whose fields a named
//! struct literal does not reliably reach — an assignment does. And an
//! `@external` member is a `T*` whose value has to go through
//! `zdwire.zdPtr`, which is a call and not a literal position.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out, a map is an array of two-element arrays.
//!
//! | IDL | D | what this writes |
//! |---|---|---|
//! | `boolean` | `bool` | `true` / `false` |
//! | `char` / `wchar` | `ubyte` / `ushort` | `zdwire.CheckedChar/Wchar(<code>)` |
//! | integers | their widths | the number, `L`/`UL` past `int` |
//! | `float`, `double` | `float`/`double` | a literal, or `float.nan` and kin |
//! | `long double`, `fixed` | `string` | the decimal, quoted |
//! | `string`, `wstring` | `string` | a quoted literal |
//! | `sequence<T>`, `T[N]` | `T[]`, static arrays | array literals |
//! | `map<K,V>` | entry structs | positional `ZdMapEntry_..(k, v)` |
//! | `enum` | `enum Name` | the enumerator, `Mode.IDLE` |
//! | `bitmask` | `alias` to `uintN` | the or of the bits |
//! | `struct`, `union`, `bitset` | structs | a temp and assignments |
//! | `@optional T` | `Nullable!(T)` | `zdwire.zdNullable(cast(T)(v))` |
//! | `@external T` | `T*` | `zdwire.zdPtr(v)` |

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::d::{d_map_entry_name, d_type_name, d_value_type, field_name};

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
        format!("zd_v{}", self.counter)
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
    assign(&mut filler, ast, type_, values, into, "sample", 0)?;
    Ok(filler.lines)
}

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// Assign `value` into the place `into`, appending statements.
fn assign(
    filler: &mut Filler,
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    into: &str,
    path: &str,
    depth: usize,
) -> Result<(), String> {
    let expr = emit(filler, ast, type_, value, path, depth)?;
    filler.lines.push(format!("{into} = {expr};"));
    Ok(())
}

/// The D expression `value` is under `type_`, with any statements it
/// needs appended first.
/// zerodds-lint: recursion-depth 64 (member, case, collection and alias
/// nesting; bounded by [`MAX_DEPTH`], which refuses with the path in
/// the sample. The walk pairs a type with the part of the sample that
/// fills it, and a recursive IDL type ends only because the sample
/// ends — a termination argument, not a bound, so the bound is the
/// constant.)
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
            Ok(d_string_literal(text))
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
                    return Ok(format!("{}.{}", d_type_name(ast, type_), field_name(name)));
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
            Ok(suffixed(i128::try_from(bits).map_err(|_| {
                format!("{path}: the mask does not fit sixty-four bits")
            })?))
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
                .push(format!("{} {temp};", d_type_name(ast, type_)));
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
                filler
                    .lines
                    .push(format!("{temp}.{} = {number};", field_name(name)));
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
                .push(format!("{} {temp};", d_type_name(ast, type_)));
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
                    .push(format!("{temp}.{} = {expr};", field_name("parent")));
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
                        let wrapped = if external {
                            format!("zdwire.zdPtr({expr})")
                        } else if optional {
                            format!(
                                "zdwire.zdNullable(cast({})({expr}))",
                                type_text(ast, member_type, &member_path)?
                            )
                        } else {
                            expr
                        };
                        filler
                            .lines
                            .push(format!("{temp}.{} = {wrapped};", field_name(&name)));
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
                .push(format!("{} {temp};", d_type_name(ast, type_)));
            let disc = emit(
                filler,
                ast,
                discriminator,
                given,
                &format!("{path}._d"),
                depth + 1,
            )?;
            filler.lines.push(format!("{temp}._d = {disc};"));
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
                // An `@external` branch is a `T*`, as an `@external`
                // struct member is: the value goes through `zdPtr`.
                // Assigning it bare did not compile ("cannot implicitly
                // convert expression `zd_v3` of type `Inner` to
                // `Inner*`", annotation-external-union-branch, 88 matrix
                // cells).
                let external = zerodds_lang_rules::adapter::held_externally(ast, member);
                let member_type = member
                    .type_
                    .resolved()
                    .ok_or_else(|| format!("{path}.{name}: the case type does not resolve"))?;
                let expr = emit(
                    filler,
                    ast,
                    member_type,
                    member_value,
                    &format!("{path}.{name}"),
                    depth + 1,
                )?;
                let expr = if external {
                    format!("zdwire.zdPtr({expr})")
                } else {
                    expr
                };
                filler
                    .lines
                    .push(format!("{temp}.{} = {expr};", field_name(name)));
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
            let mut parts = Vec::new();
            for (index, item) in items.iter().enumerate() {
                parts.push(emit(
                    filler,
                    ast,
                    element,
                    item,
                    &format!("{path}[{index}]"),
                    depth + 1,
                )?);
            }
            if parts.is_empty() {
                // An empty literal `[]` is a `void[]` D will not
                // convert in every position; `null` is the empty slice
                // of any element type.
                return Ok("null".to_string());
            }
            Ok(format!("[{}]", parts.join(", ")))
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
            fill_array(filler, ast, element, value, &lengths, path, depth)
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
            // `d_map_entry_name` answers with a `Result` since the bound became
            // part of the hoisted routine's name: a map nested past
            // `MAX_TYPE_DEPTH` has no name to give.
            let entry_name =
                d_map_entry_name(ast, type_).map_err(|error| format!("{path}: {error:?}"))?;
            let mut parts = Vec::new();
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
                parts.push(format!("{entry_name}({key_text}, {value_text})"));
            }
            if parts.is_empty() {
                return Ok("null".to_string());
            }
            Ok(format!("[{}]", parts.join(", ")))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(filler, ast, target, value, path, depth + 1)
        }

        other => Err(format!(
            "{path}: {other:?} has no D form this filler can build"
        )),
    }
}

/// One dimension of an IDL array as a bare array literal; the enclosing
/// static array type converts it.
///
/// zerodds-lint: recursion-depth 64 (one level per array dimension,
/// then the element through [`emit`]; bounded by [`MAX_DEPTH`])
fn fill_array(
    filler: &mut Filler,
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lengths: &[i128],
    path: &str,
    depth: usize,
) -> Result<String, String> {
    let Some((length, rest)) = lengths.split_first() else {
        return emit(filler, ast, element, value, path, depth + 1);
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
            filler,
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

/// One primitive value, as a D expression.
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
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::UInt64 => suffixed(number(value)?),
        // `long.min` and not the digits: D reads `-9223372036854775808L`
        // as the negation of a literal past `long`, "signed integer
        // overflow" (integer-extremes, 44 matrix cells). The codec writes
        // the same spelling for a constant (`emit::d`).
        PrimitiveKind::Int64 => {
            let raw = number(value)?;
            if raw == i128::from(i64::MIN) {
                "long.min".to_string()
            } else {
                suffixed(raw)
            }
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
                "CheckedChar"
            } else {
                "CheckedWchar"
            };
            format!("zdwire.{checker}({})", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = f64::from(given as f32);
            d_float(narrowed, "float")
        }
        PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            d_float(given, "double")
        }
        // The decimal text; runtime/d's WriteF128 parses it exactly.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            d_string_literal(text)
        }
    })
}

/// An integer literal wide enough to survive its own width: D types an
/// unsuffixed literal `int`.
fn suffixed(value: i128) -> String {
    if value > i128::from(i64::MAX) {
        return format!("{value}UL");
    }
    if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
        return format!("{value}L");
    }
    format!("{value}")
}

/// A D floating-point spelling: a literal, or the type's own properties
/// for the three values with no literal. `-0.0` is a literal in D and
/// keeps its sign.
fn d_float(number: f64, type_name: &str) -> String {
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
    // A subnormal has no decimal literal D accepts: "number `5e-324` is
    // not representable as a `double`" (float-negative-zero-denormal, 44
    // matrix cells). The hexadecimal spelling is the exact bit pattern
    // and D reads it as written.
    if number != 0.0 && number.abs() < f64::MIN_POSITIVE {
        return hex_float(number);
    }
    // `{:?}` is Rust's shortest round-tripping form; every spelling it
    // produces is a D floating literal too, and always carries a `.` or
    // an `e`, so the constant is floating and not integral.
    format!("{number:?}")
}

/// `0x0.mmmmmmmmmmmmmp-1022`: a subnormal `double` as D's hexadecimal
/// floating literal, its 52 stored mantissa bits after the point.
fn hex_float(number: f64) -> String {
    let bits = number.to_bits();
    let sign = if bits >> 63 == 1 { "-" } else { "" };
    let mantissa = bits & ((1u64 << 52) - 1);
    format!("{sign}0x0.{mantissa:013x}p-1022")
}

/// The D type of `id`, spelled by the materialiser's own function.
fn type_text(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, String> {
    match d_value_type(ast, id) {
        Ok(type_) => Ok(zerodds_lang_render::d::type_(&type_)),
        Err(error) => Err(format!("{path}: the type has no D form: {error:?}")),
    }
}

/// A D interpreted string literal; a control character goes out as
/// `\u....` so the file stays printable.
pub fn d_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 || other as u32 == 0x7F => {
                out.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
