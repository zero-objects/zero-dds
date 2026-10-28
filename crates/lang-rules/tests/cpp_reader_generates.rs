// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The five shapes a value can be absent in, each of which needs Table 9.
//!
//! `docs/specs/zerodds-interop-suite-1.0.md` names the read direction as
//! its own row, and the C++ column did not have one because
//! `CodecTarget::default_into` refused. These are the five forms that
//! refusal reached, and they are here rather than only in the corpus
//! sweep because the sweep needs a compiler and this does not: a
//! refusal comes back from the generator, and a generator that starts
//! refusing again should fail in `cargo test`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::decode_cpp::{CppCodecOptions, cpp_decoder_from};
use zerodds_lang_rules::emit::encode::codec_from;

fn model(source: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

fn root(ast: &Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name.as_str() == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("`{name}` is not a type in this model"))
}

/// A reader and a writer for `source`, under every encoding the suite
/// runs, or the refusal that came back.
fn both_directions(source: &str, type_name: &str) {
    let ast = model(source);
    let id = root(&ast, type_name);
    for encoding in [
        Encoding::PlainCdr1,
        Encoding::PlCdr1,
        Encoding::PlainCdr2,
        Encoding::DelimitedCdr,
        Encoding::PlCdr2,
    ] {
        let key = LayoutKey::in_dialect(
            Dialect::Specification,
            encoding,
            Endianness::Little,
            Purpose::Data,
        );
        let read = cpp_decoder_from(&ast, id, key, CppCodecOptions::default())
            .unwrap_or_else(|error| panic!("{type_name} under {encoding:?}: no reader: {error}"));
        assert!(
            !read.is_empty(),
            "{type_name} under {encoding:?}: the reader is empty"
        );
        let written = codec_from::<zerodds_lang_rules::emit::cpp::Cpp>(
            &ast,
            id,
            key,
            CppCodecOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{type_name} under {encoding:?}: no writer: {error}"));
        assert!(!written.is_empty());
    }
}

#[test]
fn an_appendable_struct_gets_a_reader() {
    both_directions("@appendable struct S { long a; long b; };", "S");
}

#[test]
fn a_mutable_struct_gets_a_reader() {
    both_directions("@mutable struct S { @id(1) long a; @id(2) long b; };", "S");
}

#[test]
fn an_optional_member_gets_a_reader() {
    both_directions("@appendable struct S { long a; @optional long b; };", "S");
}

#[test]
fn a_bounded_string_gets_a_reader() {
    both_directions("@final struct S { string<5> text; };", "S");
}

#[test]
fn a_union_gets_a_reader() {
    both_directions(
        "@final union Choice switch (long) { case 1: long a; case 2: string b; }; \
         @final struct S { Choice picked; };",
        "S",
    );
}

/// The kinds Table 9 answers for, all in one type: a defaulter that
/// refuses one of them refuses the whole reader, so this is where a hole
/// shows up rather than in whichever corpus case happened to reach it.
#[test]
fn every_kind_table_9_answers_for_gets_a_defaulter() {
    both_directions(
        "enum Colour { red, green, blue }; \
         @bit_bound(8) bitmask Flags { flag_a, flag_b }; \
         bitset Bits { bitfield<3> lo; bitfield<5> hi; }; \
         @final struct Inner { long a; }; \
         @appendable struct S { \
             boolean flag; octet o; char c; wchar w; \
             short small; unsigned short us; long l; unsigned long ul; \
             long long ll; unsigned long long ull; \
             float f; double d; \
             string str; wstring wstr; \
             Colour hue; Flags mask; Bits packed; Inner nested; \
             sequence<long> seq; long arr[2][3]; \
             @optional long opt; \
         };",
        "S",
    );
}

/// An enumeration whose first literal is not zero.
///
/// The row `T{}` gets wrong, and the reason `default_cpp` is a function
/// per type rather than value-initialisation: Table 9 for an enumeration
/// is "the first literal", and here that is 7.
#[test]
fn an_enumeration_whose_first_literal_is_not_zero() {
    both_directions(
        "enum Sparse { @value(7) seven, @value(9) nine }; \
         @appendable struct S { Sparse e; long tail; };",
        "S",
    );
}
