// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The read direction of the TypeScript column, generated and then
//! *type-checked*.
//!
//! Two halves, and the second is the one that matters. The first asks
//! the walk for a decoder over the shapes the corpus has and fails where
//! a construct still refuses — the same thing
//! `cpp_reader_generates.rs` does, and it needs no toolchain.
//!
//! The second writes the materialiser's declarations and the generated
//! decoders into one file and runs `tsc --strict` over it beside
//! `runtime/typescript/src`. That is the only reader of a great many
//! decisions in this backend: `CodecTarget::cast` renders `as T`, which
//! `tsc` refuses between a `bigint` and a `number` and *accepts*
//! silently where the two overlap; `CodecTarget::value_type` names a
//! type that either resolves at the file root or does not;
//! `wide_unsigned` writes a literal that either parses or does not.
//! None of those has a Rust reader at all. Every one of them was wrong
//! in this backend before this test existed.
//!
//! `tsc` is skipped where no compiler is installed, and the skip says so
//! rather than passing quietly — and inside a CI job it is not skipped
//! at all but refused, because the image carries the compiler.
// `print_stdout`: a skipped compiler has to say so on the harness's own
// channel. A test that went green without running `tsc` would be the
// exact silent success this column exists to refuse, and `cargo test`
// shows a passing test's output only when asked — so the note goes to
// stdout beside the test's own name.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stdout
)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::default_typescript::typescript_defaulters_called_by;
use zerodds_lang_rules::emit::decode::decoder_from;
use zerodds_lang_rules::emit::encode::codec_from;
use zerodds_lang_rules::emit::typescript::TypeScript;
use zerodds_lang_rules::{ModelGraph, apply};

mod typescript_toolchain;

use typescript_toolchain::compiler_or_note;

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
        let read = decoder_from::<TypeScript>(&ast, id, key, Default::default())
            .unwrap_or_else(|error| panic!("{type_name} under {encoding:?}: no reader: {error}"));
        assert!(
            !read.is_empty(),
            "{type_name} under {encoding:?}: the reader is empty"
        );
        let defaulters = typescript_defaulters_called_by(&ast, &read)
            .unwrap_or_else(|error| panic!("{type_name} under {encoding:?}: no Table 9: {error}"));
        // Every defaulter the decoder calls has to be one the batch also
        // defines: a call with no definition is a `tsc` error in the
        // generated file rather than here, which is a whole sweep later.
        assert!(
            !defaulters.is_empty(),
            "{type_name} under {encoding:?}: the decoder calls no defaulter, so its \
             own type has none either"
        );
        let written = codec_from::<TypeScript>(&ast, id, key, Default::default())
            .unwrap_or_else(|error| panic!("{type_name} under {encoding:?}: no writer: {error}"));
        assert!(!written.is_empty());
    }
}

#[test]
fn a_final_struct_gets_a_reader() {
    both_directions("@final struct S { long a; double b; boolean c; };", "S");
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
    both_directions("@final struct S { @optional long a; long b; };", "S");
}

#[test]
fn a_sequence_gets_a_reader() {
    both_directions("@final struct S { sequence<long> many; };", "S");
}

#[test]
fn an_array_gets_a_reader() {
    both_directions("@final struct S { long grid[2][3]; };", "S");
}

#[test]
fn a_string_and_a_wide_string_get_a_reader() {
    both_directions(
        "@final struct S { string<8> narrow; wstring<8> wide; };",
        "S",
    );
}

#[test]
fn an_enumeration_gets_a_reader() {
    both_directions(
        "enum Colour { RED, GREEN, BLUE }; @final struct S { Colour c; };",
        "S",
    );
}

/// `@bit_bound(8)` with a value above 127, which is where the Java
/// column's `readEnum` and its generated chain disagreed: §7.4.3.5.3
/// rule (5) makes the holder signed, so `@value(200)` is the pattern
/// `-56` and an unsigned read matched no arm at all.
#[test]
fn a_signed_enumerator_pattern_gets_a_reader() {
    both_directions(
        "@bit_bound(8) enum Level { @value(1) LOW, @value(200) HIGH }; \
         @final struct S { Level l; };",
        "S",
    );
}

#[test]
fn a_bitmask_gets_a_reader() {
    both_directions(
        "@bit_bound(40) bitmask Wide { @position(0) A, @position(39) B }; \
         @final struct S { Wide w; };",
        "S",
    );
}

#[test]
fn a_bitset_gets_a_reader() {
    both_directions(
        "bitset B { bitfield<3> a; bitfield<5> b; }; @final struct S { B bits; };",
        "S",
    );
}

#[test]
fn a_union_gets_a_reader() {
    both_directions(
        "@final union Choice switch (long) { case 1: long one; default: short other; }; \
         @final struct S { Choice pick; };",
        "S",
    );
}

#[test]
fn a_nested_struct_gets_a_reader() {
    both_directions(
        "@final struct Inner { long x; }; @final struct S { Inner part; };",
        "S",
    );
}

#[test]
fn a_module_nested_type_gets_a_reader() {
    both_directions(
        "module a { module b { @final struct Point { long x; long y; }; }; }; \
         @final struct S { a::b::Point p; };",
        "S",
    );
}

#[test]
fn a_sixty_four_bit_member_gets_a_reader() {
    both_directions(
        "@final struct S { long long a; unsigned long long b; };",
        "S",
    );
}

