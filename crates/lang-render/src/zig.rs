// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Zig CST. Spec §5.
//!
//! One style, `zig fmt`'s: four spaces, the opening brace on the same
//! line, a trailing comma on every member of a multi-line container,
//! `//` and `///` comments. Not a preference — `zig fmt` ships with the
//! compiler and every reader of a generated file runs it, so a file that
//! moves under it is a diff nobody asked for. The layout here is checked
//! against the real formatter by
//! `zerodds-lang-rules/tests/zig_end_to_end.rs`.
//!
//! Nothing is decided here. A Zig type reads left to right and carries
//! its own length, optionality and pointer depth, so unlike
//! [`crate::c`] there is no declarator to split around a name:
//! a field is the name, a colon, and the type.
//!
//! ## Three places Zig's lexical rules differ from the C family
//!
//! 1. **There is no `\0` escape.** Zig's string escapes are `\n`, `\r`,
//!    `\t`, `\\`, `\'`, `\"`, `\xNN` and `\u{…}` (Zig language
//!    reference, "String Literals and Unicode Code Point Literals"). The
//!    shared [`crate::writer::escape_string`] writes `\0`, which no Zig
//!    compiler reads, so [`string_text`] is this module's own.
//! 2. **A documentation comment must be attached.** `///` with no
//!    declaration under it is `error: unattached documentation comment`,
//!    and one above a `test` block is refused as well. The file's own
//!    comment is therefore `//!` — the container form, which is only
//!    legal at the top — and [`zerodds_lang_cst::zig::AnalyseAll`]'s is
//!    a plain `//`.
//! 3. **A container's members end in a comma, not a semicolon.** A
//!    struct field, an enum variant and a function parameter are all
//!    comma-separated; only a declaration ends in `;`.

