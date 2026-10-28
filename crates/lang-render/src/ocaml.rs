// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the OCaml CST. Spec §5.
//!
//! One style, the one `ocamlformat` produces with its defaults: two
//! spaces per level, a leading `|` on every variant constructor and
//! every match arm, `(** … *)` for documentation.
//!
//! Two things are decided here that are not decided in the sibling
//! renderers, and both are lexical:
//!
//! 1. **A comment's text is sanitised.** OCaml comments nest *and*
//!    contain string literals: the manual's "Lexical conventions"
//!    require a comment to hold well-formed strings, and `ocamlc`
//!    reports `This comment contains an unterminated string literal`
//!    for a `(* … " … *)`. A `*)` inside a comment closes it early and
//!    a `(*` opens a nested one that then never closes. None of those
//!    is a hazard in C, Go, Rust or Java, and a documentation comment
//!    here is assembled from IDL text. [`comment_safe`] is where the
//!    three are neutralised.
//! 2. **Types are postfix and left-associative.** `int32 array option`
//!    is an option of an array; `int32 option array` is an array of
//!    options. Nesting therefore needs no brackets at all — except
//!    around a tuple, which is why [`Type::Tuple`] always renders
//!    parenthesised.
//!
//! Nothing else is decided. What is a group and what is a single
//! declaration is settled in `zerodds_lang_rules::adapter::ocaml`,
//! because it is a property of the type graph and not of the text.

