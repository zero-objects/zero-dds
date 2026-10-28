// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Nim CST. Spec §5.
//!
//! One style, the one `nimpretty` writes: two spaces per level, a
//! section keyword on a line of its own, `#` comments.
//!
//! ## Indentation is the grammar here
//!
//! Nim is indentation-sensitive, so this renderer is closer to
//! [`crate::python`] than to [`crate::c`]: a wrong indent is a different
//! program, or none. [`Writer`] owns the depth, every body goes through
//! the section and declaration functions, and nothing else in this
//! module writes a newline. Two spaces and not four, and never a tab —
//! Nim's compiler rejects a tab used for indentation outright (Nim
//! manual, Lexical Analysis: "Tabulators are not allowed as part of the
//! indentation").
//!
//! ## `#` and not `##`
//!
//! Nim's documentation comment is `##`, and it is not a comment the
//! parser skips: it becomes an AST node, and it is only accepted where a
//! documentation comment may attach. A `#` comment is dropped by the
//! lexer wherever it appears at a legal indentation. A generated file
//! gains nothing from `nim doc` output and loses the whole file if one
//! `##` lands where the parser will not take it, so every comment this
//! renderer writes is a `#`.
//!
//! ## The output is pure ASCII
//!
//! Every code point outside printable ASCII is escaped, in string
//! literals and comments alike, for the reason the Python renderer
//! gives: a file that is pure ASCII cannot be changed by an editor, a
//! terminal or a CI runner that guessed the wrong encoding.

