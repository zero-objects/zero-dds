// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript expression one JSON sample is.
//!
//! The counterpart of `wireprobe-zerodds-java-gen/src/fill.rs`, and much
//! shorter than it for one reason: a Java sample is built by statements
//! because a Java array initialiser cannot hold the statements a nested
//! aggregate needs, and a TypeScript sample is one expression because an
//! object literal can hold anything. So this produces an expression and
//! the harness assigns it once, under the declared type — which is what
//! makes the assignment a type check rather than a formality.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs` documents:
//! a 64-bit integer is a string, an absent `@optional` member is a key
//! left out rather than a `null`, and a map is an array of two-element
//! arrays.
//!
//! ## What the TypeScript mapping makes of each construct
//!
//! Read off `crates/lang-rules/src/adapter/typescript.rs`, which is
//! where the decisions are made and argued. Repeated here because a
//! filler that guessed one of these would produce a literal that does
//! not typecheck against the declaration beside it.
//!
//! | IDL | TypeScript | what this writes |
//! |---|---|---|
//! | `boolean` | `boolean` | `true` / `false` |
//! | `char`, `wchar` | `string` | a one-character string literal |
//! | `octet` … `unsigned long` | `number` | a number |
//! | `long long`, `unsigned long long` | `bigint` | `123n` |
//! | `float`, `double` | `number` | a number, or `Number.NaN` |
//! | `long double` | `ZdFloat128` | `{ octets: float128FromDecimalString("…") }` |
//! | `string`, `wstring`, `fixed` | `string` | a quoted literal |
//! | `sequence<T>`, `T[N]` | `T[]` | an array literal |
//! | `map<K,V>` | `ReadonlyMap<K,V>` | `new Map([[key, value]])` |
//! | `enum` | a string-literal union | the enumerator's **name** |
//! | `bitmask` | `number` or `bigint` | the or of the named bits |
//! | `bitset` | an interface per bitfield | one property per field |
//! | `union` | `{ discriminator, <case> }` | the discriminant and the selected member |
//! | `@optional T` | `field?: T` | the value, or the key left out |
//!
//! ## Why an enumerator becomes its name and not `Colour.RED`
//!
//! `adapter/typescript.rs` puts a declaration in the namespace its IDL
//! module names, and the reference an expression outside that namespace
//! has to write is the qualified path — which the materialiser computes
//! privately and after a rule application may rename. A filler that
//! reconstructed the path would be a second implementation of that
//! naming, and the two would part on the first `@key`-renamed
//! enumeration. The enumerator's **name** needs no path: DDS-TS 1.0
//! §7.6 makes the alias a string-literal union whose members are the
//! enumerator names, so the bare string is a value of the type and is
//! contextually typed to the member it is written into. A name no
//! enumerator has is `tsc`'s to report, and is reported at the
//! declaration the sample is written against.
//!
//! ## Why the discriminant is written under two names
//!
//! The JSON a case carries spells a union's discriminant `_d`, because
//! that is the interop suite's canonical dialect
//! (`crates/wire-probe/src/canonical.rs`) and every column reads and
//! writes the same one. The TypeScript property is `discriminator`,
//! because that is what DDS-TS 1.0 §7.5 names it. The two are not the
//! same question and the filler crosses between them here.

use serde_json::Value;
use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::typescript::DISCRIMINANT_PROPERTY;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::typescript::TypeScript;
use zerodds_wireprobe_gen::{Refusal, gap};

/// The TypeScript expression `values` is, under `type_`.
///
/// # Errors
/// A message naming the path in the value, in the shape the Rust probe
/// reports: a sample two probes read differently is a difference in the
/// readers and not in the serialisers.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, Refusal> {
    emit(ast, type_, values, "sample")
}

