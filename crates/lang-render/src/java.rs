// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Java CST. Spec §5.
//!
//! One style, the Oracle convention: four spaces, the opening brace on
//! the same line, one member per line with a blank line between methods.

use crate::writer::{Writer, comment_lines, escape_char, escape_string, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::java::{
    Annotation, AnnotationArg, AnnotationElement, AnnotationType, Block, Class, Constructor, Enum,
    EnumConstant, Expr, Field, File, Import, Interface, Member, Method, Modifier, Param, Record,
    Statement, SwitchCase, Type, TypeDecl, TypeParam,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    doc(&mut w, &file.doc);
    if let Some(package) = &file.package {
        let mut line = String::from("package ");
        line.push_str(&path_text(package));
        line.push(';');
        w.line(&line);
        w.blank_line();
    }
    for import in &file.imports {
        w.line(&import_text(import));
    }
    if !file.imports.is_empty() {
        w.blank_line();
    }
    for (index, decl) in file.types.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_type_decl(&mut w, decl);
    }
    w.finish()
}

/// Render one type declaration, for a caller that has only one.
#[must_use]
pub fn type_decl(node: &TypeDecl) -> String {
    let mut w = Writer::spaces4();
    render_type_decl(&mut w, node);
    w.finish()
}

/// Render one type, for a caller that has only one.
#[must_use]
pub fn type_(node: &Type) -> String {
    type_text(node)
}

fn import_text(import: &Import) -> String {
    let mut out = String::from("import ");
    if import.static_ {
        out.push_str("static ");
    }
    out.push_str(&path_text(&import.path));
    if import.wildcard {
        out.push_str(".*");
    }
    out.push(';');
    out
}

