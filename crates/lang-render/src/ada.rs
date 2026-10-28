// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Ada CST. Spec §5.
//!
//! One style: three spaces per level, `--` comments, one declaration per
//! line, the reserved words in lower case and the identifiers in
//! `Mixed_Case_With_Underscores`. Three spaces because that is what the
//! Ada Reference Manual's own examples use and what GNAT's `-gnaty`
//! style checks expect; nothing in the language depends on it.
//!
//! Nothing is decided here. A record either has components or is written
//! `null record`, and which of the two it is was decided by
//! [`zerodds_lang_cst::ada::Record::is_null`] — the renderer only asks.
//!
//! Two differences from every other renderer in this crate, and both are
//! Ada's:
//!
//! 1. **A string literal doubles its quote.** Ada 2012 RM 2.6 spells an
//!    embedded quotation mark `""`, where the C family writes `\"`, so
//!    [`crate::writer::escape_string`] is not used here at all — it
//!    would produce a backslash Ada reads as a graphic character of its
//!    own.
//! 2. **A comment marker is `--` and has no block form.** RM 2.7: a
//!    comment runs to the end of the line and there is no closing
//!    delimiter, so a comment can never swallow the declaration after
//!    it.

use crate::writer::{Writer, comment_lines, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::ada::{
    AllocInit, Arm, Choice, Component, Decl, Discriminant, EnumRep, Expr, File, Index, Mode,
    ObjectDecl, Param, Stmt, Subprogram, SubprogramBody, SubprogramKind, SubtypeDecl, TypeDecl,
    TypeDef, VarDecl, Variant,
};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// The indentation one level costs.
const UNIT: &str = "   ";

/// Render a whole package specification.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::new(UNIT);
    doc(&mut w, &file.doc);
    if !file.withs.is_empty() || !file.use_types.is_empty() {
        if !file.doc.is_empty() {
            w.blank_line();
        }
        for unit in &file.withs {
            let mut line = String::from("with ");
            line.push_str(&path_text(unit));
            line.push(';');
            w.line(&line);
        }
        // After the `with` clauses, because a `use_type_clause` names a
        // subtype the unit has to be able to see (RM 8.4), and it is the
        // `with` that makes it visible. Both are context items
        // (RM 10.1.2), so both stand above the package.
        for mark in &file.use_types {
            let mut line = String::from("use type ");
            line.push_str(&path_text(mark));
            line.push(';');
            w.line(&line);
        }
    }
    w.blank_line();
    let mut head = String::from("package ");
    head.push_str(file.package.as_str());
    head.push_str(" is");
    w.line(&head);
    w.indent();
    for decl in &file.decls {
        w.blank_line();
        render_decl(&mut w, decl);
    }
    w.dedent();
    w.blank_line();
    let mut tail = String::from("end ");
    tail.push_str(file.package.as_str());
    tail.push(';');
    w.line(&tail);
    w.finish()
}

/// Render a package body: `package body Name is … end Name;`.
///
/// The second half of what a generated Ada unit is. A subprogram body
/// is not a declaration a package *specification* may carry (RM 7.1
/// admits `basic_declarative_item`s, and a `body` is not one), so the
/// codec's bodies stand here and the profiles that name them stand in
/// the spec. The `with` clauses are the spec's: RM 10.1.2 extends a
/// context clause of a library unit declaration over the corresponding
/// body, so a body that repeated them would say the same thing twice.
#[must_use]
pub fn package_body(package: &Ident, decls: &[Decl]) -> String {
    let mut w = Writer::new(UNIT);
    let mut head = String::from("package body ");
    head.push_str(package.as_str());
    head.push_str(" is");
    w.line(&head);
    w.indent();
    for decl in decls {
        w.blank_line();
        render_decl(&mut w, decl);
    }
    w.dedent();
    w.blank_line();
    let mut tail = String::from("end ");
    tail.push_str(package.as_str());
    tail.push(';');
    w.line(&tail);
    w.finish()
}

/// Render one declaration, for a caller that has only one.
#[must_use]
pub fn decl(node: &Decl) -> String {
    let mut w = Writer::new(UNIT);
    render_decl(&mut w, node);
    w.finish()
}

/// Render one expression, for a caller assembling one.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

/// A qualified name, joined with the selector Ada uses.
#[must_use]
pub fn path(name: &Path) -> String {
    path_text(name)
}

fn path_text(name: &Path) -> String {
    name.segments()
        .iter()
        .map(|segment| segment.as_str())
        .collect::<Vec<&str>>()
        .join(".")
}

/// Render one statement, for a test that holds only one.
#[must_use]
pub fn stmt(node: &Stmt) -> String {
    let mut w = Writer::new(UNIT);
    render_stmts(&mut w, core::slice::from_ref(node));
    w.finish()
}