use crate::writer::{Writer, escape_string, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::ocaml::{
    Arm, ClassTypeDecl, Constructor, Decl, Expr, Field, File, LetDecl, Method, Pattern, Type,
    TypeBody, TypeDecl, TypeGroup,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::new("  ");
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
    let mut w = Writer::new("  ");
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
        Decl::Types(group) => render_group(w, group),
        Decl::Let(binding) => render_let(w, binding),
        Decl::ClassType(class) => render_class_type(w, class),
        Decl::Comment(lines) => doc(w, lines),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// `type a = … and b = …`.
///
/// The keyword changes at the first item and only there. A group of one
/// is an ordinary declaration; a group of more is the only way OCaml
/// writes a cycle, because a type name resolves against what precedes it
/// and against its own group and nothing else.
fn render_group(w: &mut Writer, group: &TypeGroup) {
    doc(w, &group.doc);
    for (index, item) in group.items.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_type_decl(w, item, if index == 0 { "type" } else { "and" });
    }
}

fn render_type_decl(w: &mut Writer, node: &TypeDecl, keyword: &str) {
    doc(w, &node.doc);
    let mut head = String::from(keyword);
    head.push(' ');
    head.push_str(node.name.as_str());
    match &node.body {
        TypeBody::Abstract => w.line(&head),
        TypeBody::Alias(target) => {
            head.push_str(" = ");
            head.push_str(&type_text(target));
            w.line(&head);
        }
        TypeBody::Record(fields) => {
            head.push_str(" = {");
            w.line(&head);
            w.indent();
            for text in &node.body_text.begin {
                verbatim_lines(w, text);
            }
            for field in fields {
                render_field(w, field);
            }
            for text in &node.body_text.end {
                verbatim_lines(w, text);
            }
            w.dedent();
            w.line("}");
        }
        TypeBody::Variant(constructors) => {
            head.push_str(" =");
            w.line(&head);
            w.indent();
            for constructor in constructors {
                render_constructor(w, constructor);
            }
            w.dedent();
        }
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    line.push_str(" : ");
    line.push_str(&type_text(&node.type_));
    // A trailing `;` on the last field too. OCaml permits it, and a
    // field added later is then a one-line diff rather than two.
    line.push(';');
    w.line(&line);
}

fn render_constructor(w: &mut Writer, node: &Constructor) {
    doc(w, &node.doc);
    let mut line = String::from("| ");
    line.push_str(node.name.as_str());
    if !node.args.is_empty() {
        line.push_str(" of ");
        let parts: Vec<String> = node.args.iter().map(type_text).collect();
        line.push_str(&parts.join(" * "));
    }
    w.line(&line);
}

fn render_let(w: &mut Writer, node: &LetDecl) {
    doc(w, &node.doc);
    let mut head = String::from("let ");
    head.push_str(node.name.as_str());
    if let Some(type_) = &node.type_ {
        head.push_str(" : ");
        head.push_str(&type_text(type_));
    }
    head.push_str(" = ");
    match &node.body {
        // A `function` is the one expression that spans lines: its arms
        // are written one per line so that an enumeration with fifty
        // enumerators is readable and diffs one arm at a time.
        Expr::Function(arms) => {
            head.push_str("function");
            w.line(&head);
            w.indent();
            for arm in arms {
                render_arm(w, arm);
            }
            w.dedent();
        }
        other => {
            head.push_str(&expr_text(other));
            w.line(&head);
        }
    }
}

fn render_arm(w: &mut Writer, node: &Arm) {
    let mut line = String::from("| ");
    line.push_str(&pattern_text(&node.pattern));
    line.push_str(" -> ");
    line.push_str(&expr_text(&node.body));
    w.line(&line);
}

fn render_class_type(w: &mut Writer, node: &ClassTypeDecl) {
    doc(w, &node.doc);
    let mut head = String::from("class type ");
    head.push_str(node.name.as_str());
    head.push_str(" = object");
    if node.methods.is_empty() {
        // `object end` on one line. An interface declaring no operation
        // is legal IDL and `object\nend` would read as an omission.
        head.push_str(" end");
        w.line(&head);
        return;
    }
    w.line(&head);
    w.indent();
    for method in &node.methods {
        render_method(w, method);
    }
    w.dedent();
    w.line("end");
}

fn render_method(w: &mut Writer, node: &Method) {
    doc(w, &node.doc);
    let mut line = String::from("method ");
    line.push_str(node.name.as_str());
    line.push_str(" : ");
    for param in &node.params {
        line.push_str(&type_text(param));
        line.push_str(" -> ");
    }
    line.push_str(&type_text(&node.result));
    w.line(&line);
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/ocaml.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        // Postfix and left-associative, so no bracket is needed at any
        // depth: `int32 array array` is an array of arrays and
        // `int32 array option` an option of an array.
        Type::Array(element) => {
            let mut out = type_text(element);
            out.push_str(" array");
            out
        }
        Type::Option(inner) => {
            let mut out = type_text(inner);
            out.push_str(" option");
            out
        }
        // Always bracketed. `a * b array` is `a * (b array)`, so a tuple
        // written without brackets changes meaning the moment it appears
        // under a postfix constructor.
        Type::Tuple(parts) => {
            let mut out = String::from("(");
            let rendered: Vec<String> = parts.iter().map(type_text).collect();
            out.push_str(&rendered.join(" * "));
            out.push(')');
            out
        }
        Type::Qualified { module, name } => {
            let mut out = String::new();
            for segment in module {
                out.push_str(segment.as_str());
                out.push('.');
            }
            out.push_str(name.as_str());
            out
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_ocaml.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => name.as_str().to_string(),
        Expr::Construct(name) => name.as_str().to_string(),
        Expr::Apply { func, args } => {
            let mut out = expr_text(func);
            for arg in args {
                out.push(' ');
                out.push_str(&expr_text(arg));
            }
            out
        }
        // Reached only where a `function` is not the whole right-hand
        // side of a `let`; [`render_let`] writes that case over several
        // lines.
        Expr::Function(arms) => {
            let mut out = String::from("function ");
            let rendered: Vec<String> = arms
                .iter()
                .map(|arm| {
                    let mut text = pattern_text(&arm.pattern);
                    text.push_str(" -> ");
                    text.push_str(&expr_text(&arm.body));
                    text
                })
                .collect();
            out.push_str(&rendered.join(" | "));
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
    }
}

fn pattern_text(node: &Pattern) -> String {
    match node {
        Pattern::Literal(literal) => literal_text(literal),
        Pattern::Construct(name) | Pattern::Bind(name) => name.as_str().to_string(),
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        // The suffix an `int32` or an `int64` needs is already in the
        // text: `zerodds_lang_cst::ocaml::int32_literal` puts it there,
        // because the width is a property of the value's type and not of
        // the rendering.
        Literal::Int(text) | Literal::Float(text) => text.clone(),
        Literal::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // An OCaml `char` is one octet, and an IDL `char` maps onto
        // `int` in this column, so a character literal is written as its
        // code point. `'\xff'` would be the other spelling and would be
        // a `char` where the member is an `int`.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
        // OCaml has no byte-string literal; a `string` holds arbitrary
        // octets, so the escaped form is the byte string.
        Literal::Bytes(bytes) => {
            let mut out = String::from("\"");
            for byte in bytes {
                out.push_str(&alloc::format!("\\x{byte:02x}"));
            }
            out.push('"');
            out
        }
        Literal::Null => "None".to_string(),
    }
}

/// Write a documentation comment, or nothing when there is none.
///
/// One `(** … *)` block for the whole comment rather than one per line.
/// Several consecutive documentation comments attached to one
/// declaration are what `ocamlc`'s warning 50 is about, and a block is
/// also what a reader gets from `ocamldoc`.
fn doc(w: &mut Writer, doc: &Doc) {
    let lines = doc.as_lines();
    let Some((first, rest)) = lines.split_first() else {
        return;
    };
    if rest.is_empty() {
        let mut only = String::from("(** ");
        only.push_str(&comment_safe(first));
        only.push_str(" *)");
        w.line(&only);
        return;
    }
    let mut head = String::from("(** ");
    head.push_str(&comment_safe(first));
    w.line(&head);
    for (index, line) in rest.iter().enumerate() {
        let last = index + 1 == rest.len();
        // A blank line inside the block gets no indentation: trailing
        // whitespace is what a formatter strips and what a byte
        // comparison then reports as a difference.
        if line.is_empty() && !last {
            w.line("");
            continue;
        }
        let mut text = String::from("    ");
        text.push_str(&comment_safe(line));
        if last {
            text.push_str(" *)");
        }
        w.line(&text);
    }
}

/// Text that cannot end, nest or unbalance the comment holding it.
///
/// Three substitutions, and each is a way an OCaml comment differs from
/// a `//` or a `/* */` one:
///
/// - `"` becomes `'`. A comment must contain well-formed string
///   literals (OCaml manual, "Lexical conventions"); `ocamlc` reports
///   `This comment contains an unterminated string literal` and refuses
///   the file. A C or Go comment has no such rule.
/// - `*)` becomes `* )`, because it would close the comment early and
///   leave the rest of the sentence as source.
/// - `(*` becomes `( *`, because OCaml comments nest and it would open
///   one that never closes.
#[must_use]
pub fn comment_safe(text: &str) -> String {
    text.replace('"', "'")
        .replace("*)", "* )")
        .replace("(*", "( *")
}

// -- the codec layer -------------------------------------------------------
//
// Rendering `zerodds_lang_cst::ocaml`'s codec nodes. Unlike the
// declaration layer above, this half carries a typing environment: OCaml
// has no implicit conversion between `int`, `int32` and `int64`, no
// statements, no `return` and no `break`, so what a C renderer writes
// down verbatim has to be *typed* here before it can be spelled. The
// environment is built from the declarations the body itself makes —
// every local is a `ref` cell with a declared `CodecType` — plus the
// function's own parameters, and the runtime's signatures are the table
// below.
//
// A store shape this renderer does not know is rendered as the unbound
// name `zd_unsupported_store`, so the generated file fails to compile
// naming the site — the same loudness the Python backend chooses for a
// walk that changed underneath it.

use alloc::format;
use zerodds_lang_cst::ocaml::{CodeExpr, CodecFn, Stmt, UNSIGNED_HOLDER, ValueFn, ValueKind};

/// How a name in scope is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvForm {
    /// A parameter or loop counter: read as `x`.
    Plain,
    /// A `ref` cell: read as `!x`, stored as `x := …`.
    Cell,
    /// The discarded scratch entry of an uncounted map's counting pass:
    /// stores into it are `ignore`.
    Scratch,
}

/// The typing environment: name, kind, form. A `Vec` and not a map —
/// bodies hold tens of names — with the *latest* binding winning, which
/// is OCaml's own shadowing order.
type Env = Vec<(String, ValueKind, EnvForm)>;

fn env_get<'a>(env: &'a Env, name: &str) -> Option<(&'a ValueKind, EnvForm)> {
    env.iter()
        .rev()
        .find(|(bound, _, _)| bound == name)
        .map(|(_, kind, form)| (kind, *form))
}

/// The runtime's signature for one operation: the parameter kinds after
/// the receiver, and the result kind. The authority is
/// `runtime/ocaml/zdwire.ml`; an operation missing here renders with
/// its arguments unconverted, and `ocamlc` names the mismatch.
#[allow(clippy::too_many_lines)]
fn runtime_signature(method: &str) -> Option<(&'static [ValueKind], ValueKind)> {
    use ValueKind as K;
    const I: K = K::Int;
    const I64: K = K::Int64;
    const I32: K = K::Int32;
    const F: K = K::Float;
    const B: K = K::Bool;
    const T: K = K::Text;
    const U: K = K::UnitValue;
    Some(match method {
        "write_bool" => (&[B], U),
        "write_u8" | "write_i8" | "write_char" | "write_i16" | "write_u16" | "write_u32" => {
            (&[I], U)
        }
        "write_wchar" => (&[I, I, T, T], U),
        "write_i32" => (&[I32], U),
        "write_i64" => (&[I64], U),
        "write_u64" | "write_string" | "write_f128" => (&[T], U),
        "write_string_bounded" => (&[T, I], U),
        "write_f32" | "write_f64" => (&[F], U),
        "write_wstring_utf8" => (&[T, T, T], U),
        "write_wstring_bounded" => (&[T, I, T, T], U),
        "check_sequence_bound" | "check_map_bound" => (&[I, I], U),
        "write_fixed" => (&[T, I, I, T], U),
        "write_enum" => (&[I, I64], U),
        "write_bitmask" | "write_bitset" => (&[I, I64], U),
        "write_union_overlay" => (&[I64, I], U),
        "bitfield" => (&[I64, I, I], I64),
        "write_dheader" => (&[], K::Handle("dheader")),
        "finish_dheader" => (&[K::Handle("dheader")], U),
        "write_emheader" => (&[I, I, B], K::Handle("emheader")),
        "finish_emheader" => (&[K::Handle("emheader")], U),
        "write_pid" | "write_pid_extended" => (&[I, K::Handle("role"), B], K::Handle("pid")),
        "finish_pid" => (&[K::Handle("pid")], U),
        "write_sentinel" => (&[], U),
        "read_bool" => (&[], B),
        "read_u8" | "read_i8" | "read_char" | "read_i16" | "read_u16" | "read_u32" => (&[], I),
        "read_wchar" => (&[I, T, T], I),
        "read_i32" => (&[], I32),
        "read_i64" => (&[], I64),
        "read_u64" | "read_string_alloc" | "read_f128" => (&[], T),
        "read_f32" | "read_f64" => (&[], F),
        "read_wstring_utf8" => (&[T, T], T),
        "read_fixed" => (&[I, I, T], T),
        "read_enum" | "read_bitmask" => (&[I], I64),
        "read_bitset" => (&[I], I64),
        "read_dheader" | "reader_position" | "reader_remaining" => (&[], I),
        "reader_status" => (&[], K::Handle("status")),
        "reader_refuse" => (&[K::Handle("status")], U),
        "reader_clear_discard" => (&[], B),
        "reader_skip_to" => (&[I], U),
        "reader_save" => (&[], K::Handle("reader_mark")),
        "reader_restore" => (&[K::Handle("reader_mark")], U),
        "read_pid" => (&[], K::Handle("pid_read")),
        "read_pid_as" => (&[K::Handle("role")], K::Handle("pid_read")),
        "finish_pid_read" => (&[K::Handle("pid_read")], U),
        "read_emheader" => (&[], K::Handle("emheader_read")),
        "trim_utf8" | "trim_utf16" => (&[T, I], T),
        // The free functions, reached with no receiver.
        "no_dheader" => (&[], K::Handle("dheader")),
        "utf8_octets" | "utf16_units" => (&[T], I),
        "pid_is_user_member" => (&[K::Handle("pid_read")], B),
        "bit_pattern_int" => (&[I64, I], I64),
        "bit_pattern_bool" => (&[B], I64),
        "bit_pattern_f32" | "bit_pattern_f64" => (&[F], I64),
        "u64_less" | "strcmp" => (&[T, T], if method == "strcmp" { I } else { B }),
        "u64_of_dec" => (&[T], I64),
        _ => return None,
    })
}

/// The runtime's field for a walk-named header field, per handle kind.
fn header_field_name(kind: &ValueKind, field: &str) -> String {
    let mapped = match (kind, field) {
        (ValueKind::Handle("emheader_read"), "member_id") => "em_member_id",
        (ValueKind::Handle("emheader_read"), "length_code") => "em_length_code",
        (ValueKind::Handle("emheader_read"), "must_understand") => "em_must_understand",
        (ValueKind::Handle("emheader_read"), "extent") => "em_extent",
        (ValueKind::Handle("pid_read"), "pid") => "pr_pid",
        (ValueKind::Handle("pid_read"), "length") => "pr_length",
        (ValueKind::Handle("pid_read"), "end") => "pr_end",
        (ValueKind::Handle("pid_read"), "must_understand") => "pr_must_understand",
        (ValueKind::Handle("pid_read"), "ignore") => "pr_ignore",
        // A field on a header the environment could not type: the
        // unbound name makes `ocamlc` report the site.
        _ => return format!("zd_unknown_header_field_{field}"),
    };
    mapped.to_string()
}

fn header_field_kind(field: &str) -> ValueKind {
    match field {
        "must_understand" | "end" | "ignore" => ValueKind::Bool,
        _ => ValueKind::Int,
    }
}

/// A literal integer in the spelling `kind` reads.
fn literal_in(value: i128, kind: &ValueKind) -> String {
    match kind {
        ValueKind::Int32 => format!("({value}l)"),
        ValueKind::Int64 => format!("({value}L)"),
        ValueKind::Text => format!("\"{value}\""),
        ValueKind::Bool => (if value != 0 { "true" } else { "false" }).to_string(),
        ValueKind::Float => format!("({value}.0)"),
        ValueKind::Enum(name) => format!("({name}_of_int32 ({value}l))"),
        _ => {
            if value < 0 {
                format!("({value})")
            } else {
                format!("{value}")
            }
        }
    }
}

/// `text` (of kind `from`) as a value of kind `to`, through the one
/// conversion OCaml has for the pair. Kinds with no conversion between
/// them pass through unchanged: the pair agrees by construction or
/// `ocamlc` names the site.
fn convert(text: String, from: &ValueKind, to: &ValueKind) -> String {
    use ValueKind as K;
    if from == to {
        return text;
    }
    match (from, to) {
        (K::Int, K::Int64) => format!("(Int64.of_int {text})"),
        (K::Int, K::Int32) => format!("(Int32.of_int {text})"),
        (K::Int32, K::Int64) => format!("(Int64.of_int32 {text})"),
        (K::Int32, K::Int) => format!("(Int32.to_int {text})"),
        (K::Int64, K::Int) => format!("(Int64.to_int {text})"),
        (K::Int64, K::Int32) => format!("(Int64.to_int32 {text})"),
        (K::Enum(name), K::Int64) => format!("(Int64.of_int32 (int32_of_{name} {text}))"),
        (K::Enum(name), K::Int32) => format!("(int32_of_{name} {text})"),
        (K::Enum(name), K::Int) => format!("(Int32.to_int (int32_of_{name} {text}))"),
        (K::Int32, K::Enum(name)) => format!("({name}_of_int32 {text})"),
        (K::Int, K::Enum(name)) => format!("({name}_of_int32 (Int32.of_int {text}))"),
        (K::Int64, K::Enum(name)) => format!("({name}_of_int32 (Int64.to_int32 {text}))"),
        // The decimal holder of an `unsigned long long` against its
        // 64-bit pattern.
        (K::Text, K::Int64) => format!("(Zdwire.u64_of_dec {text})"),
        (K::Int64, K::Text) => format!("(Zdwire.u64_to_dec {text})"),
        _ => text,
    }
}

/// The kind an expression naturally has, before any expectation.
///
/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_ocaml.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn natural_kind(env: &Env, expr: &CodeExpr) -> ValueKind {
    match expr {
        CodeExpr::Name(name) => {
            env_get(env, name).map_or(ValueKind::Aggregate, |(kind, _)| kind.clone())
        }
        CodeExpr::Whole(name) => {
            env_get(env, name).map_or(ValueKind::Aggregate, |(kind, _)| kind.clone())
        }
        CodeExpr::Int(_) => ValueKind::Int,
        CodeExpr::Int64Lit(_) => ValueKind::Int64,
        CodeExpr::BoolLit(_) => ValueKind::Bool,
        CodeExpr::TextLit(_) => ValueKind::Text,
        CodeExpr::FloatLit(_) => ValueKind::Float,
        CodeExpr::Runtime { method, .. } | CodeExpr::Static { method, .. } => {
            runtime_signature(method).map_or(ValueKind::Aggregate, |(_, ret)| ret)
        }
        CodeExpr::HeaderField { field, .. } => header_field_kind(field),
        CodeExpr::CountOf(_) => ValueKind::Int,
        CodeExpr::IsSome { .. } => ValueKind::Bool,
        CodeExpr::Absent => ValueKind::Aggregate,
        CodeExpr::Binary { op, lhs, rhs } => match *op {
            "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" => ValueKind::Bool,
            // An arithmetic node's kind is its operands' joint one —
            // whichever side knows, a bare literal deferring to the
            // other.
            _ => {
                let left = natural_kind(env, lhs);
                if left == ValueKind::Aggregate || matches!(lhs.as_ref(), CodeExpr::Int(_)) {
                    natural_kind(env, rhs)
                } else {
                    left
                }
            }
        },
        CodeExpr::Unary { op, value } => {
            if *op == "!" {
                ValueKind::Bool
            } else {
                natural_kind(env, value)
            }
        }
        CodeExpr::Paren(inner) => natural_kind(env, inner),
        CodeExpr::Convert { target, .. } => target.kind.clone(),
        CodeExpr::RuntimeConst { kind, .. } | CodeExpr::Raw { kind, .. } => kind.clone(),
        // The structural kinds: a member access, a subscript and an
        // entry half read their answer off the base's kind tree, which
        // is how a bitset field's `Int64.of_int` and a union arm's
        // `int32_of_<enum>` get their operand kind without the model.
        CodeExpr::Member { base, field, case } => {
            let record = env_get(env, base).map(|(kind, _)| kind.clone());
            // A field with dots is a path through the members that hold a base
            // (`parent.x`, XTypes 1.3 §7.5.1.4.1.3).
            let field_kind = field.split('.').fold(record, |kind, segment| match kind {
                Some(ValueKind::RecordOf(fields)) => fields
                    .iter()
                    .find(|(name, _)| name == segment)
                    .map(|(_, kind)| kind.clone()),
                _ => None,
            });
            let field_kind = field_kind.unwrap_or(ValueKind::Aggregate);
            if *case {
                match field_kind {
                    ValueKind::OptionOf(inner) => *inner,
                    other => other,
                }
            } else {
                field_kind
            }
        }
        CodeExpr::OptionGet(inner) => match natural_kind(env, inner) {
            ValueKind::OptionOf(kind) => *kind,
            other => other,
        },
        CodeExpr::Index { base, .. } => match natural_kind(env, base) {
            ValueKind::ArrayOf(element) => *element,
            _ => ValueKind::Aggregate,
        },
        CodeExpr::EntryHalf { entry, key } => match natural_kind(env, entry) {
            ValueKind::RecordOf(fields) => {
                let half = if *key { "key" } else { "value" };
                fields
                    .iter()
                    .find(|(name, _)| name == half)
                    .map_or(ValueKind::Aggregate, |(_, kind)| kind.clone())
            }
            _ => ValueKind::Aggregate,
        },
        CodeExpr::Elements(base) => {
            env_get(env, base).map_or(ValueKind::Aggregate, |(kind, _)| kind.clone())
        }
        CodeExpr::OptionSlot { .. } | CodeExpr::Construct(_) | CodeExpr::Call { .. } => {
            ValueKind::Aggregate
        }
    }
}

/// The value a name reads as.
fn name_value(env: &Env, name: &str) -> String {
    match env_get(env, name) {
        Some((_, EnvForm::Cell)) => format!("(!{name})"),
        _ => name.to_string(),
    }
}

/// An OCaml string literal of `text`'s bytes.
fn ocaml_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            byte if !(0x20..0x7F).contains(&byte) => out.push_str(&format!("\\x{byte:02x}")),
            byte => out.push(byte as char),
        }
    }
    out.push('"');
    out
}

