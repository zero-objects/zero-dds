// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants over the Python codegen output.
//!
//! These assert properties of the emitted source string WITHOUT a Python
//! interpreter, so they run everywhere `cargo test` runs. They guard the
//! IDL-construct fix campaign findings for `idl-python`:
//!
//! * F35/A35 — the module-flatten must be injective: two distinct IDL scopes
//!   never produce the same top-level Python symbol (a naive `join("_")`
//!   collides `A::B_C` with `A::B::C`).
//! * F7/A7   — a `wstring`/`wchar` const must not leak the IDL `L"…"`/`L'…'`
//!   prefix (a Python syntax error).
//! * F13/A13 — a `boolean`-switch union must render `True`/`False`, never the
//!   IDL `TRUE`/`FALSE` tokens (undefined names in Python).
//! * reserved-word safety — no emitted identifier is a bare Python keyword.
//! * no empty class body — every `class` carries a body (`pass` or fields).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_python::{PythonGenOptions, generate_python_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_python_module(&ast, &PythonGenOptions::default()).expect("gen")
}

/// The full set of Python 3 hard keywords plus the `match`/`case` soft
/// keywords — the emitter escapes any of these to `<name>_`.
const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield", "match", "case",
];

/// Collects the identifiers bound at module top level (column 0): `class X`,
/// `X: TypeAlias`, and `X = …`. Attribute assignments (`X.TYPE_OBJECT = …`,
/// `X._idl_bit_bound = …`) are NOT new bindings and are excluded.
fn top_level_symbols(py: &str) -> Vec<String> {
    let mut syms = Vec::new();
    for line in py.lines() {
        if line.starts_with(char::is_whitespace) || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                syms.push(name);
            }
            continue;
        }
        // `NAME = …` / `NAME: …` — but not `NAME.attr = …`.
        if let Some(eq) = line.find([':', '=']) {
            let head = line[..eq].trim();
            if !head.is_empty()
                && head.chars().all(|c| c.is_alphanumeric() || c == '_')
                && !head.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                syms.push(head.to_string());
            }
        }
    }
    syms
}

/// Splits off the leading Python identifier of `s`, returning it and the tail.
fn split_ident(s: &str) -> (String, &str) {
    let end = s
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    (s[..end].to_string(), &s[end..])
}

/// Every *bound* identifier — a class name, a top-level/dataclass/enum binding
/// (`name: T` or `name = v`) — must not be a bare Python keyword. Import
/// statements and function-call kwargs (`typename=…`, no surrounding spaces)
/// are not bindings and are skipped.
fn assert_no_bare_keyword(py: &str) {
    for line in py.lines() {
        let indent = line.len() - line.trim_start().len();
        if indent != 0 && indent != 4 {
            continue; // only column-0 statements and 4-space class-body members
        }
        let rest = line.trim_start();
        let ident = if let Some(after) = rest.strip_prefix("class ") {
            split_ident(after).0
        } else {
            let (ident, tail) = split_ident(rest);
            if ident.is_empty() {
                continue;
            }
            // A binding is `name:` (annotation) or `name = ` (assignment with
            // surrounding spaces). A kwarg `name=value` has no spaces → skipped.
            if !(tail.starts_with(':') || tail.starts_with(" = ")) {
                continue;
            }
            ident
        };
        assert!(
            !PYTHON_KEYWORDS.contains(&ident.as_str()),
            "bare Python keyword `{ident}` used as identifier:\n{line}\n---\n{py}"
        );
    }
}

/// A `class` header is always followed by an indented body line (`pass` or a
/// field) — never an empty body (the cross-backend "leere record/{}" pattern).
fn assert_no_empty_class_body(py: &str) {
    let lines: Vec<&str> = py.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("class ") && line.trim_end().ends_with(':') {
            let next = lines
                .iter()
                .skip(i + 1)
                .find(|l| !l.trim().is_empty())
                .copied()
                .unwrap_or("");
            assert!(
                next.starts_with(char::is_whitespace) && !next.trim().is_empty(),
                "class with empty body at line {i}:\n{line}\n---\n{py}"
            );
        }
    }
}

/// Runs every structural invariant over one emitted module.
fn assert_well_formed(py: &str) {
    // F7/A7: no leaked IDL wide-literal prefix.
    assert!(
        !py.contains("L\""),
        "leaked `L\"` wide-string prefix:\n{py}"
    );
    assert!(!py.contains("L'"), "leaked `L'` wide-char prefix:\n{py}");
    // F13/A13: no bare IDL boolean tokens.
    assert!(!py.contains("TRUE"), "leaked IDL `TRUE` token:\n{py}");
    assert!(!py.contains("FALSE"), "leaked IDL `FALSE` token:\n{py}");
    // generic-array / non-brand junk should never appear.
    assert!(
        !py.contains("[]::"),
        "leaked generic-array construct:\n{py}"
    );
    assert_no_bare_keyword(py);
    assert_no_empty_class_body(py);
    // No duplicated top-level binding (F35/A35 flatten collision).
    let mut syms = top_level_symbols(py);
    syms.sort();
    let before = syms.len();
    syms.dedup();
    assert_eq!(
        before,
        syms.len(),
        "duplicate top-level symbol (flatten collision):\n{py}"
    );
}

