// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the C# CST. Spec §5.
//!
//! One style, the .NET convention: four spaces, the opening brace on a
//! line of its own — which is where C# puts it and Java does not — one
//! member per line, and a blank line before anything with a body.
//!
//! Two things this renderer does that the Java one has no need for:
//!
//! - A [`Path`] that is rooted renders with `global::`. That prefix is
//!   what makes a generated reference immune to capture by a name in
//!   scope, and the tree is where the decision to root it was made.
//! - A documentation comment renders as a `///` XML block, so the text
//!   is escaped for XML. A member comment saying `sequence<long>` would
//!   otherwise be a malformed element the moment anyone compiles with
//!   `/doc`.

use crate::writer::{Writer, comment_lines, escape_char, escape_string, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::csharp::{
    Accessor, Attribute, AttributeArg, Block, Class, Constructor, Delegate, Enum, EnumMember, Expr,
    Field, File, Interface, Item, Member, Method, Modifier, Namespace, Param, Property, Record,
    Statement, Struct, SwitchSection, Type, TypeConstraint, TypeDecl, TypeParam, Using,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    line_comment(&mut w, &file.doc);
    if !file.doc.is_empty() {
        w.blank_line();
    }
    for using in &file.usings {
        render_using(&mut w, using);
    }
    if !file.usings.is_empty() {
        w.blank_line();
    }
    render_items(&mut w, &file.items);
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

/// Render one expression, for a caller that has only one.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/csharp.rs`)
fn render_items(w: &mut Writer, items: &[Item]) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 && !matches!(item, Item::Comment(_)) {
            w.blank_line();
        }
        match item {
            Item::Namespace(node) => render_namespace(w, node),
            Item::Type(node) => render_type_decl(w, node),
            Item::Comment(node) => line_comment(w, node),
            Item::Verbatim(text) => verbatim_lines(w, text),
        }
    }
}

/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/csharp.rs`)
fn render_namespace(w: &mut Writer, node: &Namespace) {
    // A `//` comment and not `///`: a namespace is not a documentable
    // element, and an XML doc on one is CS1587 the moment anyone builds
    // with `/doc`.
    line_comment(w, &node.doc);
    let mut head = String::from("namespace ");
    head.push_str(&path_text(&node.name));
    w.line(&head);
    w.line("{");
    w.indent();
    for using in &node.usings {
        render_using(w, using);
    }
    if !node.usings.is_empty() && !node.items.is_empty() {
        w.blank_line();
    }
    render_items(w, &node.items);
    w.dedent();
    w.line("}");
}

fn render_using(w: &mut Writer, node: &Using) {
    line_comment(w, &node.doc);
    let mut line = String::new();
    if node.global {
        line.push_str("global ");
    }
    line.push_str("using ");
    if node.static_ {
        line.push_str("static ");
    }
    if let Some(alias) = &node.alias {
        line.push_str(alias.as_str());
        line.push_str(" = ");
    }
    line.push_str(&type_text(&node.target));
    line.push(';');
    w.line(&line);
}

fn render_type_decl(w: &mut Writer, node: &TypeDecl) {
    match node {
        TypeDecl::Class(inner) => render_class(w, inner),
        TypeDecl::Struct(inner) => render_struct(w, inner),
        TypeDecl::Interface(inner) => render_interface(w, inner),
        TypeDecl::Enum(inner) => render_enum(w, inner),
        TypeDecl::Record(inner) => render_record(w, inner),
        TypeDecl::Delegate(inner) => render_delegate(w, inner),
    }
}

fn render_class(w: &mut Writer, node: &Class) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("class ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push_str(&bases_text(&node.bases));
    w.line(&head);
    constraints(w, &node.constraints);
    render_body(w, &node.members);
}

fn render_struct(w: &mut Writer, node: &Struct) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("struct ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push_str(&bases_text(&node.bases));
    w.line(&head);
    constraints(w, &node.constraints);
    render_body(w, &node.members);
}

fn render_interface(w: &mut Writer, node: &Interface) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("interface ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push_str(&bases_text(&node.bases));
    w.line(&head);
    constraints(w, &node.constraints);
    render_body(w, &node.members);
}

