// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the Kotlin CST. Spec §5.
//!
//! Four spaces, the opening brace on the same line, `//` and `/** */`
//! comments — `ktfmt`'s and `ktlint`'s shared conventions, and close
//! enough to [`crate::java`]'s that the two read the same way beside
//! each other. Nothing is decided here that
//! [`zerodds_lang_cst::kotlin`] or `zerodds_lang_rules::adapter::kotlin`
//! did not decide first.
//!
//! ## Where this differs from [`crate::java`]
//!
//! - **No wrapper class.** [`file()`] writes top-level declarations
//!   directly, because a `.kt` file is not limited to one public type
//!   the way a `.java` file is (Kotlin language specification, "Kotlin
//!   files").
//! - **The specialised array factories.** `array_of` picks
//!   `intArrayOf`/`byteArrayOf`/… for a primitive element and
//!   `arrayOf<T>` otherwise, and `array_type` does the matching
//!   choice for the *type*: `IntArray` against `Array<Pose>`. Getting
//!   the two out of step is a type Kotlin refuses to unify —
//!   `arrayOf(1, 2)` against a declared `IntArray` is
//!   "type mismatch: inferred type is Array\<Int\> but IntArray was
//!   expected" — so both read the element type through one function,
//!   `is_specialised`.
//! - **No `new`.** Kotlin object creation is the type name applied to
//!   arguments (Kotlin language specification, "Object creation
//!   expressions"), so `expr_text`'s [`zerodds_lang_cst::kotlin::Expr::New`]
//!   arm writes none.

