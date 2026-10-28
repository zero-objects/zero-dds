// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The statements that put one JSON sample into the generated Java.
//!
//! The counterpart of `wireprobe-zerodds-c-gen/src/fill.rs`, which does
//! the same for the C shapes. It knows the JSON dialect and the Java
//! mapping and nothing else: it places values, the codec frames them and
//! `runtime/java` writes them. A filler that computed a bit pattern or a
//! length would move a rule out of the runtime, and having a second
//! implementation of the rule is the whole reason for a second column.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs` documents:
//! a 64-bit integer is a string, an absent `@optional` member is a key
//! left out rather than a `null`, and a map is an array of two-element
//! arrays.
//!
//! ## What the Java mapping makes of each construct
//!
//! Read off `crates/lang-rules/src/adapter/java.rs`, which is where the
//! decisions are made and argued. Repeated here as a table because a
//! filler that guessed one of these would produce source that compiles
//! against nothing.
//!
//! | IDL | Java | what this writes |
//! |---|---|---|
//! | `boolean` | `boolean` | `true` / `false` |
//! | `char`, `wchar` | `char` | a `'\uXXXX'` escape |
//! | `int8` | `byte` | `(byte) n` |
//! | `octet`, `uint8`, `unsigned short` | `int` | a number |
//! | `short` | `short` | `(short) n` |
//! | `long` | `int` | a number |
//! | `unsigned long`, `long long` | `long` | `nL` |
//! | `unsigned long long` | `long` | the **pattern**, as a signed decimal with `L` |
//! | `float` | `float` | `nf` |
//! | `double` | `double` | a number |
//! | `long double` | `Float128` | `Float128.fromDecimalString("…")` |
//! | `string`, `wstring`, `fixed` | `String` | a quoted literal |
//! | `sequence<T>`, `T[N]` | `T[]` | `new T[]{ … }` |
//! | `map<K,V>` | `ZdEntry_k_v[]` | an array of entries, each with `key` and `value` |
//! | `enum` | the enumeration class | the object of the enumerator |
//! | `bitmask` | the holder | an `or` of the named bits |
//! | `bitset` | a class per bitfield | one assignment per field |
//! | `union` | `_d` and one field per case | the discriminator and the selected member |
//! | `@optional T` | the boxed `T`, `null` when absent | the value, or the key left out |
//!
//! ## `unsigned long long` is the one that has to be converted here
//!
//! Java's widest integer is signed, so 16045690984833335998 has no
//! literal. `adapter/java.rs` holds it as a **pattern** in a `long` and
//! `Writer.writeU64(long pattern)` takes it that way, so this writes the
//! same bits as a signed decimal: `-2401053088876215618L`. Anything else
//! would not compile, and a `BigInteger` would be a second holder for a
//! value the runtime already decided how to keep.

use serde_json::Value;
use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::java::Java;
use zerodds_wireprobe_gen::{Refusal, gap, no_form};

/// The Java the filler produced.
///
/// Statements and no declarations, which is where this differs from the
/// C filler: a C sequence points into file-scope storage that has to
/// outlive the fill, and a Java array is a value the sample holds.
#[derive(Debug, Default)]
pub struct Fill {
    statements: String,
    counter: usize,
}

impl Fill {
    /// A local name no other statement of this fill uses.
    fn temporary(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("zd_{prefix}_{}", self.counter)
    }

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

        TypeKind::String { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            let _ = writeln!(out.statements, "        {lvalue} = {};", java_string(text));
            Ok(())
        }

