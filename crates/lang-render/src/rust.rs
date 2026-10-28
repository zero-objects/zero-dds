// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Rust CST. Spec §5.
//!
//! One style, the one `rustfmt` produces: four spaces, braces on the
//! same line, one item per line with a blank line between items.

use crate::writer::{Writer, comment_lines, escape_char, escape_string, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path, Visibility};
use zerodds_lang_cst::rust::{
    Attr, AttrArg, Block, Const, Derive, Enum, Expr, Field, File, Function, GenericParam, Impl,
    Item, MatchArm, Module, Param, Pattern, Receiver, Statement, Struct, StructBody, Trait, Type,
    TypeAlias, Use, Variant, VariantBody,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    for attr in &file.inner_attrs {
        w.line(&attr_text(attr, true));
    }
    if !file.inner_attrs.is_empty() {
        w.blank_line();
    }
    doc(&mut w, &file.doc);
    for use_ in &file.uses {
        w.line(&use_text(use_));
    }
    if !file.uses.is_empty() {
        w.blank_line();
    }
    for (index, item) in file.items.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_item(&mut w, item);
    }
    w.finish()
}

/// Render one item, for a caller that has only one.
#[must_use]
pub fn item(node: &Item) -> String {
    let mut w = Writer::spaces4();
    render_item(&mut w, node);
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
/// `lang-rules/src/adapter/cst.rs`)
fn render_item(w: &mut Writer, node: &Item) {
    match node {
        Item::Module(module) => render_module(w, module),
        Item::Struct(s) => render_struct(w, s),
        Item::Enum(e) => render_enum(w, e),
        Item::TypeAlias(alias) => render_alias(w, alias),
        Item::Const(constant) => render_const(w, constant),
        Item::Trait(t) => render_trait(w, t),
        Item::Impl(i) => render_impl(w, i),
        Item::Function(f) => render_function(w, f),
        Item::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/cst.rs`)
fn render_module(w: &mut Writer, node: &Module) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut head = String::new();
    head.push_str(&vis_text(node.visibility));
    head.push_str("mod ");
    head.push_str(node.name.as_str());
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for use_ in &node.uses {
        w.line(&use_text(use_));
    }
    if !node.uses.is_empty() && !node.items.is_empty() {
        w.blank_line();
    }
    for (index, item) in node.items.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_item(w, item);
    }
    w.dedent();
    w.line("}");
}

fn render_struct(w: &mut Writer, node: &Struct) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    derives(w, &node.derives);
    let mut head = String::new();
    head.push_str(&vis_text(node.visibility));
    head.push_str("struct ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));

    let text = &node.body_text;
    let has_text = !text.begin.is_empty() || !text.end.is_empty();
    match &node.body {
        StructBody::Unit if has_text => {
            head.push_str(" {");
            w.line(&head);
            w.indent();
            for line in text.begin.iter().chain(&text.end) {
                verbatim_lines(w, line);
            }
            w.dedent();
            w.line("}");
        }
        StructBody::Unit => {
            head.push(';');
            w.line(&head);
        }
        StructBody::Tuple(types) => {
            head.push('(');
            let parts: Vec<String> = types.iter().map(type_text).collect();
            head.push_str(&parts.join(", "));
            head.push_str(");");
            w.line(&head);
        }
        StructBody::Named(fields) => {
            head.push_str(" {");
            w.line(&head);
            w.indent();
            for line in &text.begin {
                verbatim_lines(w, line);
            }
            for field in fields {
                render_field(w, field);
            }
            for line in &text.end {
                verbatim_lines(w, line);
            }
            w.dedent();
            w.line("}");
        }
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut line = String::new();
    line.push_str(&vis_text(node.visibility));
    line.push_str(node.name.as_str());
    line.push_str(": ");
    line.push_str(&type_text(&node.type_));
    line.push(',');
    w.line(&line);
}

fn render_enum(w: &mut Writer, node: &Enum) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    derives(w, &node.derives);
    let mut head = String::new();
    head.push_str(&vis_text(node.visibility));
    head.push_str("enum ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for variant in &node.variants {
        render_variant(w, variant);
    }
    w.dedent();
    w.line("}");
}

fn render_variant(w: &mut Writer, node: &Variant) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut line = String::from(node.name.as_str());
    match &node.body {
        VariantBody::Unit => {}
        VariantBody::Tuple(types) => {
            line.push('(');
            let parts: Vec<String> = types.iter().map(type_text).collect();
            line.push_str(&parts.join(", "));
            line.push(')');
        }
        VariantBody::Named(fields) => {
            line.push_str(" {");
            w.line(&line);
            w.indent();
            for field in fields {
                render_field(w, field);
            }
            w.dedent();
            w.line("},");
            return;
        }
    }
    if let Some(value) = &node.discriminant {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    line.push(',');
    w.line(&line);
}

fn render_alias(w: &mut Writer, node: &TypeAlias) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut line = String::new();
    line.push_str(&vis_text(node.visibility));
    line.push_str("type ");
    line.push_str(node.name.as_str());
    line.push_str(&generics_text(&node.generics));
    line.push_str(" = ");
    line.push_str(&type_text(&node.type_));
    line.push(';');
    w.line(&line);
}

fn render_const(w: &mut Writer, node: &Const) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut line = String::new();
    line.push_str(&vis_text(node.visibility));
    line.push_str("const ");
    line.push_str(node.name.as_str());
    line.push_str(": ");
    line.push_str(&type_text(&node.type_));
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    line.push(';');
    w.line(&line);
}

/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/cst.rs`)
fn render_trait(w: &mut Writer, node: &Trait) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut head = String::new();
    head.push_str(&vis_text(node.visibility));
    head.push_str("trait ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    if !node.supertraits.is_empty() {
        head.push_str(": ");
        let parts: Vec<String> = node.supertraits.iter().map(type_text).collect();
        head.push_str(&parts.join(" + "));
    }
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for (index, item) in node.items.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_item(w, item);
    }
    w.dedent();
    w.line("}");
}