/// The arithmetic spelling of `op` over `kind`.
fn arithmetic(op: &str, kind: &ValueKind, lhs: &str, rhs: &str) -> String {
    match kind {
        ValueKind::Int64 => {
            let name = match op {
                "+" => "Int64.add",
                "-" => "Int64.sub",
                "*" => "Int64.mul",
                "/" => "Int64.div",
                "%" => "Int64.rem",
                "&" => "Int64.logand",
                "|" => "Int64.logor",
                "^" => "Int64.logxor",
                "<<" => "Int64.shift_left",
                ">>" => "Int64.shift_right_logical",
                other => other,
            };
            format!("({name} {lhs} {rhs})")
        }
        ValueKind::Float => {
            let symbol = match op {
                "+" => "+.",
                "-" => "-.",
                "*" => "*.",
                "/" => "/.",
                other => other,
            };
            format!("({lhs} {symbol} {rhs})")
        }
        _ => {
            let symbol = match op {
                "%" => "mod",
                "&" => "land",
                "|" => "lor",
                "^" => "lxor",
                "<<" => "lsl",
                ">>" => "lsr",
                other => other,
            };
            format!("({lhs} {symbol} {rhs})")
        }
    }
}

/// Render one expression as a value, converted into `expected` where
/// one is given.
///
/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_ocaml.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
#[allow(clippy::too_many_lines)]
fn value(env: &Env, expr: &CodeExpr, expected: Option<&ValueKind>) -> String {
    // A widthless literal takes the expected width directly, with no
    // conversion function around it.
    if let (CodeExpr::Int(number), Some(kind)) = (expr, expected) {
        return literal_in(*number, kind);
    }
    // An `int64` literal a site wants as an `int` — a shift distance —
    // is the digits, not a conversion around a suffix.
    if let (CodeExpr::Int64Lit(number), Some(ValueKind::Int)) = (expr, expected) {
        return literal_in(i128::from(*number), &ValueKind::Int);
    }
    let natural = natural_kind(env, expr);
    let text = match expr {
        CodeExpr::Name(name) => name_value(env, name),
        CodeExpr::Whole(name) => name_value(env, name),
        CodeExpr::Int(number) => {
            if *number < 0 {
                format!("({number})")
            } else {
                format!("{number}")
            }
        }
        CodeExpr::Int64Lit(number) => format!("({number}L)"),
        CodeExpr::BoolLit(flag) => (if *flag { "true" } else { "false" }).to_string(),
        CodeExpr::TextLit(text) => ocaml_string_literal(text),
        CodeExpr::FloatLit(text) => format!("({text})"),
        CodeExpr::Runtime {
            receiver,
            method,
            args,
        } => {
            let mut out = format!("(Zdwire.{method} {receiver}");
            let params = runtime_signature(method).map(|(params, _)| params);
            for (index, arg) in args.iter().enumerate() {
                let expected = params.and_then(|params| params.get(index));
                out.push(' ');
                out.push_str(&value(env, arg, expected));
            }
            out.push(')');
            out
        }
        CodeExpr::Static { method, args } => {
            let mut out = format!("(Zdwire.{method}");
            let params = runtime_signature(method).map(|(params, _)| params);
            if args.is_empty() {
                out.push_str(" ()");
            }
            for (index, arg) in args.iter().enumerate() {
                let expected = params.and_then(|params| params.get(index));
                out.push(' ');
                out.push_str(&value(env, arg, expected));
            }
            out.push(')');
            out
        }
        CodeExpr::Call { callee, args } => {
            let mut out = format!("({callee}");
            if args.is_empty() {
                out.push_str(" ()");
            }
            for arg in args {
                out.push(' ');
                out.push_str(&value(env, arg, None));
            }
            out.push(')');
            out
        }
        CodeExpr::Member { base, field, case } => {
            let read = format!("{}.{field}", name_value(env, base));
            if *case {
                format!("(Option.get {read})")
            } else {
                read
            }
        }
        CodeExpr::OptionSlot { base, field } => {
            // Read through the marked slot: the value, unwrapped.
            format!("(Option.get {}.{field})", name_value(env, base))
        }
        CodeExpr::OptionGet(inner) => format!("(Option.get {})", value(env, inner, None)),
        CodeExpr::IsSome { base, field } => {
            format!("({}.{field} <> None)", name_value(env, base))
        }
        CodeExpr::Absent => "None".to_string(),
        CodeExpr::Index { base, index } => format!(
            "{}.({})",
            value(env, base, None),
            value(env, index, Some(&ValueKind::Int))
        ),
        CodeExpr::EntryHalf { entry, key } => format!(
            "{}.{}",
            value(env, entry, None),
            if *key { "key" } else { "value" }
        ),
        CodeExpr::HeaderField {
            value: header,
            field,
        } => {
            let kind = natural_kind(env, header);
            format!(
                "{}.Zdwire.{}",
                value(env, header, None),
                header_field_name(&kind, field)
            )
        }
        CodeExpr::CountOf(base) => format!("(Array.length {})", name_value(env, base)),
        CodeExpr::Elements(base) => name_value(env, base),
        CodeExpr::Construct(name) => name.clone(),
        CodeExpr::Binary { op, lhs, rhs } => match *op {
            "&&" | "||" => format!(
                "({} {op} {})",
                value(env, lhs, Some(&ValueKind::Bool)),
                value(env, rhs, Some(&ValueKind::Bool))
            ),
            "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                // The comparison's operand kind: whichever side knows.
                let left_kind = natural_kind(env, lhs);
                let right_kind = natural_kind(env, rhs);
                let joint = if left_kind == ValueKind::Aggregate
                    || matches!(lhs.as_ref(), CodeExpr::Int(_))
                {
                    right_kind
                } else {
                    left_kind
                };
                let expect = if joint == ValueKind::Aggregate {
                    None
                } else {
                    Some(&joint)
                };
                let symbol = match *op {
                    "==" => "=",
                    "!=" => "<>",
                    other => other,
                };
                format!(
                    "({} {symbol} {})",
                    value(env, lhs, expect),
                    value(env, rhs, expect)
                )
            }
            op => {
                // Arithmetic: the operand kind is whichever side knows,
                // and a shift's distance is an `int` whatever the value
                // is.
                let left_kind = natural_kind(env, lhs);
                let right_kind = natural_kind(env, rhs);
                let joint = if left_kind == ValueKind::Aggregate
                    || matches!(lhs.as_ref(), CodeExpr::Int(_))
                {
                    right_kind.clone()
                } else {
                    left_kind.clone()
                };
                let shift = op == "<<" || op == ">>";
                let right_expect = if shift && joint == ValueKind::Int64 {
                    ValueKind::Int
                } else {
                    joint.clone()
                };
                arithmetic(
                    op,
                    &joint,
                    &value(env, lhs, Some(&joint)),
                    &value(env, rhs, Some(&right_expect)),
                )
            }
        },
        CodeExpr::Unary { op, value: operand } => match *op {
            "!" => format!("(not {})", value(env, operand, Some(&ValueKind::Bool))),
            "~" => {
                let kind = natural_kind(env, operand);
                if kind == ValueKind::Int64 {
                    format!("(Int64.lognot {})", value(env, operand, None))
                } else {
                    format!("(lnot {})", value(env, operand, None))
                }
            }
            op => format!("({op} {})", value(env, operand, None)),
        },
        CodeExpr::Paren(inner) => value(env, inner, None),
        CodeExpr::Convert {
            target,
            value: operand,
        } => {
            if target.text == "signed char" {
                // C's signed reading of one code-point octet, spelled
                // as arithmetic: (((cp + 128) mod 256) - 128).
                let inner = value(env, operand, Some(&ValueKind::Int));
                format!("((({inner} + 128) mod 256) - 128)")
            } else if let CodeExpr::Int(number) = operand.as_ref() {
                literal_in(*number, &target.kind)
            } else if target.text == UNSIGNED_HOLDER {
                // The walk's two unsigned widenings: `write_bitmask`'s
                // pattern and `bitfield`'s value. A bitmask of at most
                // 32 bits is held in an `int32` — Table 33 gives the
                // width and OCaml has no unsigned integer — so a bound
                // of 32 with bit 31 set is negative there, and
                // `Int64.of_int32` would set bits 32..63 as well. The
                // runtime then measures that against 2^32 and refuses a
                // value that is inside the bound. Masking to the
                // holder's width is the widening that keeps the
                // pattern.
                let from = natural_kind(env, operand);
                let inner = value(env, operand, None);
                match from {
                    ValueKind::Int32 => {
                        format!("(Int64.logand (Int64.of_int32 {inner}) 0xFFFFFFFFL)")
                    }
                    other => convert(inner, &other, &ValueKind::Int64),
                }
            } else {
                let from = natural_kind(env, operand);
                convert(value(env, operand, None), &from, &target.kind)
            }
        }
        CodeExpr::RuntimeConst { text, .. } => text.clone(),
        CodeExpr::Raw { text, .. } => text.clone(),
    };
    match expected {
        Some(kind) if *kind != ValueKind::Aggregate => convert(text, &natural, kind),
        _ => text,
    }
}

