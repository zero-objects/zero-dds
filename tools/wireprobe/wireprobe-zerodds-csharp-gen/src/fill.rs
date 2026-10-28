// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The statements that put one JSON sample into the generated C#.
//!
//! The counterpart of `wireprobe-zerodds-java-gen/src/fill.rs`, which
//! does the same for the Java shapes. It knows the JSON dialect and the
//! C# mapping and nothing else: it places values, the codec frames them
//! and `runtime/csharp` writes them. A filler that computed a bit
//! pattern or a length would move a rule out of the runtime, and having
//! a second implementation of the rule is the whole reason for a second
//! column.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs`
//! documents: a 64-bit integer is a string, an absent `@optional` member
//! is a key left out rather than a `null`, and a map is an array of
//! two-element arrays.
//!
//! ## What the C# mapping makes of each construct
//!
//! Read off `crates/lang-rules/src/adapter/csharp.rs`, which is where
//! the decisions are made and argued.
//!
//! | IDL | C# | what this writes |
//! |---|---|---|
//! | `boolean` | `bool` | `true` / `false` |
//! | `char`, `wchar` | `char` | a `'\uXXXX'` escape |
//! | `int8` | `sbyte` | a number |
//! | `octet`, `uint8` | `byte` | a number |
//! | `short` / `unsigned short` | `short` / `ushort` | a number |
//! | `long` / `unsigned long` | `int` / `uint` | a number, `U` where it needs one |
//! | `long long` / `unsigned long long` | `long` / `ulong` | `nL` / `nUL` |
//! | `float` | `float` | `nf` |
//! | `double` | `double` | a number |
//! | `long double` | `Float128` | `Float128.FromDecimalString("…")` |
//! | `string`, `wstring`, `fixed` | `string` | a quoted literal |
//! | `sequence<T>`, `T[N]` | `T[]` | `new T[n]` and one assignment per element |
//! | `map<K,V>` | `ZdEntry_k_v[]` | an array of entries, each with `Key` and `Value` |
//! | `enum` | a real `enum` | the enumerator, rooted |
//! | `bitmask` | a `[Flags]` enum | an `or` of the named bits, rooted |
//! | `bitset` | a class per bitfield | one assignment per field |
//! | `union` | `_D` and one field per case | the discriminator and the selected member |
//! | `@optional T` | `T?` or a reference | the value, or `null` |
//!
//! ## The two the C# mapping settles that Java could not
//!
//! `unsigned long long` needs no conversion: `16045690984833335998UL`
//! is the number. And an enumerator is a member of a real `enum`, so
//! the sample assigns `global::a.Colour.BLUE` rather than an `int`
//! constant.

use serde_json::Value;
use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::csharp::{declared_path, field_name};
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::csharp::CSharp;
use zerodds_wireprobe_gen::{Refusal, gap, no_form};

/// The C# the filler produced.
#[derive(Debug, Default)]
pub struct Fill {
    statements: String,
}

impl Fill {
    /// The statements, for the body of the fill method.
    #[must_use]
    pub fn statements(&self) -> &str {
        &self.statements
    }
}

