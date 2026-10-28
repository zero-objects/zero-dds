// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The statements that print one filled C# sample as canonical JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into the
//! generated C#. This takes one back out, after `runtime/csharp` and the
//! generated decoder have filled it from the bytes.
//!
//! ## The dialect is the shared one
//!
//! `crates/wire-probe/src/canonical.rs` defines what a value document
//! means and `crates/wire-probe/src/contract.rs` holds the rules all the
//! backends answer by. Two of them cannot be recovered by any reader and
//! so have to be right here:
//!
//! - a 64-bit integer is a **string**, because a JSON number is a double
//!   and loses the low bits past 2^53;
//! - an absent `@optional` member is a key that is **left out**, never a
//!   `null`, which the canonical form refuses.
//!
//! `tools/wireprobe/Zdj.cs` holds the helpers, the C# counterpart of
//! `tools/wireprobe/Zdj.java` and of `tools/wireprobe/dumpjson.h`.
//!
//! ## Why it is built at run time and not baked in
//!
//! A sequence's length, a union's selected member and an `@optional`
//! member's presence are all decided by the bytes, not by the type. So
//! the punctuation is statements — `Zdj.Key` puts the comma in only when
//! something has already been printed at that level — rather than one
//! `print` per aggregate.
//!
//! ## The one thing C# adds over the Java dumper
//!
//! An `@optional` member of a value type is a `Nullable<T>`, and every
//! helper here takes a `T`. So the printer unwraps it — `.Value`, inside
//! the branch the presence test guarded — where the Java column's boxed
//! field needed nothing. `adapter/csharp.rs::is_nullable_wrapped` is the
//! one place that question is answered, for this and for the encoder.

use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::csharp::{declared_path, field_name, is_nullable_wrapped};
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::csharp::CSharp;

/// The statements, ready to paste into the probe's dump method.
#[derive(Debug, Default)]
struct Dump {
    body: String,
    depth: usize,
    /// The types this descent is inside, innermost last.
    ///
    /// The printer walks the **type** and not the value, so a type that
    /// reaches itself has no bottom: `struct Node { sequence<Node> }` is
    /// finite in every sample and infinite as a shape.
    active: Vec<Id<TypeNode>>,
    /// The aggregates on a cycle, each of which gets a method of its
    /// own — `ZdRecDump<n>` — instead of an inline expansion, which for
    /// a cycle has no end. The same shape the C and C++ emitters carry
    /// (`zerodds_wire_probe::recursion`).
    rec: Vec<Id<TypeNode>>,
    /// A counter, so two loops in one sample get distinct indices.
    next: usize,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    union_label: UnionLabelMatch,
}

impl Dump {
    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.body.push_str("    ");
        }
        self.body.push_str(text);
        self.body.push('\n');
    }

    fn open(&mut self, text: &str) {
        self.line(text);
        self.depth += 1;
    }

    fn close(&mut self, text: &str) {
        self.depth = self.depth.saturating_sub(1);
        self.line(text);
    }

    fn raw(&mut self, literal: &str) {
        let escaped = literal.replace('\\', "\\\\").replace('"', "\\\"");
        self.line(&format!("Zdj.Raw(\"{escaped}\");"));
    }

    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// The two halves of the printer: methods for the types on a cycle,
/// and the statements for the dump method's body.
#[derive(Debug, Default)]
pub struct DumpParts {
    /// `static void ZdRecDump<n>(T zdv)` definitions, for the class
    /// holding [`Self::body`].
    pub functions: String,
    /// The statements that print `zd_sample` as canonical JSON.
    pub body: String,
}

/// The C# spelling of one type, as the harness spells the sample's.
fn spelled(ast: &Ast, id: Id<TypeNode>) -> String {
    zerodds_lang_render::csharp::type_(&<CSharp as CodecTarget>::value_type(ast, id))
}

/// Emit the statements that print `zd_sample` as canonical JSON.
///
/// # Errors
/// A message naming the path in the type, in the same shape the fill
/// side reports one.
pub fn dump(
    ast: &Ast,
    type_: Id<TypeNode>,
    union_label: UnionLabelMatch,
) -> Result<DumpParts, String> {
    let mut out = Dump {
        union_label,
        rec: zerodds_wire_probe::recursion::recursive_aggregates(ast, type_),
        ..Dump::default()
    };
    // No forward declaration: a C# method sees every other member of
    // its class regardless of order, so a mutual cycle needs nothing
    // that the C++ column's file-scope functions needed.
    let functions = if out.rec.is_empty() {
        String::new()
    } else {
        for position in 0..out.rec.len() {
            let id = out.rec[position];
            let name = spelled(ast, id);
            if position > 0 {
                out.body.push('\n');
            }
            out.line(&format!("static void ZdRecDump{position}({name} zdv)"));
            out.open("{");
            out.active.push(id);
            let outcome = emit_inner(ast, id, "zdv", &mut out, &format!("<{name}>"));
            out.active.pop();
            outcome?;
            out.close("}");
        }
        std::mem::take(&mut out.body)
    };
    emit(ast, type_, "zd_sample", &mut out, "sample")?;
    out.line("Zdj.Raw(\"\\n\");");
    Ok(DumpParts {
        functions,
        body: out.body,
    })
}

