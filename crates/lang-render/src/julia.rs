// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Julia CST. Spec §5.
//!
//! One style: four spaces per level, `#` comments, one blank line between
//! declarations. Four spaces because that is what the manual's own style
//! guide and every formatter for the language use, and because Julia has
//! no `gofmt` to make the question moot.
//!
//! Nothing is decided here. A Julia type reads left to right and carries
//! its own length and element type, so unlike [`crate::c`]
//! there is no declarator to split around a name: a field is the name,
//! `::`, and the type.
//!
//! ## `#` and not a docstring
//!
//! Julia's documentation form is a string literal above the declaration,
//! which the parser attaches to the binding. It is an expression that
//! runs when the file is loaded and it can only be attached to a binding
//! Julia has a docsystem entry for — not, for instance, to one field of a
//! struct. A `#` comment carries the same text, is attached to nothing,
//! and cannot fail at load time. For a file whose whole content is type
//! declarations, that is the honest form.
//!
//! ## `$` is escaped in a string literal
//!
//! Julia interpolates `$name` inside `"…"` (manual, "Strings"), so a
//! `const string PROMPT = "$HOME";` written through the C-family escape
//! alone would be a file that either fails to load — `UndefVarError:
//! HOME` — or, worse, loads with whatever `HOME` happens to name. It is
//! escaped here, in the one place a Julia string literal is written.