fn render_record(w: &mut Writer, node: &Record) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str(node.kind.as_str());
    head.push(' ');
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    if let Some(params) = &node.parameters {
        head.push('(');
        head.push_str(&params_text(params));
        head.push(')');
    }
    head.push_str(&bases_text(&node.bases));
    // A record with no members is one line: `record class P(int X);`.
    if node.members.is_empty() && node.constraints.is_empty() {
        head.push(';');
        w.line(&head);
        return;
    }
    w.line(&head);
    constraints(w, &node.constraints);
    render_body(w, &node.members);
}

fn render_delegate(w: &mut Writer, node: &Delegate) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut line = modifiers_text(&node.modifiers);
    line.push_str("delegate ");
    line.push_str(&type_text(&node.returns));
    line.push(' ');
    line.push_str(node.name.as_str());
    line.push_str(&generics_text(&node.generics));
    line.push('(');
    line.push_str(&params_text(&node.params));
    line.push(')');
    if node.constraints.is_empty() {
        line.push(';');
        w.line(&line);
        return;
    }
    w.line(&line);
    w.indent();
    for constraint in &node.constraints {
        let mut text = constraint_text(constraint);
        text.push(';');
        w.line(&text);
    }
    w.dedent();
}

fn render_enum(w: &mut Writer, node: &Enum) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str("enum ");
    head.push_str(node.name.as_str());
    if let Some(underlying) = node.underlying {
        head.push_str(" : ");
        head.push_str(underlying.as_str());
    }
    w.line(&head);
    w.line("{");
    w.indent();
    for member in &node.members {
        render_enum_member(w, member);
    }
    w.dedent();
    w.line("}");
}

/// Every member ends in a comma, the last one included.
///
/// C# allows the trailing comma, and writing it means adding a member is
/// a one-line diff rather than a two-line one.
fn render_enum_member(w: &mut Writer, node: &EnumMember) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut line = String::from(node.name.as_str());
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(',');
    w.line(&line);
}

fn render_body(w: &mut Writer, members: &[Member]) {
    w.line("{");
    w.indent();
    render_members(w, members);
    w.dedent();
    w.line("}");
}

/// Fields and comments run together; anything with a body gets a blank
/// line before it.
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
        Member::Field(inner) => render_field(w, inner),
        Member::Property(inner) => render_property(w, inner),
        Member::Method(inner) => render_method(w, inner),
        Member::Constructor(inner) => render_constructor(w, inner),
        Member::Nested(decl) => render_type_decl(w, decl),
        Member::Comment(inner) => line_comment(w, inner),
        Member::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
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

fn render_property(w: &mut Writer, node: &Property) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str(&type_text(&node.type_));
    head.push(' ');
    head.push_str(node.name.as_str());

    if let Some(expression) = &node.expression {
        head.push_str(" => ");
        head.push_str(&expr_text(expression));
        head.push(';');
        w.line(&head);
        return;
    }

    // Auto-accessors go on one line; a body puts each on its own.
    let auto = node
        .accessors
        .iter()
        .all(|accessor| accessor.body.is_none() && accessor.expression.is_none());
    if auto {
        head.push_str(" { ");
        for accessor in &node.accessors {
            head.push_str(&modifiers_text(&accessor.modifiers));
            head.push_str(accessor.kind.as_str());
            head.push_str("; ");
        }
        head.push('}');
        if let Some(initialiser) = &node.initialiser {
            head.push_str(" = ");
            head.push_str(&expr_text(initialiser));
            head.push(';');
        }
        w.line(&head);
        return;
    }

    w.line(&head);
    w.line("{");
    w.indent();
    for accessor in &node.accessors {
        render_accessor(w, accessor);
    }
    w.dedent();
    w.line("}");
}

