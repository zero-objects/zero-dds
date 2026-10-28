// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the TypeScript backend.
//!
//! What this proves is not that the octets are right — that is
//! `runtime/typescript`'s test against the ledger, and later a corpus
//! sweep — but that the walk reaches the end for the shapes the corpus
//! has and that what comes out is TypeScript rather than C with
//! different punctuation. Every defect the Java and C++ columns found
//! on their first day was of that second kind: a runtime name from
//! another language, a cast that is not a cast here, an operator that
//! groups differently.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_ast::Ast;
use zerodds_lang_rules::emit::typescript::TypeScript;
use zerodds_lang_rules::{ModelGraph, apply};

/// One IDL source, as the TypeScript codec for its root type.
fn encoder(idl: &str, type_name: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let ast: Ast = lowered.ast;
    let model = ModelGraph::of(&ast);
    let _applied =
        apply(&model, &zerodds_lang_rules::rule::typescript::catalogue()).expect("the rules apply");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    let decls = zerodds_lang_rules::emit::encode::codec_from::<TypeScript>(
        &ast,
        root,
        key,
        Default::default(),
    )
    .expect("the walk reaches the end");
    decls
        .into_iter()
        .map(|decl| zerodds_lang_render::typescript::decl(&decl))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_structure_of_primitives_becomes_a_typescript_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in ["function", "w.writeI32", "w.writeF64", "w.writeBool", "v.x"] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    // Nothing from another column's runtime.
    for foreign in ["zdw_", "->", "static void", "Writer.write"] {
        assert!(
            !text.contains(foreign),
            "{foreign} reached TypeScript:\n{text}"
        );
    }
}

#[test]
fn a_sequence_counts_and_loops() {
    let text = encoder("@final struct S { sequence<long> many; };", "S");
    assert!(
        text.contains(".length"),
        "the count is not the array's:\n{text}"
    );
    assert!(text.contains("for (let"), "no counted loop:\n{text}");
}

#[test]
fn a_wide_integer_is_a_bigint_literal() {
    let text = encoder("@final struct S { unsigned long long big; };", "S");
    assert!(text.contains("writeU64"), "no 64-bit write:\n{text}");
}