/// Emit the statements that print `expr` under `type_`.
///
/// zerodds-lint: recursion-depth = distinct types on the descent path,
/// via [`emit_inner`]. A type on a cycle is a call into its file-scope
/// function — `recursive_aggregates` names them and `Dump::rec` holds
/// them — so a cycle is never unrolled; a type the collector missed is
/// caught by `Dump::active` and refused by name rather than descended
/// into. That is the guard the CORBA C++ emitter did not have when
/// `A -> sequence<B> -> B -> sequence<A>` ran the generator's stack out.
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;

    // A type on a cycle is a call into its own method, never an inline
    // expansion — the expansion of a cycle has no end.
    if let Some(position) = out.rec.iter().position(|id| *id == type_) {
        out.line(&format!("ZdRecDump{position}({expr});"));
        return Ok(());
    }
    if out.active.contains(&type_) {
        return Err(format!(
            "{path}: `{}` reaches itself outside the collected cycles — a defect in \
             the cycle collector, not in the IDL",
            node.name
        ));
    }
    out.active.push(type_);
    let outcome = emit_inner(ast, type_, expr, out, path);
    out.active.pop();
    outcome
}

/// The body of [`emit`], with `type_` already on the descent path.
///
/// zerodds-lint: recursion-depth = distinct types on the descent path
/// (every recursive call goes back through [`emit`], which pushes onto
/// `Dump::active` and refuses a repeat)
fn emit_inner(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            out.line(&primitive_call(*kind, expr));
            Ok(())
        }
        TypeKind::String { width, .. } => {
            out.line(&match width {
                CharWidth::Char => format!("Zdj.String({expr});"),
                CharWidth::WChar => format!("Zdj.Wstring({expr});"),
            });
            Ok(())
        }
        // A `fixed<P,S>` is the decimal it was written as — this column
        // holds it as a `string`, for the reason `adapter/csharp.rs`
        // gives — and the canonical form is a string for the reason the
        // type exists: a double cannot carry one and neither can a
        // `decimal`.
        TypeKind::Fixed { .. } => {
            out.line(&format!("Zdj.Fixed({expr});"));
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the typedef target is unresolved"))?;
            emit(ast, target, expr, out, path)
        }
        TypeKind::Enumeration { .. } => emit_enumeration(ast, type_, expr, out, path),
        TypeKind::Bitmask { .. } => emit_bitmask(ast, type_, expr, out, path),
        TypeKind::Bitset { .. } => {
            // The whole list, base first: a derived `bitset` carries the
            // base's bitfields and the decoder fills them, so printing
            // only the derived type's own would return a value with
            // members missing and say nothing about it.
            let first = out.temporary("first");
            out.open("{");
            out.line(&format!("bool[] {first} = new bool[] {{ true }};"));
            out.raw("{");
            for field in &ast.all_bitfields_of(type_) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding and carries no value.
                let Some(name) = field_node.name.as_ref() else {
                    continue;
                };
                out.line(&format!("Zdj.Key({first}, \"{name}\");"));
                out.line(&format!("Zdj.U64((ulong)({expr}.{}));", field_name(name)));
            }
            out.raw("}");
            out.close("}");
            Ok(())
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            emit_aggregate(ast, type_, expr, out, path)
        }
        TypeKind::Union { .. } => emit_union(ast, type_, expr, out, path),
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the array element is unresolved"))?;
            let counts: Result<Vec<i128>, String> = dims
                .iter()
                .map(|dim| {
                    ast.eval_integral(*dim)
                        .map_err(|_| format!("{path}: an array length does not evaluate"))
                })
                .collect();
            emit_array(ast, element, expr, out, path, &counts?)
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the sequence element is unresolved"))?;
            emit_sequence(ast, element, expr, out, path)
        }
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{path}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{path}: the map value type is unresolved"))?;
            emit_map(ast, key_type, value_type, expr, out, path)
        }
        other => Err(format!(
            "{path}: {} has no C# shape to print from",
            other.as_str()
        )),
    }
}

