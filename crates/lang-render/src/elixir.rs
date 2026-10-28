// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Elixir CST. Spec §5.
//!
//! One style, `mix format`'s: two spaces per level, `do`/`end` blocks,
//! `#` comments, `@moduledoc` as a heredoc. Two spaces and not four
//! because `mix format` is not configurable in that respect, and a
//! generated file that changes under the formatter every consumer runs
//! is a diff nobody asked for.
//!
//! Nothing is decided here. Three spellings are the language's rather
//! than a choice, and each is written down where it happens:
//!
//! - a float literal must have a digit, a point and a digit before any
//!   exponent — `1e10` does not parse (measured, Elixir 1.20.2), while
//!   C, C++, Java and Rust all take it, so the shared
//!   `zerodds_lang_rules::adapter::constant::float_text` cannot be
//!   handed straight through;
//! - `#{` inside a double-quoted string is interpolation, so `#` is
//!   escaped;
//! - the keyword pairs of a list come last, which is why
//!   [`zerodds_lang_cst::elixir::Field`] always carries a default.

use crate::writer::{Writer, comment_lines, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::elixir::{
    Access, Attribute, CodecFunction, Expr, Field, File, Function, Item, Module, Param, Signature,
    Statement, TopLevel, Type, TypeAlias,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::new("  ");
    if !file.doc.is_empty() {
        comment_lines(&mut w, "# ", file.doc.as_lines());
    }
    for entry in &file.modules {
        w.blank_line();
        render_top_level(&mut w, entry);
    }
    w.finish()
}

/// Render one top-level entry, for a caller that has only one.
#[must_use]
pub fn top_level(node: &TopLevel) -> String {
    let mut w = Writer::new("  ");
    render_top_level(&mut w, node);
    w.finish()
}

fn render_top_level(w: &mut Writer, node: &TopLevel) {
    match node {
        TopLevel::Module(module) => render_module(w, module),
        TopLevel::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// Render one module, for a caller that has only one.
#[must_use]
pub fn module(node: &Module) -> String {
    let mut w = Writer::new("  ");
    render_module(&mut w, node);
    w.finish()
}

/// Render one item, for a caller that has only one.
#[must_use]
pub fn item(node: &Item) -> String {
    let mut w = Writer::new("  ");
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

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

fn render_module(w: &mut Writer, node: &Module) {
    let mut head = String::from("defmodule ");
    head.push_str(&path_text(&node.name));
    head.push_str(" do");
    w.line(&head);
    w.indent();
    render_moduledoc(w, &node.doc);
    let mut previous: Option<&Item> = None;
    for (index, item) in node.body.iter().enumerate() {
        let separated = match previous {
            None => !node.doc.is_empty(),
            Some(previous) => !belong_together(previous, item),
        };
        if separated && (index > 0 || !node.doc.is_empty()) {
            w.blank_line();
        }
        render_item(w, item);
        previous = Some(item);
    }
    w.dedent();
    w.line("end");
}

/// `@moduledoc """ … """`.
///
/// A heredoc rather than a single-quoted line, because the text is
/// several lines and a `\n` inside a one-line literal is a documentation
/// string nobody can read in the source. The closing delimiter's
/// indentation is what Elixir strips from every line, so both are
/// written at the writer's current depth.
fn render_moduledoc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    w.line("@moduledoc \"\"\"");
    for line in doc.as_lines() {
        w.line(&escape_heredoc(line));
    }
    w.line("\"\"\"");
}

/// Whether two adjacent items are written with no blank line between
/// them.
///
/// The clauses of one function are one definition and Elixir requires
/// them to be adjacent — it warns "clauses with the same name should be
/// grouped together" otherwise — so a blank line between them would
/// suggest they could be moved apart. A `@spec` belongs to the clauses
/// that follow it for the same reason, and the `@callback`s of a
/// behaviour are one declaration read together. Two definitions that
/// take no parameters are a table of constants — one enumerator or one
/// bit each — and a blank line between every row of a table is noise.
fn belong_together(previous: &Item, next: &Item) -> bool {
    match (previous, next) {
        (Item::Function(before), Item::Function(after)) => {
            before.name == after.name || (before.params.is_empty() && after.params.is_empty())
        }
        (Item::Spec(before), Item::Function(after)) => before.name == after.name,
        (Item::Callback(_), Item::Callback(_)) => true,
        _ => false,
    }
}

fn render_item(w: &mut Writer, node: &Item) {
    match node {
        Item::Attribute(attribute) => render_attribute(w, attribute),
        Item::Defstruct(fields) => render_defstruct(w, fields),
        Item::TypeAlias(alias) => render_type_alias(w, alias),
        Item::Spec(signature) => render_signature(w, "@spec ", signature),
        Item::Callback(signature) => render_signature(w, "@callback ", signature),
        Item::Function(function) => render_function(w, function),
        Item::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_attribute(w: &mut Writer, node: &Attribute) {
    doc_comment(w, &node.doc);
    let mut head = String::from("@");
    head.push_str(node.name.as_str());
    head.push(' ');
    render_value(w, &head, &node.value);
}

/// `defstruct [ … ]`.
///
/// The bracketed form always, including for the empty case:
/// `defstruct []` is a struct with no fields, which is legal and is what
/// an IDL aggregate with no members becomes.
fn render_defstruct(w: &mut Writer, fields: &[Field]) {
    if fields.is_empty() {
        w.line("defstruct []");
        return;
    }
    let pairs: Vec<(Ident, Expr)> = fields
        .iter()
        .map(|field| (field.name.clone(), field.default.clone()))
        .collect();
    render_value(w, "defstruct ", &Expr::Keyword(pairs));
}

fn render_type_alias(w: &mut Writer, node: &TypeAlias) {
    doc_comment(w, &node.doc);
    let mut head = String::from("@type ");
    head.push_str(node.name.as_str());
    head.push_str(" :: ");
    render_type_value(w, &head, &node.type_);
}

fn render_signature(w: &mut Writer, marker: &str, node: &Signature) {
    doc_comment(w, &node.doc);
    let mut line = String::from(marker);
    line.push_str(node.name.as_str());
    line.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    line.push_str(&params.join(", "));
    line.push_str(") :: ");
    line.push_str(&type_text(&node.result));
    w.line(&line);
}

fn param_text(param: &Param) -> String {
    let mut out = String::new();
    if let Some(name) = &param.name {
        out.push_str(name.as_str());
        out.push_str(" :: ");
    }
    out.push_str(&type_text(&param.type_));
    out
}

/// `def name(patterns), do: body`.
///
/// The one-line body form, because every function this column writes has
/// a body of one expression: a `do`/`end` block for `do: @limit` would
/// be three lines saying the same thing.
fn render_function(w: &mut Writer, node: &Function) {
    doc_comment(w, &node.doc);
    let mut line = String::from("def ");
    line.push_str(node.name.as_str());
    if !node.params.is_empty() {
        line.push('(');
        let params: Vec<String> = node.params.iter().map(expr_text).collect();
        line.push_str(&params.join(", "));
        line.push(')');
    }
    line.push_str(", do: ");
    line.push_str(&expr_text(&node.body));
    w.line(&line);
}

/// The column a line is broken at.
///
/// `mix format`'s own default. A member list of twenty entries on one
/// line is legal and unreadable, and the attribute is the part of a
/// generated file a reader goes to: it is where every width, bound and
/// IDL spelling is written down.
const LINE_LENGTH: usize = 98;

/// Whether `head` and `text` fit on one line at the writer's depth.
fn fits(w: &Writer, head: &str, text: &str) -> bool {
    w.depth()
        .saturating_mul(2)
        .saturating_add(head.len())
        .saturating_add(text.len())
        <= LINE_LENGTH
}

/// A value written after `head`, broken over lines when it does not fit.
fn render_value(w: &mut Writer, head: &str, value: &Expr) {
    if fits(w, head, &expr_text(value)) {
        let mut line = String::from(head);
        line.push_str(&expr_text(value));
        w.line(&line);
        return;
    }
    match value {
        Expr::List(items) if items.len() > 1 => {
            let mut line = String::from(head);
            line.push('[');
            w.line(&line);
            w.indent();
            for (index, item) in items.iter().enumerate() {
                let mut text = expr_text(item);
                if index + 1 < items.len() {
                    text.push(',');
                }
                w.line(&text);
            }
            w.dedent();
            w.line("]");
        }
        Expr::Keyword(pairs) if pairs.len() > 1 => {
            let mut line = String::from(head);
            line.push('[');
            w.line(&line);
            w.indent();
            for (index, (key, item)) in pairs.iter().enumerate() {
                let mut text = keyword_text(key, item);
                if index + 1 < pairs.len() {
                    text.push(',');
                }
                w.line(&text);
            }
            w.dedent();
            w.line("]");
        }
        other => {
            let mut line = String::from(head);
            line.push_str(&expr_text(other));
            w.line(&line);
        }
    }
}

/// A type written after `head`, broken over lines when it does not fit.
fn render_type_value(w: &mut Writer, head: &str, type_: &Type) {
    if fits(w, head, &type_text(type_)) {
        let mut line = String::from(head);
        line.push_str(&type_text(type_));
        w.line(&line);
        return;
    }
    match type_ {
        Type::Struct(fields) if fields.len() > 1 => {
            let mut line = String::from(head);
            line.push_str("%__MODULE__{");
            w.line(&line);
            w.indent();
            for (index, (key, field)) in fields.iter().enumerate() {
                let mut text = String::from(key.as_str());
                text.push_str(": ");
                text.push_str(&type_text(field));
                if index + 1 < fields.len() {
                    text.push(',');
                }
                w.line(&text);
            }
            w.dedent();
            w.line("}");
        }
        other => {
            let mut line = String::from(head);
            line.push_str(&type_text(other));
            w.line(&line);
        }
    }
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/adapter/elixir.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Basic(basic) => basic.as_str().to_string(),
        Type::Atom(name) => atom_text(name),
        Type::Local(name) => {
            let mut out = String::from(name.as_str());
            out.push_str("()");
            out
        }
        Type::Remote { module, name } => {
            let mut out = path_text(module);
            out.push('.');
            out.push_str(name.as_str());
            out.push_str("()");
            out
        }
        Type::List(element) => {
            let mut out = String::from("list(");
            out.push_str(&type_text(element));
            out.push(')');
            out
        }
        Type::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(type_text).collect();
            let mut out = String::from("{");
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Type::Struct(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(key, field)| {
                    let mut out = String::from(key.as_str());
                    out.push_str(": ");
                    out.push_str(&type_text(field));
                    out
                })
                .collect();
            let mut out = String::from("%__MODULE__{");
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Type::Union(arms) => {
            let parts: Vec<String> = arms.iter().map(type_text).collect();
            parts.join(" | ")
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_elixir.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Atom(name) => atom_text(name),
        Expr::Var(name) => name.as_str().to_string(),
        Expr::Attr(name) => {
            let mut out = String::from("@");
            out.push_str(name.as_str());
            out
        }
        Expr::Module(path) => path_text(path),
        Expr::List(items) => {
            let parts: Vec<String> = items.iter().map(expr_text).collect();
            let mut out = String::from("[");
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Expr::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(expr_text).collect();
            let mut out = String::from("{");
            out.push_str(&parts.join(", "));
            out.push('}');
            out
        }
        Expr::Keyword(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(key, value)| keyword_text(key, value))
                .collect();
            let mut out = String::from("[");
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Expr::Call { func, args } => {
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            let mut out = path_text(func);
            out.push('(');
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Binary { op, lhs, rhs } => {
            format!("{} {op} {}", expr_text(lhs), expr_text(rhs))
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
            out
        }
        Expr::Paren(value) => format!("({})", expr_text(value)),
        Expr::Field { value, name } => format!("{}.{}", expr_text(value), name.as_str()),
        // `Enum.at` and not the `Access` bracket: a generated list is
        // dense — the codec sized it — so a `nil` from a short list is a
        // generator defect that shows up as a refused write, which is
        // louder than an exception here would be quiet.
        Expr::Index { value, index } => {
            format!("Enum.at({}, {})", expr_text(value), expr_text(index))
        }
        Expr::IfElse {
            condition,
            then,
            otherwise,
        } => format!(
            "(if {}, do: {}, else: {})",
            expr_text(condition),
            expr_text(then),
            expr_text(otherwise)
        ),
        Expr::StructLiteral { module, fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(key, value)| keyword_text(key, value))
                .collect();
            format!("%{}{{{}}}", path_text(module), parts.join(", "))
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_elixir.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn keyword_text(key: &Ident, value: &Expr) -> String {
    let mut out = String::new();
    out.push_str(key.as_str());
    out.push_str(": ");
    out.push_str(&expr_text(value));
    out
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Int(text) => text.clone(),
        Literal::Float(text) => float_text(text),
        Literal::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // The code point as a number. An IDL `char` is one octet and an
        // IDL `wchar` two, and Elixir's `?a` is a code point — the
        // conversion the member's width asks for is then explicit in the
        // attribute rather than implicit in a literal that happens to
        // fit.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_elixir(text));
            out.push('"');
            out
        }
        // `<<1, 2>>` — a binary, which is what a run of octets is in
        // Elixir. A charlist `[1, 2]` would be a list of integers and
        // would compare unequal to the binary the wire carries.
        Literal::Bytes(bytes) => {
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("{b}")).collect();
            let mut out = String::from("<<");
            out.push_str(&parts.join(", "));
            out.push_str(">>");
            out
        }
        Literal::Null => "nil".to_string(),
    }
}

/// A float literal Elixir reads as the number the source wrote.
///
/// Elixir's float syntax requires a digit, a point and a digit before
/// any exponent: `1e10` is rejected with "invalid character e after
/// number 1" (measured, Elixir 1.20.2), where C, C++, Java and Rust all
/// accept it. The value is never changed here — a point and a zero are
/// added where the spelling needs them, and nothing else.
fn float_text(text: &str) -> String {
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    };
    let mut out = String::new();
    if mantissa.starts_with('.') {
        out.push('0');
    } else if let Some(rest) = mantissa.strip_prefix('-').filter(|r| r.starts_with('.')) {
        out.push_str("-0");
        out.push_str(rest);
        return with_exponent(out, exponent);
    }
    out.push_str(mantissa);
    if out.ends_with('.') {
        out.push('0');
    }
    if !out.contains('.') {
        out.push_str(".0");
    }
    with_exponent(out, exponent)
}

fn with_exponent(mut text: String, exponent: Option<&str>) -> String {
    if let Some(exponent) = exponent {
        text.push('e');
        text.push_str(exponent);
    }
    text
}

/// An atom, quoted only where it has to be.
///
/// Every atom this column writes comes from
/// `zerodds_lang_cst::elixir::atom_name` and needs no quotes, but the
/// renderer takes what it is given: an atom whose text is not a plain
/// identifier is written `:"…"` rather than produced as text that does
/// not parse.
fn atom_text(name: &str) -> String {
    let plain = !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first == '_')
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '?' || ch == '!');
    let mut out = String::from(":");
    if plain {
        out.push_str(name);
    } else {
        out.push('"');
        out.push_str(&escape_elixir(name));
        out.push('"');
    }
    out
}

fn path_text(path: &Path) -> String {
    path.segments()
        .iter()
        .map(|segment| segment.as_str())
        .collect::<Vec<&str>>()
        .join(".")
}

/// Escape a string for a double-quoted Elixir literal.
///
/// The C-family escapes, and one more: `#` becomes `\#`. `"a#{b}"` is
/// interpolation and `"a#{}"` compiles to `"a"` (measured, Elixir
/// 1.20.2), so a `const string` whose value contains `#{` would
/// otherwise put a different string in the generated file than the IDL
/// wrote — or fail to compile, if the text between the braces is not an
/// expression.
fn escape_elixir(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            '#' => out.push_str("\\#"),
            other => out.push(other),
        }
    }
    out
}