        // A `fixed` is a `java.math.BigDecimal` (CORBA 3.3 part 3 §1.5.6), built from the
        // decimal text the corpus gave.
        TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            let _ = writeln!(
                out.statements,
                "        {lvalue} = new java.math.BigDecimal({});",
                java_string(text)
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
                    found = Some(enumerator.name.clone());
                }
            }
            let Some(constant) = found else {
                return Err(gap!("{path}: `{name}` is not one of {declared:?}"));
            };
            // A member of the enumeration is the enumeration class (CORBA 3.3
            // part 3 1.5.9) and the value is the object `<label>` of it beside
            // the int constant `_<label>` (XTypes 1.3 7.5.1.2.1.3); the class
            // sits where the declaration does — nested, not flattened into
            // the codec's name.
            let holder = class_path(ast, type_, path)?;
            let object = Java::escape(&constant);
            let _ = writeln!(out.statements, "        {lvalue} = {holder}.{object};");
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
                terms.push(format!("{holder}.{}", Java::escape(name)));
            }
            let value = if terms.is_empty() {
                "0".to_string()
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
                let holder = if field.bits > 32 {
                    format!("{number}L")
                } else {
                    format!("{number}")
                };
                let _ = writeln!(
                    out.statements,
                    "        {lvalue}.{} = {holder};",
                    Java::escape(name)
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
                let field = format!("{lvalue}.{}", Java::escape(&name));
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(gap!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        emit(ast, member_type, member_value, &field, out, &member_path)?
                    }
                    // Absence is the value: `adapter/java.rs` declares
                    // one boxed field and lets `null` mean absent, so a
                    // member left out needs no flag cleared.
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
                &format!("{lvalue}._d"),
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
                    &format!("{lvalue}.{}", Java::escape(name)),
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
            // Allocated and then written into, element by element: a
            // Java array initialiser cannot hold the statements a nested
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
            let mut brackets = String::new();
            for length in &lengths {
                let _ = write!(brackets, "[{length}]");
            }
            let (base, nested) = split_array(&rendered_type(ast, element, path)?);
            let _ = writeln!(
                out.statements,
                "        {lvalue} = new {base}{brackets}{nested};"
            );
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
            let entry = Java::map_entry_name(ast, type_);
            // A key a `java.util.Map` can be keyed by makes the member a map
            // (XTypes 1.3 §7.5.1.3.5): the entries are filled into an array
            // and the map is made of it. Any other key keeps the array.
            let class = if zerodds_lang_rules::adapter::java::map_key_supported(ast, key_type) {
                Some(format!(
                    "{}.{}",
                    zerodds_lang_rules::adapter::java::OUTER,
                    zerodds_lang_rules::adapter::java::map_class_name(ast, type_)
                        .map_err(|error| gap!("{path}: the map has no class: {error:?}"))?
                ))
            } else {
                None
            };
            let target = if class.is_some() {
                out.temporary("entries")
            } else {
                lvalue.to_string()
            };
            if class.is_some() {
                let _ = writeln!(
                    out.statements,
                    "        {entry}[] {target} = new {entry}[{}];",
                    entries.len()
                );
            } else {
                let _ = writeln!(
                    out.statements,
                    "        {target} = new {entry}[{}];",
                    entries.len()
                );
            }
            for (index, pair) in entries.iter().enumerate() {
                let pair = pair
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| gap!("{path}[{index}]: expected a two-element array"))?;
                let at = format!("{target}[{index}]");
                let _ = writeln!(out.statements, "        {at} = new {entry}();");
                emit(
                    ast,
                    key_type,
                    &pair[0],
                    &format!("{at}.key"),
                    out,
                    &format!("{path}[{index}].key"),
                )?;
                emit(
                    ast,
                    value_type,
                    &pair[1],
                    &format!("{at}.value"),
                    out,
                    &format!("{path}[{index}].value"),
                )?;
            }
            if let Some(class) = class {
                let _ = writeln!(out.statements, "        {lvalue} = {class}.of({target});");
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
            "{path}: {other:?} has no Java form this filler can build"
        )),
    }
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
/// `new int[][2]` is not Java: the dimensions go outermost first, so a
/// sequence of `int[3]` allocates `new int[2][]` and fills each element
/// with its own `new int[3]`. Splitting off the trailing brackets is
/// what puts the count in the first pair.
fn split_array(rendered: &str) -> (String, String) {
    let base = rendered.trim_end_matches("[]");
    let nested = &rendered[base.len()..];
    (base.to_string(), nested.to_string())
}

