// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Swift CST. Spec §5.
//!
//! One style, the one `swift-format` produces with its defaults: four
//! spaces for indentation, the opening brace on the same line, `///` for
//! documentation and `//` for a plain comment.
//!
//! Nothing is decided here. A Swift type reads name-first with the type
//! after a colon and carries its own nesting, so unlike
//! [`crate::c`] there is no declarator to split around a name:
//! a property is `var`, the name, a colon and the type.
//!
//! Two spellings this renderer chooses and the reason for each:
//!
//! - [`Visibility::Internal`] writes **nothing**. `internal` is Swift's
//!   default access level, so the keyword adds a word to every line and
//!   changes no program.
//! - [`Aggregate::Class`] writes `final class`. Nothing generated is
//!   subclassed, and a non-final public class is an extension point the
//!   IDL did not declare.

use crate::writer::{Writer, comment_lines, escape_string, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Visibility};
use zerodds_lang_cst::swift::{
    Aggregate, AggregateDecl, Argument, ConstDecl, Decl, EnumCase, EnumDecl, Expr, File, FuncDecl,
    Init, Method, Param, Property, ProtocolDecl, StaticLet, Stmt, Type, TypeAliasDecl,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    if !file.doc.is_empty() {
        comment_lines(&mut w, "// ", file.doc.as_lines());
    }
    if !file.imports.is_empty() {
        w.blank_line();
        for import in &file.imports {
            let mut line = String::from("import ");
            line.push_str(import.as_str());
            w.line(&line);
        }
    }
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

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Aggregate(decl) => render_aggregate(w, decl),
        Decl::Enum(decl) => render_enum(w, decl),
        Decl::TypeAlias(decl) => render_type_alias(w, decl),
        Decl::Constant(decl) => render_const(w, decl),
        Decl::Protocol(decl) => render_protocol(w, decl),
        Decl::Func(decl) => render_func(w, decl),
        Decl::Comment(lines) => doc(w, lines),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_func(w: &mut Writer, node: &FuncDecl) {
    doc(w, &node.doc);
    let mut head = visibility_text(node.visibility);
    head.push_str("func ");
    head.push_str(node.name.as_str());
    head.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    head.push_str(&params.join(", "));
    head.push(')');
    if node.throws {
        head.push_str(" throws");
    }
    if let Some(returns) = &node.returns {
        head.push_str(" -> ");
        head.push_str(&type_text(returns));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for stmt in &node.body {
        render_stmt(w, stmt);
    }
    w.dedent();
    w.line("}");
}

fn render_aggregate(w: &mut Writer, node: &AggregateDecl) {
    doc(w, &node.doc);
    let mut head = visibility_text(node.visibility);
    head.push_str(match node.kind {
        Aggregate::Struct => "struct ",
        Aggregate::Class => "final class ",
    });
    head.push_str(node.name.as_str());
    if !node.generics.is_empty() {
        head.push('<');
        let parts: Vec<&str> = node.generics.iter().map(Ident::as_str).collect();
        head.push_str(&parts.join(", "));
        head.push('>');
    }
    if !node.conformances.is_empty() {
        head.push_str(": ");
        let parts: Vec<&str> = node.conformances.iter().map(Ident::as_str).collect();
        head.push_str(&parts.join(", "));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for text in &node.body_text.begin {
        verbatim_lines(w, text);
    }
    for property in &node.properties {
        render_property(w, property);
    }
    for static_ in &node.statics {
        render_static(w, static_);
    }
    for init in &node.inits {
        render_init(w, init);
    }
    for text in &node.body_text.end {
        verbatim_lines(w, text);
    }
    w.dedent();
    w.line("}");
}

fn render_property(w: &mut Writer, node: &Property) {
    doc(w, &node.doc);
    let mut line = visibility_text(node.visibility);
    line.push_str(if node.mutable { "var " } else { "let " });
    line.push_str(node.name.as_str());
    line.push_str(": ");
    line.push_str(&type_text(&node.type_));
    w.line(&line);
}

fn render_static(w: &mut Writer, node: &StaticLet) {
    doc(w, &node.doc);
    let mut line = visibility_text(node.visibility);
    line.push_str("static let ");
    line.push_str(node.name.as_str());
    if let Some(type_) = &node.type_ {
        line.push_str(": ");
        line.push_str(&type_text(type_));
    }
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

/// An initialiser.
///
/// An empty body renders as `init() {}` on one line, which is what
/// `swift-format` produces for a body with no statements.
fn render_init(w: &mut Writer, node: &Init) {
    doc(w, &node.doc);
    let mut head = visibility_text(node.visibility);
    head.push_str("init(");
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    head.push_str(&params.join(", "));
    head.push(')');
    if node.body.is_empty() {
        head.push_str(" {}");
        w.line(&head);
        return;
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for stmt in &node.body {
        render_stmt(w, stmt);
    }
    w.dedent();
    w.line("}");
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::AssignProperty { property, value } => {
            let mut line = String::from("self.");
            line.push_str(property.as_str());
            line.push_str(" = ");
            line.push_str(&expr_text(value));
            w.line(&line);
        }
        Stmt::Var { name, type_, value } => {
            let mut line = String::from("var ");
            line.push_str(name.as_str());
            if let Some(type_) = type_ {
                line.push_str(": ");
                line.push_str(&type_text(type_));
            }
            if let Some(value) = value {
                line.push_str(" = ");
                line.push_str(&expr_text(value));
            }
            w.line(&line);
        }
        Stmt::Assign { place, value } => {
            let mut line = expr_text(place);
            line.push_str(" = ");
            line.push_str(&expr_text(value));
            w.line(&line);
        }
        Stmt::Expr(value) => w.line(&expr_text(value)),
        Stmt::Return(value) => match value {
            Some(value) => {
                let mut line = String::from("return ");
                line.push_str(&expr_text(value));
                w.line(&line);
            }
            None => w.line("return"),
        },
        Stmt::If {
            condition,
            then,
            otherwise,
        } => render_if(w, condition, then, otherwise.as_deref()),
        Stmt::ForRange {
            counter,
            bound,
            body,
        } => {
            let mut head = String::from("for ");
            head.push_str(counter.as_str());
            head.push_str(" in 0..<");
            head.push_str(&tight_operand(bound));
            head.push_str(" {");
            w.line(&head);
            w.indent();
            for stmt in body {
                render_stmt(w, stmt);
            }
            w.dedent();
            w.line("}");
        }
        Stmt::While { condition, body } => {
            let mut head = String::from("while ");
            head.push_str(&expr_text(condition));
            head.push_str(" {");
            w.line(&head);
            w.indent();
            for stmt in body {
                render_stmt(w, stmt);
            }
            w.dedent();
            w.line("}");
        }
        Stmt::Break => w.line("break"),
        Stmt::Comment(lines) => {
            if !lines.is_empty() {
                comment_lines(w, "// ", lines.as_lines());
            }
        }
    }
}

/// An `if`, with an `else` whose whole body is one more `if` written
/// as `} else if ... {` — the shape a union's arm chain reads best in,
/// and what keeps a long chain from indenting off the page.
fn render_if(w: &mut Writer, condition: &Expr, then: &[Stmt], otherwise: Option<&[Stmt]>) {
    let mut head = String::from("if ");
    head.push_str(&expr_text(condition));
    head.push_str(" {");
    w.line(&head);
    render_if_tail(w, then, otherwise);
}

/// The next arm is taken in a loop and not by a call to this function
/// again. The depth this used to claim — 256 — was nothing's: the
/// chain's length is the union's case count, the enumeration's
/// enumerator count or the `@mutable` aggregate's member count, the walk
/// in `lang-rules/src/emit/encode.rs` builds one `If` per arm in the
/// previous one's `else`, and nothing in the model bounds any of the
/// three.
///
/// zerodds-lint: recursion-depth 64 (block nesting only; the arm chain
/// is walked iteratively, and what remains is one level per nested block
/// of a generated body — the codec walk emits a routine per type rather
/// than inlining, so a body nests by its framing and by one loop per
/// array dimension, never by the type graph)
fn render_if_tail(w: &mut Writer, then: &[Stmt], otherwise: Option<&[Stmt]>) {
    let mut then = then;
    let mut otherwise = otherwise;
    loop {
        w.indent();
        for stmt in then {
            render_stmt(w, stmt);
        }
        w.dedent();
        let Some(branch) = otherwise else {
            w.line("}");
            return;
        };
        if let [
            Stmt::If {
                condition: next_condition,
                then: next_then,
                otherwise: next_otherwise,
            },
        ] = branch
        {
            let mut head = String::from("} else if ");
            head.push_str(&expr_text(next_condition));
            head.push_str(" {");
            w.line(&head);
            then = next_then;
            otherwise = next_otherwise.as_deref();
            continue;
        }
        w.line("} else {");
        w.indent();
        for stmt in branch {
            render_stmt(w, stmt);
        }
        w.dedent();
        w.line("}");
        return;
    }
}

/// A bound as a `0..<` operand: a binary expression, or a `try`
/// carried by one, is bracketed so the range operator does not bind
/// into it.
fn tight_operand(bound: &Expr) -> String {
    match bound {
        Expr::Binary { .. } | Expr::Try(_) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(bound));
            out.push(')');
            out
        }
        other => expr_text(other),
    }
}

fn param_text(param: &Param) -> String {
    let mut out = String::from(param.label.as_str());
    if let Some(internal) = &param.internal_name {
        out.push(' ');
        out.push_str(internal.as_str());
    }
    out.push_str(": ");
    out.push_str(&type_text(&param.type_));
    out
}

fn render_enum(w: &mut Writer, node: &EnumDecl) {
    doc(w, &node.doc);
    let mut head = visibility_text(node.visibility);
    head.push_str("enum ");
    head.push_str(node.name.as_str());
    if let Some(raw) = &node.raw_type {
        head.push_str(": ");
        head.push_str(&type_text(raw));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for case in &node.cases {
        render_case(w, case);
    }
    w.dedent();
    w.line("}");
}

fn render_case(w: &mut Writer, node: &EnumCase) {
    doc(w, &node.doc);
    let mut line = String::from("case ");
    line.push_str(node.name.as_str());
    if let Some(value) = &node.raw_value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    w.line(&line);
}

fn render_type_alias(w: &mut Writer, node: &TypeAliasDecl) {
    doc(w, &node.doc);
    let mut line = visibility_text(node.visibility);
    line.push_str("typealias ");
    line.push_str(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&type_text(&node.type_));
    w.line(&line);
}

fn render_const(w: &mut Writer, node: &ConstDecl) {
    doc(w, &node.doc);
    let mut line = visibility_text(node.visibility);
    line.push_str("let ");
    line.push_str(node.name.as_str());
    if let Some(type_) = &node.type_ {
        line.push_str(": ");
        line.push_str(&type_text(type_));
    }
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

fn render_protocol(w: &mut Writer, node: &ProtocolDecl) {
    doc(w, &node.doc);
    let mut head = visibility_text(node.visibility);
    head.push_str("protocol ");
    head.push_str(node.name.as_str());
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for method in &node.methods {
        render_method(w, method);
    }
    w.dedent();
    w.line("}");
}

fn render_method(w: &mut Writer, node: &Method) {
    doc(w, &node.doc);
    let mut line = String::from("func ");
    line.push_str(node.name.as_str());
    line.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    line.push_str(&params.join(", "));
    line.push(')');
    if node.throws {
        line.push_str(" throws");
    }
    if let Some(returns) = &node.returns {
        line.push_str(" -> ");
        line.push_str(&type_text(returns));
    }
    w.line(&line);
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/swift.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::Generic { base, arguments } => {
            let mut out = String::from(base.as_str());
            out.push('<');
            let parts: Vec<String> = arguments.iter().map(type_text).collect();
            out.push_str(&parts.join(", "));
            out.push('>');
            out
        }
        Type::Array(element) => {
            let mut out = String::from("[");
            out.push_str(&type_text(element));
            out.push(']');
            out
        }
        Type::Optional(inner) => {
            let mut out = type_text(inner);
            out.push('?');
            out
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_swift.rs`, and
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
        Expr::Call { callee, args } => {
            let mut out = String::from(callee.as_str());
            out.push('(');
            let parts: Vec<String> = args.iter().map(argument_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::Member { value, name } => {
            let mut out = expr_text(value);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Expr::Invoke {
            receiver,
            name,
            args,
        } => {
            let mut out = match receiver {
                Some(receiver) => {
                    let mut head = expr_text(receiver);
                    head.push('.');
                    head
                }
                None => String::new(),
            };
            out.push_str(name.as_str());
            out.push('(');
            let parts: Vec<String> = args.iter().map(argument_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Index { value, index } => {
            let mut out = expr_text(value);
            out.push('[');
            out.push_str(&expr_text(index));
            out.push(']');
            out
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
            out
        }
        Expr::ForceUnwrap(value) => {
            let mut out = expr_text(value);
            out.push('!');
            out
        }
        Expr::ArrayLit(elements) => {
            let mut out = String::from("[");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Expr::RepeatingArray {
            element,
            repeated,
            count,
        } => {
            let mut out = String::from("[");
            out.push_str(&type_text(element));
            out.push_str("](repeating: ");
            out.push_str(&expr_text(repeated));
            out.push_str(", count: ");
            out.push_str(&expr_text(count));
            out.push(')');
            out
        }
        Expr::TypeSelf(type_) => {
            let mut out = type_text(type_);
            out.push_str(".self");
            out
        }
        // `try` at the very front of what it wraps — the CST only ever
        // builds one of these around the outermost expression a
        // throwing call reached, never around a bare operand, so no
        // parenthesising is needed here for `try` to bind correctly.
        Expr::Try(inner) => {
            let mut out = String::from("try ");
            out.push_str(&expr_text(inner));
            out
        }
    }
}

fn argument_text(arg: &Argument) -> String {
    let mut out = String::new();
    if let Some(label) = &arg.label {
        out.push_str(label.as_str());
        out.push_str(": ");
    }
    out.push_str(&expr_text(&arg.value));
    out
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
        // The code point as a number. A Swift `Character` is an extended
        // grapheme cluster and an IDL `char` is one octet, so a quoted
        // literal would not be the same value; the member's own type —
        // `UInt8` for `char`, `UInt16` for `wchar` — is what the number
        // is written into.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
        Literal::Bytes(bytes) => {
            let mut out = String::from("[");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Literal::Null => "nil".to_string(),
    }
}

/// The access-level keyword, with a trailing space where there is one.
///
/// `internal` writes nothing: it is Swift's default access level, so the
/// keyword would be a word on every line that changes no program.
fn visibility_text(visibility: Visibility) -> String {
    match visibility {
        Visibility::Public => "public ".to_string(),
        Visibility::Internal => String::new(),
        Visibility::Private => "private ".to_string(),
    }
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
    use zerodds_lang_cst::swift::Primitive;

    fn int32() -> Type {
        Type::Primitive(Primitive::Int32)
    }

    #[test]
    fn a_struct_writes_its_properties_and_its_public_initialiser() {
        let decl = Decl::Aggregate(AggregateDecl {
            doc: Doc::none(),
            visibility: Visibility::Public,
            kind: Aggregate::Struct,
            name: Ident::new("Pose"),
            generics: Vec::new(),
            conformances: Vec::new(),
            properties: alloc::vec![Property {
                doc: Doc::none(),
                visibility: Visibility::Public,
                mutable: true,
                name: Ident::new("x"),
                type_: int32(),
            }],
            statics: Vec::new(),
            body_text: zerodds_lang_cst::common::BodyText::none(),
            inits: alloc::vec![Init {
                doc: Doc::none(),
                visibility: Visibility::Public,
                params: alloc::vec![Param {
                    label: Ident::new("x"),
                    internal_name: None,
                    type_: int32(),
                }],
                body: alloc::vec![Stmt::AssignProperty {
                    property: Ident::new("x"),
                    value: Expr::Name(Ident::new("x")),
                }],
            }],
        });
        assert_eq!(
            super::decl(&decl),
            "public struct Pose {\n    public var x: Int32\n    public init(x: Int32) {\n        \
             self.x = x\n    }\n}\n"
        );
    }

    #[test]
    fn an_empty_initialiser_stays_on_one_line() {
        let init = Init {
            doc: Doc::none(),
            visibility: Visibility::Public,
            params: Vec::new(),
            body: Vec::new(),
        };
        let mut w = Writer::spaces4();
        render_init(&mut w, &init);
        assert_eq!(w.finish(), "public init() {}\n");
    }

    #[test]
    fn a_parameter_with_a_second_name_writes_both() {
        let param = Param {
            label: Ident::new("`self`"),
            internal_name: Some(Ident::new("zdSelf")),
            type_: int32(),
        };
        assert_eq!(param_text(&param), "`self` zdSelf: Int32");
    }

    #[test]
    fn a_class_is_written_final() {
        let decl = Decl::Aggregate(AggregateDecl {
            doc: Doc::none(),
            visibility: Visibility::Public,
            kind: Aggregate::Class,
            name: Ident::new("ZdExternal"),
            generics: alloc::vec![Ident::new("Value")],
            conformances: Vec::new(),
            properties: Vec::new(),
            statics: Vec::new(),
            body_text: zerodds_lang_cst::common::BodyText::none(),
            inits: Vec::new(),
        });
        assert_eq!(
            super::decl(&decl),
            "public final class ZdExternal<Value> {\n}\n"
        );
    }

    #[test]
    fn an_option_set_writes_its_conformance_and_its_flags() {
        let decl = Decl::Aggregate(AggregateDecl {
            doc: Doc::none(),
            visibility: Visibility::Public,
            kind: Aggregate::Struct,
            name: Ident::new("Flags"),
            generics: Vec::new(),
            conformances: alloc::vec![Ident::new("OptionSet")],
            properties: Vec::new(),
            statics: alloc::vec![StaticLet {
                doc: Doc::none(),
                visibility: Visibility::Public,
                name: Ident::new("READY"),
                type_: None,
                value: Expr::Call {
                    callee: Ident::new("Flags"),
                    args: alloc::vec![Argument {
                        label: Some(Ident::new("rawValue")),
                        value: Expr::Binary {
                            op: "<<",
                            lhs: alloc::boxed::Box::new(Expr::int(1)),
                            rhs: alloc::boxed::Box::new(Expr::int(4)),
                        },
                    }],
                },
            }],
            body_text: zerodds_lang_cst::common::BodyText::none(),
            inits: Vec::new(),
        });
        assert_eq!(
            super::decl(&decl),
            "public struct Flags: OptionSet {\n    public static let READY = \
             Flags(rawValue: 1 << 4)\n}\n"
        );
    }

    #[test]
    fn an_enum_writes_every_raw_value_out() {
        let decl = Decl::Enum(EnumDecl {
            doc: Doc::none(),
            visibility: Visibility::Public,
            name: Ident::new("Mode"),
            raw_type: Some(int32()),
            cases: alloc::vec![
                EnumCase {
                    doc: Doc::none(),
                    name: Ident::new("IDLE"),
                    raw_value: Some(Expr::int(0)),
                },
                EnumCase {
                    doc: Doc::none(),
                    name: Ident::new("ACTIVE"),
                    raw_value: Some(Expr::int(4)),
                },
            ],
        });
        assert_eq!(
            super::decl(&decl),
            "public enum Mode: Int32 {\n    case IDLE = 0\n    case ACTIVE = 4\n}\n"
        );
    }

    #[test]
    fn an_enum_with_no_cases_writes_no_raw_type() {
        let decl = Decl::Enum(EnumDecl {
            doc: Doc::none(),
            visibility: Visibility::Public,
            name: Ident::new("Empty"),
            raw_type: None,
            cases: Vec::new(),
        });
        assert_eq!(super::decl(&decl), "public enum Empty {\n}\n");
    }

    #[test]
    fn a_protocol_writes_its_requirements() {
        let decl = Decl::Protocol(ProtocolDecl {
            doc: Doc::none(),
            visibility: Visibility::Public,
            name: Ident::new("Control"),
            methods: alloc::vec![
                Method {
                    doc: Doc::none(),
                    name: Ident::new("go"),
                    params: alloc::vec![Param {
                        label: Ident::new("target"),
                        internal_name: None,
                        type_: Type::named("Pose"),
                    }],
                    throws: false,
                    returns: None,
                },
                Method {
                    doc: Doc::none(),
                    name: Ident::new("status"),
                    params: Vec::new(),
                    throws: false,
                    returns: Some(int32()),
                },
            ],
        });
        assert_eq!(
            super::decl(&decl),
            "public protocol Control {\n    func go(target: Pose)\n    func status() -> Int32\n}\n"
        );
    }

    #[test]
    fn a_typealias_and_a_constant_each_write_one_line() {
        assert_eq!(
            super::decl(&Decl::TypeAlias(TypeAliasDecl {
                doc: Doc::none(),
                visibility: Visibility::Public,
                name: Ident::new("Distance"),
                type_: int32(),
            })),
            "public typealias Distance = Int32\n"
        );
        assert_eq!(
            super::decl(&Decl::Constant(ConstDecl {
                doc: Doc::none(),
                visibility: Visibility::Public,
                name: Ident::new("LIMIT"),
                type_: Some(Type::Primitive(Primitive::UInt8)),
                value: Expr::int(8),
            })),
            "public let LIMIT: UInt8 = 8\n"
        );
    }

    #[test]
    fn a_collection_type_writes_the_sugar_and_not_the_generic() {
        // `[T]` means `Swift.Array<T>` whatever else the file declares,
        // and `T?` means `Swift.Optional<T>`. A generated type called
        // `Array` cannot capture either.
        assert_eq!(type_(&Type::array(int32())), "[Int32]");
        assert_eq!(type_(&Type::array(Type::array(int32()))), "[[Int32]]");
        assert_eq!(type_(&Type::optional(int32())), "Int32?");
        assert_eq!(type_(&Type::optional(Type::array(int32()))), "[Int32]?");
    }

    #[test]
    fn a_generic_type_writes_its_arguments() {
        assert_eq!(
            type_(&Type::generic(
                "ZdExternal",
                alloc::vec![Type::named("Node")]
            )),
            "ZdExternal<Node>"
        );
    }

    #[test]
    fn an_internal_declaration_writes_no_keyword_because_that_is_the_default() {
        assert_eq!(visibility_text(Visibility::Internal), "");
        assert_eq!(visibility_text(Visibility::Public), "public ");
        assert_eq!(visibility_text(Visibility::Private), "private ");
    }

    #[test]
    fn a_string_literal_is_escaped_and_a_char_is_its_code_point() {
        assert_eq!(
            literal_text(&Literal::Str("a\"b".to_string())),
            "\"a\\\"b\""
        );
        assert_eq!(literal_text(&Literal::Char('A')), "65");
        assert_eq!(literal_text(&Literal::Null), "nil");
        assert_eq!(literal_text(&Literal::Bool(true)), "true");
    }

    #[test]
    fn a_file_writes_its_header_as_a_plain_comment() {
        let out = file(&File {
            doc: Doc::lines(&["Generated by ZeroDDS. Do not edit."]),
            imports: alloc::vec![Ident::new("Foundation")],
            decls: Vec::new(),
        });
        assert_eq!(
            out,
            "// Generated by ZeroDDS. Do not edit.\n\nimport Foundation\n"
        );
    }
}