/// Render a statement list, `declare` blocks folded in.
#[must_use]
pub fn stmts(nodes: &[Stmt]) -> String {
    let mut w = Writer::new(UNIT);
    render_stmts(&mut w, nodes);
    w.finish()
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Type(node) => render_type_decl(w, node),
        Decl::Subtype(node) => render_subtype(w, node),
        Decl::Object(node) => render_object(w, node),
        Decl::EnumRep(node) => render_enum_rep(w, node),
        Decl::Subprogram(node) => render_subprogram(w, node),
        Decl::SubprogramBody(node) => render_subprogram_body(w, node),
        Decl::Comment(lines) => doc(w, lines),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_type_decl(w: &mut Writer, node: &TypeDecl) {
    doc(w, &node.doc);
    let mut head = String::from("type ");
    head.push_str(node.name.as_str());
    match &node.def {
        TypeDef::Record(record) => {
            head.push_str(&discriminant_text(&record.discriminants));
            if record.is_null() {
                head.push_str(" is null record;");
                w.line(&head);
                return;
            }
            head.push_str(" is record");
            w.line(&head);
            w.indent();
            for text in &record.body_text.begin {
                verbatim_lines(w, text);
            }
            for component in &record.components {
                render_component(w, component);
            }
            if let Some(variant) = &record.variant {
                render_variant(w, variant);
            }
            for text in &record.body_text.end {
                verbatim_lines(w, text);
            }
            w.dedent();
            w.line("end record;");
        }
        TypeDef::Enumeration(literals) => {
            head.push_str(" is (");
            head.push_str(
                &literals
                    .iter()
                    .map(|literal| literal.as_str())
                    .collect::<Vec<&str>>()
                    .join(", "),
            );
            head.push_str(");");
            w.line(&head);
        }
        TypeDef::Derived(mark) => {
            head.push_str(" is new ");
            head.push_str(&path_text(mark));
            head.push(';');
            w.line(&head);
        }
        TypeDef::Array { index, element } => {
            head.push_str(" is array (");
            head.push_str(
                &index
                    .iter()
                    .map(index_text)
                    .collect::<Vec<String>>()
                    .join(", "),
            );
            head.push_str(") of ");
            head.push_str(&path_text(element));
            head.push(';');
            w.line(&head);
        }
        TypeDef::Access(designated) => {
            head.push_str(" is access ");
            head.push_str(&path_text(designated));
            head.push(';');
            w.line(&head);
        }
        TypeDef::Interface => {
            head.push_str(" is interface;");
            w.line(&head);
        }
        // RM 3.10.1: the name and nothing else. The completion is a
        // later declaration in the same declarative part.
        TypeDef::Incomplete => {
            head.push(';');
            w.line(&head);
        }
    }
}

/// `(D : Mode := IDLE)`, or nothing.
fn discriminant_text(discriminants: &[Discriminant]) -> String {
    if discriminants.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = discriminants
        .iter()
        .map(|discriminant| {
            let mut out = String::from(discriminant.name.as_str());
            out.push_str(" : ");
            out.push_str(&path_text(&discriminant.type_));
            if let Some(default) = &discriminant.default {
                out.push_str(" := ");
                out.push_str(&expr_text(default));
            }
            out
        })
        .collect();
    let mut out = String::from(" (");
    out.push_str(&parts.join("; "));
    out.push(')');
    out
}

fn render_component(w: &mut Writer, node: &Component) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    line.push_str(" : ");
    line.push_str(&path_text(&node.type_));
    line.push(';');
    w.line(&line);
}

/// `case D is … end case;`
///
/// An arm with no components is written `null;` — RM 3.8 gives `null` as
/// a `component_list` of its own, and an arm with nothing at all between
/// `=>` and the next `when` is not Ada.
fn render_variant(w: &mut Writer, node: &Variant) {
    let mut head = String::from("case ");
    head.push_str(node.discriminant.as_str());
    head.push_str(" is");
    w.line(&head);
    w.indent();
    for arm in &node.arms {
        render_arm(w, arm);
    }
    w.dedent();
    w.line("end case;");
}

fn render_arm(w: &mut Writer, node: &Arm) {
    let mut head = String::from("when ");
    head.push_str(
        &node
            .choices
            .iter()
            .map(choice_text)
            .collect::<Vec<String>>()
            .join(" | "),
    );
    head.push_str(" =>");
    w.line(&head);
    w.indent();
    if node.components.is_empty() {
        w.line("null;");
    } else {
        for component in &node.components {
            render_component(w, component);
        }
    }
    w.dedent();
}

fn choice_text(node: &Choice) -> String {
    match node {
        Choice::Value(value) => expr_text(value),
        Choice::Others => "others".to_string(),
    }
}

fn index_text(node: &Index) -> String {
    match node {
        Index::Range { low, high } => {
            let mut out = expr_text(low);
            out.push_str(" .. ");
            out.push_str(&expr_text(high));
            out
        }
        Index::Unconstrained(mark) => {
            let mut out = path_text(mark);
            out.push_str(" range <>");
            out
        }
    }
}

fn render_subtype(w: &mut Writer, node: &SubtypeDecl) {
    doc(w, &node.doc);
    let mut line = String::from("subtype ");
    line.push_str(node.name.as_str());
    line.push_str(" is ");
    line.push_str(&path_text(&node.mark));
    line.push(';');
    w.line(&line);
}

fn render_object(w: &mut Writer, node: &ObjectDecl) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    line.push_str(" : constant ");
    line.push_str(&path_text(&node.type_));
    line.push_str(" := ");
    line.push_str(&expr_text(&node.value));
    line.push(';');
    w.line(&line);
}