use crate::writer::{Writer, comment_lines, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{BodyText, Doc, Ident, Literal};
use zerodds_lang_cst::zig::{
    AnalyseAll, ConstDecl, Decl, Expr, Field, File, FuncDecl, Param, Stmt, Type, Variant,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    if !file.doc.is_empty() {
        comment_lines(&mut w, "//! ", file.doc.as_lines());
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
        Decl::Const(decl) => render_const(w, decl),
        Decl::Func(decl) => render_func(w, decl),
        Decl::Comment(lines) => comment_lines(w, "// ", lines.as_lines()),
        Decl::Verbatim(text) => verbatim_lines(w, text),
        Decl::AnalyseAll(block) => render_analyse_all(w, block),
    }
}

/// `pub fn name(a: T, b: U) R { body }`.
fn render_func(w: &mut Writer, node: &FuncDecl) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.is_pub {
        head.push_str("pub ");
    }
    head.push_str("fn ");
    head.push_str(node.name.as_str());
    head.push('(');
    let params: Vec<String> = node
        .params
        .iter()
        .map(|param| {
            let mut out = String::from(param.name.as_str());
            out.push_str(": ");
            out.push_str(&type_text(&param.type_));
            out
        })
        .collect();
    head.push_str(&params.join(", "));
    head.push_str(") ");
    head.push_str(&type_text(&node.returns));
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for statement in &node.body {
        render_stmt(w, statement);
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
        Stmt::Var {
            name,
            type_,
            value,
            mutable,
        } => {
            let mut line = String::from(if *mutable { "var " } else { "const " });
            line.push_str(name.as_str());
            if let Some(type_) = type_ {
                line.push_str(": ");
                line.push_str(&type_text(type_));
            }
            line.push_str(" = ");
            match value {
                Some(value) => line.push_str(&expr_text(value)),
                None => line.push_str("undefined"),
            }
            line.push(';');
            w.line(&line);
        }
        Stmt::Assign { place, value } => {
            let mut line = expr_text(place);
            line.push_str(" = ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Stmt::Expr(value) => {
            let mut line = expr_text(value);
            line.push(';');
            w.line(&line);
        }
        Stmt::DiscardExpr(value) => {
            let mut line = String::from("_ = ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Stmt::Discard(name) => {
            let mut line = String::from("_ = ");
            line.push_str(name.as_str());
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
        Stmt::If { .. } => render_if(w, node, "if ("),
        Stmt::CountUp {
            counter,
            type_,
            bound,
            body,
        } => {
            // The outer block scopes the counter, so two sequential
            // loops may share a name without Zig's shadowing error.
            w.line("{");
            w.indent();
            let mut line = String::from("var ");
            line.push_str(counter.as_str());
            line.push_str(": ");
            line.push_str(&type_text(type_));
            line.push_str(" = 0;");
            w.line(&line);
            let mut head = String::from("while (");
            head.push_str(counter.as_str());
            head.push_str(" < ");
            head.push_str(&expr_text(bound));
            head.push_str(") : (");
            head.push_str(counter.as_str());
            head.push_str(" += 1) {");
            w.line(&head);
            w.indent();
            for statement in body {
                render_stmt(w, statement);
            }
            w.dedent();
            w.line("}");
            w.dedent();
            w.line("}");
        }
        Stmt::While { condition, body } => {
            let mut head = String::from("while (");
            head.push_str(&expr_text(condition));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            for statement in body {
                render_stmt(w, statement);
            }
            w.dedent();
            w.line("}");
        }
        Stmt::Break => w.line("break;"),
        Stmt::Comment(lines) => comment_lines(w, "// ", lines.as_lines()),
    }
}

/// An `if`, with an `else`-block of exactly one `if` collapsed into
/// `else if` — the shape a long dispatch chain stays readable in, and a
/// shape `zig fmt` leaves exactly as written.
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
        head.push_str(") {");
        w.line(&head);
        w.indent();
        for statement in then {
            render_stmt(w, statement);
        }
        w.dedent();
        match otherwise {
            None => {
                w.line("}");
                return;
            }
            Some(chain) if chain.len() == 1 && matches!(chain[0], Stmt::If { .. }) => {
                node = &chain[0];
                keyword = "} else if (";
            }
            Some(block) => {
                w.line("} else {");
                w.indent();
                for statement in block {
                    render_stmt(w, statement);
                }
                w.dedent();
                w.line("}");
                return;
            }
        }
    }
}

/// `pub const Name: T = value;`
///
/// A value that is a container — a struct or an enum with members — is
/// written across lines, so its fields can be indented against the
/// declaration and can carry documentation of their own. Every other
/// value fits on the line.
fn render_const(w: &mut Writer, node: &ConstDecl) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.is_pub {
        head.push_str("pub ");
    }
    head.push_str("const ");
    head.push_str(node.name.as_str());
    if let Some(type_) = &node.type_ {
        head.push_str(": ");
        head.push_str(&type_text(type_));
    }
    head.push_str(" = ");
    match &node.value {
        Expr::Type(type_) => render_container(w, head, type_, &node.body_text),
        other => {
            head.push_str(&expr_text(other));
            head.push(';');
            w.line(&head);
        }
    }
}

/// A type on the right-hand side of a declaration, opened out where it
/// has members or `@verbatim` text.
fn render_container(w: &mut Writer, mut head: String, node: &Type, body_text: &BodyText) {
    let has_text = !body_text.begin.is_empty() || !body_text.end.is_empty();
    match node {
        Type::Struct(fields) if !fields.is_empty() || has_text => {
            head.push_str("struct {");
            w.line(&head);
            w.indent();
            for text in &body_text.begin {
                verbatim_lines(w, text);
            }
            for field in fields {
                render_field(w, field);
            }
            for text in &body_text.end {
                verbatim_lines(w, text);
            }
            w.dedent();
            w.line("};");
        }
        Type::Enum { tag, variants } if !variants.is_empty() => {
            head.push_str("enum(");
            head.push_str(&type_text(tag));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            for variant in variants {
                render_variant(w, variant);
            }
            w.dedent();
            w.line("};");
        }
        other => {
            head.push_str(&type_text(other));
            head.push(';');
            w.line(&head);
        }
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    line.push_str(": ");
    line.push_str(&type_text(&node.type_));
    if let Some(default) = &node.default {
        line.push_str(" = ");
        line.push_str(&expr_text(default));
    }
    line.push(',');
    w.line(&line);
}

fn render_variant(w: &mut Writer, node: &Variant) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(',');
    w.line(&line);
}