fn render_type_decl(w: &mut Writer, node: &TypeDecl) {
    match node {
        TypeDecl::Class(class) => render_class(w, class),
        TypeDecl::Interface(interface) => render_interface(w, interface),
        TypeDecl::Enum(e) => render_enum(w, e),
        TypeDecl::Record(record) => render_record(w, record),
        TypeDecl::AnnotationType(annotation) => render_annotation_type(w, annotation),
        TypeDecl::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_class(w: &mut Writer, node: &Class) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("class ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    if let Some(extends) = &node.extends {
        head.push_str(" extends ");
        head.push_str(&type_text(extends));
    }
    if !node.implements.is_empty() {
        head.push_str(" implements ");
        let parts: Vec<String> = node.implements.iter().map(type_text).collect();
        head.push_str(&parts.join(", "));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_members(w, &node.members);
    w.dedent();
    w.line("}");
}

fn render_interface(w: &mut Writer, node: &Interface) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("interface ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    if !node.extends.is_empty() {
        head.push_str(" extends ");
        let parts: Vec<String> = node.extends.iter().map(type_text).collect();
        head.push_str(&parts.join(", "));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_members(w, &node.members);
    w.dedent();
    w.line("}");
}

fn render_enum(w: &mut Writer, node: &Enum) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("enum ");
    head.push_str(node.name.as_str());
    if !node.implements.is_empty() {
        head.push_str(" implements ");
        let parts: Vec<String> = node.implements.iter().map(type_text).collect();
        head.push_str(&parts.join(", "));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for (index, constant) in node.constants.iter().enumerate() {
        let last = index + 1 == node.constants.len();
        render_enum_constant(w, constant, last, node.members.is_empty());
    }
    if !node.members.is_empty() {
        w.blank_line();
        render_members(w, &node.members);
    }
    w.dedent();
    w.line("}");
}

/// The constants end in `;` when members follow and in nothing when they
/// do not — which is what the Java grammar requires and what a reader
/// checking the output against the language expects.
fn render_enum_constant(w: &mut Writer, node: &EnumConstant, last: bool, no_members: bool) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut line = String::from(node.name.as_str());
    if !node.args.is_empty() {
        line.push('(');
        let parts: Vec<String> = node.args.iter().map(expr_text).collect();
        line.push_str(&parts.join(", "));
        line.push(')');
    }
    if !node.body.is_empty() {
        line.push_str(" {");
        w.line(&line);
        w.indent();
        render_members(w, &node.body);
        w.dedent();
        w.line(if last { "}" } else { "}," });
        return;
    }
    if last {
        if !no_members {
            line.push(';');
        }
    } else {
        line.push(',');
    }
    w.line(&line);
}

fn render_record(w: &mut Writer, node: &Record) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("record ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push('(');
    head.push_str(&params_text(&node.components));
    head.push(')');
    if !node.implements.is_empty() {
        head.push_str(" implements ");
        let parts: Vec<String> = node.implements.iter().map(type_text).collect();
        head.push_str(&parts.join(", "));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_members(w, &node.members);
    w.dedent();
    w.line("}");
}

fn render_annotation_type(w: &mut Writer, node: &AnnotationType) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("@interface ");
    head.push_str(node.name.as_str());
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for element in &node.elements {
        render_annotation_element(w, element);
    }
    w.dedent();
    w.line("}");
}

fn render_annotation_element(w: &mut Writer, node: &AnnotationElement) {
    doc(w, &node.doc);
    let mut line = type_text(&node.type_);
    line.push(' ');
    line.push_str(node.name.as_str());
    line.push_str("()");
    if let Some(default) = &node.default {
        line.push_str(" default ");
        line.push_str(&expr_text(default));
    }
    line.push(';');
    w.line(&line);
}

/// Fields run together; anything with a body gets a blank line before it.
fn render_members(w: &mut Writer, members: &[Member]) {
    let mut previous_had_body = false;
    for (index, member) in members.iter().enumerate() {
        let has_body = !matches!(member, Member::Field(_) | Member::Comment(_));
        if index > 0 && (has_body || previous_had_body) {
            w.blank_line();
        }
        render_member(w, member);
        previous_had_body = has_body;
    }
}

fn render_member(w: &mut Writer, node: &Member) {
    match node {
        Member::Field(field) => render_field(w, field),
        Member::Method(method) => render_method(w, method),
        Member::Constructor(constructor) => render_constructor(w, constructor),
        Member::Nested(decl) => render_type_decl(w, decl),
        Member::Initialiser { static_, body } => {
            w.line(if *static_ { "static {" } else { "{" });
            w.indent();
            render_block(w, body);
            w.dedent();
            w.line("}");
        }
        Member::Comment(c) => doc(w, c),
        Member::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut line = modifiers_text(&node.modifiers);
    line.push_str(&type_text(&node.type_));
    line.push(' ');
    line.push_str(node.name.as_str());
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(';');
    w.line(&line);
}

fn render_method(w: &mut Writer, node: &Method) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut head = modifiers_text(&node.modifiers);
    let generics = generics_text(&node.generics);
    if !generics.is_empty() {
        head.push_str(&generics);
        head.push(' ');
    }
    head.push_str(&type_text(&node.returns));
    head.push(' ');
    head.push_str(node.name.as_str());
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if !node.throws.is_empty() {
        head.push_str(" throws ");
        let parts: Vec<String> = node.throws.iter().map(type_text).collect();
        head.push_str(&parts.join(", "));
    }
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

fn render_constructor(w: &mut Writer, node: &Constructor) {
    doc(w, &node.doc);
    annotations(w, &node.annotations);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str(node.name.as_str());
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if !node.throws.is_empty() {
        head.push_str(" throws ");
        let parts: Vec<String> = node.throws.iter().map(type_text).collect();
        head.push_str(&parts.join(", "));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_block(w, &node.body);
    w.dedent();
    w.line("}");
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Java nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_block(w: &mut Writer, block: &Block) {
    for statement in &block.statements {
        render_statement(w, statement);
    }
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Java nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Declare {
            final_,
            type_,
            name,
            value,
        } => {
            let mut line = String::new();
            if *final_ {
                line.push_str("final ");
            }
            line.push_str(&type_text(type_));
            line.push(' ');
            line.push_str(name.as_str());
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
        Statement::If {
            condition,
            then,
            otherwise,
        } => {
            let mut head = String::from("if (");
            head.push_str(&expr_text(condition));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            render_block(w, then);
            w.dedent();
            match otherwise {
                Some(block) => {
                    w.line("} else {");
                    w.indent();
                    render_block(w, block);
                    w.dedent();
                    w.line("}");
                }
                None => w.line("}"),
            }
        }
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
        Statement::ForEach {
            type_,
            binding,
            range,
            body,
        } => {
            let mut head = String::from("for (");
            head.push_str(&type_text(type_));
            head.push(' ');
            head.push_str(binding.as_str());
            head.push_str(" : ");
            head.push_str(&expr_text(range));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            render_block(w, body);
            w.dedent();
            w.line("}");
        }
        Statement::Switch { value, cases } => {
            let mut head = String::from("switch (");
            head.push_str(&expr_text(value));
            head.push_str(") {");
            w.line(&head);
            for case in cases {
                render_case(w, case);
            }
            w.line("}");
        }
        Statement::Throw(value) => {
            let mut line = String::from("throw ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Statement::Comment(c) => doc(w, c),
    }
}

fn render_case(w: &mut Writer, case: &SwitchCase) {
    if case.labels.is_empty() {
        w.line("default:");
    } else {
        for label in &case.labels {
            let mut line = String::from("case ");
            line.push_str(&expr_text(label));
            line.push(':');
            w.line(&line);
        }
    }
    w.indent();
    render_block(w, &case.body);
    if case.breaks {
        w.line("break;");
    }
    w.dedent();
}

fn modifiers_text(modifiers: &[Modifier]) -> String {
    let mut out = String::new();
    for modifier in modifiers {
        out.push_str(modifier.as_str());
        out.push(' ');
    }
    out
}

fn annotations(w: &mut Writer, annotations: &[Annotation]) {
    for annotation in annotations {
        w.line(&annotation_text(annotation));
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting; an
/// annotation value nests only as deep as the IDL's annotation application,
/// and `expr_text` above states why no number is claimed.
fn annotation_text(annotation: &Annotation) -> String {
    let mut out = String::from("@");
    out.push_str(&path_text(&annotation.path));
    if !annotation.args.is_empty() {
        out.push('(');
        let parts: Vec<String> = annotation
            .args
            .iter()
            .map(|arg| match arg {
                AnnotationArg::Value(value) => expr_text(value),
                AnnotationArg::Pair { name, value } => {
                    let mut text = name.as_str().to_string();
                    text.push_str(" = ");
                    text.push_str(&expr_text(value));
                    text
                }
            })
            .collect();
        out.push_str(&parts.join(", "));
        out.push(')');
    }
    out
}

fn generics_text(params: &[TypeParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::from("<");
    let parts: Vec<String> = params
        .iter()
        .map(|param| {
            let mut text = param.name.as_str().to_string();
            if !param.bounds.is_empty() {
                text.push_str(" extends ");
                let bounds: Vec<String> = param.bounds.iter().map(type_text).collect();
                text.push_str(&bounds.join(" & "));
            }
            text
        })
        .collect();
    out.push_str(&parts.join(", "));
    out.push('>');
    out
}

fn params_text(params: &[Param]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|param| {
            let mut text = String::new();
            for annotation in &param.annotations {
                text.push_str(&annotation_text(annotation));
                text.push(' ');
            }
            if param.final_ {
                text.push_str("final ");
            }
            text.push_str(&type_text(&param.type_));
            if param.varargs {
                text.push_str("...");
            }
            text.push(' ');
            text.push_str(param.name.as_str());
            text
        })
        .collect();
    parts.join(", ")
}

fn path_text(path: &Path) -> String {
    let parts: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
    parts.join(".")
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/adapter/java.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named { path, args } => {
            let mut out = path_text(path);
            if !args.is_empty() {
                out.push('<');
                let parts: Vec<String> = args.iter().map(type_text).collect();
                out.push_str(&parts.join(", "));
                out.push('>');
            }
            out
        }
        Type::Array(element) => {
            let mut out = type_text(element);
            out.push_str("[]");
            out
        }
        Type::Wildcard { extends, super_ } => {
            let mut out = String::from("?");
            if let Some(bound) = extends {
                out.push_str(" extends ");
                out.push_str(&type_text(bound));
            } else if let Some(bound) = super_ {
                out.push_str(" super ");
                out.push_str(&type_text(bound));
            }
            out
        }
        Type::Void => "void".to_string(),
        Type::Var => "var".to_string(),
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_java.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
#[allow(clippy::too_many_lines)]
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Path(path) => path_text(path),
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let mut out = String::new();
            if let Some(receiver) = receiver {
                out.push_str(&expr_text(receiver));
                out.push('.');
            }
            out.push_str(name.as_str());
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
        Expr::New { type_, args } => {
            let mut out = String::from("new ");
            out.push_str(&type_text(type_));
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::NewArray {
            element,
            lengths,
            elements,
        } => {
            // The element's own brackets go behind the sized ones:
            // `new int[n][]` is `n` rows of `int[]`, and `new int[][n]`
            // is not a form Java has.
            let mut base = element.as_ref();
            let mut trailing = 0;
            while let Type::Array(inner) = base {
                base = inner;
                trailing += 1;
            }
            if lengths.is_empty() {
                let mut out = String::from("new ");
                out.push_str(&type_text(element));
                out.push_str("[]{");
                let parts: Vec<String> = elements.iter().map(expr_text).collect();
                out.push_str(&parts.join(", "));
                out.push('}');
                return out;
            }
            let mut out = String::from("new ");
            out.push_str(&type_text(base));
            for length in lengths {
                out.push('[');
                out.push_str(&expr_text(length));
                out.push(']');
            }
            for _ in 0..trailing {
                out.push_str("[]");
            }
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
        Expr::Paren(value) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(value));
            out.push(')');
            out
        }
        Expr::Cast { type_, value } => {
            let mut out = String::from("(");
            out.push_str(&type_text(type_));
            out.push_str(") ");
            out.push_str(&expr_text(value));
            out
        }
        Expr::InstanceOf { value, type_ } => {
            let mut out = expr_text(value);
            out.push_str(" instanceof ");
            out.push_str(&type_text(type_));
            out
        }
        Expr::Ternary {
            condition,
            then,
            otherwise,
        } => {
            let mut out = expr_text(condition);
            out.push_str(" ? ");
            out.push_str(&expr_text(then));
            out.push_str(" : ");
            out.push_str(&expr_text(otherwise));
            out
        }
        Expr::This => "this".to_string(),
        Expr::Lambda { params, body } => {
            let mut out = String::from("(");
            let parts: Vec<&str> = params.iter().map(Ident::as_str).collect();
            out.push_str(&parts.join(", "));
            out.push_str(") -> ");
            out.push_str(&expr_text(body));
            out
        }
        Expr::Annotation(annotation) => annotation_text(annotation),
        Expr::ArrayInit(elements) => {
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            let mut out = String::from("{");
            out.push_str(&parts.join(", "));
            out.push('}');
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
            let mut out = String::from("new byte[]{");
            let parts: Vec<String> = bytes
                .iter()
                .map(|b| alloc::format!("(byte) 0x{b:02X}"))
                .collect();
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Literal::Null => "null".to_string(),
    }
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    // Javadoc, which is what the convention and the tooling expect.
    w.line("/**");
    comment_lines(w, " * ", doc.as_lines());
    w.line(" */");
}

/// A statement as it appears inside a `for` clause: no semicolon, no
/// line of its own.
///
/// Only the two forms a `for` header can hold. Anything else returns
/// nothing rather than a half-rendered statement, which would put a
/// silently empty initialiser in generated source.
fn statement_inline(node: &Statement) -> String {
    match node {
        Statement::Declare {
            final_,
            type_,
            name,
            value,
        } => {
            let mut out = String::new();
            if *final_ {
                out.push_str("final ");
            }
            out.push_str(&type_text(type_));
            out.push(' ');
            out.push_str(name.as_str());
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