#[test]
fn flatten_is_injective_across_nested_and_underscore_scopes() {
    // `A::B_C` and `A::B::C` both flatten to `A_B_C` under a naive `join("_")`,
    // so the second class silently rebinds the first — one type lost (F35/A35).
    // The injective flatten must keep them distinct.
    let py = emit(
        "module A { \
            struct B_C { long x; }; \
            module B { struct C { long y; }; }; \
        };",
    );
    assert!(py.contains("class A_B__C:"), "{py}");
    assert!(py.contains("class A_B_C:"), "{py}");
    assert_well_formed(&py);
}

#[test]
fn deeper_flatten_collision_stays_distinct() {
    // `A::B::C_D`, `A::B_C::D`, `A_B::C::D` — three distinct scopes that a naive
    // flatten collapses to `A_B_C_D`.
    let py = emit(
        "module A { module B { struct C_D { long a; }; }; }; \
         module A { module B_C { struct D { long b; }; }; }; \
         module A_B { module C { struct D { long c; }; }; };",
    );
    let mut classes: Vec<&str> = py.lines().filter(|l| l.starts_with("class ")).collect();
    classes.sort_unstable();
    classes.dedup();
    assert_eq!(classes.len(), 3, "expected 3 distinct classes:\n{py}");
    assert_well_formed(&py);
}

#[test]
fn wide_string_and_wchar_consts_drop_the_l_prefix() {
    // F7/A7: `L"…"` / `L'…'` are IDL wide-literal syntax; Python has no such
    // prefix, so it must be stripped.
    let py = emit("const wstring W = L\"hi\"; const wchar C = L'x';");
    assert!(py.contains("W = \"hi\""), "{py}");
    assert!(py.contains("C = 'x'"), "{py}");
    assert_well_formed(&py);
}

#[test]
fn boolean_union_labels_render_python_bools_not_idl_tokens() {
    // F13/A13: a `boolean`-switch union must key on `1`/`0`, never `TRUE`/`FALSE`.
    let py = emit("union B switch(boolean){ case TRUE: long y; case FALSE: long n; };");
    assert!(py.contains("1: (\"y\""), "{py}");
    assert!(py.contains("0: (\"n\""), "{py}");
    assert_well_formed(&py);
}

#[test]
fn char_union_labels_render_code_points() {
    // F12/A12: a `char`-switch union keys on the character code point.
    let py =
        emit("union U switch(char){ case 'A': long a; case '\\n': long b; default: long c; };");
    assert!(py.contains("65: (\"a\""), "{py}"); // 'A'
    assert!(py.contains("10: (\"b\""), "{py}"); // '\n'
    assert_well_formed(&py);
}

#[test]
fn enum_value_annotation_is_honored() {
    // F4/A4: `@value(N)` sets the enumerator's wire integer; an un-annotated
    // follower continues from the annotated value + 1.
    let py = emit("enum Color { RED, @value(5) GREEN, BLUE };");
    assert!(py.contains("RED = 0"), "{py}");
    assert!(py.contains("GREEN = 5"), "{py}");
    assert!(py.contains("BLUE = 6"), "{py}");
    assert_well_formed(&py);
}

#[test]
fn enum_switch_union_labels_track_value_annotation() {
    // The union `case` label for a `@value`-gapped enumerator must resolve to
    // the same integer the IntEnum member carries (F4 consistency).
    let py = emit("enum K { K_A, @value(10) K_B }; union UK switch(K){ case K_B: long v; };");
    assert!(py.contains("K_B = 10"), "{py}");
    assert!(py.contains("10: (\"v\""), "{py}");
    assert_well_formed(&py);
}

#[test]
fn python_keyword_identifiers_are_escaped_at_every_position() {
    // Reserved words as IDL identifiers at struct/enum-member/member positions
    // must be escaped so the module is syntactically valid Python.
    let py = emit(
        "enum E { None, True, False }; \
         struct None_holder { long def; boolean lambda; long class; };",
    );
    assert!(py.contains("None_ = 0"), "{py}");
    assert!(py.contains("True_ = 1"), "{py}");
    assert!(py.contains("def_:"), "{py}");
    assert!(py.contains("lambda_:"), "{py}");
    assert!(py.contains("class_:"), "{py}");
    assert_well_formed(&py);
}

#[test]
fn representative_construct_spread_is_well_formed() {
    let py = emit(
        "module m { \
            enum Mode { A, @value(3) B, C }; \
            struct Inner { long x; wchar w; }; \
            struct Outer { Inner nested; sequence<long, 4> items; string<8> name; }; \
            union V switch(long){ case 0: long i; case 1: string s; default: octet o; }; \
            const boolean ENABLED = TRUE; \
            const wstring TAG = L\"tag\"; \
        };",
    );
    assert_well_formed(&py);
}