/// Escape one line of a heredoc.
///
/// A heredoc interpolates exactly as a double-quoted string does, so `#`
/// is escaped for the same reason. The quotes are not: a `"` inside a
/// heredoc is an ordinary character, and only a line that is `"""` on
/// its own would close it.
fn escape_heredoc(line: &str) -> String {
    line.replace('#', "\\#")
}

fn doc_comment(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "# ", doc.as_lines());
}

/// A double-quoted Elixir string literal, escaped.
///
/// Public for the probe generator, which writes harness text rather
/// than a tree and still has to spell strings the way this renderer
/// does — `#` included, because `#{` interpolates.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&escape_elixir(text));
    out.push('"');
    out
}

// -- the codec lowering ---------------------------------------------------
//
// The codec statement layer of `zerodds_lang_cst::elixir` is imperative
// and Elixir is not: a rebinding inside an `if` branch or an
// `Enum.reduce` body does not survive the block. The lowering here is
// what makes the two meet, and it rests on one computable fact — every
// binding a statement makes is named on the statement — so the renderer
// can thread exactly the changed bindings out of each block as a tuple.
//
// Three rules, each measured against Elixir 1.20.2:
//
// 1. **A block threads what it rebinds and was bound before it.** A
//    variable first bound inside a branch is branch-local; one bound
//    before and rebound inside comes out through the accumulator tuple,
//    which both branches of an `if` and the tail of every loop body
//    answer with. A variable read at the end of its block this way is
//    never "unused", so the generated file compiles without warnings.
// 2. **`break` and early `return` are `throw`.** A `catch` clause that
//    does not match re-raises, so the innermost loop's
//    `{:zd_break, acc}` handler and the routine's `{:zd_return, value}`
//    handler compose without inspecting each other.
// 3. **A counted loop ranges `0..(n - 1)//1`.** Without the step,
//    `0..-1` is a *descending* two-element range and a zero-bound loop
//    would run twice.