/// A structure or a valuetype, as a JSON object in wire order.
fn emit_aggregate(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let first = out.temporary("first");
    out.open("{");
    out.line(&format!("bool[] {first} = new bool[] {{ true }};"));
    out.raw("{");
    for member in ast.all_members_of(type_) {
        let Ok(member_node) = ast.member(member) else {
            continue;
        };
        let member_type = member_node
            .type_
            .resolved()
            .ok_or_else(|| format!("{path}.{}: the member type is unresolved", member_node.name))?;
        let name = member_node.name.clone();
        let field = field_name(&name);
        let member_path = format!("{path}.{name}");
        if member_node.facets.is_optional() {
            // An absent member is a key that is not printed. `null`
            // would read as "present and empty" to one backend and as
            // "absent" to another, and the canonical form refuses it.
            out.open(&format!("if ({expr}.{field} != null) {{"));
            out.line(&format!("Zdj.Key({first}, \"{name}\");"));
            // A value type's `@optional` member is a `Nullable<T>` and
            // every helper takes a `T`; a reference type was never
            // wrapped. Inside this branch `.Value` is defined.
            let held = if is_nullable_wrapped(ast, member_type) {
                format!("{expr}.{field}.Value")
            } else {
                format!("{expr}.{field}")
            };
            emit(ast, member_type, &held, out, &member_path)?;
            out.close("}");
            continue;
        }
        out.line(&format!("Zdj.Key({first}, \"{name}\");"));
        emit(
            ast,
            member_type,
            &format!("{expr}.{field}"),
            out,
            &member_path,
        )?;
    }
    out.raw("}");
    out.close("}");
    Ok(())
}

/// A union, as `{"_d": …}` plus the member the discriminator selects.
fn emit_union(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;
    let TypeKind::Union { discriminator, .. } = &node.kind else {
        return Err(format!("{path}: not a union"));
    };
    let discriminator = discriminator
        .resolved()
        .ok_or_else(|| format!("{path}: the discriminator is unresolved"))?;

    let first = out.temporary("first");
    let selector = format!("{expr}.{}", field_name("_d"));
    out.open("{");
    out.line(&format!("bool[] {first} = new bool[] {{ true }};"));
    out.raw("{");
    out.line(&format!("Zdj.Key({first}, \"_d\");"));
    emit(ast, discriminator, &selector, out, &format!("{path}._d"))?;

    // The one crossing `UnionLabelMatch` has, asked of the same
    // discriminator the generated decoder asked it of.
    let suppress_negative = union_negative_matches_nothing(ast, type_, out.union_label);
    let mut arms: Vec<(String, String, Id<TypeNode>)> = Vec::new();
    let mut fallback: Option<(String, Id<TypeNode>)> = None;
    for case in ast.cases_of(type_) {
        let Ok(case_node) = ast.case(*case) else {
            continue;
        };
        let Ok(member) = ast.member(case_node.member) else {
            continue;
        };
        let member_type = member
            .type_
            .resolved()
            .ok_or_else(|| format!("{path}.{}: the case type is unresolved", member.name))?;
        let mut condition = String::new();
        let mut is_default = false;
        for label in &case_node.labels {
            match label.expr() {
                None => is_default = true,
                Some(value_expr) => {
                    let value = ast.eval_integral(value_expr).map_err(|error| {
                        format!("{path}: a case label does not evaluate: {error}")
                    })?;
                    // A label the setting says selects nothing gets no
                    // test, so the chain falls to the default where the
                    // union has one and prints only `_d` where it has
                    // none — which is what the generated decoder left in
                    // the sample.
                    if suppress_negative && value < 0 {
                        continue;
                    }
                    if !condition.is_empty() {
                        condition.push_str(" || ");
                    }
                    let _ = write!(
                        condition,
                        "{selector} == {}",
                        csharp_label(ast, discriminator, value)
                    );
                }
            }
        }
        if is_default {
            fallback = Some((member.name.clone(), member_type));
        }
        if !condition.is_empty() {
            arms.push((condition, member.name.clone(), member_type));
        }
    }

    let mut opened = 0usize;
    for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
        let head = if index == 0 {
            format!("if ({condition}) {{")
        } else {
            format!("}} else if ({condition}) {{")
        };
        if index == 0 {
            out.open(&head);
        } else {
            out.close(&head);
            out.depth += 1;
        }
        opened += 1;
        out.line(&format!("Zdj.Key({first}, \"{member_name}\");"));
        emit(
            ast,
            *member_type,
            &format!("{expr}.{}", field_name(member_name)),
            out,
            &format!("{path}.{member_name}"),
        )?;
    }
    if let Some((member_name, member_type)) = fallback {
        if opened == 0 {
            out.open("if (true) {");
        } else {
            out.close("} else {");
            out.depth += 1;
        }
        opened += 1;
        out.line(&format!("Zdj.Key({first}, \"{member_name}\");"));
        emit(
            ast,
            member_type,
            &format!("{expr}.{}", field_name(&member_name)),
            out,
            &format!("{path}.{member_name}"),
        )?;
    }
    if opened > 0 {
        out.close("}");
    }

    out.raw("}");
    out.close("}");
    Ok(())
}

