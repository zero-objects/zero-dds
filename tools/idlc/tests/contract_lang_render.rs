// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-idlc` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-lang-render` (`src/generate.rs`, the
//! `*_support.rs` files): the renderers of every column, through which the
//! files `generate` writes are text, and `cpp::decl`/`cpp::type_` for the
//! C++ topic trait the tool appends to a header. It relies on every
//! rendered file ending in exactly one newline with no trailing whitespace
//! on a line (the tool concatenates rendered parts and writes the result
//! unchanged), and on a C++ string literal escaping the characters C++
//! reads as escapes.
//!
//! Oracle: ISO C++17 [lex.string] and [lex.ccon] (a backslash and a quote
//! inside a string literal are written `\\` and `\"`); POSIX.1-2017 §3.206
//! (a text file's lines end in a newline).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

#[test]
fn every_file_the_tool_writes_ends_in_one_newline_and_no_line_has_trailing_space() {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        "module m { struct S { long a; string b; }; enum E { A, B }; };",
        &zerodds_idl_compose::PathLoader::bare(),
    );
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    for language in Language::ALL {
        let text = zerodds_lang_rules::generate::source(&composed.ast, language, key).unwrap();
        assert!(
            text.ends_with('\n') && !text.ends_with("\n\n"),
            "{language:?}"
        );
        for (number, line) in text.lines().enumerate() {
            assert_eq!(line, line.trim_end(), "{language:?} line {}", number + 1);
        }
    }
}

#[test]
fn a_cpp_string_literal_escapes_backslash_and_quote() {
    let literal = zerodds_lang_render::cpp::expr(&zerodds_lang_cst::cpp::Expr::Literal(
        zerodds_lang_cst::common::Literal::Str("a\\b\"c".into()),
    ));
    assert_eq!(literal, "\"a\\\\b\\\"c\"");
}