/// Rendering state one routine's lowering carries.
struct Lowering {
    /// How many `While` loops have been named, for `zd_loop<n>`.
    loops_named: usize,
    /// The accumulator tuple of each enclosing loop, innermost last —
    /// what a `Break` throws.
    loop_accs: Vec<Vec<String>>,
    /// Every name an enclosing context reads after the statements at
    /// hand — the routine's result variables, and each enclosing
    /// block's accumulator names, which its trailing tuple reads. What
    /// decides whether a block's outer binding of a threaded name is a
    /// binding or an `_`-prefixed one; a name nothing reads again and
    /// bound anyway is a warning, and warnings are failures in this
    /// column's compile gates.
    outer_reads: Vec<String>,
}

/// Render one codec routine.
#[must_use]
pub fn codec_function(node: &CodecFunction) -> String {
    let mut w = Writer::new("  ");
    doc_comment(&mut w, &node.doc);
    let mut head = String::from("def ");
    head.push_str(node.name.as_str());
    if !node.params.is_empty() {
        head.push('(');
        let params: Vec<&str> = node.params.iter().map(Ident::as_str).collect();
        head.push_str(&params.join(", "));
        head.push(')');
    }
    head.push_str(" do");
    w.line(&head);
    w.indent();
    let needs_try = contains_return(&node.body);
    if needs_try {
        w.line("try do");
        w.indent();
    }
    let mut bound: Vec<String> = node
        .params
        .iter()
        .map(|param| param.as_str().to_string())
        .collect();
    let mut result_reads = Vec::new();
    expr_reads(&node.result, &mut result_reads);
    let mut lowering = Lowering {
        loops_named: 0,
        loop_accs: Vec::new(),
        outer_reads: result_reads,
    };
    let body = flatten(&node.body);
    render_statements(&mut w, &body, &mut bound, &mut lowering);
    w.line(&expr_text(&node.result));
    if needs_try {
        w.dedent();
        w.line("catch");
        w.indent();
        w.line(":throw, {:zd_return, zd_ret} -> zd_ret");
        w.dedent();
        w.line("end");
    }
    w.dedent();
    w.line("end");
    w.finish()
}

