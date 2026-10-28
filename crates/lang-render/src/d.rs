// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Rendering the D CST. Spec §5.
//!
//! One style, the one `dfmt` and Phobos use: four spaces, Allman braces
//! — the opening brace on a line of its own — and `///` for
//! documentation, which is what Ddoc reads.
//!
//! Nothing is decided here, with one exception that is a rendering fact
//! rather than a choice: **a D type suffix is written after the type it
//! applies to and reads right to left**. `int[3][2]` is two arrays of
//! three (D language specification, "Arrays", "Rectangular Arrays"),
//! which is the reverse of C. [`zerodds_lang_cst::d::Type::Array`] holds
//! the outermost dimension outermost, exactly as the Go tree does, and
//! this renderer writes the element before the length — so one tree
//! shape produces `[2][3]int32` in Go and `int[3][2]` in D and both mean
//! two rows of three.
//!
//! Unlike [`crate::c`] there is no declarator to split around
//! a name: a D declaration is the whole type, a space, and the name.

use crate::writer::{Writer, comment_lines, escape_string, verbatim_lines};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::d::{
    AliasDecl, ConstDecl, Decl, EnumDecl, EnumMember, Expr, Field, File, FuncDecl, Import,
    InterfaceDecl, Member, Method, Param, Stmt, StructDecl, Type, Uda,
};