use crate::writer::{Writer, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::nim::{
    ConstDecl, EnumField, Expr, Field, File, Import, Param, ProcDecl, Section, Stmt, Type,
    TypeBody, TypeDecl,
};

/// The indentation Nim files are written with.
///
/// Two spaces, and spaces rather than a tab because Nim refuses a tab in
/// the indentation of a line.
const UNIT: &str = "  ";

/// Render a whole file.
#[must_use]
pub fn file(node: &File) -> String {
    let mut w = Writer::new(UNIT);
    comment(&mut w, &node.doc);
    if !node.imports.is_empty() {
        w.blank_line();
        for import in &node.imports {
            w.line(&import_text(import));
        }
    }
    for section in &node.sections {
        w.blank_line();
        render_section(&mut w, section);
    }
    w.finish()
}

/// Render one section, for a caller that has only one.
#[must_use]
pub fn section(node: &Section) -> String {
    let mut w = Writer::new(UNIT);
    render_section(&mut w, node);
    w.finish()
}

/// Render one type, for a caller that wants its spelling.
#[must_use]
pub fn type_(node: &Type) -> String {
    type_text(node)
}

/// Render one expression, for a caller that wants its spelling.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

fn import_text(node: &Import) -> String {
    format!("import {}", node.path)
}

/// A section keyword and the declarations under it.
///
/// A section with nothing in it writes nothing at all: `type` on a line
/// with no indented block after it is a parse error, not an empty
/// section.
fn render_section(w: &mut Writer, node: &Section) {
    match node {
        Section::Type(decls) => {
            if decls.is_empty() {
                return;
            }
            w.line("type");
            w.indent();
            for (index, decl) in decls.iter().enumerate() {
                if index > 0 {
                    w.blank_line();
                }
                render_type_decl(w, decl);
            }
            w.dedent();
        }
        Section::Const(decls) => {
            if decls.is_empty() {
                return;
            }
            w.line("const");
            w.indent();
            for decl in decls {
                render_const(w, decl);
            }
            w.dedent();
        }
        Section::Comment(doc) => comment(w, doc),
        Section::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_type_decl(w: &mut Writer, node: &TypeDecl) {
    for text in &node.text.before {
        verbatim_lines(w, text);
    }
    render_type_decl_proper(w, node);
    for text in &node.text.after {
        verbatim_lines(w, text);
    }
}

fn render_type_decl_proper(w: &mut Writer, node: &TypeDecl) {
    // Above the declaration and never inside the body: an object with no
    // fields has no body to put a comment in, and a body holding only a
    // comment is not an object body the parser will take.
    comment(w, &node.doc);
    let head = format!("{} = ", name_text(&node.name, node.exported));
    match &node.body {
        TypeBody::Object(fields) => {
            w.line(&format!("{head}object"));
            let body = &node.text.body;
            if fields.is_empty() && body.begin.is_empty() && body.end.is_empty() {
                return;
            }
            w.indent();
            for text in &body.begin {
                verbatim_lines(w, text);
            }
            for field in fields {
                render_field(w, field);
            }
            for text in &body.end {
                verbatim_lines(w, text);
            }
            w.dedent();
        }
        TypeBody::Enum(fields) => {
            w.line(&format!("{head}enum"));
            w.indent();
            for field in fields {
                render_enum_field(w, field);
            }
            w.dedent();
        }
        TypeBody::Alias(type_) => w.line(&format!("{head}{}", type_text(type_))),
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    comment(w, &node.doc);
    w.line(&format!(
        "{}: {}",
        name_text(&node.name, node.exported),
        type_text(&node.type_)
    ));
}

/// One field of an enumeration, on a line of its own.
///
/// No comma: Nim's `enumDecl` makes the separator optional and the
/// newline does the work, which is the form `nimpretty` writes. No
/// export marker either — the production takes a `symbol`, and the
/// fields of an exported enumeration are reachable without one.
fn render_enum_field(w: &mut Writer, node: &EnumField) {
    comment(w, &node.doc);
    let mut line = node.name.as_str().to_string();
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    w.line(&line);
}

fn render_const(w: &mut Writer, node: &ConstDecl) {
    comment(w, &node.doc);
    let mut line = name_text(&node.name, node.exported);
    if let Some(type_) = &node.type_ {
        line.push_str(": ");
        line.push_str(&type_text(type_));
    }
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

/// A declared name, with the export marker when it carries one.
///
/// The `*` goes after the name and before the colon or the `=`, which is
/// the `identVis` production: `x*: int32`, `Pose* = object`.
fn name_text(name: &Ident, exported: bool) -> String {
    let mut out = name.as_str().to_string();
    if exported {
        out.push('*');
    }
    out
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/nim.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::Seq(element) => format!("seq[{}]", type_text(element)),
        Type::Array { length, element } => {
            format!("array[{}, {}]", expr_text(length), type_text(element))
        }
        Type::Option(inner) => format!("Option[{}]", type_text(inner)),
        // A prefix, with no brackets of its own: `ref` binds to the type
        // expression that follows, so `ref seq[int32]` is a reference to
        // a sequence and not a sequence of references.
        Type::Ref(target) => format!("ref {}", type_text(target)),
        Type::Proc { params, result } => {
            let parts: Vec<String> = params.iter().map(param_text).collect();
            let mut out = format!("proc ({})", parts.join(", "));
            if let Some(result) = result {
                out.push_str(": ");
                out.push_str(&type_text(result));
            }
            out
        }
    }
}

fn param_text(node: &Param) -> String {
    format!("{}: {}", node.name.as_str(), type_text(&node.type_))
}

fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => name.as_str().to_string(),
        Expr::Binary { op, lhs, rhs } => {
            format!("{} {op} {}", expr_text(lhs), expr_text(rhs))
        }
        Expr::Convert { type_, value } => {
            format!("{}({})", type_text(type_), expr_text(value))
        }
        Expr::Paren(inner) => format!("({})", expr_text(inner)),
        Expr::Call { func, args } => {
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            format!("{}({})", expr_text(func), parts.join(", "))
        }
        Expr::Dot { value, name } => format!("{}.{}", expr_text(value), name.as_str()),
        Expr::Index { value, index } => format!("{}[{}]", expr_text(value), expr_text(index)),
        // A space and never brackets of its own: `not` is a prefix
        // operator, and the operand carries its parentheses where the
        // builder asked for them.
        Expr::Unary { op, value } => format!("{op} {}", expr_text(value)),
        Expr::Cast { type_, value } => {
            format!("cast[{}]({})", type_text(type_), expr_text(value))
        }
        Expr::NewSeq { element, length } => {
            format!("newSeq[{}]({})", type_text(element), expr_text(length))
        }
        Expr::Deref(value) => format!("{}[]", expr_text(value)),
    }
}

/// Render one procedure, for a caller assembling a codec file.
#[must_use]
pub fn proc_decl(node: &ProcDecl) -> String {
    let mut w = Writer::new(UNIT);
    render_proc(&mut w, node);
    w.finish()
}

/// Render a run of procedures, one blank line between them.
#[must_use]
pub fn proc_decls(nodes: &[ProcDecl]) -> String {
    let mut w = Writer::new(UNIT);
    for (index, node) in nodes.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_proc(&mut w, node);
    }
    w.finish()
}

fn render_proc(w: &mut Writer, node: &ProcDecl) {
    comment(w, &node.doc);
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    // The export marker goes on the name and only on the definition
    // Nim reads first; it is written on both the forward declaration
    // and the body here, which Nim accepts as long as the two agree.
    let marker = if node.exported { "*" } else { "" };
    let mut head = format!(
        "proc {}{}({})",
        node.name.as_str(),
        marker,
        params.join(", ")
    );
    if let Some(returns) = &node.returns {
        head.push_str(": ");
        head.push_str(&type_text(returns));
    }
    let Some(body) = &node.body else {
        // The forward declaration: the signature with no `=`, which is
        // what lets two codecs call each other whatever order their
        // bodies come in.
        w.line(&head);
        return;
    };
    head.push_str(" =");
    w.line(&head);
    w.indent();
    render_body(w, body);
    w.dedent();
}

/// A statement list at the current depth; an empty one renders
/// `discard`, because a block with nothing indented under it does not
/// parse.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Nim nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_body(w: &mut Writer, body: &[Stmt]) {
    if body.iter().all(|stmt| matches!(stmt, Stmt::Comment(_))) {
        for stmt in body {
            render_stmt(w, stmt);
        }
        w.line("discard");
        return;
    }
    for stmt in body {
        render_stmt(w, stmt);
    }
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Nim nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::Var { name, type_, value } => {
            let mut line = format!("var {}", name.as_str());
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
            w.line(&format!("{} = {}", expr_text(place), expr_text(value)));
        }
        Stmt::Expr(value) => w.line(&expr_text(value)),
        Stmt::Discard(value) => w.line(&format!("discard {}", expr_text(value))),
        Stmt::Return(value) => match value {
            Some(value) => w.line(&format!("return {}", expr_text(value))),
            None => w.line("return"),
        },
        Stmt::If {
            condition,
            then,
            otherwise,
        } => {
            w.line(&format!("if {}:", expr_text(condition)));
            w.indent();
            render_body(w, then);
            w.dedent();
            if let Some(otherwise) = otherwise {
                w.line("else:");
                w.indent();
                render_body(w, otherwise);
                w.dedent();
            }
        }
        Stmt::For {
            counter,
            bound,
            body,
        } => {
            // The counter is a `uint32` because every bound the shared
            // walk hands over is a `uint32` count or a literal, and a
            // literal converts against the range's type.
            w.line(&format!(
                "for {} in 0'u32 ..< {}:",
                counter.as_str(),
                expr_text(bound)
            ));
            w.indent();
            render_body(w, body);
            w.dedent();
        }
        Stmt::While { condition, body } => {
            w.line(&format!("while {}:", expr_text(condition)));
            w.indent();
            render_body(w, body);
            w.dedent();
        }
        Stmt::Break => w.line("break"),
        Stmt::Comment(doc) => comment(w, doc),
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
        // A Nim character literal is one byte and takes no escape for a
        // newline: `'\n'` is refused because a newline may be one
        // character or two, and `'\L'` and `'\x0A'` are what is left.
        // The numeric form is written for every character outside
        // printable ASCII, so the arm needs no table of Nim's letter
        // escapes.
        Literal::Char(ch) => char_literal(*ch),
        Literal::Str(text) => string_literal(text),
        // `@[...]` is a `seq` literal; the `'u8` suffix is what makes
        // the elements octets rather than the platform's `int`.
        Literal::Bytes(bytes) => {
            let parts: Vec<String> = bytes.iter().map(|b| format!("0x{b:02x}'u8")).collect();
            format!("@[{}]", parts.join(", "))
        }
        Literal::Null => "nil".to_string(),
    }
}

