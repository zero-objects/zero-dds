// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the TypeScript CST. Spec §5.
//!
//! One style, the one `prettier` produces and every TypeScript project
//! in this repository is written in: two spaces, the opening brace on
//! the same line, a semicolon after every member.
//!
//! Two spaces and not the four the other three renderers use. That is
//! not a preference: TypeScript nests further than Java does for the
//! same IDL — a namespace per module, then an interface, then a member —
//! and the community formatter that every consumer of this output runs
//! would reindent a four-space file on first save, turning every
//! generated file into a diff.
//!
//! Nothing is decided here. Whether a declaration is `export`ed, whether
//! a property is `readonly`, whether a name is quoted: all of it is in
//! the tree, and a renderer that chose any of it would put the choice
//! where no rule diff can see it.

use crate::writer::{Writer, comment_lines, escape_string, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::typescript::{
    Block, Class, ClassMember, Constructor, Decl, Enum, EnumMember, Expr, Field, File, Function,
    Import, IndexSignature, Interface, Method, MethodSignature, Namespace, Param, Property,
    PropertyName, Signature, Statement, SwitchCase, Type, TypeAlias, TypeParam, Variable,
};

/// Render a whole file.
#[must_use]
pub fn file(node: &File) -> String {
    let mut w = writer();
    doc(&mut w, &node.doc);
    for import in &node.imports {
        w.line(&import_text(import));
    }
    if !node.imports.is_empty() {
        w.blank_line();
    }
    render_decls(&mut w, &node.decls);
    w.finish()
}

/// Render one declaration, for a caller that has only one.
#[must_use]
pub fn decl(node: &Decl) -> String {
    let mut w = writer();
    render_decl(&mut w, node);
    w.finish()
}

/// Render one type, for a caller that has only one.
#[must_use]
pub fn type_(node: &Type) -> String {
    type_text(node)
}

/// Render one expression, for a caller that has only one.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// Two spaces per level. See the module documentation.
fn writer() -> Writer {
    Writer::new("  ")
}

fn import_text(node: &Import) -> String {
    let mut out = String::from("import ");
    if node.type_only {
        out.push_str("type ");
    }
    let mut clauses: Vec<String> = Vec::new();
    if let Some(default) = &node.default {
        clauses.push(default.as_str().to_string());
    }
    if let Some(namespace) = &node.namespace {
        let mut text = String::from("* as ");
        text.push_str(namespace.as_str());
        clauses.push(text);
    }
    if !node.names.is_empty() {
        let parts: Vec<String> = node
            .names
            .iter()
            .map(|name| {
                let mut text = String::new();
                if name.type_only {
                    text.push_str("type ");
                }
                text.push_str(name.name.as_str());
                if let Some(alias) = &name.alias {
                    text.push_str(" as ");
                    text.push_str(alias.as_str());
                }
                text
            })
            .collect();
        let mut text = String::from("{ ");
        text.push_str(&parts.join(", "));
        text.push_str(" }");
        clauses.push(text);
    }
    out.push_str(&clauses.join(", "));
    out.push_str(" from \"");
    out.push_str(&escape_string(&node.module));
    out.push_str("\";");
    out
}

/// Declarations run together where they are one-liners and get a blank
/// line where they have a body — the same rule the Java renderer applies
/// to members, and for the same reason: a run of `export const` lines is
/// a table, and a run of interfaces is not.
///
/// zerodds-lint: recursion-depth bounded by the tree's own nesting;
/// `Decl::Namespace` is the only recursive arm and a materialiser builds
/// it from a module chain it has already bounded.
fn render_decls(w: &mut Writer, decls: &[Decl]) {
    let mut previous_had_body = false;
    for (index, node) in decls.iter().enumerate() {
        let has_body = matches!(
            node,
            Decl::Interface(_)
                | Decl::Class(_)
                | Decl::Enum(_)
                | Decl::Namespace(_)
                | Decl::Function(_)
        );
        if index > 0 && (has_body || previous_had_body) {
            w.blank_line();
        }
        render_decl(w, node);
        previous_had_body = has_body;
    }
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Interface(inner) => render_interface(w, inner),
        Decl::Class(inner) => render_class(w, inner),
        Decl::TypeAlias(inner) => render_type_alias(w, inner),
        Decl::Enum(inner) => render_enum(w, inner),
        Decl::Variable(inner) => render_variable(w, inner),
        Decl::Function(inner) => render_function(w, inner),
        Decl::Namespace(inner) => render_namespace(w, inner),
        Decl::Statement(inner) => render_statement(w, inner),
        Decl::Comment(inner) => doc(w, inner),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_interface(w: &mut Writer, node: &Interface) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.exported {
        head.push_str("export ");
    }
    head.push_str("interface ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    if !node.extends.is_empty() {
        head.push_str(" extends ");
        let parts: Vec<String> = node.extends.iter().map(type_text).collect();
        head.push_str(&parts.join(", "));
    }
    // An interface with no members is `{}` on the head line: a pair of
    // braces on two lines is what a formatter collapses.
    if node.members.is_empty() {
        head.push_str(" {}");
        w.line(&head);
        return;
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for member in &node.members {
        render_signature(w, member);
    }
    w.dedent();
    w.line("}");
}

fn render_class(w: &mut Writer, node: &Class) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.exported {
        head.push_str("export ");
    }
    if node.abstract_ {
        head.push_str("abstract ");
    }
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
    if node.members.is_empty() {
        head.push_str(" {}");
        w.line(&head);
        return;
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_class_members(w, &node.members);
    w.dedent();
    w.line("}");
}

fn render_class_members(w: &mut Writer, members: &[ClassMember]) {
    let mut previous_had_body = false;
    for (index, member) in members.iter().enumerate() {
        let has_body = matches!(member, ClassMember::Method(_) | ClassMember::Constructor(_));
        if index > 0 && (has_body || previous_had_body) {
            w.blank_line();
        }
        match member {
            ClassMember::Field(field) => render_field(w, field),
            ClassMember::Method(method) => render_method(w, method),
            ClassMember::Constructor(constructor) => render_constructor(w, constructor),
            ClassMember::Comment(text) => doc(w, text),
            ClassMember::Verbatim(text) => verbatim_lines(w, text),
        }
        previous_had_body = has_body;
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    let mut line = String::new();
    if let Some(accessibility) = node.accessibility {
        line.push_str(accessibility.as_str());
        line.push(' ');
    }
    if node.static_ {
        line.push_str("static ");
    }
    if node.readonly {
        line.push_str("readonly ");
    }
    line.push_str(&property_name_text(&node.name));
    if node.optional {
        line.push('?');
    }
    if node.definite {
        line.push('!');
    }
    if let Some(type_) = &node.type_ {
        line.push_str(": ");
        line.push_str(&type_text(type_));
    }
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(';');
    w.line(&line);
}

fn render_method(w: &mut Writer, node: &Method) {
    doc(w, &node.doc);
    let mut head = String::new();
    if let Some(accessibility) = node.accessibility {
        head.push_str(accessibility.as_str());
        head.push(' ');
    }
    if node.static_ {
        head.push_str("static ");
    }
    if node.abstract_ {
        head.push_str("abstract ");
    }
    head.push_str(&property_name_text(&node.name));
    head.push_str(&generics_text(&node.generics));
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if let Some(returns) = &node.returns {
        head.push_str(": ");
        head.push_str(&type_text(returns));
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
    let mut head = String::new();
    if let Some(accessibility) = node.accessibility {
        head.push_str(accessibility.as_str());
        head.push(' ');
    }
    head.push_str("constructor(");
    head.push_str(&params_text(&node.params));
    head.push_str(") {");
    w.line(&head);
    w.indent();
    render_block(w, &node.body);
    w.dedent();
    w.line("}");
}

fn render_type_alias(w: &mut Writer, node: &TypeAlias) {
    doc(w, &node.doc);
    let mut line = String::new();
    if node.exported {
        line.push_str("export ");
    }
    line.push_str("type ");
    line.push_str(node.name.as_str());
    line.push_str(&generics_text(&node.generics));
    line.push_str(" = ");
    line.push_str(&type_text(&node.type_));
    line.push(';');
    w.line(&line);
}

fn render_enum(w: &mut Writer, node: &Enum) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.exported {
        head.push_str("export ");
    }
    if node.const_ {
        head.push_str("const ");
    }
    head.push_str("enum ");
    head.push_str(node.name.as_str());
    if node.members.is_empty() {
        head.push_str(" {}");
        w.line(&head);
        return;
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for member in &node.members {
        render_enum_member(w, member);
    }
    w.dedent();
    w.line("}");
}

/// Every member ends in a comma, the last one included. A trailing comma
/// is what the formatter writes and what keeps a one-line diff to one
/// line when a member is appended.
fn render_enum_member(w: &mut Writer, node: &EnumMember) {
    doc(w, &node.doc);
    let mut line = property_name_text(&node.name);
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(',');
    w.line(&line);
}

fn render_variable(w: &mut Writer, node: &Variable) {
    doc(w, &node.doc);
    let mut line = String::new();
    if node.exported {
        line.push_str("export ");
    }
    if node.declare {
        line.push_str("declare ");
    }
    line.push_str(node.kind.as_str());
    line.push(' ');
    line.push_str(node.name.as_str());
    if let Some(type_) = &node.type_ {
        line.push_str(": ");
        line.push_str(&type_text(type_));
    }
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(';');
    w.line(&line);
}

fn render_function(w: &mut Writer, node: &Function) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.exported {
        head.push_str("export ");
    }
    head.push_str("function ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if let Some(returns) = &node.returns {
        head.push_str(": ");
        head.push_str(&type_text(returns));
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

/// zerodds-lint: recursion-depth bounded by the tree's own nesting; a
/// materialiser builds the chain from a module chain it has bounded.
fn render_namespace(w: &mut Writer, node: &Namespace) {
    doc(w, &node.doc);
    let mut head = String::new();
    if node.exported {
        head.push_str("export ");
    }
    if node.declare {
        head.push_str("declare ");
    }
    head.push_str("namespace ");
    head.push_str(node.name.as_str());
    if node.decls.is_empty() {
        head.push_str(" {}");
        w.line(&head);
        return;
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    render_decls(w, &node.decls);
    w.dedent();
    w.line("}");
}

fn render_signature(w: &mut Writer, node: &Signature) {
    match node {
        Signature::Comment(text) => doc(w, text),
        Signature::Verbatim(text) => verbatim_lines(w, text),
        Signature::Property(property) => {
            doc(w, &property.doc);
            let mut line = property_head(property);
            line.push(';');
            w.line(&line);
        }
        Signature::Method(method) => {
            doc(w, &method.doc);
            let mut line = method_signature_head(method);
            line.push(';');
            w.line(&line);
        }
        Signature::Index(index) => {
            doc(w, &index.doc);
            let mut line = index_signature_head(index);
            line.push(';');
            w.line(&line);
        }
    }
}

fn property_head(node: &Property) -> String {
    let mut out = String::new();
    if node.readonly {
        out.push_str("readonly ");
    }
    out.push_str(&property_name_text(&node.name));
    if node.optional {
        out.push('?');
    }
    out.push_str(": ");
    out.push_str(&type_text(&node.type_));
    out
}

fn method_signature_head(node: &MethodSignature) -> String {
    let mut out = property_name_text(&node.name);
    if node.optional {
        out.push('?');
    }
    out.push_str(&generics_text(&node.generics));
    out.push('(');
    out.push_str(&params_text(&node.params));
    out.push_str("): ");
    out.push_str(&type_text(&node.returns));
    out
}

fn index_signature_head(node: &IndexSignature) -> String {
    let mut out = String::new();
    if node.readonly {
        out.push_str("readonly ");
    }
    out.push('[');
    out.push_str(node.key.as_str());
    out.push_str(": ");
    out.push_str(&type_text(&node.key_type));
    out.push_str("]: ");
    out.push_str(&type_text(&node.value_type));
    out
}

/// One signature as it appears inside an inline object type, on one
/// line.
///
/// A doc comment becomes a `/* … */` on the same line rather than being
/// dropped: an inline object type has nowhere to put a line comment, and
/// silently losing the text would be losing what the tree carried.
fn inline_signature_text(node: &Signature) -> String {
    let (doc, body) = match node {
        Signature::Property(property) => (&property.doc, property_head(property)),
        Signature::Method(method) => (&method.doc, method_signature_head(method)),
        Signature::Index(index) => (&index.doc, index_signature_head(index)),
        Signature::Comment(text) => (text, String::new()),
        // Raw text has no one-line form; the lines are joined with a
        // space, as the doc comment above them is.
        Signature::Verbatim(text) => return text.lines().collect::<Vec<_>>().join(" "),
    };
    if doc.is_empty() {
        return body;
    }
    let mut out = String::from("/* ");
    out.push_str(&doc.as_lines().join(" "));
    out.push_str(" */");
    if !body.is_empty() {
        out.push(' ');
        out.push_str(&body);
    }
    out
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and TypeScript nests the arms in
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
/// The model bounds none of the three counts, and TypeScript nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Declare {
            kind,
            name,
            type_,
            value,
        } => {
            let mut line = kind.as_str().to_string();
            line.push(' ');
            line.push_str(name.as_str());
            if let Some(type_) = type_ {
                line.push_str(": ");
                line.push_str(&type_text(type_));
            }
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
        Statement::Break => w.line("break;"),
        Statement::For {
            counter,
            bound,
            body,
        } => {
            let name = counter.as_str();
            w.line(&format!(
                "for (let {name} = 0; {name} < {}; {name} += 1) {{",
                expr_text(bound)
            ));
            w.indent();
            render_block(w, body);
            w.dedent();
            w.line("}");
        }
        Statement::While { condition, body } => {
            w.line(&format!("while ({}) {{", expr_text(condition)));
            w.indent();
            render_block(w, body);
            w.dedent();
            w.line("}");
        }
        Statement::ForOf {
            kind,
            binding,
            range,
            body,
        } => {
            let mut head = String::from("for (");
            head.push_str(kind.as_str());
            head.push(' ');
            head.push_str(binding.as_str());
            head.push_str(" of ");
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
        Statement::Comment(text) => doc(w, text),
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

fn generics_text(params: &[TypeParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = params
        .iter()
        .map(|param| {
            let mut text = param.name.as_str().to_string();
            if let Some(extends) = &param.extends {
                text.push_str(" extends ");
                text.push_str(&type_text(extends));
            }
            if let Some(default) = &param.default {
                text.push_str(" = ");
                text.push_str(&type_text(default));
            }
            text
        })
        .collect();
    let mut out = String::from("<");
    out.push_str(&parts.join(", "));
    out.push('>');
    out
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/adapter/typescript.rs` refuses a type that nests deeper)
fn params_text(params: &[Param]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|param| {
            let mut text = String::new();
            if param.rest {
                text.push_str("...");
            }
            text.push_str(param.name.as_str());
            if param.optional {
                text.push('?');
            }
            if let Some(type_) = &param.type_ {
                text.push_str(": ");
                text.push_str(&type_text(type_));
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

/// A property name, quoted or bare exactly as the tree says.
fn property_name_text(name: &PropertyName) -> String {
    match name {
        PropertyName::Ident(ident) => ident.as_str().to_string(),
        PropertyName::Quoted(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
    }
}

/// A path, joined with `.`.
///
/// A rooted path renders no differently from a relative one: TypeScript
/// has no root operator — there is no `::A::B` — so a materialiser that
/// needs a name to be unambiguous has to make it unambiguous itself.
fn path_text(path: &Path) -> String {
    let parts: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
    parts.join(".")
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/adapter/typescript.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Keyword(keyword) => keyword.as_str().to_string(),
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
        Type::ReadonlyArray(element) => {
            let mut out = String::from("readonly ");
            out.push_str(&type_text(element));
            out.push_str("[]");
            out
        }
        Type::Tuple(elements) => {
            let parts: Vec<String> = elements.iter().map(type_text).collect();
            let mut out = String::from("[");
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Type::Union(arms) => {
            let parts: Vec<String> = arms.iter().map(type_text).collect();
            parts.join(" | ")
        }
        Type::Intersection(arms) => {
            let parts: Vec<String> = arms.iter().map(type_text).collect();
            parts.join(" & ")
        }
        Type::Object(members) => {
            if members.is_empty() {
                return "{}".to_string();
            }
            let parts: Vec<String> = members.iter().map(inline_signature_text).collect();
            let mut out = String::from("{ ");
            out.push_str(&parts.join("; "));
            out.push_str(" }");
            out
        }
        Type::Literal(literal) => literal_text(literal),
        Type::Function { params, returns } => {
            let mut out = String::from("(");
            out.push_str(&params_text(params));
            out.push_str(") => ");
            out.push_str(&type_text(returns));
            out
        }
        Type::Parenthesised(inner) => {
            let mut out = String::from("(");
            out.push_str(&type_text(inner));
            out.push(')');
            out
        }
        Type::TypeOf(path) => {
            let mut out = String::from("typeof ");
            out.push_str(&path_text(path));
            out
        }
        Type::KeyOf(inner) => {
            let mut out = String::from("keyof ");
            out.push_str(&type_text(inner));
            out
        }
        Type::Predicate { param, type_ } => {
            let mut out = param.as_str().to_string();
            out.push_str(" is ");
            out.push_str(&type_text(type_));
            out
        }
        Type::Indexed { object, index } => {
            let mut out = type_text(object);
            out.push('[');
            out.push_str(&type_text(index));
            out.push(']');
            out
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_typescript.rs`,
/// and nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
#[allow(clippy::too_many_lines)]
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Path(path) => path_text(path),
        Expr::Array(elements) => {
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            let mut out = String::from("[");
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Expr::Object(entries) => {
            if entries.is_empty() {
                return "{}".to_string();
            }
            let parts: Vec<String> = entries
                .iter()
                .map(|(name, value)| {
                    let mut text = property_name_text(name);
                    text.push_str(": ");
                    text.push_str(&expr_text(value));
                    text
                })
                .collect();
            let mut out = String::from("{ ");
            out.push_str(&parts.join(", "));
            out.push_str(" }");
            out
        }
        Expr::New { callee, args } => {
            let mut out = String::from("new ");
            out.push_str(&path_text(callee));
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Call { callee, args } => {
            let mut out = expr_text(callee);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Member { object, name } => {
            let mut out = expr_text(object);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Expr::Index { object, index } => {
            let mut out = expr_text(object);
            out.push('[');
            out.push_str(&expr_text(index));
            out.push(']');
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
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
        Expr::Unary { op, value } => {
            let mut out = (*op).to_string();
            out.push_str(&expr_text(value));
            out
        }
        Expr::As { value, type_ } => {
            let mut out = expr_text(value);
            out.push_str(" as ");
            out.push_str(&type_text(type_));
            out
        }
        Expr::AsConst(value) => {
            let mut out = expr_text(value);
            out.push_str(" as const");
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
        Expr::Arrow { params, body } => {
            let parts: Vec<&str> = params.iter().map(Ident::as_str).collect();
            let mut out = String::from("(");
            out.push_str(&parts.join(", "));
            out.push_str(") => ");
            out.push_str(&expr_text(body));
            out
        }
        Expr::Parenthesised(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::NonNull(inner) => {
            let mut out = expr_text(inner);
            out.push('!');
            out
        }
        Expr::This => "this".to_string(),
        Expr::Undefined => "undefined".to_string(),
    }
}

/// A literal, in the one spelling TypeScript reads it back as.
///
/// [`Literal::Char`] becomes a **string** literal of one code unit, not
/// a number and not a single-quoted form: TypeScript has no character
/// type, so an IDL `char` is held as a one-unit `string` and its literal
/// has to be one too.
///
/// [`Literal::Bytes`] becomes a `Uint8Array`. That is a call expression
/// rather than a literal because JavaScript has no byte-string syntax;
/// what it is *not* is an ordinary array, which would be a `number[]`
/// and would not hold octets.
fn literal_text(literal: &Literal) -> String {
    match literal {
        // As written, which is where a `bigint` literal's `n` comes
        // from: `Literal::Int("2n")` is the tree's way of saying so.
        Literal::Int(text) | Literal::Float(text) => text.clone(),
        Literal::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Literal::Char(ch) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(&ch.to_string()));
            out.push('"');
            out
        }
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
        Literal::Bytes(bytes) => {
            let mut out = String::from("new Uint8Array([");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            out.push_str(&parts.join(", "));
            out.push_str("])");
            out
        }
        Literal::Null => "null".to_string(),
    }
}

/// TSDoc, which is what the convention and the tooling expect.
fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    w.line("/**");
    comment_lines(w, " * ", doc.as_lines());
    w.line(" */");
}
