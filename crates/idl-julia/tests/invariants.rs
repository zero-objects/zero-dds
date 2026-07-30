// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the generated Julia (no `julia`
//! needed).
//!
//! These guard the whole class of defects the IDL-construct fix campaign found
//! in the thin backends, checked purely on the emitted source string:
//!
//! - **No duplicate top-level symbol** — every `struct X` / `mutable struct X`
//!   / `@enum X` type name and single-line `const X` name is unique. Catches
//!   the non-injective module flatten (#A35), interface-nested promotion
//!   (#A39), and struct-inheritance duplication (#A10). (Julia methods share a
//!   name by design — multiple dispatch — so `function` names are NOT symbols.)
//! - **No unescaped reserved word** as a generated identifier (a Julia keyword
//!   used bare as a type / field / const name).
//! - **No non-compile literal** — a bare `TRUE`/`FALSE` token (#A13) or an
//!   `L"…"`/`L'…'` wide-literal prefix (#A7 family) in a `const` value.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::HashSet;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};

/// Julia's reserved keywords (mirrors `keywords::JULIA_RESERVED`).
const JULIA_KEYWORDS: &[&str] = &[
    "baremodule",
    "begin",
    "break",
    "catch",
    "const",
    "continue",
    "do",
    "else",
    "elseif",
    "end",
    "export",
    "false",
    "finally",
    "for",
    "function",
    "global",
    "if",
    "import",
    "let",
    "local",
    "macro",
    "module",
    "quote",
    "return",
    "struct",
    "true",
    "try",
    "using",
    "while",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

/// Collects the top-level declared TYPE and single-line `const` names:
/// `struct <name>`, `mutable struct <name>`, `@enum <name> …` and
/// `const <name> = …`. Function definitions are deliberately excluded — Julia
/// dispatches `marshal_into!`/`read_*` on argument type, so a repeated name is
/// valid, not a collision.
fn top_level_symbols(jl: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in jl.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("mutable struct ") {
            names.push(first_token(rest));
        } else if let Some(rest) = t.strip_prefix("struct ") {
            names.push(first_token(rest));
        } else if let Some(rest) = t.strip_prefix("@enum ") {
            names.push(first_token(rest));
        } else if let Some(rest) = t.strip_prefix("const ") {
            names.push(first_token(rest));
        }
    }
    names
}

