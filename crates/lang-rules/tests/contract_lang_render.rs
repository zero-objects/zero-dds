// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-lang-rules` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render`: the renderer of every
//! column (`<language>::file` and its parts) for the text
//! `generate::source` returns, and the literal helpers the materialisers
//! and codec backends call to write a value the target reads back as the
//! same value: `c::integer_text`, `cpp::integer_text`,
//! `kotlin::long_text`. It relies on every rendered
//! file ending in exactly one newline with no trailing whitespace (the
//! generator concatenates rendered parts), and on the literal helpers
//! spelling the extremes of a type in a form the language accepts.
//!
//! Oracle: ISO C11 §6.4.4.1 and ISO C++17 [lex.icon] (an integer literal
//! has no sign; `9223372036854775808` fits no signed type, so the minimum
//! `long long` is an expression), the Kotlin language specification
//! ("Integer literals": a `Long` literal is suffixed `L`, and
//! `-9223372036854775808L` is out of range, `Long.MIN_VALUE` names the
//! value); POSIX.1-2017 §3.206 (a line ends in a newline).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

#[test]
fn the_minimum_long_long_is_an_expression_in_c_and_cpp_and_a_constant_in_kotlin() {
    assert_eq!(
        zerodds_lang_render::c::integer_text(i128::from(i64::MIN)),
        "(-9223372036854775807LL - 1)"
    );
    assert_eq!(
        zerodds_lang_render::cpp::integer_text(i128::from(i64::MIN)),
        "(-9223372036854775807LL - 1)"
    );
    assert_eq!(
        zerodds_lang_render::kotlin::long_text(i64::MIN),
        "Long.MIN_VALUE"
    );
    assert_eq!(zerodds_lang_render::kotlin::long_text(-1), "-1L");
}

#[test]
fn every_column_renders_a_file_that_ends_in_one_newline_without_trailing_space() {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        "module m { @appendable struct S { long a; string b; sequence<double> c; }; enum E { A, B }; };",
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
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
