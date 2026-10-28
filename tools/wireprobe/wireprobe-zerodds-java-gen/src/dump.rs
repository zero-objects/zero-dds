// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The statements that print one filled Java sample as canonical JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into the
//! generated Java. This takes one back out, after `runtime/java` and the
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
//! `tools/wireprobe/Zdj.java` holds the helpers, and it is the same file
//! the JacORB probe prints through. That is deliberate: the JSON dialect
//! is a shared contract and not a wire rule, and two backends holding two
//! copies of it is how they come to disagree about what a sample is.
//!
//! ## Why it is built at run time and not baked in
//!
//! A sequence's length, a union's selected member and an `@optional`
//! member's presence are all decided by the bytes, not by the type. So
//! the punctuation is statements — `Zdj.key` puts the comma in only when
//! something has already been printed at that level — rather than one
//! `print` per aggregate.

use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::java::declared_path;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::java::Java;

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
    /// own — `zd_rec_dump_<n>` — instead of an inline expansion, which
    /// for a cycle has no end. The same shape the C and C++ emitters
    /// carry (`zerodds_wire_probe::recursion`).
    rec: Vec<Id<TypeNode>>,
    /// A counter, so two loops in one sample get distinct indices.
    next: usize,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    ///
    /// It has to be here at all because the **selection** is the
    /// codec's. A generated decoder that falls through to the default
    /// case and a dump that prints the negative one would print a member
    /// the decoder never wrote.
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
        self.line(&format!("Zdj.raw(\"{escaped}\");"));
    }

    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
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
    // No forward declaration: a Java method sees every other member of
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
            out.open(&format!(
                "static void zd_rec_dump_{position}({name} zdv) {{"
            ));
            out.active.push(id);
            let outcome = emit_inner(ast, id, "zdv", &mut out, &format!("<{name}>"));
            out.active.pop();
            outcome?;
            out.close("}");
        }
        std::mem::take(&mut out.body)
    };
    emit(ast, type_, "zd_sample", &mut out, "sample")?;
    out.line("Zdj.raw(\"\\n\");");
    Ok(DumpParts {
        functions,
        body: out.body,
    })
}

/// The two halves of the printer: methods for the types on a cycle,
/// and the statements for the dump method's body.
#[derive(Debug, Default)]
pub struct DumpParts {
    /// `static void zd_rec_dump_<n>(T zdv)` definitions, for the class
    /// holding [`Self::body`].
    pub functions: String,
    /// The statements that print `zd_sample` as canonical JSON.
    pub body: String,
}

/// The Java spelling of one type, as the harness spells the sample's.
fn spelled(ast: &Ast, id: Id<TypeNode>) -> String {
    zerodds_lang_render::java::type_(&<Java as CodecTarget>::value_type(ast, id))
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
        out.line(&format!("zd_rec_dump_{position}({expr});"));
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
                CharWidth::Char => format!("Zdj.string({expr});"),
                CharWidth::WChar => format!("Zdj.wstring({expr});"),
            });
            Ok(())
        }
        // A `fixed<P,S>` is a `BigDecimal`, printed as the decimal text it holds: the canonical
        // form is a string for the reason the type exists, a double cannot carry one.
        TypeKind::Fixed { .. } => {
            out.line(&format!("Zdj.fixed(({expr}).toPlainString());"));
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the typedef target is unresolved"))?;
            emit(ast, target, expr, out, path)
        }
        TypeKind::Enumeration { .. } => {
            emit_enumeration(ast, type_, expr, out, path)?;
            Ok(())
        }
        TypeKind::Bitmask { .. } => emit_bitmask(ast, type_, expr, out, path),
        TypeKind::Bitset { .. } => {
            // The whole list, base first: a derived `bitset` carries the
            // base's bitfields and the decoder fills them, so printing
            // only the derived type's own would return a value with
            // members missing and say nothing about it.
            let first = out.temporary("first");
            out.open("{");
            out.line(&format!("boolean[] {first} = {{ true }};"));
            out.raw("{");
            for field in &ast.all_bitfields_of(type_) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding and carries no value.
                let Some(name) = field_node.name.as_ref() else {
                    continue;
                };
                let field_name = zerodds_lang_cst::java::escape_keyword(name);
                out.line(&format!("Zdj.key({first}, \"{name}\");"));
                out.line(&format!("Zdj.u64({expr}.{field_name});"));
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
            // A map with a class of its own is read through its entries.
            let entries = if zerodds_lang_rules::adapter::java::map_key_supported(ast, key_type) {
                let name = out.temporary("entries");
                // spelled out: the probe compiles for Java 8, where `var` warns
                let entry = zerodds_lang_rules::adapter::java::map_entry_name(ast, type_)
                    .map_err(|error| format!("{path}: the map has no entry class: {error:?}"))?;
                out.line(&format!(
                    "final {}.{entry}[] {name} = {expr}.entries();",
                    zerodds_lang_rules::adapter::java::OUTER
                ));
                name
            } else {
                expr.to_string()
            };
            emit_map(ast, key_type, value_type, &entries, out, path)
        }
        other => Err(format!(
            "{path}: {} has no Java shape to print from",
            other.as_str()
        )),
    }
}

