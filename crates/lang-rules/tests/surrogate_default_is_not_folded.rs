// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A `@default` that is a lone surrogate reaches the generated code, in
//! every column that has a defaulter.
//!
//! # Why this is a test
//!
//! Because the loss had no reader. Rust's `char` is a Unicode scalar
//! value and rejects `0xD800..=0xDFFF`; a Java `char`, a C# `char` and a
//! TypeScript string are UTF-16 and hold a lone surrogate. Three
//! backends routed the declared code through `char::from_u32` and
//! answered the rejection with `'\0'` — a value the IDL never named and
//! no peer ever sent, emitted with no diagnostic and exit 0.
//!
//! Measured through the generators on `@default(0xD800) wchar c` before
//! the fix: the C column wrote `v->c = 55296;`, the Java column
//! `v.c = (char) 55296;`, and the C# column `v.C = '\0';`. The C#
//! backend carried a doc comment arguing that "the frontend has already
//! refused the declaration"; it had not — the generator ran with exit 0
//! — and nothing in the tree read the claim.
//!
//! # What this reads
//!
//! The statement tree, not the rendered text: `lang-render` publishes
//! `expr`, `decl` and `file`, and no per-statement entry point. The
//! distinction the fold turned on survives into the tree — a
//! `Literal::Int("55296")` against a `Literal::Char('\0')` — so the
//! tree is enough to tell the two apart. The rendered text was measured
//! through `wireprobe-zerodds-{c,java,csharp}-gen` by hand and is quoted
//! above.
//!
//! `0xD800` is the value here because it is the boundary the two
//! character types disagree on. `0x10000` is the other end: above the
//! Basic Multilingual Plane there is no single code unit at all, which
//! is a refusal and not a fold.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_ast::{Ast, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Ident, Path};

/// One column's answer, as text: the debug form of the statements or of
/// the refusal.
fn render<S: core::fmt::Debug, E: core::fmt::Debug>(
    answer: Result<Vec<S>, E>,
) -> Result<String, String> {
    match answer {
        Ok(statements) => Ok(format!("{statements:?}")),
        Err(error) => Err(format!("{error:?}")),
    }
}

/// The one IDL these cases are cut from.
fn source(default_value: &str) -> String {
    format!(
        "@extensibility(MUTABLE)\n\
         struct S {{\n\
         \x20 @id(1) @default({default_value}) wchar c;\n\
         \x20 @id(2) long n;\n\
         }};\n"
    )
}

fn model(idl: &str) -> (Ast, Id<TypeNode>) {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower, so the frontend refused the declaration \
         after all and this test measures the wrong thing: {:?}",
        lowered.diagnostics
    );
    let ast: Ast = lowered.ast;
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == "S")
        .map(|(id, _)| id)
        .expect("the root type");
    (ast, root)
}

/// The member named `c` and the type it is declared with.
fn member_c(ast: &Ast, root: Id<TypeNode>) -> (MemberNode, Id<TypeNode>) {
    let node = ast.type_(root).expect("the root");
    let TypeKind::Structure { members, .. } = &node.kind else {
        panic!("the root is a structure");
    };
    for member in members {
        let member_node = ast.member(*member).expect("a member");
        if member_node.name.as_str() == "c" {
            let type_ = member_node
                .type_
                .resolved()
                .expect("the member's type resolves");
            return (member_node.clone(), type_);
        }
    }
    panic!("the structure declares `c`");
}

/// What each column makes of `@default(<value>) wchar c`, as the debug
/// form of the statements its defaulter emits.
fn emitted_for(default_value: &str) -> Vec<(&'static str, Result<String, String>)> {
    let idl = source(default_value);
    let (ast, root) = model(&idl);
    let (member, type_) = member_c(&ast, root);

    vec![
        (
            "c",
            render(zerodds_lang_rules::default_c::member_default(
                &ast,
                &member,
                type_,
                zerodds_lang_cst::c::Expr::Name(Ident::new("v")),
            )),
        ),
        (
            "java",
            render(zerodds_lang_rules::default_java::member_default(
                &ast,
                &member,
                type_,
                zerodds_lang_cst::java::Expr::Path(Path::new(&["v"])),
            )),
        ),
        (
            "csharp",
            render(zerodds_lang_rules::default_csharp::member_default(
                &ast,
                &member,
                type_,
                zerodds_lang_cst::csharp::Expr::Path(Path::new(&["v"])),
            )),
        ),
        (
            "typescript",
            render(zerodds_lang_rules::default_typescript::member_default(
                &ast,
                &member,
                type_,
                zerodds_lang_cst::typescript::Expr::Path(Path::new(&["v"])),
            )),
        ),
        (
            "python",
            render(zerodds_lang_rules::default_python::member_default(
                &ast,
                &member,
                type_,
                zerodds_lang_cst::python::Expr::Path(Path::new(&["v"])),
            )),
        ),
    ]
}

#[test]
fn a_lone_surrogate_default_reaches_every_column() {
    for (column, answer) in emitted_for("0xD800") {
        let emitted = answer.unwrap_or_else(|error| {
            panic!(
                "the {column} column refused `@default(0xD800)` with {error}; \
                 U+D800 is one UTF-16 code unit and every column here has a \
                 type that holds it"
            )
        });
        assert!(
            emitted.contains("55296") || emitted.contains("0xD800"),
            "the {column} column emitted {emitted} for `@default(0xD800)`, \
             which names neither 55296 nor 0xD800 — the declared value was \
             replaced by another with no diagnostic"
        );
    }
}

/// The specific substitution that happened, named so a regression reads
/// as itself rather than as "the number is missing".
#[test]
fn no_column_answers_a_surrogate_with_nul() {
    for (column, answer) in emitted_for("0xD800") {
        let Ok(emitted) = answer else { continue };
        assert!(
            !emitted.contains("Char('\\0')"),
            "the {column} column emitted {emitted}: a NUL where the IDL \
             declared U+D800. Rust's `char` cannot hold a surrogate and this \
             column's character type can, so the fold happened in the backend"
        );
    }
}

/// Above the Basic Multilingual Plane there is no single code unit, and
/// that is a refusal rather than a fold.
///
/// Only the three columns whose character type **is** one code unit are
/// asserted. C stores a `uint16_t` and Java's own answer is its to make;
/// this test is about the fold, not about who refuses.
#[test]
fn a_default_above_the_bmp_is_refused_rather_than_folded() {
    for (column, answer) in emitted_for("0x10000") {
        if !matches!(column, "csharp" | "typescript" | "python") {
            continue;
        }
        assert!(
            answer.is_err(),
            "the {column} column took `@default(0x10000)` on a `wchar` and \
             emitted {answer:?}; one UTF-16 code unit cannot carry it, so the \
             only honest answers are a refusal or a surrogate pair — never a \
             substituted character"
        );
    }
}
