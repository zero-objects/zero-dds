// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A run of octets in the C++ column is one copy, not one call per octet.
//!
//! `sequence<octet>` and a one-dimensional `octet[N]` (and the same over
//! `uint8`, directly or through a typedef) write through
//! `Writer::write_octets` and read through `Reader::read_octets`. An octet
//! has alignment 1 and no byte order (XTypes 1.3 §7.4.3.4.1), so the
//! bytes are the loop's; what the copy removes is the call per element.
//!
//! Two consequences are held here. A multi-dimensional array and an
//! `int8` element keep the loop. And an octet codec only a copy would
//! have called is not emitted: clang reports an uncalled `static`
//! function in a header under `-Wunused-function`, so a header that
//! carried one would fail every consumer that builds with `-Werror`.
//! The header `idlc` writes is compiled and run under clang in
//! `tools/idlc/tests/cpp_octet_runs.rs`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::generate;

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

/// The C++ column's whole text for `idl`, under XCDR2 and XCDR1.
fn cpp(idl: &str) -> Vec<String> {
    let ast = model(idl);
    [Encoding::PlainCdr2, Encoding::PlainCdr1]
        .into_iter()
        .map(|encoding| {
            generate::source(
                &ast,
                Language::Cpp,
                LayoutKey::data(encoding, Endianness::Little),
            )
            .unwrap_or_else(|error| panic!("{encoding:?}: C++ refused the source: {error}"))
        })
        .collect()
}

const ONLY_RUNS: &str = "\
typedef octet Byte;
@final struct OnlyRuns {
    sequence<octet, 64> a;
    octet b[4];
    sequence<Byte> c;
    uint8 d[3];
};
";

#[test]
fn a_sequence_and_a_one_dimensional_array_of_octets_are_copied_in_one_piece() {
    for text in cpp(ONLY_RUNS) {
        assert!(
            text.contains("w.write_octets(v.data(), v.size());"),
            "{text}"
        );
        assert!(text.contains("w.write_octets(v.data(), 4);"), "{text}");
        assert!(text.contains("w.write_octets(v.data(), 3);"), "{text}");
        assert!(text.contains("r.read_octets(v.data(), zd_n);"), "{text}");
        assert!(text.contains("r.read_octets(v.data(), 4u);"), "{text}");
        assert!(text.contains("r.read_octets(v.data(), 3u);"), "{text}");
    }
}

#[test]
fn an_octet_codec_only_a_copy_would_call_is_not_emitted() {
    for text in cpp(ONLY_RUNS) {
        for uncalled in [
            "zd_enc_p_octet",
            "zd_dec_p_octet",
            "zd_enc_p_uint8",
            "zd_dec_p_uint8",
            "zd_enc_Byte",
            "zd_dec_Byte",
        ] {
            assert!(!text.contains(uncalled), "{uncalled} in:\n{text}");
        }
    }
}

#[test]
fn an_octet_member_beside_a_run_keeps_its_codec() {
    let idl = "@final struct Mixed { sequence<octet> a; octet single; };";
    for text in cpp(idl) {
        assert!(
            text.contains("w.write_octets(v.data(), v.size());"),
            "{text}"
        );
        assert!(text.contains("zd_enc_p_octet(w, v.single);"), "{text}");
        assert!(text.contains("zd_dec_p_octet(r, v.single"), "{text}");
    }
}

#[test]
fn a_grid_of_octets_and_a_run_of_int8_keep_the_loop() {
    let idl = "@final struct Grid { octet g[2][3]; sequence<int8> s; };";
    for text in cpp(idl) {
        assert!(!text.contains("write_octets"), "{text}");
        assert!(!text.contains("read_octets"), "{text}");
        assert!(text.contains("zd_enc_p_octet(w, v[i0][i1]);"), "{text}");
        assert!(text.contains("zd_enc_p_int8(w, v[i]);"), "{text}");
    }
}
