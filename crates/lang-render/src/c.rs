// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the C CST. Spec §5.
//!
//! One style, the one the emitters this replaces wrote: four spaces,
//! the opening brace on the same line, `#include`s grouped as the tree
//! holds them.

use crate::writer::{Writer, comment_lines, escape_char, escape_string, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::c::{
    Block, Decl, Define, Enum, Enumerator, Expr, File, Function, Include, Member, Param, Statement,
    Struct, Type, Typedef, Union, Variable,
};
use zerodds_lang_cst::common::{Doc, Ident, Literal};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    doc(&mut w, &file.doc);
    if let Some(guard) = &file.guard {
        let mut line = String::from("#ifndef ");
        line.push_str(guard.as_str());
        w.line(&line);
        let mut define = String::from("#define ");
        define.push_str(guard.as_str());
        w.line(&define);
        w.blank_line();
    }
    for include in &file.includes {
        w.line(&include_text(include));
    }
    if !file.includes.is_empty() {
        w.blank_line();
    }
    for (index, decl) in file.decls.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_decl(&mut w, decl);
    }
    if let Some(guard) = &file.guard {
        w.blank_line();
        let mut line = String::from("#endif /* ");
        line.push_str(guard.as_str());
        line.push_str(" */");
        w.line(&line);
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

/// One expression, as it would stand inside a statement.
///
/// For a generator that writes a statement around an expression a rule
/// produced — a union's case label, spelled the way the codec spells it.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// An `#include` line; the C++ renderer writes the same one.
pub(crate) fn include_text(include: &Include) -> String {
    let mut out = String::from("#include ");
    if include.system {
        out.push('<');
        out.push_str(&include.path);
        out.push('>');
    } else {
        out.push('"');
        out.push_str(&include.path);
        out.push('"');
    }
    out
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Struct(s) => render_struct(w, s),
        Decl::Union(u) => render_union(w, u),
        Decl::Enum(e) => render_enum(w, e),
        Decl::Typedef(t) => render_typedef(w, t),
        Decl::Function(f) => render_function(w, f),
        Decl::Variable(v) => render_variable(w, v),
        Decl::Define(d) => render_define(w, d),
        Decl::Comment(c) => doc(w, c),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_struct(w: &mut Writer, node: &Struct) {
    doc(w, &node.doc);
    let mut head = String::from("struct");
    if let Some(name) = &node.name {
        head.push(' ');
        head.push_str(name.as_str());
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for text in &node.body_text.begin {
        verbatim_lines(w, text);
    }
    for member in &node.members {
        render_member(w, member);
    }
    for text in &node.body_text.end {
        verbatim_lines(w, text);
    }
    w.dedent();
    w.line("};");
}

fn render_union(w: &mut Writer, node: &Union) {
    doc(w, &node.doc);
    let mut head = String::from("union");
    if let Some(name) = &node.name {
        head.push(' ');
        head.push_str(name.as_str());
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for member in &node.members {
        render_member(w, member);
    }
    w.dedent();
    w.line("};");
}

fn render_member(w: &mut Writer, node: &Member) {
    doc(w, &node.doc);
    let mut line = declarator_text(&node.type_, node.name.as_str(), &node.dims);
    if let Some(bits) = node.bits {
        line.push_str(" : ");
        line.push_str(&alloc::format!("{bits}"));
    }
    line.push(';');
    w.line(&line);
}

/// `int32_t name[2][3]` — the dimensions belong to the declarator, not
/// to the type, which is what makes C's declaration syntax what it is.
fn declarator_text(type_: &Type, name: &str, dims: &[Expr]) -> String {
    let mut out = type_text(type_);
    if !out.ends_with('*') {
        out.push(' ');
    }
    out.push_str(name);
    for dim in dims {
        out.push('[');
        out.push_str(&expr_text(dim));
        out.push(']');
    }
    out
}

fn render_enum(w: &mut Writer, node: &Enum) {
    doc(w, &node.doc);
    let mut head = String::from("enum");
    if let Some(name) = &node.name {
        head.push(' ');
        head.push_str(name.as_str());
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for (index, enumerator) in node.enumerators.iter().enumerate() {
        render_enumerator(w, enumerator, index + 1 == node.enumerators.len());
    }
    w.dedent();
    w.line("};");
}

fn render_enumerator(w: &mut Writer, node: &Enumerator, last: bool) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    if !last {
        line.push(',');
    }
    w.line(&line);
}

fn render_typedef(w: &mut Writer, node: &Typedef) {
    doc(w, &node.doc);
    let mut line = String::from("typedef ");
    line.push_str(&declarator_text(
        &node.type_,
        node.name.as_str(),
        &node.dims,
    ));
    line.push(';');
    w.line(&line);
}

fn render_function(w: &mut Writer, node: &Function) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.is_static {
        head.push_str("static ");
    }
    if node.is_inline {
        head.push_str("inline ");
    }
    // Ahead of the return type, where GCC and Clang both take it on a
    // definition. See `c::Function::is_maybe_unused` for the one case
    // that sets it.
    if node.is_maybe_unused {
        head.push_str("__attribute__((unused)) ");
    }
    head.push_str(&type_text(&node.returns));
    if !head.ends_with('*') {
        head.push(' ');
    }
    head.push_str(node.name.as_str());
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    match &node.body {
        None => {
            head.push(';');
            w.line(&head);
        }
        Some(block) => {
            head.push_str(" {");
            w.line(&head);
            w.indent();
            render_block(w, block);
            w.dedent();
            w.line("}");
        }
    }
}

/// `void` for an empty parameter list: in C an empty `()` means
/// "unspecified", which is not what a generated prototype states.
fn params_text(params: &[Param]) -> String {
    if params.is_empty() {
        return "void".to_string();
    }
    let parts: Vec<String> = params
        .iter()
        .map(|param| match &param.name {
            Some(name) => declarator_text(&param.type_, name.as_str(), &[]),
            None => type_text(&param.type_),
        })
        .collect();
    parts.join(", ")
}

fn render_variable(w: &mut Writer, node: &Variable) {
    doc(w, &node.doc);
    let mut line = String::new();
    if node.is_static {
        line.push_str("static ");
    }
    if node.is_const {
        line.push_str("const ");
    }
    line.push_str(&declarator_text(&node.type_, node.name.as_str(), &[]));
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(';');
    w.line(&line);
}

fn render_define(w: &mut Writer, node: &Define) {
    doc(w, &node.doc);
    let mut line = String::from("#define ");
    line.push_str(node.name.as_str());
    if let Some(value) = &node.value {
        line.push(' ');
        line.push_str(&expr_text(value));
    }
    w.line(&line);
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_block(w: &mut Writer, block: &Block) {
    for statement in &block.statements {
        render_statement(w, statement);
    }
}

/// An `if` and its `else`, with a lone nested `if` written as `else if`.
///
/// The tree has no `else if` node — an else branch is a block, and a
/// chain is a block holding one `if`. Rendering that literally nests one
/// pair of braces per arm, which is correct C and is also a cliff: a
/// `@mutable` aggregate's decoder dispatches on the member id with one
/// arm per member, and C99 §5.2.4.1 requires a compiler to support only
/// 127 levels of nested compound statement. A hundred-and-thirty-member
/// type would have generated a file no conforming compiler must accept.
/// Flattening the chain costs one comparison here and removes the
/// limit.
///
/// The arm is taken in a loop and not by a call to this function again.
/// The chain's length is the union's case count, the enumeration's
/// enumerator count or the `@mutable` aggregate's member count — the
/// walk in `lang-rules/src/emit/encode.rs` builds one `If` per arm, each
/// in the previous one's `else` — and nothing in the model bounds any of
/// the three. Recursing once per arm would have made the renderer's own
/// stack the limit on how many cases a union may have.
///
/// zerodds-lint: recursion-depth 64 (block nesting only; the arm chain
/// is walked iteratively, and what remains is one level per nested block
/// of a generated body — the codec walk emits a routine per type rather
/// than inlining, so a body nests by its framing and by one loop per
/// array dimension, never by the type graph)
fn render_if_chain(w: &mut Writer, node: &Statement, opener: &str) {
    let mut node = node;
    let mut opener = opener;
    loop {
        let Statement::If {
            condition,
            then,
            otherwise,
        } = node
        else {
            render_statement(w, node);
            return;
        };
        let mut head = String::from(opener);
        head.push_str(&expr_text(condition));
        head.push_str(") {");
        w.line(&head);
        w.indent();
        render_block(w, then);
        w.dedent();
        match otherwise {
            Some(block) => match block.statements.as_slice() {
                [nested @ Statement::If { .. }] => {
                    node = nested;
                    opener = "} else if (";
                }
                _ => {
                    w.line("} else {");
                    w.indent();
                    render_block(w, block);
                    w.dedent();
                    w.line("}");
                    return;
                }
            },
            None => {
                w.line("}");
                return;
            }
        }
    }
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Declare { type_, name, value } => {
            let mut line = declarator_text(type_, name.as_str(), &[]);
            if let Some(value) = value {
                line.push_str(" = ");
                line.push_str(&expr_text(value));
            }
            line.push(';');
            w.line(&line);
        }
        Statement::Expr(value) => {
            let mut line = expr_text(value);
            line.push(';');
            w.line(&line);
        }
        Statement::Return(value) => {
            let mut line = String::from("return");
            if let Some(value) = value {
                line.push(' ');
                line.push_str(&expr_text(value));
            }
            line.push(';');
            w.line(&line);
        }
        Statement::If { .. } => render_if_chain(w, node, "if ("),
        Statement::For {
            init,
            condition,
            step,
            body,
        } => {
            let mut head = String::from("for (");
            head.push_str(statement_inline(init).trim_end_matches(';'));
            head.push_str("; ");
            head.push_str(&expr_text(condition));
            head.push_str("; ");
            head.push_str(&expr_text(step));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            render_block(w, body);
            w.dedent();
            w.line("}");
        }
        Statement::While { condition, body } => {
            let mut head = String::from("while (");
            head.push_str(&expr_text(condition));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            render_block(w, body);
            w.dedent();
            w.line("}");
        }
        Statement::Break => w.line("break;"),
        Statement::Comment(c) => doc(w, c),
    }
}

/// A statement written on one line, for a `for` header.
fn statement_inline(node: &Statement) -> String {
    match node {
        Statement::Declare { type_, name, value } => {
            let mut out = declarator_text(type_, name.as_str(), &[]);
            if let Some(value) = value {
                out.push_str(" = ");
                out.push_str(&expr_text(value));
            }
            out
        }
        Statement::Expr(value) => expr_text(value),
        _ => String::new(),
    }
}

/// zerodds-lint: recursion-depth 3 (`const T *`, and no deeper. The C
/// materialiser gives every collection and every aggregate a `typedef`
/// of its own (`lang-rules/src/adapter/c.rs`), so a `Type` here is a tag
/// or a primitive under at most a `const` and a pointer; the IDL's own
/// nesting never reaches this walk.)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::StructTag(name) => {
            let mut out = String::from("struct ");
            out.push_str(name.as_str());
            out
        }
        Type::UnionTag(name) => {
            let mut out = String::from("union ");
            out.push_str(name.as_str());
            out
        }
        Type::EnumTag(name) => {
            let mut out = String::from("enum ");
            out.push_str(name.as_str());
            out
        }
        Type::Pointer(target) => {
            let mut out = type_text(target);
            if !out.ends_with('*') {
                out.push(' ');
            }
            out.push('*');
            out
        }
        Type::Const(target) => {
            let mut out = String::from("const ");
            out.push_str(&type_text(target));
            out
        }
        Type::Void => "void".to_string(),
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_c.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => name.as_str().to_string(),
        Expr::Call { callee, args } => {
            let mut out = expr_text(callee);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Member { value, name } => {
            let mut out = expr_text(value);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Expr::Arrow { value, name } => {
            let mut out = expr_text(value);
            out.push_str("->");
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
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
            out
        }
        Expr::AddressOf(value) => {
            let mut out = String::from("&");
            out.push_str(&expr_text(value));
            out
        }
        Expr::Deref(value) => {
            let mut out = String::from("*");
            out.push_str(&expr_text(value));
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
        Expr::Cast { type_, value } => {
            let mut out = String::from("(");
            out.push_str(&type_text(type_));
            out.push(')');
            out.push_str(&expr_text(value));
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::SizeOf(type_) => {
            let mut out = String::from("sizeof(");
            out.push_str(&type_text(type_));
            out.push(')');
            out
        }
        Expr::Initializer(elements) => {
            let mut out = String::from("{ ");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push_str(" }");
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
            let mut out = String::from("'");
            out.push_str(&escape_char(*ch));
            out.push('\'');
            out
        }
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
        Literal::Bytes(bytes) => {
            let mut out = String::from("{ ");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            out.push_str(&parts.join(", "));
            out.push_str(" }");
            out
        }
        Literal::Null => "NULL".to_string(),
    }
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    // `//` rather than `/* */` per line: a block comment opened per
    // line and never closed is not C, and closing it per line is what
    // the emitters this replaces avoided by using the C99 form.
    comment_lines(w, "// ", doc.as_lines());
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

/// An integer as the source text of a C constant expression.
///
/// Every value but one is its own decimal. −2^63 is not a literal in C
/// at all: C99 6.4.4.1 has no negative integer constant, so
/// `-9223372036854775808` is a unary minus over the decimal constant
/// `9223372036854775808`, and that constant fits no signed type in the
/// list 6.4.4.1 gives it — the implementation may give it an extended
/// unsigned type, and the negation that follows is then an
/// implementation-defined conversion rather than the value the source
/// wrote. Wrapping it in `INT64_C(..)` changes nothing: C99 7.18.4.1
/// makes that macro paste a suffix onto the token it is handed, so the
/// same too-large constant is what the minus applies to. Measured, gcc
/// 16.1.0 answers "integer constant is so large that it is unsigned"
/// and Apple clang 21.0.0 `-Wimplicitly-unsigned-literal`.
///
/// The subtraction `<stdint.h>` itself writes `INT64_MIN` as is exact
/// and has no such constant in it. `crates/wire-probe/src/fill.rs`,
/// `fill_ros.rs` and `fill_cpp.rs` already spell the value this way.
#[must_use]
pub fn integer_text(value: i128) -> String {
    if value == i128::from(i64::MIN) {
        return "(-9223372036854775807LL - 1)".to_string();
    }
    format!("{value}")
}
