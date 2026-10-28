// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust expression that prints one decoded HDDS sample as JSON.
//!
//! The read half of [`crate::fill_rust`]. `hddsgen gen rust` emits a
//! `decode_xcdr{1,2}_le` beside every encoder and writes plain structs
//! with `pub` fields, so the sample that comes out is one Rust value and
//! the probe template only has to bind it. This emitter walks the model
//! and writes the expression that turns that value back into the JSON
//! document `wireprobe-compare` reads — the same key names and value
//! forms every other dump backend emits.
//!
//! ## The shape table
//!
//! Every arm here mirrors a row measured in [`crate::fill_rust`]'s
//! module note, because a read that guessed a different shape than the
//! write built would not compile against the generated type:
//!
//! | IDL | what the dump reads |
//! |---|---|
//! | `enum Color { RED }` | `match &sample.c { Color::RED => "RED", … }` |
//! | `long arr[3]` | `[sample.arr[0], sample.arr[1], sample.arr[2]]` |
//! | `@optional long maybe` | a key only inside `if let Some(…) = &sample.maybe` |
//! | `wstring wide` | `sample.wide.clone()` |
//! | `char letter` | `sample.letter.to_string()` |
//! | `sequence<long>` | `sample.ids.iter()` |
//! | `map<K,V>` | an array of `[key, value]` pairs, the form the value file uses |
//! | `struct Mid : Base` | the base's members flattened under their own names, read through the generated `base` field |
//!
//! ## The value forms are `canonical`'s
//!
//! The output is a `serde_json::Value`; `wireprobe-compare` parses it and
//! puts it through `crates/wire-probe/src/canonical.rs` before comparing.
//! Two forms are decided here because only the type can recover them and
//! the comparison would otherwise read a formatting choice as a wire
//! difference:
//!
//! - a 64-bit integer is a **string**, because a JSON number is a double
//!   and loses the low bits past 2^53;
//! - an absent `@optional` member is a key that is **left out**, never a
//!   `null`, which the canonical form refuses.
//!
//! ## What this emitter refuses, and why
//!
//! A shape whose read side was not measured is named, not guessed: a
//! bitset (whose `bits: u64` holder has no measured getter), a `fixed`,
//! a `long double`, a recursive type (which `hddsgen` emits with
//! infinite size and no decoder), and a union whose selected case is a
//! `default:` (whose discriminator the generated data-carrying enum does
//! not store, so `_d` cannot be recovered). Each is an error carrying
//! its own reason; none is a default value.

use core::fmt::Write as _;

use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_wire_codec::encode::ProbeError;

use crate::contract;
use crate::fill_rust::{rust_path, upper_camel};
use crate::target_keywords::rust as rust_identifier;

/// The Rust expression that prints the decoded `sample` as JSON.
///
/// The expression evaluates to a `serde_json::Value` and is meant to be
/// `include!`d where a value is expected; it binds no name of its own
/// and reads the value out of a variable called `sample`.
///
/// # Errors
/// [`ProbeError::Unsupported`] for a construct `hddsgen` has no Rust
/// shape for (a `long double`, a recursive type) and
/// [`ProbeError::Unfilled`] for a construct whose read shape this probe
/// has not measured (a bitset, a `fixed`, a defaulted union). Never a
/// default.
pub fn dump_hdds(ast: &Ast, type_: Id<TypeNode>) -> Result<String, ProbeError> {
    // Before `recursive_aggregates`, which follows a typedef to find the
    // aggregate below it and would itself not end on a chain that
    // returns to itself. See `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let path = contract::root_path(ast, type_);
    // The dump walks the *type*, not a sample, so a type that reaches
    // itself has no bottom. `hddsgen` emits such a type with infinite
    // size and no encoder or decoder at all (measured: `recursive-optional`
    // and `recursive-mutual-optional` both fail to compile), so no
    // sample of one can reach this emitter through the read direction.
    if !crate::recursion::recursive_aggregates(ast, type_).is_empty() {
        return Err(unsupported(
            &path,
            "a recursive type, which hddsgen emits with infinite size and no decoder",
        ));
    }
    let mut out = Dump { next: 0 };
    let body = out.value(ast, type_, "sample", &path)?;
    Ok(format!(
        "{{\n\
         \x20   fn zd_json_number(value: f64) -> serde_json::Value {{\n\
         \x20       match serde_json::Number::from_f64(value) {{\n\
         \x20           Some(number) => serde_json::Value::Number(number),\n\
         \x20           None => panic!(\n\
         \x20               \"the HDDS decoder produced a non-finite float, which JSON cannot write\"\n\
         \x20           ),\n\
         \x20       }}\n\
         \x20   }}\n\
         \x20   fn zd_json_f32(value: f32) -> serde_json::Value {{ zd_json_number(f64::from(value)) }}\n\
         \x20   fn zd_json_f64(value: f64) -> serde_json::Value {{ zd_json_number(value) }}\n\
         \x20   {body}\n\
         }}"
    ))
}