/// An array, one dimension at a time.
///
/// zerodds-lint: recursion-depth = array dimensions (one level per
/// entry of `counts`, which the split shortens on every call; the
/// element then goes through [`emit`])
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
    counts: &[i128],
) -> Result<(), String> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, expr, out, path);
    };
    let index = out.temporary("a");
    out.open("{");
    out.raw("[");
    out.open(&format!(
        "for (int {index} = 0; {index} < {count}; {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ Zdj.Raw(\",\"); }}"));
    emit_array(
        ast,
        element,
        &format!("{expr}[{index}]"),
        out,
        &format!("{path}[]"),
        rest,
    )?;
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// A sequence: the length is the bytes' and not the type's, so the
/// element statements go inside a `for`.
fn emit_sequence(
    ast: &Ast,
    element: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let index = out.temporary("s");
    out.open("{");
    out.raw("[");
    out.open(&format!(
        "for (int {index} = 0; {index} < {expr}.Length; {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ Zdj.Raw(\",\"); }}"));
    emit(
        ast,
        element,
        &format!("{expr}[{index}]"),
        out,
        &format!("{path}[]"),
    )?;
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// A map, as the array of two-element arrays the dialect calls for.
fn emit_map(
    ast: &Ast,
    key_type: Id<TypeNode>,
    value_type: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let index = out.temporary("m");
    out.open("{");
    out.raw("[");
    out.open(&format!(
        "for (int {index} = 0; {index} < {expr}.Length; {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ Zdj.Raw(\",\"); }}"));
    out.raw("[");
    emit(
        ast,
        key_type,
        &format!("{expr}[{index}].Key"),
        out,
        &format!("{path}[].key"),
    )?;
    out.raw(",");
    emit(
        ast,
        value_type,
        &format!("{expr}[{index}].Value"),
        out,
        &format!("{path}[].value"),
    )?;
    out.raw("]");
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// An enumeration, printed as the enumerator's name.
///
/// An if-chain over the declared constants: two enumerators may carry
/// the same `@value`, which a `switch` cannot hold at all, and the first
/// match is what the Rust reader also returns. A value that names no
/// enumerator prints a string no enumerator has, so the comparison
/// refuses it by name — a bare number here would read as some other
/// member's value and pass.
fn emit_enumeration(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let owner = declared_path(ast, type_)
        .map_err(|error| format!("{path}: the enumeration has no C# path: {error:?}"))?;
    let mut first = true;
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        let constant = format!(
            "{owner}.{}",
            zerodds_lang_cst::csharp::escape_keyword(&node.name)
        );
        let head = format!("if ({expr} == {constant}) {{");
        if first {
            out.open(&head);
            first = false;
        } else {
            out.close(&format!("}} else {head}"));
            out.depth += 1;
        }
        out.line(&format!("Zdj.Raw(\"\\\"{}\\\"\");", node.name));
    }
    if first {
        out.line(&format!("Zdj.EnumUnknown((long)({expr}));"));
        return Ok(());
    }
    out.close("} else {");
    out.depth += 1;
    out.line(&format!("Zdj.EnumUnknown((long)({expr}));"));
    out.close("}");
    Ok(())
}

/// A bitmask, printed as the set bits' names in declaration order.
///
/// The flags are named rather than shifted: §7.5.1.2.2 declares them as
/// members with the position already applied, so the position-to-bit
/// mapping stays in the materialiser where the codec also reads it.
fn emit_bitmask(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let owner = declared_path(ast, type_)
        .map_err(|error| format!("{path}: the bitmask has no C# path: {error:?}"))?;
    let first = out.temporary("first");
    out.open("{");
    out.line(&format!("bool {first} = true;"));
    out.raw("[");
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        let constant = format!(
            "{owner}.{}",
            zerodds_lang_cst::csharp::escape_keyword(&node.name)
        );
        // A `[Flags]` enum `&`s to the enum type, and C# admits the
        // literal zero on either side of a comparison with one.
        out.open(&format!("if (({expr} & {constant}) != 0) {{"));
        out.line(&format!("if (!{first}) {{ Zdj.Raw(\",\"); }}"));
        out.line(&format!("{first} = false;"));
        out.line(&format!("Zdj.Raw(\"\\\"{}\\\"\");", node.name));
        out.close("}");
    }
    out.raw("]");
    out.close("}");
    Ok(())
}