fn render_accessor(w: &mut Writer, node: &Accessor) {
    let mut head = modifiers_text(&node.modifiers);
    head.push_str(node.kind.as_str());
    if let Some(expression) = &node.expression {
        head.push_str(" => ");
        head.push_str(&expr_text(expression));
        head.push(';');
        w.line(&head);
        return;
    }
    match &node.body {
        None => {
            head.push(';');
            w.line(&head);
        }
        Some(block) => {
            w.line(&head);
            w.line("{");
            w.indent();
            render_block(w, block);
            w.dedent();
            w.line("}");
        }
    }
}

fn render_method(w: &mut Writer, node: &Method) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str(&type_text(&node.returns));
    head.push(' ');
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if let Some(expression) = &node.expression {
        head.push_str(" => ");
        head.push_str(&expr_text(expression));
        head.push(';');
        w.line(&head);
        constraints(w, &node.constraints);
        return;
    }
    match &node.body {
        None => {
            head.push(';');
            w.line(&head);
            constraints(w, &node.constraints);
        }
        Some(block) => {
            w.line(&head);
            constraints(w, &node.constraints);
            w.line("{");
            w.indent();
            render_block(w, block);
            w.dedent();
            w.line("}");
        }
    }
}

fn render_constructor(w: &mut Writer, node: &Constructor) {
    doc(w, &node.doc);
    attributes(w, &node.attributes);
    let mut head = modifiers_text(&node.modifiers);
    head.push_str(node.name.as_str());
    head.push('(');
    head.push_str(&params_text(&node.params));
    head.push(')');
    if let Some(initialiser) = &node.initialiser {
        head.push_str(" : ");
        head.push_str(if initialiser.this { "this(" } else { "base(" });
        let parts: Vec<String> = initialiser.args.iter().map(expr_text).collect();
        head.push_str(&parts.join(", "));
        head.push(')');
    }
    w.line(&head);
    w.line("{");
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
/// The model bounds none of the three counts, and C# nests the arms in
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
/// The model bounds none of the three counts, and C# nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
#[allow(clippy::too_many_lines)]
fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Declare { type_, name, value } => {
            let mut line = declare_text(type_, name, value.as_ref());
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
            head.push(')');
            w.line(&head);
            w.line("{");
            w.indent();
            render_block(w, then);
            w.dedent();
            w.line("}");
            if let Some(block) = otherwise {
                w.line("else");
                w.line("{");
                w.indent();
                render_block(w, block);
                w.dedent();
                w.line("}");
            }
        }
        Statement::ForEach {
            type_,
            binding,
            range,
            body,
        } => {
            let mut head = String::from("foreach (");
            head.push_str(&type_text(type_));
            head.push(' ');
            head.push_str(binding.as_str());
            head.push_str(" in ");
            head.push_str(&expr_text(range));
            head.push(')');
            w.line(&head);
            block_braces(w, body);
        }
        Statement::For {
            init,
            condition,
            step,
            body,
        } => {
            let mut head = String::from("for (");
            head.push_str(&inline_statement_text(init));
            head.push_str("; ");
            head.push_str(&expr_text(condition));
            head.push_str("; ");
            head.push_str(&expr_text(step));
            head.push(')');
            w.line(&head);
            block_braces(w, body);
        }
        Statement::While { condition, body } => {
            let mut head = String::from("while (");
            head.push_str(&expr_text(condition));
            head.push(')');
            w.line(&head);
            block_braces(w, body);
        }
        Statement::Switch { value, sections } => {
            let mut head = String::from("switch (");
            head.push_str(&expr_text(value));
            head.push(')');
            w.line(&head);
            w.line("{");
            w.indent();
            for section in sections {
                render_section(w, section);
            }
            w.dedent();
            w.line("}");
        }
        Statement::Throw(value) => {
            let mut line = String::from("throw ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Statement::Using { resource, body } => {
            let mut head = String::from("using (");
            head.push_str(&inline_statement_text(resource));
            head.push(')');
            w.line(&head);
            block_braces(w, body);
        }
        Statement::Block(block) => block_braces(w, block),
        Statement::Break => w.line("break;"),
        Statement::Comment(node) => line_comment(w, node),
    }
}

fn block_braces(w: &mut Writer, body: &Block) {
    w.line("{");
    w.indent();
    render_block(w, body);
    w.dedent();
    w.line("}");
}