use crate::writer::{Writer, comment_lines, escape_string, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::julia::{
    AbstractDecl, ConstDecl, Decl, EnumDecl, Enumerator, Expr, Field, File, FunctionDecl,
    FunctionDef, Statement, StructDecl, Type,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    doc(&mut w, &file.doc);
    for decl in &file.decls {
        w.blank_line();
        render_decl(&mut w, decl);
    }
    w.finish()
}

/// Render one declaration, for a caller that has only one.
#[must_use]
pub fn decl(node: &Decl) -> String {
    let mut w = Writer::spaces4();
    render_decl(&mut w, node);
    w.finish()
}

/// Render one type, for a caller that has only one.
#[must_use]
pub fn type_(node: &Type) -> String {
    type_text(node)
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

/// Render one statement, for a caller that has only one.
#[must_use]
pub fn statement(node: &Statement) -> String {
    let mut w = Writer::spaces4();
    render_statement(&mut w, node);
    w.finish()
}

/// Render one expression, for a caller assembling source by hand.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// A Julia string literal, quotes and escapes included.
///
/// The same escapes a [`Literal::Str`] gets — the C-family set and `$`,
/// which Julia interpolates inside `"…"`.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&julia_string(text));
    out.push('"');
    out
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Abstract(decl) => render_abstract(w, decl),
        Decl::Struct(decl) => render_struct(w, decl),
        Decl::Enum(decl) => render_enum(w, decl),
        Decl::Const(decl) => render_const(w, decl),
        Decl::Function(decl) => render_function(w, decl),
        Decl::FunctionDef(decl) => render_function_def(w, decl),
        Decl::Comment(lines) => doc(w, lines),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// `function name(a::T, b)[::R]`, the body, `end`.
fn render_function_def(w: &mut Writer, node: &FunctionDef) {
    doc(w, &node.doc);
    let mut head = String::from("function ");
    head.push_str(node.name.as_str());
    head.push('(');
    let params: Vec<String> = node
        .params
        .iter()
        .map(|param| match &param.type_ {
            Some(type_) => {
                let mut out = String::from(param.name.as_str());
                out.push_str("::");
                out.push_str(&type_text(type_));
                out
            }
            None => param.name.as_str().to_string(),
        })
        .collect();
    head.push_str(&params.join(", "));
    head.push(')');
    if let Some(returns) = &node.returns {
        head.push_str("::");
        head.push_str(&type_text(returns));
    }
    w.line(&head);
    w.indent();
    for statement in &node.body {
        render_statement(w, statement);
    }
    w.dedent();
    w.line("end");
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Julia nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Local { name, type_, value } => {
            let mut line = String::from("local ");
            line.push_str(name.as_str());
            if let Some(type_) = type_ {
                line.push_str("::");
                line.push_str(&type_text(type_));
            }
            if let Some(value) = value {
                line.push_str(" = ");
                line.push_str(&expr_text(value));
            }
            w.line(&line);
        }
        Statement::Assign { place, value } => {
            let mut line = expr_text(place);
            line.push_str(" = ");
            line.push_str(&expr_text(value));
            w.line(&line);
        }
        Statement::Expr(value) => w.line(&expr_text(value)),
        Statement::If {
            condition,
            then,
            otherwise,
        } => {
            let mut head = String::from("if ");
            head.push_str(&expr_text(condition));
            w.line(&head);
            w.indent();
            for statement in then {
                render_statement(w, statement);
            }
            w.dedent();
            if let Some(otherwise) = otherwise {
                w.line("else");
                w.indent();
                for statement in otherwise {
                    render_statement(w, statement);
                }
                w.dedent();
            }
            w.line("end");
        }
        Statement::For {
            counter,
            iter,
            body,
        } => {
            let mut head = String::from("for ");
            head.push_str(counter.as_str());
            head.push_str(" in ");
            head.push_str(&expr_text(iter));
            w.line(&head);
            w.indent();
            for statement in body {
                render_statement(w, statement);
            }
            w.dedent();
            w.line("end");
        }
        Statement::While { condition, body } => {
            let mut head = String::from("while ");
            head.push_str(&expr_text(condition));
            w.line(&head);
            w.indent();
            for statement in body {
                render_statement(w, statement);
            }
            w.dedent();
            w.line("end");
        }
        Statement::Break => w.line("break"),
        Statement::Return(value) => match value {
            Some(value) => {
                let mut line = String::from("return ");
                line.push_str(&expr_text(value));
                w.line(&line);
            }
            None => w.line("return"),
        },
        Statement::Comment(lines) => doc(w, lines),
    }
}

fn render_abstract(w: &mut Writer, node: &AbstractDecl) {
    doc(w, &node.doc);
    let mut line = String::from("abstract type ");
    line.push_str(node.name.as_str());
    line.push_str(" end");
    w.line(&line);
}

/// `struct Name [<: Super]`, the fields, `end`.
///
/// `struct` and never `mutable struct`: the tree has no arm for it, and
/// the materialiser's documentation carries why.
fn render_struct(w: &mut Writer, node: &StructDecl) {
    doc(w, &node.doc);
    let mut head = String::from("struct ");
    head.push_str(node.name.as_str());
    if let Some(supertype) = &node.supertype {
        head.push_str(" <: ");
        head.push_str(supertype.as_str());
    }
    w.line(&head);
    w.indent();
    for text in &node.body_text.begin {
        verbatim_lines(w, text);
    }
    for field in &node.fields {
        render_field(w, field);
    }
    for text in &node.body_text.end {
        verbatim_lines(w, text);
    }
    w.dedent();
    w.line("end");
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    line.push_str("::");
    line.push_str(&type_text(&node.type_));
    w.line(&line);
}

/// `@enum Name::Base begin … end`.
///
/// The block form and not the one-line one, because a one-line `@enum`
/// with a value per member runs past any width a reader scans, and the
/// two forms are the same macro call.
fn render_enum(w: &mut Writer, node: &EnumDecl) {
    doc(w, &node.doc);
    let mut head = String::from("@enum ");
    head.push_str(node.name.as_str());
    head.push_str("::");
    head.push_str(node.base.as_str());
    head.push_str(" begin");
    w.line(&head);
    w.indent();
    for member in &node.members {
        render_enumerator(w, member);
    }
    w.dedent();
    w.line("end");
}

fn render_enumerator(w: &mut Writer, node: &Enumerator) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

fn render_const(w: &mut Writer, node: &ConstDecl) {
    doc(w, &node.doc);
    let mut line = String::from("const ");
    line.push_str(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

fn render_function(w: &mut Writer, node: &FunctionDecl) {
    doc(w, &node.doc);
    let mut line = String::from("function ");
    line.push_str(node.name.as_str());
    line.push_str(" end");
    w.line(&line);
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/julia.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::Vector(element) => {
            let mut out = String::from("Vector{");
            out.push_str(&type_text(element));
            out.push('}');
            out
        }
        Type::NTuple { length, element } => {
            let mut out = String::from("NTuple{");
            out.push_str(&expr_text(length));
            out.push_str(", ");
            out.push_str(&type_text(element));
            out.push('}');
            out
        }
        Type::Union(arms) => {
            let mut out = String::from("Union{");
            let parts: Vec<String> = arms.iter().map(type_text).collect();
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_julia.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => name.as_str().to_string(),
        Expr::Type(type_) => type_text(type_),
        Expr::Call { func, args } => {
            let mut out = type_text(func);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Binary { op, lhs, rhs } => {
            let mut out = expr_text(lhs);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(&expr_text(rhs));
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::FreeCall { func, args } => {
            let mut out = String::from(func.as_str());
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Field { value, name } => {
            let mut out = expr_text(value);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Expr::Index { value, index } => {
            let mut out = expr_text(value);
            out.push('[');
            out.push_str(&expr_text(index));
            out.push(']');
            out
        }
        // No spaces: `0 : n` parses but reads as three tokens, and a
        // range inside a subscript — `v[1:n]` — is the form every Julia
        // reader knows.
        Expr::Range { start, stop } => {
            let mut out = expr_text(start);
            out.push(':');
            out.push_str(&expr_text(stop));
            out
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
            out
        }
        Expr::AnyVector(items) => {
            let mut out = String::from("Any[");
            let parts: Vec<String> = items.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Int(text) | Literal::Float(text) => text.clone(),
        Literal::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // The code point as a number. A Julia `Char` is a 32-bit code
        // point and an IDL `char` is one octet, so the conversion the
        // member's type asks for is explicit at the declaration rather
        // than implicit in a literal that happens to fit.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&julia_string(text));
            out.push('"');
            out
        }
        Literal::Bytes(bytes) => {
            let mut out = String::from("UInt8[");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02x}")).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Literal::Null => "nothing".to_string(),
    }
}

/// A string literal's body: the C-family escapes, and `$`.
///
/// `$` is Julia's interpolation marker inside `"…"`, and no other column
/// has to escape it.
fn julia_string(text: &str) -> String {
    let mut out = String::new();
    for ch in escape_string(text).chars() {
        if ch == '$' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "# ", doc.as_lines());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;
    use zerodds_lang_cst::julia::Primitive;

    fn field(name: &str, type_: Type) -> Field {
        Field {
            doc: Doc::none(),
            name: Ident::new(name),
            type_,
        }
    }

    #[test]
    fn a_struct_writes_its_fields_with_a_double_colon() {
        let text = decl(&Decl::Struct(StructDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("Pose"),
            supertype: None,
            fields: vec![
                field("x", Type::Primitive(Primitive::Int32)),
                field("y", Type::Primitive(Primitive::Float64)),
            ],
        }));
        assert_eq!(text, "struct Pose\n    x::Int32\n    y::Float64\nend\n");
    }

    #[test]
    fn an_empty_struct_is_still_a_struct() {
        // A singleton type, which is what an IDL aggregate with no
        // members is. No refusal is needed: Julia accepts it.
        let text = decl(&Decl::Struct(StructDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("Empty"),
            supertype: None,
            fields: Vec::new(),
        }));
        assert_eq!(text, "struct Empty\nend\n");
    }

    #[test]
    fn a_struct_with_a_supertype_writes_the_subtype_operator() {
        let text = decl(&Decl::Struct(StructDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("B"),
            supertype: Some(Ident::new("ZdFwd_B")),
            fields: vec![field("x", Type::Primitive(Primitive::Int32))],
        }));
        assert_eq!(text, "struct B <: ZdFwd_B\n    x::Int32\nend\n");
    }

    #[test]
    fn an_abstract_type_is_one_line() {
        let text = decl(&Decl::Abstract(AbstractDecl {
            doc: Doc::none(),
            name: Ident::new("Control"),
        }));
        assert_eq!(text, "abstract type Control end\n");
    }

    #[test]
    fn an_enum_writes_its_base_and_every_value() {
        let text = decl(&Decl::Enum(EnumDecl {
            doc: Doc::none(),
            name: Ident::new("Mode"),
            base: Primitive::Int32,
            members: vec![
                Enumerator {
                    doc: Doc::none(),
                    name: Ident::new("IDLE"),
                    value: Expr::int(0),
                },
                Enumerator {
                    doc: Doc::none(),
                    name: Ident::new("ACTIVE"),
                    value: Expr::int(4),
                },
            ],
        }));
        assert_eq!(
            text,
            "@enum Mode::Int32 begin\n    IDLE = 0\n    ACTIVE = 4\nend\n"
        );
    }

    #[test]
    fn a_const_writes_a_type_as_readily_as_a_value() {
        let alias = decl(&Decl::Const(ConstDecl {
            doc: Doc::none(),
            name: Ident::new("Distance"),
            value: Expr::Type(alloc::boxed::Box::new(Type::Primitive(Primitive::Int32))),
        }));
        assert_eq!(alias, "const Distance = Int32\n");

        let value = decl(&Decl::Const(ConstDecl {
            doc: Doc::none(),
            name: Ident::new("LIMIT"),
            value: Expr::convert(Type::Primitive(Primitive::UInt8), Expr::int(8)),
        }));
        assert_eq!(value, "const LIMIT = UInt8(8)\n");
    }

    #[test]
    fn a_bitmask_constant_is_a_shift_of_a_converted_one() {
        let text = decl(&Decl::Const(ConstDecl {
            doc: Doc::none(),
            name: Ident::new("MOVING"),
            value: Expr::Binary {
                op: "<<",
                lhs: alloc::boxed::Box::new(Expr::convert(Type::named("Flags"), Expr::int(1))),
                rhs: alloc::boxed::Box::new(Expr::int(4)),
            },
        }));
        assert_eq!(text, "const MOVING = Flags(1) << 4\n");
    }

    #[test]
    fn a_function_stub_binds_the_name_and_adds_no_method() {
        let text = decl(&Decl::Function(FunctionDecl {
            doc: Doc::line("go(target::Pose)::Nothing"),
            name: Ident::new("go"),
        }));
        assert_eq!(text, "# go(target::Pose)::Nothing\nfunction go end\n");
    }

    #[test]
    fn a_comment_declaration_writes_only_comment_lines() {
        let text = decl(&Decl::Comment(Doc::lines(&["first", "second"])));
        assert_eq!(text, "# first\n# second\n");
    }

    #[test]
    fn every_collection_writes_its_element_inside_braces() {
        assert_eq!(
            type_(&Type::vector(Type::Primitive(Primitive::Int32))),
            "Vector{Int32}"
        );
        assert_eq!(
            type_(&Type::ntuple(
                2,
                Type::ntuple(3, Type::Primitive(Primitive::Int32))
            )),
            "NTuple{2, NTuple{3, Int32}}"
        );
        assert_eq!(
            type_(&Type::optional(Type::named("Pose"))),
            "Union{Pose, Nothing}"
        );
    }

    #[test]
    fn a_string_literal_escapes_the_interpolation_marker() {
        // `"$HOME"` in Julia is an interpolation, not four characters.
        // A generated constant carrying it would either fail to load with
        // `UndefVarError` or load with whatever the name happens to hold.
        let text = decl(&Decl::Const(ConstDecl {
            doc: Doc::none(),
            name: Ident::new("PROMPT"),
            value: Expr::Literal(Literal::string("$HOME\"x\"")),
        }));
        assert_eq!(text, "const PROMPT = \"\\$HOME\\\"x\\\"\"\n");
    }

    #[test]
    fn the_other_literal_forms_write_what_julia_reads() {
        assert_eq!(expr_text(&Expr::Literal(Literal::Bool(true))), "true");
        assert_eq!(expr_text(&Expr::Literal(Literal::Null)), "nothing");
        assert_eq!(expr_text(&Expr::Literal(Literal::Char('A'))), "65");
        assert_eq!(
            expr_text(&Expr::Literal(Literal::Float("1.5".into()))),
            "1.5"
        );
        assert_eq!(
            expr_text(&Expr::Literal(Literal::Bytes(alloc::vec![1, 255]))),
            "UInt8[0x01, 0xff]"
        );
        assert_eq!(
            expr_text(&Expr::Paren(alloc::boxed::Box::new(Expr::Name(
                Ident::new("x")
            )))),
            "(x)"
        );
    }

    #[test]
    fn a_file_separates_its_declarations_with_one_blank_line() {
        let text = file(&File {
            doc: Doc::line("Generated."),
            decls: vec![
                Decl::Abstract(AbstractDecl {
                    doc: Doc::none(),
                    name: Ident::new("A"),
                }),
                Decl::Abstract(AbstractDecl {
                    doc: Doc::none(),
                    name: Ident::new("B"),
                }),
            ],
        });
        assert_eq!(
            text,
            "# Generated.\n\nabstract type A end\n\nabstract type B end\n"
        );
    }

    #[test]
    fn a_named_identifier_renders_as_itself() {
        assert_eq!(ident(&Ident::new("Pose")), "Pose");
    }

    #[test]
    fn the_codec_expressions_render_what_julia_reads() {
        assert_eq!(
            expr(&Expr::free_call(
                "write_u32",
                vec![Expr::name("w"), Expr::field(Expr::name("v"), "x")]
            )),
            "write_u32(w, v.x)"
        );
        assert_eq!(
            expr(&Expr::index(
                Expr::name("v"),
                Expr::Binary {
                    op: "+",
                    lhs: alloc::boxed::Box::new(Expr::Paren(alloc::boxed::Box::new(Expr::name(
                        "zd_i"
                    )))),
                    rhs: alloc::boxed::Box::new(Expr::int(1)),
                }
            )),
            "v[(zd_i) + 1]"
        );
        assert_eq!(
            expr(&Expr::range(
                Expr::int(0),
                Expr::Paren(alloc::boxed::Box::new(Expr::name("zd_n")))
            )),
            "0:(zd_n)"
        );
        assert_eq!(
            expr(&Expr::Unary {
                op: "!",
                value: alloc::boxed::Box::new(Expr::name("zd_sent")),
            }),
            "!zd_sent"
        );
        assert_eq!(
            expr(&Expr::AnyVector(vec![
                Expr::Literal(Literal::Null),
                Expr::int(2)
            ])),
            "Any[nothing, 2]"
        );
    }

    #[test]
    fn a_function_definition_renders_params_returns_and_body() {
        let text = decl(&Decl::FunctionDef(FunctionDef {
            doc: Doc::none(),
            name: Ident::new("zd_enc_Pose"),
            params: vec![
                zerodds_lang_cst::julia::Param {
                    name: Ident::new("w"),
                    type_: Some(Type::named("ZdWriter")),
                },
                zerodds_lang_cst::julia::Param {
                    name: Ident::new("v"),
                    type_: None,
                },
            ],
            returns: None,
            body: vec![Statement::Expr(Expr::free_call(
                "write_u32",
                vec![Expr::name("w"), Expr::field(Expr::name("v"), "x")],
            ))],
        }));
        assert_eq!(
            text,
            "function zd_enc_Pose(w::ZdWriter, v)\n    write_u32(w, v.x)\nend\n"
        );
    }

    #[test]
    fn the_statement_forms_render_their_julia_shapes() {
        let local = statement(&Statement::Local {
            name: Ident::new("zd_n"),
            type_: Some(Type::Primitive(Primitive::UInt32)),
            value: Some(Expr::free_call("read_u32", vec![Expr::name("r")])),
        });
        assert_eq!(local, "local zd_n::UInt32 = read_u32(r)\n");

        let bare = statement(&Statement::Local {
            name: Ident::new("zd_h"),
            type_: None,
            value: None,
        });
        assert_eq!(bare, "local zd_h\n");

        let chain = statement(&Statement::If {
            condition: Expr::name("a"),
            then: vec![Statement::Break],
            otherwise: Some(vec![Statement::Return(Some(Expr::name("v")))]),
        });
        assert_eq!(chain, "if a\n    break\nelse\n    return v\nend\n");

        let counted = statement(&Statement::For {
            counter: Ident::new("zd_i"),
            iter: Expr::range(Expr::int(0), Expr::int(2)),
            body: vec![Statement::Assign {
                place: Expr::name("x"),
                value: Expr::name("zd_i"),
            }],
        });
        assert_eq!(counted, "for zd_i in 0:2\n    x = zd_i\nend\n");

        let looped = statement(&Statement::While {
            condition: Expr::name("ok"),
            body: vec![Statement::Expr(Expr::name("x"))],
        });
        assert_eq!(looped, "while ok\n    x\nend\n");
    }

    #[test]
    fn a_string_literal_helper_matches_the_literal_form() {
        assert_eq!(string_literal("$HOME\"x\""), "\"\\$HOME\\\"x\\\"\"");
    }

    #[test]
    fn an_empty_if_body_still_renders_a_closed_block() {
        // Julia accepts an empty `if` body, so no `pass` counterpart is
        // needed and none is invented.
        let text = statement(&Statement::If {
            condition: Expr::name("a"),
            then: Vec::new(),
            otherwise: None,
        });
        assert_eq!(text, "if a\nend\n");
    }
}
