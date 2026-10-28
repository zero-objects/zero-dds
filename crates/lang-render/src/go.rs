// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Go CST. Spec §5.
//!
//! One style, `gofmt`'s: tabs for indentation, the opening brace on the
//! same line, `//` comments. Tabs and not spaces because `gofmt` is not
//! a preference in Go — `go vet` does not check formatting, but every
//! reader of a generated file runs it, and a file that changes under it
//! is a diff nobody asked for.
//!
//! Nothing is decided here. A Go type reads left to right and carries
//! its own length and pointer depth, so unlike
//! [`crate::c`] there is no declarator to split around a name:
//! a field is the name, a space, and the type.

use crate::writer::{Writer, comment_lines, escape_string, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::go::{
    ConstDecl, Decl, Expr, Field, File, FuncDecl, Import, InterfaceDecl, Method, Param, Stmt, Tag,
    Type, TypeDecl,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::new("\t");
    doc(&mut w, &file.doc);
    let mut package = String::from("package ");
    package.push_str(file.package.as_str());
    w.line(&package);
    if !file.imports.is_empty() {
        w.blank_line();
        render_imports(&mut w, &file.imports);
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
    let mut w = Writer::new("\t");
    render_decl(&mut w, node);
    w.finish()
}

/// Render one type, for a caller that has only one.
#[must_use]
pub fn type_(node: &Type) -> String {
    type_text(node)
}

/// A single import is written on one line; two or more take the
/// parenthesised group `gofmt` produces.
fn render_imports(w: &mut Writer, imports: &[Import]) {
    if let [only] = imports {
        let mut line = String::from("import ");
        line.push_str(&import_text(only));
        w.line(&line);
        return;
    }
    w.line("import (");
    w.indent();
    for import in imports {
        w.line(&import_text(import));
    }
    w.dedent();
    w.line(")");
}

fn import_text(import: &Import) -> String {
    let mut out = String::new();
    if let Some(alias) = &import.alias {
        out.push_str(alias.as_str());
        out.push(' ');
    }
    out.push('"');
    out.push_str(&escape_string(&import.path));
    out.push('"');
    out
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Type(decl) => render_type_decl(w, decl),
        Decl::Const(decl) => render_const(w, decl),
        Decl::Interface(decl) => render_interface(w, decl),
        Decl::Func(decl) => render_func(w, decl),
        Decl::Comment(lines) => doc(w, lines),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// `func name(params) results { body }`.
fn render_func(w: &mut Writer, node: &FuncDecl) {
    doc(w, &node.doc);
    let mut head = String::from("func ");
    head.push_str(node.name.as_str());
    head.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    head.push_str(&params.join(", "));
    head.push(')');
    match node.results.as_slice() {
        [] => {}
        [only] => {
            head.push(' ');
            head.push_str(&type_text(only));
        }
        many => {
            head.push_str(" (");
            let parts: Vec<String> = many.iter().map(type_text).collect();
            head.push_str(&parts.join(", "));
            head.push(')');
        }
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_stmts(w, &node.body);
    w.dedent();
    w.line("}");
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stmts(w: &mut Writer, body: &[Stmt]) {
    for statement in body {
        render_stmt(w, statement);
    }
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::Var { name, type_, value } => {
            let mut line;
            match (type_, value) {
                // `name := value`: the type is the initialiser's.
                (None, Some(value)) => {
                    line = String::from(name.as_str());
                    line.push_str(" := ");
                    line.push_str(&expr_text(value));
                }
                (Some(type_), value) => {
                    line = String::from("var ");
                    line.push_str(name.as_str());
                    line.push(' ');
                    line.push_str(&type_text(type_));
                    if let Some(value) = value {
                        line.push_str(" = ");
                        line.push_str(&expr_text(value));
                    }
                }
                // A declaration with neither type nor value is not Go;
                // written out so the defect names itself in the diff
                // rather than compiling into something else.
                (None, None) => {
                    line = String::from("var ");
                    line.push_str(name.as_str());
                }
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
        } => render_if(w, condition, then, otherwise.as_deref(), "if "),
        Stmt::For {
            counter,
            init,
            bound,
            body,
        } => {
            let mut line = String::from("for ");
            line.push_str(counter.as_str());
            line.push_str(" := ");
            line.push_str(&expr_text(init));
            line.push_str("; ");
            line.push_str(counter.as_str());
            line.push_str(" < ");
            line.push_str(&expr_text(bound));
            line.push_str("; ");
            line.push_str(counter.as_str());
            line.push_str("++ {");
            w.line(&line);
            w.indent();
            render_stmts(w, body);
            w.dedent();
            w.line("}");
        }
        Stmt::While { condition, body } => {
            let mut line = String::from("for ");
            line.push_str(&expr_text(condition));
            line.push_str(" {");
            w.line(&line);
            w.indent();
            render_stmts(w, body);
            w.dedent();
            w.line("}");
        }
        Stmt::Break => w.line("break"),
        Stmt::Comment(lines) => doc(w, lines),
    }
}

/// An `if` chain, with `else if` written the way `gofmt` writes it: an
/// else-branch holding exactly one `if` and nothing else continues the
/// line rather than nesting.
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
fn render_if(
    w: &mut Writer,
    condition: &Expr,
    then: &[Stmt],
    otherwise: Option<&[Stmt]>,
    head: &str,
) {
    let mut condition = condition;
    let mut then = then;
    let mut otherwise = otherwise;
    let mut head = head;
    loop {
        let mut line = String::from(head);
        line.push_str(&expr_text(condition));
        line.push_str(" {");
        w.line(&line);
        w.indent();
        render_stmts(w, then);
        w.dedent();
        match otherwise {
            None => {
                w.line("}");
                return;
            }
            Some(
                [
                    Stmt::If {
                        condition: next_condition,
                        then: next_then,
                        otherwise: next_otherwise,
                    },
                ],
            ) => {
                condition = next_condition;
                then = next_then;
                otherwise = next_otherwise.as_deref();
                head = "} else if ";
            }
            Some(branch) => {
                w.line("} else {");
                w.indent();
                render_stmts(w, branch);
                w.dedent();
                w.line("}");
                return;
            }
        }
    }
}

fn render_type_decl(w: &mut Writer, node: &TypeDecl) {
    doc(w, &node.doc);
    let mut head = String::from("type ");
    head.push_str(node.name.as_str());
    head.push(' ');
    if node.alias {
        head.push_str("= ");
    }
    match &node.type_ {
        Type::Struct(fields) => {
            head.push_str("struct {");
            w.line(&head);
            w.indent();
            for text in &node.body_text.begin {
                verbatim_lines(w, text);
            }
            render_fields(w, fields);
            for text in &node.body_text.end {
                verbatim_lines(w, text);
            }
            w.dedent();
            w.line("}");
        }
        other => {
            head.push_str(&type_text(other));
            w.line(&head);
        }
    }
}

/// A struct's fields, with the type and tag columns aligned.
///
/// `gofmt` aligns the cells of consecutive field lines, and a generated
/// file that changes under `gofmt` is a diff nobody asked for, on a file
/// nobody edited. So the renderer aligns them itself, and
/// `the_generated_go_is_gofmt_stable` in `go_end_to_end.rs` is what says
/// it got the rule right rather than plausible.
///
/// The rule, measured against `gofmt` over the whole corpus: pad the
/// name to the widest name and the type to the widest type **within a
/// run**, with the single space `gofmt` leaves between columns. A field
/// carrying a documentation comment **starts a new run** — `go/printer`
/// writes through a `text/tabwriter`, and a line with fewer cells than
/// the block (a comment line has one) terminates the column block. Taking
/// the widths over the whole field list instead left fourteen of the
/// corpus's four hundred and eleven files changing under `gofmt`, all of
/// them structs with an `@optional` member, which is the member this
/// column documents.
///
/// What is deliberately not implemented is `go/printer`'s outlier rule,
/// which starts a new section when one cell is far wider than the
/// running mean. Nothing this column emits reaches it: a field's name is
/// an IDL identifier and its type is a Go type spelled inline.
fn render_fields(w: &mut Writer, fields: &[Field]) {
    let mut run: Vec<&Field> = Vec::new();
    for field in fields {
        if !field.doc.is_empty() && !run.is_empty() {
            flush_run(w, &run);
            run.clear();
        }
        run.push(field);
    }
    flush_run(w, &run);
}

/// One alignment block: the widths are this run's and no wider.
fn flush_run(w: &mut Writer, run: &[&Field]) {
    let name_width = run
        .iter()
        .map(|field| field.name.as_str().chars().count())
        .max()
        .unwrap_or(0);
    let type_width = run
        .iter()
        .filter(|field| field.tag.is_some())
        .map(|field| type_text(&field.type_).chars().count())
        .max()
        .unwrap_or(0);
    for field in run {
        render_field(w, field, name_width, type_width);
    }
}

fn render_field(w: &mut Writer, node: &Field, name_width: usize, type_width: usize) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    pad_to(&mut line, name_width);
    line.push(' ');
    let type_ = type_text(&node.type_);
    line.push_str(&type_);
    if let Some(tag) = &node.tag {
        pad_to(&mut line, name_width + 1 + type_width);
        line.push(' ');
        line.push_str(&tag_text(tag));
    }
    w.line(&line);
}

/// Pad with spaces until the line is `width` characters wide.
///
/// Characters and not bytes: an identifier the IDL wrote is ASCII
/// (IDL 4.2 §7.2.3), but a type name reached through a facet need not
/// be, and `gofmt` counts what it prints.
fn pad_to(line: &mut String, width: usize) {
    while line.chars().count() < width {
        line.push(' ');
    }
}

/// A struct tag, in the raw-string form `gofmt` and `reflect` expect.
///
/// Back quotes rather than a double-quoted literal: a tag is read back
/// with `reflect.StructTag.Get`, whose documented convention is
/// `key:"value"` pairs, and writing that inside a double-quoted string
/// means escaping every quote. The raw form cannot carry a back quote,
/// and nothing put here can hold one — an IDL identifier is
/// `[A-Za-z_][A-Za-z0-9_]*` (IDL 4.2 §7.2.3) and the keys are this
/// column's own.
fn tag_text(tag: &Tag) -> String {
    let mut out = String::from("`");
    for (index, (key, value)) in tag.entries.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(key);
        out.push_str(":\"");
        out.push_str(value);
        out.push('"');
    }
    out.push('`');
    out
}

fn render_const(w: &mut Writer, node: &ConstDecl) {
    doc(w, &node.doc);
    let mut line = String::from("const ");
    line.push_str(node.name.as_str());
    if let Some(type_) = &node.type_ {
        line.push(' ');
        line.push_str(&type_text(type_));
    }
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

fn render_interface(w: &mut Writer, node: &InterfaceDecl) {
    doc(w, &node.doc);
    let mut head = String::from("type ");
    head.push_str(node.name.as_str());
    head.push_str(" interface {");
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
    let mut line = String::from(node.name.as_str());
    line.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    line.push_str(&params.join(", "));
    line.push(')');
    match node.results.as_slice() {
        [] => {}
        [only] => {
            line.push(' ');
            line.push_str(&type_text(only));
        }
        many => {
            line.push_str(" (");
            let parts: Vec<String> = many.iter().map(type_text).collect();
            line.push_str(&parts.join(", "));
            line.push(')');
        }
    }
    w.line(&line);
}

fn param_text(param: &Param) -> String {
    let mut out = String::new();
    if let Some(name) = &param.name {
        out.push_str(name.as_str());
        out.push(' ');
    }
    out.push_str(&type_text(&param.type_));
    out
}

/// A struct type written inline, for a field whose type is a struct.
///
/// Reached only through [`type_text`]; a top-level `type T struct` is
/// written by [`render_type_decl`], which can indent it against the
/// declaration.
fn struct_text(fields: &[Field]) -> String {
    if fields.is_empty() {
        return "struct{}".to_string();
    }
    let mut w = Writer::new("\t");
    w.line("struct {");
    w.indent();
    render_fields(&mut w, fields);
    w.dedent();
    w.push("}");
    w.finish().trim_end().to_string()
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/go.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::Slice(element) => {
            let mut out = String::from("[]");
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
        Type::Pointer(target) => {
            let mut out = String::from("*");
            out.push_str(&type_text(target));
            out
        }
        Type::Struct(fields) => struct_text(fields),
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_go.rs`, and
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
        Expr::Convert { type_, value } => {
            let mut out = type_text(type_);
            out.push('(');
            out.push_str(&expr_text(value));
            out.push(')');
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
        Expr::Selector { value, name } => {
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
        Expr::SliceTo { value, high } => {
            let mut out = expr_text(value);
            out.push_str("[:");
            out.push_str(&expr_text(high));
            out.push(']');
            out
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
            out
        }
        Expr::Composite { type_, elems } => {
            let mut out = match type_ {
                Some(type_) => type_text(type_),
                None => String::new(),
            };
            out.push('{');
            let parts: Vec<String> = elems
                .iter()
                .map(|elem| match &elem.key {
                    Some(key) => {
                        let mut part = String::from(key.as_str());
                        part.push_str(": ");
                        part.push_str(&expr_text(&elem.value));
                        part
                    }
                    None => expr_text(&elem.value),
                })
                .collect();
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Expr::Make { type_, length } => {
            let mut out = String::from("make(");
            out.push_str(&type_text(type_));
            out.push_str(", ");
            out.push_str(&expr_text(length));
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
        // A Go rune literal is a code point and not an octet, so a
        // `char` above the execution set is still one literal. The
        // numeric form is written rather than the quoted one: a rune
        // literal's type is `rune`, which is `int32`, and an IDL `char`
        // is one octet — the conversion the member's type asks for is
        // then explicit at the declaration rather than implicit in a
        // literal that happens to fit.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
        Literal::Bytes(bytes) => {
            let mut out = String::from("[]byte{");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Literal::Null => "nil".to_string(),
    }
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "// ", doc.as_lines());
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}