/// Emit the statements that fill `zd_sample` with `values`.
///
/// # Errors
/// A message naming the path in the value, in the shape the Rust probe
/// reports: a sample two probes read differently is a difference in the
/// readers and not in the serialisers.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<Fill, Refusal> {
    let mut out = Fill::default();
    emit(ast, type_, values, "zd_sample", &mut out, "sample")?;
    Ok(out)
}

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
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
) -> Result<(), Refusal> {
    let node = ast.type_(type_).map_err(|error| gap!("{path}: {error}"))?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            let literal = primitive_literal(*kind, value, path)?;
            let _ = writeln!(out.statements, "        {lvalue} = {literal};");
            Ok(())
        }

        // A `fixed` is a `string` here and not a `decimal`, because
        // `decimal` carries 28 to 29 significant digits and IDL allows
        // 31. `adapter/csharp.rs` argues it and `fixed-scales` measures
        // it.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            let _ = writeln!(
                out.statements,
                "        {lvalue} = {};",
                csharp_string(text)
            );
            Ok(())
        }

        TypeKind::Enumeration { .. } => {
            let name = value
                .as_str()
                .ok_or_else(|| gap!("{path}: expected an enumerator name, as a string"))?;
            let mut declared = Vec::new();
            let mut found = None;
            for id in ast.enumerators_of(type_) {
                let Ok(enumerator) = ast.enumerator(*id) else {
                    continue;
                };
                declared.push(enumerator.name.clone());
                if enumerator.name == name {
                    // Verbatim but for keyword escaping: the C#
                    // materialiser folds a *member* name and leaves an
                    // enumerator's alone.
                    found = Some(zerodds_lang_cst::csharp::escape_keyword(&enumerator.name));
                }
            }
            let Some(constant) = found else {
                return Err(gap!("{path}: `{name}` is not one of {declared:?}"));
            };
            let holder = class_path(ast, type_, path)?;
            let _ = writeln!(out.statements, "        {lvalue} = {holder}.{constant};");
            Ok(())
        }

        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| gap!("{path}: expected an array of bit names"))?;
            let holder = class_path(ast, type_, path)?;
            let declared: Vec<String> = ast
                .bits_of(type_)
                .iter()
                .filter_map(|id| ast.bit_value(*id).ok())
                .map(|bit| bit.name.clone())
                .collect();
            let mut terms = Vec::new();
            for entry in names {
                let name = entry
                    .as_str()
                    .ok_or_else(|| gap!("{path}: a bit name is a string"))?;
                if !declared.iter().any(|known| known == name) {
                    return Err(gap!("{path}: `{name}` is not one of {declared:?}"));
                }
                terms.push(format!(
                    "{holder}.{}",
                    zerodds_lang_cst::csharp::escape_keyword(name)
                ));
            }
            // A `[Flags]` enum has no implicit conversion from zero, so
            // the empty set is written as the cast rather than as `0`.
            let value = if terms.is_empty() {
                format!("({holder})0")
            } else {
                terms.join(" | ")
            };
            let _ = writeln!(out.statements, "        {lvalue} = {value};");
            Ok(())
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
            let _ = writeln!(
                out.statements,
                "        {lvalue} = new {}();",
                rendered_type(ast, type_, path)?
            );
            for id in &fields {
                let Ok(field) = ast.bitfield(*id) else {
                    continue;
                };
                // An anonymous bitfield takes no value: it shifts the
                // fields above it and never appears in a sample.
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let Some(given) = object.get(name) else {
                    continue;
                };
                let number = given
                    .as_u64()
                    .ok_or_else(|| gap!("{path}.{name}: expected a number"))?;
                let _ = writeln!(
                    out.statements,
                    "        {lvalue}.{} = {number};",
                    field_name(name)
                );
            }
            Ok(())
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
            let _ = writeln!(
                out.statements,
                "        {lvalue} = new {}();",
                rendered_type(ast, type_, path)?
            );
            for (name, member_type, optional) in members {
                let member_path = format!("{path}.{name}");
                let field = format!("{lvalue}.{}", field_name(&name));
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(gap!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        emit(ast, member_type, member_value, &field, out, &member_path)?;
                    }
                    // Absence is the value: `adapter/csharp.rs` declares
                    // one field and lets `null` mean absent — a `T?` for
                    // a value type and the reference itself otherwise.
                    None if optional => {
                        let _ = writeln!(out.statements, "        {field} = null;");
                    }
                    None => {
                        return Err(gap!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(())
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
            let _ = writeln!(
                out.statements,
                "        {lvalue} = new {}();",
                rendered_type(ast, type_, path)?
            );
            emit(
                ast,
                discriminator,
                given,
                &format!("{lvalue}.{}", field_name("_d")),
                out,
                &format!("{path}._d"),
            )?;
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
                emit(
                    ast,
                    member_type,
                    member_value,
                    &format!("{lvalue}.{}", field_name(name)),
                    out,
                    &format!("{path}.{name}"),
                )?;
            }
            Ok(())
        }

        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| gap!("{path}: the element type does not resolve"))?;
            let items = value
                .as_array()
                .ok_or_else(|| gap!("{path}: expected an array"))?;
            // Allocated and then written into, element by element: a C#
            // array initialiser cannot hold the statements a nested
            // aggregate needs.
            let (base, nested) = split_array(&rendered_type(ast, element, path)?);
            let _ = writeln!(
                out.statements,
                "        {lvalue} = new {base}[{}]{nested};",
                items.len()
            );
            for (index, item) in items.iter().enumerate() {
                emit(
                    ast,
                    element,
                    item,
                    &format!("{lvalue}[{index}]"),
                    out,
                    &format!("{path}[{index}]"),
                )?;
            }
            Ok(())
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| gap!("{path}: the element type does not resolve"))?;
            let lengths: Vec<i128> = dims
                .iter()
                .map(|dim| ast.eval_integral(*dim).unwrap_or(0))
                .collect();
            allocate_array(ast, element, lvalue, &lengths, out, path)?;
            fill_array(ast, element, value, lvalue, &lengths, out, path)
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
            let entry = CSharp::map_entry_name(ast, type_);
            let _ = writeln!(
                out.statements,
                "        {lvalue} = new {entry}[{}];",
                entries.len()
            );
            for (index, pair) in entries.iter().enumerate() {
                let pair = pair
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| gap!("{path}[{index}]: expected a two-element array"))?;
                let at = format!("{lvalue}[{index}]");
                let _ = writeln!(out.statements, "        {at} = new {entry}();");
                emit(
                    ast,
                    key_type,
                    &pair[0],
                    &format!("{at}.Key"),
                    out,
                    &format!("{path}[{index}].key"),
                )?;
                emit(
                    ast,
                    value_type,
                    &pair[1],
                    &format!("{at}.Value"),
                    out,
                    &format!("{path}[{index}].value"),
                )?;
            }
            Ok(())
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| gap!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, lvalue, out, path)
        }

        other => Err(gap!(
            "{path}: {other:?} has no C# form this filler can build"
        )),
    }
}