/// The unique-name counter, so two nested loops do not read each
/// other's binding. The C dump grew the same counter the day a nested
/// `sequence<sequence<long>>` indexed the outer collection with the
/// inner counter.
#[derive(Debug, Default)]
struct Dump {
    next: usize,
}

impl Dump {
    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }

    /// The JSON expression for one value, read out of the place
    /// `expr`, whose Rust type is `type_`.
    ///
    /// `expr` is always a Rust *place* of the generated type — `sample`,
    /// `sample.ids[0]`, `(*zd_v_1)` — never a reference, so appending
    /// `.field` and the occasional `&` keeps working at every depth.
    #[allow(clippy::too_many_lines)]
    fn value(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        expr: &str,
        path: &str,
    ) -> Result<String, ProbeError> {
        let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?;
        match &node.kind {
            TypeKind::Primitive(kind) => primitive(*kind, expr, path),
            TypeKind::String { .. } => Ok(format!("serde_json::Value::String(({expr}).clone())")),
            TypeKind::Enumeration { .. } => self.enumeration(ast, type_, expr),
            TypeKind::Bitmask { .. } => self.bitmask(ast, type_, expr),
            TypeKind::Alias { aliased } => {
                let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the typedef target is unresolved"),
                })?;
                self.value(ast, target, expr, path)
            }
            TypeKind::Structure { .. } => self.aggregate(ast, type_, expr, path),
            TypeKind::Sequence { element, .. } => {
                let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the sequence element is unresolved"),
                })?;
                let item = self.temporary("v");
                let inner =
                    self.value(ast, element, &format!("(*{item})"), &format!("{path}[]"))?;
                Ok(format!(
                    "serde_json::Value::Array(({expr}).iter().map(|{item}| {inner}).collect())"
                ))
            }
            TypeKind::Array { element, dims } => {
                let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the array element is unresolved"),
                })?;
                let counts = contract::array_dimensions(ast, dims, path)?;
                self.array(ast, element, expr, path, &counts)
            }
            TypeKind::Union { .. } => self.union(ast, type_, expr, path),
            TypeKind::Map {
                key, value: entry, ..
            } => {
                let key_type = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the map key type is unresolved"),
                })?;
                let value_type = entry.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the map value type is unresolved"),
                })?;
                let key_name = self.temporary("k");
                let value_name = self.temporary("v");
                let key_expr = self.value(
                    ast,
                    key_type,
                    &format!("(*{key_name})"),
                    &format!("{path}[].key"),
                )?;
                let value_expr = self.value(
                    ast,
                    value_type,
                    &format!("(*{value_name})"),
                    &format!("{path}[].value"),
                )?;
                Ok(format!(
                    "serde_json::Value::Array(({expr}).iter().map(|({key_name}, {value_name})| \
                     serde_json::Value::Array(vec![{key_expr}, {value_expr}])).collect())"
                ))
            }
            // A bitset's generated holder is `pub struct N { pub bits: u64 }`
            // with `set_<field>` accessors and SHIFT/MASK constants whose
            // read side was not measured. The value is on the wire, but
            // this emitter has no measured field to read it from.
            TypeKind::Bitset { .. } => Err(unfilled(path, "bitset")),
            TypeKind::Fixed { .. } => Err(unfilled(path, "fixed")),
            other => Err(unfilled(path, other.as_str())),
        }
    }

    /// One structure, its base's members flattened under their own
    /// names and in front of its own — XTypes 1.3 §7.4.1.1.5.1.
    fn aggregate(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        expr: &str,
        path: &str,
    ) -> Result<String, ProbeError> {
        let members = rust_members(ast, type_, expr)?;
        let mut out = String::from("{\n    let mut zd_object = serde_json::Map::new();\n");
        for member in members {
            let member_path = contract::join(path, &member.name);
            if member.optional {
                // XTypes 1.3 §7.4.1.2.3: an absent member is a key that
                // is left out, never written as `null`, which the
                // canonical form refuses.
                let bound = self.temporary("some");
                let inner = self.value(ast, member.type_, &format!("(*{bound})"), &member_path)?;
                let _ = writeln!(
                    out,
                    "    if let Some({bound}) = &{accessor} {{",
                    accessor = member.accessor
                );
                let _ = writeln!(
                    out,
                    "        zd_object.insert(\"{name}\".to_string(), {inner});",
                    name = member.name
                );
                out.push_str("    }\n");
            } else {
                let inner = self.value(ast, member.type_, &member.accessor, &member_path)?;
                let _ = writeln!(
                    out,
                    "    zd_object.insert(\"{name}\".to_string(), {inner});",
                    name = member.name
                );
            }
        }
        out.push_str("    serde_json::Value::Object(zd_object)\n}");
        Ok(out)
    }

    /// One enumeration, printed as the enumerator's name.
    ///
    /// `hddsgen` emits a unit-variant Rust enum, so each variant is a
    /// name and the match reproduces it. An out-of-range value cannot
    /// reach here: the generated decoder is what reads the holder, and
    /// a value naming no enumerator is its refusal.
    fn enumeration(
        &self,
        ast: &Ast,
        type_: Id<TypeNode>,
        expr: &str,
    ) -> Result<String, ProbeError> {
        let path = rust_path(ast, type_);
        let mut arms = String::new();
        for id in ast.enumerators_of(type_) {
            let node = ast.enumerator(*id).map_err(|_| ProbeError::NoLayout {
                detail: format!("{expr}: an enumerator of {path} does not resolve"),
            })?;
            let _ = writeln!(arms, "{path}::{} => \"{}\",", node.name, node.name);
        }
        Ok(format!(
            "serde_json::Value::String(String::from(match &{expr} {{\n{arms}}}))"
        ))
    }

    /// A bitmask, printed as the set bits' names in declaration order.
    ///
    /// The names come from the type and the bits from the generated
    /// `u64`, read through the constants `hddsgen` emits — the same
    /// `NAME_FLAG` spelling the fill side ORs together.
    fn bitmask(&self, ast: &Ast, type_: Id<TypeNode>, expr: &str) -> Result<String, ProbeError> {
        let mut out = String::from("{\n    let mut zd_flags = Vec::new();\n");
        for id in ast.bits_of(type_) {
            let node = ast.bit_value(*id).map_err(|_| ProbeError::NoLayout {
                detail: format!("{expr}: a bit of the bitmask does not resolve"),
            })?;
            let constant = bitmask_constant(ast, type_, &node.name);
            let _ = writeln!(
                out,
                "    if (({expr}) & {constant}) != 0 {{ \
                 zd_flags.push(serde_json::Value::String(\"{name}\".to_string())); }}",
                name = node.name
            );
        }
        out.push_str("    serde_json::Value::Array(zd_flags)\n}");
        Ok(out)
    }

    /// A union, as `{"_d": …}` plus the member the selected variant
    /// carries.
    ///
    /// A case selected by `default:` is refused: `hddsgen`'s generated
    /// enum stores the member and bakes a discriminator into the
    /// variant's encoder, so nothing in the decoded value says which
    /// value on the wire selected the default. A labelled case's first
    /// label is the discriminator the encoder writes, so it is
    /// recoverable and emitted.
    fn union(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        expr: &str,
        path: &str,
    ) -> Result<String, ProbeError> {
        let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?;
        let TypeKind::Union { discriminator, .. } = &node.kind else {
            return Err(unfilled(
                path,
                "a union whose kind changed under the emitter",
            ));
        };
        let discriminator = discriminator
            .resolved()
            .ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the discriminator is unresolved"),
            })?;

        let mut out = String::from("{\n    let mut zd_object = serde_json::Map::new();\n");
        let _ = writeln!(out, "    match &{expr} {{");
        for case in contract::union_cases(ast, type_) {
            let mut label = None;
            for candidate in &case.labels {
                if let Some(expr) = candidate.expr() {
                    label = Some(expr);
                    break;
                }
            }
            let Some(label) = label else {
                return Err(unfilled(
                    path,
                    "a union case selected by `default:`, whose discriminator hddsgen's \
                     generated enum does not carry",
                ));
            };
            let value = ast
                .eval_integral(label)
                .map_err(|error| ProbeError::NoLayout {
                    detail: format!("{path}: a case label does not evaluate: {error}"),
                })?;
            let printed = Self::discriminator(ast, discriminator, value, path)?;
            let payload = self.temporary("p");
            let member_path = contract::join(path, &case.name);
            let inner = self.value(ast, case.type_, &format!("(*{payload})"), &member_path)?;
            let variant = format!("{}::{}", rust_path(ast, type_), upper_camel(&case.name));
            let name = case.name.clone();
            let _ = writeln!(
                out,
                "        {variant}({payload}) => {{\n\
                 \x20           zd_object.insert(\"_d\".to_string(), {printed});\n\
                 \x20           zd_object.insert(\"{name}\".to_string(), {inner});\n\
                 \x20       }}"
            );
        }
        out.push_str("    }\n    serde_json::Value::Object(zd_object)\n}");
        Ok(out)
    }

    /// The JSON form of a union's discriminator literal.
    ///
    /// Recovered from a case label rather than read from storage, so it
    /// is a constant: the primitive shapes are written as literals and
    /// the 64-bit widths as strings, the canonical form of the
    /// discriminator's own type.
    fn discriminator(
        ast: &Ast,
        type_: Id<TypeNode>,
        value: i128,
        path: &str,
    ) -> Result<String, ProbeError> {
        let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?;
        match &node.kind {
            TypeKind::Primitive(PrimitiveKind::Boolean) => {
                Ok(format!("serde_json::Value::Bool({})", value != 0))
            }
            TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
                let scalar = u32::try_from(value).ok().and_then(char::from_u32);
                let character = scalar.ok_or_else(|| ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: "a case label that is a character".to_string(),
                    found: format!("{value}"),
                })?;
                Ok(format!(
                    "serde_json::Value::String('\\u{{{:x}}}'.to_string())",
                    u32::from(character)
                ))
            }
            TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => Ok(format!(
                "serde_json::Value::String(\"{value}\".to_string())"
            )),
            TypeKind::Primitive(kind) if is_integer(*kind) => {
                Ok(format!("serde_json::Value::from({value}i64)"))
            }
            TypeKind::Enumeration { .. } => {
                let mut name = None;
                for id in ast.enumerators_of(type_) {
                    if ast.effective_enumerator_value(*id) == Some(value) {
                        if let Ok(node) = ast.enumerator(*id) {
                            name = Some(node.name.clone());
                        }
                        break;
                    }
                }
                let name = name.ok_or_else(|| ProbeError::NoLayout {
                    detail: format!(
                        "{path}: a case label names no enumerator of the discriminator"
                    ),
                })?;
                Ok(format!("serde_json::Value::String(\"{name}\".to_string())"))
            }
            TypeKind::Alias { aliased } => {
                let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the discriminator's typedef is unresolved"),
                })?;
                Self::discriminator(ast, target, value, path)
            }
            _ => Err(unfilled(
                path,
                "a union discriminator whose canonical form this emitter does not build",
            )),
        }
    }

    /// One array, one index per dimension, innermost last.
    ///
    /// zerodds-lint: recursion-depth 8 — one frame per array dimension.
    /// `counts` drops its head at each frame and the innermost element
    /// goes to [`Self::value`], which is bounded because a cycle among
    /// aggregates has been refused at the entry.
    fn array(
        &mut self,
        ast: &Ast,
        element: Id<TypeNode>,
        expr: &str,
        path: &str,
        counts: &[usize],
    ) -> Result<String, ProbeError> {
        let Some((count, rest)) = counts.split_first() else {
            return self.value(ast, element, expr, path);
        };
        let mut items = Vec::with_capacity(*count);
        for index in 0..*count {
            items.push(self.array(
                ast,
                element,
                &format!("{expr}[{index}]"),
                &format!("{path}[{index}]"),
                rest,
            )?);
        }
        Ok(format!(
            "serde_json::Value::Array(vec![{}])",
            items.join(", ")
        ))
    }
}

