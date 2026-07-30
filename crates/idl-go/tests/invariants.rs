// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the generated Go (no `go` needed).
//!
//! These guard the whole class of defects the IDL-construct fix campaign found
//! in the thin backends, checked purely on the emitted source string:
//!
//! - **No duplicate top-level symbol** — every `type X`, top-level `func X`
//!   (non-method) and single-line `const X` name is unique. Catches the
//!   non-injective module flatten (#A35), interface-nested promotion (#A39),
//!   struct-inheritance duplication (#A10) and cross-member folding (#A41).
//! - **No unescaped reserved word** as a generated identifier (Go keyword as a
//!   type / field / const name).
//! - **No non-compile literal**: a bare `TRUE`/`FALSE` token (#A13) or an
//!   `L"…"`/`L'…'` wide-literal prefix (#A7 family) in a `const` value.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::HashSet;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

/// The 25 Go keywords (Go Language Specification, "Keywords").
const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_go_module(&ast, &GoGenOptions::default()).expect("gen")
}

/// Collects the top-level declared symbol names: `type <name>`, top-level
/// `func <name>(` (NOT a method `func (recv) …`), and single-line
/// `const <NAME> …` (a `const (` block holds enum members, which are unique by
/// construction and skipped here).
fn top_level_symbols(go: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in go.lines() {
        if let Some(rest) = line.strip_prefix("type ") {
            names.push(rest.split_whitespace().next().unwrap_or("").to_string());
        } else if let Some(rest) = line.strip_prefix("func ") {
            // Skip methods: `func (v T) Name(...)`.
            if !rest.starts_with('(') {
                let name = rest.split('(').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    names.push(name);
                }
            }
        } else if let Some(rest) = line.strip_prefix("const ") {
            // Single-line const only (a `const (` group has no name here).
            if !rest.starts_with('(') {
                names.push(rest.split_whitespace().next().unwrap_or("").to_string());
            }
        }
    }
    names
}

fn assert_no_duplicate_top_level(go: &str) {
    let names = top_level_symbols(go);
    let mut seen = HashSet::new();
    for n in &names {
        assert!(
            seen.insert(n.clone()),
            "duplicate top-level symbol `{n}` in:\n{go}"
        );
    }
}

fn struct_field_names(go: &str) -> Vec<String> {
    // Field names are the first token of each `\t<name> <type>` line inside a
    // `type X struct { … }` block (struct bodies here are flat — no nested
    // braces). Comment/verbatim lines (`\t//`) are skipped.
    let mut fields = Vec::new();
    let mut in_struct = false;
    for line in go.lines() {
        if line.starts_with("type ") && line.trim_end().ends_with("struct {") {
            in_struct = true;
            continue;
        }
        if in_struct {
            if line.starts_with('}') {
                in_struct = false;
                continue;
            }
            let t = line.trim_start();
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            if let Some(name) = t.split_whitespace().next() {
                fields.push(name.to_string());
            }
        }
    }
    fields
}

fn assert_no_reserved_identifiers(go: &str) {
    // A generated field / type / const identifier must never be a bare Go
    // keyword.
    for f in struct_field_names(go) {
        assert!(
            !GO_KEYWORDS.contains(&f.as_str()),
            "reserved word `{f}` used as a struct field name in:\n{go}"
        );
    }
    for n in top_level_symbols(go) {
        assert!(
            !GO_KEYWORDS.contains(&n.as_str()),
            "reserved word `{n}` used as a top-level symbol in:\n{go}"
        );
    }
}

fn assert_no_noncompile_literals(go: &str) {
    // A `const` value must never carry a bare TRUE/FALSE token or an L-prefixed
    // wide literal (both invalid Go).
    for line in go.lines() {
        if let Some(rest) = line.strip_prefix("const ") {
            assert!(
                !rest.contains(" TRUE") && !rest.contains("=TRUE") && !rest.contains(" FALSE"),
                "TRUE/FALSE literal leaked into a const:\n{line}"
            );
            assert!(
                !rest.contains("L\"") && !rest.contains("L'"),
                "L-prefixed wide literal leaked into a const:\n{line}"
            );
        }
    }
}

fn check_all(go: &str) {
    assert_no_duplicate_top_level(go);
    assert_no_reserved_identifiers(go);
    assert_no_noncompile_literals(go);
}

#[test]
fn module_underscore_flatten_is_injective() {
    // #A35: `module A_B { struct C }` and `module A { module B { struct C }}`
    // must NOT both flatten to `type A_B_C`.
    let go = emit(
        "module A_B { struct C { long x; }; };
         module A { module B { struct C { double y; }; }; };",
    );
    check_all(&go);
    assert!(go.contains("type A__B_C struct"), "{go}");
    assert!(go.contains("type A_B_C struct"), "{go}");
}

