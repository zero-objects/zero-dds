// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The statements that print one decoded TypeScript sample as canonical
//! JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into the
//! generated TypeScript. This takes one back out, after
//! `runtime/typescript` and the generated decoder have filled it from
//! the bytes. The port of `wireprobe-zerodds-csharp-gen/src/dump.rs`,
//! statement for statement.
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
//! `tools/wireprobe/zdj.ts` holds the helpers, the TypeScript
//! counterpart of `tools/wireprobe/Zdj.cs`, of `tools/wireprobe/Zdj.java`
//! and of `tools/wireprobe/dumpjson.h`.
//!
//! ## Why it is built at run time and not baked in
//!
//! A sequence's length, a union's selected member and an `@optional`
//! member's presence are all decided by the bytes, not by the type. So
//! the punctuation is statements — `Zdj.key` puts the comma in only when
//! something has already been printed at that level — rather than one
//! `print` per aggregate.
//!
//! ## What TypeScript changes over the C# dumper
//!
//! Where C#'s `@optional` member of a value type is a `Nullable<T>` the
//! printer has to unwrap, this column's is `field?: T` under
//! `exactOptionalPropertyTypes`: absence is `undefined` and presence is
//! the value itself, so the printer binds the property to a `const` and
//! tests it against `undefined` — the binding is what `tsc` narrows,
//! where a repeated property access would depend on control-flow
//! analysis holding across the helper calls in between.
//!
//! Two spellings follow `crate::fill` rather than the C# dumper, and
//! for the reason its module doc argues at length: the materialiser
//! computes a declaration's qualified path privately, so an enumerator
//! is compared by its **name** — the string DDS-TS 1.0 §7.6 makes the
//! surface hold — and a bitmask bit by its **pattern** out of
//! `ast.effective_bit_position`. Neither is a second implementation of
//! a wire rule; a wrong answer in the model shows up as a wrong octet,
//! not as agreement between two copies of the same mistake.
//!
//! Collections are walked with `for … of` and a separate counter rather
//! than by index, because the tree compiles under
//! `noUncheckedIndexedAccess` and `v[i]` is `T | undefined` however the
//! loop is bounded.

use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::typescript::DISCRIMINANT_PROPERTY;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::typescript::TypeScript;

use crate::fill::{is_plain_identifier, ts_string};

/// The statements, ready to paste into the probe's tail.
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
    /// The aggregates on a cycle, each of which gets a function of its
    /// own — `zdRecDump<n>` — instead of an inline expansion, which for
    /// a cycle has no end. The same shape the C and C++ emitters carry
    /// (`zerodds_wire_probe::recursion`).
    rec: Vec<Id<TypeNode>>,
    /// A counter, so two loops in one sample get distinct names.
    next: usize,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    union_label: UnionLabelMatch,
}

impl Dump {
    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.body.push_str("  ");
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

/// Emit the statements that print `zdSample` as canonical JSON.
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
    // No forward declaration: a `function` declaration is hoisted over
    // the whole module, so a mutual cycle needs nothing that the C++
    // column's file-scope prototypes needed.
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
                "function zdRecDump{position}(zdv: {name}): void {{"
            ));
            out.active.push(id);
            let outcome = emit_inner(ast, id, "zdv", &mut out, &format!("<{name}>"));
            out.active.pop();
            outcome?;
            out.close("}");
        }
        std::mem::take(&mut out.body)
    };
    emit(ast, type_, "zdSample", &mut out, "sample")?;
    out.line("Zdj.raw(\"\\n\");");
    Ok(DumpParts {
        functions,
        body: out.body,
    })
}

/// The two halves of the printer: functions for the types on a cycle,
/// and the statements that print one sample.
#[derive(Debug, Default)]
pub struct DumpParts {
    /// `function zdRecDump<n>(zdv: T): void` declarations, for the
    /// module holding [`Self::body`].
    pub functions: String,
    /// The statements that print `zdSample` as canonical JSON.
    pub body: String,
}

/// The TypeScript spelling of one type, as the harness spells the
/// sample's.
fn spelled(ast: &Ast, id: Id<TypeNode>) -> String {
    zerodds_lang_render::typescript::type_(&<TypeScript as CodecTarget>::value_type(ast, id))
}

