// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The initialiser, destructor and copy functions of the C aggregates that
//! hold `@external` members. DDS-XTypes 1.3 §7.5.1.2.3.1.
//!
//! What this file does not do is compile the output: `c_corpus_compiles`
//! and the wire probe do that.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::c::c_items;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

fn declarations(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::c::catalogue()).expect("apply");
    let decls = c_items(&ast, &applied).expect("materialise");
    zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        guard: None,
        includes: Vec::new(),
        decls,
    })
}

const SAMPLE: &str = "module m { struct Inner { long a; }; \
                      @final struct Sample { @external Inner nested; @external long a; long b; }; };";

#[test]
fn an_aggregate_with_an_external_member_gets_the_four_functions() {
    let generated = declarations(SAMPLE);
    for function in [
        "zd_init_m_Sample",
        "zd_fini_m_Sample",
        "zd_copy_m_Sample",
        "zd_copy_with_m_Sample",
    ] {
        assert!(generated.contains(function), "{function}:\n{generated}");
    }
    // The aggregate without one gets none.
    assert!(!generated.contains("zd_init_m_Inner"), "{generated}");
}

#[test]
fn the_initialiser_sets_the_external_pointers_to_null() {
    let generated = declarations(SAMPLE);
    assert!(generated.contains("v->nested = NULL;"), "{generated}");
    assert!(generated.contains("v->a = NULL;"), "{generated}");
}

#[test]
fn the_destructor_deletes_what_a_non_null_pointer_references() {
    let generated = declarations(SAMPLE);
    assert!(
        generated.contains("if (v->nested != NULL) {"),
        "{generated}"
    );
    assert!(generated.contains("free(v->nested);"), "{generated}");
    assert!(generated.contains("free(v->a);"), "{generated}");
}

#[test]
fn the_copy_allocates_a_null_destination_and_fills_it() {
    let generated = declarations(SAMPLE);
    assert!(
        generated.contains("dst->nested = (m_Inner *)malloc(sizeof(m_Inner));"),
        "{generated}"
    );
    assert!(
        generated.contains("(*dst->nested) = (*src->nested);"),
        "{generated}"
    );
    // A failed allocation fails the copy of the containing object.
    assert!(generated.contains("return false;"), "{generated}");
}

#[test]
fn the_copy_has_the_modes_of_table_45() {
    let generated = declarations(SAMPLE);
    for name in [
        "ZD_COPY_SHALLOW",
        "ZD_COPY_DEEP",
        "ZD_COPY_REUSE",
        "ZD_COPY_REPLACE",
        "ZD_COPY_RELEASE",
    ] {
        assert!(generated.contains(name), "{name}:\n{generated}");
    }
    // The default copy is deep and reuses.
    assert!(
        generated.contains("ZD_COPY_DEEP, ZD_COPY_REUSE"),
        "{generated}"
    );
}

#[test]
fn an_aggregate_that_holds_one_inline_calls_the_functions_of_it() {
    let generated = declarations(
        "module m { struct Inner { @external long a; }; \
         @final struct Outer { Inner held; long z; }; };",
    );
    assert!(generated.contains("zd_init_m_Outer"), "{generated}");
    assert!(
        generated.contains("zd_init_m_Inner(&v->held);"),
        "{generated}"
    );
    assert!(
        generated.contains("zd_fini_m_Inner(&v->held);"),
        "{generated}"
    );
}

#[test]
fn an_external_branch_of_a_union_is_released_by_the_live_discriminator() {
    let generated = declarations(
        "module m { struct Inner { long a; }; \
         union Choice switch (long) { case 1: @external Inner nested; case 2: long plain; }; };",
    );
    assert!(generated.contains("zd_fini_m_Choice"), "{generated}");
    assert!(generated.contains("v->_d == (int32_t)1"), "{generated}");
    assert!(generated.contains("free(v->_u.nested);"), "{generated}");
}

#[test]
fn a_model_without_an_external_member_gets_nothing() {
    let generated = declarations("module m { struct Plain { long a; string s; }; };");
    assert!(!generated.contains("ZD_LIFECYCLE_DEFINED"), "{generated}");
    assert!(!generated.contains("zd_init_"), "{generated}");
    // The standard headers of the lifecycle functions; the one include
    // there is is the map operations' (XTypes 1.3 §7.5.1.3.2).
    assert!(!generated.contains("#include <stdlib.h>"), "{generated}");
}

#[test]
fn an_external_string_is_a_pointer_the_initialiser_clears_and_nothing_releases() {
    // §7.5.1.2.3.1: a string "is already represented using pointers", so
    // the mapping does not change, and "The constructor/initializer of
    // the enclosing object shall set the external member pointers to
    // NULL". The bytes are the caller's: no `free`.
    let generated =
        declarations("module m { @final struct Holder { @external string s; long z; }; };");
    assert!(generated.contains("zd_init_m_Holder"), "{generated}");
    assert!(generated.contains("v->s = NULL;"), "{generated}");
    assert!(!generated.contains("free(v->s);"), "{generated}");
}

#[test]
fn an_optional_member_is_a_pointer_the_functions_initialise_release_and_copy() {
    // §7.5.1.2.4.1 holds an optional member behind a pointer and says a
    // NULL one is the absent member. The binding gives it the functions
    // an external member has (the clause is silent on how the pointer is
    // owned): NULL after the initialiser, `free` and NULL at the
    // destructor, a deep copy. A `string` is a pointer already and its
    // bytes are the caller's, so it is only set to NULL.
    let generated = declarations(
        "module m { @final struct O { @optional long a; @optional string s; long z; }; };",
    );
    assert!(generated.contains("zd_init_m_O"), "{generated}");
    assert!(generated.contains("v->a = NULL;"), "{generated}");
    assert!(generated.contains("v->s = NULL;"), "{generated}");
    assert!(generated.contains("free(v->a);"), "{generated}");
    assert!(!generated.contains("free(v->s);"), "{generated}");
    assert!(
        generated.contains("dst->a = (int32_t *)malloc(sizeof(int32_t));"),
        "{generated}"
    );
    assert!(generated.contains("dst->s = src->s;"), "{generated}");
}