#[test]
fn struct_inheritance_no_duplicate_and_carries_base_fields() {
    // #A10: base fields are inlined base-first; no duplicate `type`.
    let go = emit(
        "@final struct Base { long a; long b; };
         @final struct Derived : Base { long c; };",
    );
    check_all(&go);
    assert!(go.contains("type Derived struct"), "{go}");
    // Derived carries A, B (inherited) then C.
    let der = go
        .split("type Derived struct {")
        .nth(1)
        .expect("Derived body");
    let body = der.split('}').next().unwrap();
    assert!(body.contains("\tA int32"), "{go}");
    assert!(body.contains("\tB int32"), "{go}");
    assert!(body.contains("\tC int32"), "{go}");
}

#[test]
fn const_all_types_emit_valid_go_no_bad_literals() {
    // #A5/P1 + #A13/#A7: every const type emits, booleans normalized, no L".
    let go = emit(
        "const long SHIFT = 1 << 4;
         const boolean FLAG = TRUE;
         const boolean OFF = FALSE;
         const string GREETING = \"hi\";
         const double PI = 3.14;
         const octet BYTE = 7;
         const char CH = 'x';
         const wstring WS = L\"hello\";
         const wchar WC = L'y';",
    );
    check_all(&go);
    assert!(go.contains("const SHIFT int32 = (1 << 4)"), "{go}");
    assert!(go.contains("const FLAG bool = true"), "{go}");
    assert!(go.contains("const OFF bool = false"), "{go}");
    assert!(go.contains("const GREETING string = \"hi\""), "{go}");
    assert!(go.contains("const WS string = \"hello\""), "{go}");
    assert!(go.contains("const WC rune = 'y'"), "{go}");
}

#[test]
fn union_enum_char_bool_discriminators_do_not_abort() {
    // #A11/A12/A13/P4: enum / char / boolean labels resolve, no early error.
    let go = emit(
        "enum Color { RED, GREEN, BLUE };
         @final union EU switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
         @final union CU switch (char) { case 'A': long a; case 'B': short b; };
         @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };",
    );
    check_all(&go);
    // Enum labels resolve to ordinals.
    assert!(go.contains("case 0:") && go.contains("case 1:"), "{go}");
    // Char label 'A' -> 65.
    assert!(go.contains("case 65:"), "{go}");
    // Boolean labels -> Go true/false, not integers.
    assert!(
        go.contains("case true:") && go.contains("case false:"),
        "{go}"
    );
}

#[test]
fn interface_nested_types_survive() {
    // #A39: the interface body's nested struct must be emitted, not dropped.
    let go = emit(
        "interface Calculator { struct Config { long precision; }; long add(in long a, in long b); };",
    );
    check_all(&go);
    assert!(go.contains("type Calculator_Config struct"), "{go}");
    assert!(go.contains("\tPrecision int32"), "{go}");
}

#[test]
fn member_name_folding_dedups() {
    // #A41: `foo`/`Foo` both fold to the exported `Foo` (Go capitalizes the
    // first letter); the second is de-duplicated to `Foo_2`.
    let go = emit("@final struct S { long foo; long Foo; };");
    check_all(&go);
    assert!(go.contains("\tFoo int32"), "{go}");
    assert!(go.contains("\tFoo_2 int32"), "{go}");
}

#[test]
fn nested_sequence_decode_indices_are_depth_unique() {
    // #A21/P9: the nested sequence/map decoders must not reuse `i`/`zdn`.
    let go = emit(
        "module M { struct S { sequence<sequence<long> > v; }; };
         module N { struct T { map<string, map<string, long> > w; }; };",
    );
    check_all(&go);
    // Distinct loop indices per depth (zi0 outer, zi1 inner) — never a bare
    // `for i :=` in the collection decoders.
    assert!(
        go.contains("for zi0 :=") && go.contains("for zi1 :="),
        "{go}"
    );
    // Distinct map temporaries per depth.
    assert!(go.contains("zk0") && go.contains("zk1"), "{go}");
}

#[test]
fn keyword_named_members_are_escaped_by_export() {
    // Go keywords as IDL member names (that are not themselves IDL keywords):
    // exported() capitalizes, dodging every (lowercase) Go keyword — no bare
    // reserved identifier survives.
    let go = emit("@final struct S { long defer; long chan; long fallthrough; long goto; };");
    check_all(&go);
    assert!(go.contains("\tDefer int32"), "{go}");
    assert!(go.contains("\tChan int32"), "{go}");
    assert!(go.contains("\tFallthrough int32"), "{go}");
    assert!(go.contains("\tGoto int32"), "{go}");
}