/// A Nim character literal holding `ch`.
///
/// Nim's `char` is one byte, so a code point above 0x7F has no character
/// literal at all; it is written as the first octet of nothing and would
/// be wrong. Only the printable ASCII range is written as a quoted
/// character — everything else takes `'\xNN'`, and a code point that
/// does not fit in one octet is refused a literal by being written as
/// its low octet is not: it takes `'\x00'` only when it genuinely is
/// zero. A caller with a wide character writes an integer instead, which
/// is what `zerodds_lang_rules::adapter::nim` does.
#[must_use]
pub fn char_literal(ch: char) -> String {
    let point = u32::from(ch);
    match ch {
        '\'' => "'\\''".to_string(),
        '\\' => "'\\\\'".to_string(),
        _ if (0x20..0x7f).contains(&point) => format!("'{ch}'"),
        _ if point <= 0xff => format!("'\\x{point:02x}'"),
        // Not one octet. Written as the escape for the replacement
        // octet would be a substitution; the code point is written as a
        // number instead, which is not a `char` and will not typecheck
        // where one belongs — which is the loud failure.
        _ => format!("{point}"),
    }
}

/// A double-quoted Nim string literal holding `text`, in ASCII.
///
/// The escape is `\xHH` per **octet** of the UTF-8 encoding, not per
/// code point. A Nim `string` is a sequence of bytes with no encoding of
/// its own, and `\xHH` inserts one byte (Nim manual, Lexical Analysis,
/// "String literals"), so writing the UTF-8 octets is what puts the same
/// bytes in the string that the source had. Exactly two hex digits are
/// read after `\x`, so a literal character that happens to be a hex
/// digit cannot extend the escape.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&escape_ascii(text));
    out.push('"');
    out
}