fn render_enum_rep(w: &mut Writer, node: &EnumRep) {
    doc(w, &node.doc);
    let mut line = String::from("for ");
    line.push_str(node.type_.as_str());
    line.push_str(" use (");
    line.push_str(
        &node
            .values
            .iter()
            .map(|(literal, code)| {
                let mut out = String::from(literal.as_str());
                out.push_str(" => ");
                out.push_str(&expr_text(code));
                out
            })
            .collect::<Vec<String>>()
            .join(", "),
    );
    line.push_str(");");
    w.line(&line);
}

fn subprogram_head(node: &Subprogram) -> String {
    let mut line = String::from(match node.kind {
        SubprogramKind::Procedure => "procedure ",
        SubprogramKind::Function => "function ",
    });
    line.push_str(node.name.as_str());
    if !node.params.is_empty() {
        line.push_str(" (");
        line.push_str(
            &node
                .params
                .iter()
                .map(param_text)
                .collect::<Vec<String>>()
                .join("; "),
        );
        line.push(')');
    }
    if let Some(result) = &node.result {
        line.push_str(" return ");
        line.push_str(&path_text(result));
    }
    line
}

fn render_subprogram(w: &mut Writer, node: &Subprogram) {
    doc(w, &node.doc);
    let mut line = subprogram_head(node);
    if node.is_abstract {
        line.push_str(" is abstract");
    }
    line.push(';');
    w.line(&line);
}

/// `procedure P (…) is <declarations> begin <statements> end P;`
///
/// The leading run of [`Stmt::Var`], [`Stmt::Subtype`] and
/// [`Stmt::Comment`] goes into the declarative part; everything after it
/// is the statement sequence, and an empty one is `null;` (RM 5.1 — a
/// `sequence_of_statements` is never empty).
fn render_subprogram_body(w: &mut Writer, node: &SubprogramBody) {
    doc(w, &node.spec.doc);
    let mut head = subprogram_head(&node.spec);
    head.push_str(" is");
    w.line(&head);
    let split = node
        .body
        .iter()
        .position(|statement| {
            !matches!(
                statement,
                Stmt::Var(_) | Stmt::Subtype { .. } | Stmt::Comment(_)
            )
        })
        .unwrap_or(node.body.len());
    w.indent();
    for statement in &node.body[..split] {
        match statement {
            Stmt::Var(var) => w.line(&var_text(var)),
            Stmt::Subtype { name, mark } => w.line(&subtype_text(name, mark)),
            Stmt::Comment(lines) => doc(w, lines),
            _ => {}
        }
    }
    w.dedent();
    w.line("begin");
    w.indent();
    if node.body[split..].is_empty() {
        w.line("null;");
    } else {
        render_stmts(w, &node.body[split..]);
    }
    w.dedent();
    let mut tail = String::from("end ");
    tail.push_str(node.spec.name.as_str());
    tail.push(';');
    w.line(&tail);
}

/// `Name : Mark (Constraint) := Value;`
fn var_text(node: &VarDecl) -> String {
    let mut out = String::from(node.name.as_str());
    out.push_str(" : ");
    out.push_str(&path_text(&node.mark));
    if let Some(constraint) = &node.constraint {
        out.push_str(" (");
        out.push_str(&expr_text(constraint));
        out.push(')');
    }
    if let Some(value) = &node.value {
        out.push_str(" := ");
        out.push_str(&expr_text(value));
    }
    out.push(';');
    out
}

fn subtype_text(name: &Ident, mark: &Path) -> String {
    let mut out = String::from("subtype ");
    out.push_str(name.as_str());
    out.push_str(" is ");
    out.push_str(&path_text(mark));
    out.push(';');
    out
}

/// A statement list. A declaration met mid-list opens a `declare` block
/// that scopes the rest of the list, so everything the walk declared
/// stays visible to everything after it.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Ada nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_stmts(w: &mut Writer, nodes: &[Stmt]) {
    let mut at = 0;
    while at < nodes.len() {
        let statement = &nodes[at];
        if matches!(statement, Stmt::Var(_) | Stmt::Subtype { .. }) {
            w.line("declare");
            w.indent();
            while at < nodes.len() {
                match &nodes[at] {
                    Stmt::Var(var) => w.line(&var_text(var)),
                    Stmt::Subtype { name, mark } => w.line(&subtype_text(name, mark)),
                    Stmt::Comment(lines) => doc(w, lines),
                    _ => break,
                }
                at += 1;
            }
            w.dedent();
            w.line("begin");
            w.indent();
            if nodes[at..].is_empty() {
                w.line("null;");
            } else {
                render_stmts(w, &nodes[at..]);
            }
            w.dedent();
            w.line("end;");
            return;
        }
        render_stmt(w, statement);
        at += 1;
    }
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Ada nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::Null => w.line("null;"),
        // Reached only through `render_stmts`, which folds these into a
        // `declare` block or the subprogram's declarative part.
        Stmt::Var(var) => w.line(&var_text(var)),
        Stmt::Subtype { name, mark } => w.line(&subtype_text(name, mark)),
        Stmt::Assign { place, value } => {
            let mut line = expr_text(place);
            line.push_str(" := ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Stmt::Call(value) => {
            let mut line = expr_text(value);
            line.push(';');
            w.line(&line);
        }
        Stmt::Return(value) => match value {
            Some(value) => {
                let mut line = String::from("return ");
                line.push_str(&expr_text(value));
                line.push(';');
                w.line(&line);
            }
            None => w.line("return;"),
        },
        Stmt::If {
            condition,
            then,
            otherwise,
        } => {
            let mut head = String::from("if ");
            head.push_str(&expr_text(condition));
            head.push_str(" then");
            w.line(&head);
            w.indent();
            if then.is_empty() {
                w.line("null;");
            } else {
                render_stmts(w, then);
            }
            w.dedent();
            if let Some(otherwise) = otherwise {
                w.line("else");
                w.indent();
                if otherwise.is_empty() {
                    w.line("null;");
                } else {
                    render_stmts(w, otherwise);
                }
                w.dedent();
            }
            w.line("end if;");
        }
        Stmt::For {
            counter,
            low,
            high,
            body,
        } => {
            let mut head = String::from("for ");
            head.push_str(counter.as_str());
            head.push_str(" in ");
            head.push_str(&expr_text(low));
            head.push_str(" .. ");
            head.push_str(&expr_text(high));
            head.push_str(" loop");
            w.line(&head);
            w.indent();
            if body.is_empty() {
                w.line("null;");
            } else {
                render_stmts(w, body);
            }
            w.dedent();
            w.line("end loop;");
        }
        Stmt::While { condition, body } => {
            let mut head = String::from("while ");
            head.push_str(&expr_text(condition));
            head.push_str(" loop");
            w.line(&head);
            w.indent();
            if body.is_empty() {
                w.line("null;");
            } else {
                render_stmts(w, body);
            }
            w.dedent();
            w.line("end loop;");
        }
        Stmt::Exit => w.line("exit;"),
        Stmt::Declare { vars, body } => {
            w.line("declare");
            w.indent();
            for var in vars {
                w.line(&var_text(var));
            }
            w.dedent();
            w.line("begin");
            w.indent();
            if body.is_empty() {
                w.line("null;");
            } else {
                render_stmts(w, body);
            }
            w.dedent();
            w.line("end;");
        }
        Stmt::Comment(lines) => doc(w, lines),
    }
}