/// One property access, spelled the way the declaration spells the
/// member.
///
/// The materialiser escapes a member whose IDL name is a TypeScript
/// reserved word, and an access that wrote the IDL name would name a
/// property the interface does not declare. Bracketed where even the
/// escape is not an identifier, which is what `PropertyName` does on
/// the declaration side.
fn access(expr: &str, name: &str) -> String {
    let escaped = <TypeScript as CodecTarget>::escape(name);
    if is_plain_identifier(&escaped) {
        format!("{expr}.{escaped}")
    } else {
        format!("{expr}[{}]", ts_string(&escaped))
    }
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

    // A type on a cycle is a call into its own function, never an
    // inline expansion — the expansion of a cycle has no end.
    if let Some(position) = out.rec.iter().position(|id| *id == type_) {
        out.line(&format!("zdRecDump{position}({expr});"));
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
        // A `fixed<P,S>` is the decimal it was written as — this column
        // holds it as a `string`, for the reason `adapter/typescript.rs`
        // gives — and the canonical form is a string for the reason the
        // type exists: a double cannot carry one.
        TypeKind::Fixed { .. } => {
            out.line(&format!("Zdj.fixed({expr});"));
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
            out.line(&format!("const {first} = {{ v: true }};"));
            out.raw("{");
            for field in &ast.all_bitfields_of(type_) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding and carries no value.
                let Some(name) = field_node.name.as_ref() else {
                    continue;
                };
                out.line(&format!("Zdj.key({first}, \"{name}\");"));
                // The holder is a `number` to 32 bits and a `bigint`
                // past them; `Zdj.u64` takes both.
                out.line(&format!("Zdj.u64({});", access(expr, name)));
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
            emit_array(ast, element, expr, out, path, dims.len())
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
            "{path}: {} has no TypeScript shape to print from",
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
    out.line(&format!("const {first} = {{ v: true }};"));
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
        let member_path = format!("{path}.{name}");
        // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
        let spelled = zerodds_lang_rules::emit::member_field::<TypeScript>(
            ast,
            type_,
            member_node.node_id,
            &name,
        );
        let depth = spelled.matches('.').count();
        let mut holder = expr.to_string();
        for _ in 0..depth {
            holder = access(&holder, "parent");
        }
        let reached = access(&holder, &name);
        if member_node.facets.is_optional() {
            // An absent member is a key that is not printed. `null`
            // would read as "present and empty" to one backend and as
            // "absent" to another, and the canonical form refuses it.
            //
            // The property is bound once: the `const` is what `tsc`
            // narrows past the `undefined` test, and the module doc says
            // why an access repeated after the helper calls would not
            // be.
            let held = out.temporary("o");
            out.open("{");
            out.line(&format!("const {held} = {reached};"));
            out.open(&format!("if ({held} !== undefined) {{"));
            out.line(&format!("Zdj.key({first}, \"{name}\");"));
            emit(ast, member_type, &held, out, &member_path)?;
            out.close("}");
            out.close("}");
            continue;
        }
        out.line(&format!("Zdj.key({first}, \"{name}\");"));
        emit(ast, member_type, &reached, out, &member_path)?;
    }
    out.raw("}");
    out.close("}");
    Ok(())
}

/// One case member, reached through the arm that has it.
///
/// `(u as Extract<typeof u, { field: unknown }>).field`, which is the
/// expression `emit/typescript.rs`'s `case_member` writes and for the
/// same reason: the declared type is a *discriminated union* and a case
/// member lives on one arm only, while a `default:` arm carries the
/// discriminator's whole type — so narrowing on `_d` leaves both arms
/// live and `u.field` is TS2339. `base` is always an identifier here,
/// which is what `typeof` takes.
fn union_member(base: &str, name: &str) -> String {
    let escaped = <TypeScript as CodecTarget>::escape(name);
    if is_plain_identifier(&escaped) {
        format!("({base} as Extract<typeof {base}, {{ {escaped}: unknown }}>).{escaped}")
    } else {
        let quoted = ts_string(&escaped);
        format!("({base} as Extract<typeof {base}, {{ {quoted}: unknown }}>)[{quoted}]")
    }
}

/// A union, as `{"_d": …}` plus the member the discriminator selects.
///
/// `_d` is the JSON key and `discriminator` the TypeScript property; see
/// [`crate::fill`] for why the two differ.
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
    // The union value under one name: `typeof` in the arm cast takes an
    // identifier, and `expr` may be an access chain by the time a union
    // sits inside a sequence.
    let held = out.temporary("u");
    // The property DDS-TS 1.0 §7.5 names; the JSON key below stays
    // `_d`, which is the interop suite's canonical dialect and not this
    // mapping's.
    let selector = format!("{held}.{DISCRIMINANT_PROPERTY}");
    out.open("{");
    out.line(&format!("const {held} = {expr};"));
    out.line(&format!("const {first} = {{ v: true }};"));
    out.raw("{");
    out.line(&format!("Zdj.key({first}, \"_d\");"));
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
                        "{selector} === {}",
                        ts_label(ast, discriminator, value)
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
        out.line(&format!("Zdj.key({first}, \"{member_name}\");"));
        emit(
            ast,
            *member_type,
            &union_member(&held, member_name),
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
        out.line(&format!("Zdj.key({first}, \"{member_name}\");"));
        emit(
            ast,
            member_type,
            &union_member(&held, &member_name),
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
/// The value is walked and not the declared count: the decoder filled
/// exactly the declared number of elements, and a decoder that did not
/// prints a shorter array the comparison then refuses — where a loop
/// over the count would have indexed past what is there, which
/// `noUncheckedIndexedAccess` spells `T | undefined`.
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
    dims: usize,
) -> Result<(), String> {
    if dims == 0 {
        return emit(ast, element, expr, out, path);
    }
    let index = out.temporary("i");
    let item = out.temporary("a");
    out.open("{");
    out.raw("[");
    out.line(&format!("let {index} = 0;"));
    out.open(&format!("for (const {item} of {expr}) {{"));
    out.line(&format!("if ({index} > 0) {{ Zdj.raw(\",\"); }}"));
    out.line(&format!("{index} += 1;"));
    emit_array(ast, element, &item, out, &format!("{path}[]"), dims - 1)?;
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// A sequence: the length is the bytes' and not the type's, so the
/// element statements go inside a loop.
fn emit_sequence(
    ast: &Ast,
    element: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let index = out.temporary("i");
    let item = out.temporary("s");
    out.open("{");
    out.raw("[");
    out.line(&format!("let {index} = 0;"));
    out.open(&format!("for (const {item} of {expr}) {{"));
    out.line(&format!("if ({index} > 0) {{ Zdj.raw(\",\"); }}"));
    out.line(&format!("{index} += 1;"));
    emit(ast, element, &item, out, &format!("{path}[]"))?;
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
    let index = out.temporary("i");
    let entry = out.temporary("m");
    out.open("{");
    out.raw("[");
    out.line(&format!("let {index} = 0;"));
    out.open(&format!("for (const {entry} of {expr}) {{"));
    out.line(&format!("if ({index} > 0) {{ Zdj.raw(\",\"); }}"));
    out.line(&format!("{index} += 1;"));
    out.raw("[");
    // DDS-TS 1.0 §7.9.1 makes a map a `ReadonlyMap`, so iterating it
    // yields `[key, value]` pairs and not the entry objects the codec
    // stages the wire form through.
    emit(
        ast,
        key_type,
        &format!("{entry}[0]"),
        out,
        &format!("{path}[].key"),
    )?;
    out.raw(",");
    emit(
        ast,
        value_type,
        &format!("{entry}[1]"),
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
///
/// The comparison is against the **value** and not a named constant,
/// for the reason [`crate::fill`] writes an enumerator as its value:
/// `ast.effective_enumerator_value` is the same function the
/// materialiser writes the constant from.
fn emit_enumeration(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    // The value under a name of its own, and not the access itself in
    // every arm. §7.6 makes an enumeration a string-literal union, so
    // the chain **narrows** what it tests: after the last enumerator the
    // else branch has `never`, and a property access written there is
    // TS2339 — "Property 'discriminator' does not exist on type
    // 'never'", measured on `union-enum-crossmodule`,
    // `union-discriminator-enum-bitbound` and its explicit-
    // extensibility sibling. A binding narrows to `never` too, and
    // `never` is an argument every parameter takes.
    let held = out.temporary("e");
    out.open("{");
    out.line(&format!("const {held} = {expr};"));
    let mut first = true;
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        let value = ast
            .effective_enumerator_value(*id)
            .ok_or_else(|| format!("{path}: `{}` has no value the model can state", node.name))?;
        // The enumerator's name, which is what DDS-TS 1.0 §7.6 makes
        // the surface hold; `value` is asked for so that a model that
        // will not state one is still a refusal here.
        let _ = value;
        let head = format!(
            "if ({held} === {}) {{",
            ts_string(&<TypeScript as CodecTarget>::escape(&node.name))
        );
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
        out.line(&format!("Zdj.enumUnknown({held});"));
        out.close("}");
        return Ok(());
    }
    out.close("} else {");
    out.depth += 1;
    out.line(&format!("Zdj.enumUnknown({held});"));
    out.close("}");
    out.close("}");
    Ok(())
}

/// A bitmask, printed as the set bits' names in declaration order.
///
/// The pattern is computed from `ast.effective_bit_position` — the same
/// function the materialiser emits the named constants from, and the
/// same answer [`crate::fill`] ors the sample together with; its module
/// doc carries the argument for why that is not a second implementation
/// of the bit layout.
fn emit_bitmask(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    // Wider than 32 bits and the holder is a `bigint`, which mixes with
    // a `number` in no operator: the mask and the zero take the suffix.
    let wide = ast.effective_bit_bound(type_) > 32;
    let first = out.temporary("first");
    out.open("{");
    out.line(&format!("let {first} = true;"));
    out.raw("[");
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        let position = ast.effective_bit_position(*id).ok_or_else(|| {
            format!(
                "{path}: `{}` has no position the model can state",
                node.name
            )
        })?;
        if position >= 64 {
            return Err(format!(
                "{path}: `{}` sits at bit {position}, past 64",
                node.name
            ));
        }
        let test = if wide {
            format!("({expr} & {}n) !== 0n", 1u128 << position)
        } else {
            format!("({expr} & {}) !== 0", 1u128 << position)
        };
        out.open(&format!("if ({test}) {{"));
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
fn primitive_call(kind: PrimitiveKind, expr: &str) -> String {
    match kind {
        PrimitiveKind::Boolean => format!("Zdj.bool({expr});"),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::UInt16
        | PrimitiveKind::UInt32 => format!("Zdj.u64({expr});"),
        PrimitiveKind::Int8 | PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("Zdj.i64({expr});")
        }
        PrimitiveKind::Char => format!("Zdj.ch({expr});"),
        PrimitiveKind::WChar => format!("Zdj.wch({expr});"),
        // A JSON number is a double, so both 64-bit widths go out as
        // decimal in a string. The one thing here no reader can undo.
        // The holder is a `bigint` and the helper's signature says so.
        PrimitiveKind::Int64 => format!("Zdj.i64String({expr});"),
        PrimitiveKind::UInt64 => format!("Zdj.u64String({expr});"),
        PrimitiveKind::Float32 => format!("Zdj.f32({expr});"),
        PrimitiveKind::Float64 => format!("Zdj.f64({expr});"),
        // A string holding every digit the value has, which is the
        // canonical form for a `long double`. Printing it as a `number`
        // would report agreement for a value this column never carried.
        PrimitiveKind::Float128 => format!("Zdj.f128({expr}.octets);"),
    }
}

/// A union label as a TypeScript literal in the discriminator's own
/// holder.
///
/// A `boolean` discriminator is compared as `true` or `false` and a
/// `char` as the one-character string the mapping holds it in. A 64-bit
/// discriminator's holder is a `bigint`, which mixes with a `number` in
/// no comparison, so the literal takes the `n`. Everything else —
/// including an enumeration, whose declared type is `type E = number` —
/// is the value itself.
fn ts_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    match resolve_primitive(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => {
            return if value == 0 { "false" } else { "true" }.to_string();
        }
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            let code = u32::try_from(value).unwrap_or(0);
            return format!("\"\\u{code:04X}\"");
        }
        Some(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            return format!("{value}n");
        }
        _ => {}
    }
    // An enumeration discriminator holds the enumerator's name, not its
    // ordinal (DDS-TS 1.0 §7.6): `u.discriminator === 1` against a
    // string-literal union is TS2367, "the types have no overlap".
    if let Some(name) = enumerator_named_by(ast, discriminator, value) {
        return ts_string(&<TypeScript as CodecTarget>::escape(&name));
    }
    format!("{value}")
}

/// The first enumerator of `discriminator` whose effective value is
/// `value`, once typedefs are seen through.
fn enumerator_named_by(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Option<String> {
    let mut current = discriminator;
    for _ in 0..32 {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Enumeration { .. }) => break,
            Ok(TypeKind::Alias { aliased }) => current = aliased.resolved()?,
            _ => return None,
        }
    }
    for enumerator in ast.enumerators_of(current) {
        if ast.effective_enumerator_value(*enumerator) == Some(value) {
            return ast
                .enumerator(*enumerator)
                .ok()
                .map(|node| node.name.clone());
        }
    }
    None
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
