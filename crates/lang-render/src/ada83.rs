// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Ada 83 CST. Spec §5.
//!
//! Line for line the same style `zerodds_lang_render::ada` writes —
//! three spaces per level, `--` comments, `Mixed_Case_With_Underscores`
//! — because RM83 fixes none of that either, and there is nothing in
//! this crate's Ada 83 support that departs from the convention its
//! Ada 2012 sibling already follows.
//!
//! What differs is only what `zerodds_lang_cst::ada83` has no node for:
//! no `use type` clause line (see the module documentation there), no
//! enumeration type and no representation clause, no `is abstract`.
//! Everything else below is the identical rendering of an identically
//! shaped node.

use crate::writer::{Writer, comment_lines, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::ada83::{
    AllocInit, Arm, Choice, Component, Decl, Discriminant, Expr, File, Index, Mode, ObjectDecl,
    Param, Stmt, Subprogram, SubprogramBody, SubprogramKind, SubtypeDecl, TypeDecl, TypeDef,
    VarDecl, Variant,
};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// The indentation one level costs.
const UNIT: &str = "   ";

/// Render a whole package specification.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::new(UNIT);
    doc(&mut w, &file.doc);
    if !file.withs.is_empty() {
        if !file.doc.is_empty() {
            w.blank_line();
        }
        for unit in &file.withs {
            let mut line = String::from("with ");
            line.push_str(&path_text(unit));
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
        // RM83 3.10.1: the name and nothing else.
        TypeDef::Incomplete => {
            head.push(';');
            w.line(&head);
        }
    }
}

/// `(D : Long_Integer := 0)`, or nothing.
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

/// `case D is … end case;` (RM83 3.7.3).
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

fn subprogram_head(
    kind: SubprogramKind,
    name: &Ident,
    params: &[Param],
    result: &Option<Path>,
) -> String {
    let mut line = String::from(match kind {
        SubprogramKind::Procedure => "procedure ",
        SubprogramKind::Function => "function ",
    });
    line.push_str(name.as_str());
    if !params.is_empty() {
        line.push_str(" (");
        line.push_str(
            &params
                .iter()
                .map(param_text)
                .collect::<Vec<String>>()
                .join("; "),
        );
        line.push(')');
    }
    if let Some(result) = result {
        line.push_str(" return ");
        line.push_str(&path_text(result));
    }
    line
}

fn render_subprogram(w: &mut Writer, node: &Subprogram) {
    doc(w, &node.doc);
    let mut line = subprogram_head(node.kind, &node.name, &node.params, &node.result);
    line.push(';');
    w.line(&line);
}

/// `procedure P (…) is <declarations> begin <statements> end P;`
fn render_subprogram_body(w: &mut Writer, node: &SubprogramBody) {
    doc(w, &node.spec.doc);
    let mut head = subprogram_head(
        node.spec.kind,
        &node.spec.name,
        &node.spec.params,
        &node.spec.result,
    );
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
/// that scopes the rest of the list.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Ada 83 nests the arms in
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
/// The model bounds none of the three counts, and Ada 83 nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::Null => w.line("null;"),
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
/// literal per dimension, built in `lang-rules/src/default_ada83.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => path_text(name),
        Expr::Call { name, args } => {
            let mut out = path_text(name);
            // A parameterless call is the bare name (RM83 6.4).
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
/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_ada83.rs`, and
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
        // The code point as a number: an IDL `char`/`wchar` is a
        // `Long_Integer` in this column, not `Standard.Character`.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => ada_string(text),
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

/// An Ada string literal — RM83 2.6 doubles the quote, unchanged in
/// every later revision, and there is no backslash escape.
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
    use zerodds_lang_cst::ada83::{Primitive, Record};

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
                component("X", Primitive::LongInteger),
                component("Y", Primitive::LongFloat),
            ])),
        });
        assert_eq!(
            decl(&node),
            "type Pose is record\n   \
             X : Long_Integer;\n   \
             Y : Long_Float;\n\
             end record;\n"
        );
    }

    #[test]
    fn a_record_with_no_components_is_a_null_record() {
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
                    type_: Primitive::LongInteger.path(),
                    default: Some(Expr::int(0)),
                }],
                components: Vec::new(),
                variant: Some(Variant {
                    discriminant: Ident::new("D"),
                    arms: vec![
                        Arm {
                            choices: vec![Choice::Value(Expr::int(0))],
                            components: vec![component("Idle", Primitive::LongInteger)],
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
            "type Payload (D : Long_Integer := 0) is record\n   \
             case D is\n      \
             when 0 =>\n         \
             Idle : Long_Integer;\n      \
             when others =>\n         \
             null;\n   \
             end case;\n\
             end record;\n"
        );
    }

    #[test]
    fn no_enumeration_representation_clause_form_exists() {
        // Confirms the CST cannot even express what the Ada 2012 column
        // writes: an enumerator here is an object declaration, not a
        // type plus a clause. See the module documentation.
        let node = Decl::Object(ObjectDecl {
            doc: Doc::none(),
            name: Ident::new("ACTIVE"),
            type_: Primitive::LongInteger.path(),
            value: Expr::int(4),
        });
        assert_eq!(decl(&node), "ACTIVE : constant Long_Integer := 4;\n");
    }

    #[test]
    fn an_unconstrained_array_and_the_access_type_that_reaches_it() {
        let array = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Zd_Seq_Long_Array"),
            def: TypeDef::Array {
                index: vec![Index::Unconstrained(Path::single("Natural"))],
                element: Primitive::LongInteger.path(),
            },
        });
        let access = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Zd_Seq_Long"),
            def: TypeDef::Access(Path::single("Zd_Seq_Long_Array")),
        });
        assert_eq!(
            decl(&array),
            "type Zd_Seq_Long_Array is array (Natural range <>) of Long_Integer;\n"
        );
        assert_eq!(
            decl(&access),
            "type Zd_Seq_Long is access Zd_Seq_Long_Array;\n"
        );
    }

    #[test]
    fn a_string_is_an_access_to_character_not_unbounded_string() {
        let array = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Zd_Str_Array"),
            def: TypeDef::Array {
                index: vec![Index::Unconstrained(Path::single("Natural"))],
                element: Path::single("Character"),
            },
        });
        assert_eq!(
            decl(&array),
            "type Zd_Str_Array is array (Natural range <>) of Character;\n"
        );
    }

    #[test]
    fn an_incomplete_type_declares_a_name_and_nothing_else() {
        let incomplete = Decl::Type(TypeDecl {
            doc: Doc::none(),
            name: Ident::new("Node"),
            def: TypeDef::Incomplete,
        });
        assert_eq!(decl(&incomplete), "type Node;\n");
    }

    #[test]
    fn a_subprogram_has_no_is_abstract_form() {
        // Unlike the Ada 2012 tree, `Subprogram` has no such field: this
        // is always a forward declaration for a recursive pair.
        let node = Decl::Subprogram(Subprogram {
            doc: Doc::none(),
            kind: SubprogramKind::Function,
            name: Ident::new("Zd_Dec_Node"),
            params: vec![Param {
                name: Ident::new("R"),
                mode: Mode::InOut,
                type_: Path::new(&["Zd_Wire83", "Reader"]),
            }],
            result: Some(Path::single("Node")),
        });
        assert_eq!(
            decl(&node),
            "function Zd_Dec_Node (R : in out Zd_Wire83.Reader) return Node;\n"
        );
    }

    #[test]
    fn a_file_writes_its_context_clause_before_the_package() {
        let node = File {
            doc: Doc::line("Generated. Do not edit."),
            withs: vec![Path::single("Zd_Wire83")],
            package: Ident::new("Zd83_Types"),
            decls: vec![Decl::Type(TypeDecl {
                doc: Doc::none(),
                name: Ident::new("Pose"),
                def: TypeDef::Record(Record::plain(vec![component("X", Primitive::LongInteger)])),
            })],
        };
        assert_eq!(
            file(&node),
            "--  Generated. Do not edit.\n\
             \n\
             with Zd_Wire83;\n\
             \n\
             package Zd83_Types is\n\
             \n   \
             type Pose is record\n      \
             X : Long_Integer;\n   \
             end record;\n\
             \n\
             end Zd83_Types;\n"
        );
    }

    #[test]
    fn a_boolean_is_written_with_the_case_ada_declares_it_in() {
        assert_eq!(expr(&Expr::Literal(Literal::Bool(true))), "True");
        assert_eq!(expr(&Expr::Literal(Literal::Bool(false))), "False");
    }

    #[test]
    fn a_hexadecimal_literal_uses_the_ada_base_form() {
        assert_eq!(
            expr(&Expr::Literal(Literal::Bytes(alloc::vec![1, 255]))),
            "(16#01#, 16#FF#)"
        );
    }

    #[test]
    fn every_aggregate_carries_at_least_one_association() {
        // No `BoxDefault`, no way to write an empty aggregate: this
        // tree cannot spell `(null record)` at all. See the module
        // documentation.
        let node = Expr::Aggregate(vec![zerodds_lang_cst::ada83::Association {
            choice: Ident::new("D"),
            value: Expr::int(0),
        }]);
        assert_eq!(expr(&node), "(D => 0)");
    }

    #[test]
    fn selection_indexing_dereference_and_slice_render_as_ada_writes_them() {
        let name = |text: &str| Expr::name(text);
        let selected = Expr::Selected {
            value: alloc::boxed::Box::new(name("v")),
            name: Ident::new("Key"),
        };
        assert_eq!(expr(&selected), "v.Key");
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
            init: AllocInit::Qualified(alloc::boxed::Box::new(Expr::name("v"))),
        };
        assert_eq!(expr(&kept), "new Zd_Seq_Long_Array'(v)");
    }

    #[test]
    fn a_declaration_mid_list_opens_a_declare_block_over_the_rest() {
        let nodes = vec![
            Stmt::Assign {
                place: Expr::name("a"),
                value: Expr::int(1),
            },
            Stmt::Var(VarDecl {
                name: Ident::new("zd_n"),
                mark: Primitive::LongInteger.path(),
                constraint: None,
                value: Some(Expr::int(0)),
            }),
            Stmt::Assign {
                place: Expr::name("a"),
                value: Expr::name("zd_n"),
            },
        ];
        assert_eq!(
            stmts(&nodes),
            "a := 1;\n\
             declare\n   \
             zd_n : Long_Integer := 0;\n\
             begin\n   \
             a := zd_n;\n\
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
            condition: Expr::name("ok"),
            then: Vec::new(),
            otherwise: Some(vec![Stmt::Return(None)]),
        };
        assert_eq!(
            stmt(&node),
            "if ok then\n   null;\nelse\n   return;\nend if;\n"
        );
    }
}
