// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-build` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`src/lib.rs`,
//! `render_with_stem`; `src/dds_type.rs`): `apply` with
//! `rule::rust::catalogue()`, `adapter::cst::{rust_item_pairs,
//! nest_by_module, RustMapForm, RustMapFormScope, path_from_file_root}`,
//! `encode::rust_codec` for each type's XCDR2 codec and
//! `encode::rust_codec_pair` for the XCDR1 pair, whose functions the
//! generated `DdsType` impl calls by the name `encode::SECONDARY_SUFFIX`
//! gives them, and `verbatim::weave_file` with `verbatim::aliases::RUST`.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.3.4.2 (an APPENDABLE aggregate under XCDR2
//! writes a DHEADER, a FINAL one none), IDL 4.2 §8.3.5.1 (`@verbatim` with
//! `placement` `BEGIN_FILE` and `END_FILE` puts its text at the start and
//! the end of the file, for the language it names), §7.5.2 (the scoped name
//! `outer::inner::T`), the Rust reference ("Raw identifiers": a keyword is
//! written `r#type`), zerodds-xcdr2-rust-1.0 (a `map` is
//! `Vec<zerodds_wire::MapEntry<K, V>>` unless `BTreeMap` is chosen).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::Doc;
use zerodds_lang_rules::adapter::cst::{
    RustMapForm, RustMapFormScope, path_from_file_root, rust_item_pairs,
};
use zerodds_lang_rules::encode::{SECONDARY_SUFFIX, rust_codec, rust_codec_pair};
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply};

fn model(idl: &str) -> Ast {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        idl,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    composed.ast
}

fn type_id(ast: &Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types().find(|(_, n)| n.name == name).unwrap().0
}

fn render(items: Vec<zerodds_lang_cst::rust::Item>) -> String {
    zerodds_lang_render::rust::file(&zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: Doc::lines(&[]),
        uses: Vec::new(),
        items,
    })
}

fn codec(idl: &str, name: &str, encoding: Encoding) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(encoding, Endianness::Little);
    render(rust_codec(&ast, type_id(&ast, name), key).unwrap())
}

#[test]
fn xtypes_7_4_3_4_2_an_appendable_codec_writes_a_dheader_and_a_final_one_none() {
    let appendable = codec(
        "@appendable struct Ap { long x; };",
        "Ap",
        Encoding::DelimitedCdr,
    );
    assert!(appendable.contains("write_dheader"), "{appendable}");
    let final_ = codec("@final struct F { long a; };", "F", Encoding::PlainCdr2);
    assert!(!final_.contains("write_dheader"), "{final_}");
}

#[test]
fn the_xcdr1_pair_is_named_with_the_secondary_suffix() {
    assert_eq!(SECONDARY_SUFFIX, "_xcdr1");
    let ast = model("@final struct P { long a; };");
    let key = LayoutKey::data(Encoding::PlainCdr1, Endianness::Little);
    let text = render(rust_codec_pair(&ast, type_id(&ast, "P"), key).unwrap());
    assert!(
        text.contains(&format!("fn encode{SECONDARY_SUFFIX}")),
        "{text}"
    );
    assert!(
        text.contains(&format!("fn decode{SECONDARY_SUFFIX}")),
        "{text}"
    );
}

#[test]
fn idl_8_3_5_1_verbatim_text_for_rust_wraps_the_file_and_text_for_c_does_not() {
    let ast = model(
        "@verbatim(language = \"rust\", placement = BEGIN_FILE, text = \"// rust begins\")
         @verbatim(language = \"rust\", placement = END_FILE, text = \"// rust ends\")
         @verbatim(language = \"c\", placement = BEGIN_FILE, text = \"// c begins\")
         struct S { long a; };",
    );
    let applied = apply(&ModelGraph::of(&ast), &rust::catalogue()).unwrap();
    let items = zerodds_lang_rules::adapter::cst::nest_by_module(
        &ast,
        rust_item_pairs(&ast, &applied).unwrap(),
    )
    .unwrap();
    let text = render(zerodds_lang_rules::verbatim::weave_file(
        &ast,
        zerodds_lang_rules::verbatim::aliases::RUST,
        items,
    ));
    let begins = text.find("// rust begins").expect("BEGIN_FILE");
    let ends = text.find("// rust ends").expect("END_FILE");
    let declared = text.find("struct S").expect("the struct");
    assert!(begins < declared && declared < ends, "{text}");
    assert!(!text.contains("// c begins"), "{text}");
}

#[test]
fn idl_7_5_2_the_path_from_the_file_root_is_the_scoped_name_with_keywords_escaped() {
    let ast = model("module outer { module inner { struct type { long a; }; }; };");
    assert_eq!(
        path_from_file_root(&ast, type_id(&ast, "type")).unwrap(),
        "outer::inner::r#type"
    );
}

#[test]
fn a_map_is_a_vector_of_entries_unless_the_build_chooses_a_btreemap() {
    let ast = model("struct M { map<long, string> table; };");
    let applied = apply(&ModelGraph::of(&ast), &rust::catalogue()).unwrap();
    let entries = render(
        rust_item_pairs(&ast, &applied)
            .unwrap()
            .into_iter()
            .map(|(_, item)| item)
            .collect(),
    );
    assert!(entries.contains("MapEntry<i32, String>"), "{entries}");
    let _scope = RustMapFormScope::enter(RustMapForm::BTreeMap);
    let btree = render(
        rust_item_pairs(&ast, &applied)
            .unwrap()
            .into_iter()
            .map(|(_, item)| item)
            .collect(),
    );
    assert!(btree.contains("BTreeMap<i32, String>"), "{btree}");
}