/// The `test` block that makes Zig analyse every declaration.
///
/// Its comment is `//` and not `///`: Zig refuses a documentation
/// comment attached to a test block.
fn render_analyse_all(w: &mut Writer, node: &AnalyseAll) {
    if !node.doc.is_empty() {
        comment_lines(w, "// ", node.doc.as_lines());
    }
    // The walk is written out rather than called from `std.testing`.
    // `refAllDeclsRecursive` was removed in Zig 0.16; what remains is
    // `refAllDecls`, which stops at the top level. Using that instead
    // would leave every nested container unanalysed — and a nested
    // container is exactly where a generated codec puts a union's arms
    // and a map's entry type, so the block would still be there and no
    // longer do what it is for.
    w.line("test {");
    w.indent();
    w.line("const zd_walk = struct {");
    w.indent();
    // `zd_` on the parameter too: a single-letter name would shadow a
    // generated declaration, and an IDL is free to declare `struct T`.
    // Zig rejects the shadowing outright, so the escape is not a matter
    // of taste — the file would not compile.
    w.line("fn all(comptime zd_T: type) void {");
    w.indent();
    w.line("inline for (comptime @import(\"std\").meta.declarations(zd_T)) |zd_decl| {");
    w.indent();
    w.line("const zd_field = @field(zd_T, zd_decl.name);");
    w.line("if (@TypeOf(zd_field) == type) {");
    w.indent();
    w.line("switch (@typeInfo(zd_field)) {");
    w.indent();
    w.line(".@\"struct\", .@\"enum\", .@\"union\", .@\"opaque\" => all(zd_field),");
    w.line("else => {},");
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");
    w.line("_ = &zd_field;");
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}.all;");
    w.line("zd_walk(@This());");
    w.dedent();
    w.line("}");
}

/// A struct or enum written inline, for a member whose type is one.
///
/// Reached only through [`type_text`]; a top-level container is written
/// by [`render_container`], which can indent it against the
/// declaration.
fn container_text(node: &Type) -> String {
    let mut w = Writer::spaces4();
    render_container(&mut w, String::new(), node, &BodyText::none());
    // `render_container` closes a declaration, so the `;` it wrote is
    // not part of the type. An inline container is a type expression and
    // the member's own comma follows it.
    w.finish().trim_end().trim_end_matches(';').to_string()
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/zig.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::Slice { is_const, element } => {
            let mut out = String::from("[]");
            if *is_const {
                out.push_str("const ");
            }
            out.push_str(&type_text(element));
            out
        }
        Type::Array { length, element } => {
            let mut out = String::from("[");
            out.push_str(&expr_text(length));
            out.push(']');
            out.push_str(&type_text(element));
            out
        }
        Type::Optional(inner) => {
            let mut out = String::from("?");
            out.push_str(&type_text(inner));
            out
        }
        Type::Pointer { is_const, target } => {
            let mut out = String::from("*");
            if *is_const {
                out.push_str("const ");
            }
            out.push_str(&type_text(target));
            out
        }
        Type::Struct(fields) => {
            if fields.is_empty() {
                return "struct {}".to_string();
            }
            container_text(node)
        }
        Type::Enum { tag, variants } => {
            if variants.is_empty() {
                let mut out = String::from("enum(");
                out.push_str(&type_text(tag));
                out.push_str(") {}");
                return out;
            }
            container_text(node)
        }
        // `fn (a: T) R`, with the space `zig fmt` puts after `fn`.
        Type::Fn { params, returns } => {
            let mut out = String::from("fn (");
            let parts: Vec<String> = params.iter().map(param_text).collect();
            out.push_str(&parts.join(", "));
            out.push_str(") ");
            out.push_str(&type_text(returns));
            out
        }
        Type::ErrorUnion { error_set, payload } => {
            let mut out = type_text(error_set);
            out.push('!');
            out.push_str(&type_text(payload));
            out
        }
    }
}

