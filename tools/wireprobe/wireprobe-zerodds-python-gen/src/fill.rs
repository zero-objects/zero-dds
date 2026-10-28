// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Python expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-typescript-gen/src/fill.rs`, and
//! the same length for the same reason: a Python literal can hold a
//! nested aggregate, so the sample is one expression the harness assigns
//! once under the declared type rather than the run of statements the
//! Java column needs.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs` documents
//! and `tools/wireprobe/wireprobe-zerodds-python`'s `from_json` path
//! already consumes: a 64-bit integer is a string, an absent `@optional`
//! member is a key left out rather than a `null`, and a map is an array
//! of two-element arrays.
//!
//! ## What the Python mapping makes of each construct
//!
//! Read off `emit::python::value_type_of` in
//! `crates/lang-rules/src/emit/python.rs`, which is what the codec walks
//! against, and `crates/lang-rules/src/adapter/python.rs`, which is what
//! declares it. Repeated here because a filler that guessed one of these
//! would produce a value the encoder beside it cannot write.
//!
//! | IDL | Python | what this writes |
//! |---|---|---|
//! | `boolean` | `bool` | `True` / `False` |
//! | `char`, `wchar` | `str` | a one-character string literal |
//! | every integer width | `int` | the number |
//! | `float`, `double` | `float` | a number, or `float("nan")` |
//! | `long double` | `Float128` | `Float128.from_decimal("…")` |
//! | `string`, `wstring`, `fixed` | `str` | a quoted literal |
//! | `sequence<T>`, `T[N]` | `list[T]` | a list per dimension |
//! | `map<K,V>` | `list[tuple[K, V]]` | `[(k, v)]` |
//! | `enum` | `enum.IntEnum` | the enumerator's **value** |
//! | `bitmask` | `enum.IntFlag` | the or of the named bits |
//! | `bitset` | a dataclass | one keyword per bitfield |
//! | `struct`, `union` | a dataclass | keyword arguments |
//! | `@optional T` | `T | None` | the value, or the argument left out |
//!
//! ## Three places this differs from the TypeScript filler
//!
//! **A 64-bit integer needs no widening.** A Python `int` is unbounded,
//! so `18446744073709551615` is written as it is where TypeScript needs
//! a `bigint` suffix and Java has to write the two's-complement pattern.
//! `Writer._check` range-checks it against the declared width at the
//! octet.
//!
//! **An aggregate is a call and not a literal.** `adapter/python.rs`
//! declares a `dataclasses.dataclass` with required fields, so there is
//! no object literal to write: the constructor takes the members by
//! keyword, and a member left out of the call is a `TypeError` naming it
//! rather than a value that is quietly `None`. That is also how an
//! absent `@optional` member is written — the argument is omitted and
//! the field's declared default of `None` stands.
//!
//! **A map entry is a `tuple` and not a class.** §7.4.1.1.4 defines a
//! `map` as a sequence of entries and this column writes that as
//! `list[tuple[K, V]]` with no entry class at all, which is what
//! `emit::python::entry_half` indexes and what `make_room` rebuilds on
//! the read side.
//!
//! ## Why an enumerator becomes its value and not `Colour.RED`
//!
//! `adapter/python.rs` nests an IDL module in a class and the
//! enumeration inside it, so the reference an expression at the module
//! root has to write is the qualified path — which the materialiser
//! computes and a rule application may rename. A filler that
//! reconstructed the path would be a second implementation of that
//! naming, and the two would part on the first renamed enumeration. The
//! **value** is the model's and needs no naming, and an `enum.IntEnum`
//! member *is* an `int`: `ast.effective_enumerator_value` is the same
//! function the materialiser writes the constant from, so a wrong
//! constant in the declaration shows up as a mismatch in the octets
//! rather than as a name that does not resolve.

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::python::escape_name;
use zerodds_lang_render::python::string_literal;
use zerodds_lang_rules::adapter::python::declared_path;

/// The Python expression `values` is, under `type_`.
///
/// # Errors
/// A message naming the path in the value, in the shape the Rust probe
/// reports: a sample two probes read differently is a difference in the
/// readers and not in the serialisers.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, String> {
    emit(ast, type_, values, "sample", 0)
}

