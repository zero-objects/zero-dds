// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated Rust codec carries the three HDDS axes.
//!
//! `Dialect::Hdds` selects `ElementPadding::Packed`,
//! `SequenceElementDelimiter::BeforeEachNonFinalElement` and
//! `WstringCodeUnits::Utf8`. The reflective codec `zerodds-wire-codec`
//! writes all three and reproduces the recorded HDDS octets; this checks
//! that the Rust column emits the calls that make the generated codec
//! write them too.
//!
//! The text is not the claim — `tools/idlc/tests/dialect_byte_equality.rs`
//! and the recorded `tests/interop-results/<case>/hdds/*.bin` files are —
//! but a generated file that does not name these functions cannot be
//! writing their octets.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::encode::rust_codec;

/// Lower `idl`, find the type at `path`, and render its Rust codec under
/// `key`.
fn codec(idl: &str, path: &[&str], key: LayoutKey) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "hdds.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let ast: Ast = lowered.ast;

    let scoped: Vec<String> = path.iter().map(|s| (*s).to_string()).collect();
    let id = ast
        .find_by_scoped_name(&ScopedName::absolute(scoped))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{path:?} is not in the model"));

    let items = rust_codec(&ast, id, key).expect("the Rust codec carries the HDDS axes");
    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    zerodds_lang_render::rust::file(&file)
}

fn hdds(encoding: Encoding) -> LayoutKey {
    LayoutKey::in_dialect(Dialect::Hdds, encoding, Endianness::Little, Purpose::Data)
}

/// `array-1d-struct-final` is `@final struct Cell { long a; short b; };
/// @final struct Sample { Cell items[2]; octet tail; };` and its recorded
/// octets are `tests/interop-results/array-1d-struct-final/hdds/`
/// `plain-cdr1.le.bin`: `01 00 00 00 02 00 03 00 00 00 04 00 77`, the
/// second `Cell` at offset six and not eight. That is
/// `ElementPadding::Packed`: every array element begins where the
/// previous one ended.
#[test]
fn an_array_element_begins_its_own_alignment_block() {
    let idl = " \
        @final struct Cell { long a; short b; }; \
        @final struct Sample { Cell items[2]; octet tail; };";
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        let source = codec(idl, &["Sample"], hdds(encoding));
        assert!(
            source.contains("w.begin_packed_element()"),
            "{encoding:?}: the encoder does not open a packed element:\n{source}"
        );
        assert!(
            source.contains("w.finish_packed_element("),
            "{encoding:?}: the encoder does not close a packed element:\n{source}"
        );
        assert!(
            source.contains("r.begin_packed_element()"),
            "{encoding:?}: the decoder does not open a packed element:\n{source}"
        );
        assert!(
            source.contains("r.finish_packed_element("),
            "{encoding:?}: the decoder does not close a packed element:\n{source}"
        );
        // An array never carries an element length: only a `sequence`
        // has that axis. A `write_dheader` here would be the wrong one.
        assert!(
            !source.contains("write_dheader"),
            "{encoding:?}: an array element took a length it has no axis for:\n{source}"
        );
    }
}

/// `sequence-struct-appendable` is `@appendable struct Cell { long a;
/// short b; }; @final struct Sample { sequence<Cell> none; sequence<Cell>
/// one; sequence<Cell> many; long tail; };` and its recorded octets are
/// `tests/interop-results/sequence-struct-appendable/hdds/plain-cdr1.le.bin`,
/// byte-identical under cdr2: `00000000 01000000 06000000 7929edff
/// feff0000 …`. The `06000000` in front of each `Cell` is
/// `SequenceElementDelimiter::BeforeEachNonFinalElement`.
#[test]
fn a_sequence_element_of_a_non_final_struct_carries_its_own_length() {
    let idl = " \
        @appendable struct Cell { long a; short b; }; \
        @final struct Sample { sequence<Cell> none; sequence<Cell> one; \
        sequence<Cell> many; long tail; };";
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        let source = codec(idl, &["Sample"], hdds(encoding));
        assert!(
            source.contains("w.write_dheader()"),
            "{encoding:?}: the encoder does not open an element length:\n{source}"
        );
        assert!(
            source.contains("w.finish_dheader("),
            "{encoding:?}: the encoder does not patch an element length:\n{source}"
        );
        assert!(
            source.contains("r.read_u32()"),
            "{encoding:?}: the decoder does not read an element length:\n{source}"
        );
        assert!(
            source.contains("begin_packed_element()"),
            "{encoding:?}: a sequence element is not packed:\n{source}"
        );
    }
}

/// `wstring-member` is `module m { @appendable struct S { wstring w; };
/// };` and its recorded octets are
/// `tests/interop-results/wstring-member/hdds/plain-cdr1.le.bin`:
/// `04 00 00 00 61 62 63 00` for `w = "abc"`. `WstringCodeUnits::Utf8`
/// writes the characters' own octets, three plus the terminator the cell
/// counts.
#[test]
fn a_wstring_is_written_and_read_as_utf8() {
    let idl = "module m { @appendable struct S { wstring w; }; };";
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        let source = codec(idl, &["m", "S"], hdds(encoding));
        assert!(
            source.contains("write_wstring_utf8"),
            "{encoding:?}: the wstring write is not UTF-8:\n{source}"
        );
        assert!(
            source.contains("read_wstring_utf8"),
            "{encoding:?}: the wstring read is not UTF-8:\n{source}"
        );
        // The UTF-16 entry point takes a unit order and a cell; the
        // UTF-8 one takes neither unit width nor order, so naming it is
        // the whole of what distinguishes the two.
        assert!(
            !source.contains("write_wstring("),
            "{encoding:?}: the wstring also emitted the UTF-16 write:\n{source}"
        );
    }
}

/// The axes do not leak into the specification's codec: the same types
/// under `Dialect::Specification` name none of these functions, so a
/// change that applied the axes unconditionally would fail here.
#[test]
fn the_specification_key_emits_none_of_the_three_axes() {
    let idl = " \
        @final struct Cell { long a; short b; }; \
        @final struct Sample { Cell items[2]; sequence<Cell> cells; wstring w; };";
    let key = LayoutKey::in_dialect(
        Dialect::Specification,
        Encoding::PlainCdr1,
        Endianness::Little,
        Purpose::Data,
    );
    let source = codec(idl, &["Sample"], key);
    for absent in [
        "begin_packed_element",
        "write_dheader",
        "write_wstring_utf8",
        "read_wstring_utf8",
    ] {
        assert!(
            !source.contains(absent),
            "the specification's codec names `{absent}`:\n{source}"
        );
    }
}
