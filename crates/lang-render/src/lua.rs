// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Lua CST. Spec §5.
//!
//! Two spaces per level. Lua has no `gofmt` and no formatter a generated
//! file has to agree with, so the width is chosen for the content this
//! column produces: a type descriptor is a table of tables of tables, and
//! at four spaces the innermost member sits past column sixty before its
//! first key.
//!
//! Nothing is decided here. There is one layout rule and it reads off
//! the node in hand: **a table is written on one line when no entry
//! carries a comment and no entry's value is itself a table.** Nothing is
//! lost either way — the same entries in the same order — and a renderer
//! that had to be told which form to use would be carrying a decision
//! the tree could not show.
//!
//! ## Two comment markers, and they are not interchangeable
//!
//! `--` is read by nothing. `---` is read by `lua-language-server`, which
//! attaches the block to the declaration that follows it. So
//! [`zerodds_lang_cst::lua::Stat::Comment`] renders with `--` and
//! [`zerodds_lang_cst::lua::Stat::Annotation`] with `---`, and the file
//! header renders with `--` because there is no declaration it is about.
//! Neither is read by the Lua runtime.

use crate::writer::{Writer, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::lua::{Expr, File, Function, Key, Place, Stat, Table};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::new("  ");
    plain(&mut w, &file.doc);
    if !file.doc.is_empty() {
        w.blank_line();
    }
    for statement in &file.body {
        // A statement that carries a comment starts a block; one that
        // does not belongs with what came before it. That is the second
        // and last layout rule in this file, and like the first it reads
        // off the node: `M.Pose = { … }` and the registry line under it
        // are one thing, and the next `---` block is another.
        if opens_block(statement) {
            w.blank_line();
        }
        render_stat(&mut w, statement);
    }
    w.blank_line();
    // The chunk's own `return`, which is what `require` binds.
    w.push("return ");
    render_expr(&mut w, &file.returns);
    w.newline();
    for text in &file.end_text {
        verbatim_lines(&mut w, text);
    }
    w.finish()
}

/// Render one statement, for a caller that has only one.
#[must_use]
pub fn stat(node: &Stat) -> String {
    let mut w = Writer::new("  ");
    render_stat(&mut w, node);
    w.finish()
}