/// How deep an alias chain is followed before the model is taken to be
/// malformed. The materialiser bounds its own spelling at the same
/// number and IDL nests nowhere near it.
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

        // A `fixed` is a `str` here and not a `float` or a
        // `decimal.Decimal`: the digit count is a measured axis —
        // `FixedDigitCount::ValuePrecision` — and a holder that
        // normalised leading zeros away would have answered it.
        // `emit/python.rs`'s `value_type_of` says the same.
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

        // The or of the named bits, as one integer. No holder question
        // to answer: an `enum.IntFlag` member is an `int` and a Python
        // `int` carries a `@bit_bound(64)` mask exactly, which is where
        // the TypeScript column has to choose between `number` and
        // `bigint`.
        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array of bit names"))?;
            let mut declared = Vec::new();
            let mut bits: u128 = 0;
            for id in ast.bits_of(type_) {
                let Ok(bit) = ast.bit_value(*id) else {
                    continue;
                };
                declared.push(bit.name.clone());
            }
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
            // `all_bitfields_of` and not the node's own list: a `bitset`
            // that extends another carries the base's fields and the
            // codec beside this writes them.
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
                // An anonymous bitfield takes no value: it shifts the
                // fields above it and never appears in a sample, and the
                // materialiser declares no field for it.
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let given = object.get(name).ok_or_else(|| {
                    format!("{path}.{name}: no value, and a bitfield is never optional")
                })?;
                let number = given
                    .as_u64()
                    .ok_or_else(|| format!("{path}.{name}: expected a number"))?;
                parts.push(format!("{}={number}", escape_name(name)));
            }
            Ok(construct(ast, type_, &parts, path)?)
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
                parts.push(format!("{}={text}", escape_name("parent")));
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
                        parts.push(format!("{}={text}", escape_name(&name)));
                    }
                    // Absence is the argument left out. The field is
                    // declared `T | None = None`, so the constructor
                    // supplies the `None` and a `field=None` written here
                    // would say the same thing twice.
                    None if optional => {}
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(construct(ast, type_, &parts, path)?)
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
            // `_d` is the materialiser's own field name and not an IDL
            // one, so it is written as it is — `escape_name` would leave
            // it alone anyway and asking would suggest it might not.
            let mut parts = vec![format!(
                "_d={}",
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
                parts.push(format!("{}={text}", escape_name(name)));
            }
            Ok(construct(ast, type_, &parts, path)?)
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
                parts.push(format!("({key_text}, {value_text})"));
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
            "{path}: {other:?} has no Python form this filler can build"
        )),
    }
}

/// One dimension of an IDL array, recursively.
///
/// A multi-dimensional IDL array is nested Python lists, which is what
/// `emit::python::value_type_of` answers — `long g[2][3]` is
/// `list[list[int]]` — so each dimension is one level of brackets and the
/// length is checked against the declaration rather than taken from the
/// sample.
///
/// A `list` and not the `tuple` the materialiser writes for a *member's*
/// annotation: the two are the same sequence to everything that reads
/// one, and a `tuple` is what the reader cannot write into — see
/// `emit/python.rs`'s `make_room`. Nothing at run time checks the
/// annotation.
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

/// One primitive value, as a Python literal.
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
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        // Every integer width is one `int`, and the number is written as
        // it is. 18446744073709551615 needs no suffix and no widening:
        // a Python `int` is unbounded, and `Writer._check` refuses a
        // value outside the declared width at the octet rather than
        // folding it silently.
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
            // A `str` and not a code point: `emit/python.rs`'s
            // `code_point` hands the value through unchanged and
            // `Writer.write_char` does the refusing, which is where it
            // belongs — a caller that truncated first would put a
            // different character on the wire.
            string_literal(&one.to_string())
        }
        // `nan` and the two infinities have no literal in Python, so
        // each is written as the call that produces it. `-0.0` keeps its
        // sign because `write_f64` puts a different octet on the wire
        // for it.
        PrimitiveKind::Float32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            python_float(f64::from(number as f32))
        }
        PrimitiveKind::Float64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            python_float(number)
        }
        // Parsed by the runtime from the decimal the corpus gave, for
        // the reason `runtime/cpp/tests/corpus_bytes.cpp` states: a
        // column that parsed it a hundredth of an ulp differently would
        // produce sixteen entirely plausible bytes. Rust's `f64` cannot
        // hold a binary128 at all, so a generator that computed the
        // octets here would be the Rust column answering for this one.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            // Qualified, like every other runtime name the harness
            // writes: the sample is an expression at the module root
            // and a corpus case declaring a type named `Float128` would
            // otherwise take the name over. `crate::RUNTIME` says why.
            format!(
                "{}.Float128.from_decimal({})",
                crate::RUNTIME,
                string_literal(text)
            )
        }
    })
}

/// A Python floating-point literal.
///
/// The three values with no literal at all are named by the call that
/// builds them: `float("nan")` is the only spelling Python has, and
/// `math.nan` would need an import the materialiser does not write.
fn python_float(number: f64) -> String {
    if number.is_nan() {
        return "float(\"nan\")".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 {
            "float(\"inf\")".to_string()
        } else {
            "float(\"-inf\")".to_string()
        };
    }
    // `{:?}` is Rust's shortest round-tripping form and is also what
    // Python's `repr` produces: both print the shortest decimal that
    // reads back as the same binary64. It always carries a `.` or an
    // `e`, so the literal is a `float` and not an `int` — which matters,
    // because `write_f32` would take an `int` without complaint.
    format!("{number:?}")
}

/// A dataclass constructed by keyword.
///
/// The path is rooted at the module and not the declaration's own name:
/// `adapter/python.rs` nests an IDL module in a class, so `Point` at the
/// module root names nothing. `declared_path` is the materialiser's own
/// answer, so the two cannot part.
fn construct(
    ast: &Ast,
    type_: Id<TypeNode>,
    parts: &[String],
    path: &str,
) -> Result<String, String> {
    let declared = declared_path(ast, type_)
        .map_err(|error| format!("{path}: the type has no Python path: {error:?}"))?;
    let name = declared
        .segments()
        .iter()
        .map(|segment| segment.as_str())
        .collect::<Vec<&str>>()
        .join(".");
    Ok(format!("{name}({})", parts.join(", ")))
}

fn list_literal(parts: &[String]) -> String {
    format!("[{}]", parts.join(", "))
}
