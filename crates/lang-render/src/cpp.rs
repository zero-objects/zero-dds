// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the C++ CST. Spec §5.
//!
//! One style, the one the emitters this replaces wrote: four spaces, the
//! opening brace on the same line, access labels at the class's own
//! indentation, and a namespace body not indented — which is what the
//! fixture output shows.

use crate::c::include_text;
use crate::writer::{Writer, comment_lines, escape_char, escape_string, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::cpp::{
    Alias, Base, Block, CastKind, Class, ClassMember, Constructor, Decl, Destructor, Enum,
    Enumerator, Expr, Field, File, Function, FunctionBody, Method, Namespace, Param, Section,
    Statement, SwitchCase, Template, TemplateParamKind, Type, Variable,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    doc(&mut w, &file.doc);
    if file.pragma_once {
        w.line("#pragma once");
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

/// Render one expression, for a caller that has only one.
///
/// What a generator building C++ by hand needs where the *value* is
/// already a tree: the read harness's union arms compare against the
/// same label the generated codec does, and the label's written form —
/// which suffix a wide one takes — is `CodecTarget::wide_int`'s answer.
/// Asking for the tree's text is how the two stay one rule rather than
/// two that agree today.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/cpp.rs`)
fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Namespace(namespace) => render_namespace(w, namespace),
        Decl::Class(class) => render_class(w, class),
        Decl::Enum(e) => render_enum(w, e),
        Decl::Alias(alias) => render_alias(w, alias),
        Decl::Function(f) => render_function(w, f),
        Decl::Variable(v) => render_variable(w, v),
        Decl::Template(t) => render_template(w, t),
        Decl::Comment(c) => doc(w, c),
        Decl::Verbatim(text) => verbatim_lines(w, text),
        Decl::Forward { is_struct, name } => {
            let mut line = String::from(if *is_struct { "struct " } else { "class " });
            line.push_str(name.as_str());
            line.push(';');
            w.line(&line);
        }
    }
}

/// A namespace body is not indented, which is what the fixture output
/// shows and what keeps deeply nested modules readable.
///
/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/cpp.rs`)
fn render_namespace(w: &mut Writer, node: &Namespace) {
    doc(w, &node.doc);
    let mut head = String::from("namespace ");
    head.push_str(node.name.as_str());
    head.push_str(" {");
    w.line(&head);
    for (index, decl) in node.decls.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_decl(w, decl);
    }
    let mut close = String::from("} // namespace ");
    close.push_str(node.name.as_str());
    w.line(&close);
}

fn render_class(w: &mut Writer, node: &Class) {
    render_class_specialised(w, node, "");
}

/// A class, with the template arguments of an explicit specialisation
/// written after its name.
///
/// `suffix` is `""` for an ordinary class and `<A, B>` for
/// `template <> struct traits<A, B>`. It is threaded rather than baked
/// into the name because a name is an [`Ident`] and `traits<::a::B>` is
/// not one.
///
/// [`Ident`]: zerodds_lang_cst::common::Ident
fn render_class_specialised(w: &mut Writer, node: &Class, suffix: &str) {
    doc(w, &node.doc);
    let mut head = String::from(if node.is_struct { "struct " } else { "class " });
    head.push_str(node.name.as_str());
    head.push_str(suffix);
    if !node.bases.is_empty() {
        head.push_str(" : ");
        let parts: Vec<String> = node.bases.iter().map(base_text).collect();
        head.push_str(&parts.join(", "));
    }
    head.push_str(" {");
    w.line(&head);
    for section in &node.sections {
        render_section(w, section);
    }
    w.line("};");
}

fn base_text(base: &Base) -> String {
    let mut out = String::from(base.access.as_str());
    out.push(' ');
    if base.virtual_ {
        out.push_str("virtual ");
    }
    out.push_str(&path_text(&base.path));
    out
}

/// The access label sits at the class's own indentation; its members are
/// indented one level.
fn render_section(w: &mut Writer, node: &Section) {
    let mut label = String::from(node.access.as_str());
    label.push(':');
    w.line(&label);
    w.indent();
    for member in &node.members {
        render_class_member(w, member);
    }
    w.dedent();
    if !node.members.is_empty() {
        w.blank_line();
    }
}

fn render_class_member(w: &mut Writer, node: &ClassMember) {
    match node {
        ClassMember::Field(field) => render_field(w, field),
        ClassMember::Method(method) => render_method(w, method),
        ClassMember::Constructor(constructor) => render_constructor(w, constructor),
        ClassMember::Destructor(destructor) => render_destructor(w, destructor),
        ClassMember::Nested(decl) => render_decl(w, decl),
        ClassMember::Alias(alias) => render_alias(w, alias),
        ClassMember::Comment(c) => doc(w, c),
        ClassMember::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    let mut line = String::new();
    if node.is_static {
        line.push_str("static ");
    }
    if node.is_constexpr {
        line.push_str("constexpr ");
    }
    line.push_str(&type_text(&node.type_));
    line.push(' ');
    line.push_str(node.name.as_str());
    for dim in &node.dims {
        line.push('[');
        line.push_str(&expr_text(dim));
        line.push(']');
    }
    if let Some(bits) = node.bits {
        line.push_str(" : ");
        line.push_str(&alloc::format!("{bits}"));
    }
    if let Some(default) = &node.default {
        line.push_str(" = ");
        line.push_str(&expr_text(default));
    }
    line.push(';');
    w.line(&line);
}

fn render_method(w: &mut Writer, node: &Method) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.is_static {
        head.push_str("static ");
    }
    if node.is_virtual {
        head.push_str("virtual ");
    }
    if node.is_constexpr {
        head.push_str("constexpr ");
    }
    head.push_str(&type_text(&node.returns));
    head.push(' ');
    head.push_str(node.name.as_str());
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if node.is_const {
        head.push_str(" const");
    }
    if node.is_noexcept {
        head.push_str(" noexcept");
    }
    if node.is_override {
        head.push_str(" override");
    }
    if node.is_pure {
        head.push_str(" = 0;");
        w.line(&head);
        return;
    }
    render_body(w, head, node.body.as_ref());
}

/// A body that fits on one line stays on one line, which is what the
/// accessors in the fixture output do.
fn render_body(w: &mut Writer, mut head: String, body: Option<&Block>) {
    let Some(block) = body else {
        head.push(';');
        w.line(&head);
        return;
    };
    if let Some(single) = one_line_body(block) {
        head.push_str(" { ");
        head.push_str(&single);
        head.push_str(" }");
        w.line(&head);
        return;
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_block(w, block);
    w.dedent();
    w.line("}");
}

/// The text of a body of exactly one simple statement.
fn one_line_body(block: &Block) -> Option<String> {
    match block.statements.as_slice() {
        [Statement::Return(Some(value))] => {
            let mut out = String::from("return ");
            out.push_str(&expr_text(value));
            out.push(';');
            Some(out)
        }
        [Statement::Expr(value)] => {
            let mut out = expr_text(value);
            out.push(';');
            Some(out)
        }
        _ => None,
    }
}

fn render_constructor(w: &mut Writer, node: &Constructor) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.is_explicit {
        head.push_str("explicit ");
    }
    if node.is_constexpr {
        head.push_str("constexpr ");
    }
    head.push_str(node.name.as_str());
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if node.is_noexcept {
        head.push_str(" noexcept");
    }

    match &node.body {
        FunctionBody::Defaulted => {
            head.push_str(" = default;");
            w.line(&head);
        }
        FunctionBody::Deleted => {
            head.push_str(" = delete;");
            w.line(&head);
        }
        FunctionBody::None => {
            head.push(';');
            w.line(&head);
        }
        FunctionBody::Block(block) => {
            if node.initialisers.is_empty() {
                render_body(w, head, Some(block));
                return;
            }
            // The initialiser list goes on its own line, indented, which
            // is the form the fixture output has.
            w.line(&head);
            w.indent();
            let parts: Vec<String> = node
                .initialisers
                .iter()
                .map(|(name, value)| {
                    let mut text = name.as_str().to_string();
                    text.push('(');
                    text.push_str(&expr_text(value));
                    text.push(')');
                    text
                })
                .collect();
            let mut line = String::from(": ");
            line.push_str(&parts.join(", "));
            if block.statements.is_empty() {
                line.push_str(" {}");
                w.line(&line);
                w.dedent();
                return;
            }
            line.push_str(" {");
            w.line(&line);
            w.indent();
            render_block(w, block);
            w.dedent();
            w.line("}");
            w.dedent();
        }
    }
}

fn render_destructor(w: &mut Writer, node: &Destructor) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.is_virtual {
        head.push_str("virtual ");
    }
    head.push('~');
    head.push_str(node.name.as_str());
    head.push_str("()");
    match &node.body {
        FunctionBody::Defaulted => {
            head.push_str(" = default;");
            w.line(&head);
        }
        FunctionBody::Deleted => {
            head.push_str(" = delete;");
            w.line(&head);
        }
        FunctionBody::None => {
            head.push(';');
            w.line(&head);
        }
        FunctionBody::Block(block) => render_body(w, head, Some(block)),
    }
}

fn render_enum(w: &mut Writer, node: &Enum) {
    doc(w, &node.doc);
    let mut head = String::from(if node.is_scoped {
        "enum class "
    } else {
        "enum "
    });
    head.push_str(node.name.as_str());
    if let Some(underlying) = &node.underlying {
        head.push_str(" : ");
        head.push_str(&type_text(underlying));
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

fn render_alias(w: &mut Writer, node: &Alias) {
    doc(w, &node.doc);
    let mut line = String::from("using ");
    line.push_str(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&type_text(&node.type_));
    line.push(';');
    w.line(&line);
}

fn render_function(w: &mut Writer, node: &Function) {
    render_function_specialised(w, node, "");
}

/// A function, with the template arguments of an explicit
/// specialisation written after its name. See
/// [`render_class_specialised`].
fn render_function_specialised(w: &mut Writer, node: &Function, suffix: &str) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.is_maybe_unused {
        head.push_str("[[maybe_unused]] ");
    }
    if node.is_static {
        head.push_str("static ");
    }
    if node.is_inline {
        head.push_str("inline ");
    }
    if node.is_constexpr {
        head.push_str("constexpr ");
    }
    head.push_str(&type_text(&node.returns));
    head.push(' ');
    head.push_str(node.name.as_str());
    head.push_str(suffix);
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if node.is_noexcept {
        head.push_str(" noexcept");
    }
    match &node.body {
        FunctionBody::Block(block) => render_body(w, head, Some(block)),
        FunctionBody::Defaulted => {
            head.push_str(" = default;");
            w.line(&head);
        }
        FunctionBody::Deleted => {
            head.push_str(" = delete;");
            w.line(&head);
        }
        FunctionBody::None => {
            head.push(';');
            w.line(&head);
        }
    }
}

fn render_variable(w: &mut Writer, node: &Variable) {
    doc(w, &node.doc);
    let mut line = String::new();
    if node.is_static {
        line.push_str("static ");
    }
    if node.is_inline {
        line.push_str("inline ");
    }
    if node.is_constexpr {
        line.push_str("constexpr ");
    }
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

/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/cpp.rs`)
fn render_template(w: &mut Writer, node: &Template) {
    doc(w, &node.doc);
    let mut head = String::from("template <");
    let parts: Vec<String> = node
        .params
        .iter()
        .map(|param| {
            let mut text = match &param.kind {
                TemplateParamKind::TypeName => "typename ".to_string(),
                TemplateParamKind::Class => "class ".to_string(),
                TemplateParamKind::NonType(type_) => {
                    let mut out = type_text(type_);
                    out.push(' ');
                    out
                }
            };
            text.push_str(param.name.as_str());
            if let Some(default) = &param.default {
                text.push_str(" = ");
                text.push_str(&type_text(default));
            }
            text
        })
        .collect();
    head.push_str(&parts.join(", "));
    head.push('>');
    w.line(&head);
    if node.specialisation.is_empty() {
        render_decl(w, &node.decl);
        return;
    }
    // `Template::specialisation` used to be built and dropped here, so
    // `template <> struct traits<::a::B>` came out as
    // `template <> struct traits`, which is a redeclaration of the
    // primary template and not a specialisation of it. Nothing in the
    // tree built one until the CORBA Annex A.1.1 traits did.
    let args: Vec<String> = node.specialisation.iter().map(type_text).collect();
    let suffix = alloc::format!("<{}>", args.join(", "));
    match node.decl.as_ref() {
        Decl::Class(class) => render_class_specialised(w, class, &suffix),
        Decl::Function(function) => render_function_specialised(w, function, &suffix),
        // A namespace, an alias, an enumeration, a variable, a comment,
        // a forward declaration and a nested template cannot carry
        // explicit specialisation arguments in C++, so there is nothing
        // to write them after and the declaration is rendered plain.
        other => render_decl(w, other),
    }
}

