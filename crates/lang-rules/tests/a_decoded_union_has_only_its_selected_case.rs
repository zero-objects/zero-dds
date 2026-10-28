// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A decoded union carries the case its discriminator selects, and no
//! other, in every column whose case fields have a "not set".
//!
//! The decoder starts from the union's Table 9 default, which holds the
//! member the default discriminator selects. Where an unselected case is
//! a field holding `None`, `nil`, `nothing`, an empty `Option` or a
//! property or key that is not there, the decoder has to clear that
//! member before it reads the selected one — otherwise the sample comes
//! back with two cases live. Measured in `idl-conformance/_interop`:
//! Python's `Sel` read with `_d = 2` carried `n = 9` and `z = 0`, and
//! TypeScript's `combo::Reading` read as `MODE_ACTIVE` carried
//! `idleTicks: 0` beside `activeRate`.
//!
//! `Sel`'s default discriminator is 0, which no label names, so the
//! default is the `default:` case `z`; a sample read with `_d = 1` must
//! not keep it.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_lang_cst::Language;

fn union_ast() -> zerodds_ast::Ast {
    let source =
        "@final union Sel switch (long) { case 1: long p; case 2: long n; default: octet z; };";
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "sel.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// Per column: where the union's decoder begins, the statement that
/// clears a case (`{}` is the case's name), and the statement that reads
/// case `p`.
const COLUMNS: [(Language, &str, &str, &str); 9] = [
    (
        Language::Python,
        "def zd_dec_Sel(",
        "v.{} = None",
        "v.p = zd_dec_",
    ),
    (
        Language::TypeScript,
        "function zd_dec_Sel(",
        "delete (v as { {}?: unknown }).{};",
        ".p = zd_dec_",
    ),
    (
        Language::Node,
        "function zd_dec_Sel(",
        "delete v.{};",
        "v.p = zd_dec_",
    ),
    (
        Language::Julia,
        "function zd_dec_Sel(",
        "v.{} = nothing",
        "v.p = zd_dec_",
    ),
    (
        Language::Swift,
        "func zd_dec_Sel(",
        "v.{} = nil",
        "v.p = try zd_dec_",
    ),
    (
        Language::Nim,
        "proc zd_dec_Sel*(",
        "reset(v.{})",
        "v.p = some(",
    ),
    (
        Language::OCaml,
        "zd_dec_sel (r",
        "v := { !v with {} = None };",
        "p = Some (",
    ),
    (
        Language::Lua,
        "function ZD.zd_dec_Sel(",
        "v.{} = nil",
        "v.p = ZD.zd_dec_",
    ),
    (
        Language::Elixir,
        "def zd_dec_Sel(",
        "v = %{v | {}: nil}",
        "v = %{v | p: zd_rd",
    ),
];

#[test]
fn every_case_is_cleared_before_the_selected_one_is_read() {
    let ast = union_ast();
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    for (language, begins, clear, reads_p) in COLUMNS {
        let text = zerodds_lang_rules::generate::source(&ast, language, key)
            .unwrap_or_else(|error| panic!("{language:?}: {error}"));
        // The last occurrence: Nim declares the procedure ahead of its
        // body.
        let start = text
            .rfind(begins)
            .unwrap_or_else(|| panic!("{language:?}: no `{begins}` in:\n{text}"));
        let decoder = &text[start..];
        let read = decoder
            .find(reads_p)
            .unwrap_or_else(|| panic!("{language:?}: case `p` is not read:\n{decoder}"));
        for case in ["p", "n", "z"] {
            let statement = clear.replace("{}", case);
            let at = decoder.find(&statement).unwrap_or_else(|| {
                panic!("{language:?}: case `{case}` is not cleared (`{statement}`):\n{decoder}")
            });
            assert!(
                at < read,
                "{language:?}: case `{case}` is cleared after case `p` is read:\n{decoder}"
            );
        }
    }
}
