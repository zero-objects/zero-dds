// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Toolchain-free structural invariants for the idl-csharp emitter.
//!
//! These assert directly on the generated C# source string — no `dotnet`
//! required — and pin the construct-fix-campaign findings for this backend:
//!
//! - **F4** enum `@value` gaps emit explicit values (not plain ordinals).
//! - **F6** `const boolean = TRUE/FALSE` normalises to `true`/`false`.
//! - **F7** `const wstring L"…"` / `const wchar L'x'` drop the IDL `L` prefix.
//! - **F36** two case-only-differing members (`my_field` / `myField`) get
//!   distinct C# property names instead of a CS0102 duplicate.
//! - **F3** a `long double` member yields the data record but NO codec class
//!   (no encode-throw / silent-`default(decimal)` TypeSupport).
//! - **F35** the CORBA-trait module flatten is injective (`A::B::C` vs
//!   `A::(B_C)` map to distinct method identifiers).
//!
//! Cross-cutting: no duplicate top-level symbol, no C# reserved word emitted as
//! a bare (unescaped) identifier, and none of the known non-compiling literal
//! patterns (`TRUE`/`FALSE`, `L"`/`L'`, empty `enum`/`record` body).
//!
//! The heavier proof (real `dotnet` compile) lives in the gated
//! `adversarial_corpus.rs`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_csharp::{CsGenOptions, generate_csharp, generate_csharp_with_corba_traits};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp(&ast, &CsGenOptions::default()).expect("gen")
}

fn try_gen(src: &str) -> Result<String, String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp(&ast, &CsGenOptions::default()).map_err(|e| format!("{e:?}"))
}

fn emit_corba(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp_with_corba_traits(&ast, &CsGenOptions::default()).expect("gen")
}

// ---------------------------------------------------------------------------
// F4 — enum @value
// ---------------------------------------------------------------------------

#[test]
fn enum_value_gaps_emit_explicit_values() {
    // XTypes 1.3 §7.3.1.2.1.6: a gap must carry to the C# constant, else C#
    // re-derives `0,1,2,…` and the wire value diverges.
    let cs = emit("enum Sparse { A, @value(5) B, C, @value(100) D };");
    for tok in ["A = 0,", "B = 5,", "C = 6,", "D = 100,"] {
        assert!(cs.contains(tok), "missing {tok:?} in:\n{cs}");
    }
}

#[test]
fn enum_negative_value_is_signed_and_continues() {
    let cs = emit("enum N { A, @value(-3) B, C };");
    assert!(
        cs.contains("B = -3,"),
        "negative @value not honoured:\n{cs}"
    );
    assert!(
        cs.contains("C = -2,"),
        "successor of negative not continued:\n{cs}"
    );
}

#[test]
fn plain_enum_still_gets_explicit_ordinals() {
    let cs = emit("enum Color { RED, GREEN, BLUE };");
    for tok in ["RED = 0,", "GREEN = 1,", "BLUE = 2,"] {
        assert!(cs.contains(tok), "missing {tok:?} in:\n{cs}");
    }
}

// ---------------------------------------------------------------------------
// F6 — boolean const TRUE/FALSE
// ---------------------------------------------------------------------------

#[test]
fn boolean_const_normalises_case() {
    let cs = emit("const boolean FLAG_ON = TRUE; const boolean FLAG_OFF = FALSE;");
    assert!(cs.contains("true"), "expected lower-case true:\n{cs}");
    assert!(cs.contains("false"), "expected lower-case false:\n{cs}");
    // C# has no `TRUE`/`FALSE` token — the raw IDL spelling must not survive.
    assert!(!cs.contains("TRUE"), "raw TRUE literal leaked:\n{cs}");
    assert!(!cs.contains("FALSE"), "raw FALSE literal leaked:\n{cs}");
}

// ---------------------------------------------------------------------------
// F7 — wide literal L prefix
// ---------------------------------------------------------------------------

#[test]
fn wide_literals_drop_the_l_prefix() {
    // `L"…"` / `L'…'` are not valid C#; C# string/char are UTF-16 already.
    let cs = emit("const wstring WS = L\"hi\"; const wchar WC = L'x';");
    assert!(
        !cs.contains("L\""),
        "F7: wide-string `L\"` prefix leaked:\n{cs}"
    );
    assert!(
        !cs.contains("L'"),
        "F7: wide-char `L'` prefix leaked:\n{cs}"
    );
    assert!(cs.contains("\"hi\""), "F7: expected the string body:\n{cs}");
    assert!(cs.contains("'x'"), "F7: expected the char body:\n{cs}");
}

// ---------------------------------------------------------------------------
// F36 — PascalCase collision dedup
// ---------------------------------------------------------------------------

#[test]
fn case_only_members_get_distinct_property_names() {
    // `my_field` and `myField` both fold to `MyField` — a CS0102 duplicate.
    let cs = emit("struct S { long my_field; long myField; };");
    assert_eq!(
        cs.matches("public int MyField {").count(),
        1,
        "F36: `MyField` must appear exactly once:\n{cs}"
    );
    assert!(
        cs.contains("public int MyField_2 {"),
        "F36: the colliding member must be suffixed `MyField_2`:\n{cs}"
    );
}

#[test]
fn dedup_agrees_between_record_and_typesupport() {
    // The codec must reference the SAME deduped names as the data record, so a
    // suffixed property is not left unassigned/unencoded.
    let cs = emit("struct S { long my_field; long myField; };");
    // Both properties are assigned in the decode object-initializer.
    assert!(
        cs.contains("MyField = ") && cs.contains("MyField_2 = "),
        "F36: both deduped names must be wired in the codec:\n{cs}"
    );
}

