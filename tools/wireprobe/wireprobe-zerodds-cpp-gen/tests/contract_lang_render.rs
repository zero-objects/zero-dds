// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-cpp-gen` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`src/lib.rs`, `src/fill.rs`,
//! `src/dump.rs`): the C++ renderer, whose text is the program the C++
//! wire probe compiles. It relies on the rendered
//! file ending in exactly one newline with no trailing whitespace (the
//! generator appends its harness to it).
//!
//! Oracle: POSIX.1-2017 §3.206 (a line of a text file ends in a newline).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

#[test]
fn the_rendered_file_ends_in_one_newline_and_no_line_carries_trailing_whitespace() {
    let parsed = zerodds_idl_parse::parse(
        "module m { @appendable struct S { long a; string b; sequence<double> c; }; enum E { A, B }; };",
    );
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "contract.idl").ast;
    ast.resolve_all_by_name().unwrap();
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let text = zerodds_lang_rules::generate::source(&ast, Language::Cpp, key).unwrap();
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
    for (number, line) in text.lines().enumerate() {
        assert_eq!(line, line.trim_end(), "line {}", number + 1);
    }
}
