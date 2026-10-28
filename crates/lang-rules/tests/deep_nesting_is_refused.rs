// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A type that nests past the walks' bound is stopped, and by a counter.
//!
//! Every reference walk in this crate carries the same 64 that
//! `zerodds-wire-codec`'s `default.rs` and `zerodds-types`'
//! `dynamic/type_.rs` stop at. A dozen of them carried no counter at
//! all: the materialisers followed a member's type through `sequence`,
//! `array`, `map` and `typedef` edges and stopped only because the graph
//! they had been handed happened to be acyclic. That is a termination
//! argument, not a bound — the same distinction that took the CORBA C++
//! emitter's stack out on `A → sequence<B> → B → sequence<A>`.
//!
//! These fire the counters. The input is legal IDL the frontend accepts,
//! so what is measured is each walk's own answer and not the parser's.
//!
//! Two shapes of answer, because the walks have two shapes of return.
//! A walk that returns a `Result` refuses and names the spelling that
//! nested. A walk that returns a `String` — a type name, a structural
//! tag — has no error channel to refuse through, so it stops and names
//! the type by its arena slot: a *distinct* name per type, so two types
//! that both nest past the bound cannot collapse onto one declaration.
//! Both are counted stops. Neither is the stack running out.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{
    CollectionDelimiterPolicy, Encoding, Endianness, LayoutKey, LengthCodePolicy,
};
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::adapter::cst::rust_items;
use zerodds_lang_rules::encode::rust_codec;
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply, generate};

/// How many `sequence<>` the member is wrapped in.
///
/// Past the 64 every walk stops at, and not by one: a bound written
/// `depth > MAX` and one written `depth >= MAX` both have to fire, so an
/// off-by-one in either direction still fails the test rather than
/// passing it by accident.
const NESTING: usize = 80;

/// The languages whose walk answers with a `String` and stops by naming
/// the type after its arena slot rather than by refusing.
///
/// C's `c_type_name`, Ada's `ada_type_name` and Ada 83's are name
/// builders the emitters call where no `Result` can be returned — the
/// `CodecTarget` trait spells them `-> String`. The counter is the same
/// 64; what differs is what it hands back.
const NAMES_THE_SLOT: [Language; 3] = [Language::C, Language::Ada, Language::Ada83];

/// The marker each of those three writes into the output.
///
/// Two functions in each column stop this way — the type name and the
/// structural tag — and they differ only in the prefix the column puts
/// in front (`Zd_Deep_7` from the name builder, `Deep_7` from the tag).
/// The shared tail is what is looked for, so either one satisfies it.
fn slot_marker(language: Language) -> &'static str {
    match language {
        Language::C => "zd_deep_",
        _ => "Deep_",
    }
}

/// `struct S { sequence<sequence<…<long>…>> a; };`, `NESTING` deep.
fn deeply_nested_idl(prefix: &str, suffix: &str) -> String {
    let mut text = String::from(prefix);
    for _ in 0..NESTING {
        text.push_str("sequence<");
    }
    text.push_str("long");
    for _ in 0..NESTING {
        text.push_str(" >");
    }
    text.push_str(suffix);
    text
}

/// The model a deep source lowers to.
fn deep_ast(prefix: &str, suffix: &str) -> Ast {
    let idl = deeply_nested_idl(prefix, suffix);
    let parsed = zerodds_idl_parse::parse(&idl);
    zerodds_idl_lower::lower(&parsed.syntax(), "deep_nesting.idl").ast
}

/// The frontend accepts the source, so the answers below are the walks'
/// and not the parser's.
///
/// Without this the whole file could pass on a model with no members in
/// it at all.
#[test]
fn the_frontend_builds_the_deep_model() {
    let ast = deep_ast("struct S { ", " a; };");
    assert!(
        ast.types().count() > NESTING,
        "the frontend did not build the nesting: {} types",
        ast.types().count()
    );
}