/// One member of a structure, with the Rust place that reads it.
struct Accessor {
    name: String,
    type_: Id<TypeNode>,
    accessor: String,
    optional: bool,
}

/// Every member of a structure, a base's before its own, with the place
/// each is read from.
///
/// `hddsgen` does not flatten inheritance: it nests the base in a field
/// called `base`, so `struct Mid : Base` reaches `base_a` as
/// `sample.base.base_a`. The JSON stays flat under the member's own
/// name, because the wire and the canonical form are flat.
///
/// zerodds-lint: recursion-depth 64 — one frame per base,
/// `zerodds_idl_lower::complete::MAX_INHERITANCE_DEPTH`, which is also
/// what `Ast::all_members_of` walks. An inheritance cycle is invalid IDL
/// and the frontend reports it; a base chain in a model that passed
/// `model_from_idl` is finite.
fn rust_members(ast: &Ast, type_: Id<TypeNode>, expr: &str) -> Result<Vec<Accessor>, ProbeError> {
    let mut out = Vec::new();
    if let Ok(node) = ast.type_(type_) {
        if let TypeKind::Structure {
            base: Some(base), ..
        } = &node.kind
        {
            let base = base.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{expr}: the base type is unresolved"),
            })?;
            out.extend(rust_members(ast, base, &format!("{expr}.base"))?);
        }
    }
    for id in ast.members_of(type_) {
        let member = ast.member(*id).map_err(|_| ProbeError::NoLayout {
            detail: format!("{expr}: a member does not resolve"),
        })?;
        let Some(member_type) = member.type_.resolved() else {
            return Err(ProbeError::NoLayout {
                detail: format!("{expr}.{}: the member type is unresolved", member.name),
            });
        };
        out.push(Accessor {
            name: member.name.clone(),
            type_: member_type,
            accessor: format!("{expr}.{}", rust_identifier(&member.name)),
            optional: member.facets.is_optional(),
        });
    }
    Ok(out)
}