use crate::writer::{Writer, comment_lines, verbatim_lines};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::kotlin::{
    AliasDecl, Class, ConstDecl, Decl, Expr, File, FuncDecl, Object, Param, Property, Stmt, Type,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    if !file.doc.is_empty() {
        comment_lines(&mut w, "// ", file.doc.as_lines());
        w.blank_line();
    }
    if let Some(package) = &file.package {
        w.line(&alloc::format!("package {}", path_text(package)));
        w.blank_line();
    }
    if !file.imports.is_empty() {
        for import in &file.imports {
            w.line(&alloc::format!("import {}", path_text(import)));
        }
        w.blank_line();
    }
    for decl in &file.decls {
        render_decl(&mut w, decl);
        w.blank_line();
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

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Class(class) => render_class(w, class, true),
        Decl::Interface(class) => render_class(w, class, false),
        Decl::Object(object) => render_object(w, object),
        Decl::Alias(alias) => render_alias(w, alias),
        Decl::Const(const_) => render_const(w, const_),
        Decl::Func(func) => render_func(w, func),
        Decl::Comment(lines) => comment_lines(w, "// ", lines.as_lines()),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

/// `data class Name(var a: T = x, ...)`, or `class Name(val a: T, ...)`
/// where `as_data` is false.
fn render_class(w: &mut Writer, node: &Class, as_data: bool) {
    doc(w, &node.doc);
    let mut head = String::new();
    if as_data && node.is_data {
        head.push_str("data ");
    }
    head.push_str("class ");
    head.push_str(node.name.as_str());
    head.push('(');
    w.line(&head);
    w.indent();
    let last = node.properties.len().saturating_sub(1);
    for (index, property) in node.properties.iter().enumerate() {
        render_property(w, property, index == last);
    }
    w.dedent();
    if node.body_text.begin.is_empty() && node.body_text.end.is_empty() {
        w.line(")");
        return;
    }
    w.line(") {");
    w.indent();
    for text in node.body_text.begin.iter().chain(&node.body_text.end) {
        verbatim_lines(w, text);
    }
    w.dedent();
    w.line("}");
}

fn render_property(w: &mut Writer, node: &Property, last: bool) {
    doc(w, &node.doc);
    let mut line = String::from(if node.mutable { "var " } else { "val " });
    line.push_str(node.name.as_str());
    line.push_str(": ");
    line.push_str(&type_text(&node.type_));
    if let Some(default) = &node.default {
        line.push_str(" = ");
        line.push_str(&expr_text(default));
    }
    if !last {
        line.push(',');
    }
    w.line(&line);
}

/// `object Name { const val A: T = x }`.
fn render_object(w: &mut Writer, node: &Object) {
    doc(w, &node.doc);
    let mut head = String::from("object ");
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
        render_const(w, member);
    }
    w.dedent();
    w.line("}");
}

/// `typealias Name = T`.
fn render_alias(w: &mut Writer, node: &AliasDecl) {
    doc(w, &node.doc);
    let mut line = String::from("typealias ");
    line.push_str(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&type_text(&node.target));
    w.line(&line);
}

/// `const val Name: T = value`, or `val` where `is_const` is false.
fn render_const(w: &mut Writer, node: &ConstDecl) {
    doc(w, &node.doc);
    let mut line = String::from(if node.is_const { "const val " } else { "val " });
    line.push_str(node.name.as_str());
    line.push_str(": ");
    line.push_str(&type_text(&node.type_));
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    w.line(&line);
}

/// `fun name(a: T, b: U): R { body }`.
fn render_func(w: &mut Writer, node: &FuncDecl) {
    doc(w, &node.doc);
    let mut head = String::from("fun ");
    head.push_str(node.name.as_str());
    head.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    head.push_str(&params.join(", "));
    head.push_str("): ");
    head.push_str(&type_text(&node.returns));
    head.push_str(" {");
    w.line(&head);
    w.indent();
    for statement in &node.body {
        render_stmt(w, statement);
    }
    w.dedent();
    w.line("}");
}

fn param_text(param: &Param) -> String {
    let mut out = String::from(param.name.as_str());
    out.push_str(": ");
    out.push_str(&type_text(&param.type_));
    out
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::Declare {
            mutable,
            type_,
            name,
            value,
        } => {
            let mut line = String::from(if *mutable { "var " } else { "val " });
            line.push_str(name.as_str());
            line.push_str(": ");
            line.push_str(&type_text(type_));
            if let Some(value) = value {
                line.push_str(" = ");
                line.push_str(&expr_text(value));
            }
            w.line(&line);
        }
        Stmt::Expr(value) => w.line(&expr_text(value)),
        Stmt::Return(value) => match value {
            Some(value) => w.line(&alloc::format!("return {}", expr_text(value))),
            None => w.line("return"),
        },
        Stmt::If { .. } => render_if(w, node, "if ("),
        Stmt::CountUp {
            counter,
            bound,
            body,
        } => {
            let mut head = String::from("for (");
            head.push_str(counter.as_str());
            head.push_str(" in 0 until ");
            head.push_str(&expr_text(bound));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            for statement in &body.statements {
                render_stmt(w, statement);
            }
            w.dedent();
            w.line("}");
        }
        Stmt::While { condition, body } => {
            let mut head = String::from("while (");
            head.push_str(&expr_text(condition));
            head.push_str(") {");
            w.line(&head);
            w.indent();
            for statement in &body.statements {
                render_stmt(w, statement);
            }
            w.dedent();
            w.line("}");
        }
        Stmt::Break => w.line("break"),
        Stmt::Throw(value) => w.line(&alloc::format!("throw {}", expr_text(value))),
        Stmt::Comment(lines) => comment_lines(w, "// ", lines.as_lines()),
    }
}