// The depth counter this filler carried is gone, and the bound it was
// for is `zerodds_wireprobe_gen::bounded_alias_chains`. The recursion
// below shrinks the sample at every step except through
// `TypeKind::Alias`, where the same value is handed on — so an alias
// cycle was the one thing that could loop, and an alias cycle is a
// property of the model rather than of TypeScript. Four sibling fillers
// had no counter at all; a bound in the stem covers all five and the
// dumpers with it.

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by
/// `zerodds_wireprobe_gen::MAX_ALIAS_CHAIN`, which
/// `zerodds_wireprobe_gen::bounded_alias_chains` enforces over every
/// type in the model once, in `Session::open`, before any generation
/// runs).
///
/// Every other arm descends into a sub-value of the sample — a member,
/// a case, a collection item, a map half — and `serde_json::Value` is
/// an owned tree, so those arms cannot cycle whatever the type graph
/// does. `TypeKind::Alias` is the one arm that hands the same value on,
/// which is why the alias chain is the thing that has to be bounded,
/// and the only thing.
#[allow(clippy::too_many_lines)]
fn emit(ast: &Ast, type_: Id<TypeNode>, value: &Value, path: &str) -> Result<String, Refusal> {
    let node = ast.type_(type_).map_err(|error| gap!("{path}: {error}"))?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive_literal(*kind, value, path),

        // A `fixed` is a `string` here and not a `number`: the digit
        // count is a measured axis and a binary64 has no `fixed<31,2>`.
        // `adapter/typescript.rs` argues it.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            Ok(ts_string(text))
        }

        TypeKind::Enumeration { .. } => {
            let name = value
                .as_str()
                .ok_or_else(|| gap!("{path}: expected an enumerator name, as a string"))?;
            let mut declared = Vec::new();
            for id in ast.enumerators_of(type_) {
                let Ok(enumerator) = ast.enumerator(*id) else {
                    continue;
                };
                declared.push(enumerator.name.clone());
                if enumerator.name == name {
                    // The enumerator's *name* and not its ordinal.
                    // DDS-TS 1.0 §7.6 makes an enumeration a
                    // string-literal union, so the number is not a
                    // value of the type at all; the string is, and it
                    // is the same string the `as const` object holds.
                    ast.effective_enumerator_value(*id).ok_or_else(|| {
                        format!("{path}: `{name}` has no value the model can state")
                    })?;
                    return Ok(ts_string(&TypeScript::escape(name)));
                }
            }
            Err(gap!("{path}: `{name}` is not one of {declared:?}"))
        }

        // The or of the named bits, as one literal. A `@bit_bound(64)`
        // bitmask is a `bigint` and everything narrower a `number`,
        // which is the boundary `adapter/typescript.rs` draws and the
        // one `emit/typescript.rs` writes the cast against.
        //
        // ## Why this writes the pattern and its four siblings write the
        // ## names
        //
        // The C, C++, C# and Java fillers write `Holder.BIT_A |
        // Holder.BIT_B` and let the generated declaration carry the
        // numbers; this writes the number. Both read the same answer —
        // `ast.effective_bit_position`, which is also the function the
        // materialiser emits those constants from — so neither is a
        // second implementation of the bit layout, and a wrong position
        // shows up as a wrong octet in either spelling.
        //
        // What the two spellings differ over is naming, and that is
        // decided the same way this file decides it for an enumerator,
        // three doc paragraphs up: `adapter/typescript.rs` puts a
        // declaration in the namespace its IDL module names and computes
        // the qualified path privately, so a filler that wrote the path
        // would be a second implementation of *that*, and the two would
        // part on the first `@key`-renamed bitmask. The four languages
        // whose materialisers give a bitmask a class or a header
        // constant have a name to write and write it.
        //
        // So the rule the sibling module docs state — a filler decides
        // its language's spelling and never a wire value — holds in both
        // columns. Reading `effective_bit_position` is not deciding it.
        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| gap!("{path}: expected an array of bit names"))?;
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
                    .ok_or_else(|| gap!("{path}: a bit name is a string"))?;
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
                        return Err(gap!("{path}: `{name}` sits at bit {position}, past 64"));
                    }
                    bits |= 1u128 << position;
                    found = true;
                    break;
                }
                if !found {
                    return Err(gap!("{path}: `{name}` is not one of {declared:?}"));
                }
            }
            Ok(if ast.effective_bit_bound(type_) > 32 {
                format!("{bits}n")
            } else {
                format!("{bits}")
            })
        }

        TypeKind::Bitset { .. } => {
            // `all_bitfields_of` and not the node's own list: a `bitset`
            // that extends another carries the base's fields and the
            // codec beside this writes them.
            let fields = ast.all_bitfields_of(type_);
            let object = value
                .as_object()
                .ok_or_else(|| gap!("{path}: expected an object keyed by bitfield name"))?;
            let declared: Vec<String> = fields
                .iter()
                .filter_map(|id| ast.bitfield(*id).ok())
                .filter_map(|field| field.name.clone())
                .collect();
            for name in object.keys() {
                if !declared.contains(name) {
                    return Err(gap!("{path}.{name}: not a bitfield; {declared:?}"));
                }
            }
            let mut parts = Vec::new();
            for id in &fields {
                let Ok(field) = ast.bitfield(*id) else {
                    continue;
                };
                // An anonymous bitfield takes no value: it shifts the
                // fields above it and never appears in a sample, and
                // the materialiser declares no property for it.
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let given = object.get(name).ok_or_else(|| {
                    format!("{path}.{name}: no value, and a bitfield is never optional")
                })?;
                let number = given
                    .as_u64()
                    .ok_or_else(|| gap!("{path}.{name}: expected a number"))?;
                // Wider than 32 bits and the holder is a `bigint`; the
                // materialiser's `unsigned_holder` draws the boundary
                // and this has to write the literal that matches it.
                let literal = if field.bits > 32 {
                    format!("{number}n")
                } else {
                    format!("{number}")
                };
                parts.push(format!("{}: {literal}", property(name)));
            }
            Ok(object_literal(&parts))
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| gap!("{path}: expected an object"))?;
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
            for name in object.keys() {
                if !declared.contains(&name.as_str()) {
                    return Err(gap!("{path}.{name}: not a member; {declared:?}"));
                }
            }
            let mut parts = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3): it takes the
            // keys of its own members, and this structure the others.
            let inherited = zerodds_wireprobe_gen::fill::Inherited::of(ast, type_, object);
            if let Some(inherited) = &inherited {
                let filled = emit(
                    ast,
                    inherited.base,
                    &inherited.part,
                    &format!("{path}.parent"),
                )?;
                parts.push(format!("parent: {filled}"));
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
                        return Err(gap!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        let text = emit(ast, member_type, member_value, &member_path)?;
                        parts.push(format!("{}: {text}", property(&name)));
                    }
                    // Absence is the key left out. `adapter/typescript.rs`
                    // declares `field?: T` and the tree compiles with
                    // `exactOptionalPropertyTypes`, under which
                    // `field: undefined` and no `field` at all are two
                    // different things — so writing `undefined` here
                    // would not typecheck.
                    None if optional => {}
                    None => {
                        return Err(gap!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(object_literal(&parts))
        }

        TypeKind::Union { discriminator, .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| gap!("{path}: expected an object with `_d` and the member"))?;
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| gap!("{path}: the discriminator does not resolve"))?;
            let given = object
                .get("_d")
                .ok_or_else(|| gap!("{path}._d: no value"))?;
            // The JSON key is `_d` — the interop suite's canonical
            // dialect, which every column shares — and the TypeScript
            // property is `discriminator`, which is what DDS-TS 1.0
            // §7.5 names it and what the materialiser declares.
            let mut parts = vec![format!(
                "{}: {}",
                DISCRIMINANT_PROPERTY,
                emit(ast, discriminator, given, &format!("{path}._d"))?
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
                    .ok_or_else(|| gap!("{path}.{name}: not a case of this union"))?;
                let member_type = member
                    .type_
                    .resolved()
                    .ok_or_else(|| gap!("{path}.{name}: the case type does not resolve"))?;
                let text = emit(ast, member_type, member_value, &format!("{path}.{name}"))?;
                parts.push(format!("{}: {text}", property(name)));
            }
            Ok(object_literal(&parts))
        }

        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| gap!("{path}: the element type does not resolve"))?;
            let items = value
                .as_array()
                .ok_or_else(|| gap!("{path}: expected an array"))?;
            let mut parts = Vec::new();
            for (index, item) in items.iter().enumerate() {
                parts.push(emit(ast, element, item, &format!("{path}[{index}]"))?);
            }
            Ok(array_literal(&parts))
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| gap!("{path}: the element type does not resolve"))?;
            let lengths: Vec<i128> = dims
                .iter()
                .map(|dim| ast.eval_integral(*dim).unwrap_or(0))
                .collect();
            fill_array(ast, element, value, &lengths, path)
        }

        TypeKind::Map { key, value: v, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| gap!("{path}: the key type does not resolve"))?;
            let value_type = v
                .resolved()
                .ok_or_else(|| gap!("{path}: the value type does not resolve"))?;
            let entries = value
                .as_array()
                .ok_or_else(|| gap!("{path}: a map is an array of two-element arrays"))?;
            let mut parts = Vec::new();
            for (index, pair) in entries.iter().enumerate() {
                let pair = pair
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| gap!("{path}[{index}]: expected a two-element array"))?;
                let key_text = emit(ast, key_type, &pair[0], &format!("{path}[{index}].key"))?;
                let value_text =
                    emit(ast, value_type, &pair[1], &format!("{path}[{index}].value"))?;
                parts.push(format!("[{key_text}, {value_text}]"));
            }
            // DDS-TS 1.0 §7.9.1: a map is a `ReadonlyMap`, so the
            // sample constructs one. `new Map()` and not `new Map([])`
            // where the map is empty: an empty array literal infers
            // nothing, and the constructor with no argument takes the
            // key and value types from the contextual type alone.
            if parts.is_empty() {
                return Ok("new Map()".to_string());
            }
            Ok(format!("new Map({})", array_literal(&parts)))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| gap!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path)
        }

        other => Err(gap!(
            "{path}: {other:?} has no TypeScript form this filler can build"
        )),
    }
}