/// Allocate every rank of a jagged array.
///
/// A C# multi-rank IDL array is `T[][]` and is allocated outermost
/// first: `new T[a][]` and then one `new T[b]` per element. `new T[a][b]`
/// is not an expression C# has, and the Java column's one-line
/// allocation has no counterpart here.
fn allocate_array(
    ast: &Ast,
    element: Id<TypeNode>,
    lvalue: &str,
    lengths: &[i128],
    out: &mut Fill,
    path: &str,
) -> Result<(), Refusal> {
    let (base, nested) = split_array(&rendered_type(ast, element, path)?);
    for level in 0..lengths.len() {
        // The brackets after the sized one: the ranks still to come,
        // plus whatever the element type carries of its own.
        let trailing = "[]".repeat(lengths.len() - level - 1) + &nested;
        let mut at = lvalue.to_string();
        for below in 0..level {
            let _ = write!(at, "[zd_a{below}]");
        }
        let _ = writeln!(
            out.statements,
            "        {at} = new {base}[{}]{trailing};",
            lengths[level]
        );
        if level + 1 < lengths.len() {
            let _ = writeln!(
                out.statements,
                "        for (int zd_a{level} = 0; zd_a{level} < {}; zd_a{level}++)",
                lengths[level]
            );
            out.statements.push_str("        {\n");
        }
    }
    for _ in 0..lengths.len().saturating_sub(1) {
        out.statements.push_str("        }\n");
    }
    Ok(())
}