/// The one primitive shape each width takes in a JSON document.
fn primitive(kind: PrimitiveKind, expr: &str, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => format!("serde_json::Value::Bool({expr})"),
        // `hddsgen` makes both `char` and `wchar` a Rust `char`, and the
        // canonical form of one is a one-character string.
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            format!("serde_json::Value::String(({expr}).to_string())")
        }
        PrimitiveKind::Float32 => format!("zd_json_f32({expr})"),
        PrimitiveKind::Float64 => format!("zd_json_f64({expr})"),
        // A 64-bit integer is a string; see the module note.
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => {
            format!("serde_json::Value::String(({expr}).to_string())")
        }
        // No catch-all: a primitive kind added later is a compile error
        // here rather than a string that reads as some other width.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32 => format!("serde_json::Value::from({expr})"),
        // `hddsgen` has no Rust primitive for a `long double`, which is
        // the same limit the fill side refuses by.
        PrimitiveKind::Float128 => {
            return Err(unsupported(
                path,
                "a long double, which has no Rust primitive in hddsgen's output",
            ));
        }
    })
}

/// Whether a primitive is an integer, for the discriminator arms.
const fn is_integer(kind: PrimitiveKind) -> bool {
    matches!(
        kind,
        PrimitiveKind::Octet
            | PrimitiveKind::UInt8
            | PrimitiveKind::Int8
            | PrimitiveKind::Int16
            | PrimitiveKind::UInt16
            | PrimitiveKind::Int32
            | PrimitiveKind::UInt32
    )
}

/// The constant `hddsgen` emits for one bitmask flag: `NAME_FLAG`, with
/// the module path flattened in front. The same rule the fill side ORs
/// together; the two are separate copies because each is compared
/// against `hddsgen` through a different direction.
fn bitmask_constant(ast: &Ast, type_: Id<TypeNode>, flag: &str) -> String {
    let scoped = rust_path(ast, type_);
    let (prefix, type_name) = match scoped.rsplit_once("::") {
        Some((module, name)) => (format!("{module}::"), name.to_string()),
        None => (String::new(), scoped),
    };
    format!(
        "{prefix}{}_{}",
        type_name.to_uppercase(),
        flag.to_uppercase()
    )
}

fn unsupported(path: &str, kind: &'static str) -> ProbeError {
    ProbeError::Unsupported {
        path: path.to_string(),
        kind,
    }
}

fn unfilled(path: &str, kind: &'static str) -> ProbeError {
    ProbeError::Unfilled {
        path: path.to_string(),
        kind,
    }
}