fn param_text(param: &Param) -> String {
    let mut out = String::new();
    if let Some(name) = &param.name {
        out.push_str(name.as_str());
        out.push_str(": ");
    }
    out.push_str(&type_text(&param.type_));
    out
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_zig.rs`, and
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
        Expr::Type(type_) => type_text(type_),
        Expr::Call { func, args } => {
            let mut out = expr_text(func);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
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
        Expr::Try(value) => {
            let mut out = String::from("try ");
            out.push_str(&expr_text(value));
            out
        }
        Expr::StructLit(fields) => {
            if fields.is_empty() {
                return ".{}".to_string();
            }
            let mut out = String::from(".{ ");
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| {
                    let mut part = String::from(".");
                    part.push_str(name.as_str());
                    part.push_str(" = ");
                    part.push_str(&expr_text(value));
                    part
                })
                .collect();
            out.push_str(&parts.join(", "));
            out.push_str(" }");
            out
        }
        Expr::ListLit(elements) => {
            if elements.is_empty() {
                return ".{}".to_string();
            }
            let mut out = String::from(".{ ");
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
            out.push_str(&char_text(*ch));
            out.push('\'');
            out
        }
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&string_text(text));
            out.push('"');
            out
        }
        // `&[_]u8{ … }` and not `[_]u8{ … }`: a byte string used where a
        // `[]const u8` belongs has to be a slice, and taking the address
        // of the array literal is what makes it one.
        Literal::Bytes(bytes) => {
            let mut out = String::from("&[_]u8{ ");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02x}")).collect();
            out.push_str(&parts.join(", "));
            out.push_str(" }");
            out
        }
        Literal::Null => "null".to_string(),
    }
}

/// Escape a string for a Zig string literal.
///
/// This module's own rather than [`crate::writer::escape_string`],
/// because that one writes `\0` for a NUL and Zig has no `\0` escape:
/// its escapes are `\n`, `\r`, `\t`, `\\`, `\'`, `\"`, `\xNN` and
/// `\u{…}` (Zig language reference, "String Literals and Unicode Code
/// Point Literals"). A NUL in an IDL string constant is legal — IDL 4.2
/// puts no restriction on a `string`'s contents — and would otherwise
/// produce a literal no Zig compiler reads.
///
/// Everything outside the printable ASCII range that is not one of the
/// named escapes is written `\xNN` if it is an octet and passed through
/// otherwise: Zig source is UTF-8, so a character above the ASCII range
/// is already what the file should contain.
#[must_use]
pub fn string_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        push_escaped(&mut out, ch, '"');
    }
    out
}

/// The same, for a character literal.
#[must_use]
pub fn char_text(ch: char) -> String {
    let mut out = String::new();
    push_escaped(&mut out, ch, '\'');
    out
}