/// The functional update of a record held in the cell `base`: `{ !v with x = e }`, and for a
/// field with dots, a path through the members that hold a base (`parent.x`, XTypes 1.3
/// §7.5.1.4.1.3), `{ !v with parent = { (!v).parent with x = e } }`.
fn record_update(base: &str, field: &str, value_text: &str) -> String {
    /// zerodds-lint: recursion-depth = the dots of a field path, the depth of the base chain.
    fn update(record: &str, segments: &[&str], value_text: &str) -> String {
        match segments {
            [] => value_text.to_string(),
            [last] => format!("{{ {record} with {last} = {value_text} }}"),
            [head, rest @ ..] => {
                let inner = update(&format!("({record}).{head}"), rest, value_text);
                format!("{{ {record} with {head} = {inner} }}")
            }
        }
    }
    let segments: Vec<&str> = field.split('.').collect();
    update(&format!("!{base}"), &segments, value_text)
}

/// One store, in the shape the place calls for.
fn store(env: &Env, place: &CodeExpr, value_text: &str, value_is_absent: bool) -> String {
    match place {
        CodeExpr::Whole(name) | CodeExpr::Name(name) => match env_get(env, name) {
            Some((_, EnvForm::Scratch)) => format!("ignore {value_text};"),
            _ => format!("{name} := {value_text};"),
        },
        CodeExpr::Member { base, field, case } => {
            let wrapped = if *case && !value_is_absent {
                format!("Some {value_text}")
            } else {
                value_text.to_string()
            };
            format!("{base} := {};", record_update(base, field, &wrapped))
        }
        CodeExpr::OptionSlot { base, field } => {
            let wrapped = if value_is_absent {
                value_text.to_string()
            } else {
                format!("Some {value_text}")
            };
            format!("{base} := {};", record_update(base, field, &wrapped))
        }
        CodeExpr::Index { base, index } => format!(
            "{}.({}) <- {value_text};",
            value(env, base, None),
            value(env, index, Some(&ValueKind::Int))
        ),
        CodeExpr::EntryHalf { entry, key } => {
            let half = if *key { "key" } else { "value" };
            match entry.as_ref() {
                // A store into the discarded scratch entry consumes the
                // stream and keeps nothing.
                CodeExpr::Name(name)
                    if matches!(env_get(env, name), Some((_, EnvForm::Scratch))) =>
                {
                    format!("ignore {value_text};")
                }
                CodeExpr::Index { base, index } => {
                    let array = value(env, base, None);
                    let at = value(env, index, Some(&ValueKind::Int));
                    format!("{array}.({at}) <- {{ {array}.({at}) with {half} = {value_text} }};")
                }
                _ => format!("zd_unsupported_store {value_text};"),
            }
        }
        _ => format!("zd_unsupported_store {value_text};"),
    }
}

