// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the F# CST. Spec §5.
//!
//! One style: four spaces, F#'s offside rule doing the indentation's own
//! job, `///` for a documentation comment and `//` for a plain one. F#
//! has no canonical formatter shipped with the compiler the way `zig
//! fmt` is one, so unlike [`crate::zig`] there is no external tool this
//! module's layout is checked against; the generated file's own
//! indentation is the only contract, and it is four spaces throughout
//! because F#'s indentation is significant and a generated file that
//! mixed widths would not parse.
//!
//! Nothing is decided here. An F# type reads left to right and carries
//! its own optionality and dimensionality, so — as in
//! [`crate::zig`] and unlike [`crate::c`] — there
//! is no declarator to split around a name: a property is a name, a
//! colon, and a type.
//!
//! ## Two things F#'s grammar puts here and nowhere else in this arc
//!
//! 1. **`module rec`.** [`file()`] always writes the `rec`; the module
//!    documentation of [`zerodds_lang_cst::fsharp`] says why one word
//!    replaces a forward-declaration pass.
//! 2. **The loop-break exception.** [`Stmt::Until`] opens a `try` around the
//!    `while` it renders and closes it with `with :? ZdWire.LoopBreak ->
//!    ()`; [`Stmt::Leave`] is the
//!    `raise` inside it. Neither statement carries a name for the other
//!    to agree on — the exception type is the coordination, and it is
//!    the runtime's, not this renderer's invention.
//!    [`Stmt::Guarded`] and
//!    [`Stmt::GiveBack`] are
//!    the same pair one `try` further out, for a `return` the shared
//!    walk writes inside a loop body: a filter matches on the
//!    exception's type, so `ZdWire.GiveBack` passes straight through
//!    `with :? ZdWire.LoopBreak -> ()`.

