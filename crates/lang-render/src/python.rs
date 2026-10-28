// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Python CST. Spec §5.
//!
//! One style, PEP 8: four spaces, two blank lines around a top-level
//! class, one around a nested one, and a docstring as the first statement
//! of every class body.
//!
//! ## Indentation is the grammar here
//!
//! In the other four renderers a wrong indent is untidy output. In Python
//! it is a different program, or none. So the block structure is the only
//! thing this file computes: [`Writer`] owns the depth, every body goes
//! through `render_body`, and nothing else in this module writes a
//! newline.
//!
//! ## The output is pure ASCII
//!
//! Every code point outside printable ASCII is escaped, in string
//! literals and in docstrings alike. A generated file that is pure ASCII
//! cannot be changed by an editor, a terminal or a CI runner that guessed
//! the wrong encoding — and a wire test whose expected string arrived
//! mangled would report a byte difference that is really a file-transfer
//! difference.

use crate::writer::{Writer, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::python::{
    Annotated, Arg, Assign, Class, Decorator, Expr, Function, Import, ImportName, Keyword, Module,
    Param, Statement, Type,
};

/// Render a whole module.
#[must_use]
pub fn module(node: &Module) -> String {
    let mut w = Writer::spaces4();
    docstring(&mut w, &node.doc);
    if !node.future.is_empty() {
        w.blank_line();
        let names: Vec<&str> = node.future.iter().map(Ident::as_str).collect();
        w.line(&format!("from __future__ import {}", names.join(", ")));
    }
    if !node.imports.is_empty() {
        w.blank_line();
        for import in &node.imports {
            w.line(&import_text(import));
        }
    }
    render_body(&mut w, &node.body, true);
    w.finish()
}

/// Render one statement, for a caller that has only one.
#[must_use]
pub fn statement(node: &Statement) -> String {
    let mut w = Writer::spaces4();
    render_statement(&mut w, node);
    w.finish()
}

/// Render one annotation, for a caller that wants its spelling.
#[must_use]
pub fn type_(node: &Type) -> String {
    type_text(node)
}

/// Render one expression, for a caller that wants its spelling.
#[must_use]
pub fn expr(node: &Expr) -> String {
    expr_text(node)
}

fn import_text(import: &Import) -> String {
    match import {
        Import::Module { path, alias } => {
            let mut out = format!("import {}", path_text(path));
            if let Some(alias) = alias {
                out.push_str(" as ");
                out.push_str(alias.as_str());
            }
            out
        }
        Import::From { path, names } => {
            let parts: Vec<String> = names.iter().map(import_name_text).collect();
            format!("from {} import {}", path_text(path), parts.join(", "))
        }
    }
}

fn import_name_text(name: &ImportName) -> String {
    match &name.alias {
        Some(alias) => format!("{} as {}", name.name.as_str(), alias.as_str()),
        None => name.name.as_str().to_string(),
    }
}

/// A run of statements, with the blank lines PEP 8 puts between them.
///
/// `top` says whether this is the module body, which is the only thing
/// that changes: two blank lines around a top-level definition and one
/// around a nested one (PEP 8, "Blank Lines"). Bindings and comments run
/// together, which is what a block of fields should look like.
fn render_body(w: &mut Writer, body: &[Statement], top: bool) {
    let gap = |w: &mut Writer| {
        w.blank_line();
        if top {
            w.newline();
        }
    };
    let mut previous_was_definition = false;
    for (index, node) in body.iter().enumerate() {
        let definition = matches!(node, Statement::Class(_) | Statement::Function(_));
        if (index > 0 && (definition || previous_was_definition)) || (index == 0 && top) {
            gap(w);
        }
        render_statement(w, node);
        previous_was_definition = definition;
    }
}

/// The body of a compound statement — an `if`, a loop, a `def`.
///
/// `pass` where the body is empty. Not the renderer deciding what the
/// tree says: an empty suite is a `SyntaxError` and there is no program
/// the tree could have meant. Every producer in this workspace fills the
/// body itself, so this is the guard and not the mechanism —
/// `crates/lang-rules/src/emit/python.rs` pads at each of the four
/// constructors instead, where it can say *which* body was empty.
fn render_suite(w: &mut Writer, body: &[Statement]) {
    w.indent();
    if body.is_empty() {
        w.line("pass");
    } else {
        render_body(w, body, false);
    }
    w.dedent();
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_statement(w: &mut Writer, node: &Statement) {
    match node {
        Statement::Class(class) => render_class(w, class),
        Statement::Function(function) => render_function(w, function),
        Statement::Annotated(binding) => render_annotated(w, binding),
        Statement::Assign(binding) => render_assign(w, binding),
        Statement::Set { place, value } => {
            w.line(&format!("{} = {}", expr_text(place), expr_text(value)));
        }
        Statement::Expr(value) => w.line(&expr_text(value)),
        // The condition is written bare. `Expr::Binary` brackets itself,
        // so `if (a == b):` is what a comparison renders as and the
        // outer parentheses Python would allow are not added a second
        // time.
        Statement::If { .. } => render_if(w, node, "if"),
        Statement::For {
            counter,
            bound,
            body,
        } => {
            // `range(bound)` and not `range(0, bound)`: the walk's loop
            // is always from zero, and the second argument would be a
            // zero written into every generated loop for nothing.
            w.line(&format!(
                "for {} in range({}):",
                counter.as_str(),
                expr_text(bound)
            ));
            render_suite(w, body);
        }
        Statement::While { condition, body } => {
            w.line(&format!("while {}:", expr_text(condition)));
            render_suite(w, body);
        }
        Statement::Break => w.line("break"),
        Statement::Return(value) => match value {
            Some(value) => w.line(&format!("return {}", expr_text(value))),
            None => w.line("return"),
        },
        Statement::Comment(doc) => comment(w, doc),
        Statement::Pass => w.line("pass"),
        Statement::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// An `if`, and its `else` written as `elif` where that is what it is.
///
/// `elif c:` *is* `else:` followed by an `if c:` — the reference calls
/// the two equivalent (Python 3.13 reference §8.1) — so this changes no
/// program. What it changes is the indentation, and that is not
/// cosmetic here.
///
/// The shared walk builds a member chain as one `If` per member, each in
/// the previous one's `else`. Rendered as nested suites, a `@mutable`
/// aggregate of *n* members indents *n* levels deep, and CPython's
/// tokenizer stops at 100 — `IndentationError: too many levels of
/// indentation`, which is a `SyntaxError`, so the whole module fails to
/// import rather than the one member failing to decode. Every other
/// column's compiler has no such limit, which makes this the one place a
/// chain six columns build identically stops being a program in exactly
/// one of them.
///
/// **Not measured on the corpus**: its largest case declares twenty
/// members and reaches twenty levels. The bound is CPython's, documented
/// and fixed, and it is a property of any IDL a user writes rather than
/// of the cases recorded here — which is the reason to render the chain
/// flat now rather than after somebody's fifty-member type.
///
/// `keyword` is `"if"` at the head of a chain and `"elif"` inside it.
///
/// The next arm is taken in a loop and not by a call to this function
/// again: the member count that would have indented CPython past its
/// limit would equally have been the renderer's own stack depth.
///
/// zerodds-lint: recursion-depth 64 (suite nesting only; the arm chain
/// is walked iteratively, and what remains is one level per nested suite
/// of a generated body — the codec walk emits a routine per type rather
/// than inlining, so a body nests by its framing and by one loop per
/// array dimension, never by the type graph)
fn render_if(w: &mut Writer, node: &Statement, keyword: &str) {
    let mut node = node;
    let mut keyword = keyword;
    loop {
        let Statement::If {
            condition,
            then,
            otherwise,
        } = node
        else {
            // Not reachable: the only caller matches on `Statement::If`
            // first. Rendering the statement plainly is the answer that
            // loses nothing if that ever stops being true.
            render_statement(w, node);
            return;
        };
        w.line(&format!("{keyword} {}:", expr_text(condition)));
        render_suite(w, then);
        let Some(otherwise) = otherwise else { return };
        // One `If` and nothing else is a chain; anything else is a real
        // `else` body. A trailing comment beside the `if` would make it
        // the second, which is right — `elif` has nowhere to put one.
        if let [nested @ Statement::If { .. }] = otherwise.as_slice() {
            node = nested;
            keyword = "elif";
            continue;
        }
        w.line("else:");
        render_suite(w, otherwise);
        return;
    }
}

fn render_function(w: &mut Writer, node: &Function) {
    for decorator in &node.decorators {
        w.line(&decorator_text(decorator));
    }
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    let mut head = format!("def {}({})", node.name.as_str(), params.join(", "));
    if let Some(returns) = &node.returns {
        head.push_str(" -> ");
        head.push_str(&type_text(returns));
    }
    head.push(':');
    w.line(&head);
    w.indent();
    docstring(w, &node.doc);
    // Unlike a class, a `def` needs no blank line after its docstring:
    // PEP 257 asks for one after a *class* docstring and says nothing
    // for a function's, and a generated codec body reads as one run.
    if node.body.is_empty() {
        w.line("pass");
    } else {
        render_body(w, &node.body, false);
    }
    w.dedent();
}

fn param_text(node: &Param) -> String {
    let mut out = node.name.as_str().to_string();
    if let Some(type_) = &node.type_ {
        out.push_str(": ");
        out.push_str(&type_text(type_));
    }
    if let Some(default) = &node.default {
        // ` = ` with spaces where the parameter is annotated and `=`
        // without where it is not. PEP 8, "Function Annotations": the
        // annotation is what changes the spelling, and a generator that
        // wrote one form for both would be wrong for half its output.
        out.push_str(if node.type_.is_some() { " = " } else { "=" });
        out.push_str(&expr_text(default));
    }
    out
}

fn render_class(w: &mut Writer, node: &Class) {
    for decorator in &node.decorators {
        w.line(&decorator_text(decorator));
    }
    let mut head = format!("class {}", node.name.as_str());
    if !node.bases.is_empty() {
        let parts: Vec<String> = node.bases.iter().map(type_text).collect();
        head.push('(');
        head.push_str(&parts.join(", "));
        head.push(')');
    }
    head.push(':');
    w.line(&head);
    w.indent();
    docstring(w, &node.doc);
    // The docstring counts as a statement, so a class carrying one and
    // nothing else is already legal; a class carrying neither is not,
    // and the tree says so with `Statement::Pass` rather than this
    // renderer deciding to add one.
    if !node.doc.is_empty() && !node.body.is_empty() {
        w.blank_line();
    }
    // Inside a class the nesting is one blank line, never two, whatever
    // the module body does.
    render_body(w, &node.body, false);
    w.dedent();
}

fn render_annotated(w: &mut Writer, node: &Annotated) {
    comment(w, &node.doc);
    let mut line = format!("{}: {}", node.name.as_str(), type_text(&node.type_));
    if let Some(value) = &node.value {
        line.push_str(" = ");
        line.push_str(&expr_text(value));
    }
    w.line(&line);
}

fn render_assign(w: &mut Writer, node: &Assign) {
    comment(w, &node.doc);
    w.line(&format!(
        "{} = {}",
        node.name.as_str(),
        expr_text(&node.value)
    ));
}

fn decorator_text(node: &Decorator) -> String {
    let mut out = format!("@{}", path_text(&node.path));
    // A decorator with no arguments is written bare. `@dataclass` and
    // `@dataclass()` are the same thing and the bare form is the
    // conventional one.
    if !node.args.is_empty() {
        out.push('(');
        let parts: Vec<String> = node.args.iter().map(arg_text).collect();
        out.push_str(&parts.join(", "));
        out.push(')');
    }
    out
}

fn arg_text(node: &Arg) -> String {
    match node {
        Arg::Positional(value) => expr_text(value),
        Arg::Keyword(keyword) => keyword_text(keyword),
    }
}

fn keyword_text(node: &Keyword) -> String {
    format!("{}={}", node.name.as_str(), expr_text(&node.value))
}

/// A dotted name.
///
/// [`Path::is_rooted`] is not read: Python has no spelling for a rooted
/// name — `a.b.C` is resolved against the enclosing scopes and there is
/// no `::` to force the module scope — so a materialiser for this tree
/// writes every reference from the module root and builds no rooted
/// paths. A renderer that invented a marker would emit a name Python
/// does not have.
fn path_text(path: &Path) -> String {
    let parts: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
    parts.join(".")
}

fn type_text(node: &Type) -> String {
    match node {
        Type::Name(path) => path_text(path),
        Type::Subscript { base, args } => {
            let parts: Vec<String> = args.iter().map(type_text).collect();
            format!("{}[{}]", path_text(base), parts.join(", "))
        }
        Type::Optional(inner) => format!("typing.Optional[{}]", type_text(inner)),
        Type::Annotated { base, markers } => {
            let mut parts = alloc::vec![type_text(base)];
            parts.extend(markers.iter().map(expr_text));
            format!("typing.Annotated[{}]", parts.join(", "))
        }
    }
}

fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Path(path) => path_text(path),
        Expr::Call { func, args } => {
            let parts: Vec<String> = args.iter().map(arg_text).collect();
            format!("{}({})", path_text(func), parts.join(", "))
        }
        // A one-element tuple needs the trailing comma, or the
        // parentheses are grouping and the value is not a tuple at all.
        Expr::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(expr_text).collect();
            if parts.len() == 1 {
                format!("({},)", parts[0])
            } else {
                format!("({})", parts.join(", "))
            }
        }
        Expr::List(items) => {
            let parts: Vec<String> = items.iter().map(expr_text).collect();
            format!("[{}]", parts.join(", "))
        }
        Expr::Attribute { value, name } => {
            format!("{}.{}", expr_text(value), name.as_str())
        }
        Expr::Index { value, index } => {
            format!("{}[{}]", expr_text(value), expr_text(index))
        }
        Expr::Slice { start, end } => {
            let side = |bound: &Option<alloc::boxed::Box<Expr>>| {
                bound.as_deref().map_or_else(String::new, expr_text)
            };
            format!("{}:{}", side(start), side(end))
        }
        // The operator carries its own trailing space where one is
        // needed — `not ` against `~` — because the tree is where that
        // difference is recorded and not here.
        Expr::Unary { op, value } => format!("{op}{}", expr_text(value)),
        Expr::Paren(inner) => format!("({})", expr_text(inner)),
        Expr::Dict(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{}: {}", expr_text(key), expr_text(value)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        // Parenthesised: the tree records the nesting and precedence is
        // then not something a reader has to reconstruct.
        Expr::Binary { op, lhs, rhs } => {
            format!("({} {op} {})", expr_text(lhs), expr_text(rhs))
        }
        Expr::Annotation(type_) => type_text(type_),
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Int(text) | Literal::Float(text) => text.clone(),
        Literal::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        // Python has no character type: a character is a string of one.
        Literal::Char(character) => {
            let mut text = String::new();
            text.push(*character);
            string_literal(&text)
        }
        Literal::Str(text) => string_literal(text),
        Literal::Bytes(bytes) => {
            let mut out = String::from("b\"");
            for byte in bytes {
                out.push_str(&format!("\\x{byte:02x}"));
            }
            out.push('"');
            out
        }
        Literal::Null => "None".to_string(),
    }
}

/// A double-quoted Python string literal holding `text`, in ASCII.
///
/// Every code point outside printable ASCII becomes `\xXX`, `\uXXXX` or
/// `\UXXXXXXXX`, which Python reads back as the code point it names.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    out.push_str(&escape_ascii(text));
    out.push('"');
    out
}

/// The body of a Python string literal: `text`, escaped down to ASCII.
fn escape_ascii(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => {
                let point = u32::from(other);
                if (0x20..0x7f).contains(&point) {
                    out.push(other);
                } else if point <= 0xff {
                    out.push_str(&format!("\\x{point:02x}"));
                } else if point <= 0xffff {
                    out.push_str(&format!("\\u{point:04x}"));
                } else {
                    out.push_str(&format!("\\U{point:08x}"));
                }
            }
        }
    }
    out
}