/// Every name the statements bind, in first-assignment order.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn assigned(statements: &[Statement], out: &mut Vec<String>) {
    let push = |name: &Ident, out: &mut Vec<String>| {
        let text = name.as_str();
        if text != "_" && !out.iter().any(|seen| seen == text) {
            out.push(text.to_string());
        }
    };
    for statement in statements {
        match statement {
            Statement::Bind { name, .. } => push(name, out),
            Statement::BindTuple { names, .. } => {
                for name in names {
                    push(name, out);
                }
            }
            Statement::Update { base, .. } => push(base, out),
            Statement::If {
                then, otherwise, ..
            } => {
                assigned(then, out);
                assigned(otherwise, out);
            }
            // The counter is scoped to the body and never threads.
            Statement::For { body, .. } | Statement::While { body, .. } => assigned(body, out),
            Statement::Seq(inner) => assigned(inner, out),
            Statement::Break | Statement::Return(_) | Statement::Comment(_) => {}
        }
    }
}

/// The names a block rebinds *of those already bound*, in the order they
/// were first bound — the accumulator tuple, and a deterministic one.
fn threaded(statements: &[Statement], bound: &[String]) -> Vec<String> {
    let mut inside = Vec::new();
    assigned(statements, &mut inside);
    bound
        .iter()
        .filter(|name| inside.iter().any(|assigned| assigned == *name))
        .cloned()
        .collect()
}

/// Whether a `Break` sits in these statements outside any nested loop.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn breaks_at_this_level(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::Break => true,
        Statement::If {
            then, otherwise, ..
        } => breaks_at_this_level(then) || breaks_at_this_level(otherwise),
        Statement::Seq(inner) => breaks_at_this_level(inner),
        // A nested loop catches its own breaks.
        Statement::For { .. } | Statement::While { .. } => false,
        _ => false,
    })
}

/// Whether any `Return` appears anywhere, loops included.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn contains_return(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::Return(_) => true,
        Statement::If {
            then, otherwise, ..
        } => contains_return(then) || contains_return(otherwise),
        Statement::For { body, .. } | Statement::While { body, .. } => contains_return(body),
        Statement::Seq(inner) => contains_return(inner),
        _ => false,
    })
}

/// The statements with every [`Statement::Seq`] spliced into its
/// parts, recursively — what makes "the statements after this one" a
/// plain slice for the used-later analysis.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn flatten(statements: &[Statement]) -> Vec<Statement> {
    let mut out = Vec::new();
    for statement in statements {
        match statement {
            Statement::Seq(inner) => out.extend(flatten(inner)),
            Statement::If {
                condition,
                then,
                otherwise,
            } => out.push(Statement::If {
                condition: condition.clone(),
                then: flatten(then),
                otherwise: flatten(otherwise),
            }),
            Statement::For {
                counter,
                bound,
                body,
            } => out.push(Statement::For {
                counter: counter.clone(),
                bound: bound.clone(),
                body: flatten(body),
            }),
            Statement::While { condition, body } => out.push(Statement::While {
                condition: condition.clone(),
                body: flatten(body),
            }),
            other => out.push(other.clone()),
        }
    }
    out
}