/// Where a declaration sits, as a caller has to write it.
fn class_path(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, Refusal> {
    zerodds_lang_rules::adapter::java::declared_path(ast, id)
        .map_err(|error| gap!("{path}: {error:?}"))
}

/// The Java spelling of a type, as `new` needs it.
fn rendered_type(ast: &Ast, id: Id<TypeNode>, path: &str) -> Result<String, Refusal> {
    let type_ = <Java as CodecTarget>::value_type(ast, id);
    let text = zerodds_lang_render::java::type_(&type_);
    if text.starts_with("UnresolvedType") {
        return Err(gap!("{path}: the Java materialiser has no type for this"));
    }
    Ok(text)
}

/// One primitive value, as a Java literal.
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
        // A number in [0, 255] or [0, 65535] in an `int`, because Java's
        // byte and short are signed and 200 in a byte is −56.
        PrimitiveKind::UInt8 | PrimitiveKind::UInt16 | PrimitiveKind::Int32 => {
            format!("{}", number(value)?)
        }
        // `octet` is a `byte` (CORBA 3.3 part 3 §1.5.2): 200 is the bit pattern of -56.
        PrimitiveKind::Octet | PrimitiveKind::Int8 => format!("(byte) {}", number(value)?),
        PrimitiveKind::Int16 => format!("(short) {}", number(value)?),
        PrimitiveKind::UInt32 | PrimitiveKind::Int64 => format!("{}L", number(value)?),
        // The pattern, not the number: Java has no wider integer, and
        // `Writer.writeU64` takes the bits. 16045690984833335998 is
        // −2401053088876215618 with the same sixty-four bits.
        PrimitiveKind::UInt64 => {
            let raw = number(value)?;
            let pattern = u64::try_from(raw)
                .map_err(|_| gap!("{path}: `{raw}` is not a 64-bit unsigned value"))?;
            #[allow(clippy::cast_possible_wrap)]
            let signed = pattern as i64;
            format!("{signed}L")
        }
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| gap!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(gap!("{path}: expected exactly one character"));
            };
            // A code point past the BMP has no `char` in Java; the
            // runtime takes an `int` code point and the generated field
            // is a `char`, so this is where it would be lost.
            //
            // `no-form` and not a gap in this tree: the carrier the
            // materialiser chose is what cannot hold the value, which
            // is a statement about Java and is exit 3 by
            // `docs/specs/zerodds-verdicts-1.0.md`. It says nothing
            // about the wire — a `wchar` past the BMP is well formed
            // there, and the C++ and TypeScript columns carry it.
            let code = u32::from(one);
            if code > 0xFFFF {
                return Err(no_form!(
                    "{path}: U+{code:04X} is past the BMP and a Java char holds one code unit"
                ));
            }
            format!("'\\u{code:04X}'")
        }
        // `10000000000` is an `int` literal Java will not accept and
        // `NaN` is not a literal at all, so both suffixes and both
        // special values are written out.
        PrimitiveKind::Float32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| gap!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            java_float(number as f32 as f64, "Float", "f")
        }
        PrimitiveKind::Float64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| gap!("{path}: expected a number"))?;
            java_float(number, "Double", "d")
        }
        // Parsed by the runtime from the decimal the corpus gave, for
        // the reason `runtime/cpp/tests/corpus_bytes.cpp` states: a
        // column that parsed it a hundredth of an ulp differently would
        // produce sixteen entirely plausible bytes.
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| gap!("{path}: a binary128 arrives as a decimal string"))?;
            format!("Float128.fromDecimalString({})", java_string(text))
        }
    })
}

/// A Java floating-point literal.
///
/// The suffix is not decoration: without it `10000000000` is an `int`
/// literal too large for its type and the probe does not compile. The
/// three values that have no literal at all are named instead.
fn java_float(number: f64, boxed: &str, suffix: &str) -> String {
    if number.is_nan() {
        return format!("{boxed}.NaN");
    }
    if number.is_infinite() {
        return if number > 0.0 {
            format!("{boxed}.POSITIVE_INFINITY")
        } else {
            format!("{boxed}.NEGATIVE_INFINITY")
        };
    }
    format!("{number:?}{suffix}")
}

/// A Java string literal.
///
/// Every non-ASCII character becomes a `\uXXXX` escape rather than
/// arriving as UTF-8 in the source: `javac` reads a file in the platform
/// encoding unless told otherwise, and a probe whose bytes depended on
/// the locale would be measuring the locale.
fn java_string(text: &str) -> String {
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