fn param_text(node: &Param) -> String {
    let mut out = String::from(node.name.as_str());
    out.push_str(" : ");
    // The mode is written even for `in`, which is the default (RM 6.1).
    // The IDL direction is the one part of an operation's signature that
    // no other column in this crate can keep, and a spelled-out `in` is
    // what makes it visible in the generated text.
    out.push_str(mode_text(node.mode));
    out.push(' ');
    out.push_str(&path_text(&node.type_));
    out
}

const fn mode_text(mode: Mode) -> &'static str {
    mode.as_str()
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_ada.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => path_text(name),
        Expr::Call { name, args } => {
            let mut out = path_text(name);
            // A parameterless call is the bare name: `F ()` is not Ada
            // (RM 6.4 — an actual_parameter_part is non-empty).
            if !args.is_empty() {
                out.push_str(" (");
                out.push_str(
                    &args
                        .iter()
                        .map(expr_text)
                        .collect::<Vec<String>>()
                        .join(", "),
                );
                out.push(')');
            }
            out
        }
        Expr::Negate(inner) => {
            let mut out = String::from("-");
            out.push_str(&expr_text(inner));
            out
        }
        Expr::Binary { op, lhs, rhs } => {
            let mut out = operand_text(lhs);
            out.push(' ');
            out.push_str(ada_op(op));
            out.push(' ');
            out.push_str(&operand_text(rhs));
            out
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(ada_op(op));
            if *op != "-" {
                out.push(' ');
            }
            out.push_str(&operand_text(value));
            out
        }
        Expr::Selected { value, name } => {
            let mut out = operand_text(value);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Expr::Indexed { value, index } => {
            let mut out = operand_text(value);
            out.push_str(" (");
            out.push_str(&expr_text(index));
            out.push(')');
            out
        }
        Expr::Deref(value) => {
            let mut out = operand_text(value);
            out.push_str(".all");
            out
        }
        Expr::Slice { value, low, high } => {
            let mut out = operand_text(value);
            out.push_str(" (");
            out.push_str(&expr_text(low));
            out.push_str(" .. ");
            out.push_str(&expr_text(high));
            out.push(')');
            out
        }
        Expr::Attribute { value, name, args } => {
            let mut out = operand_text(value);
            out.push('\'');
            out.push_str(name.as_str());
            if !args.is_empty() {
                out.push_str(" (");
                out.push_str(
                    &args
                        .iter()
                        .map(expr_text)
                        .collect::<Vec<String>>()
                        .join(", "),
                );
                out.push(')');
            }
            out
        }
        Expr::Convert { mark, value } => {
            let mut out = path_text(mark);
            out.push_str(" (");
            out.push_str(&expr_text(value));
            out.push(')');
            out
        }
        Expr::Aggregate(parts) => {
            if parts.is_empty() {
                return "(null record)".to_string();
            }
            let mut out = String::from("(");
            out.push_str(
                &parts
                    .iter()
                    .map(|part| {
                        let mut one = String::from(part.choice.as_str());
                        one.push_str(" => ");
                        one.push_str(&expr_text(&part.value));
                        one
                    })
                    .collect::<Vec<String>>()
                    .join(", "),
            );
            out.push(')');
            out
        }
        Expr::BoxDefault => "<>".to_string(),
        Expr::New { mark, init } => {
            let mut out = String::from("new ");
            out.push_str(&path_text(mark));
            match init {
                AllocInit::Range { low, high } => {
                    out.push_str(" (");
                    out.push_str(&expr_text(low));
                    out.push_str(" .. ");
                    out.push_str(&expr_text(high));
                    out.push(')');
                }
                AllocInit::Qualified(value) => {
                    out.push_str("'(");
                    out.push_str(&expr_text(value));
                    out.push(')');
                }
            }
            out
        }
    }
}