/// One dimension of an IDL array, recursively.
///
/// zerodds-lint: recursion-depth = the array's declared dimension count
/// (`lengths` is one shorter on every call and the element then goes
/// through [`emit`], whose bound the note there states)
fn fill_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lvalue: &str,
    lengths: &[i128],
    out: &mut Fill,
    path: &str,
) -> Result<(), Refusal> {
    let Some((length, rest)) = lengths.split_first() else {
        return emit(ast, element, value, lvalue, out, path);
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
    for (index, item) in items.iter().enumerate() {
        fill_array(
            ast,
            element,
            item,
            &format!("{lvalue}[{index}]"),
            rest,
            out,
            &format!("{path}[{index}]"),
        )?;
    }
    Ok(())
}

/// A rendered type split into what `new` puts before the count and what
/// it puts after.
///
/// `new int[][2]` is not C#: the sized dimension comes first and the
/// inner ranks are empty, so an array of `int[3]` allocates
/// `new int[2][]` and fills each element with its own `new int[3]`.
fn split_array(rendered: &str) -> (String, String) {
    let base = rendered.trim_end_matches("[]");
    let nested = &rendered[base.len()..];
    (base.to_string(), nested.to_string())
}

/// Where a declaration sits, as a caller has to write it: rooted.
fn class_path(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, Refusal> {
    declared_path(ast, id).map_err(|error| gap!("{path}: {error:?}"))
}

/// The C# spelling of a type, as `new` needs it.
fn rendered_type(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, Refusal> {
    let type_ = <CSharp as CodecTarget>::value_type(ast, id);
    let text = zerodds_lang_render::csharp::type_(&type_);
    if text.starts_with("UnresolvedType") {
        return Err(gap!("{path}: the C# materialiser has no type for this"));
    }
    Ok(text)
}

/// One primitive value, as a C# literal.
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
        // The narrow widths take the decimal alone: ECMA-334 §6.4.5.3's
        // implicit constant expression conversion carries an in-range
        // `int` constant into `sbyte`, `byte`, `short` and `ushort`.
        PrimitiveKind::Int8
        | PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32 => format!("{}", number(value)?),
        // `4294967295` is not an `int`, so the suffix says which type
        // the literal is rather than leaving it to the value.
        PrimitiveKind::UInt32 => format!("{}U", number(value)?),
        PrimitiveKind::Int64 => format!("{}L", number(value)?),
        // The number, not a pattern: C# has an unsigned 64-bit integer
        // and `16045690984833335998UL` is what it is called. This is the
        // one row where the Java column has to convert and this one does
        // not.
        PrimitiveKind::UInt64 => {
            let raw = number(value)?;
            let unsigned = u64::try_from(raw)
                .map_err(|_| gap!("{path}: `{raw}` is not a 64-bit unsigned value"))?;
            format!("{unsigned}UL")
        }
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| gap!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(gap!("{path}: expected exactly one character"));
            };
            // A code point past the BMP has no `char` in C#: a `char` is
            // one UTF-16 code unit, and the generated field is one.
            //
            // `no-form` and not a gap in this tree: the carrier the
            // materialiser chose is what cannot hold the value, which
            // is a statement about C# and is exit 3 by
            // `docs/specs/zerodds-verdicts-1.0.md`. It says nothing
            // about the wire — a `wchar` past the BMP is well formed
            // there, and the C++ and TypeScript columns carry it.
            let code = u32::from(one);
            if code > 0xFFFF {
                return Err(no_form!(
                    "{path}: U+{code:04X} is past the BMP and a C# char holds one code unit"
                ));
            }
            format!("'\\u{code:04X}'")
        }
        // `10000000000` is not an `int` and `NaN` is not a literal at
        // all, so both suffixes and all three special values are written
        // out.
        PrimitiveKind::Float32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| gap!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            csharp_float(f64::from(number as f32), "float", "f")
        }
        PrimitiveKind::Float64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| gap!("{path}: expected a number"))?;
            csharp_float(number, "double", "")
        }
        // Parsed by the runtime from the decimal the corpus gave, for
        // the reason `runtime/cpp/tests/corpus_bytes.cpp` states: a
        // column that parsed it a hundredth of an ulp differently would
        // produce sixteen entirely plausible bytes. And never through
        // `decimal`, which holds neither the range nor the digits.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| gap!("{path}: a binary128 arrives as a decimal string"))?;
            format!(
                "global::ZeroDds.Idl.Float128.FromDecimalString({})",
                csharp_string(text)
            )
        }
    })
}

/// A C# floating-point literal.
///
/// The suffix is not decoration: without it `1.5` is a `double` and
/// `float f = 1.5;` is a compile error rather than a narrowing C#
/// performs for you. The three values that have no literal at all are
/// named instead.
fn csharp_float(number: f64, type_: &str, suffix: &str) -> String {
    if number.is_nan() {
        return format!("{type_}.NaN");
    }
    if number.is_infinite() {
        return if number > 0.0 {
            format!("{type_}.PositiveInfinity")
        } else {
            format!("{type_}.NegativeInfinity")
        };
    }
    format!("{number:?}{suffix}")
}

/// A C# string literal.
///
/// Every non-ASCII character becomes a `\uXXXX` escape rather than
/// arriving as UTF-8 in the source. `csc` reads a file without a BOM as
/// UTF-8 by default, so this is belt and braces — and it is the same
/// belt the Java column wears, where `javac` reads the platform encoding
/// and a probe whose bytes depended on the locale would be measuring the
/// locale.
fn csharp_string(text: &str) -> String {
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