/// The call that prints one primitive.
///
/// The widening is written out because every helper takes the widest
/// type of its signedness: `Zdj.U64(ulong)` and `Zdj.I64(long)`. C# has
/// no implicit conversion from a `uint` to a `long`'s signed
/// counterpart that would surprise anyone, but writing the cast keeps
/// one call per row rather than one overload per width.
fn primitive_call(kind: PrimitiveKind, expr: &str) -> String {
    match kind {
        PrimitiveKind::Boolean => format!("Zdj.Bool({expr});"),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::UInt16 => {
            format!("Zdj.U64((ulong)({expr}));")
        }
        PrimitiveKind::Int8 | PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("Zdj.I64((long)({expr}));")
        }
        PrimitiveKind::UInt32 => format!("Zdj.U64((ulong)({expr}));"),
        PrimitiveKind::Char => format!("Zdj.Ch({expr});"),
        PrimitiveKind::WChar => format!("Zdj.Wch({expr});"),
        // A JSON number is a double, so both 64-bit widths go out as
        // decimal in a string. The one thing here no reader can undo.
        PrimitiveKind::Int64 => format!("Zdj.I64String({expr});"),
        PrimitiveKind::UInt64 => format!("Zdj.U64String({expr});"),
        PrimitiveKind::Float32 => format!("Zdj.F32({expr});"),
        PrimitiveKind::Float64 => format!("Zdj.F64({expr});"),
        // A string holding every digit the value has, which is the
        // canonical form for a `long double`. Printing it as a `double`
        // would report agreement for a value this column never carried.
        PrimitiveKind::Float128 => {
            format!("Zdj.Raw(\"\\\"\" + {expr}.ToExactDecimal() + \"\\\"\");")
        }
    }
}

/// A union label as a C# literal in the discriminator's own type.
///
/// A `bool` discriminator has no conversion from the 0 and 1 the IDL
/// evaluator gives, and a `char` none from an integer; both are written
/// as the value they stand for. Everything else takes the suffix that
/// says which integral type the literal is, so the comparison is not
/// decided by the operand the compiler happened to widen.
fn csharp_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_primitive(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => {
            return if value == 0 { "false" } else { "true" }.to_string();
        }
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            let code = u32::try_from(value).unwrap_or(0);
            return format!("'\\u{code:04X}'");
        }
        _ => {}
    }
    // An enumerated discriminator is a real C# `enum`, which has no
    // implicit conversion from an integer — not even a constant, apart
    // from zero. The cast is what makes the comparison legal, and it is
    // the same one `emit/csharp.rs` writes on the codec's side.
    if let Some(path) = enumerated_path(ast, discriminator, 0) {
        return format!("({path}){value}");
    }
    if value > i128::from(i64::MAX) {
        return format!("{value}UL");
    }
    if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
        return format!("{value}L");
    }
    format!("{value}")
}

/// How far a chain of typedefs is followed before the model is taken
/// to be malformed.
///
/// A cycle cannot be written: a `typedef`'s target must already be
/// declared where it is written, and IDL has no forward declaration for
/// one. The number is here so that a model assembled by something other
/// than the front end stops the walk instead of the walk stopping the
/// process.
const MAX_ALIAS_DEPTH: usize = 32;

/// The rooted path of an enumerated type, once typedefs are seen
/// through, or `None` for anything else.
///
/// zerodds-lint: recursion-depth 32 (typedef chains only —
/// every other kind answers without a descent, so the walk never
/// reaches an aggregate and cannot cycle; bounded by [`MAX_ALIAS_DEPTH`])
fn enumerated_path(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<String> {
    if depth > MAX_ALIAS_DEPTH {
        return None;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. }) => declared_path(ast, id).ok(),
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .and_then(|target| enumerated_path(ast, target, depth + 1)),
        _ => None,
    }
}

/// The primitive kind a discriminator is, once typedefs are seen
/// through.
///
/// zerodds-lint: recursion-depth 32 (typedef chains only —
/// every other kind answers without a descent, so the walk never
/// reaches an aggregate and cannot cycle; bounded by [`MAX_ALIAS_DEPTH`])
fn resolve_primitive(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<PrimitiveKind> {
    if depth > MAX_ALIAS_DEPTH {
        return None;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => Some(kind),
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .and_then(|target| resolve_primitive(ast, target, depth + 1)),
        _ => None,
    }
}
