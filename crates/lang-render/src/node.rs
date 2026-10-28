// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Node CST. Spec §5.
//!
//! Two spaces, the opening brace on the same line, a semicolon on every
//! statement, `//` comments — the shape `prettier`'s defaults and
//! `node --check` both accept without complaint. Nothing is decided
//! here: the tree already carries every choice, down to which operator
//! is written and whether a value needs parentheses.
//!
//! The one JavaScript-specific lexical fact this module owns is
//! [`string_text`]'s escaping, which is the same set C, Java and every
//! other column with a `\0` in its string grammar already use — unlike
//! [`crate::zig`], JavaScript's string literals do accept `\0`
//! (ECMA-262 §12.9.4's `SingleEscapeCharacter`), so there is no
//! Zig-shaped exception here.

use crate::writer::{Writer, comment_lines, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::node::{Expr, File, Function, Stat};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::new("  ");
    if !file.doc.is_empty() {
        comment_lines(&mut w, "// ", file.doc.as_lines());
    }
    for statement in &file.body {
        w.blank_line();
        render_stat(&mut w, statement);
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
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stat(w: &mut Writer, node: &Stat) {
    match node {
        Stat::Declare {
            doc,
            kind,
            name,
            value,
        } => {
            note(w, doc);
            let mut line = String::from(kind.as_str());
            line.push(' ');
            line.push_str(name.as_str());
            if let Some(value) = value {
                line.push_str(" = ");
                line.push_str(&expr_text(value));
            }
            line.push(';');
            w.line(&line);
        }
        Stat::Assign { doc, place, value } => {
            note(w, doc);
            let mut line = expr_text(place);
            line.push_str(" = ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Stat::Function(function) => render_function(w, function),
        Stat::Return(value) => match value {
            Some(value) => {
                let mut line = String::from("return ");
                line.push_str(&expr_text(value));
                line.push(';');
                w.line(&line);
            }
            None => w.line("return;"),
        },
        Stat::Expr(value) => {
            let mut line = expr_text(value);
            line.push(';');
            w.line(&line);
        }
        Stat::If { .. } => render_if(w, node, "if ("),
        Stat::For {
            counter,
            bound,
            body,
        } => {
            let mut head = String::from("for (let ");
            head.push_str(counter.as_str());
            head.push_str(" = 0; ");
            head.push_str(counter.as_str());
            head.push_str(" < ");
            head.push_str(&expr_text(bound));
            head.push_str("; ");
            head.push_str(counter.as_str());
            head.push_str(" += 1) {");
            w.line(&head);
            w.indent();
            for statement in body {
                render_stat(w, statement);
            }
            w.dedent();
            w.line("}");
        }
        Stat::While { condition, body } => {
            let mut head = String::from("while (");
            head.push_str(&expr_text(condition));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            for statement in body {
                render_stat(w, statement);
            }
            w.dedent();
            w.line("}");
        }
        Stat::Break => w.line("break;"),
        Stat::Throw(value) => {
            let mut line = String::from("throw ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Stat::Comment(lines) => comment_lines(w, "// ", lines.as_lines()),
        Stat::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// An `if`, with an `else`-block of exactly one `if` collapsed into
/// `else if`.
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
fn render_if(w: &mut Writer, node: &Stat, keyword: &str) {
    let mut node = node;
    let mut keyword = keyword;
    loop {
        let Stat::If {
            condition,
            then,
            otherwise,
        } = node
        else {
            return;
        };
        let mut head = String::from(keyword);
        head.push_str(&expr_text(condition));
        head.push_str(") {");
        w.line(&head);
        w.indent();
        for statement in then {
            render_stat(w, statement);
        }
        w.dedent();
        match otherwise {
            None => {
                w.line("}");
                return;
            }
            Some(chain) if chain.len() == 1 && matches!(chain[0], Stat::If { .. }) => {
                node = &chain[0];
                keyword = "} else if (";
            }
            Some(block) => {
                w.line("} else {");
                w.indent();
                for statement in block {
                    render_stat(w, statement);
                }
                w.dedent();
                w.line("}");
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
    note(w, &node.doc);
    let mut head = String::from("function ");
    head.push_str(node.name.as_str());
    head.push('(');
    let params: Vec<&str> = node.params.iter().map(Ident::as_str).collect();
    head.push_str(&params.join(", "));
    head.push_str(") {");
    w.line(&head);
    w.indent();
    for statement in &node.body {
        render_stat(w, statement);
    }
    w.dedent();
    w.line("}");
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_node.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Undefined => "undefined".to_string(),
        Expr::Name(name) => name.as_str().to_string(),
        Expr::Array(elements) => {
            if elements.is_empty() {
                return "[]".to_string();
            }
            let mut out = String::from("[");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Expr::Object(fields) => {
            if fields.is_empty() {
                return "{}".to_string();
            }
            let mut out = String::from("{ ");
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| {
                    let mut part = String::from(name.as_str());
                    part.push_str(": ");
                    part.push_str(&expr_text(value));
                    part
                })
                .collect();
            out.push_str(&parts.join(", "));
            out.push_str(" }");
            out
        }
        Expr::New { callee, args } => {
            let mut out = String::from("new ");
            out.push_str(callee.as_str());
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
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
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
            out.push_str(&char_text(*ch));
            out.push('\'');
            out
        }
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&string_text(text));
            out.push('"');
            out
        }
        Literal::Bytes(bytes) => {
            let mut out = String::from("Uint8Array.from([");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02x}")).collect();
            out.push_str(&parts.join(", "));
            out.push_str("])");
            out
        }
        Literal::Null => "null".to_string(),
    }
}

/// Escape a string for a JavaScript double-quoted string literal.
///
/// Unlike [`crate::zig::string_text`], JavaScript's grammar has a `\0`
/// escape (ECMA-262 §12.9.4), so the shared [`crate::writer::escape_string`]
/// would be the right answer here — this is this module's own only so
/// every column keeps the same shape of function at the same name, and
/// so a future divergence (there is none today) has a place already
/// waiting for it.
#[must_use]
pub fn string_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        push_escaped(&mut out, ch, '"');
    }
    out
}

/// The same, for a character literal.
#[must_use]
pub fn char_text(ch: char) -> String {
    let mut out = String::new();
    push_escaped(&mut out, ch, '\'');
    out
}

fn push_escaped(out: &mut String, ch: char, delimiter: char) {
    match ch {
        '\\' => out.push_str("\\\\"),
        '\0' => out.push_str("\\0"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if c == delimiter => {
            out.push('\\');
            out.push(c);
        }
        c if (c as u32) < 0x20 || c as u32 == 0x7F => {
            out.push_str(&alloc::format!("\\x{:02x}", c as u32));
        }
        other => out.push(other),
    }
}

fn note(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "// ", doc.as_lines());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use zerodds_lang_cst::node::{VarKind, escape_decl};

    #[test]
    fn a_const_declaration_ends_in_a_semicolon() {
        let text = stat(&Stat::Declare {
            doc: Doc::none(),
            kind: VarKind::Const,
            name: Ident::new("READY"),
            value: Some(Expr::int(4)),
        });
        assert_eq!(text, "const READY = 4;\n");
    }

    #[test]
    fn an_object_literal_writes_every_key_bare() {
        let text = expr(&Expr::Object(alloc::vec![
            (Ident::new("x"), Expr::int(1)),
            (Ident::new("class"), Expr::int(2)),
        ]));
        assert_eq!(text, "{ x: 1, class: 2 }");
    }

    #[test]
    fn a_for_loop_counts_from_zero_with_no_off_by_one_correction() {
        let text = stat(&Stat::For {
            counter: Ident::new("i"),
            bound: Expr::name("n"),
            body: alloc::vec![Stat::Break],
        });
        assert_eq!(text, "for (let i = 0; i < n; i += 1) {\n  break;\n}\n");
    }

    #[test]
    fn a_function_declaration_has_no_type_annotation_anywhere() {
        let text = stat(&Stat::Function(Function {
            doc: Doc::none(),
            name: Ident::new("zd_enc_p_long"),
            params: alloc::vec![Ident::new("w"), Ident::new("v")],
            body: alloc::vec![Stat::Return(None)],
        }));
        assert_eq!(text, "function zd_enc_p_long(w, v) {\n  return;\n}\n");
        assert!(!text.contains(':'), "a type annotation reached the output");
    }

    #[test]
    fn a_nul_is_written_as_the_named_escape_because_javascript_has_one() {
        assert_eq!(string_text("a\0b"), "a\\0b");
        assert_eq!(string_text("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(string_text("hä"), "hä");
    }

    #[test]
    fn a_reserved_word_still_renders_bare_as_a_property_key() {
        // The renderer never re-decides what the tree already carries:
        // `escape_decl` is for a binding, not for a property, and this
        // tree never routes a property key through it.
        assert_eq!(escape_decl("class"), "class_");
        let text = expr(&Expr::member(Expr::name("v"), "class"));
        assert_eq!(text, "v.class");
    }

    #[test]
    fn a_delete_expression_keeps_its_trailing_space() {
        let text = expr(&Expr::Unary {
            op: "delete ",
            value: alloc::boxed::Box::new(Expr::member(Expr::name("v"), "m")),
        });
        assert_eq!(text, "delete v.m");
    }

    #[test]
    fn an_if_else_if_chain_stays_flat() {
        let text = stat(&Stat::If {
            condition: Expr::name("a"),
            then: alloc::vec![Stat::Break],
            otherwise: Some(alloc::vec![Stat::If {
                condition: Expr::name("b"),
                then: alloc::vec![Stat::Break],
                otherwise: None,
            }]),
        });
        assert_eq!(text, "if (a) {\n  break;\n} else if (b) {\n  break;\n}\n");
    }
}
