// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A value the generator cannot work out is refused, not defaulted.
//!
//! `1/0` is the shortest expression the frontend accepts and the
//! evaluator rejects, so it is what these use. Nothing about the defect
//! was specific to division: any expression that does not evaluate took
//! the same path, and `@value(-1)` reaching the wire as `0` was the same
//! substitution one crate over.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{
    CollectionDelimiterPolicy, Encoding, Endianness, LayoutKey, LengthCodePolicy,
};
use zerodds_lang_rules::adapter::cst::rust_items;
use zerodds_lang_rules::encode::rust_codec;
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply};

/// The Rust the chain generates, or the refusal it produces.
fn generate(idl: &str) -> Result<String, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "unevaluated.idl");
    let ast: Ast = lowered.ast;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply");
    let items = rust_items(&ast, &applied).map_err(|error| error.to_string())?;
    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    Ok(zerodds_lang_render::rust::file(&file))
}

/// An array length that does not evaluate is not a zero-length array.
///
/// `[i32; 0]` compiles, holds nothing, and is indistinguishable in the
/// generated source from an array the author asked to be empty. The
/// frontend reports no diagnostic for this source, so the refusal here
/// is the only thing between it and a silently emptied field.
#[test]
fn an_array_length_that_does_not_evaluate_is_refused() {
    let error = generate("struct S { long a[1/0]; };").expect_err("a refusal");
    assert!(
        error.contains("array length does not evaluate"),
        "unexpected refusal: {error}"
    );
}

/// The same array, through the codec generator rather than the
/// declaration generator.
///
/// `encode.rs`'s `write_of` has its own refusal for this —
/// `MemberWrite::Unsupported("array whose length does not evaluate")` —
/// and until now nothing reached it: the test above goes through
/// `rust_items` → `adapter/cst.rs`, whose message is the differently
/// worded "array length does not evaluate" from a different function in
/// a different file. `docs/codec-coverage-report.md` measured the site
/// at zero regions under every configuration including the whole
/// workspace. The fix is right; it had no test.
#[test]
fn an_array_length_that_does_not_evaluate_is_refused_by_the_codec_too() {
    let idl = "struct S { long a[1/0]; };";
    let parsed = zerodds_idl_parse::parse(idl);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "unevaluated.idl");
    let mut ast: Ast = lowered.ast;
    let _ = ast.resolve_all_by_name();
    let type_ = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![String::from("S")]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("S is in the model");

    // Both encodings, because the refusal is a property of the member
    // and not of the framing it would have been written under.
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        let key = LayoutKey::data(encoding, Endianness::Little)
            .with_length_codes(LengthCodePolicy::default())
            .with_collection_delimiters(CollectionDelimiterPolicy::default());
        let error = rust_codec(&ast, type_, key).expect_err("a refusal");
        let error = error.to_string();
        assert!(
            error.contains("array whose length does not evaluate"),
            "unexpected refusal under {encoding:?}: {error}"
        );
    }
}

/// A constant whose value does not evaluate is not zero.
#[test]
fn a_constant_that_does_not_evaluate_is_refused() {
    let error = generate("const long C = 1/0;").expect_err("a refusal");
    assert!(
        error.contains("C: the constant's value does not evaluate"),
        "unexpected refusal: {error}"
    );
}

/// The same source, with the expression repaired, still generates.
///
/// Without this the tests above would pass for a generator that refused
/// everything.
#[test]
fn the_same_sources_generate_once_the_expression_is_sound() {
    assert_eq!(
        generate("struct S { long a[2/1]; };").expect("generated"),
        "#[derive(Debug, Clone, PartialEq)]\npub struct S {\n    pub a: [i32; 2],\n}\n"
    );
    assert_eq!(
        generate("const long C = 6/2;").expect("generated"),
        "pub const C: i32 = 3;\n"
    );
}

/// A derived struct's generated type holds its base as the first member `parent`.
///
/// `Ast::members_of` answers with a type's own members. The layout plan
/// walks `all_members_of` and writes the base's members first, so a
/// generated struct built from the partial accessor could not hold what
/// the encoder writes for it — and it compiled, because a struct with
/// fewer fields is still a struct. XTypes 1.3 §7.4.1.1.5.1.
#[test]
fn a_generated_struct_has_the_members_it_inherits() {
    let generated = generate(
        "struct Base { long base_a; short base_b; };
         struct Sample : Base { double own; };",
    )
    .expect("generated");
    assert!(
        generated.contains(
            "pub struct Sample {\n    /// The base structure, the first member (XTypes 1.3 §7.5.1.4.1.3).\n    pub parent: Base,\n    pub own: f64,\n}"
        ),
        "the base's members are missing:\n{generated}"
    );
}