use crate::writer::{Writer, comment_lines, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::fsharp::{
    ClassField, Decl, Expr, File, FnParam, LetDecl, Primitive, Stmt, Type, TypeBody, TypeDecl,
    Variant,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    if !file.doc.is_empty() {
        comment_lines(&mut w, "// ", file.doc.as_lines());
    }
    let mut head = String::from("module rec ");
    head.push_str(file.module.as_str());
    w.line(&head);
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

/// Render one expression, for a caller — a generator's harness — that
/// builds a [`Expr`] tree and needs its text to splice into a larger,
/// hand-assembled program fragment.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Type(decl) => render_type_decl(w, decl),
        Decl::Let(decl) => render_let(w, decl),
        Decl::Comment(lines) => comment_lines(w, "// ", lines.as_lines()),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// `type Name = <alias>` / `type Name() = …` / `type Name = | A = 0 …`.
fn render_type_decl(w: &mut Writer, node: &TypeDecl) {
    doc(w, &node.doc);
    match &node.body {
        TypeBody::Alias(target) => {
            let mut line = String::from("type ");
            line.push_str(node.name.as_str());
            line.push_str(" = ");
            line.push_str(&type_text(target));
            w.line(&line);
        }
        TypeBody::Class(fields) => {
            let mut head = String::from("type ");
            head.push_str(node.name.as_str());
            head.push_str("() =");
            w.line(&head);
            w.indent();
            let text = &node.body_text;
            let has_text = !text.begin.is_empty() || !text.end.is_empty();
            if fields.is_empty() && !has_text {
                // A class with nothing to hold — an empty `struct`, an
                // empty exception. `class end` is F#'s empty class body;
                // written so the declaration has something under it.
                w.line("class end");
            } else if fields.is_empty() {
                // `@verbatim` text and no field: the explicit `class … end`
                // form, which stays a class body whether the text declares
                // members or is nothing but comments.
                w.line("class");
                w.indent();
                for line in text.begin.iter().chain(&text.end) {
                    verbatim_lines(w, line);
                }
                w.dedent();
                w.line("end");
            } else {
                for line in &text.begin {
                    verbatim_lines(w, line);
                }
                for field in fields {
                    render_class_field(w, field);
                }
                for line in &text.end {
                    verbatim_lines(w, line);
                }
            }
            w.dedent();
        }
        TypeBody::Enum { backing, variants } => {
            let mut head = String::from("type ");
            head.push_str(node.name.as_str());
            head.push_str(" =");
            w.line(&head);
            w.indent();
            for variant in variants {
                render_variant(w, backing, variant);
            }
            w.dedent();
        }
    }
}

fn render_class_field(w: &mut Writer, node: &ClassField) {
    doc(w, &node.doc);
    let mut line = String::from("member val ");
    line.push_str(node.name.as_str());
    line.push_str(" : ");
    line.push_str(&type_text(&node.type_));
    line.push_str(" = ");
    line.push_str(&expr_text(&node.default));
    line.push_str(" with get, set");
    w.line(&line);
}

/// `| Name = value` — the backing type is written once, on the
/// declaration line, and not repeated per case.
fn render_variant(w: &mut Writer, backing: &Type, node: &Variant) {
    let _ = backing;
    doc(w, &node.doc);
    let mut line = String::from("| ");
    line.push_str(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

/// `let name (p: T) : R = …` or `let name : T = value` for a constant.
fn render_let(w: &mut Writer, node: &LetDecl) {
    doc(w, &node.doc);
    let mut head = String::from("let ");
    head.push_str(node.name.as_str());
    if node.params.is_empty() {
        if let Some(type_) = &node.type_ {
            head.push_str(" : ");
            head.push_str(&type_text(type_));
        }
    } else {
        for param in &node.params {
            head.push_str(" (");
            head.push_str(param_text(param).as_str());
            head.push(')');
        }
        if let Some(type_) = &node.type_ {
            head.push_str(" : ");
            head.push_str(&type_text(type_));
        }
    }
    head.push_str(" =");
    // A one-statement body that is a bare trailing expression fits the
    // declaration line; every other shape opens a block underneath.
    if let [Stmt::Return(value)] = node.body.as_slice() {
        head.push(' ');
        head.push_str(&expr_text(value));
        w.line(&head);
        return;
    }
    w.line(&head);
    w.indent();
    for statement in &node.body {
        render_stmt(w, statement);
    }
    w.dedent();
}

fn param_text(param: &FnParam) -> String {
    let mut out = String::from(param.name.as_str());
    out.push_str(": ");
    out.push_str(&type_text(&param.type_));
    out
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::Let {
            name,
            type_,
            value,
            mutable,
        } => {
            let mut line = String::from(if *mutable { "let mutable " } else { "let " });
            line.push_str(name.as_str());
            if let Some(type_) = type_ {
                line.push_str(" : ");
                line.push_str(&type_text(type_));
            }
            line.push_str(" = ");
            line.push_str(&expr_text(value));
            w.line(&line);
        }
        Stmt::Assign { place, value } => {
            let mut line = expr_text(place);
            line.push_str(" <- ");
            line.push_str(&expr_text(value));
            w.line(&line);
        }
        Stmt::Expr(value) => w.line(&expr_text(value)),
        Stmt::If { .. } => render_if(w, node, "if "),
        Stmt::CountUp {
            counter,
            type_,
            bound,
            body,
        } => {
            // `for i = 0 to bound - 1 do` — F#'s counted loop needs no
            // declared type on the counter; it is inferred from `zero_of`.
            let mut start = String::from("for ");
            start.push_str(counter.as_str());
            start.push_str(" = ");
            start.push_str(&zero_of(type_));
            start.push_str(" to (");
            start.push_str(&expr_text(bound));
            start.push_str(") - ");
            start.push_str(&one_of(type_));
            start.push_str(" do");
            w.line(&start);
            w.indent();
            for statement in body {
                render_stmt(w, statement);
            }
            w.dedent();
        }
        Stmt::Until { condition, body } => {
            w.line("try");
            w.indent();
            let mut head = String::from("while ");
            head.push_str(&expr_text(condition));
            head.push_str(" do");
            w.line(&head);
            w.indent();
            for statement in body {
                render_stmt(w, statement);
            }
            w.dedent();
            w.dedent();
            w.line("with");
            w.indent();
            w.line(":? ZdWire.LoopBreak -> ()");
            w.dedent();
        }
        Stmt::Leave => w.line("raise (ZdWire.LoopBreak())"),
        Stmt::GiveBack => w.line("raise (ZdWire.GiveBack())"),
        Stmt::Guarded { body } => {
            w.line("try");
            w.indent();
            for statement in body {
                render_stmt(w, statement);
            }
            w.dedent();
            w.line("with");
            w.indent();
            w.line(":? ZdWire.GiveBack -> ()");
            w.dedent();
        }
        Stmt::Comment(lines) => comment_lines(w, "// ", lines.as_lines()),
        Stmt::Return(value) => w.line(&expr_text(value)),
    }
}

/// The counted loop's zero, in the counter's own type — F#'s `for … =
/// 0 to …` needs the bound written in the same numeric type as the
/// start, and this walk's counters are `uint32`.
fn zero_of(type_: &Type) -> String {
    if *type_ == Type::Primitive(Primitive::UInt32) {
        "0u".to_string()
    } else {
        "0".to_string()
    }
}

fn one_of(type_: &Type) -> String {
    if *type_ == Type::Primitive(Primitive::UInt32) {
        "1u".to_string()
    } else {
        "1".to_string()
    }
}

/// An `if`, with an `else`-block of exactly one `if` collapsed into
/// `elif` — the shape a long dispatch chain stays readable in.
///
/// The next arm is taken in a loop and not by a call to this function
/// again. The chain's length is the union's case count, the
/// enumeration's enumerator count or the `@mutable` aggregate's member
/// count — `lang-rules/src/emit/encode.rs` builds one `If` per arm, each
/// in the previous one's `else` — and nothing in the model bounds any of
/// the three.
///
/// zerodds-lint: recursion-depth 64 (block nesting only; the arm chain
/// is walked iteratively, and what remains is one level per nested block
/// of a generated body — the codec walk emits a routine per type rather
/// than inlining, so a body nests by its framing and by one loop per
/// array dimension, never by the type graph)
fn render_if(w: &mut Writer, node: &Stmt, keyword: &str) {
    let mut node = node;
    let mut keyword = keyword;
    loop {
        let Stmt::If {
            condition,
            then,
            otherwise,
        } = node
        else {
            return;
        };
        let mut head = String::from(keyword);
        head.push_str(&expr_text(condition));
        head.push_str(" then");
        w.line(&head);
        w.indent();
        if then.is_empty() {
            w.line("()");
        }
        for statement in then {
            render_stmt(w, statement);
        }
        w.dedent();
        match otherwise {
            None => return,
            Some(chain) if chain.len() == 1 && matches!(chain[0], Stmt::If { .. }) => {
                node = &chain[0];
                keyword = "elif ";
            }
            Some(block) => {
                w.line("else");
                w.indent();
                if block.is_empty() {
                    w.line("()");
                }
                for statement in block {
                    render_stmt(w, statement);
                }
                w.dedent();
                return;
            }
        }
    }
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/fsharp.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::Array(element) => {
            let mut out = type_text(element);
            out.push_str(" []");
            out
        }
        Type::Option(inner) => {
            let mut out = type_text(inner);
            out.push_str(" option");
            out
        }
        Type::Fn { params, returns } => {
            if params.is_empty() {
                let mut out = String::from("unit -> ");
                out.push_str(&type_text(returns));
                return out;
            }
            let parts: Vec<String> = params.iter().map(type_text).collect();
            let mut out = parts.join(" -> ");
            out.push_str(" -> ");
            out.push_str(&type_text(returns));
            out
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_fsharp.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => name.as_str().to_string(),
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
        Expr::Call { func, args } => {
            let mut out = expr_text(func);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::CurriedCall { func, args } => {
            let mut out = expr_text(func);
            if args.is_empty() {
                // `f ()` — the unit argument, applied. A bare `f` here
                // would name the function value and never invoke it:
                // `let x = f` binds `x` to a `unit -> T`, not to the
                // `T` calling it would answer.
                out.push_str(" ()");
                return out;
            }
            for arg in args {
                out.push_str(" (");
                out.push_str(&expr_text(arg));
                out.push(')');
            }
            out
        }
        Expr::Dot { value, name } => {
            let mut out = expr_text(value);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Expr::Index { value, index } => {
            let mut out = expr_text(value);
            out.push_str(".[");
            out.push_str(&expr_text(index));
            out.push(']');
            out
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
            out
        }
        Expr::Some_(inner) => {
            let mut out = String::from("Some (");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::None_ => "None".to_string(),
        Expr::New { type_, args } => {
            let mut out = type_.as_str().to_string();
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::ObjectInit { type_, fields } => {
            let mut out = type_.as_str().to_string();
            out.push('(');
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| {
                    let mut part = name.as_str().to_string();
                    part.push_str(" = ");
                    part.push_str(&expr_text(value));
                    part
                })
                .collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::ArrayLit(elements) => {
            if elements.is_empty() {
                return "[||]".to_string();
            }
            let mut out = String::from("[| ");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join("; "));
            out.push_str(" |]");
            out
        }
        Expr::ArrayZeroCreate { element, count } => {
            let mut out = String::from("Array.zeroCreate<");
            out.push_str(&type_text(element));
            out.push_str("> (");
            out.push_str(&expr_text(count));
            out.push(')');
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
        Literal::Char(ch) => {
            // .NET's `char` is a UTF-16 code unit; this column never
            // declares one, so a character literal only ever appears as
            // an `int` code point.
            alloc::format!("{}", *ch as u32)
        }
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&string_text(text));
            out.push('"');
            out
        }
        Literal::Bytes(bytes) => {
            let mut out = String::from("[| ");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("{b}uy")).collect();
            out.push_str(&parts.join("; "));
            out.push_str(" |]");
            out
        }
        Literal::Null => "None".to_string(),
    }
}

/// Escape a string for an F# string literal.
///
/// F#'s escapes agree with the shared [`crate::writer::escape_string`]
/// on every one it writes (`\"`, `\\`, `\n`, `\r`, `\t`, `\0`), so this
/// module has no escaping of its own the way [`crate::zig`]'s does.
#[must_use]
pub fn string_text(text: &str) -> String {
    crate::writer::escape_string(text)
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "/// ", doc.as_lines());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::boxed::Box;
    use zerodds_lang_cst::fsharp::Primitive;

    fn class_decl(fields: Vec<ClassField>) -> Decl {
        Decl::Type(TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("Pose"),
            body: TypeBody::Class(fields),
        })
    }

    fn field(name: &str, type_: Type, default: Expr) -> ClassField {
        ClassField {
            doc: Doc::none(),
            name: Ident::new(name),
            type_,
            default,
        }
    }

    #[test]
    fn a_class_declaration_indents_by_four_and_writes_member_val() {
        let text = decl(&class_decl(alloc::vec![
            field("X", Type::Primitive(Primitive::Int32), Expr::int(0)),
            field(
                "Y",
                Type::Primitive(Primitive::Double),
                Expr::Literal(Literal::Float("0.0".to_string())),
            ),
        ]));
        assert_eq!(
            text,
            "type Pose() =\n    member val X : int32 = 0 with get, set\n    \
             member val Y : float = 0.0 with get, set\n"
        );
    }

    #[test]
    fn an_empty_class_writes_class_end() {
        assert_eq!(
            decl(&class_decl(Vec::new())),
            "type Pose() =\n    class end\n"
        );
    }

    #[test]
    fn a_type_reads_left_to_right_with_no_declarator_to_split() {
        assert_eq!(
            type_(&Type::array(Type::array(Type::Primitive(Primitive::Int32)))),
            "int32 [] []"
        );
        assert_eq!(type_(&Type::text()), "string");
        assert_eq!(type_(&Type::array(Type::named("Entry"))), "Entry []");
        assert_eq!(
            type_(&Type::option(Type::Primitive(Primitive::Byte))),
            "byte option"
        );
    }

    #[test]
    fn an_enum_writes_its_backing_type_once_and_every_value() {
        let text = decl(&Decl::Type(TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("Mode"),
            body: TypeBody::Enum {
                backing: Type::Primitive(Primitive::Int32),
                variants: alloc::vec![
                    Variant {
                        doc: Doc::none(),
                        name: Ident::new("IDLE"),
                        value: Expr::int(0),
                    },
                    Variant {
                        doc: Doc::none(),
                        name: Ident::new("ACTIVE"),
                        value: Expr::int(4),
                    },
                ],
            },
        }));
        assert_eq!(text, "type Mode =\n    | IDLE = 0\n    | ACTIVE = 4\n");
    }

    #[test]
    fn a_constant_writes_its_ascription_before_its_value() {
        let text = decl(&Decl::Let(LetDecl {
            doc: Doc::none(),
            name: Ident::new("LIMIT"),
            params: Vec::new(),
            type_: Some(Type::Primitive(Primitive::Byte)),
            body: alloc::vec![Stmt::Return(Expr::int(8))],
        }));
        assert_eq!(text, "let LIMIT : byte = 8\n");
    }

    #[test]
    fn a_function_writes_its_parameters_curried() {
        let text = decl(&Decl::Let(LetDecl {
            doc: Doc::none(),
            name: Ident::new("zd_enc_Pose"),
            params: alloc::vec![
                FnParam {
                    name: Ident::new("w"),
                    type_: Type::named("ZdWire.Writer"),
                },
                FnParam {
                    name: Ident::new("v"),
                    type_: Type::named("Pose"),
                },
            ],
            type_: Some(Type::Primitive(Primitive::Unit)),
            body: alloc::vec![Stmt::Return(Expr::Literal(Literal::Bool(true)))],
        }));
        assert_eq!(
            text,
            "let zd_enc_Pose (w: ZdWire.Writer) (v: Pose) : unit = true\n"
        );
    }

    #[test]
    fn the_module_line_always_carries_rec() {
        let text = file(&File {
            doc: Doc::none(),
            module: Ident::new("ZdTypes"),
            decls: Vec::new(),
        });
        assert!(text.starts_with("module rec ZdTypes\n"), "{text}");
    }

    #[test]
    fn a_loop_break_renders_as_a_try_with_and_a_raise() {
        let text = decl(&Decl::Let(LetDecl {
            doc: Doc::none(),
            name: Ident::new("f"),
            params: Vec::new(),
            type_: Some(Type::Primitive(Primitive::Unit)),
            body: alloc::vec![
                Stmt::Until {
                    condition: Expr::Literal(Literal::Bool(true)),
                    body: alloc::vec![Stmt::Leave],
                },
                Stmt::Return(Expr::Literal(Literal::Bool(true))),
            ],
        }));
        assert!(text.contains("try\n"), "{text}");
        assert!(text.contains("while true do\n"), "{text}");
        assert!(text.contains("raise (ZdWire.LoopBreak())\n"), "{text}");
        assert!(text.contains(":? ZdWire.LoopBreak -> ()\n"), "{text}");
    }

    #[test]
    fn a_counted_loop_writes_fs_for_syntax() {
        let text = decl(&Decl::Let(LetDecl {
            doc: Doc::none(),
            name: Ident::new("f"),
            params: Vec::new(),
            type_: Some(Type::Primitive(Primitive::Unit)),
            body: alloc::vec![
                Stmt::CountUp {
                    counter: Ident::new("i"),
                    type_: Box::new(Type::Primitive(Primitive::UInt32)),
                    bound: Box::new(Expr::Name(Ident::new("n"))),
                    body: alloc::vec![Stmt::Expr(Expr::Name(Ident::new("i")))],
                },
                Stmt::Return(Expr::Literal(Literal::Bool(true))),
            ],
        }));
        assert!(text.contains("for i = 0u to (n) - 1u do\n"), "{text}");
    }

    #[test]
    fn a_doc_comment_sits_above_the_field_it_belongs_to() {
        let mut documented = field("type_", Type::Primitive(Primitive::Int32), Expr::int(0));
        documented.doc = Doc::line("IDL: `type`.");
        let text = decl(&class_decl(alloc::vec![documented]));
        assert_eq!(
            text,
            "type Pose() =\n    /// IDL: `type`.\n    member val type_ : int32 = 0 with get, set\n"
        );
    }
}
