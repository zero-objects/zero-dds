// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-c-gen` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`src/lib.rs`, `src/fill.rs`,
//! `src/dump.rs`): `c::file` and `c::decl` for the program the C wire probe
//! compiles, and `c::integer_text` for the integer values of a sample. It
//! relies on the rendered file ending in exactly one newline with no
//! trailing whitespace, and on an integer value arriving as a C expression
//! of that value.
//!
//! Oracle: ISO C11 §6.4.4.1 (an integer constant has no sign and
//! `9223372036854775808` fits no signed type, so the minimum `long long` is
//! an expression) and POSIX.1-2017 §3.206 (a line of a text file ends in a
//! newline).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

#[test]
fn the_rendered_file_ends_in_one_newline_and_no_line_carries_trailing_whitespace() {
    let ast = zerodds_wire_probe::encode::model_from_idl(
        "module m { @appendable struct S { long a; string b; sequence<double> c; }; enum E { A, B }; };",
        "contract.idl",
    )
    .unwrap();
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let text = zerodds_lang_rules::generate::source(&ast, Language::C, key).unwrap();
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
    for (number, line) in text.lines().enumerate() {
        assert_eq!(line, line.trim_end(), "line {}", number + 1);
    }
}

#[test]
fn c11_6_4_4_1_the_minimum_long_long_is_written_as_an_expression() {
    assert_eq!(
        zerodds_lang_render::c::integer_text(i128::from(i64::MIN)),
        "(-9223372036854775807LL - 1)"
    );
    assert_eq!(zerodds_lang_render::c::integer_text(-5), "-5");
}