fn params_text(params: &[Param]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|param| {
            let mut text = type_text(&param.type_);
            if let Some(name) = &param.name {
                text.push(' ');
                text.push_str(name.as_str());
            }
            if let Some(default) = &param.default {
                text.push_str(" = ");
                text.push_str(&expr_text(default));
            }
            text
        })
        .collect();
    parts.join(", ")
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and C++ nests the arms in
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
/// The model bounds none of the three counts, and C++ nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Declare { type_, name, value } => {
            let mut line = type_text(type_);
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
            binding,
            range,
            body,
        } => {
            let mut head = String::from("for (auto& ");
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
        Statement::Try { body, handlers } => {
            w.line("try {");
            w.indent();
            render_block(w, body);
            w.dedent();
            for handler in handlers {
                let mut head = String::from("} catch (");
                match &handler.type_ {
                    Some(type_) => {
                        head.push_str("const ");
                        head.push_str(&type_text(type_));
                        head.push('&');
                        if let Some(name) = &handler.binding {
                            head.push(' ');
                            head.push_str(name.as_str());
                        }
                    }
                    None => head.push_str("..."),
                }
                head.push_str(") {");
                w.line(&head);
                w.indent();
                render_block(w, &handler.body);
                w.dedent();
            }
            w.line("}");
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

fn path_text(path: &Path) -> String {
    let parts: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
    let joined = parts.join("::");
    if path.is_rooted() {
        let mut out = String::from("::");
        out.push_str(&joined);
        return out;
    }
    joined
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/adapter/cpp.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named { path, args } => {
            let mut out = path_text(path);
            if !args.is_empty() {
                out.push('<');
                let parts: Vec<String> = args.iter().map(type_text).collect();
                out.push_str(&parts.join(", "));
                // `>>` closing two argument lists is one token in C++03
                // and two in C++11; the target is C++17, so no space.
                out.push('>');
            }
            out
        }
        Type::Pointer(target) => {
            let mut out = type_text(target);
            out.push('*');
            out
        }
        Type::Ref(target) => {
            let mut out = type_text(target);
            out.push('&');
            out
        }
        Type::RvalueRef(target) => {
            let mut out = type_text(target);
            out.push_str("&&");
            out
        }
        Type::Const(target) => {
            let mut out = String::from("const ");
            out.push_str(&type_text(target));
            out
        }
        Type::Array { element, length } => {
            let mut out = type_text(element);
            out.push('[');
            out.push_str(&expr_text(length));
            out.push(']');
            out
        }
        Type::Void => "void".to_string(),
        Type::Auto => "auto".to_string(),
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_cpp.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
#[allow(clippy::too_many_lines)]
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Path(path) => path_text(path),
        Expr::Call { callee, args } => {
            let mut out = expr_text(callee);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let mut out = expr_text(receiver);
            out.push('.');
            out.push_str(name.as_str());
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
        Expr::Construct {
            type_,
            args,
            braced,
        } => {
            let mut out = type_text(type_);
            out.push(if *braced { '{' } else { '(' });
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(if *braced { '}' } else { ')' });
            out
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
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
        Expr::Cast { kind, type_, value } => {
            let mut out = String::from(cast_text(*kind));
            out.push('<');
            out.push_str(&type_text(type_));
            out.push_str(">(");
            out.push_str(&expr_text(value));
            out.push(')');
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
        Expr::InitList(elements) => {
            let mut out = String::from("{");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Expr::SizeOf(type_) => {
            let mut out = String::from("sizeof(");
            out.push_str(&type_text(type_));
            out.push(')');
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

const fn cast_text(kind: CastKind) -> &'static str {
    kind.as_str()
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
            let mut out = String::from("{");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Literal::Null => "nullptr".to_string(),
    }
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "// ", doc.as_lines());
}

/// A statement as a `for` clause holds it: no semicolon, no line.
///
/// Only the two forms a header can carry; anything else returns nothing
/// rather than half a statement, which would put a silently empty
/// initialiser in generated source.
fn statement_inline(node: &Statement) -> String {
    match node {
        Statement::Declare { type_, name, value } => {
            let mut out = type_text(type_);
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

/// An integer as the source text of a C++ constant expression.
///
/// The same value C has no literal for, for the same reason one step
/// on: [lex.icon] gives an integer literal no sign, so
/// `-9223372036854775808LL` is a unary minus over `9223372036854775808`
/// with an `LL` suffix, and [lex.icon]/3 says a literal whose value
/// fits none of the types its suffix admits is ill-formed. Measured,
/// g++ 16.1.0 answers "integer constant is so large that it is
/// unsigned" and Apple clang 21.0.0 `-Wimplicitly-unsigned-literal`.
///
/// The subtraction is the one `<cstdint>` writes `INT64_MIN` as, and
/// `crates/wire-probe/src/corba_cpp.rs` and
/// `tools/wireprobe/wireprobe-zerodds-cpp-gen/src/fill.rs` already
/// spell the value that way.
#[must_use]
pub fn integer_text(value: i128) -> String {
    if value == i128::from(i64::MIN) {
        return "(-9223372036854775807LL - 1)".to_string();
    }
    format!("{value}")
}