/// An operand, parenthesised where its own operator could re-associate.
///
/// Blanket rather than minimal: Ada refuses `A and then B or else C`
/// outright (RM 4.4 puts one `logical_operator` class per expression),
/// and a spelled-out parenthesis is never wrong.
///
/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_ada.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn operand_text(node: &Expr) -> String {
    match node {
        Expr::Binary { .. } | Expr::Unary { .. } | Expr::Negate(_) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(node));
            out.push(')');
            out
        }
        _ => expr_text(node),
    }
}

/// The Ada spelling of the shared walk's operator.
fn ada_op(op: &str) -> &str {
    match op {
        "==" => "=",
        "!=" => "/=",
        "&&" => "and then",
        "||" => "or else",
        "&" => "and",
        "|" => "or",
        "^" => "xor",
        "%" => "mod",
        "!" | "~" => "not",
        other => other,
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Int(text) | Literal::Float(text) => text.clone(),
        Literal::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        // The code point as a number, not `'x'`. An Ada character
        // literal is a value of `Standard.Character`, which is Latin-1
        // (RM A.1); an IDL `char` is one octet and an IDL `wchar` two,
        // and this column holds both as `Interfaces` integers. Writing
        // the number is what keeps the constant's type the member's.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => ada_string(text),
        // A positional array aggregate. Ada writes a hexadecimal literal
        // as `16#FF#` (RM 2.4.2) — there is no `0x`.
        Literal::Bytes(bytes) => {
            let mut out = String::from("(");
            let parts: Vec<String> = bytes
                .iter()
                .map(|b| alloc::format!("16#{b:02X}#"))
                .collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Literal::Null => "null".to_string(),
    }
}

/// An Ada string literal.
///
/// RM 2.6: a quotation mark inside a string literal is written twice.
/// There is no backslash escape in Ada at all, so
/// [`crate::writer::escape_string`] would put a graphic character into
/// the literal rather than an escape — a `\n` in an IDL constant would
/// arrive in the generated spec as a backslash and an `n`.
///
/// Characters an Ada string literal cannot hold — a line break, a
/// control character — are refused by `zerodds_lang_rules::adapter::ada`
/// before they reach here, because there is no spelling for them and
/// dropping one would change the constant.
fn ada_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "--  ", doc.as_lines());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;
    use zerodds_lang_cst::ada::{Primitive, Record};

    fn component(name: &str, type_: Primitive) -> Component {
        Component {
            doc: Doc::none(),
            name: Ident::new(name),
            type_: type_.path(),
        }
    }

    #[test]
    fn a_record_writes_one_component_per_line() {
        let node = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Pose"),
            def: TypeDef::Record(Record::plain(vec![
                component("X", Primitive::Integer32),
                component("Y", Primitive::Float64),
            ])),
        });
        assert_eq!(
            decl(&node),
            "type Pose is record\n   \
             X : Interfaces.Integer_32;\n   \
             Y : Interfaces.IEEE_Float_64;\n\
             end record;\n"
        );
    }

    #[test]
    fn a_record_with_no_components_is_a_null_record() {
        // RM 3.8: `record end record` has an empty `component_list`,
        // which is not Ada. The corpus contains aggregates with no
        // members, so this arm is reached.
        let node = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Empty"),
            def: TypeDef::Record(Record::plain(Vec::new())),
        });
        assert_eq!(decl(&node), "type Empty is null record;\n");
    }

    #[test]
    fn a_discriminated_record_writes_its_variant_part() {
        let node = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Payload"),
            def: TypeDef::Record(Record {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                discriminants: vec![Discriminant {
                    name: Ident::new("D"),
                    type_: Path::single("Mode"),
                    default: Some(Expr::name("IDLE")),
                }],
                components: Vec::new(),
                variant: Some(Variant {
                    discriminant: Ident::new("D"),
                    arms: vec![
                        Arm {
                            choices: vec![Choice::Value(Expr::name("IDLE"))],
                            components: vec![component("Idle", Primitive::Integer32)],
                        },
                        Arm {
                            choices: vec![Choice::Others],
                            components: Vec::new(),
                        },
                    ],
                }),
            }),
        });
        assert_eq!(
            decl(&node),
            "type Payload (D : Mode := IDLE) is record\n   \
             case D is\n      \
             when IDLE =>\n         \
             Idle : Interfaces.Integer_32;\n      \
             when others =>\n         \
             null;\n   \
             end case;\n\
             end record;\n"
        );
    }

    #[test]
    fn an_enumeration_and_its_representation_clause_are_two_declarations() {
        // RM 3.5.1 numbers the literals 0, 1, 2 … in declaration order;
        // the clause of RM 13.4 is the only thing that carries the IDL
        // `@value` gaps into the generated spec.
        let type_ = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Mode"),
            def: TypeDef::Enumeration(vec![Ident::new("IDLE"), Ident::new("ACTIVE")]),
        });
        let rep = Decl::EnumRep(EnumRep {
            doc: Doc::none(),
            type_: Ident::new("Mode"),
            values: vec![
                (Ident::new("IDLE"), Expr::int(0)),
                (Ident::new("ACTIVE"), Expr::int(4)),
            ],
        });
        assert_eq!(decl(&type_), "type Mode is (IDLE, ACTIVE);\n");
        assert_eq!(decl(&rep), "for Mode use (IDLE => 0, ACTIVE => 4);\n");
    }

    #[test]
    fn an_unconstrained_array_and_the_access_type_that_reaches_it() {
        let array = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Zd_Seq_Long_Array"),
            def: TypeDef::Array {
                index: vec![Index::Unconstrained(Path::single("Natural"))],
                element: Primitive::Integer32.path(),
            },
        });
        let access = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Zd_Seq_Long"),
            def: TypeDef::Access(Path::single("Zd_Seq_Long_Array")),
        });
        assert_eq!(
            decl(&array),
            "type Zd_Seq_Long_Array is array (Natural range <>) of Interfaces.Integer_32;\n"
        );
        assert_eq!(
            decl(&access),
            "type Zd_Seq_Long is access Zd_Seq_Long_Array;\n"
        );
    }

    #[test]
    fn a_constrained_array_writes_its_bounds() {
        let node = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Zd_Arr_Long_2"),
            def: TypeDef::Array {
                index: vec![Index::Range {
                    low: Expr::int(0),
                    high: Expr::int(1),
                }],
                element: Primitive::Integer32.path(),
            },
        });
        assert_eq!(
            decl(&node),
            "type Zd_Arr_Long_2 is array (0 .. 1) of Interfaces.Integer_32;\n"
        );
    }

    #[test]
    fn an_incomplete_type_declares_a_name_and_nothing_else() {
        // RM 3.10.1. The completion is a later declaration; between the
        // two the name may only be the designated subtype of an access
        // type.
        let incomplete = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Node"),
            def: TypeDef::Incomplete,
        });
        let access = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Node_Array"),
            def: TypeDef::Access(Path::single("Node")),
        });
        assert_eq!(decl(&incomplete), "type Node;\n");
        assert_eq!(decl(&access), "type Node_Array is access Node;\n");
    }

    #[test]
    fn a_subtype_is_not_a_derived_type() {
        // RM 3.2.2 against RM 3.4: the first is another name for one
        // type, the second is a new type with its own identity.
        let alias = Decl::Subtype(SubtypeDecl {
            doc: Doc::none(),
            name: Ident::new("Distance"),
            mark: Primitive::Integer32.path(),
        });
        let derived = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Flags"),
            def: TypeDef::Derived(Primitive::Unsigned16.path()),
        });
        assert_eq!(decl(&alias), "subtype Distance is Interfaces.Integer_32;\n");
        assert_eq!(
            decl(&derived),
            "type Flags is new Interfaces.Unsigned_16;\n"
        );
    }

    #[test]
    fn a_constant_names_its_type_because_ada_has_no_untyped_one() {
        let node = Decl::Object(ObjectDecl {
            doc: Doc::none(),
            name: Ident::new("LIMIT"),
            type_: Primitive::Unsigned8.path(),
            value: Expr::int(8),
        });
        assert_eq!(
            decl(&node),
            "LIMIT : constant Interfaces.Unsigned_8 := 8;\n"
        );
    }

    #[test]
    fn an_abstract_subprogram_writes_its_modes() {
        let procedure = Decl::Subprogram(Subprogram {
            doc: Doc::none(),
            kind: SubprogramKind::Procedure,
            name: Ident::new("Go"),
            params: vec![
                Param {
                    name: Ident::new("Self"),
                    mode: Mode::In,
                    type_: Path::single("Control"),
                },
                Param {
                    name: Ident::new("Target"),
                    mode: Mode::Out,
                    type_: Path::single("Pose"),
                },
            ],
            result: None,
            is_abstract: true,
        });
        let function = Decl::Subprogram(Subprogram {
            doc: Doc::none(),
            kind: SubprogramKind::Function,
            name: Ident::new("Status"),
            params: vec![Param {
                name: Ident::new("Self"),
                mode: Mode::In,
                type_: Path::single("Control"),
            }],
            result: Some(Primitive::Integer32.path()),
            is_abstract: true,
        });
        assert_eq!(
            decl(&procedure),
            "procedure Go (Self : in Control; Target : out Pose) is abstract;\n"
        );
        assert_eq!(
            decl(&function),
            "function Status (Self : in Control) return Interfaces.Integer_32 is abstract;\n"
        );
    }

    #[test]
    fn a_string_literal_doubles_its_quote_and_never_escapes_it() {
        // RM 2.6. A backslash is an ordinary graphic character in Ada,
        // so the C-family escape would land in the value.
        assert_eq!(
            expr(&Expr::Literal(Literal::Str("a\"b".to_string()))),
            "\"a\"\"b\""
        );
        assert_eq!(
            expr(&Expr::Literal(Literal::Str("a\\b".to_string()))),
            "\"a\\b\""
        );
    }

    #[test]
    fn a_string_constant_is_a_call_because_a_private_type_has_no_literal() {
        let node = Decl::Object(ObjectDecl {
            doc: Doc::none(),
            name: Ident::new("TAG"),
            type_: Primitive::UnboundedString.path(),
            value: Expr::Call {
                name: Path::new(&["Ada", "Strings", "Unbounded", "To_Unbounded_String"]),
                args: vec![Expr::Literal(Literal::Str("abc".to_string()))],
            },
        });
        assert_eq!(
            decl(&node),
            "TAG : constant Ada.Strings.Unbounded.Unbounded_String := \
             Ada.Strings.Unbounded.To_Unbounded_String (\"abc\");\n"
        );
    }

    #[test]
    fn a_file_writes_its_context_clause_before_the_package() {
        // RM 10.1.2: a `with` clause belongs to the compilation unit and
        // comes before it, not inside it.
        let node = File {
            doc: Doc::line("Generated. Do not edit."),
            withs: vec![Path::single("Interfaces")],
            use_types: Vec::new(),
            package: Ident::new("Zd_Types"),
            decls: vec![Decl::Type(TypeDecl {
                doc: Doc::none(),
                name: Ident::new("Pose"),
                def: TypeDef::Record(Record::plain(vec![component("X", Primitive::Integer32)])),
            })],
        };
        assert_eq!(
            file(&node),
            "--  Generated. Do not edit.\n\
             \n\
             with Interfaces;\n\
             \n\
             package Zd_Types is\n\
             \n   \
             type Pose is record\n      \
             X : Interfaces.Integer_32;\n   \
             end record;\n\
             \n\
             end Zd_Types;\n"
        );
    }

    #[test]
    fn a_use_type_clause_stands_after_the_with_it_depends_on() {
        // RM 10.1.2 makes both context items; RM 8.4 needs the subtype
        // mark visible, and it is the `with` that makes it so.
        let node = File {
            doc: Doc::none(),
            withs: vec![Path::single("Interfaces")],
            use_types: vec![Primitive::Integer16.path()],
            package: Ident::new("Zd_Types"),
            decls: Vec::new(),
        };
        assert_eq!(
            file(&node),
            "with Interfaces;\n\
             use type Interfaces.Integer_16;\n\
             \n\
             package Zd_Types is\n\
             \n\
             end Zd_Types;\n"
        );
    }

    #[test]
    fn a_boolean_is_written_with_the_case_ada_declares_it_in() {
        // RM A.1 declares `Boolean` as `(False, True)`. The identifiers
        // fold, so the case is convention — but a file that wrote `true`
        // would differ from every Ada source beside it.
        assert_eq!(expr(&Expr::Literal(Literal::Bool(true))), "True");
        assert_eq!(expr(&Expr::Literal(Literal::Bool(false))), "False");
    }

    #[test]
    fn a_hexadecimal_literal_uses_the_ada_base_form() {
        // RM 2.4.2: `16#FF#`, never `0xFF`.
        assert_eq!(
            expr(&Expr::Literal(Literal::Bytes(alloc::vec![1, 255]))),
            "(16#01#, 16#FF#)"
        );
    }

    #[test]
    fn a_comment_uses_the_ada_marker_and_has_no_closing_delimiter() {
        // RM 2.7.
        let node = Decl::Comment(Doc::lines(&["first", "second"]));
        assert_eq!(decl(&node), "--  first\n--  second\n");
    }

    use zerodds_lang_cst::ada::{AllocInit, Association, Stmt, SubprogramBody, VarDecl};

    fn name(text: &str) -> Expr {
        Expr::name(text)
    }

    #[test]
    fn the_walks_operators_take_their_ada_spellings() {
        let node = Expr::Binary {
            op: "&&",
            lhs: alloc::boxed::Box::new(Expr::Binary {
                op: "==",
                lhs: alloc::boxed::Box::new(name("A")),
                rhs: alloc::boxed::Box::new(name("B")),
            }),
            rhs: alloc::boxed::Box::new(Expr::Unary {
                op: "!",
                value: alloc::boxed::Box::new(name("C")),
            }),
        };
        // Both operands parenthesised: Ada refuses a mixed logical
        // expression without them (RM 4.4), and blanket parentheses are
        // never wrong.
        assert_eq!(expr(&node), "(A = B) and then (not C)");
    }

    #[test]
    fn selection_indexing_dereference_and_slice_render_as_ada_writes_them() {
        let selected = Expr::Selected {
            value: alloc::boxed::Box::new(name("v")),
            name: Ident::new("Key"),
        };
        assert_eq!(expr(&selected), "v.Key");
        let indexed = Expr::Indexed {
            value: alloc::boxed::Box::new(name("v")),
            index: alloc::boxed::Box::new(name("zd_i")),
        };
        assert_eq!(expr(&indexed), "v (zd_i)");
        let deref = Expr::Deref(alloc::boxed::Box::new(name("v")));
        assert_eq!(expr(&deref), "v.all");
        let slice = Expr::Slice {
            value: alloc::boxed::Box::new(deref),
            low: alloc::boxed::Box::new(Expr::int(0)),
            high: alloc::boxed::Box::new(name("zd_n")),
        };
        assert_eq!(expr(&slice), "v.all (0 .. zd_n)");
    }

    #[test]
    fn an_attribute_a_conversion_and_an_aggregate_render() {
        let length = Expr::Attribute {
            value: alloc::boxed::Box::new(Expr::Deref(alloc::boxed::Box::new(name("v")))),
            name: Ident::new("Length"),
            args: Vec::new(),
        };
        assert_eq!(expr(&length), "v.all'Length");
        let enum_val = Expr::Attribute {
            value: alloc::boxed::Box::new(name("Colour")),
            name: Ident::new("Enum_Val"),
            args: vec![Expr::int(4)],
        };
        assert_eq!(expr(&enum_val), "Colour'Enum_Val (4)");
        let convert = Expr::Convert {
            mark: Primitive::Unsigned32.path(),
            value: alloc::boxed::Box::new(length),
        };
        assert_eq!(expr(&convert), "Interfaces.Unsigned_32 (v.all'Length)");
        let aggregate = Expr::Aggregate(vec![
            Association {
                choice: Ident::new("Present"),
                value: Expr::Literal(Literal::Bool(true)),
            },
            Association {
                choice: Ident::new("Value"),
                value: Expr::int(7),
            },
        ]);
        assert_eq!(expr(&aggregate), "(Present => True, Value => 7)");
        let boxed = Expr::Aggregate(vec![Association {
            choice: Ident::new("others"),
            value: Expr::BoxDefault,
        }]);
        assert_eq!(expr(&boxed), "(others => <>)");
    }

    #[test]
    fn an_allocator_takes_a_range_or_a_qualified_value() {
        let empty = Expr::New {
            mark: Path::single("Zd_Seq_Long_Array"),
            init: AllocInit::Range {
                low: alloc::boxed::Box::new(Expr::int(0)),
                high: alloc::boxed::Box::new(Expr::int(-1)),
            },
        };
        assert_eq!(expr(&empty), "new Zd_Seq_Long_Array (0 .. -1)");
        let kept = Expr::New {
            mark: Path::single("Zd_Seq_Long_Array"),
            init: AllocInit::Qualified(alloc::boxed::Box::new(name("v"))),
        };
        assert_eq!(expr(&kept), "new Zd_Seq_Long_Array'(v)");
    }

    #[test]
    fn a_declaration_mid_list_opens_a_declare_block_over_the_rest() {
        // RM 3.11 keeps declarations out of a statement sequence; the
        // fold is what keeps the walk's declared local visible to every
        // statement after it.
        let nodes = vec![
            Stmt::Assign {
                place: name("a"),
                value: Expr::int(1),
            },
            Stmt::Var(VarDecl {
                name: Ident::new("zd_n"),
                mark: Primitive::Unsigned32.path(),
                constraint: None,
                value: Some(Expr::int(0)),
            }),
            Stmt::Assign {
                place: name("a"),
                value: name("zd_n"),
            },
        ];
        assert_eq!(
            stmts(&nodes),
            "a := 1;\n\
             declare\n   \
             zd_n : Interfaces.Unsigned_32 := 0;\n\
             begin\n   \
             a := zd_n;\n\
             end;\n"
        );
    }

    #[test]
    fn a_subprogram_body_hoists_its_leading_declarations() {
        let node = Decl::SubprogramBody(SubprogramBody {
            spec: Subprogram {
                doc: Doc::none(),
                kind: SubprogramKind::Function,
                name: Ident::new("zd_dec_X"),
                params: vec![Param {
                    name: Ident::new("r"),
                    mode: Mode::InOut,
                    type_: Path::new(&["Zd_Wire", "Reader"]),
                }],
                result: Some(Path::single("X")),
                is_abstract: false,
            },
            body: vec![
                Stmt::Var(VarDecl {
                    name: Ident::new("v"),
                    mark: Path::single("X"),
                    constraint: None,
                    value: None,
                }),
                Stmt::Return(Some(name("v"))),
            ],
        });
        assert_eq!(
            decl(&node),
            "function zd_dec_X (r : in out Zd_Wire.Reader) return X is\n   \
             v : X;\n\
             begin\n   \
             return v;\n\
             end zd_dec_X;\n"
        );
    }

    #[test]
    fn a_constrained_object_declaration_carries_its_discriminant() {
        // RM 3.7.1: the one way a body obtains a value whose
        // discriminant it only knows at run time.
        let node = Stmt::Declare {
            vars: vec![VarDecl {
                name: Ident::new("zd_nd"),
                mark: Path::single("Zd_Sample"),
                constraint: Some(name("zd_d")),
                value: None,
            }],
            body: vec![Stmt::Assign {
                place: name("v"),
                value: name("zd_nd"),
            }],
        };
        assert_eq!(
            stmt(&node),
            "declare\n   \
             zd_nd : Zd_Sample (zd_d);\n\
             begin\n   \
             v := zd_nd;\n\
             end;\n"
        );
    }

    #[test]
    fn the_loop_forms_and_exit_render() {
        let counted = Stmt::For {
            counter: Ident::new("zd_i"),
            low: Expr::int(0),
            high: Expr::int(2),
            body: vec![Stmt::Null],
        };
        assert_eq!(
            stmt(&counted),
            "for zd_i in 0 .. 2 loop\n   null;\nend loop;\n"
        );
        let headed = Stmt::While {
            condition: Expr::Literal(Literal::Bool(true)),
            body: vec![Stmt::Exit],
        };
        assert_eq!(stmt(&headed), "while True loop\n   exit;\nend loop;\n");
    }

    #[test]
    fn an_if_writes_its_else_and_an_empty_arm_is_null() {
        let node = Stmt::If {
            condition: name("ok"),
            then: Vec::new(),
            otherwise: Some(vec![Stmt::Return(None)]),
        };
        assert_eq!(
            stmt(&node),
            "if ok then\n   null;\nelse\n   return;\nend if;\n"
        );
    }
}
