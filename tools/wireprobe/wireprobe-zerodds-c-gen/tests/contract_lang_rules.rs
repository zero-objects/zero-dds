// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-c-gen` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`src/lib.rs`, `src/fill.rs`,
//! `src/dump.rs`): `adapter::c::c_items` over `apply` with
//! `rule::c::catalogue()`, `encode_c::c_codec_from` with `codec_name`,
//! `encode_c::c_key_holder`, `decode_c::c_decoder_from` with `decoder_name`
//! (the defaulters ahead of the decoders), `adapter::c::{c_type_name,
//! module_prefix, c_members}` for the sample's fill and dump, and
//! `EncodeError::is_spec_refusal`. It writes the C program the C wire probe
//! builds; the harness calls the codec by the name `codec_name` gives.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.3.4.2 (FINAL writes the members alone,
//! APPENDABLE a DHEADER before them, MUTABLE an EMHEADER before each),
//! §7.6.8 (a keyed type has a key holder, an unkeyed one none),
//! §7.2.2.4.4.4.9 Table 9 (a decoder of an APPENDABLE type calls defaulters),
//! IDL 4.2 §7.5.2 (a C name of a type in a module carries the module as a
//! prefix, C having no namespaces) and §7.4.1.4.4.4.4 (a type that contains
//! itself with no sequence, map or optional member on the way has no
//! finite value).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::decode_c::{c_decoder_from, decoder_name};
use zerodds_lang_rules::encode_c::{c_codec_from, c_key_holder, codec_name};

fn model(idl: &str) -> zerodds_ast::Ast {
    zerodds_wire_probe::encode::model_from_idl(idl, "contract.idl").unwrap()
}

fn id(ast: &zerodds_ast::Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types().find(|(_, n)| n.name == name).unwrap().0
}

fn flat<T: core::fmt::Debug>(items: &[T]) -> String {
    format!("{items:?}").to_lowercase().replace('_', "")
}

fn codec(idl: &str, name: &str, encoding: Encoding) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(encoding, Endianness::Little);
    flat(&c_codec_from(&ast, id(&ast, name), key, Default::default()).unwrap())
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
    assert!(
        codec("@mutable struct M { long a; };", "M", Encoding::PlCdr2).contains("writeemheader")
    );
}

#[test]
fn the_harness_finds_the_codec_and_the_decoder_by_their_names() {
    let ast = model("@appendable struct Reading { long id; string label; };");
    let key = LayoutKey::data(Encoding::DelimitedCdr, Endianness::Little);
    let root = id(&ast, "Reading");
    let encoder = c_codec_from(&ast, root, key, Default::default()).unwrap();
    assert!(format!("{encoder:?}").contains(&codec_name(&ast, root, Purpose::Data)));
    let decoders = c_decoder_from(&ast, root, key, Default::default()).unwrap();
    assert!(format!("{decoders:?}").contains(&decoder_name(&ast, root)));
}

#[test]
fn xtypes_7_6_8_a_keyed_type_has_a_key_holder_and_an_unkeyed_one_none() {
    let ast = model("@final struct K { @key long id; long v; }; @final struct N { long v; };");
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    assert!(c_key_holder(&ast, id(&ast, "K"), key).unwrap().is_some());
    assert!(c_key_holder(&ast, id(&ast, "N"), key).unwrap().is_none());
}

#[test]
fn idl_7_5_2_a_c_type_name_carries_its_module_as_a_prefix() {
    let ast = model("module m { struct S { long a; }; };");
    let name = zerodds_lang_rules::adapter::c::c_type_name(&ast, id(&ast, "S"));
    assert!(
        name.starts_with('m') && name.ends_with('S') && name.len() > 2,
        "{name}"
    );
}

#[test]
fn idl_7_4_1_4_4_4_4_a_type_without_a_finite_value_is_the_specifications_refusal() {
    let parsed = zerodds_idl_parse::parse("struct Loop { long a; Loop again; };");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "loop.idl").ast;
    let _ = ast.resolve_all_by_name();
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let error = c_codec_from(&ast, id(&ast, "Loop"), key, Default::default())
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
        &zerodds_lang_rules::rule::c::catalogue(),
    )
    .unwrap();
    assert!(zerodds_lang_rules::adapter::c::c_items(&ast, &applied).is_ok());
}