/// A structure or a valuetype, as a JSON object in wire order.
///
/// `all_members_of`, so a base's members come out under their own names
/// and in front of the derived type's — §7.4.1.1.5.1, and the same walk
/// the fill side makes.
fn emit_aggregate(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let first = out.temporary("first");
    out.open("{");
    out.line(&format!("boolean[] {first} = {{ true }};"));
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
        let field = zerodds_lang_cst::java::escape_keyword(&name);
        let member_path = format!("{path}.{name}");
        if member_node.facets.is_optional() {
            // An absent member is a key that is not printed. `null`
            // would read as "present and empty" to one backend and as
            // "absent" to another, and the canonical form refuses it.
            out.open(&format!("if ({expr}.{field} != null) {{"));
            out.line(&format!("Zdj.key({first}, \"{name}\");"));
            emit(
                ast,
                member_type,
                &format!("{expr}.{field}"),
                out,
                &member_path,
            )?;
            out.close("}");
            continue;
        }
        out.line(&format!("Zdj.key({first}, \"{name}\");"));
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
///
/// An if-chain over the labels and not a `switch`, for the reason the
/// codec emitter gives: a `case 16045690984833335998:` over an
/// `unsigned long long` does not fit a signed label, and the comparison
/// has to be made in the discriminator's own type.
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
    out.open("{");
    out.line(&format!("boolean[] {first} = {{ true }};"));
    out.raw("{");
    out.line(&format!("Zdj.key({first}, \"_d\");"));
    emit(
        ast,
        discriminator,
        &format!("{expr}._d"),
        out,
        &format!("{path}._d"),
    )?;

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
                        "{expr}._d == {}",
                        java_label(ast, discriminator, value)
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
        let field = zerodds_lang_cst::java::escape_keyword(member_name);
        out.line(&format!("Zdj.key({first}, \"{member_name}\");"));
        emit(
            ast,
            *member_type,
            &format!("{expr}.{field}"),
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
        let field = zerodds_lang_cst::java::escape_keyword(&member_name);
        out.line(&format!("Zdj.key({first}, \"{member_name}\");"));
        emit(
            ast,
            member_type,
            &format!("{expr}.{field}"),
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
    out.line(&format!("if ({index} > 0) {{ Zdj.raw(\",\"); }}"));
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
        "for (int {index} = 0; {index} < {expr}.length; {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ Zdj.raw(\",\"); }}"));
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
///
/// A JSON object keys on a string and `map<long, float>` does not, so
/// the pair form is what §7.4.1.1.4's scalar key restriction allows.
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
        "for (int {index} = 0; {index} < {expr}.length; {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ Zdj.raw(\",\"); }}"));
    out.raw("[");
    emit(
        ast,
        key_type,
        &format!("{expr}[{index}].key"),
        out,
        &format!("{path}[].key"),
    )?;
    out.raw(",");
    emit(
        ast,
        value_type,
        &format!("{expr}[{index}].value"),
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
        .map_err(|error| format!("{path}: the enumeration has no Java path: {error:?}"))?;
    let mut first = true;
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        // The object `<label>` of the type-safe class (XTypes 1.3
        // 7.5.1.2.1.3); `_<label>` is its int constant.
        let constant = format!(
            "{owner}.{}",
            zerodds_lang_cst::java::escape_keyword(&node.name)
        );
        let head = format!("if ({expr} == {constant}) {{");
        if first {
            out.open(&head);
            first = false;
        } else {
            out.close(&format!("}} else {head}"));
            out.depth += 1;
        }
        out.line(&format!("Zdj.raw(\"\\\"{}\\\"\");", node.name));
    }
    if first {
        out.line(&format!("Zdj.enumUnknown((long) ({expr}.value()));"));
        return Ok(());
    }
    out.close("} else {");
    out.depth += 1;
    out.line(&format!("Zdj.enumUnknown((long) ({expr}.value()));"));
    out.close("}");
    Ok(())
}