/// An `if`, with an `else`-block of exactly one `if` collapsed into
/// `else if` — as in the Zig and Java columns, the shape a long dispatch
/// chain stays readable in.
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
fn render_if(w: &mut Writer, node: &Stmt, keyword: &str) {
    let mut node = node;
    let mut keyword = keyword;
    loop {
        let Stmt::If {
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
        for statement in &then.statements {
            render_stmt(w, statement);
        }
        w.dedent();
        match otherwise {
            None => {
                w.line("}");
                return;
            }
            Some(chain)
                if chain.statements.len() == 1
                    && matches!(chain.statements[0], Stmt::If { .. }) =>
            {
                node = &chain.statements[0];
                keyword = "} else if (";
            }
            Some(block) => {
                w.line("} else {");
                w.indent();
                for statement in &block.statements {
                    render_stmt(w, statement);
                }
                w.dedent();
                w.line("}");
                return;
            }
        }
    }
}

/// Whether `element` is one of the eight types the JVM has a
/// specialised, unboxed array for — `BooleanArray`, `ByteArray`,
/// `ShortArray`, `IntArray`, `LongArray`, `FloatArray`, `DoubleArray`,
/// `CharArray` (Kotlin language specification, "Array types"). Anything
/// else, including a nullable primitive, is a boxed `Array<T>`: a `null`
/// has no representation in an unboxed array.
fn is_specialised(element: &Type) -> bool {
    matches!(element, Type::Primitive(_))
}

/// The value each slot of a new `Array<T>` starts with, before the
/// codec fills it. `T()` is right for a generated class, whose every
/// property has a default, and wrong for three other element kinds:
/// a nested array (`IntArray()` and `Array<T>()` take arguments), a
/// `Float128` (its constructor is private; `Float128.zero()` is the
/// runtime's) and a nullable (`null`).
fn element_init(element: &Type) -> String {
    match element {
        Type::Array(inner) => match inner.as_ref() {
            Type::Primitive(primitive) => alloc::format!("{}Array(0)", primitive.as_str()),
            other => alloc::format!("emptyArray<{}>()", type_text(other)),
        },
        Type::Nullable(_) => "null".to_string(),
        _ if type_text(element) == "Float128" => "Float128.zero()".to_string(),
        _ => alloc::format!("{}()", type_text(element)),
    }
}

/// `IntArray`, or `Array<T>` where `T` is not one of the eight.
fn array_type(element: &Type) -> String {
    if let Type::Primitive(primitive) = element {
        return alloc::format!("{}Array", primitive.as_str());
    }
    alloc::format!("Array<{}>", type_text(element))
}

/// `intArrayOf(a, b)`, or `arrayOf<T>(a, b)`.
fn array_of(element: &Type, elements: &[Expr]) -> String {
    let parts: Vec<String> = elements.iter().map(expr_text).collect();
    if let Type::Primitive(primitive) = element {
        let mut factory = primitive.as_str().to_string();
        // `Int` -> `intArrayOf`, `Boolean` -> `booleanArrayOf`: the
        // factory's own first letter is lower-cased and the rest is the
        // primitive's name unchanged.
        let mut chars = factory.drain(..);
        let mut out = String::new();
        if let Some(first) = chars.next() {
            out.extend(first.to_lowercase());
        }
        out.push_str(chars.as_str());
        out.push_str("ArrayOf(");
        out.push_str(&parts.join(", "));
        out.push(')');
        return out;
    }
    let mut out = String::from("arrayOf<");
    out.push_str(&type_text(element));
    out.push_str(">(");
    out.push_str(&parts.join(", "));
    out.push(')');
    out
}

/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/adapter/kotlin.rs` refuses a type that nests deeper)
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
        Type::Array(element) => array_type(element),
        Type::Nullable(inner) => {
            let mut out = type_text(inner);
            out.push('?');
            out
        }
        Type::Function { params, returns } => {
            let mut out = String::from("(");
            let parts: Vec<String> = params.iter().map(type_text).collect();
            out.push_str(&parts.join(", "));
            out.push_str(") -> ");
            out.push_str(&type_text(returns));
            out
        }
        Type::Unit => "Unit".to_string(),
    }
}

fn path_text(path: &Path) -> String {
    let parts: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
    parts.join(".")
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_kotlin.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Path(path) => path_text(path),
        Expr::Call {
            receiver,
            name,
            args,
        } => {
            let mut out = match receiver {
                Some(receiver) => {
                    let mut out = expr_text(receiver);
                    out.push('.');
                    out
                }
                None => String::new(),
            };
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
            let mut out = type_text(type_);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::NewArray { element, length } => {
            if is_specialised(element) {
                alloc::format!("{}({})", array_type(element), expr_text(length))
            } else {
                alloc::format!(
                    "Array({}) {{ {} }}",
                    expr_text(length),
                    element_init(element)
                )
            }
        }
        Expr::ArrayOf { element, elements } => array_of(element, elements),
        Expr::Binary { op, lhs, rhs } => {
            let mut out = expr_text(lhs);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(&expr_text(rhs));
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::As { value, type_ } => {
            let mut out = expr_text(value);
            out.push_str(" as ");
            out.push_str(&type_text(type_));
            out
        }
        Expr::If {
            condition,
            then,
            otherwise,
        } => alloc::format!(
            "if ({}) {} else {}",
            expr_text(condition),
            expr_text(then),
            expr_text(otherwise)
        ),
        Expr::Not(value) => {
            let mut out = String::from("!");
            out.push_str(&expr_text(value));
            out
        }
        Expr::NotNull(value) => {
            let mut out = expr_text(value);
            out.push_str("!!");
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
        // Kotlin has no byte-string literal; the caller writes a
        // `byteArrayOf(0x1, 0x2, ..)` call instead of reaching this arm.
        Literal::Bytes(bytes) => {
            let mut out = String::from("byteArrayOf(");
            let parts: Vec<String> = bytes
                .iter()
                .map(|b| alloc::format!("{}", i16::from(*b as i8)))
                .collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Literal::Null => "null".to_string(),
    }
}

/// Escape a string for a Kotlin string literal.
///
/// Kotlin's escapes are `\t`, `\b`, `\n`, `\r`, `\'`, `\"`, `\\`, `\$`
/// and `\uXXXX` (Kotlin language specification, "String literals");
/// there is no `\0`, as in Zig, so a NUL goes out `\\u0000`. `$` is
/// escaped everywhere, not only before `{` or a letter: Kotlin's string
/// template only ever *reads* `$name` or `${expr}`, but escaping the
/// character unconditionally means this function never has to look
/// ahead at what follows a `$` in the source text to decide.
#[must_use]
pub fn string_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        push_escaped(&mut out, ch, '"');
    }
    out
}