/// One dimension of an IDL array, recursively.
///
/// A multi-dimensional IDL array is nested TypeScript arrays, which is
/// what the materialiser declares — `long g[2][3]` is `number[][]` — so
/// each dimension is one level of brackets and the length is checked
/// against the declaration rather than taken from the sample.
///
/// zerodds-lint: recursion-depth = the array's declared dimension count
/// (`lengths` is one shorter on every call and the element then goes
/// through [`emit`], whose bound the note there states)
fn fill_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lengths: &[i128],
    path: &str,
) -> Result<String, Refusal> {
    let Some((length, rest)) = lengths.split_first() else {
        return emit(ast, element, value, path);
    };
    let items = value
        .as_array()
        .ok_or_else(|| gap!("{path}: expected an array of {length}"))?;
    if i128::try_from(items.len()).unwrap_or(-1) != *length {
        return Err(gap!(
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
        )?);
    }
    Ok(array_literal(&parts))
}

/// One primitive value, as a TypeScript literal.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, Refusal> {
    let number = |value: &Value| -> Result<i128, Refusal> {
        // A 64-bit value arrives as a string: JSON's number is a double
        // and a double cannot hold every 64-bit integer.
        match value {
            Value::String(text) => text
                .parse::<i128>()
                .map_err(|_| gap!("{path}: `{text}` is not an integer")),
            Value::Number(number) => number
                .as_i64()
                .map(i128::from)
                .ok_or_else(|| gap!("{path}: not an integer")),
            other => Err(gap!("{path}: expected a number, got {other}")),
        }
    };
    Ok(match kind {
        PrimitiveKind::Boolean => value
            .as_bool()
            .ok_or_else(|| gap!("{path}: expected a boolean"))?
            .to_string(),
        // Every integer to 32 bits is a `number` and is written as
        // itself: `Number.MAX_SAFE_INTEGER` is 2^53 − 1, so 4294967295
        // is exact here where the Java column has to widen it to a
        // `long`.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32 => format!("{}", number(value)?),
        // The value and not a pattern: a `bigint` is arbitrary
        // precision, so 16045690984833335998 is written out as it is
        // and `Writer.writeU64` range-checks it against 2^64. The Java
        // column has to write −2401053088876215618 for the same octets.
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => format!("{}n", number(value)?),
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| gap!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(gap!("{path}: expected exactly one character"));
            };
            // A `string` and not a code point: `emit/typescript.rs`'s
            // `code_point` hands the value through unchanged and
            // `Writer.writeChar` does the narrowing, which is where it
            // belongs — a caller that truncated first would put a
            // different character on the wire.
            ts_string(&one.to_string())
        }
        // `NaN` and the two infinities have no literal in TypeScript
        // any more than in Java, so they are named. `-0` is written as
        // `-0` and is not the same octets as `0`.
        PrimitiveKind::Float32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| gap!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            ts_float(f64::from(number as f32))
        }
        PrimitiveKind::Float64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| gap!("{path}: expected a number"))?;
            ts_float(number)
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
                .ok_or_else(|| gap!("{path}: a binary128 arrives as a decimal string"))?;
            format!(
                "{{ octets: float128FromDecimalString({}) }}",
                ts_string(text)
            )
        }
    })
}