/// Render a whole file.
#[must_use]
pub fn file(file: &File) -> String {
    let mut w = Writer::spaces4();
    doc(&mut w, &file.doc);
    let mut head = String::from("module ");
    head.push_str(&path_text(&file.module));
    head.push(';');
    w.line(&head);
    if !file.imports.is_empty() {
        w.blank_line();
        for import in &file.imports {
            w.line(&import_text(import));
        }
    }
    for decl in &file.decls {
        w.blank_line();
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

/// The identifier a name renders as, for a caller assembling one.
#[must_use]
pub fn ident(name: &Ident) -> String {
    name.as_str().to_string()
}

/// A dotted module path: `std.typecons`.
fn path_text(path: &Path) -> String {
    path.segments()
        .iter()
        .map(|segment| segment.as_str())
        .collect::<Vec<&str>>()
        .join(".")
}

fn import_text(import: &Import) -> String {
    let mut out = String::from("import ");
    out.push_str(&path_text(&import.path));
    if !import.symbols.is_empty() {
        out.push_str(" : ");
        out.push_str(
            &import
                .symbols
                .iter()
                .map(Ident::as_str)
                .collect::<Vec<&str>>()
                .join(", "),
        );
    }
    out.push(';');
    out
}

/// `ReturnType name(params)` with an Allman body.
fn render_func(w: &mut Writer, node: &FuncDecl) {
    doc(w, &node.doc);
    let mut head = String::new();
    match &node.returns {
        Some(returns) => head.push_str(&type_text(returns)),
        None => head.push_str("void"),
    }
    head.push(' ');
    head.push_str(node.name.as_str());
    head.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    head.push_str(&params.join(", "));
    head.push(')');
    w.line(&head);
    w.line("{");
    w.indent();
    render_stmts(w, &node.body);
    w.dedent();
    w.line("}");
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stmts(w: &mut Writer, body: &[Stmt]) {
    for statement in body {
        render_stmt(w, statement);
    }
}

/// zerodds-lint: recursion-depth 64 (one level per nested block of a
/// generated body. The codec walk emits a routine per type rather than
/// inlining one — `lang-rules/src/emit/encode.rs` — so a body nests by
/// its own framing and by one loop per array dimension, never by the
/// type graph, whose descent stops at 64 anyway; the else-if chain, one
/// arm per union case, is walked iteratively and costs no depth.)
fn render_stmt(w: &mut Writer, node: &Stmt) {
    match node {
        Stmt::Var { name, type_, value } => {
            let mut line;
            match (type_, value) {
                // `auto name = value;`: the type is the initialiser's.
                (None, Some(value)) => {
                    line = String::from("auto ");
                    line.push_str(name.as_str());
                    line.push_str(" = ");
                    line.push_str(&expr_text(value));
                }
                (Some(type_), value) => {
                    line = type_text(type_);
                    line.push(' ');
                    line.push_str(name.as_str());
                    if let Some(value) = value {
                        line.push_str(" = ");
                        line.push_str(&expr_text(value));
                    }
                }
                // Neither a type nor a value is not a declaration;
                // written out so the defect names itself in the diff.
                (None, None) => {
                    line = String::from("auto ");
                    line.push_str(name.as_str());
                }
            }
            line.push(';');
            w.line(&line);
        }
        Stmt::Assign { place, value } => {
            let mut line = expr_text(place);
            line.push_str(" = ");
            line.push_str(&expr_text(value));
            line.push(';');
            w.line(&line);
        }
        Stmt::Expr(value) => {
            let mut line = expr_text(value);
            line.push(';');
            w.line(&line);
        }
        Stmt::Return(value) => match value {
            Some(value) => {
                let mut line = String::from("return ");
                line.push_str(&expr_text(value));
                line.push(';');
                w.line(&line);
            }
            None => w.line("return;"),
        },
        Stmt::If {
            condition,
            then,
            otherwise,
        } => render_if(w, condition, then, otherwise.as_deref(), "if ("),
        Stmt::For {
            counter,
            type_,
            bound,
            body,
        } => {
            let mut line = String::from("for (");
            line.push_str(&type_text(type_));
            line.push(' ');
            line.push_str(counter.as_str());
            line.push_str(" = 0; ");
            line.push_str(counter.as_str());
            line.push_str(" < ");
            line.push_str(&expr_text(bound));
            line.push_str("; ");
            line.push_str(counter.as_str());
            line.push_str("++)");
            w.line(&line);
            w.line("{");
            w.indent();
            render_stmts(w, body);
            w.dedent();
            w.line("}");
        }
        Stmt::While { condition, body } => {
            let mut line = String::from("while (");
            line.push_str(&expr_text(condition));
            line.push(')');
            w.line(&line);
            w.line("{");
            w.indent();
            render_stmts(w, body);
            w.dedent();
            w.line("}");
        }
        Stmt::Break => w.line("break;"),
        Stmt::Comment(lines) => doc(w, lines),
    }
}

/// An `if` chain, with `else if` continuing the line the way dfmt
/// writes it: an else-branch holding exactly one `if` and nothing else
/// does not nest.
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
fn render_if(
    w: &mut Writer,
    condition: &Expr,
    then: &[Stmt],
    otherwise: Option<&[Stmt]>,
    head: &str,
) {
    let mut condition = condition;
    let mut then = then;
    let mut otherwise = otherwise;
    let mut head = head;
    loop {
        let mut line = String::from(head);
        line.push_str(&expr_text(condition));
        line.push(')');
        w.line(&line);
        w.line("{");
        w.indent();
        render_stmts(w, then);
        w.dedent();
        w.line("}");
        match otherwise {
            None => return,
            Some(
                [
                    Stmt::If {
                        condition: next_condition,
                        then: next_then,
                        otherwise: next_otherwise,
                    },
                ],
            ) => {
                condition = next_condition;
                then = next_then;
                otherwise = next_otherwise.as_deref();
                head = "else if (";
            }
            Some(branch) => {
                w.line("else");
                w.line("{");
                w.indent();
                render_stmts(w, branch);
                w.dedent();
                w.line("}");
                return;
            }
        }
    }
}

fn render_decl(w: &mut Writer, node: &Decl) {
    match node {
        Decl::Struct(decl) => render_struct(w, decl),
        Decl::Enum(decl) => render_enum(w, decl),
        Decl::Alias(decl) => render_alias(w, decl),
        Decl::Const(decl) => render_const(w, decl),
        Decl::Interface(decl) => render_interface(w, decl),
        Decl::Func(decl) => render_func(w, decl),
        Decl::Comment(lines) => doc(w, lines),
        Decl::Verbatim(text) => verbatim_lines(w, text),
    }
}

fn render_struct(w: &mut Writer, node: &StructDecl) {
    doc(w, &node.doc);
    let mut head = String::from("struct ");
    head.push_str(node.name.as_str());
    w.line(&head);
    w.line("{");
    w.indent();
    for member in &node.members {
        render_member(w, member);
    }
    w.dedent();
    w.line("}");
}

fn render_member(w: &mut Writer, node: &Member) {
    match node {
        Member::Field(field) => render_field(w, field),
        Member::Verbatim(text) => verbatim_lines(w, text),
        Member::Union(fields) => {
            // An empty `union { }` is never written: it says nothing and
            // is a body a reader has to check the grammar for.
            if fields.is_empty() {
                return;
            }
            w.line("union");
            w.line("{");
            w.indent();
            for field in fields {
                render_field(w, field);
            }
            w.dedent();
            w.line("}");
        }
    }
}

fn render_field(w: &mut Writer, node: &Field) {
    doc(w, &node.doc);
    let mut line = String::new();
    for uda in &node.udas {
        line.push_str(&uda_text(uda));
        line.push(' ');
    }
    line.push_str(&type_text(&node.type_));
    line.push(' ');
    line.push_str(node.name.as_str());
    line.push(';');
    w.line(&line);
}

/// A user-defined attribute: `@("idl:x")`.
///
/// A single string, parenthesised. `@(…)` is the general form
/// (D language specification, "Attributes", "User-Defined Attributes");
/// the bare `@name` form names a symbol, which would need a declaration
/// this file would then have to emit and every consumer import.
fn uda_text(uda: &Uda) -> String {
    let mut out = String::from("@(\"");
    out.push_str(&escape_string(&uda.text));
    out.push_str("\")");
    out
}

fn render_enum(w: &mut Writer, node: &EnumDecl) {
    doc(w, &node.doc);
    let mut head = String::from("enum ");
    head.push_str(node.name.as_str());
    if let Some(base) = &node.base {
        head.push_str(" : ");
        head.push_str(&type_text(base));
    }
    w.line(&head);
    w.line("{");
    w.indent();
    for (index, member) in node.members.iter().enumerate() {
        let last = index + 1 == node.members.len();
        render_enum_member(w, member, last);
    }
    w.dedent();
    w.line("}");
}

/// One enum member, with the comma every one but the last carries.
///
/// No trailing comma on the last: D accepts one, and leaving it off
/// keeps the output the same shape as the enum bodies in Phobos.
fn render_enum_member(w: &mut Writer, node: &EnumMember, last: bool) {
    doc(w, &node.doc);
    let mut line = String::from(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    if !last {
        line.push(',');
    }
    w.line(&line);
}

fn render_alias(w: &mut Writer, node: &AliasDecl) {
    doc(w, &node.doc);
    // `alias Name = T;` and not the old `alias T Name;`. The second form
    // is deprecated and reads in the opposite order, which is what made
    // it deprecated.
    let mut line = String::from("alias ");
    line.push_str(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&type_text(&node.type_));
    line.push(';');
    w.line(&line);
}

/// A manifest constant: `enum T NAME = value;`.
///
/// `enum` and not `immutable`: a manifest constant has no storage and no
/// address (D language specification, "Enums", "Manifest Constants"),
/// where an `immutable` declaration at module scope is a variable with
/// both. A constant that a consumer can take the address of is a symbol
/// the linker has to place, per generated file, in every binary that
/// imports it.
fn render_const(w: &mut Writer, node: &ConstDecl) {
    doc(w, &node.doc);
    let mut line = String::from("enum ");
    if let Some(type_) = &node.type_ {
        line.push_str(&type_text(type_));
        line.push(' ');
    }
    line.push_str(node.name.as_str());
    line.push_str(" = ");
    line.push_str(&expr_text(&node.value));
    line.push(';');
    w.line(&line);
}

fn render_interface(w: &mut Writer, node: &InterfaceDecl) {
    doc(w, &node.doc);
    let mut head = String::from("interface ");
    head.push_str(node.name.as_str());
    w.line(&head);
    w.line("{");
    w.indent();
    for method in &node.methods {
        render_method(w, method);
    }
    w.dedent();
    w.line("}");
}

fn render_method(w: &mut Writer, node: &Method) {
    doc(w, &node.doc);
    let mut line = match &node.returns {
        Some(type_) => type_text(type_),
        None => "void".to_string(),
    };
    line.push(' ');
    line.push_str(node.name.as_str());
    line.push('(');
    let params: Vec<String> = node.params.iter().map(param_text).collect();
    line.push_str(&params.join(", "));
    line.push_str(");");
    w.line(&line);
}

fn param_text(param: &Param) -> String {
    let mut out = type_text(&param.type_);
    out.push(' ');
    out.push_str(param.name.as_str());
    out
}

/// A type, with every suffix written after what it applies to.
///
/// The direction is the whole of what this function decides, and it is
/// D's: `int[3][2]` is two arrays of three. A tree holding the outermost
/// dimension outermost therefore renders its element first and appends
/// its own length last.
///
/// zerodds-lint: recursion-depth 64 (one level per collection, alias or
/// generic argument of the spelled type; `MAX_TYPE_DEPTH` in
/// `lang-rules/src/emit/d.rs` refuses a type that nests deeper)
fn type_text(node: &Type) -> String {
    match node {
        Type::Primitive(primitive) => primitive.as_str().to_string(),
        Type::Named(name) => name.as_str().to_string(),
        Type::Slice(element) => {
            let mut out = type_text(element);
            out.push_str("[]");
            out
        }
        Type::Array { length, element } => {
            let mut out = type_text(element);
            out.push('[');
            out.push_str(&expr_text(length));
            out.push(']');
            out
        }
        Type::Pointer(target) => {
            let mut out = type_text(target);
            out.push('*');
            out
        }
        // Always parenthesised: `Nullable!int[]` parses as
        // `(Nullable!int)[]`, because D's single-token template argument
        // binds to the token and not to the suffix after it.
        Type::Nullable(target) => {
            let mut out = String::from("Nullable!(");
            out.push_str(&type_text(target));
            out.push(')');
            out
        }
    }
}

/// zerodds-lint: recursion-depth = the built expression's nesting. A walk
/// calls a routine per type rather than nesting one, so the one operand
/// tree that grows with the IDL is an array's default value: one composite
/// literal per dimension, built in `lang-rules/src/default_d.rs`, and
/// nothing bounds the dimension count. No number is claimed: this
/// signature returns no error.
fn expr_text(node: &Expr) -> String {
    match node {
        Expr::Literal(literal) => literal_text(literal),
        Expr::Name(name) => name.as_str().to_string(),
        Expr::Binary { op, lhs, rhs } => {
            let mut out = expr_text(lhs);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(&expr_text(rhs));
            out
        }
        Expr::Cast { type_, value } => {
            let mut out = String::from("cast(");
            out.push_str(&type_text(type_));
            out.push(')');
            out.push_str(&expr_text(value));
            out
        }
        Expr::Paren(inner) => {
            let mut out = String::from("(");
            out.push_str(&expr_text(inner));
            out.push(')');
            out
        }
        Expr::Call { func, args } => {
            let mut out = expr_text(func);
            out.push('(');
            let parts: Vec<String> = args.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::Dot { value, name } => {
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
        Expr::SliceTo { value, high } => {
            let mut out = expr_text(value);
            out.push_str("[0 .. ");
            out.push_str(&expr_text(high));
            out.push(']');
            out
        }
        Expr::Unary { op, value } => {
            let mut out = String::from(*op);
            out.push_str(&expr_text(value));
            out
        }
        Expr::StructLit { name, fields } => {
            let mut out = String::from(name.as_str());
            out.push('(');
            let parts: Vec<String> = fields
                .iter()
                .map(|(key, value)| match key {
                    Some(key) => {
                        let mut part = String::from(key.as_str());
                        part.push_str(": ");
                        part.push_str(&expr_text(value));
                        part
                    }
                    None => expr_text(value),
                })
                .collect();
            out.push_str(&parts.join(", "));
            out.push(')');
            out
        }
        Expr::ArrayLit(elements) => {
            let mut out = String::from("[");
            let parts: Vec<String> = elements.iter().map(expr_text).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Expr::NewSlice { element, length } => {
            let mut out = String::from("new ");
            out.push_str(&type_text(element));
            out.push_str("[](");
            out.push_str(&expr_text(length));
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
        // The code point as a number and not `'c'`. A D character
        // literal has type `char`, `wchar` or `dchar` — UTF code units —
        // and an IDL `char` is one octet held here in a `ubyte`. Writing
        // the number puts the conversion at the declaration, where the
        // declared type states it, rather than inside a literal that
        // happens to fit.
        Literal::Char(ch) => alloc::format!("{}", u32::from(*ch)),
        Literal::Str(text) => {
            let mut out = String::from("\"");
            out.push_str(&escape_string(text));
            out.push('"');
            out
        }
        // A D array literal of unsigned octets. `cast(ubyte[])` is not
        // written: the declared type of what this initialises says
        // `ubyte[]`, and the literal's elements are all in range.
        Literal::Bytes(bytes) => {
            let mut out = String::from("[");
            let parts: Vec<String> = bytes.iter().map(|b| alloc::format!("0x{b:02X}")).collect();
            out.push_str(&parts.join(", "));
            out.push(']');
            out
        }
        Literal::Null => "null".to_string(),
    }
}

fn doc(w: &mut Writer, doc: &Doc) {
    if doc.is_empty() {
        return;
    }
    comment_lines(w, "/// ", doc.as_lines());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;
    use zerodds_lang_cst::d::Primitive;

    fn field(name: &str, type_: Type) -> Field {
        Field {
            doc: Doc::none(),
            udas: Vec::new(),
            name: Ident::new(name),
            type_,
        }
    }

    #[test]
    fn a_file_declares_its_module_first() {
        let rendered = file(&File::default());
        assert_eq!(rendered, "module zdtypes;\n");
    }

    #[test]
    fn an_import_is_selective_when_it_names_symbols() {
        let mut source = File::default();
        source
            .imports
            .push(Import::selective(&["std", "typecons"], "Nullable"));
        let rendered = file(&source);
        assert!(
            rendered.contains("import std.typecons : Nullable;"),
            "{rendered}"
        );
    }

    #[test]
    fn a_struct_writes_its_brace_on_a_line_of_its_own() {
        let rendered = decl(&Decl::Struct(StructDecl {
            doc: Doc::none(),
            name: Ident::new("Pose"),
            members: vec![Member::Field(field("x", Type::Primitive(Primitive::Int)))],
        }));
        assert_eq!(rendered, "struct Pose\n{\n    int x;\n}\n");
    }

    #[test]
    fn a_field_carries_its_uda_before_the_type() {
        let mut node = field("version_", Type::Primitive(Primitive::Int));
        node.udas.push(Uda::keyed("idl", "version"));
        let mut w = Writer::spaces4();
        render_field(&mut w, &node);
        assert_eq!(w.finish(), "@(\"idl:version\") int version_;\n");
    }

    #[test]
    fn an_array_writes_its_dimensions_right_to_left() {
        // `long grid[2][3]` is two rows of three; the D declaration is
        // `int[3][2]`. The tree holds 2 outermost and the renderer
        // appends it last, which is what produces the reversal.
        let type_ = Type::array(2, Type::array(3, Type::Primitive(Primitive::Int)));
        assert_eq!(type_text(&type_), "int[3][2]");
    }

    #[test]
    fn a_slice_of_a_static_array_keeps_the_two_apart() {
        // `sequence<long[3]>` is a dynamic array whose element is
        // `int[3]`, which D writes `int[3][]`.
        let type_ = Type::slice(Type::array(3, Type::Primitive(Primitive::Int)));
        assert_eq!(type_text(&type_), "int[3][]");
        // And the other way round: a static array of two slices.
        let other = Type::array(2, Type::slice(Type::Primitive(Primitive::Int)));
        assert_eq!(type_text(&other), "int[][2]");
    }

    #[test]
    fn a_nullable_parenthesises_its_argument() {
        // `Nullable!int[]` parses as `(Nullable!int)[]`. The
        // parenthesised form is the only one that cannot be read the
        // wrong way.
        let type_ = Type::nullable(Type::slice(Type::Primitive(Primitive::Int)));
        assert_eq!(type_text(&type_), "Nullable!(int[])");
    }

    #[test]
    fn a_pointer_writes_its_star_after_the_type() {
        assert_eq!(
            type_text(&Type::pointer(Type::Primitive(Primitive::Void))),
            "void*"
        );
    }

    #[test]
    fn an_enum_states_its_base_and_every_value() {
        let rendered = decl(&Decl::Enum(EnumDecl {
            doc: Doc::none(),
            name: Ident::new("Mode"),
            base: Some(Type::Primitive(Primitive::Int)),
            members: vec![
                EnumMember {
                    doc: Doc::none(),
                    name: Ident::new("IDLE"),
                    value: Expr::int(0),
                },
                EnumMember {
                    doc: Doc::none(),
                    name: Ident::new("ACTIVE"),
                    value: Expr::int(4),
                },
            ],
        }));
        assert_eq!(
            rendered,
            "enum Mode : int\n{\n    IDLE = 0,\n    ACTIVE = 4\n}\n"
        );
    }

    #[test]
    fn a_manifest_constant_is_an_enum_and_not_an_immutable() {
        let rendered = decl(&Decl::Const(ConstDecl {
            doc: Doc::none(),
            name: Ident::new("LIMIT"),
            type_: Some(Type::Primitive(Primitive::UByte)),
            value: Expr::int(8),
        }));
        assert_eq!(rendered, "enum ubyte LIMIT = 8;\n");
    }

    #[test]
    fn an_alias_is_written_in_the_new_order() {
        let rendered = decl(&Decl::Alias(AliasDecl {
            doc: Doc::none(),
            name: Ident::new("Distance"),
            type_: Type::Primitive(Primitive::Int),
        }));
        assert_eq!(rendered, "alias Distance = int;\n");
    }

    #[test]
    fn an_anonymous_union_nests_inside_the_struct() {
        let rendered = decl(&Decl::Struct(StructDecl {
            doc: Doc::none(),
            name: Ident::new("Payload"),
            members: vec![
                Member::Field(field("_d", Type::named("Mode"))),
                Member::Union(vec![
                    field("idle", Type::Primitive(Primitive::Int)),
                    field("active", Type::Primitive(Primitive::Double)),
                ]),
            ],
        }));
        assert_eq!(
            rendered,
            "struct Payload\n{\n    Mode _d;\n    union\n    {\n        int idle;\n        \
             double active;\n    }\n}\n"
        );
    }

    #[test]
    fn an_empty_union_member_writes_nothing() {
        // A `union { }` with no fields says nothing and is a body a
        // reader would have to check the grammar for.
        let rendered = decl(&Decl::Struct(StructDecl {
            doc: Doc::none(),
            name: Ident::new("Empty"),
            members: vec![Member::Union(Vec::new())],
        }));
        assert_eq!(rendered, "struct Empty\n{\n}\n");
    }

    #[test]
    fn an_interface_writes_a_signature_per_method() {
        let rendered = decl(&Decl::Interface(InterfaceDecl {
            doc: Doc::none(),
            name: Ident::new("Control"),
            methods: vec![
                Method {
                    doc: Doc::none(),
                    name: Ident::new("go"),
                    returns: None,
                    params: vec![Param {
                        name: Ident::new("target"),
                        type_: Type::named("Pose"),
                    }],
                },
                Method {
                    doc: Doc::none(),
                    name: Ident::new("status"),
                    returns: Some(Type::Primitive(Primitive::Int)),
                    params: Vec::new(),
                },
            ],
        }));
        assert_eq!(
            rendered,
            "interface Control\n{\n    void go(Pose target);\n    int status();\n}\n"
        );
    }

    #[test]
    fn a_documentation_comment_uses_the_ddoc_marker() {
        let rendered = decl(&Decl::Comment(Doc::lines(&["first", "", "third"])));
        assert_eq!(rendered, "/// first\n///\n/// third\n");
    }

    #[test]
    fn a_character_literal_is_written_as_its_code_point() {
        assert_eq!(literal_text(&Literal::Char('a')), "97");
    }

    #[test]
    fn a_string_literal_is_escaped_for_d() {
        assert_eq!(
            literal_text(&Literal::Str("a\"b\n".into())),
            "\"a\\\"b\\n\""
        );
    }

    #[test]
    fn the_null_literal_is_ds_own_spelling() {
        assert_eq!(literal_text(&Literal::Null), "null");
    }

    #[test]
    fn a_shift_expression_renders_with_its_suffix() {
        let expr = Expr::Binary {
            op: "<<",
            lhs: alloc::boxed::Box::new(Expr::suffixed_int(1, "UL")),
            rhs: alloc::boxed::Box::new(Expr::int(63)),
        };
        assert_eq!(expr_text(&expr), "1UL << 63");
    }

    #[test]
    fn a_cast_writes_its_target_in_parentheses() {
        let expr = Expr::Cast {
            type_: alloc::boxed::Box::new(Type::Primitive(Primitive::UByte)),
            value: alloc::boxed::Box::new(Expr::int(3)),
        };
        assert_eq!(expr_text(&expr), "cast(ubyte)3");
    }
}