/// The declaration name at the head of `rest`, cut at the first whitespace,
/// `=`, or `::` (so `const X = 1` and `X::T` both yield `X`).
fn first_token(rest: &str) -> String {
    rest.split([' ', '=', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn assert_no_duplicate_top_level(jl: &str) {
    let mut seen = HashSet::new();
    for n in top_level_symbols(jl) {
        assert!(
            seen.insert(n.clone()),
            "duplicate top-level symbol `{n}` in:\n{jl}"
        );
    }
}

/// Field names are the token before `::` on each `    <name>::<type>` line
/// inside a `struct`/`mutable struct` block (bodies here are flat — no nested
/// braces). Comment/verbatim lines are skipped.
fn struct_field_names(jl: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_struct = false;
    for line in jl.lines() {
        let t = line.trim_start();
        if t.starts_with("struct ") || t.starts_with("mutable struct ") {
            in_struct = true;
            continue;
        }
        if in_struct {
            if t == "end" {
                in_struct = false;
                continue;
            }
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if let Some((name, _)) = t.split_once("::") {
                fields.push(name.trim().to_string());
            }
        }
    }
    fields
}

fn assert_no_reserved_identifiers(jl: &str) {
    for f in struct_field_names(jl) {
        assert!(
            !JULIA_KEYWORDS.contains(&f.as_str()),
            "reserved word `{f}` used as a struct field name in:\n{jl}"
        );
    }
    for n in top_level_symbols(jl) {
        assert!(
            !JULIA_KEYWORDS.contains(&n.as_str()),
            "reserved word `{n}` used as a top-level symbol in:\n{jl}"
        );
    }
}

fn assert_no_noncompile_literals(jl: &str) {
    for line in jl.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("const ") {
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

fn check_all(jl: &str) {
    assert_no_duplicate_top_level(jl);
    assert_no_reserved_identifiers(jl);
    assert_no_noncompile_literals(jl);
}

#[test]
fn module_underscore_flatten_is_injective() {
    // #A35: `module A_B { struct C }` and `module A { module B { struct C }}`
    // must NOT both flatten to `struct A_B_C`.
    let jl = emit(
        "module A_B { @final struct C { long x; }; };
         module A { module B { @final struct C { double y; }; }; };",
    );
    check_all(&jl);
    assert!(jl.contains("struct A__B_C"), "{jl}");
    assert!(jl.contains("struct A_B_C"), "{jl}");
}

#[test]
fn struct_inheritance_no_duplicate_and_carries_base_fields() {
    // #A10: base fields are inlined base-first; no duplicate `struct`.
    let jl = emit(
        "@final struct Base { long a; long b; };
         @final struct Derived : Base { long c; };",
    );
    check_all(&jl);
    let der = jl.split("struct Derived\n").nth(1).expect("Derived body");
    let body = der.split("\nend").next().unwrap();
    assert!(body.contains("a::Int32"), "{jl}");
    assert!(body.contains("b::Int32"), "{jl}");
    assert!(body.contains("c::Int32"), "{jl}");
}

#[test]
fn const_all_types_emit_valid_julia_no_bad_literals() {
    // #A5/P1 + #A13/#A7: every const type emits, booleans normalized, no L".
    let jl = emit(
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
    check_all(&jl);
    assert!(jl.contains("const SHIFT = Int32((1 << 4))"), "{jl}");
    assert!(jl.contains("const FLAG = true"), "{jl}");
    assert!(jl.contains("const OFF = false"), "{jl}");
    assert!(jl.contains("const GREETING = \"hi\""), "{jl}");
    assert!(jl.contains("const WS = \"hello\""), "{jl}");
    assert!(jl.contains("const WC = 'y'"), "{jl}");
    // No bare TRUE/FALSE token, no L-prefixed literal anywhere in a const.
    assert!(!jl.contains("= TRUE") && !jl.contains("= FALSE"), "{jl}");
}

#[test]
fn union_enum_char_bool_discriminators_do_not_abort() {
    // #A11/A12/A13/P4: enum / char / boolean labels resolve, no early error,
    // and each label is rendered in the discriminator's own Julia type.
    let jl = emit(
        "enum Color { RED, GREEN, BLUE };
         @final union EU switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
         @final union CU switch (char) { case 'A': long a; case 'B': short b; };
         @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };",
    );
    check_all(&jl);
    // Enum labels resolve to typed enum constructors (ordinals 0 / 1).
    assert!(
        jl.contains("v.disc == Color(Int32(0))") && jl.contains("v.disc == Color(Int32(1))"),
        "{jl}"
    );
    // Char label 'A' -> Char(65), never a bare integer.
    assert!(jl.contains("v.disc == Char(65)"), "{jl}");
    // Boolean labels -> Julia true/false, not integers.
    assert!(
        jl.contains("v.disc == true") && jl.contains("v.disc == false"),
        "{jl}"
    );
}

#[test]
fn mutable_union_emits_emheader_framing() {
    // #A16/F14: a @mutable union used to be rejected outright. It must now emit
    // a DHEADER-framed EMHEADER member list (discriminator = id 0, LC4).
    let jl = emit("@mutable union MutU switch (long) { case 1: long a; default: short b; };");
    check_all(&jl);
    // Discriminator EMHEADER = LC4 | id 0.
    assert!(jl.contains("put_u32!(body, 0x40000000)"), "{jl}");
    // First branch EMHEADER = LC4 | id 1.
    assert!(jl.contains("put_u32!(body, 0x40000001)"), "{jl}");
    // Framed by the struct DHEADER.
    assert!(
        jl.contains("zdBB = bytes(body)") && jl.contains("put_u32!(w, length(zdBB))"),
        "{jl}"
    );
}

#[test]
fn mutable_struct_must_understand_sets_emheader_bit31() {
    // #A17/F17: a @must_understand member sets EMHEADER bit 31; a plain member
    // does not. LC4 stays (#A19 unchanged): id 1 -> 0x40000001, @must_understand
    // id 2 -> 0xc0000002.
    let jl = emit("@mutable struct MutS { @id(1) long x; @must_understand @id(2) string s; };");
    check_all(&jl);
    assert!(jl.contains("put_u32!(body, 0x40000001)"), "{jl}");
    assert!(jl.contains("put_u32!(body, 0xc0000002)"), "{jl}");
}

#[test]
fn interface_nested_types_survive() {
    // #A39/F39: the interface body's nested struct must be emitted, not dropped.
    let jl = emit(
        "interface Calculator { struct Config { long precision; }; long add(in long a, in long b); };",
    );
    check_all(&jl);
    assert!(jl.contains("struct Calculator_Config"), "{jl}");
    assert!(jl.contains("precision::Int32"), "{jl}");
}

#[test]
fn keyword_named_members_and_types_are_escaped() {
    // Julia keywords as IDL identifiers must be escaped (trailing `_`), never
    // emitted bare — members and a top-level type name alike.
    let jl = emit(
        "@final struct S { long begin; long end; long function; long global; long while; };
         @final struct try { long x; };",
    );
    check_all(&jl);
    assert!(jl.contains("begin_::Int32"), "{jl}");
    assert!(jl.contains("end_::Int32"), "{jl}");
    assert!(jl.contains("function_::Int32"), "{jl}");
    assert!(jl.contains("global_::Int32"), "{jl}");
    assert!(jl.contains("while_::Int32"), "{jl}");
    // Top-level struct named after a keyword is escaped to `try_`.
    assert!(jl.contains("struct try_"), "{jl}");
}