fn push_escaped(out: &mut String, ch: char, delimiter: char) {
    match ch {
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if c == delimiter => {
            out.push('\\');
            out.push(c);
        }
        c if (c as u32) < 0x20 || c as u32 == 0x7F => {
            out.push_str(&alloc::format!("\\x{:02x}", c as u32));
        }
        other => out.push(other),
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
    use zerodds_lang_cst::zig::Primitive;

    fn struct_decl(fields: Vec<Field>) -> Decl {
        Decl::Const(ConstDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            is_pub: true,
            name: Ident::new("S"),
            type_: None,
            value: Expr::type_(Type::Struct(fields)),
        })
    }

    fn field(name: &str, type_: Type) -> Field {
        Field {
            doc: Doc::none(),
            name: Ident::new(name),
            type_,
            default: None,
        }
    }

    #[test]
    fn a_struct_declaration_indents_by_four_and_ends_every_field_in_a_comma() {
        let text = decl(&struct_decl(alloc::vec![
            field("x", Type::Primitive(Primitive::I32)),
            field("y", Type::Primitive(Primitive::F64)),
        ]));
        assert_eq!(
            text,
            "pub const S = struct {\n    x: i32,\n    y: f64,\n};\n"
        );
    }

    #[test]
    fn an_empty_struct_stays_on_one_line() {
        assert_eq!(decl(&struct_decl(Vec::new())), "pub const S = struct {};\n");
    }

    #[test]
    fn a_type_reads_left_to_right_with_no_declarator_to_split() {
        assert_eq!(
            type_(&Type::array(
                2,
                Type::array(3, Type::Primitive(Primitive::I32))
            )),
            "[2][3]i32"
        );
        assert_eq!(type_(&Type::text()), "[]const u8");
        assert_eq!(type_(&Type::slice(Type::named("Entry"))), "[]Entry");
        assert_eq!(
            type_(&Type::optional(Type::Primitive(Primitive::U8))),
            "?u8"
        );
        assert_eq!(
            type_(&Type::pointer(Type::Primitive(Primitive::AnyOpaque))),
            "*anyopaque"
        );
    }

    #[test]
    fn a_function_pointer_carries_its_parameter_names() {
        let type_ = Type::const_pointer(Type::Fn {
            params: alloc::vec![Param {
                name: Some(Ident::new("target")),
                type_: Type::named("Pose"),
            }],
            returns: alloc::boxed::Box::new(Type::Primitive(Primitive::Void)),
        });
        assert_eq!(super::type_(&type_), "*const fn (target: Pose) void");
    }

    #[test]
    fn an_optional_field_writes_its_absence_as_the_default() {
        let mut optional = field("b", Type::optional(Type::Primitive(Primitive::U8)));
        optional.default = Some(Expr::Literal(Literal::Null));
        let text = decl(&struct_decl(alloc::vec![optional]));
        assert!(text.contains("b: ?u8 = null,"), "{text}");
    }

    #[test]
    fn an_enum_writes_its_tag_type_and_every_value() {
        let text = decl(&Decl::Const(ConstDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            is_pub: true,
            name: Ident::new("Mode"),
            type_: None,
            value: Expr::type_(Type::Enum {
                tag: alloc::boxed::Box::new(Type::Primitive(Primitive::I32)),
                variants: alloc::vec![
                    Variant {
                        doc: Doc::none(),
                        name: Ident::new("IDLE"),
                        value: Some(Expr::int(0)),
                    },
                    Variant {
                        doc: Doc::none(),
                        name: Ident::new("ACTIVE"),
                        value: Some(Expr::int(4)),
                    },
                ],
            }),
        }));
        assert_eq!(
            text,
            "pub const Mode = enum(i32) {\n    IDLE = 0,\n    ACTIVE = 4,\n};\n"
        );
    }

    #[test]
    fn a_constant_writes_its_ascription_before_its_value() {
        let text = decl(&Decl::Const(ConstDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            is_pub: true,
            name: Ident::new("READY"),
            type_: Some(Type::named("Flags")),
            value: Expr::Binary {
                op: "<<",
                lhs: alloc::boxed::Box::new(Expr::int(1)),
                rhs: alloc::boxed::Box::new(Expr::int(4)),
            },
        }));
        assert_eq!(text, "pub const READY: Flags = 1 << 4;\n");
    }

    #[test]
    fn a_nul_is_written_as_a_hex_escape_because_zig_has_no_backslash_zero() {
        // The shared `escape_string` writes `\0`, which no Zig compiler
        // reads. A NUL in an IDL string constant is legal.
        assert_eq!(string_text("a\0b"), "a\\x00b");
        assert_eq!(string_text("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(string_text("line\nnext\ttab"), "line\\nnext\\ttab");
        // Zig source is UTF-8, so a character above ASCII is already
        // what the file should contain.
        assert_eq!(string_text("hä"), "hä");
    }

    #[test]
    fn the_file_comment_is_the_container_form_and_the_test_block_has_none() {
        // `///` with nothing under it is an unattached documentation
        // comment, and Zig refuses one above a `test` block outright.
        let text = file(&File {
            doc: Doc::lines(&["Generated.", "", "Do not edit."]),
            decls: alloc::vec![Decl::AnalyseAll(AnalyseAll {
                doc: Doc::line("Why this block is here."),
            })],
        });
        assert!(
            text.starts_with("//! Generated.\n//!\n//! Do not edit.\n"),
            "{text}"
        );
        assert!(text.contains("// Why this block is here.\n"), "{text}");
        assert!(text.contains("test {\n"), "{text}");
        assert!(
            !text.contains("/// Why"),
            "a documentation comment was attached to a test block\n{text}"
        );
    }

    #[test]
    fn a_doc_comment_sits_above_the_field_it_belongs_to() {
        let mut documented = field("type_", Type::Primitive(Primitive::I32));
        documented.doc = Doc::line("IDL: `type`.");
        let text = decl(&struct_decl(alloc::vec![documented]));
        assert_eq!(
            text,
            "pub const S = struct {\n    /// IDL: `type`.\n    type_: i32,\n};\n"
        );
    }
}