// ---------------------------------------------------------------------------
// F3 — long double gated (no throwing/silent codec)
// ---------------------------------------------------------------------------

#[test]
fn long_double_member_yields_record_but_no_codec() {
    // C# has no binary128 primitive; a long double member gets the data record
    // but NO TypeSupport (the `any`/`map` gate), never an encode-throwing /
    // silent-`default(decimal)` codec.
    let cs = try_gen("struct F { long double v; };").expect("must still generate the data type");
    assert!(
        cs.contains("record class F"),
        "F3: the data record must still be emitted:\n{cs}"
    );
    assert!(
        !cs.contains("class FTypeSupport"),
        "F3: no codec class for a long-double struct:\n{cs}"
    );
    assert!(
        !cs.contains("default(decimal)"),
        "F3: silent `default(decimal)` decode must not be emitted:\n{cs}"
    );
    assert!(
        !cs.contains("long double not in v1.0"),
        "F3: encode-throw stub must not be emitted:\n{cs}"
    );
}

#[test]
fn long_double_free_struct_still_gets_a_codec() {
    let cs = emit("struct Ok { double v; long n; };");
    assert!(
        cs.contains("class OkTypeSupport"),
        "a codecable struct must keep its TypeSupport:\n{cs}"
    );
}

// ---------------------------------------------------------------------------
// F35 — injective CORBA-trait module flatten
// ---------------------------------------------------------------------------

#[test]
fn corba_trait_flatten_is_injective() {
    // `Outer::In::C` and `Outer::(In_C)` both flattened to `Outer_In_C` before
    // the fix (CS0102 duplicate const). The injective join doubles each
    // segment's own underscores → `Outer_In_C` vs `Outer_In__C`.
    let cs = emit_corba(
        "module Outer { module In { struct C { long x; }; }; struct In_C { long y; }; };",
    );
    assert!(
        cs.contains("Outer_In_C_FullName"),
        "F35: nested `Outer::In::C` trait entry missing:\n{cs}"
    );
    assert!(
        cs.contains("Outer_In__C_FullName"),
        "F35: flat `Outer::In_C` trait entry must use the doubled separator:\n{cs}"
    );
    // The two must be distinct const identifiers (no duplicate declaration).
    assert_eq!(
        cs.matches("Outer_In_C_FullName =").count(),
        1,
        "F35: the nested-path trait const must not be duplicated:\n{cs}"
    );
}

// ---------------------------------------------------------------------------
// Cross-cutting invariants
// ---------------------------------------------------------------------------

/// C# reserved keywords that are also valid *bare* IDL identifiers (i.e. not
/// IDL keywords) — the emitter must `@`-escape each when it lands in a position
/// that keeps the original casing (type/module/enum-value/const name).
const CS_RESERVED_BARE_IDL: &[&str] = &[
    "class",
    "namespace",
    "object",
    "new",
    "lock",
    "params",
    "virtual",
    "internal",
    "sealed",
    "event",
    "delegate",
    "checked",
    "this",
    "throw",
    "static",
];

#[test]
fn reserved_words_are_escaped_as_type_and_enum_and_const_names() {
    for kw in CS_RESERVED_BARE_IDL {
        // Positions that preserve the original identifier casing: a module name
        // (→ namespace), a struct name, an enum value, a const name.
        let src = format!(
            "module {kw} {{ struct {kw} {{ long x; }}; enum E {{ {kw} }}; const long {kw}_c = 1; }};"
        );
        let cs = emit(&src);
        // The declaration line must escape the keyword (`namespace @class`); a
        // bare `namespace class {` would not compile.
        assert!(
            cs.contains(&format!("namespace @{kw}")),
            "reserved `{kw}` not `@`-escaped as a namespace in:\n{cs}"
        );
        // The escaped form must also appear for the struct/enum-value name.
        assert!(
            cs.matches(&format!("@{kw}")).count() >= 2,
            "reserved `{kw}` must be `@`-escaped at type/enum positions in:\n{cs}"
        );
    }
}

#[test]
fn no_duplicate_top_level_symbol() {
    let cs = emit("struct Alpha { long a; }; struct Beta { long b; }; enum Gamma { G0, G1 };");
    assert_eq!(
        cs.matches("record class Alpha\n").count() + cs.matches("record class Alpha ").count(),
        1,
        "Alpha data record duplicated:\n{cs}"
    );
    assert_eq!(
        cs.matches("public enum Gamma :").count(),
        1,
        "Gamma enum duplicated:\n{cs}"
    );
}

#[test]
fn no_empty_enum_or_record_body() {
    // A defined struct/enum must never emit a truly empty body — the signature
    // of a silently dropped construct.
    let cs = emit("struct S { long x; }; enum E { ONLY };");
    assert!(!cs.contains("enum E : int\n{\n}"), "empty enum body:\n{cs}");
    assert!(cs.contains("ONLY = 0,"), "enumerator dropped:\n{cs}");
    assert!(
        cs.contains("public int X {"),
        "struct member dropped:\n{cs}"
    );
}

#[test]
fn no_raw_true_false_or_wide_prefix_in_mixed_unit() {
    let cs = emit("const boolean B = TRUE; const wstring W = L\"x\"; enum E { X };");
    assert!(!cs.contains("TRUE"), "raw TRUE leaked:\n{cs}");
    assert!(!cs.contains("L\""), "wide prefix leaked:\n{cs}");
    assert!(cs.contains("X = 0,"), "enumerator dropped:\n{cs}");
}