/// Whether every materialiser stops, and how.
///
/// One test over `Language::ALL` rather than twenty-one: the bound is
/// the same 64 in every column, and a language added to the list with an
/// unbounded walk fails here on the day it is added.
///
/// `Node` is the one exclusion, and not for a depth reason:
/// `rule::catalogue(Language::Node)` is deliberately empty — the module
/// documentation on `rule/mod.rs` gives why — so its materialiser
/// produces an empty file for *any* input and has nothing to stop.
fn every_materialiser_stops(ast: &Ast, what: &str) {
    for language in Language::ALL {
        if language == Language::Node {
            continue;
        }
        let result = generate::declarations(ast, language);
        if NAMES_THE_SLOT.contains(&language) {
            let text = result.unwrap_or_else(|error| {
                panic!("{language:?} refused {what} rather than naming it: {error}")
            });
            assert!(
                text.contains(slot_marker(language)),
                "{language:?} emitted {what} with no `{}` marker — the walk did not stop at \
                 the bound",
                slot_marker(language)
            );
        } else {
            let error = result.err().unwrap_or_else(|| {
                panic!("{language:?} materialised {what} instead of refusing it")
            });
            let text = error.to_string();
            assert!(
                text.contains("nests past 64")
                    || text.contains("nested past 64")
                    || text.contains("deeper than the planner follows"),
                "{language:?} refused {what} without naming the bound: {text}"
            );
        }
    }
}

/// A member whose type is `NESTING` sequences deep.
#[test]
fn every_materialiser_stops_on_a_deeply_nested_member() {
    let ast = deep_ast("struct S { ", " a; };");
    every_materialiser_stops(&ast, "a type nested 80 deep");
}

/// A `map` whose value is `NESTING` sequences deep.
///
/// The map entry name is the one walk that used to reset its own
/// counter: `type_tag` reached `<lang>_map_entry_name`, which called
/// `type_tag` back at depth 0. A cycle through a map's value therefore
/// had no bound at all even in the columns where `type_tag` already
/// carried one. The entry name now threads the depth instead of starting
/// it over, and this is what walks that pair.
#[test]
fn every_materialiser_stops_on_a_map_whose_value_nests_past_the_bound() {
    let ast = deep_ast("struct S { map<long, ", " > m; };");
    every_materialiser_stops(&ast, "a map whose value nests 80 deep");
}

/// The Rust materialiser names the construct and the bound.
///
/// `adapter/cst.rs`'s `rust_type_of` followed `Sequence`, `Array`,
/// `Map`, `ValueBox` and *every* `typedef` — named ones included — with
/// nothing counting the steps.
#[test]
fn the_rust_materialiser_names_the_spelling_that_nested() {
    let ast = deep_ast("struct S { ", " a; };");
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply");
    let error = rust_items(&ast, &applied).expect_err("a refusal");
    let text = error.to_string();
    assert!(
        text.contains("Rust spelling nests past 64"),
        "unexpected refusal: {text}"
    );
}

/// The codec chain stops too, and says which bound it stopped at.
///
/// Two counters stand on this path and the layout planner's is the first
/// of them: `zerodds-ast-layout` reports `TooDeep` for the member before
/// `encode.rs` is asked to write anything. `write_of`'s own counter —
/// `MAX_WRITE_DEPTH`, answering with
/// `MemberWrite::Unsupported("a type whose writer nests past the
/// recursion bound")` — sits behind it, for a graph the planner admits
/// and the writer would still have descended forever on. What this
/// measures is that the chain answers rather than running the stack out;
/// which of the two counters spoke is named in the message.
#[test]
fn the_rust_codec_stops_on_a_writer_that_nests_past_the_bound() {
    let mut ast = deep_ast("struct S { ", " a; };");
    let _ = ast.resolve_all_by_name();
    let type_ = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![String::from("S")]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("S is in the model");
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
        .with_length_codes(LengthCodePolicy::default())
        .with_collection_delimiters(CollectionDelimiterPolicy::default());
    let error = rust_codec(&ast, type_, key).expect_err("a refusal");
    let text = error.to_string();
    assert!(
        text.contains("TooDeep") || text.contains("nests past the recursion bound"),
        "unexpected refusal: {text}"
    );
}
