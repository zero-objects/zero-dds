// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-ocaml-gen` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`src/lib.rs`, `source`):
//! `emit::encode::codec_from::<OCaml>` with `codec_name`, `emit::decode::decoder_from::<OCaml>` with `decoder_name`, `default_ocaml::ocaml_defaulters_called_by`, `adapter::ocaml::ocaml_file` over `apply` with `rule::ocaml::catalogue()`, `EncodeError::is_spec_refusal`. It writes the OCaml program the OCaml wire probe
//! builds: the declarations, the codec of the sample's type and, to read
//! octets back, the decoder with the defaults it calls; the harness calls
//! the codec by the name `codec_name` gives, and a refusal the
//! specification makes is a verdict of its own (exit 1), a refusal of
//! this tree's walk another (exit 6).
//!
//! Oracle: DDS-XTypes 1.3 §7.4.3.4.2 (FINAL writes the members alone,
//! APPENDABLE a DHEADER before them, MUTABLE an EMHEADER before each),
//! §7.2.2.4.4.4.9 Table 9 (a member the stream did not carry takes its
//! default, so a decoder of an APPENDABLE type calls defaulters), IDL 4.2
//! §7.4.1.4.4.4.4 (a type that contains itself with no sequence, map or
//! optional member on the way has no finite value); the documented
//! invariant of `codec_name` and `decoder_name` (the name of the function
//! the walk emits for the type).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::emit::decode::{decoder_from, decoder_name};
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from, codec_name};
use zerodds_lang_rules::emit::ocaml::OCaml;

fn model(idl: &str) -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "contract.idl").ast;
    ast.resolve_all_by_name().unwrap();
    ast
}

fn id(ast: &zerodds_ast::Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types().find(|(_, n)| n.name == name).unwrap().0
}

/// The emitted declarations as one lower-case string without underscores,
/// so that a runtime call reads the same in every spelling of it.
fn flat<T: core::fmt::Debug>(items: &[T]) -> String {
    format!("{items:?}").to_lowercase().replace('_', "")
}

fn codec(idl: &str, name: &str, encoding: Encoding) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(encoding, Endianness::Little);
    flat(&codec_from::<OCaml>(&ast, id(&ast, name), key, CodecOptions::default()).unwrap())
}

#[test]
fn xtypes_7_4_3_4_2_an_appendable_codec_opens_a_dheader_and_a_final_one_does_not() {
    assert!(
        codec(
            "@appendable struct Ap { long x; };",
            "Ap",
            Encoding::DelimitedCdr
        )
        .contains("writedheader")
    );
    assert!(
        !codec("@final struct F { long a; };", "F", Encoding::PlainCdr2).contains("writedheader")
    );
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_codec_writes_an_emheader() {
    assert!(
        codec(
            "@mutable struct M { long a; long b; };",
            "M",
            Encoding::PlCdr2
        )
        .contains("writeemheader")
    );
}

#[test]
fn the_harness_finds_the_codec_by_the_name_codec_name_gives() {
    let ast = model("@appendable struct Reading { long id; string label; };");
    let key = LayoutKey::data(Encoding::DelimitedCdr, Endianness::Little);
    let root = id(&ast, "Reading");
    let encoder = codec_from::<OCaml>(&ast, root, key, CodecOptions::default()).unwrap();
    let name = codec_name::<OCaml>(&ast, root, Purpose::Data);
    assert!(format!("{encoder:?}").contains(&name), "`{name}`");
    let decoders = decoder_from::<OCaml>(&ast, root, key, CodecOptions::default()).unwrap();
    let name = decoder_name::<OCaml>(&ast, root);
    assert!(format!("{decoders:?}").contains(&name), "`{name}`");
}

#[test]
fn xtypes_7_2_2_4_4_4_9_a_decoder_of_an_appendable_type_is_given_its_defaulters() {
    let ast = model("@appendable struct Reading { long id; string label; double value; };");
    let key = LayoutKey::data(Encoding::DelimitedCdr, Endianness::Little);
    let decoders =
        decoder_from::<OCaml>(&ast, id(&ast, "Reading"), key, CodecOptions::default()).unwrap();
    let defaulters =
        zerodds_lang_rules::default_ocaml::ocaml_defaulters_called_by(&ast, &decoders).unwrap();
    assert!(!defaulters.is_empty());
}

#[test]
fn idl_7_4_1_4_4_4_4_a_type_without_a_finite_value_is_the_specifications_refusal() {
    let parsed = zerodds_idl_parse::parse("struct Loop { long a; Loop again; };");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "loop.idl").ast;
    let _ = ast.resolve_all_by_name();
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let error = codec_from::<OCaml>(&ast, id(&ast, "Loop"), key, CodecOptions::default())
        .expect_err("a value that never ends has no layout");
    assert!(error.is_spec_refusal(), "{error}");
}

#[test]
fn the_materialiser_has_a_form_for_a_struct_an_enum_and_a_union() {
    let ast = model(
        "enum Kind { A, B }; struct S { long a; Kind k; }; union U switch (long) { case 1: long x; default: double y; };",
    );
    let applied = zerodds_lang_rules::apply(
        &zerodds_lang_rules::ModelGraph::of(&ast),
        &zerodds_lang_rules::rule::ocaml::catalogue(),
    )
    .unwrap();
    assert!(zerodds_lang_rules::adapter::ocaml::ocaml_file(&ast, &applied).is_ok());
}