/// A bitmask, printed as the set bits' names in declaration order.
///
/// The flags are named rather than shifted: §7.5.1.2.2 declares them as
/// constants with the position already applied, so the
/// position-to-bit mapping stays in the materialiser where the codec
/// also reads it.
fn emit_bitmask(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let owner = declared_path(ast, type_)
        .map_err(|error| format!("{path}: the bitmask has no Java path: {error:?}"))?;
    let first = out.temporary("first");
    out.open("{");
    out.line(&format!("boolean {first} = true;"));
    out.raw("[");
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        let constant = format!(
            "{owner}.{}",
            zerodds_lang_cst::java::escape_keyword(&node.name)
        );
        // No cast: `adapter/java.rs` declares the constants in the
        // holder the member itself takes, and a cast to the type a
        // value already has is what `-Xlint:cast -Werror` fails over.
        out.open(&format!("if (({expr} & {constant}) != 0) {{"));
        out.line(&format!("if (!{first}) {{ Zdj.raw(\",\"); }}"));
        out.line(&format!("{first} = false;"));
        out.line(&format!("Zdj.raw(\"\\\"{}\\\"\");", node.name));
        out.close("}");
    }
    out.raw("]");
    out.close("}");
    Ok(())
}

/// The call that prints one primitive.
///
/// The widening is the caller's, because Java has no unsigned type: an
/// `octet` is already an `int` holding 0..255 and an `unsigned long` is
/// already a `long` holding 0..4294967295, so `Zdj.u64` gets a number
/// and not a pattern. Only `unsigned long long` is a pattern, and
/// `u64String` prints it as one.
fn primitive_call(kind: PrimitiveKind, expr: &str) -> String {
    match kind {
        PrimitiveKind::Boolean => format!("Zdj.bool({expr});"),
        PrimitiveKind::UInt8 | PrimitiveKind::UInt16 => format!("Zdj.u64((long) ({expr}));"),
        // A `byte`, so the number is its low eight bits.
        PrimitiveKind::Octet => format!("Zdj.u64((long) (({expr}) & 0xFF));"),
        PrimitiveKind::Int8 | PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("Zdj.i64((long) ({expr}));")
        }
        PrimitiveKind::UInt32 => format!("Zdj.u64({expr});"),
        PrimitiveKind::Char => format!("Zdj.ch({expr});"),
        PrimitiveKind::WChar => format!("Zdj.wch({expr});"),
        // A JSON number is a double, so both 64-bit widths go out as
        // decimal in a string. The one thing here no reader can undo.
        PrimitiveKind::Int64 => format!("Zdj.i64String({expr});"),
        PrimitiveKind::UInt64 => format!("Zdj.u64String({expr});"),
        PrimitiveKind::Float32 => format!("Zdj.f32({expr});"),
        PrimitiveKind::Float64 => format!("Zdj.f64({expr});"),
        // A string holding every digit the value has, which is the
        // canonical form for a `long double`. Printing it as a `double`
        // would report agreement for a value this column never carried.
        PrimitiveKind::Float128 => {
            format!(
                "Zdj.raw({expr} == null ? \"null\" : \"\\\"\" + {expr}.toExactDecimal() + \"\\\"\");"
            )
        }
    }
}

/// A union label as a Java literal wide enough to hold it, in the
/// discriminator's own type.
///
/// The same rule `emit/java.rs` writes by: a bare decimal past
/// `Integer.MAX_VALUE` is not an `int`, and a `boolean` discriminator has
/// no conversion from the 0 and 1 the IDL evaluator gives.
fn java_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    if matches!(
        resolve_primitive(ast, discriminator, 0),
        Some(PrimitiveKind::Boolean)
    ) {
        return if value == 0 { "false" } else { "true" }.to_string();
    }
    if matches!(
        resolve_primitive(ast, discriminator, 0),
        Some(PrimitiveKind::Char | PrimitiveKind::WChar)
    ) {
        return format!("(char) {value}");
    }
    if value > i128::from(i64::MAX) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let pattern = value as u64 as i64;
        return format!("{pattern}L");
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