/// The body of a Nim string literal: `text`, escaped down to ASCII.
fn escape_ascii(text: &str) -> String {
    let mut out = String::new();
    let mut buffer = [0_u8; 4];
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => {
                let point = u32::from(other);
                if (0x20..0x7f).contains(&point) {
                    out.push(other);
                } else {
                    for byte in other.encode_utf8(&mut buffer).as_bytes() {
                        out.push_str(&format!("\\x{byte:02x}"));
                    }
                }
            }
        }
    }
    out
}

/// A `#` comment, one line per entry.
///
/// The text is reduced to ASCII for the reason the module documentation
/// gives. A comment carrying a newline would put the indentation of its
/// second line beyond the writer's reach, which is why [`Doc`] holds
/// lines and not a string.
fn comment(w: &mut Writer, doc: &Doc) {
    for line in doc.as_lines() {
        if line.is_empty() {
            w.line("#");
        } else {
            w.line(&format!("# {}", escape_ascii(line)));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::boxed::Box;
    use alloc::vec;
    use zerodds_lang_cst::nim::Primitive;

    fn field(name: &str, type_: Type) -> Field {
        Field {
            doc: Doc::none(),
            name: Ident::new(name),
            exported: true,
            type_,
        }
    }

    #[test]
    fn an_object_indents_its_fields_by_two_under_the_type_section() {
        // Two levels: one for the declaration inside `type`, one for the
        // fields inside the object. Nim has no closing delimiter, so the
        // dedent is the end of the block.
        let text = section(&Section::Type(vec![TypeDecl {
            text: zerodds_lang_cst::nim::TypeText::default(),
            doc: Doc::none(),
            name: Ident::new("Pose"),
            exported: true,
            body: TypeBody::Object(vec![
                field("x", Type::Primitive(Primitive::Int32)),
                field("y", Type::Primitive(Primitive::Float64)),
            ]),
        }]));
        assert_eq!(
            text,
            "type\n  Pose* = object\n    x*: int32\n    y*: float64\n"
        );
    }

    #[test]
    fn an_object_with_no_fields_is_a_bare_object() {
        // Legal Nim, so no refusal is needed for a struct with no
        // members. A body holding only the comment would not be.
        let text = section(&Section::Type(vec![TypeDecl {
            text: zerodds_lang_cst::nim::TypeText::default(),
            doc: Doc::line("Empty."),
            name: Ident::new("E"),
            exported: true,
            body: TypeBody::Object(Vec::new()),
        }]));
        assert_eq!(text, "type\n  # Empty.\n  E* = object\n");
    }

    #[test]
    fn an_enumeration_writes_one_field_per_line_with_no_export_marker() {
        let text = section(&Section::Type(vec![TypeDecl {
            text: zerodds_lang_cst::nim::TypeText::default(),
            doc: Doc::none(),
            name: Ident::new("Mode"),
            exported: true,
            body: TypeBody::Enum(vec![
                EnumField {
                    doc: Doc::none(),
                    name: Ident::new("IDLE"),
                    value: Some(Expr::int(0)),
                },
                EnumField {
                    doc: Doc::none(),
                    name: Ident::new("ACTIVE"),
                    value: Some(Expr::int(4)),
                },
            ]),
        }]));
        assert_eq!(text, "type\n  Mode* = enum\n    IDLE = 0\n    ACTIVE = 4\n");
    }

    #[test]
    fn an_empty_section_writes_nothing_because_the_keyword_alone_is_a_syntax_error() {
        assert_eq!(section(&Section::Type(Vec::new())), "");
        assert_eq!(section(&Section::Const(Vec::new())), "");
    }

    #[test]
    fn a_const_carries_its_declared_type_before_the_value() {
        let text = section(&Section::Const(vec![ConstDecl {
            doc: Doc::none(),
            name: Ident::new("LIMIT"),
            exported: true,
            type_: Some(Type::Primitive(Primitive::UInt8)),
            value: Expr::int(8),
        }]));
        assert_eq!(text, "const\n  LIMIT*: uint8 = 8\n");
    }

    #[test]
    fn a_bitmask_constant_shifts_in_the_masks_own_width() {
        // `Flags(1) shl 63` is an unsigned shift; `1 shl 63` is an `int`
        // shift and overflows into a negative number before the
        // conversion could happen.
        let text = expr(&Expr::Binary {
            op: "shl",
            lhs: Box::new(Expr::Convert {
                type_: Box::new(Type::named("Flags")),
                value: Box::new(Expr::int(1)),
            }),
            rhs: Box::new(Expr::int(63)),
        });
        assert_eq!(text, "Flags(1) shl 63");
    }

    #[test]
    fn every_collection_writes_the_bracket_form_nim_reads() {
        assert_eq!(
            type_(&Type::seq(Type::Primitive(Primitive::Int32))),
            "seq[int32]"
        );
        assert_eq!(
            type_(&Type::array(
                2,
                Type::array(3, Type::Primitive(Primitive::Int32))
            )),
            "array[2, array[3, int32]]"
        );
        assert_eq!(
            type_(&Type::option(Type::Primitive(Primitive::UInt8))),
            "Option[uint8]"
        );
    }

    #[test]
    fn a_reference_is_a_prefix_and_takes_no_brackets_of_its_own() {
        // `@optional @external Node next` is an `Option` outside a
        // `ref`: the reference is what makes the declaration finite and
        // the `Option` is what says the member may be absent.
        assert_eq!(
            type_(&Type::option(Type::reference(Type::named("Node")))),
            "Option[ref Node]"
        );
        // `ref` binds to the whole type expression after it, so this is
        // a reference to a sequence and not a sequence of references.
        assert_eq!(
            type_(&Type::reference(Type::seq(Type::Primitive(
                Primitive::Int32
            )))),
            "ref seq[int32]"
        );
    }

    #[test]
    fn a_procedural_type_writes_its_parameters_and_its_result() {
        assert_eq!(
            type_(&Type::Proc {
                params: vec![Param {
                    name: Ident::new("target"),
                    type_: Type::named("Pose"),
                }],
                result: None,
            }),
            "proc (target: Pose)"
        );
        assert_eq!(
            type_(&Type::Proc {
                params: Vec::new(),
                result: Some(Box::new(Type::Primitive(Primitive::Int32))),
            }),
            "proc (): int32"
        );
    }

    #[test]
    fn a_string_literal_leaves_the_file_pure_ascii_one_escape_per_octet() {
        assert_eq!(string_literal("Z"), "\"Z\"");
        // U+00E4 is two octets in UTF-8, and a Nim `\xHH` escape inserts
        // one byte — so it takes two escapes and not one.
        assert_eq!(string_literal("\u{e4}"), "\"\\xc3\\xa4\"");
        assert_eq!(string_literal("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(string_literal("a\nb"), "\"a\\nb\"");
    }

    #[test]
    fn a_character_literal_takes_the_hex_escape_where_nim_has_no_letter_one() {
        assert_eq!(char_literal('a'), "'a'");
        assert_eq!(char_literal('\''), "'\\''");
        // Nim refuses `'\n'`: a newline may be one character or two.
        assert_eq!(char_literal('\n'), "'\\x0a'");
    }

    #[test]
    fn a_file_puts_its_imports_above_the_sections_and_below_the_comment() {
        let text = file(&File {
            doc: Doc::line("Generated."),
            imports: vec![Import::new("std/options")],
            sections: vec![Section::Type(vec![TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: Doc::none(),
                name: Ident::new("S"),
                exported: true,
                body: TypeBody::Object(vec![field(
                    "a",
                    Type::option(Type::Primitive(Primitive::Int32)),
                )]),
            }])],
        });
        assert_eq!(
            text,
            "# Generated.\n\nimport std/options\n\ntype\n  S* = object\n    a*: Option[int32]\n"
        );
    }

    #[test]
    fn every_expression_form_writes_the_nim_spelling() {
        assert_eq!(
            expr(&Expr::call(
                Expr::dot(Expr::name("w"), "writeU32"),
                vec![Expr::dot(Expr::name("v"), "x")],
            )),
            "w.writeU32(v.x)"
        );
        assert_eq!(
            expr(&Expr::Index {
                value: Box::new(Expr::name("v")),
                index: Box::new(Expr::name("i")),
            }),
            "v[i]"
        );
        assert_eq!(
            expr(&Expr::Unary {
                op: "not",
                value: Box::new(Expr::name("zd_sent")),
            }),
            "not zd_sent"
        );
        // `cast` reinterprets where a conversion would range-check:
        // `int8(200'u8)` raises and `cast[int8](200'u8)` is `-56`.
        assert_eq!(
            expr(&Expr::Cast {
                type_: Box::new(Type::Primitive(Primitive::Int8)),
                value: Box::new(Expr::name("k")),
            }),
            "cast[int8](k)"
        );
        assert_eq!(
            expr(&Expr::NewSeq {
                element: Box::new(Type::Primitive(Primitive::Int32)),
                length: Box::new(Expr::name("zd_n")),
            }),
            "newSeq[int32](zd_n)"
        );
        assert_eq!(expr(&Expr::Deref(Box::new(Expr::name("p")))), "p[]");
    }

    #[test]
    fn a_proc_indents_its_body_and_a_forward_declaration_has_no_equals() {
        let node = ProcDecl {
            doc: Doc::none(),
            exported: false,
            name: Ident::new("zd_enc_S"),
            params: vec![
                Param {
                    name: Ident::new("w"),
                    type_: Type::named("zdwire.Writer"),
                },
                Param {
                    name: Ident::new("v"),
                    type_: Type::named("S"),
                },
            ],
            returns: None,
            body: Some(vec![Stmt::Expr(Expr::call(
                Expr::dot(Expr::name("w"), "writeBool"),
                vec![Expr::dot(Expr::name("v"), "a")],
            ))]),
        };
        assert_eq!(
            proc_decl(&node),
            "proc zd_enc_S(w: zdwire.Writer, v: S) =\n  w.writeBool(v.a)\n"
        );
        let forward = ProcDecl { body: None, ..node };
        // No `=`: Nim resolves a module-scope name only after its
        // declaration, so this is what lets mutually recursive codecs
        // compile in either order.
        assert_eq!(
            proc_decl(&forward),
            "proc zd_enc_S(w: zdwire.Writer, v: S)\n"
        );
    }

    #[test]
    fn a_proc_with_a_result_writes_it_and_an_empty_body_is_discard() {
        let node = ProcDecl {
            doc: Doc::line("Table 9."),
            exported: false,
            name: Ident::new("zd_def_p_long"),
            params: Vec::new(),
            returns: Some(Type::Primitive(Primitive::Int32)),
            body: Some(vec![Stmt::Return(Some(Expr::int(0)))]),
        };
        assert_eq!(
            proc_decl(&node),
            "# Table 9.\nproc zd_def_p_long(): int32 =\n  return 0\n"
        );
        let empty = ProcDecl {
            doc: Doc::none(),
            exported: false,
            name: Ident::new("zd_noop"),
            params: Vec::new(),
            returns: None,
            body: Some(Vec::new()),
        };
        // A bare `=` with nothing under it does not parse; `discard` is
        // Nim's statement that does nothing.
        assert_eq!(proc_decl(&empty), "proc zd_noop() =\n  discard\n");
    }

    #[test]
    fn every_statement_form_indents_the_way_nim_reads() {
        let body = vec![
            Stmt::Var {
                name: Ident::new("zd_n"),
                type_: Some(Type::Primitive(Primitive::UInt32)),
                value: Some(Expr::call(
                    Expr::dot(Expr::name("r"), "readU32"),
                    Vec::new(),
                )),
            },
            Stmt::For {
                counter: Ident::new("zd_i"),
                bound: Box::new(Expr::name("zd_n")),
                body: vec![Stmt::Assign {
                    place: Expr::Index {
                        value: Box::new(Expr::name("v")),
                        index: Box::new(Expr::Convert {
                            type_: Box::new(Type::named("int")),
                            value: Box::new(Expr::name("zd_i")),
                        }),
                    },
                    value: Expr::int(0),
                }],
            },
            Stmt::While {
                condition: Expr::Literal(Literal::Bool(true)),
                body: vec![Stmt::Break],
            },
            Stmt::If {
                condition: Expr::name("zd_over"),
                then: vec![Stmt::Discard(Expr::name("zd_tc"))],
                otherwise: Some(vec![Stmt::Return(None)]),
            },
            Stmt::Comment(Doc::line("done")),
        ];
        let node = ProcDecl {
            doc: Doc::none(),
            exported: false,
            name: Ident::new("f"),
            params: Vec::new(),
            returns: None,
            body: Some(body),
        };
        assert_eq!(
            proc_decl(&node),
            "proc f() =\n\
             \x20 var zd_n: uint32 = r.readU32()\n\
             \x20 for zd_i in 0'u32 ..< zd_n:\n\
             \x20   v[int(zd_i)] = 0\n\
             \x20 while true:\n\
             \x20   break\n\
             \x20 if zd_over:\n\
             \x20   discard zd_tc\n\
             \x20 else:\n\
             \x20   return\n\
             \x20 # done\n"
        );
    }

    #[test]
    fn no_line_carries_trailing_whitespace_or_a_tab() {
        // Nim refuses a tab in the indentation of a line outright, and
        // trailing whitespace is what a formatter strips and a byte
        // comparison then reports as a difference.
        let text = file(&File {
            doc: Doc::lines(&["one", "", "three"]),
            imports: Vec::new(),
            sections: vec![
                Section::Type(vec![TypeDecl {
                    text: zerodds_lang_cst::nim::TypeText::default(),
                    doc: Doc::none(),
                    name: Ident::new("A"),
                    exported: true,
                    body: TypeBody::Object(vec![field("a", Type::Primitive(Primitive::Bool))]),
                }]),
                Section::Const(vec![ConstDecl {
                    doc: Doc::none(),
                    name: Ident::new("N"),
                    exported: true,
                    type_: None,
                    value: Expr::int(1),
                }]),
            ],
        });
        for line in text.lines() {
            assert_eq!(line, line.trim_end(), "trailing whitespace: {line:?}");
            assert!(!line.contains('\t'), "a tab reached the output: {line:?}");
        }
    }
}