/// Every variable `expr` reads, appended to `out`.
///
/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_elixir.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_reads(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Var(name) => {
            if !out.iter().any(|seen| seen == name.as_str()) {
                out.push(name.as_str().to_string());
            }
        }
        Expr::Call { args, .. } => {
            for argument in args {
                expr_reads(argument, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            expr_reads(lhs, out);
            expr_reads(rhs, out);
        }
        Expr::Unary { value, .. } | Expr::Paren(value) => expr_reads(value, out),
        Expr::Field { value, .. } => expr_reads(value, out),
        Expr::Index { value, index } => {
            expr_reads(value, out);
            expr_reads(index, out);
        }
        Expr::IfElse {
            condition,
            then,
            otherwise,
        } => {
            expr_reads(condition, out);
            expr_reads(then, out);
            expr_reads(otherwise, out);
        }
        Expr::List(items) | Expr::Tuple(items) => {
            for item in items {
                expr_reads(item, out);
            }
        }
        Expr::Keyword(pairs) => {
            for (_, value) in pairs {
                expr_reads(value, out);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                expr_reads(value, out);
            }
        }
        Expr::Literal(_) | Expr::Atom(_) | Expr::Attr(_) | Expr::Module(_) => {}
    }
}

fn expr_mentions(expr: &Expr, name: &str) -> bool {
    let mut reads = Vec::new();
    expr_reads(expr, &mut reads);
    reads.iter().any(|read| read == name)
}

/// Whether the statements read `name`.
///
/// An assignment inside a *nested block* counts as a read, because the
/// block's own lowering reads the binding into its accumulator; a plain
/// rebinding at this level does not.
///
/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
fn statements_read(statements: &[Statement], name: &str) -> bool {
    statements.iter().any(|statement| match statement {
        Statement::Bind { value, .. } | Statement::BindTuple { value, .. } => {
            expr_mentions(value, name)
        }
        Statement::Update { base, path, value } => {
            base.as_str() == name
                || expr_mentions(value, name)
                || path.iter().any(|step| match step {
                    Access::At(index) => expr_mentions(index, name),
                    Access::Key(_) => false,
                })
        }
        Statement::If {
            condition,
            then,
            otherwise,
        } => {
            let mut inside = Vec::new();
            assigned(then, &mut inside);
            assigned(otherwise, &mut inside);
            expr_mentions(condition, name)
                || inside.iter().any(|bound| bound == name)
                || statements_read(then, name)
                || statements_read(otherwise, name)
        }
        Statement::For { bound, body, .. } => {
            let mut inside = Vec::new();
            assigned(body, &mut inside);
            expr_mentions(bound, name)
                || inside.iter().any(|assigned| assigned == name)
                || statements_read(body, name)
        }
        Statement::While { condition, body } => {
            let mut inside = Vec::new();
            assigned(body, &mut inside);
            expr_mentions(condition, name)
                || inside.iter().any(|assigned| assigned == name)
                || statements_read(body, name)
        }
        Statement::Return(value) => expr_mentions(value, name),
        Statement::Seq(inner) => statements_read(inner, name),
        Statement::Break | Statement::Comment(_) => false,
    })
}

/// The outer binding pattern of a block's threaded names: each one
/// bound plainly where something still reads it — the following
/// statements, an enclosing block's trailing tuple, the routine's
/// result — and `_`-prefixed where nothing does.
fn binding_pattern(names: &[String], lowering: &Lowering, tail: &[Statement]) -> String {
    let parts: Vec<String> = names
        .iter()
        .map(|name| {
            if statements_read(tail, name) || lowering.outer_reads.iter().any(|read| read == name) {
                name.clone()
            } else {
                format!("_{name}")
            }
        })
        .collect();
    format!("{{{}}}", parts.join(", "))
}

/// Enclosing-context reads gain this block's accumulator names while
/// its body renders: the trailing tuple reads every one of them.
fn push_outer_reads(lowering: &mut Lowering, names: &[String]) -> usize {
    let mut pushed = 0;
    for name in names {
        if !lowering.outer_reads.iter().any(|read| read == name) {
            lowering.outer_reads.push(name.clone());
            pushed += 1;
        }
    }
    pushed
}

fn pop_outer_reads(lowering: &mut Lowering, pushed: usize) {
    for _ in 0..pushed {
        lowering.outer_reads.pop();
    }
}

/// `{a, b}` over the threaded names, `{}` over none.
fn acc_tuple(names: &[String]) -> String {
    let mut out = String::from("{");
    out.push_str(
        &names
            .iter()
            .map(String::as_str)
            .collect::<Vec<&str>>()
            .join(", "),
    );
    out.push('}');
    out
}

fn bind_name(bound: &mut Vec<String>, name: &Ident) {
    let text = name.as_str();
    if text != "_" && !bound.iter().any(|seen| seen == text) {
        bound.push(text.to_string());
    }
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
#[allow(clippy::too_many_lines)]
fn render_statements(
    w: &mut Writer,
    statements: &[Statement],
    bound: &mut Vec<String>,
    lowering: &mut Lowering,
) {
    for (index, statement) in statements.iter().enumerate() {
        let tail = &statements[index + 1..];
        match statement {
            // Flattened before rendering; unreachable rather than
            // wrong, and rendered as its parts if it ever arrives.
            Statement::Seq(inner) => render_statements(w, inner, bound, lowering),
            Statement::Comment(doc) => doc_comment(w, doc),
            Statement::Bind { name, value } => {
                w.line(&format!("{} = {}", name.as_str(), expr_text(value)));
                bind_name(bound, name);
            }
            Statement::BindTuple { names, value } => {
                let pattern: Vec<&str> = names.iter().map(Ident::as_str).collect();
                w.line(&format!(
                    "{{{}}} = {}",
                    pattern.join(", "),
                    expr_text(value)
                ));
                for name in names {
                    bind_name(bound, name);
                }
            }
            Statement::Update { base, path, value } => {
                render_update(w, base, path, value);
                bind_name(bound, base);
            }
            Statement::If {
                condition,
                then,
                otherwise,
            } => render_if(w, condition, then, otherwise, bound, lowering, tail),
            Statement::For {
                counter,
                bound: limit,
                body,
            } => render_for(w, counter, limit, body, bound, lowering, tail),
            Statement::While { condition, body } => {
                render_while(w, condition, body, bound, lowering, tail);
            }
            Statement::Break => {
                let acc = lowering
                    .loop_accs
                    .last()
                    .map_or_else(|| String::from("{}"), |names| acc_tuple(names));
                w.line(&format!("throw({{:zd_break, {acc}}})"));
            }
            Statement::Return(value) => {
                w.line(&format!("throw({{:zd_return, {}}})", expr_text(value)));
            }
        }
    }
}

/// `base = %{base | f: v}` for one field, `put_in/3` for a deeper path.
fn render_update(w: &mut Writer, base: &Ident, path: &[Access], value: &Expr) {
    if let [Access::Key(field)] = path {
        w.line(&format!(
            "{base} = %{{{base} | {field}: {value}}}",
            base = base.as_str(),
            field = field.as_str(),
            value = expr_text(value)
        ));
        return;
    }
    let steps: Vec<String> = path
        .iter()
        .map(|step| match step {
            // `Access.key!`, which reaches into a struct; bare
            // `Access.key` answers a default for a missing key, and a
            // missing key here is a generator defect that must raise.
            Access::Key(field) => format!("Access.key!(:{})", field.as_str()),
            Access::At(index) => format!("Access.at({})", expr_text(index)),
        })
        .collect();
    w.line(&format!(
        "{base} = put_in({base}, [{path}], {value})",
        base = base.as_str(),
        path = steps.join(", "),
        value = expr_text(value)
    ));
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
#[allow(clippy::too_many_arguments)]
fn render_if(
    w: &mut Writer,
    condition: &Expr,
    then: &[Statement],
    otherwise: &[Statement],
    bound: &[String],
    lowering: &mut Lowering,
    tail: &[Statement],
) {
    let mut both: Vec<Statement> = then.to_vec();
    both.extend(otherwise.iter().cloned());
    let names = threaded(&both, bound);
    if names.is_empty() {
        if then.is_empty() && otherwise.is_empty() {
            return;
        }
        w.line(&format!("if {} do", expr_text(condition)));
        w.indent();
        if then.is_empty() {
            // An empty suite is not Elixir; `nil` is the value it means.
            w.line("nil");
        } else {
            let mut branch_bound = bound.to_vec();
            render_statements(w, then, &mut branch_bound, lowering);
        }
        w.dedent();
        if !otherwise.is_empty() {
            w.line("else");
            w.indent();
            let mut branch_bound = bound.to_vec();
            render_statements(w, otherwise, &mut branch_bound, lowering);
            w.dedent();
        }
        w.line("end");
        return;
    }
    let acc = acc_tuple(&names);
    w.line(&format!("{} =", binding_pattern(&names, lowering, tail)));
    w.indent();
    w.line(&format!("if {} do", expr_text(condition)));
    w.indent();
    let pushed = push_outer_reads(lowering, &names);
    let mut branch_bound = bound.to_vec();
    render_statements(w, then, &mut branch_bound, lowering);
    w.line(&acc);
    w.dedent();
    w.line("else");
    w.indent();
    let mut branch_bound = bound.to_vec();
    render_statements(w, otherwise, &mut branch_bound, lowering);
    w.line(&acc);
    w.dedent();
    w.line("end");
    w.dedent();
    pop_outer_reads(lowering, pushed);
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
#[allow(clippy::too_many_arguments)]
fn render_for(
    w: &mut Writer,
    counter: &Ident,
    limit: &Expr,
    body: &[Statement],
    bound: &[String],
    lowering: &mut Lowering,
    tail: &[Statement],
) {
    let names = threaded(body, bound);
    let breaks = breaks_at_this_level(body);
    let range = format!("0..({} - 1)//1", expr_text(limit));
    if names.is_empty() && !breaks {
        w.line(&format!("Enum.each({range}, fn {} ->", counter.as_str()));
        w.indent();
        if body.is_empty() {
            w.line("nil");
        } else {
            let mut body_bound = bound.to_vec();
            body_bound.push(counter.as_str().to_string());
            lowering.loop_accs.push(Vec::new());
            render_statements(w, body, &mut body_bound, lowering);
            lowering.loop_accs.pop();
        }
        w.dedent();
        w.line("end)");
        return;
    }
    let acc = acc_tuple(&names);
    if !names.is_empty() {
        w.line(&format!("{} =", binding_pattern(&names, lowering, tail)));
        w.indent();
    }
    if breaks {
        w.line("try do");
        w.indent();
    }
    w.line(&format!(
        "Enum.reduce({range}, {acc}, fn {}, {acc} ->",
        counter.as_str()
    ));
    w.indent();
    let pushed = push_outer_reads(lowering, &names);
    let mut body_bound = bound.to_vec();
    body_bound.push(counter.as_str().to_string());
    lowering.loop_accs.push(names.clone());
    render_statements(w, body, &mut body_bound, lowering);
    lowering.loop_accs.pop();
    pop_outer_reads(lowering, pushed);
    w.line(&acc);
    w.dedent();
    w.line("end)");
    if breaks {
        w.dedent();
        w.line("catch");
        w.indent();
        w.line(":throw, {:zd_break, zd_brk} -> zd_brk");
        w.dedent();
        w.line("end");
    }
    if !names.is_empty() {
        w.dedent();
    }
}

/// zerodds-lint: recursion-depth = the generated body's block nesting.
/// The codec walk emits a routine per type rather than inlining one
/// (`lang-rules/src/emit/encode.rs`), so nothing here follows the type
/// graph. What does grow with the IDL is the dispatch chain: a union's
/// cases, an enumeration's enumerators and a `@mutable` aggregate's
/// members each become one `If` per arm, in the previous arm's `else`.
/// The model bounds none of the three counts, and Elixir nests the arms in
/// its own output too, so they cannot be walked iteratively the way
/// `c.rs` walks them. No number is claimed: this signature returns no
/// error, so nothing here could report one.
#[allow(clippy::too_many_arguments)]
fn render_while(
    w: &mut Writer,
    condition: &Expr,
    body: &[Statement],
    bound: &[String],
    lowering: &mut Lowering,
    tail: &[Statement],
) {
    let names = threaded(body, bound);
    let breaks = breaks_at_this_level(body);
    let acc = acc_tuple(&names);
    let loop_name = format!("zd_loop{}", lowering.loops_named);
    lowering.loops_named += 1;
    w.line(&format!("{loop_name} = fn {loop_name}, {acc} ->"));
    w.indent();
    w.line(&format!("if {} do", expr_text(condition)));
    w.indent();
    let pushed = push_outer_reads(lowering, &names);
    let mut body_bound = bound.to_vec();
    lowering.loop_accs.push(names.clone());
    render_statements(w, body, &mut body_bound, lowering);
    lowering.loop_accs.pop();
    pop_outer_reads(lowering, pushed);
    w.line(&format!("{loop_name}.({loop_name}, {acc})"));
    w.dedent();
    w.line("else");
    w.indent();
    w.line(&acc);
    w.dedent();
    w.line("end");
    w.dedent();
    w.line("end");
    let call = format!("{loop_name}.({loop_name}, {acc})");
    let pattern = binding_pattern(&names, lowering, tail);
    if breaks {
        if !names.is_empty() {
            w.line(&format!("{pattern} ="));
            w.indent();
        }
        w.line("try do");
        w.indent();
        w.line(&call);
        w.dedent();
        w.line("catch");
        w.indent();
        w.line(":throw, {:zd_break, zd_brk} -> zd_brk");
        w.dedent();
        w.line("end");
        if !names.is_empty() {
            w.dedent();
        }
    } else if names.is_empty() {
        w.line(&call);
    } else {
        w.line(&format!("{pattern} = {call}"));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;
    use zerodds_lang_cst::elixir::{Basic, Field};

    fn module_of(body: Vec<Item>) -> Module {
        Module {
            doc: Doc::none(),
            name: Path::single("Probe"),
            body,
        }
    }

    #[test]
    fn a_module_is_a_do_end_block_indented_by_two_spaces() {
        let text = module(&module_of(vec![Item::Defstruct(vec![Field {
            name: Ident::new("x"),
            default: Expr::int(0),
        }])]));
        assert_eq!(text, "defmodule Probe do\n  defstruct [x: 0]\nend\n");
    }

    #[test]
    fn a_module_name_joins_its_segments_with_dots() {
        let text = module(&Module {
            doc: Doc::none(),
            name: Path::new(&["Common", "Util", "Location"]),
            body: Vec::new(),
        });
        assert!(
            text.starts_with("defmodule Common.Util.Location do"),
            "{text}"
        );
    }

    #[test]
    fn a_moduledoc_is_a_heredoc_at_the_top_of_the_body() {
        let text = module(&Module {
            doc: Doc::lines(&["first", "second"]),
            name: Path::single("Probe"),
            body: Vec::new(),
        });
        assert_eq!(
            text,
            "defmodule Probe do\n  @moduledoc \"\"\"\n  first\n  second\n  \"\"\"\nend\n"
        );
    }

    #[test]
    fn a_struct_with_no_fields_is_defstruct_with_an_empty_list() {
        let text = item(&Item::Defstruct(Vec::new()));
        assert_eq!(text, "defstruct []\n");
    }

    #[test]
    fn a_struct_of_several_short_fields_stays_on_one_line() {
        let text = item(&Item::Defstruct(vec![
            Field {
                name: Ident::new("x"),
                default: Expr::int(0),
            },
            Field {
                name: Ident::new("pose"),
                default: Expr::nil(),
            },
        ]));
        assert_eq!(text, "defstruct [x: 0, pose: nil]\n");
    }

    #[test]
    fn a_struct_too_wide_for_a_line_takes_one_field_each() {
        let text = item(&Item::Defstruct(vec![
            Field {
                name: Ident::new("a_rather_long_member_name"),
                default: Expr::string("a default long enough to push this past the column"),
            },
            Field {
                name: Ident::new("another_long_member_name"),
                default: Expr::nil(),
            },
        ]));
        assert_eq!(
            text,
            "defstruct [\n  a_rather_long_member_name: \"a default long enough to push \
             this past the column\",\n  another_long_member_name: nil\n]\n"
        );
    }

    #[test]
    fn a_struct_type_too_wide_for_a_line_takes_one_field_each() {
        let text = item(&Item::TypeAlias(TypeAlias {
            doc: Doc::none(),
            name: Ident::new("t"),
            type_: Type::Struct(vec![
                (
                    Ident::new("a_rather_long_member_name"),
                    Type::Basic(Basic::Integer),
                ),
                (
                    Ident::new("another_long_member_name"),
                    Type::Basic(Basic::Float),
                ),
                (
                    Ident::new("a_third_long_member_name"),
                    Type::Basic(Basic::Binary),
                ),
            ]),
        }));
        assert_eq!(
            text,
            "@type t :: %__MODULE__{\n  a_rather_long_member_name: integer(),\n  \
             another_long_member_name: float(),\n  a_third_long_member_name: binary()\n}\n"
        );
    }

    #[test]
    fn a_struct_type_that_fits_stays_on_its_line() {
        let text = item(&Item::TypeAlias(TypeAlias {
            doc: Doc::none(),
            name: Ident::new("t"),
            type_: Type::Struct(vec![
                (Ident::new("x"), Type::Basic(Basic::Integer)),
                (Ident::new("y"), Type::Basic(Basic::Float)),
            ]),
        }));
        assert_eq!(text, "@type t :: %__MODULE__{x: integer(), y: float()}\n");
    }

    #[test]
    fn every_type_form_writes_its_own_spelling() {
        assert_eq!(type_(&Type::Basic(Basic::Integer)), "integer()");
        assert_eq!(type_(&Type::Basic(Basic::Nil)), "nil");
        assert_eq!(type_(&Type::Atom("idle".to_string())), ":idle");
        assert_eq!(type_(&Type::Local(Ident::new("t"))), "t()");
        assert_eq!(
            type_(&Type::Remote {
                module: Path::new(&["Common", "Pose"]),
                name: Ident::new("t"),
            }),
            "Common.Pose.t()"
        );
        assert_eq!(
            type_(&Type::list(Type::Basic(Basic::Integer))),
            "list(integer())"
        );
        assert_eq!(
            type_(&Type::Tuple(vec![
                Type::Basic(Basic::Integer),
                Type::Basic(Basic::Binary)
            ])),
            "{integer(), binary()}"
        );
        assert_eq!(
            type_(&Type::Basic(Basic::Integer).or_nil()),
            "integer() | nil"
        );
    }

    #[test]
    fn an_attribute_of_a_long_list_takes_one_line_per_entry() {
        let text = item(&Item::Attribute(Attribute {
            doc: Doc::none(),
            name: Ident::new("zd_members"),
            value: Expr::List(vec![
                Expr::Tuple(vec![
                    Expr::Atom("x".to_string()),
                    Expr::Keyword(vec![
                        (Ident::new("idl"), Expr::string("x")),
                        (Ident::new("kind"), Expr::Atom("int32".to_string())),
                        (Ident::new("bits"), Expr::int(32)),
                    ]),
                ]),
                Expr::Tuple(vec![
                    Expr::Atom("y".to_string()),
                    Expr::Keyword(vec![
                        (Ident::new("idl"), Expr::string("y")),
                        (Ident::new("kind"), Expr::Atom("float64".to_string())),
                        (Ident::new("bits"), Expr::int(64)),
                        (Ident::new("signed"), Expr::Literal(Literal::Bool(true))),
                    ]),
                ]),
            ]),
        }));
        assert_eq!(
            text,
            "@zd_members [\n  {:x, [idl: \"x\", kind: :int32, bits: 32]},\n  \
             {:y, [idl: \"y\", kind: :float64, bits: 64, signed: true]}\n]\n"
        );
    }

    #[test]
    fn a_function_writes_its_body_on_the_same_line() {
        let text = item(&Item::Function(Function {
            doc: Doc::none(),
            name: Ident::new("limit"),
            params: Vec::new(),
            body: Expr::Attr(Ident::new("limit")),
        }));
        assert_eq!(text, "def limit, do: @limit\n");
    }

    #[test]
    fn a_function_with_patterns_writes_them_in_parentheses() {
        let text = item(&Item::Function(Function {
            doc: Doc::none(),
            name: Ident::new("from_value"),
            params: vec![Expr::int(0)],
            body: Expr::Tuple(vec![
                Expr::Atom("ok".to_string()),
                Expr::Atom("idle".to_string()),
            ]),
        }));
        assert_eq!(text, "def from_value(0), do: {:ok, :idle}\n");
    }

    #[test]
    fn a_spec_and_a_callback_differ_only_in_their_marker() {
        let signature = Signature {
            doc: Doc::none(),
            name: Ident::new("status"),
            params: vec![Param {
                name: Some(Ident::new("target")),
                type_: Type::Basic(Basic::Binary),
            }],
            result: Type::Basic(Basic::Integer),
        };
        assert_eq!(
            item(&Item::Spec(signature.clone())),
            "@spec status(target :: binary()) :: integer()\n"
        );
        assert_eq!(
            item(&Item::Callback(signature)),
            "@callback status(target :: binary()) :: integer()\n"
        );
    }

    #[test]
    fn a_float_literal_gets_the_point_elixir_requires() {
        // `1e10` is rejected by Elixir and accepted by C, C++, Java and
        // Rust, so the shared spelling cannot be handed straight
        // through. The value does not change.
        assert_eq!(float_text("1e10"), "1.0e10");
        assert_eq!(float_text("1"), "1.0");
        assert_eq!(float_text("1.5"), "1.5");
        assert_eq!(float_text("1.5e-3"), "1.5e-3");
        assert_eq!(float_text(".5"), "0.5");
        assert_eq!(float_text("-.5"), "-0.5");
        assert_eq!(float_text("2."), "2.0");
    }

    #[test]
    fn a_string_literal_escapes_the_interpolation_marker() {
        // `"a#{}"` compiles to `"a"`, so an unescaped `#` would put a
        // different string in the file than the IDL wrote.
        assert_eq!(expr(&Expr::string("a#{b}c")), "\"a\\#{b}c\"".to_string());
        assert_eq!(expr(&Expr::string("plain")), "\"plain\"");
        assert_eq!(expr(&Expr::string("a\"b")), "\"a\\\"b\"");
    }

    #[test]
    fn an_atom_that_is_not_an_identifier_is_quoted() {
        assert_eq!(expr(&Expr::Atom("idle".to_string())), ":idle");
        assert_eq!(expr(&Expr::Atom("has space".to_string())), ":\"has space\"");
        assert_eq!(expr(&Expr::Atom("Upper".to_string())), ":\"Upper\"");
    }

    #[test]
    fn a_module_name_in_a_value_is_written_as_the_alias() {
        // The alias evaluates to `:"Elixir.Common.Pose"`; writing the
        // alias is what a reader recognises and is the same value.
        assert_eq!(
            expr(&Expr::Module(Path::new(&["Common", "Pose"]))),
            "Common.Pose"
        );
    }

    #[test]
    fn a_byte_string_is_a_binary_and_not_a_charlist() {
        assert_eq!(
            expr(&Expr::Literal(Literal::Bytes(vec![1, 255]))),
            "<<1, 255>>"
        );
    }

    #[test]
    fn a_char_literal_is_written_as_its_code_unit() {
        assert_eq!(expr(&Expr::Literal(Literal::Char('A'))), "65");
    }

    #[test]
    fn a_file_writes_its_comment_then_its_modules() {
        let text = file(&File {
            doc: Doc::line("Generated."),
            modules: vec![TopLevel::Module(module_of(Vec::new()))],
        });
        assert_eq!(text, "# Generated.\n\ndefmodule Probe do\nend\n");
    }
}