/// Render one expression, for a caller that has only one.
///
/// Without the line break [`Writer::finish`] ends the open line with: a
/// statement is a line and an expression is not, and a caller splicing
/// this into one would get a break in the middle of it.
#[must_use]
pub fn expr(node: &Expr) -> String {
    let mut w = Writer::new("  ");
    render_expr(&mut w, node);
    let mut out = w.finish();
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

/// Whether a statement begins a block of its own.
fn opens_block(node: &Stat) -> bool {
    match node {
        Stat::Local { doc, .. } | Stat::Assign { doc, .. } => !doc.is_empty(),
        Stat::Function(function) => !function.doc.is_empty(),
        Stat::Comment(doc) | Stat::Annotation(doc) => !doc.is_empty(),
        Stat::Verbatim(_) => true,
        Stat::Return(_)
        | Stat::Perform(_)
        | Stat::If { .. }
        | Stat::While { .. }
        | Stat::NumericFor { .. }
        | Stat::Break => false,
    }
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stat(w: &mut Writer, node: &Stat) {
    match node {
        Stat::Local { doc, name, value } => {
            annotation(w, doc);
            w.push("local ");
            w.push(name.as_str());
            if let Some(value) = value {
                w.push(" = ");
                render_expr(w, value);
            }
            w.newline();
        }
        Stat::Assign { doc, target, value } => {
            annotation(w, doc);
            render_place(w, target);
            w.push(" = ");
            render_expr(w, value);
            w.newline();
        }
        Stat::Function(function) => render_function(w, function),
        Stat::Return(value) => {
            w.push("return");
            if let Some(value) = value {
                w.push(" ");
                render_expr(w, value);
            }
            w.newline();
        }
        Stat::Comment(doc) => plain(w, doc),
        Stat::Annotation(doc) => annotation(w, doc),
        Stat::Verbatim(text) => verbatim_lines(w, text),
        Stat::Perform(value) => {
            render_expr(w, value);
            w.newline();
        }
        Stat::If {
            condition,
            then,
            otherwise,
        } => {
            render_if(w, condition, then, otherwise.as_deref(), "if ");
            w.line("end");
        }
        Stat::While { condition, body } => {
            w.push("while ");
            render_expr(w, condition);
            w.push(" do");
            w.newline();
            w.indent();
            for statement in body {
                render_stat(w, statement);
            }
            w.dedent();
            w.line("end");
        }
        Stat::NumericFor {
            counter,
            from,
            to,
            body,
        } => {
            w.push("for ");
            w.push(counter.as_str());
            w.push(" = ");
            render_expr(w, from);
            w.push(", ");
            render_expr(w, to);
            w.push(" do");
            w.newline();
            w.indent();
            for statement in body {
                render_stat(w, statement);
            }
            w.dedent();
            w.line("end");
        }
        Stat::Break => w.line("break"),
    }
}

/// One `if`/`elseif` arm, and whatever follows it.
///
/// An `otherwise` that holds exactly one further [`Stat::If`] renders as
/// `elseif` rather than as a nested block. The flattening changes no
/// meaning — Lua defines `elseif` as exactly that nesting — and it is
/// what keeps a `@mutable` aggregate's member dispatch from indenting
/// once per member: the walk builds one `If` per member, each in the
/// previous one's `else`, and a hundred members would otherwise nest a
/// hundred levels toward the parser's own depth limit.
///
/// The next arm is taken in a loop and not by a call to this function
/// again: the member count that would have nested the output a hundred
/// levels would equally have been the renderer's own stack depth, and
/// nothing in the model bounds a union's case count, an enumeration's
/// enumerator count or a `@mutable` aggregate's member count.
///
/// zerodds-lint: recursion-depth 64 (block nesting only; the arm chain
/// is walked iteratively, and what remains is one level per nested block
/// of a generated body — the codec walk emits a routine per type rather
/// than inlining, so a body nests by its framing and by one loop per
/// array dimension, never by the type graph)
fn render_if(
    w: &mut Writer,
    condition: &Expr,
    then: &[Stat],
    otherwise: Option<&[Stat]>,
    keyword: &str,
) {
    let mut condition = condition;
    let mut then = then;
    let mut otherwise = otherwise;
    let mut keyword = keyword;
    loop {
        w.push(keyword);
        render_expr(w, condition);
        w.push(" then");
        w.newline();
        w.indent();
        for statement in then {
            render_stat(w, statement);
        }
        w.dedent();
        match otherwise {
            None => return,
            Some(
                [
                    Stat::If {
                        condition: next_condition,
                        then: next_then,
                        otherwise: next_otherwise,
                    },
                ],
            ) => {
                condition = next_condition;
                then = next_then;
                otherwise = next_otherwise.as_deref();
                keyword = "elseif ";
            }
            Some(rest) => {
                w.line("else");
                w.indent();
                for statement in rest {
                    render_stat(w, statement);
                }
                w.dedent();
                return;
            }
        }
    }
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_function(w: &mut Writer, node: &Function) {
    annotation(w, &node.doc);
    w.push("function ");
    render_place(w, &node.target);
    w.push("(");
    let params: Vec<String> = node
        .params
        .iter()
        .map(|param| param.as_str().to_string())
        .collect();
    w.push(&params.join(", "));
    w.push(")");
    w.newline();
    w.indent();
    for statement in &node.body {
        render_stat(w, statement);
    }
    w.dedent();
    w.line("end");
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_lua.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn render_place(w: &mut Writer, node: &Place) {
    match node {
        Place::Name(name) => w.push(name.as_str()),
        Place::Field { base, name } => {
            render_place(w, base);
            w.push(".");
            w.push(name.as_str());
        }
        Place::Index { base, key } => {
            render_place(w, base);
            w.push("[");
            render_expr(w, key);
            w.push("]");
        }
        Place::Paren(inner) => {
            w.push("(");
            render_expr(w, inner);
            w.push(")");
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_lua.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn render_expr(w: &mut Writer, node: &Expr) {
    match node {
        Expr::Table(table) if !one_line(table) => render_table(w, table),
        Expr::Table(_) | Expr::Nil | Expr::Literal(_) | Expr::Place(_) => w.push(&expr_text(node)),
        Expr::Call { func, args } => {
            render_place(w, func);
            w.push("(");
            for (index, arg) in args.iter().enumerate() {
                if index > 0 {
                    w.push(", ");
                }
                render_expr(w, arg);
            }
            w.push(")");
        }
        Expr::Binary { op, lhs, rhs } => {
            render_expr(w, lhs);
            w.push(" ");
            w.push(op);
            w.push(" ");
            render_expr(w, rhs);
        }
        Expr::Unary { op, value } => {
            w.push(op);
            if op.chars().all(|c| c.is_ascii_alphabetic()) {
                w.push(" ");
            }
            render_expr(w, value);
        }
        Expr::Paren(inner) => {
            w.push("(");
            render_expr(w, inner);
            w.push(")");
        }
    }
}

/// A table written one entry per line.
///
/// The separator goes after the last entry too. Lua's grammar allows it
/// (reference §3.4.9, `fieldlist`), and it makes appending an entry a
/// one-line diff rather than a two-line one.
///
/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_lua.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn render_table(w: &mut Writer, table: &Table) {
    w.push("{");
    w.newline();
    w.indent();
    for entry in &table.entries {
        annotation(w, &entry.doc);
        render_key(w, &entry.key);
        render_expr(w, &entry.value);
        w.push(",");
        w.newline();
    }
    w.dedent();
    w.push("}");
}

fn render_key(w: &mut Writer, key: &Key) {
    match key {
        Key::Positional => {}
        Key::Name(name) => {
            w.push(name.as_str());
            w.push(" = ");
        }
        Key::Expr(expr) => {
            w.push("[");
            render_expr(w, expr);
            w.push("] = ");
        }
    }
}

/// Whether a table fits on one line.
///
/// The only layout decision in this file, and it reads off the node: a
/// table goes on one line when no entry carries a comment there would be
/// nowhere to put, and no entry's value is a table of its own. A
/// descriptor holding a member list therefore nests, and a member
/// holding only scalars does not.
fn one_line(table: &Table) -> bool {
    table
        .entries
        .iter()
        .all(|entry| entry.doc.is_empty() && !matches!(entry.value, Expr::Table(_)))
}

/// One expression on one line.
///
/// Reached for a table only through [`one_line`]; a caller that wants
/// the nested form goes through [`render_expr`], which can indent it
/// against the statement.
///
/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_lua.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Nil => "nil".to_string(),
        Expr::Literal(literal) => literal_text(literal),
        Expr::Place(place) => place_text(place),
        Expr::Table(table) => {
            if table.is_empty() {
                return "{}".to_string();
            }
            let mut out = String::from("{ ");
            for (index, entry) in table.entries.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&key_text(&entry.key));
                out.push_str(&expr_text(&entry.value));
            }
            out.push_str(" }");
            out
        }
        Expr::Call { func, args } => {
            let mut out = place_text(func);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
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
            let mut out = String::from(*op);
            if op.chars().all(|c| c.is_ascii_alphabetic()) {
                out.push(' ');
            }
            out.push_str(&expr_text(value));
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

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_lua.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn key_text(key: &Key) -> String {
    match key {
        Key::Positional => String::new(),
        Key::Name(name) => {
            let mut out = name.as_str().to_string();
            out.push_str(" = ");
            out
        }
        Key::Expr(expr) => {
            let mut out = String::from("[");
            out.push_str(&expr_text(expr));
            out.push_str("] = ");
            out
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_lua.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn place_text(node: &Place) -> String {
    match node {
        Place::Name(name) => name.as_str().to_string(),
        Place::Field { base, name } => {
            let mut out = place_text(base);
            out.push('.');
            out.push_str(name.as_str());
            out
        }
        Place::Index { base, key } => {
            let mut out = place_text(base);
            out.push('[');
            out.push_str(&expr_text(key));
            out.push(']');
            out
        }
        Place::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
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
        // A Lua string is a run of octets, so a character is written as
        // a one-character string rather than as a form of its own: Lua
        // has no character type at all.
        Literal::Char(ch) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(&alloc::string::String::from(*ch)));
            out.push('"');
            out
        }
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
        // Lua has no byte-string form distinct from a string: both are a
        // run of octets. The octets are written as decimal escapes, so
        // the literal is ASCII whatever it holds.
        Literal::Bytes(bytes) => {
            let mut out = String::from("\"");
            for byte in bytes {
                out.push_str(&alloc::format!("\\{byte:03}"));
            }
            out.push('"');
            out
        }
        Literal::Null => "nil".to_string(),
    }
}

/// Escape a string for a Lua double-quoted literal.
///
/// Not [`crate::writer::escape_string`], and the difference is a defect
/// that form would produce here. A Lua decimal escape reads **up to
/// three** digits (Lua 5.4 reference §3.1), so `\0` followed by the
/// character `1` is `\01`, which is the octet 1 and not two octets. Every
/// escape written here is therefore three digits wide, and the ambiguity
/// cannot arise.
///
/// Everything outside the control range goes through unchanged, the
/// non-ASCII included: a Lua string is a sequence of octets with no
/// encoding of its own, so the UTF-8 of a `wstring` holder is written as
/// itself.
#[must_use]
pub fn escape_string(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 || other as u32 == 0x7f => {
                out.push_str(&alloc::format!("\\{:03}", other as u32));
            }
            other => out.push(other),
        }
    }
    out
}

/// A `--` block: a note read by nothing.
fn plain(w: &mut Writer, doc: &Doc) {
    marked(w, "--", doc);
}

/// A `---` block: what `lua-language-server` attaches to the declaration
/// below it.
fn annotation(w: &mut Writer, doc: &Doc) {
    marked(w, "---", doc);
}

/// A comment block, one line per entry, each with `marker`.
///
/// The space between the marker and the text is left out where the line
/// begins with `@`. A LuaCATS tag is read immediately after the dashes —
/// `---@class Pose` — and the space is not part of that spelling; a
/// prose line takes it, so that `--- A note` reads as prose.
fn marked(w: &mut Writer, marker: &str, doc: &Doc) {
    for line in doc.as_lines() {
        if line.is_empty() {
            w.line(marker);
            continue;
        }
        let mut text = String::from(marker);
        if !line.starts_with('@') {
            text.push(' ');
        }
        text.push_str(line);
        w.line(&text);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;
    use zerodds_lang_cst::lua::{self, Entry, table_key};

    fn name(text: &str) -> Place {
        Place::name(text)
    }

    #[test]
    fn a_file_ends_with_the_return_its_module_needs() {
        let rendered = file(&File {
            end_text: Vec::new(),
            doc: Doc::line("A header."),
            body: vec![Stat::Local {
                doc: Doc::none(),
                name: Ident::new("M"),
                value: Some(Expr::Table(Table::default())),
            }],
            returns: Expr::place(name("M")),
        });
        assert_eq!(rendered, "-- A header.\n\nlocal M = {}\n\nreturn M\n");
    }

    #[test]
    fn a_note_takes_two_dashes_and_an_annotation_three() {
        // `lua-language-server` attaches a `---` block to the declaration
        // below it, so a plain note written with `---` would become
        // documentation for something nobody wrote it about.
        assert_eq!(stat(&Stat::Comment(Doc::line("a note"))), "-- a note\n");
        assert_eq!(
            stat(&Stat::Annotation(Doc::line("@class Pose"))),
            "---@class Pose\n"
        );
    }

    #[test]
    fn a_luacats_tag_is_written_against_the_dashes_and_prose_is_not() {
        // `---@class Pose` is the spelling LuaCATS reads; the space is
        // not part of it. A prose line takes the space so that it reads
        // as prose.
        assert_eq!(
            stat(&Stat::Annotation(Doc::lines(&[
                "@field x integer",
                "a note"
            ]))),
            "---@field x integer\n--- a note\n"
        );
    }

    #[test]
    fn a_commented_statement_starts_a_block_and_an_uncommented_one_does_not() {
        // `M.Pose = { … }` and the registry line under it are one thing.
        let rendered = file(&File {
            end_text: Vec::new(),
            doc: Doc::none(),
            body: vec![
                Stat::Assign {
                    doc: Doc::line("the descriptor"),
                    target: name("M").field("Pose"),
                    value: Expr::int(1),
                },
                Stat::Assign {
                    doc: Doc::none(),
                    target: name("M").field("registry"),
                    value: Expr::int(2),
                },
                Stat::Assign {
                    doc: Doc::line("the next block"),
                    target: name("M").field("Other"),
                    value: Expr::int(3),
                },
            ],
            returns: Expr::place(name("M")),
        });
        assert_eq!(
            rendered,
            "--- the descriptor\nM.Pose = 1\nM.registry = 2\n\n\
             --- the next block\nM.Other = 3\n\nreturn M\n"
        );
    }

    #[test]
    fn an_empty_comment_line_carries_no_trailing_space() {
        let rendered = stat(&Stat::Comment(Doc::lines(&["first", "", "third"])));
        assert_eq!(rendered, "-- first\n--\n-- third\n");
    }

    #[test]
    fn a_table_of_scalars_is_written_on_one_line() {
        let table = Table::of(vec![
            Entry::named("name", Expr::string("x")),
            Entry::named("bits", Expr::int(32)),
        ]);
        assert_eq!(expr(&Expr::Table(table)), "{ name = \"x\", bits = 32 }");
    }

    #[test]
    fn a_table_holding_a_table_is_written_one_entry_per_line() {
        let table = Table::of(vec![Entry::named(
            "members",
            Expr::Table(Table::of(vec![Entry::positional(Expr::int(1))])),
        )]);
        // The outer table nests because an entry's value is a table; the
        // inner one does not, because its own entries are scalars.
        assert_eq!(expr(&Expr::Table(table)), "{\n  members = { 1 },\n}");
    }

    #[test]
    fn a_nested_table_indents_against_the_statement_that_holds_it() {
        // The reason expressions render into the writer rather than into
        // a string: a continuation line has to know how deep it is.
        let rendered = stat(&Stat::Assign {
            doc: Doc::none(),
            target: name("M").field("Pose"),
            value: Expr::Table(Table::of(vec![Entry::named(
                "members",
                Expr::Table(Table::of(vec![Entry::positional(Expr::Table(Table::of(
                    vec![Entry::named("name", Expr::string("x"))],
                )))])),
            )])),
        });
        assert_eq!(
            rendered,
            "M.Pose = {\n  members = {\n    { name = \"x\" },\n  },\n}\n"
        );
    }

    #[test]
    fn an_entry_with_a_comment_forces_the_nested_form() {
        // There is nowhere to put a comment on a one-line table, and
        // dropping it would be the silent loss this project is arranged
        // against.
        let table = Table::of(vec![Entry {
            doc: Doc::line("why"),
            key: table_key("a"),
            value: Expr::int(1),
        }]);
        assert_eq!(expr(&Expr::Table(table)), "{\n  --- why\n  a = 1,\n}");
    }

    #[test]
    fn a_keyword_key_is_written_in_the_bracket_form() {
        let table = Table::of(vec![Entry::named("end", Expr::int(1))]);
        assert_eq!(expr(&Expr::Table(table)), "{ [\"end\"] = 1 }");
    }

    #[test]
    fn a_keyword_field_of_a_place_is_written_in_the_bracket_form() {
        let place = name("M").field("common").field("end");
        assert_eq!(expr(&Expr::place(place)), "M.common[\"end\"]");
    }

    #[test]
    fn a_function_renders_with_its_body_indented() {
        let rendered = stat(&Stat::Function(Function {
            doc: Doc::line("@return Pose"),
            target: name("M").field("Pose").field("new"),
            params: Vec::new(),
            body: vec![Stat::Return(Some(Expr::Table(Table::of(vec![
                Entry::named("x", Expr::int(0)),
            ]))))],
        }));
        assert_eq!(
            rendered,
            "---@return Pose\nfunction M.Pose.new()\n  return { x = 0 }\nend\n"
        );
    }

    #[test]
    fn a_call_renders_its_arguments_in_order() {
        let call = Expr::Call {
            func: alloc::boxed::Box::new(name("assert")),
            args: vec![
                Expr::place(name("M").field("Point")),
                Expr::string("not declared"),
            ],
        };
        assert_eq!(expr(&call), "assert(M.Point, \"not declared\")");
    }

    #[test]
    fn a_binary_expression_writes_the_operator_it_was_given() {
        let shift = Expr::Binary {
            op: "<<",
            lhs: alloc::boxed::Box::new(Expr::int(1)),
            rhs: alloc::boxed::Box::new(Expr::int(4)),
        };
        assert_eq!(expr(&shift), "1 << 4");
    }

    #[test]
    fn nil_and_the_booleans_are_written_as_lua_spells_them() {
        assert_eq!(expr(&Expr::Nil), "nil");
        assert_eq!(expr(&Expr::Literal(Literal::Null)), "nil");
        assert_eq!(expr(&Expr::bool(true)), "true");
        assert_eq!(expr(&Expr::bool(false)), "false");
    }

    #[test]
    fn a_float_keeps_the_form_that_makes_math_type_answer_float() {
        assert_eq!(
            expr(&Expr::Literal(Literal::Float(lua::float_text("0")))),
            "0.0"
        );
    }

    #[test]
    fn a_control_character_escape_is_three_digits_wide() {
        // A Lua decimal escape reads up to three digits, so `"\0" .. "1"`
        // written as `"\01"` would be one octet and not two.
        assert_eq!(escape_string("\u{0}1"), "\\0001");
        assert_eq!(escape_string("a\"b"), "a\\\"b");
        assert_eq!(escape_string("a\\b"), "a\\\\b");
        assert_eq!(escape_string("a\nb"), "a\\nb");
        assert_eq!(escape_string("plain"), "plain");
    }

    #[test]
    fn a_non_ascii_character_goes_through_as_its_own_octets() {
        // A Lua string has no encoding of its own, so the UTF-8 a
        // `wstring` holder carries is written as itself.
        assert_eq!(escape_string("Größe"), "Größe");
    }

    #[test]
    fn a_char_literal_becomes_a_one_character_string() {
        assert_eq!(expr(&Expr::Literal(Literal::Char('a'))), "\"a\"");
    }

    #[test]
    fn a_byte_string_is_written_as_decimal_escapes() {
        assert_eq!(
            expr(&Expr::Literal(Literal::Bytes(vec![0, 1, 255]))),
            "\"\\000\\001\\255\""
        );
    }

    #[test]
    fn an_index_place_renders_its_key_expression() {
        let place = name("M").field("_descriptors");
        let rendered = expr(&Expr::place(Place::Index {
            base: alloc::boxed::Box::new(place),
            key: alloc::boxed::Box::new(Expr::string("::Point")),
        }));
        assert_eq!(rendered, "M._descriptors[\"::Point\"]");
    }

    #[test]
    fn a_bare_return_renders_without_a_value() {
        assert_eq!(stat(&Stat::Return(None)), "return\n");
    }

    #[test]
    fn a_local_without_a_value_renders_as_a_bare_declaration() {
        assert_eq!(
            stat(&Stat::Local {
                doc: Doc::none(),
                name: Ident::new("x"),
                value: None,
            }),
            "local x\n"
        );
    }

    #[test]
    fn an_identifier_renders_exactly_as_it_was_given() {
        assert_eq!(ident(&Ident::new("Pose")), "Pose");
    }

    #[test]
    fn a_call_statement_renders_as_the_call() {
        // Lua's grammar admits a functioncall as a statement and nothing
        // else expression-shaped; the tree's `Perform` is that.
        let rendered = stat(&Stat::Perform(Expr::Call {
            func: alloc::boxed::Box::new(name("zdw").field("write_u32")),
            args: vec![Expr::place(name("w")), Expr::int(7)],
        }));
        assert_eq!(rendered, "zdw.write_u32(w, 7)\n");
    }

    #[test]
    fn an_if_renders_its_two_arms_and_ends_once() {
        let rendered = stat(&Stat::If {
            condition: Expr::Binary {
                op: "==",
                lhs: alloc::boxed::Box::new(Expr::place(name("x"))),
                rhs: alloc::boxed::Box::new(Expr::int(1)),
            },
            then: vec![Stat::Return(Some(Expr::int(1)))],
            otherwise: Some(vec![Stat::Return(Some(Expr::int(2)))]),
        });
        assert_eq!(
            rendered,
            "if x == 1 then\n  return 1\nelse\n  return 2\nend\n"
        );
    }

    #[test]
    fn an_else_holding_one_if_flattens_to_elseif() {
        // The walk builds one `If` per member, each in the previous
        // one's `else`; a hundred members must not nest a hundred deep.
        let inner = Stat::If {
            condition: Expr::Binary {
                op: "==",
                lhs: alloc::boxed::Box::new(Expr::place(name("x"))),
                rhs: alloc::boxed::Box::new(Expr::int(2)),
            },
            then: vec![Stat::Break],
            otherwise: Some(vec![Stat::Return(None)]),
        };
        let rendered = stat(&Stat::If {
            condition: Expr::Binary {
                op: "==",
                lhs: alloc::boxed::Box::new(Expr::place(name("x"))),
                rhs: alloc::boxed::Box::new(Expr::int(1)),
            },
            then: vec![Stat::Break],
            otherwise: Some(vec![inner]),
        });
        assert_eq!(
            rendered,
            "if x == 1 then\n  break\nelseif x == 2 then\n  break\nelse\n  return\nend\n"
        );
    }

    #[test]
    fn a_while_and_a_numeric_for_render_their_bodies_indented() {
        let rendered = stat(&Stat::While {
            condition: Expr::bool(true),
            body: vec![Stat::Break],
        });
        assert_eq!(rendered, "while true do\n  break\nend\n");
        let rendered = stat(&Stat::NumericFor {
            counter: Ident::new("zd_i"),
            from: Expr::int(0),
            to: Expr::Binary {
                op: "-",
                lhs: alloc::boxed::Box::new(Expr::place(name("zd_n"))),
                rhs: alloc::boxed::Box::new(Expr::int(1)),
            },
            body: vec![Stat::Break],
        });
        assert_eq!(rendered, "for zd_i = 0, zd_n - 1 do\n  break\nend\n");
    }

    #[test]
    fn the_unary_operators_space_the_word_and_not_the_symbols() {
        // `notx` is an identifier; `#v` and `~v` take no space.
        let value = || alloc::boxed::Box::new(Expr::place(name("v")));
        assert_eq!(
            expr(&Expr::Unary {
                op: "not",
                value: value()
            }),
            "not v"
        );
        assert_eq!(
            expr(&Expr::Unary {
                op: "#",
                value: value()
            }),
            "#v"
        );
        assert_eq!(
            expr(&Expr::Unary {
                op: "~",
                value: value()
            }),
            "~v"
        );
    }

    #[test]
    fn a_deliberate_parenthesis_survives_as_expression_and_as_place_base() {
        assert_eq!(
            expr(&Expr::Paren(alloc::boxed::Box::new(Expr::int(1)))),
            "(1)"
        );
        let place = Place::Index {
            base: alloc::boxed::Box::new(Place::Paren(alloc::boxed::Box::new(Expr::place(name(
                "f",
            ))))),
            key: alloc::boxed::Box::new(Expr::int(1)),
        };
        assert_eq!(expr(&Expr::place(place)), "(f)[1]");
    }
}