/// A docstring, as the first statement of a module or a class body.
///
/// Always the triple-quoted form and always closed on a line of its own
/// when there is more than one line, which is PEP 257. The text is
/// escaped exactly as a string literal is: a docstring is a string, and
/// one carrying a backslash or a quote run would otherwise end early or
/// mean something else.
fn docstring(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    let lines = doc.as_lines();
    if lines.len() == 1 {
        w.line(&format!("\"\"\"{}\"\"\"", escape_ascii(&lines[0])));
        return;
    }
    w.line(&format!("\"\"\"{}", escape_ascii(&lines[0])));
    for line in &lines[1..] {
        if line.is_empty() {
            w.newline();
        } else {
            w.line(&escape_ascii(line));
        }
    }
    w.line("\"\"\"");
}

/// A `#` comment, one line per entry.
fn comment(w: &mut Writer, doc: &Doc) {
    for line in doc.as_lines() {
        if line.is_empty() {
            w.line("#");
        } else {
            w.line(&format!("# {}", escape_ascii(line)));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::boxed::Box;
    use alloc::vec;

    fn field(name: &str, type_: Type) -> Statement {
        Statement::Annotated(Annotated {
            doc: Doc::none(),
            name: Ident::new(name),
            type_,
            value: None,
        })
    }

    #[test]
    fn a_class_body_is_indented_by_four_and_closed_by_the_dedent_alone() {
        // Python has no closing delimiter, so the dedent *is* the end of
        // the block; a renderer that lost it would emit a different
        // program rather than untidy output.
        let text = statement(&Statement::Class(Class {
            doc: Doc::line("A point."),
            decorators: Vec::new(),
            name: Ident::new("Point"),
            bases: Vec::new(),
            body: vec![field("x", Type::name("int"))],
        }));
        assert_eq!(
            text,
            "class Point:\n    \"\"\"A point.\"\"\"\n\n    x: int\n"
        );
    }

    #[test]
    fn a_class_with_a_docstring_and_no_members_is_still_legal_python() {
        let text = statement(&Statement::Class(Class {
            doc: Doc::line("An IDL module."),
            decorators: Vec::new(),
            name: Ident::new("a"),
            bases: Vec::new(),
            body: Vec::new(),
        }));
        assert_eq!(text, "class a:\n    \"\"\"An IDL module.\"\"\"\n");
    }

    #[test]
    fn an_empty_body_takes_the_pass_the_tree_carries() {
        let text = statement(&Statement::Class(Class {
            doc: Doc::none(),
            decorators: Vec::new(),
            name: Ident::new("E"),
            bases: vec![Type::Name(Path::new(&["enum", "IntEnum"]))],
            body: vec![Statement::Pass],
        }));
        assert_eq!(text, "class E(enum.IntEnum):\n    pass\n");
    }

    #[test]
    fn an_import_carries_its_alias_and_its_from_form() {
        assert_eq!(
            import_text(&Import::Module {
                path: Path::new(&["a", "b"]),
                alias: Some(Ident::new("c")),
            }),
            "import a.b as c"
        );
        assert_eq!(
            import_text(&Import::From {
                path: Path::single("typing"),
                names: vec![
                    ImportName {
                        name: Ident::new("Annotated"),
                        alias: None,
                    },
                    ImportName {
                        name: Ident::new("Optional"),
                        alias: Some(Ident::new("Maybe")),
                    },
                ],
            }),
            "from typing import Annotated, Optional as Maybe"
        );
    }

    #[test]
    fn a_call_takes_positional_and_keyword_arguments_in_written_order() {
        assert_eq!(
            expr(&Expr::Call {
                func: Path::single("f"),
                args: vec![
                    Arg::Positional(Expr::int(1)),
                    Arg::Keyword(Keyword {
                        name: Ident::new("b"),
                        value: Expr::Literal(Literal::Bool(false)),
                    }),
                ],
            }),
            "f(1, b=False)"
        );
    }

    #[test]
    fn a_decorator_with_no_arguments_is_written_bare() {
        let node = Decorator {
            path: Path::new(&["dataclasses", "dataclass"]),
            args: Vec::new(),
        };
        assert_eq!(decorator_text(&node), "@dataclasses.dataclass");
        let node = Decorator {
            path: Path::new(&["dataclasses", "dataclass"]),
            args: vec![Arg::Keyword(Keyword {
                name: Ident::new("kw_only"),
                value: Expr::Literal(Literal::Bool(true)),
            })],
        };
        assert_eq!(
            decorator_text(&node),
            "@dataclasses.dataclass(kw_only=True)"
        );
    }

    #[test]
    fn an_annotated_type_keeps_its_metadata_in_the_annotation() {
        let node = Type::Annotated {
            base: Box::new(Type::name("int")),
            markers: vec![Expr::Call {
                func: Path::single("IdlInt"),
                args: vec![
                    Arg::Keyword(Keyword {
                        name: Ident::new("bits"),
                        value: Expr::int(32),
                    }),
                    Arg::Keyword(Keyword {
                        name: Ident::new("signed"),
                        value: Expr::Literal(Literal::Bool(true)),
                    }),
                ],
            }],
        };
        assert_eq!(
            type_(&node),
            "typing.Annotated[int, IdlInt(bits=32, signed=True)]"
        );
    }

    #[test]
    fn an_exact_length_tuple_and_a_list_are_spelled_apart() {
        // The difference a reader has to see: a `list` can be appended
        // to and a `tuple[int, int, int]` cannot even be the wrong
        // length.
        assert_eq!(
            type_(&Type::subscript(
                Path::single("list"),
                vec![Type::name("int")]
            )),
            "list[int]"
        );
        assert_eq!(
            type_(&Type::subscript(
                Path::single("tuple"),
                vec![Type::name("int"), Type::name("int")]
            )),
            "tuple[int, int]"
        );
    }

    #[test]
    fn a_one_element_tuple_keeps_its_trailing_comma() {
        // Without it the parentheses are grouping and the value is not a
        // tuple at all.
        assert_eq!(expr(&Expr::Tuple(vec![Expr::int(1)])), "(1,)");
        assert_eq!(
            expr(&Expr::Tuple(vec![Expr::int(1), Expr::int(2)])),
            "(1, 2)"
        );
    }

    #[test]
    fn every_literal_is_written_in_the_form_python_reads_back() {
        assert_eq!(literal_text(&Literal::Bool(true)), "True");
        assert_eq!(literal_text(&Literal::Null), "None");
        assert_eq!(literal_text(&Literal::Char('a')), "\"a\"");
        assert_eq!(literal_text(&Literal::Int("-1".to_string())), "-1");
        assert_eq!(
            literal_text(&Literal::Bytes(vec![0x00, 0xff])),
            "b\"\\x00\\xff\""
        );
    }

    #[test]
    fn a_string_literal_leaves_the_file_pure_ascii() {
        assert_eq!(string_literal("Z"), "\"Z\"");
        // One byte's worth takes `\xNN`; Python reads it as the code
        // point and not as a byte, which is why a `str` literal may use
        // it at all.
        assert_eq!(string_literal("\u{e4}"), "\"\\xe4\"");
        assert_eq!(string_literal("\u{2603}"), "\"\\u2603\"");
        // Outside the Basic Multilingual Plane, which is the case the
        // wstring length rule turns on.
        assert_eq!(string_literal("\u{1d11e}"), "\"\\U0001d11e\"");
        assert_eq!(string_literal("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn a_docstring_escapes_what_would_close_it_early() {
        let mut w = Writer::spaces4();
        docstring(&mut w, &Doc::line("a \"quoted\" \\ backslash"));
        assert_eq!(w.finish(), "\"\"\"a \\\"quoted\\\" \\\\ backslash\"\"\"\n");
    }

    #[test]
    fn a_multi_line_docstring_closes_on_a_line_of_its_own() {
        let mut w = Writer::spaces4();
        docstring(&mut w, &Doc::lines(&["first", "", "third"]));
        assert_eq!(w.finish(), "\"\"\"first\n\nthird\n\"\"\"\n");
    }

    #[test]
    fn a_module_puts_the_future_import_before_everything_but_the_docstring() {
        // A `from __future__` line that is not the first statement is a
        // SyntaxError, not a style problem.
        let text = module(&Module {
            doc: Doc::line("Generated."),
            future: vec![Ident::new("annotations")],
            imports: vec![Import::Module {
                path: Path::single("dataclasses"),
                alias: None,
            }],
            body: vec![field("X", Type::name("int"))],
        });
        assert_eq!(
            text,
            "\"\"\"Generated.\"\"\"\n\nfrom __future__ import annotations\n\nimport dataclasses\n\n\nX: int\n"
        );
    }

    /// The four compound statements the codec walk builds, each with its
    /// suite indented under it and closed by the dedent alone.
    #[test]
    fn a_compound_statement_indents_its_suite_and_is_closed_by_the_dedent() {
        let effect = |text: &str| Statement::Expr(Expr::name(text));
        assert_eq!(
            statement(&Statement::If {
                condition: Expr::Binary {
                    op: "==",
                    lhs: Box::new(Expr::name("a")),
                    rhs: Box::new(Expr::int(1)),
                },
                then: vec![effect("f()")],
                otherwise: Some(vec![effect("g()")]),
            }),
            "if (a == 1):\n    f()\nelse:\n    g()\n"
        );
        // An `else` holding one `if` and nothing else is a chain, and
        // `elif` is what that tree is. Rendered as nested suites, one
        // `If` per member of a `@mutable` aggregate indents one level
        // per member and CPython stops at 100.
        assert_eq!(
            statement(&Statement::If {
                condition: Expr::name("a"),
                then: vec![effect("f()")],
                otherwise: Some(vec![Statement::If {
                    condition: Expr::name("b"),
                    then: vec![effect("g()")],
                    otherwise: Some(vec![effect("h()")]),
                }]),
            }),
            "if a:\n    f()\nelif b:\n    g()\nelse:\n    h()\n"
        );
        // Two statements in the `else` is a real `else` body and not a
        // chain, because `elif` has nowhere to put the second.
        assert_eq!(
            statement(&Statement::If {
                condition: Expr::name("a"),
                then: vec![effect("f()")],
                otherwise: Some(vec![
                    Statement::If {
                        condition: Expr::name("b"),
                        then: vec![effect("g()")],
                        otherwise: None,
                    },
                    effect("h()"),
                ]),
            }),
            "if a:\n    f()\nelse:\n    if b:\n        g()\n    h()\n"
        );
        assert_eq!(
            statement(&Statement::For {
                counter: Ident::new("i"),
                bound: Expr::name("n"),
                body: vec![effect("f()")],
            }),
            "for i in range(n):\n    f()\n"
        );
        assert_eq!(
            statement(&Statement::While {
                condition: Expr::name("more"),
                body: vec![Statement::Break],
            }),
            "while more:\n    break\n"
        );
        assert_eq!(
            statement(&Statement::Return(Some(Expr::name("v")))),
            "return v\n"
        );
        assert_eq!(statement(&Statement::Return(None)), "return\n");
    }

    /// An empty suite is a `SyntaxError`, not untidy output.
    #[test]
    fn an_empty_suite_takes_a_pass_rather_than_rendering_to_nothing() {
        assert_eq!(
            statement(&Statement::If {
                condition: Expr::name("a"),
                then: Vec::new(),
                otherwise: None,
            }),
            "if a:\n    pass\n"
        );
        assert_eq!(
            statement(&Statement::Function(Function {
                doc: Doc::none(),
                decorators: Vec::new(),
                name: Ident::new("f"),
                params: Vec::new(),
                returns: None,
                body: Vec::new(),
            })),
            "def f():\n    pass\n"
        );
    }

    #[test]
    fn a_function_writes_its_annotations_and_its_return_type() {
        let text = statement(&Statement::Function(Function {
            doc: Doc::line("Write one sample."),
            decorators: Vec::new(),
            name: Ident::new("zd_enc_Point"),
            params: vec![
                Param {
                    name: Ident::new("w"),
                    type_: Some(Type::Name(Path::new(&["zerodds_wire", "Writer"]))),
                    default: None,
                },
                Param {
                    name: Ident::new("v"),
                    type_: Some(Type::Name(Path::new(&["a", "Point"]))),
                    default: None,
                },
            ],
            returns: Some(Type::name("None")),
            body: vec![Statement::Expr(Expr::Call {
                func: Path::new(&["w", "write_i32"]),
                args: vec![Arg::Positional(Expr::Attribute {
                    value: Box::new(Expr::name("v")),
                    name: Ident::new("x"),
                })],
            })],
        }));
        assert_eq!(
            text,
            "def zd_enc_Point(w: zerodds_wire.Writer, v: a.Point) -> None:\n    \
             \"\"\"Write one sample.\"\"\"\n    w.write_i32(v.x)\n"
        );
    }

    /// PEP 8, "Function Annotations": an annotated parameter's default
    /// takes spaces around the `=` and an unannotated one does not.
    #[test]
    fn a_default_is_spaced_only_where_the_parameter_is_annotated() {
        assert_eq!(
            param_text(&Param {
                name: Ident::new("order"),
                type_: Some(Type::name("str")),
                default: Some(Expr::Literal(Literal::Str("stream".to_string()))),
            }),
            "order: str = \"stream\""
        );
        assert_eq!(
            param_text(&Param {
                name: Ident::new("order"),
                type_: None,
                default: Some(Expr::int(2)),
            }),
            "order=2"
        );
    }

    /// A store writes into a place; a binding introduces a name. Both
    /// render `x = y` and the tree keeps them apart.
    #[test]
    fn a_store_reaches_a_place_a_binding_could_not_name() {
        assert_eq!(
            statement(&Statement::Set {
                place: Expr::Index {
                    value: Box::new(Expr::name("v")),
                    index: Box::new(Expr::name("i")),
                },
                value: Expr::int(0),
            }),
            "v[i] = 0\n"
        );
        assert_eq!(
            statement(&Statement::Set {
                place: Expr::Attribute {
                    value: Box::new(Expr::name("v")),
                    name: Ident::new("field"),
                },
                value: Expr::Literal(Literal::Null),
            }),
            "v.field = None\n"
        );
    }

    /// An attribute of a computed value, which `Expr::Path` cannot hold
    /// because its object is not a name.
    #[test]
    fn an_attribute_of_a_subscript_is_not_a_dotted_name() {
        assert_eq!(
            expr(&Expr::Attribute {
                value: Box::new(Expr::Index {
                    value: Box::new(Expr::name("v")),
                    index: Box::new(Expr::int(0)),
                }),
                name: Ident::new("key"),
            }),
            "v[0].key"
        );
    }

    /// The operator carries its own trailing space: `notx` is an
    /// identifier and `~x` takes none.
    #[test]
    fn a_unary_operator_carries_the_space_python_needs_and_no_other() {
        assert_eq!(
            expr(&Expr::Unary {
                op: "not ",
                value: Box::new(Expr::name("ok")),
            }),
            "not ok"
        );
        assert_eq!(
            expr(&Expr::Unary {
                op: "~",
                value: Box::new(Expr::name("bits")),
            }),
            "~bits"
        );
        assert_eq!(expr(&Expr::Paren(Box::new(Expr::name("a")))), "(a)");
    }

    /// §7.3.1.2.1.12.3's TRIM keeps a prefix, so only the far end is
    /// ever written and the near one renders as nothing.
    #[test]
    fn a_slice_writes_nothing_for_an_absent_end() {
        assert_eq!(
            expr(&Expr::Index {
                value: Box::new(Expr::name("v")),
                index: Box::new(Expr::Slice {
                    start: None,
                    end: Some(Box::new(Expr::int(5))),
                }),
            }),
            "v[:5]"
        );
        assert_eq!(
            expr(&Expr::Slice {
                start: None,
                end: None
            }),
            ":"
        );
    }

    /// A list is written into and a tuple is not, which is the whole
    /// reason the reader's collection slots are one and not the other.
    #[test]
    fn a_list_is_spelled_apart_from_a_tuple() {
        assert_eq!(
            expr(&Expr::List(vec![Expr::Literal(Literal::Null)])),
            "[None]"
        );
        assert_eq!(expr(&Expr::List(Vec::new())), "[]");
        assert_eq!(expr(&Expr::Tuple(vec![Expr::int(1)])), "(1,)");
    }

    #[test]
    fn a_blank_line_never_carries_indentation() {
        // Trailing whitespace on a blank line inside a class body is
        // what a formatter strips and what a byte comparison then
        // reports as a difference.
        let text = statement(&Statement::Class(Class {
            doc: Doc::line("Two fields."),
            decorators: Vec::new(),
            name: Ident::new("P"),
            bases: Vec::new(),
            body: vec![field("a", Type::name("int")), field("b", Type::name("int"))],
        }));
        for line in text.lines() {
            assert_eq!(line, line.trim_end(), "trailing whitespace: {line:?}");
        }
    }
}