/// zerodds-lint: recursion-depth 64 (one level per IDL module; the
/// module chain is cut at `MAX_MODULE_DEPTH` in
/// `lang-rules/src/adapter/cst.rs`)
fn render_impl(w: &mut Writer, node: &Impl) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut head = String::from("impl");
    head.push_str(&generics_text(&node.generics));
    head.push(' ');
    if let Some(trait_) = &node.trait_ {
        head.push_str(&type_text(trait_));
        head.push_str(" for ");
    }
    head.push_str(&type_text(&node.self_type));
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for (index, item) in node.items.iter().enumerate() {
        if index > 0 {
            w.blank_line();
        }
        render_item(w, item);
    }
    w.dedent();
    w.line("}");
}

fn render_function(w: &mut Writer, node: &Function) {
    doc(w, &node.doc);
    attrs(w, &node.attrs);
    let mut head = String::new();
    head.push_str(&vis_text(node.visibility));
    head.push_str("fn ");
    head.push_str(node.name.as_str());
    head.push_str(&generics_text(&node.generics));
    head.push('(');
    let parts: Vec<String> = node.params.iter().map(param_text).collect();
    head.push_str(&parts.join(", "));
    head.push(')');
    if let Some(returns) = &node.returns {
        head.push_str(" -> ");
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

/// The `if` an else-block consists of, when it consists of nothing else.
fn sole_if(block: &Block) -> Option<&Expr> {
    match block.statements.as_slice() {
        [Statement::Semi(inner @ Expr::If { .. }) | Statement::Tail(inner @ Expr::If { .. })] => {
            Some(inner)
        }
        _ => None,
    }
}

/// A closure's body. A block of one-line statements stays on the
/// closure's line — `|w, v| { w.write_i32(*v); Ok(()) }` — because the
/// closure is an argument inside a longer line, and a block rendered over
/// several lines there would carry the indentation of nothing around it.
///
/// zerodds-lint: recursion-depth = the built expression's nesting, as for
/// `expr_text`, which this is one arm of.
fn closure_body_text(body: &Expr) -> String {
    let Expr::Block(block) = body else {
        return expr_text(body);
    };
    let mut parts = Vec::with_capacity(block.statements.len());
    for statement in &block.statements {
        let text = match statement {
            Statement::Semi(value) => format!("{};", expr_text(value)),
            Statement::Tail(value) => expr_text(value),
            Statement::Let { .. } | Statement::Item(_) => return expr_text(body),
        };
        if text.contains('\n') {
            return expr_text(body);
        }
        parts.push(text);
    }
    format!("{{ {} }}", parts.join(" "))
}

fn render_block(w: &mut Writer, block: &Block) {
    for statement in &block.statements {
        render_statement(w, statement);
    }
}

fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Let {
            name,
            type_,
            value,
            mutable,
        } => {
            let mut line = String::from("let ");
            if *mutable {
                line.push_str("mut ");
            }
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
        Statement::Semi(value) => {
            let mut line = expr_text(value);
            line.push(';');
            w.line(&line);
        }
        Statement::Tail(value) => w.line(&expr_text(value)),
        Statement::Item(item) => render_item(w, item),
    }
}

fn param_text(node: &Param) -> String {
    if let Some(receiver) = node.receiver {
        return match receiver {
            Receiver::Value => "self".to_string(),
            Receiver::Ref => "&self".to_string(),
            Receiver::RefMut => "&mut self".to_string(),
        };
    }
    let mut out = String::new();
    if let Some(name) = &node.name {
        out.push_str(name.as_str());
        out.push_str(": ");
    }
    out.push_str(&type_text(&node.type_));
    out
}

fn generics_text(params: &[GenericParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::from("<");
    let parts: Vec<String> = params
        .iter()
        .map(|param| {
            let mut text = String::new();
            if param.lifetime {
                text.push('\'');
            }
            text.push_str(param.name.as_str());
            if !param.bounds.is_empty() {
                text.push_str(": ");
                let bounds: Vec<String> = param.bounds.iter().map(type_text).collect();
                text.push_str(&bounds.join(" + "));
            }
            text
        })
        .collect();
    out.push_str(&parts.join(", "));
    out.push('>');
    out
}

fn vis_text(visibility: Visibility) -> String {
    match visibility {
        Visibility::Public => "pub ".to_string(),
        Visibility::Internal => "pub(crate) ".to_string(),
        Visibility::Private => String::new(),
    }
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "/// ", doc.as_lines());
}

fn attrs(w: &mut Writer, attrs: &[Attr]) {
    for attr in attrs {
        w.line(&attr_text(attr, false));
    }
}

/// Derives render as one `#[derive(..)]` listing them in order.
fn derives(w: &mut Writer, derives: &[Derive]) {
    if derives.is_empty() {
        return;
    }
    let names: Vec<String> = derives.iter().map(|d| path_text(&d.path)).collect();
    let mut line = String::from("#[derive(");
    line.push_str(&names.join(", "));
    line.push_str(")]");
    w.line(&line);
}

fn attr_text(attr: &Attr, inner: bool) -> String {
    let mut out = String::from(if inner { "#![" } else { "#[" });
    out.push_str(&path_text(&attr.path));
    if !attr.args.is_empty() {
        out.push('(');
        let parts: Vec<String> = attr.args.iter().map(attr_arg_text).collect();
        out.push_str(&parts.join(", "));
        out.push(')');
    }
    out.push(']');
    out
}

fn attr_arg_text(arg: &AttrArg) -> String {
    match arg {
        AttrArg::Word(name) => name.as_str().to_string(),
        AttrArg::Pair { name, value } => {
            let mut out = name.as_str().to_string();
            out.push_str(" = ");
            out.push_str(&literal_text(value));
            out
        }
        AttrArg::Nested { name, args } => {
            let mut out = name.as_str().to_string();
            out.push('(');
            let parts: Vec<String> = args.iter().map(attr_arg_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
    }
}

fn use_text(node: &Use) -> String {
    let mut out = vis_text(node.visibility);
    out.push_str("use ");
    out.push_str(&path_text(&node.path));
    match node.names.len() {
        0 => {}
        1 => {
            out.push_str("::");
            out.push_str(node.names[0].as_str());
        }
        _ => {
            out.push_str("::{");
            let parts: Vec<&str> = node.names.iter().map(Ident::as_str).collect();
            out.push_str(&parts.join(", "));
            out.push('}');
        }
    }
    out.push(';');
    out
}

fn path_text(path: &Path) -> String {
    let parts: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
    let joined = parts.join("::");
    if path.is_rooted() {
        let mut out = String::from("crate::");
        out.push_str(&joined);
        return out;
    }
    joined
}

/// zerodds-lint: recursion-depth = the spelled type's nesting, one level
/// per collection or alias. `rust_type_of` in
/// `lang-rules/src/adapter/cst.rs` carries no depth guard of its own, so
/// the bound is the frontend's — `MAX_ALIAS_DEPTH` in
/// `idl-lower/src/validate.rs` for an alias chain — and, for collections,
/// the nesting the IDL source spells. No number is claimed: this
/// signature returns no error.
fn type_text(node: &Type) -> String {
    match node {
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
        Type::Ref {
            target,
            mutable,
            lifetime,
        } => {
            let mut out = String::from("&");
            if let Some(lifetime) = lifetime {
                out.push('\'');
                out.push_str(lifetime.as_str());
                out.push(' ');
            }
            if *mutable {
                out.push_str("mut ");
            }
            out.push_str(&type_text(target));
            out
        }
        Type::Array { element, length } => {
            let mut out = String::from("[");
            out.push_str(&type_text(element));
            out.push_str("; ");
            out.push_str(&expr_text(length));
            out.push(']');
            out
        }
        Type::Slice(element) => {
            let mut out = String::from("[");
            out.push_str(&type_text(element));
            out.push(']');
            out
        }
        Type::Tuple(elements) => {
            let mut out = String::from("(");
            let parts: Vec<String> = elements.iter().map(type_text).collect();
            out.push_str(&parts.join(", "));
            // A one-element tuple needs the comma or it is a parenthesis.
            if elements.len() == 1 {
                out.push(',');
            }
            out.push(')');
            out
        }
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::ImplTrait(bounds) => {
            let mut out = String::from("impl ");
            let parts: Vec<String> = bounds.iter().map(type_text).collect();
            out.push_str(&parts.join(" + "));
            out
        }
        Type::DynTrait(bounds) => {
            let mut out = String::from("dyn ");
            let parts: Vec<String> = bounds.iter().map(type_text).collect();
            out.push_str(&parts.join(" + "));
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
            let mut out = String::from("&[");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("{b}")).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Literal::Null => "None".to_string(),
    }
}

/// An operand of a binary operation or of an `as`, parenthesised where
/// Rust's precedence would otherwise re-associate it.
///
/// `rust::Expr` has no parenthesis node, so grouping is the renderer's
/// and cannot be an emitter's to remember. `as` binds tighter than every
/// binary operator, which made `bits >> 3 & 31 as u8` parse as
/// `bits >> (3 & (31 as u8))` — a `u64` where the field is a `u8`, so
/// the generated bitset decoder did not compile. A parenthesis never
/// changes what an expression means, so this is added by shape rather
/// than by a precedence table that could be wrong about a pair.
///
/// zerodds-lint: recursion-depth = as [`expr_text`], which this is one
/// arm of.
fn parenthesised(node: &Expr) -> String {
    match node {
        Expr::Binary { .. }
        | Expr::Cast { .. }
        | Expr::Closure { .. }
        | Expr::If { .. }
        | Expr::Match { .. }
        | Expr::Assign { .. }
        | Expr::Return(_) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(node));
            out.push(')');
            out
        }
        _ => expr_text(node),
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_rust.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
#[allow(clippy::too_many_lines)]
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Path(path) => path_text(path),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let mut out = expr_text(receiver);
            out.push('.');
            out.push_str(method.as_str());
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
        Expr::StructLit { path, fields, base } => {
            let mut out = path_text(path);
            out.push_str(" { ");
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| {
                    // Field-init shorthand (`x` for `x: x`) wherever the
                    // value is exactly the field's own name as a bare,
                    // unrooted, one-segment path — `clippy::
                    // redundant_field_names` denies the long form, and
                    // every generated struct literal goes through this
                    // one spot.
                    if let Expr::Path(value_path) = value {
                        if !value_path.is_rooted()
                            && value_path.segments().len() == 1
                            && value_path.last().map(Ident::as_str) == Some(name.as_str())
                        {
                            return name.as_str().to_string();
                        }
                    }
                    let mut text = name.as_str().to_string();
                    text.push_str(": ");
                    text.push_str(&expr_text(value));
                    text
                })
                .collect();
            out.push_str(&parts.join(", "));
            if let Some(base) = base {
                if !fields.is_empty() {
                    out.push_str(", ");
                }
                out.push_str("..");
                out.push_str(&expr_text(base));
            }
            out.push_str(" }");
            out
        }
        Expr::Field { receiver, name } => {
            let mut out = expr_text(receiver);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Expr::Ref { target, mutable } => {
            let mut out = String::from("&");
            if *mutable {
                out.push_str("mut ");
            }
            out.push_str(&expr_text(target));
            out
        }
        Expr::Try(inner) => {
            let mut out = expr_text(inner);
            out.push('?');
            out
        }
        Expr::Closure { params, body } => {
            let mut out = String::from("|");
            for (index, param) in params.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(param.as_str());
            }
            out.push_str("| ");
            out.push_str(&closure_body_text(body));
            out
        }
        Expr::Match { scrutinee, arms } => {
            // A match spans lines; rendered inline it would be one very
            // long line, which is not what rustfmt produces.
            let mut w = Writer::spaces4();
            let mut head = String::from("match ");
            head.push_str(&expr_text(scrutinee));
            head.push_str(" {");
            w.line(&head);
            w.indent();
            for arm in arms {
                w.line(&arm_text(arm));
            }
            w.dedent();
            w.push("}");
            w.finish().trim_end().to_string()
        }
        Expr::If {
            condition,
            then,
            otherwise,
        } => {
            let mut w = Writer::spaces4();
            let mut head = String::from("if ");
            head.push_str(&expr_text(condition));
            head.push_str(" {");
            w.line(&head);
            w.indent();
            render_block(&mut w, then);
            w.dedent();
            match otherwise {
                // An else-block holding nothing but another `if` is an
                // `else if`. Written as `} else { if .. }` it is the same
                // program one level deeper, and `clippy::
                // collapsible_else_if` in every crate that includes it:
                // a `@mutable` decode is one such link per member.
                Some(block) => match sole_if(block) {
                    Some(inner) => {
                        let mut tail = String::from("} else ");
                        tail.push_str(&expr_text(inner));
                        w.push(&tail);
                    }
                    None => {
                        w.line("} else {");
                        w.indent();
                        render_block(&mut w, block);
                        w.dedent();
                        w.push("}");
                    }
                },
                None => w.push("}"),
            }
            w.finish().trim_end().to_string()
        }
        Expr::For {
            binding,
            iterator,
            body,
        } => {
            let mut w = Writer::spaces4();
            let mut head = String::from("for ");
            head.push_str(binding.as_str());
            head.push_str(" in ");
            head.push_str(&expr_text(iterator));
            head.push_str(" {");
            w.line(&head);
            w.indent();
            render_block(&mut w, body);
            w.dedent();
            w.push("}");
            w.finish().trim_end().to_string()
        }
        Expr::While { condition, body } => {
            let mut w = Writer::spaces4();
            let mut head = String::from("while ");
            head.push_str(&expr_text(condition));
            head.push_str(" {");
            w.line(&head);
            w.indent();
            render_block(&mut w, body);
            w.dedent();
            w.push("}");
            w.finish().trim_end().to_string()
        }
        Expr::Assign { target, value } => {
            let mut out = expr_text(target);
            out.push_str(" = ");
            out.push_str(&expr_text(value));
            out
        }
        Expr::Return(value) => match value {
            Some(value) => {
                let mut out = String::from("return ");
                out.push_str(&expr_text(value));
                out
            }
            None => "return".to_string(),
        },
        Expr::Array(elements) => {
            let mut out = String::from("[");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Expr::Repeat { value, count } => {
            let mut out = String::from("[");
            out.push_str(&expr_text(value));
            out.push_str("; ");
            out.push_str(&expr_text(count));
            out.push(']');
            out
        }
        Expr::Tuple(elements) => {
            let mut out = String::from("(");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            if elements.len() == 1 {
                out.push(',');
            }
            out.push(')');
            out
        }
        Expr::Binary { op, lhs, rhs } => {
            let mut out = parenthesised(lhs);
            out.push(' ');
            out.push_str(op.as_str());
            out.push(' ');
            out.push_str(&parenthesised(rhs));
            out
        }
        Expr::Not(value) => {
            let mut out = String::from("!");
            out.push_str(&expr_text(value));
            out
        }
        Expr::Deref(value) => {
            let mut out = String::from("*");
            out.push_str(&expr_text(value));
            out
        }
        Expr::Cast { value, type_ } => {
            let mut out = parenthesised(value);
            out.push_str(" as ");
            out.push_str(&type_text(type_));
            out
        }
        Expr::Block(block) => {
            let mut w = Writer::spaces4();
            w.line("{");
            w.indent();
            render_block(&mut w, block);
            w.dedent();
            w.push("}");
            w.finish().trim_end().to_string()
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_rust.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn arm_text(arm: &MatchArm) -> String {
    let mut out = pattern_text(&arm.pattern);
    if let Some(guard) = &arm.guard {
        out.push_str(" if ");
        out.push_str(&expr_text(guard));
    }
    out.push_str(" => ");
    out.push_str(&expr_text(&arm.body));
    out.push(',');
    out
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_rust.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn pattern_text(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_string(),
        Pattern::Binding(name) => name.as_str().to_string(),
        Pattern::Literal(literal) => literal_text(literal),
        Pattern::Path(path) => path_text(path),
        Pattern::TupleStruct { path, elements } => {
            let mut out = path_text(path);
            out.push('(');
            let parts: Vec<String> = elements.iter().map(pattern_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Pattern::Struct { path, fields, rest } => {
            let mut out = path_text(path);
            out.push_str(" { ");
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, pattern)| {
                    let mut text = name.as_str().to_string();
                    text.push_str(": ");
                    text.push_str(&pattern_text(pattern));
                    text
                })
                .collect();
            out.push_str(&parts.join(", "));
            if *rest {
                if !fields.is_empty() {
                    out.push_str(", ");
                }
                out.push_str("..");
            }
            out.push_str(" }");
            out
        }
    }
}