/// A TypeScript floating-point literal.
///
/// No suffix — TypeScript has one numeric type — but the three values
/// that have no literal at all are named, and a negative zero keeps its
/// sign because `writeF64` puts a different octet on the wire for it.
fn ts_float(number: f64) -> String {
    if number.is_nan() {
        return "Number.NaN".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 {
            "Number.POSITIVE_INFINITY".to_string()
        } else {
            "Number.NEGATIVE_INFINITY".to_string()
        };
    }
    if number == 0.0 && number.is_sign_negative() {
        return "-0".to_string();
    }
    // `{:?}` is Rust's shortest round-tripping form, which is also
    // JavaScript's `Number.prototype.toString`: both print the shortest
    // decimal that reads back as the same binary64.
    format!("{number:?}")
}

/// A TypeScript string literal.
///
/// Every non-ASCII character becomes a `\uXXXX` escape rather than
/// arriving as UTF-8 in the source. `tsc` reads a file as UTF-8 and
/// would carry it, so this is not the correctness argument the Java
/// column's `-encoding` flag is; it is that a generated file kept under
/// `WIREPROBE_KEEP_GENERATED` is read beside the C and Java ones, and a
/// column whose escapes differ is a column a reader has to decode twice.
/// The escapes are UTF-16 code units, which is what a TypeScript string
/// holds.
pub(crate) fn ts_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_ascii_graphic() || ch == ' ' => out.push(ch),
            ch => {
                let mut buffer = [0u16; 2];
                for unit in ch.encode_utf16(&mut buffer) {
                    let _ = write!(out, "\\u{unit:04X}");
                }
            }
        }
    }
    out.push('"');
    out
}

/// A property name in an object literal.
///
/// The materialiser escapes a member whose IDL name is a TypeScript
/// reserved word — `escape_member` — and a literal that wrote the IDL
/// name would name a property the interface does not declare. Quoted
/// where even the escape is not an identifier, which is what
/// `PropertyName` does on the declaration side.
fn property(name: &str) -> String {
    let escaped = <TypeScript as CodecTarget>::escape(name);
    if is_plain_identifier(&escaped) {
        escaped
    } else {
        ts_string(&escaped)
    }
}

pub(crate) fn is_plain_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' || first == '$' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

/// `{}` for an empty aggregate, `{ a: 1, b: 2 }` otherwise.
fn object_literal(parts: &[String]) -> String {
    if parts.is_empty() {
        // Not `{ }`: an empty object literal is what a structure with no
        // members is, and `prettier` writes it closed.
        return "{}".to_string();
    }
    format!("{{ {} }}", parts.join(", "))
}

fn array_literal(parts: &[String]) -> String {
    format!("[{}]", parts.join(", "))
}