/// A 64-bit pattern as the source text of a Kotlin `Long`.
///
/// Every value but one is its own decimal with the `L` suffix. Kotlin
/// has no negative integer literal at all — the minus is a prefix
/// operator applied to the literal that follows (Kotlin language
/// specification, "Integer literals") — so `-9223372036854775808L`
/// asks the compiler for a literal one past `Long.MAX_VALUE` and is
/// "value out of range", where the same characters are how Java spells
/// its own minimum. `Long.MIN_VALUE` is the value itself.
#[must_use]
pub fn long_text(pattern: i64) -> String {
    if pattern == i64::MIN {
        return "Long.MIN_VALUE".to_string();
    }
    format!("{pattern}L")
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
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{8}' => out.push_str("\\b"),
        '$' => out.push_str("\\$"),
        c if c == delimiter => {
            out.push('\\');
            out.push(c);
        }
        c if (c as u32) < 0x20 || c as u32 == 0x7F => {
            out.push_str(&alloc::format!("\\u{:04x}", c as u32));
        }
        other => out.push(other),
    }
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "// ", doc.as_lines());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use zerodds_lang_cst::common::Doc;
    use zerodds_lang_cst::kotlin::Primitive;

    fn data_class(properties: Vec<Property>) -> Decl {
        Decl::Class(Class {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::none(),
            is_data: true,
            name: Ident::new("S"),
            properties,
        })
    }

    fn property(name: &str, type_: Type, default: Expr) -> Property {
        Property {
            doc: Doc::none(),
            mutable: true,
            name: Ident::new(name),
            type_,
            default: Some(default),
        }
    }

    #[test]
    fn a_data_class_indents_by_four_and_ends_every_property_but_the_last_in_a_comma() {
        let text = decl(&data_class(alloc::vec![
            property("x", Type::Primitive(Primitive::Int), Expr::int(0)),
            property("y", Type::Primitive(Primitive::Double), Expr::int(0)),
        ]));
        assert_eq!(
            text,
            "data class S(\n    var x: Int = 0,\n    var y: Double = 0\n)\n"
        );
    }

    #[test]
    fn an_int_array_is_the_specialised_form_and_a_named_type_is_boxed() {
        assert_eq!(
            type_(&Type::array(Type::Primitive(Primitive::Int))),
            "IntArray"
        );
        assert_eq!(type_(&Type::array(Type::named("Pose"))), "Array<Pose>");
        // A nullable primitive cannot live in the specialised form: no
        // `null` fits an unboxed `Int`.
        assert_eq!(
            type_(&Type::array(Type::nullable(Type::Primitive(
                Primitive::Int
            )))),
            "Array<Int?>"
        );
    }

    #[test]
    fn the_array_literal_factory_matches_the_array_type() {
        assert_eq!(
            expr_text(&Expr::ArrayOf {
                element: alloc::boxed::Box::new(Type::Primitive(Primitive::Int)),
                elements: alloc::vec![Expr::int(1), Expr::int(2)],
            }),
            "intArrayOf(1, 2)"
        );
        assert_eq!(
            expr_text(&Expr::ArrayOf {
                element: alloc::boxed::Box::new(Type::named("Pose")),
                elements: alloc::vec![Expr::name("a")],
            }),
            "arrayOf<Pose>(a)"
        );
    }

    #[test]
    fn a_constructor_call_writes_no_new() {
        assert_eq!(
            expr_text(&Expr::New {
                type_: alloc::boxed::Box::new(Type::named("Pose")),
                args: Vec::new(),
            }),
            "Pose()"
        );
    }

    #[test]
    fn a_nul_is_written_as_a_unicode_escape_because_kotlin_has_no_backslash_zero() {
        assert_eq!(string_text("a\0b"), "a\\u0000b");
        assert_eq!(string_text("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(string_text("price: $5"), "price: \\$5");
    }

    #[test]
    fn an_object_of_constants_holds_an_enumerations_values() {
        let object = Object {
            doc: Doc::none(),
            name: Ident::new("Mode"),
            members: alloc::vec![
                ConstDecl {
                    doc: Doc::none(),
                    is_const: true,
                    name: Ident::new("IDLE"),
                    type_: Type::Primitive(Primitive::Int),
                    value: Expr::int(0),
                },
                ConstDecl {
                    doc: Doc::none(),
                    is_const: true,
                    name: Ident::new("ACTIVE"),
                    type_: Type::Primitive(Primitive::Int),
                    value: Expr::int(4),
                },
            ],
        };
        assert_eq!(
            decl(&Decl::Object(object)),
            "object Mode {\n    const val IDLE: Int = 0\n    const val ACTIVE: Int = 4\n}\n"
        );
    }

    #[test]
    fn a_file_has_no_wrapper_class_around_its_declarations() {
        let text = file(&File {
            doc: Doc::line("Generated."),
            package: Some(Path::single("zerodds")),
            imports: Vec::new(),
            decls: alloc::vec![Decl::Alias(AliasDecl {
                doc: Doc::none(),
                name: Ident::new("Distance"),
                target: Type::Primitive(Primitive::Int),
            })],
        });
        assert!(text.contains("package zerodds\n"), "{text}");
        assert!(text.contains("typealias Distance = Int\n"), "{text}");
        assert!(!text.contains("class "), "{text}");
    }
}