/// A statement written inside a `for` or `using` head, with no `;`.
fn inline_statement_text(node: &Statement) -> String {
    match node {
        Statement::Declare { type_, name, value } => declare_text(type_, name, value.as_ref()),
        Statement::Expr(value) => expr_text(value),
        // Nothing else belongs in a head. Written as empty rather than
        // guessed at: an unbalanced brace would be worse than a clause
        // the reader can see is missing.
        _ => String::new(),
    }
}

fn declare_text(type_: &Type, name: &Ident, value: Option<&Expr>) -> String {
    let mut out = type_text(type_);
    out.push(' ');
    out.push_str(name.as_str());
    if let Some(value) = value {
        out.push_str(" = ");
        out.push_str(&expr_text(value));
    }
    out
}

fn render_section(w: &mut Writer, section: &SwitchSection) {
    if section.labels.is_empty() {
        w.line("default:");
    } else {
        for label in &section.labels {
            let mut line = String::from("case ");
            line.push_str(&expr_text(label));
            line.push(':');
            w.line(&line);
        }
    }
    w.indent();
    render_block(w, &section.body);
    if section.breaks {
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

fn attributes(w: &mut Writer, attributes: &[Attribute]) {
    for attribute in attributes {
        w.line(&attribute_text(attribute));
    }
}

fn attribute_text(attribute: &Attribute) -> String {
    let mut out = String::from("[");
    if let Some(target) = &attribute.target {
        out.push_str(target.as_str());
        out.push_str(": ");
    }
    out.push_str(&path_text(&attribute.path));
    if !attribute.args.is_empty() {
        out.push('(');
        let parts: Vec<String> = attribute
            .args
            .iter()
            .map(|arg| match arg {
                AttributeArg::Positional(value) => expr_text(value),
                AttributeArg::Named { name, value } => {
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
    out.push(']');
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
            let mut text = String::new();
            if let Some(variance) = param.variance {
                text.push_str(variance.as_str());
                text.push(' ');
            }
            text.push_str(param.name.as_str());
            text
        })
        .collect();
    out.push_str(&parts.join(", "));
    out.push('>');
    out
}

fn bases_text(bases: &[Type]) -> String {
    if bases.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = bases.iter().map(type_text).collect();
    let mut out = String::from(" : ");
    out.push_str(&parts.join(", "));
    out
}

fn constraints(w: &mut Writer, clauses: &[TypeConstraint]) {
    if clauses.is_empty() {
        return;
    }
    w.indent();
    for clause in clauses {
        w.line(&constraint_text(clause));
    }
    w.dedent();
}

fn constraint_text(clause: &TypeConstraint) -> String {
    let mut out = String::from("where ");
    out.push_str(clause.param.as_str());
    out.push_str(" : ");
    out.push_str(&clause.bounds.join(", "));
    out
}

fn params_text(params: &[Param]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|param| {
            let mut text = String::new();
            for attribute in &param.attributes {
                text.push_str(&attribute_text(attribute));
                text.push(' ');
            }
            if let Some(modifier) = param.modifier {
                text.push_str(modifier.as_str());
                text.push(' ');
            }
            text.push_str(&type_text(&param.type_));
            text.push(' ');
            text.push_str(param.name.as_str());
            if let Some(default) = &param.default {
                text.push_str(" = ");
                text.push_str(&expr_text(default));
            }
            text
        })
        .collect();
    parts.join(", ")
}

/// A qualified name, with `global::` where the path is rooted.
///
/// The prefix is the whole reason a rooted path exists in the tree: it
/// resolves at the global namespace and nothing declared in a scope can
/// capture it.
fn path_text(path: &Path) -> String {
    let parts: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
    let joined = parts.join(".");
    if path.is_rooted() {
        let mut out = String::from("global::");
        out.push_str(&joined);
        return out;
    }
    joined
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/adapter/csharp.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Predefined(predefined) => predefined.as_str().to_string(),
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
        Type::Array { element, rank } => {
            let mut out = type_text(element);
            out.push('[');
            // `T[,]` for rank two: one comma fewer than the rank.
            for _ in 1..*rank {
                out.push(',');
            }
            out.push(']');
            out
        }
        Type::Nullable(inner) => {
            let mut out = type_text(inner);
            out.push('?');
            out
        }
        Type::Pointer(inner) => {
            let mut out = type_text(inner);
            out.push('*');
            out
        }
        Type::Tuple(elements) => {
            let parts: Vec<String> = elements
                .iter()
                .map(|element| {
                    let mut text = type_text(&element.type_);
                    if let Some(name) = &element.name {
                        text.push(' ');
                        text.push_str(name.as_str());
                    }
                    text
                })
                .collect();
            let mut out = String::from("(");
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Type::Var => "var".to_string(),
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_csharp.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
#[allow(clippy::too_many_lines)]
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Path(path) => path_text(path),
        Expr::Invoke {
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
        Expr::Member { value, name } => {
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
        Expr::New {
            type_,
            args,
            initialiser,
        } => {
            let mut out = String::from("new ");
            out.push_str(&type_text(type_));
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            if !initialiser.is_empty() {
                out.push_str(" { ");
                let parts: Vec<String> = initialiser
                    .iter()
                    .map(|(name, value)| {
                        let mut text = name.as_str().to_string();
                        text.push_str(" = ");
                        text.push_str(&expr_text(value));
                        text
                    })
                    .collect();
                out.push_str(&parts.join(", "));
                out.push_str(" }");
            }
            out
        }
        Expr::NewArray {
            element,
            length,
            elements,
            nested,
        } => {
            let mut out = String::from("new ");
            out.push_str(&type_text(element));
            match length {
                Some(length) => {
                    out.push('[');
                    out.push_str(&expr_text(length));
                    out.push(']');
                    // The inner ranks of a jagged array, left empty:
                    // `new int[3][]` and never `new int[][3]`.
                    for _ in 0..*nested {
                        out.push_str("[]");
                    }
                }
                None => {
                    out.push_str("[]");
                    for _ in 0..*nested {
                        out.push_str("[]");
                    }
                    out.push_str(" { ");
                    let parts: Vec<String> = elements.iter().map(expr_text).collect();
                    out.push_str(&parts.join(", "));
                    out.push_str(" }");
                }
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
        Expr::Unary { op, value } => {
            let mut out = (*op).to_string();
            out.push_str(&expr_text(value));
            out
        }
        Expr::Cast { type_, value } => {
            let mut out = String::from("(");
            out.push_str(&type_text(type_));
            out.push(')');
            out.push_str(&expr_text(value));
            out
        }
        Expr::Is { value, type_ } => {
            let mut out = expr_text(value);
            out.push_str(" is ");
            out.push_str(&type_text(type_));
            out
        }
        Expr::Conditional {
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
        Expr::Assign { target, value } => {
            let mut out = expr_text(target);
            out.push_str(" = ");
            out.push_str(&expr_text(value));
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::This => "this".to_string(),
        Expr::Default(type_) => match type_ {
            None => "default".to_string(),
            Some(type_) => {
                let mut out = String::from("default(");
                out.push_str(&type_text(type_));
                out.push(')');
                out
            }
        },
        Expr::TypeOf(type_) => {
            let mut out = String::from("typeof(");
            out.push_str(&type_text(type_));
            out.push(')');
            out
        }
        Expr::NameOf(value) => {
            let mut out = String::from("nameof(");
            out.push_str(&expr_text(value));
            out.push(')');
            out
        }
        Expr::Lambda { params, body } => {
            let mut out = String::from("(");
            let parts: Vec<&str> = params.iter().map(Ident::as_str).collect();
            out.push_str(&parts.join(", "));
            out.push_str(") => ");
            out.push_str(&expr_text(body));
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
            let mut out = String::from("new byte[] { ");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            out.push_str(&parts.join(", "));
            out.push_str(" }");
            out
        }
        Literal::Null => "null".to_string(),
    }
}

/// A `///` XML documentation block.
///
/// The text is escaped for XML. A comment carrying `sequence<long>` or
/// `a && b` is prose to the reader and markup to the compiler, and the
/// moment anyone builds with `/doc` an unescaped one is a warning about
/// a file this crate generated.
fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    w.line("/// <summary>");
    let escaped: Vec<String> = doc.as_lines().iter().map(|line| escape_xml(line)).collect();
    comment_lines(w, "/// ", &escaped);
    w.line("/// </summary>");
}

/// A `//` comment block, for text that is not a member's documentation.
///
/// A file header and a note beside a declaration are not XML doc: they
/// document a decision rather than an API, and `///` on something that
/// is not a declaration is a warning (CS1587) rather than a comment.
fn line_comment(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "// ", doc.as_lines());
}

/// The three characters XML gives a meaning to.
fn escape_xml(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use zerodds_lang_cst::common::Visibility;
    use zerodds_lang_cst::csharp::{Predefined, RecordKind};

    fn class(name: &str, members: Vec<Member>) -> TypeDecl {
        TypeDecl::Class(Class {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: alloc::vec![Modifier::Public, Modifier::Sealed],
            name: Ident::new(name),
            generics: Vec::new(),
            bases: Vec::new(),
            constraints: Vec::new(),
            members,
        })
    }

    fn field(type_: Type, name: &str) -> Member {
        Member::Field(Field {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: alloc::vec![Modifier::Public],
            type_,
            name: Ident::new(name),
            value: None,
        })
    }

    #[test]
    fn a_class_opens_its_brace_on_a_line_of_its_own() {
        // The .NET convention, and the one difference in shape from the
        // Java renderer beside it.
        let text = type_decl(&class("Pose", alloc::vec![]));
        assert_eq!(text, "public sealed class Pose\n{\n}\n");
    }

    #[test]
    fn a_rooted_path_renders_with_global() {
        let type_ = Type::named(Path::rooted(&["a", "b", "Point"]));
        assert_eq!(type_text(&type_), "global::a.b.Point");
        let relative = Type::named(Path::new(&["a", "b", "Point"]));
        assert_eq!(type_text(&relative), "a.b.Point");
    }

    #[test]
    fn an_array_renders_one_pair_of_brackets_per_level() {
        let jagged = Type::array(Type::array(Type::Predefined(Predefined::Int)));
        assert_eq!(type_text(&jagged), "int[][]");
        let rectangular = Type::Array {
            element: alloc::boxed::Box::new(Type::Predefined(Predefined::Int)),
            rank: 2,
        };
        assert_eq!(type_text(&rectangular), "int[,]");
    }

    #[test]
    fn a_nullable_value_type_renders_with_a_question_mark() {
        let type_ = Type::Nullable(alloc::boxed::Box::new(Type::Predefined(Predefined::Int)));
        assert_eq!(type_text(&type_), "int?");
    }

    #[test]
    fn a_namespace_nests_and_its_members_are_indented() {
        let text = file(&File {
            doc: Doc::none(),
            usings: Vec::new(),
            items: alloc::vec![Item::Namespace(Namespace {
                doc: Doc::none(),
                name: Path::new(&["a", "b"]),
                usings: Vec::new(),
                items: alloc::vec![Item::Type(class(
                    "Point",
                    alloc::vec![field(Type::Predefined(Predefined::Double), "X")]
                ))],
            })],
        });
        assert!(text.contains("namespace a.b\n{\n"), "{text}");
        assert!(text.contains("    public sealed class Point\n"), "{text}");
        assert!(text.contains("        public double X;\n"), "{text}");
    }

    #[test]
    fn an_enum_writes_its_underlying_type_and_a_trailing_comma() {
        let text = type_decl(&TypeDecl::Enum(Enum {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: alloc::vec![Modifier::Public],
            name: Ident::new("Colour"),
            underlying: Some(Predefined::Int),
            members: alloc::vec![EnumMember {
                doc: Doc::none(),
                attributes: Vec::new(),
                name: Ident::new("RED"),
                value: Some(Expr::Literal(Literal::int(0))),
            }],
        }));
        assert!(text.contains("public enum Colour : int\n"), "{text}");
        assert!(text.contains("    RED = 0,\n"), "{text}");
    }

    #[test]
    fn an_attribute_renders_above_what_it_annotates() {
        let member = Member::Field(Field {
            doc: Doc::none(),
            attributes: alloc::vec![Attribute {
                target: None,
                path: Path::rooted(&["ZeroDds", "Idl", "IdlName"]),
                args: alloc::vec![AttributeArg::Positional(Expr::Literal(Literal::string(
                    "venue"
                )))],
            }],
            modifiers: alloc::vec![Modifier::Public],
            type_: Type::Predefined(Predefined::Int),
            name: Ident::new("Venue"),
            value: None,
        });
        let text = type_decl(&class("P", alloc::vec![member]));
        assert!(
            text.contains("[global::ZeroDds.Idl.IdlName(\"venue\")]"),
            "{text}"
        );
        assert!(text.contains("public int Venue;"), "{text}");
    }

    #[test]
    fn an_xml_doc_escapes_the_characters_xml_reads() {
        // `sequence<long>` in a comment is prose to a reader and an
        // element to the compiler.
        let mut w = Writer::spaces4();
        doc(&mut w, &Doc::line("a sequence<long> & an array"));
        let text = w.finish();
        assert!(
            text.contains("/// a sequence&lt;long&gt; &amp; an array"),
            "{text}"
        );
        assert!(text.starts_with("/// <summary>"), "{text}");
    }

    #[test]
    fn a_file_header_is_a_plain_comment_and_not_xml_doc() {
        // `///` above something that is not a declaration is CS1587.
        let text = file(&File {
            doc: Doc::line("Generated. Do not edit."),
            usings: Vec::new(),
            items: Vec::new(),
        });
        assert!(text.starts_with("// Generated. Do not edit.\n"), "{text}");
        assert!(!text.contains("///"), "{text}");
    }

    #[test]
    fn a_using_alias_renders_its_target_type() {
        let mut w = Writer::spaces4();
        render_using(
            &mut w,
            &Using {
                doc: Doc::none(),
                alias: Some(Ident::new("Row")),
                static_: false,
                global: false,
                target: Type::array(Type::Predefined(Predefined::Int)),
            },
        );
        assert_eq!(w.finish(), "using Row = int[];\n");
    }

    #[test]
    fn an_auto_property_stays_on_one_line() {
        let member = Member::Property(Property {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: alloc::vec![Modifier::Public],
            type_: Type::Predefined(Predefined::Int),
            name: Ident::new("Count"),
            accessors: alloc::vec![
                Accessor {
                    kind: zerodds_lang_cst::csharp::AccessorKind::Get,
                    modifiers: Vec::new(),
                    body: None,
                    expression: None,
                },
                Accessor {
                    kind: zerodds_lang_cst::csharp::AccessorKind::Set,
                    modifiers: Vec::new(),
                    body: None,
                    expression: None,
                },
            ],
            expression: None,
            initialiser: None,
        });
        let text = type_decl(&class("P", alloc::vec![member]));
        assert!(text.contains("public int Count { get; set; }"), "{text}");
    }

    #[test]
    fn a_record_with_no_body_ends_in_a_semicolon() {
        let text = type_decl(&TypeDecl::Record(Record {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: alloc::vec![Modifier::Public],
            kind: RecordKind::Class,
            name: Ident::new("P"),
            generics: Vec::new(),
            parameters: Some(alloc::vec![Param {
                attributes: Vec::new(),
                modifier: None,
                type_: Type::Predefined(Predefined::Int),
                name: Ident::new("X"),
                default: None,
            }]),
            bases: Vec::new(),
            constraints: Vec::new(),
            members: Vec::new(),
        }));
        assert_eq!(text, "public record class P(int X);\n");
    }

    #[test]
    fn the_renderer_decides_nothing_the_tree_did_not_say() {
        // Visibility is a common-tree concept the C# tree does not use:
        // C# spells its access as modifiers, and a renderer that mapped
        // one onto the other would be deciding.
        assert_eq!(Visibility::ALL.len(), 3);
        let text = type_decl(&class("P", alloc::vec![]));
        assert!(!text.contains("internal"), "{text}");
    }
}