/// The kind a store's place expects, where one is knowable.
fn place_kind(env: &Env, place: &CodeExpr) -> Option<ValueKind> {
    match place {
        CodeExpr::Whole(name) | CodeExpr::Name(name) => {
            env_get(env, name).map(|(kind, _)| kind.clone())
        }
        _ => None,
    }
}

/// Append `indent` spaces of depth `depth`.
fn pad(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// Render a block of statements as a `unit` expression body, ending in
/// `()` so the sequence is closed whatever the walk emitted.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and OCaml nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
#[allow(clippy::too_many_lines)]
fn block(env: &mut Env, statements: &[Stmt], depth: usize, out: &mut String) {
    for statement in statements {
        match statement {
            Stmt::Let {
                type_,
                name,
                value: init,
            } => {
                let rendered = match init {
                    Some(expr) => value(env, expr, Some(&type_.kind)),
                    None => format!("({})", type_.default),
                };
                pad(out, depth);
                out.push_str(&format!(
                    "let {name} : {} ref = ref {rendered} in\n",
                    type_.text
                ));
                env.push((name.clone(), type_.kind.clone(), EnvForm::Cell));
            }
            Stmt::Scratch { name } => {
                pad(out, depth);
                out.push_str(&format!(
                    "(* {name}: the counting pass reads and discards *)\n"
                ));
                env.push((name.clone(), ValueKind::Aggregate, EnvForm::Scratch));
            }
            Stmt::Assign {
                place,
                value: given,
            } => {
                let expected = place_kind(env, place);
                let text = value(env, given, expected.as_ref());
                pad(out, depth);
                out.push_str(&store(env, place, &text, matches!(given, CodeExpr::Absent)));
                out.push('\n');
            }
            Stmt::Store {
                place,
                type_,
                value: given,
            } => {
                let text = value(env, given, Some(&type_.kind));
                pad(out, depth);
                out.push_str(&store(env, place, &text, matches!(given, CodeExpr::Absent)));
                out.push('\n');
            }
            Stmt::ReadInto {
                callee,
                place,
                behaviour,
            } => {
                let call = format!("({callee} r {})", value(env, behaviour, None));
                pad(out, depth);
                out.push_str(&store(env, place, &call, false));
                out.push('\n');
            }
            Stmt::Perform(expr) => {
                let unit_valued = match expr {
                    CodeExpr::Runtime { method, .. } | CodeExpr::Static { method, .. } => {
                        runtime_signature(method)
                            .is_some_and(|(_, ret)| ret == ValueKind::UnitValue)
                    }
                    // A generated encoder answers unit; nothing else is
                    // performed bare.
                    CodeExpr::Call { .. } => true,
                    _ => false,
                };
                pad(out, depth);
                if unit_valued {
                    out.push_str(&format!("{};\n", value(env, expr, None)));
                } else {
                    out.push_str(&format!("ignore {};\n", value(env, expr, None)));
                }
            }
            Stmt::Discard(expr) => {
                pad(out, depth);
                out.push_str(&format!("ignore {};\n", value(env, expr, None)));
            }
            Stmt::Return => {
                pad(out, depth);
                out.push_str("raise Zdwire.Zd_return;\n");
            }
            Stmt::When {
                condition,
                then,
                otherwise,
            } => {
                pad(out, depth);
                out.push_str(&format!(
                    "if {} then begin\n",
                    value(env, condition, Some(&ValueKind::Bool))
                ));
                let mut inner = env.clone();
                block(&mut inner, then, depth + 1, out);
                match otherwise {
                    Some(other) => {
                        pad(out, depth);
                        out.push_str("end\n");
                        pad(out, depth);
                        out.push_str("else begin\n");
                        let mut inner = env.clone();
                        block(&mut inner, other, depth + 1, out);
                        pad(out, depth);
                        out.push_str("end;\n");
                    }
                    None => {
                        pad(out, depth);
                        out.push_str("end;\n");
                    }
                }
            }
            Stmt::For {
                counter,
                bound,
                body,
            } => {
                pad(out, depth);
                out.push_str("(try\n");
                pad(out, depth + 1);
                out.push_str(&format!(
                    "for {counter} = 0 to {} - 1 do\n",
                    value(env, bound, Some(&ValueKind::Int))
                ));
                let mut inner = env.clone();
                inner.push((counter.clone(), ValueKind::Int, EnvForm::Plain));
                block(&mut inner, body, depth + 2, out);
                pad(out, depth + 1);
                out.push_str("done\n");
                pad(out, depth);
                out.push_str("with Zdwire.Zd_break -> ());\n");
            }
            Stmt::While { condition, body } => {
                pad(out, depth);
                out.push_str("(try\n");
                pad(out, depth + 1);
                out.push_str(&format!(
                    "while {} do\n",
                    value(env, condition, Some(&ValueKind::Bool))
                ));
                let mut inner = env.clone();
                block(&mut inner, body, depth + 2, out);
                pad(out, depth + 1);
                out.push_str("done\n");
                pad(out, depth);
                out.push_str("with Zdwire.Zd_break -> ());\n");
            }
            Stmt::Leave => {
                pad(out, depth);
                out.push_str("raise Zdwire.Zd_break;\n");
            }
            Stmt::Note(doc) => {
                for line in doc.as_lines() {
                    pad(out, depth);
                    out.push_str(&format!("(* {} *)\n", comment_safe(line.as_str())));
                }
            }
            Stmt::Alloc {
                base,
                element,
                count,
            } => {
                pad(out, depth);
                out.push_str(&format!(
                    "{base} := Array.make {} ({});\n",
                    value(env, count, Some(&ValueKind::Int)),
                    element.default
                ));
            }
            Stmt::KeepFirst { base, count } => {
                pad(out, depth);
                out.push_str(&format!(
                    "{base} := Array.sub (!{base}) 0 {};\n",
                    value(env, count, Some(&ValueKind::Int))
                ));
            }
        }
    }
    pad(out, depth);
    out.push_str("()\n");
}

/// One codec function, under `keyword` (`let rec` or `and`).
fn codec_fn(function: &CodecFn, keyword: &str, out: &mut String) {
    for line in function.doc.as_lines() {
        out.push_str(&format!("(* {} *)\n", comment_safe(line.as_str())));
    }
    let mut env: Env = Vec::new();
    let mut head = format!("{keyword} {}", function.name);
    if function.reading {
        head.push_str(" (r : Zdwire.reader)");
        env.push(("r".to_string(), ValueKind::Handle("reader"), EnvForm::Plain));
    } else {
        head.push_str(" (w : Zdwire.writer)");
        env.push(("w".to_string(), ValueKind::Handle("writer"), EnvForm::Plain));
        head.push_str(&format!(" (v : {})", function.sample.text));
        env.push((
            "v".to_string(),
            function.sample.kind.clone(),
            EnvForm::Plain,
        ));
    }
    for (type_, name) in &function.extra {
        head.push_str(&format!(" ({name} : {})", type_.text));
        env.push((name.clone(), type_.kind.clone(), EnvForm::Plain));
    }
    if function.reading {
        head.push_str(&format!(" : {} =\n", function.sample.text));
        out.push_str(&head);
        let seed = function
            .defaulter
            .clone()
            .unwrap_or_else(|| function.sample.default.clone());
        // Seeded from Table 9, so every position the stream does not
        // carry is already what the clause says it is.
        out.push_str(&format!("  let v = ref ({seed} ()) in\n"));
        env.push(("v".to_string(), function.sample.kind.clone(), EnvForm::Cell));
        out.push_str("  (try\n");
        block(&mut env, &function.body, 2, out);
        out.push_str("  with Zdwire.Zd_return -> ());\n");
        out.push_str("  !v\n");
    } else {
        head.push_str(" : unit =\n");
        out.push_str(&head);
        block(&mut env, &function.body, 1, out);
    }
}

/// A chain of codec functions: `let rec f … and g …`. One chain, so a
/// recursive or mutually recursive type's codecs resolve whatever order
/// the walk visited them in.
#[must_use]
pub fn codec_chain(functions: &[CodecFn]) -> String {
    let mut out = String::new();
    for (index, function) in functions.iter().enumerate() {
        out.push('\n');
        codec_fn(
            function,
            if index == 0 { "let rec" } else { "and" },
            &mut out,
        );
    }
    out
}

/// A chain of value functions — the Table 9 defaulters.
#[must_use]
pub fn value_chain(functions: &[ValueFn]) -> String {
    let mut out = String::new();
    for (index, function) in functions.iter().enumerate() {
        out.push('\n');
        for line in function.doc.as_lines() {
            out.push_str(&format!("(* {} *)\n", comment_safe(line.as_str())));
        }
        let keyword = if index == 0 { "let rec" } else { "and" };
        out.push_str(&format!(
            "{keyword} {} () : {} =\n  {}\n",
            function.name, function.result, function.body
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use zerodds_lang_cst::ocaml::{Primitive, int32_literal};

    fn record(name: &str, fields: Vec<Field>) -> TypeDecl {
        TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new(name),
            body: TypeBody::Record(fields),
        }
    }

    fn field(name: &str, type_: Type) -> Field {
        Field {
            doc: Doc::none(),
            name: Ident::new(name),
            type_,
        }
    }

    #[test]
    fn a_record_writes_one_field_per_line_with_a_trailing_semicolon() {
        let text = decl(&Decl::Types(TypeGroup::one(record(
            "pose",
            alloc::vec![
                field("x", Type::Primitive(Primitive::Int32)),
                field("y", Type::Primitive(Primitive::Float)),
            ],
        ))));
        assert_eq!(text, "type pose = {\n  x : int32;\n  y : float;\n}\n");
    }

    #[test]
    fn a_group_of_two_joins_them_with_and() {
        // The only way OCaml writes a cycle: a type name resolves
        // against what precedes it and against its own group.
        let group = TypeGroup {
            doc: Doc::none(),
            items: alloc::vec![
                record("a", alloc::vec![field("b", Type::named("b"))]),
                record("b", alloc::vec![field("a", Type::named("a"))]),
            ],
        };
        let text = decl(&Decl::Types(group));
        assert!(text.starts_with("type a = {"), "{text}");
        assert!(text.contains("\nand b = {"), "{text}");
    }

    #[test]
    fn a_postfix_type_needs_no_brackets_at_any_depth() {
        let nested = Type::option(Type::array(Type::array(Type::Primitive(Primitive::Int32))));
        assert_eq!(type_(&nested), "int32 array array option");
        let other = Type::array(Type::option(Type::Primitive(Primitive::String)));
        assert_eq!(type_(&other), "string option array");
    }

    #[test]
    fn a_tuple_is_always_bracketed_because_star_binds_loosest() {
        let tuple = Type::array(Type::Tuple(alloc::vec![
            Type::Primitive(Primitive::Int32),
            Type::Primitive(Primitive::String),
        ]));
        assert_eq!(type_(&tuple), "(int32 * string) array");
    }

    #[test]
    fn a_variant_writes_a_leading_bar_per_constructor() {
        let text = decl(&Decl::Types(TypeGroup::one(TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("mode"),
            body: TypeBody::Variant(alloc::vec![
                Constructor {
                    doc: Doc::none(),
                    name: Ident::new("IDLE"),
                    args: Vec::new(),
                },
                Constructor {
                    doc: Doc::none(),
                    name: Ident::new("ACTIVE"),
                    args: Vec::new(),
                },
            ]),
        })));
        assert_eq!(text, "type mode =\n  | IDLE\n  | ACTIVE\n");
    }

    #[test]
    fn a_function_binding_writes_one_arm_per_line() {
        let text = decl(&Decl::Let(LetDecl {
            doc: Doc::none(),
            name: Ident::new("int32_of_mode"),
            type_: None,
            body: Expr::Function(alloc::vec![
                Arm {
                    pattern: Pattern::Construct(Ident::new("IDLE")),
                    body: Expr::Literal(int32_literal(0)),
                },
                Arm {
                    pattern: Pattern::Construct(Ident::new("ACTIVE")),
                    body: Expr::Literal(int32_literal(4)),
                },
            ]),
        }));
        assert_eq!(
            text,
            "let int32_of_mode = function\n  | IDLE -> 0l\n  | ACTIVE -> 4l\n"
        );
    }

    #[test]
    fn an_abstract_type_writes_only_its_name() {
        let text = decl(&Decl::Types(TypeGroup::one(TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("handle"),
            body: TypeBody::Abstract,
        })));
        assert_eq!(text, "type handle\n");
    }

    #[test]
    fn an_empty_class_type_stays_on_one_line() {
        let text = decl(&Decl::ClassType(ClassTypeDecl {
            doc: Doc::none(),
            name: Ident::new("nothing"),
            methods: Vec::new(),
        }));
        assert_eq!(text, "class type nothing = object end\n");
    }

    #[test]
    fn a_method_writes_its_parameters_as_arrows() {
        let text = decl(&Decl::ClassType(ClassTypeDecl {
            doc: Doc::none(),
            name: Ident::new("control"),
            methods: alloc::vec![
                Method {
                    doc: Doc::none(),
                    name: Ident::new("go"),
                    params: alloc::vec![Type::named("pose")],
                    result: Type::Primitive(Primitive::Unit),
                },
                Method {
                    doc: Doc::none(),
                    name: Ident::new("status"),
                    params: Vec::new(),
                    result: Type::Primitive(Primitive::Int32),
                },
            ],
        }));
        assert!(text.contains("method go : pose -> unit"), "{text}");
        assert!(text.contains("method status : int32"), "{text}");
    }

    #[test]
    fn a_quote_in_a_comment_is_replaced_because_ocamlc_refuses_the_file() {
        // `ocamlc` reports "This comment contains an unterminated string
        // literal" and stops. No other column in this crate has that
        // hazard.
        assert_eq!(comment_safe("the \"wire\" form"), "the 'wire' form");
    }

    #[test]
    fn a_comment_delimiter_inside_a_comment_is_neutralised() {
        assert_eq!(
            comment_safe("ends here *) and then"),
            "ends here * ) and then"
        );
        assert_eq!(comment_safe("opens (* here"), "opens ( * here");
    }

    #[test]
    fn a_multi_line_comment_is_one_block_and_not_one_block_per_line() {
        let mut w = Writer::new("  ");
        doc(&mut w, &Doc::lines(&["first", "second"]));
        assert_eq!(w.finish(), "(** first\n    second *)\n");
    }
}