/// The whole of one model as a TypeScript file: the prelude, the
/// materialiser's declarations, the decoders and their defaulters.
fn generated_file(source: &str, type_name: &str) -> String {
    let ast = model(source);
    let graph = ModelGraph::of(&ast);
    let applied =
        apply(&graph, &zerodds_lang_rules::rule::typescript::catalogue()).expect("the rules apply");
    let id = root(&ast, type_name);
    let key = LayoutKey::in_dialect(
        Dialect::Specification,
        Encoding::PlCdr2,
        Endianness::Little,
        Purpose::Data,
    );
    let mut decls = zerodds_lang_rules::adapter::typescript::prelude();
    decls.extend(
        zerodds_lang_rules::adapter::typescript::ts_items(&ast, &applied)
            .expect("the model materialises"),
    );
    let decoders = decoder_from::<TypeScript>(&ast, id, key, Default::default())
        .expect("the walk reaches the end");
    let defaulters =
        typescript_defaulters_called_by(&ast, &decoders).expect("Table 9 for every type reached");
    decls.extend(defaulters);
    decls.extend(decoders);
    // The **read** direction alone. The write direction is generated
    // from the same walk and has defects of its own — `writeBitset` is
    // asked for a `w.bitfield` the runtime does not declare, and a
    // bitmask's value reaches `writeBitmask` as a `number` where the
    // parameter is a `bigint` — and putting both in one file would mean
    // this test could not go green until someone else's half did.
    // Those are recorded against the write direction, not swept in here.
    let body = decls
        .iter()
        .map(zerodds_lang_render::typescript::decl)
        .collect::<Vec<_>>()
        .join("\n");
    // The runtime by relative path, which is what the probe's generated
    // file does too: a package name would need an install step, and this
    // test type-checks the tree as it stands.
    format!(
        "import {{\n  Reader,\n  ReaderMark,\n  PidRead,\n  EmHeaderRead,\n  Status as ZdReaderStatus,\n  \
         TryConstruct,\n  ParameterRole,\n  WcharWidth,\n  WcharFraming,\n  \
         WstringUnitOrder,\n  WstringLengthPolicy,\n  FixedDigitCount,\n  zdNumber,\n  \
         zdBigInt,\n  zdChar,\n  pidIsUserMember,\n  utf8Octets,\n  utf16Units,\n\
         }} from \"../src/index.js\";\n\
         // Every import is used by *some* model; a file whose model does not\n\
         // reach one would fail `noUnusedLocals`, which this tree does not set.\n\
         export type ZdUsedTypes = [ReaderMark, PidRead, EmHeaderRead];\n\
         export const zdUsed = [\n  Reader, ZdReaderStatus, TryConstruct, ParameterRole,\n  \
         WcharWidth, WcharFraming, WstringUnitOrder, WstringLengthPolicy, FixedDigitCount,\n  \
         zdNumber, zdBigInt, zdChar, pidIsUserMember, utf8Octets, utf16Units,\n];\n\n{body}\n"
    )
}

/// Every shape above, in one file each, through `tsc`.
///
/// One test and not one per shape, because `tsc` costs seconds and the
/// failure names the file it came from anyway.
#[test]
fn the_generated_reader_type_checks() {
    let shapes: &[(&str, &str, &str)] = &[
        (
            "primitives",
            "@final struct S { long a; double b; boolean c; char d; };",
            "S",
        ),
        ("wchar", "@final struct S { wchar w; };", "S"),
        (
            "wide",
            "@final struct S { long long a; unsigned long long b; };",
            "S",
        ),
        (
            "appendable",
            "@appendable struct S { long a; long b; };",
            "S",
        ),
        (
            "mutable",
            "@mutable struct S { @id(1) long a; @id(2) long b; };",
            "S",
        ),
        (
            "optional",
            "@final struct S { @optional long a; long b; };",
            "S",
        ),
        ("sequence", "@final struct S { sequence<long> many; };", "S"),
        ("array", "@final struct S { long grid[2][3]; };", "S"),
        (
            "strings",
            "@final struct S { string<8> narrow; wstring<8> wide; };",
            "S",
        ),
        (
            "enumeration",
            "enum Colour { RED, GREEN, BLUE }; @final struct S { Colour c; };",
            "S",
        ),
        (
            "enum_signed",
            "@bit_bound(8) enum Level { @value(1) LOW, @value(200) HIGH }; \
             @final struct S { Level l; };",
            "S",
        ),
        (
            "bitmask",
            "@bit_bound(40) bitmask Wide { @position(0) A, @position(39) B }; \
             @final struct S { Wide w; };",
            "S",
        ),
        (
            "bitset",
            "bitset B { bitfield<3> a; bitfield<5> b; }; @final struct S { B bits; };",
            "S",
        ),
        (
            "union",
            "@final union Choice switch (long) { case 1: long one; default: short other; }; \
             @final struct S { Choice pick; };",
            "S",
        ),
        (
            "modules",
            "module a { module b { @final struct Point { long x; long y; }; }; }; \
             @final struct S { a::b::Point p; };",
            "S",
        ),
        (
            "nested",
            "@final struct Inner { long x; }; @final struct S { Inner part; };",
            "S",
        ),
    ];
    // A skip that says so, and in CI no skip at all. A test that passed
    // here without running the compiler would be the exact silent
    // success this column is built to refuse.
    let Some(tsc) = compiler_or_note("the TypeScript column") else {
        return;
    };
    let mut failures: Vec<String> = Vec::new();
    for (name, source, type_name) in shapes {
        let text = generated_file(source, type_name);
        let report = typescript_toolchain::typecheck(&tsc, name, &text, &[]);
        if !report.trim().is_empty() {
            failures.push(format!("-- {name}\n{report}"));
        }
    }
    assert!(
        failures.is_empty(),
        "the generated TypeScript does not type-check:\n{}",
        failures.join("\n")
    );
}
