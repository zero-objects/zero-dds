// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the generated OCaml (no `ocaml`
//! needed). These guard the class of defects the IDL-construct fix campaign
//! found in the thin backends, checked purely on the emitted source string:
//!
//! - **No duplicate top-level symbol** — every top-level `module X`, `type x`
//!   and `let x` name is unique within its namespace. Catches the interface-
//!   nested promotion (#A39), struct-inheritance duplication (#A10) and the
//!   module flatten (#21).
//! - **No unescaped reserved word** — an OCaml keyword must never appear as a
//!   generated record-field label, top-level `let`/`type` name.
//! - **No non-compile pattern** — a bare `TRUE`/`FALSE` boolean token, an
//!   `L"…"`/`L'…'` wide-literal prefix, or the illegal empty record
//!   `type t = {}` (#A15 → `type t = unit`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::HashSet;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};

/// The full OCaml keyword set (stable across the codegen's supported range).
const OCAML_KEYWORDS: &[&str] = &[
    "and",
    "as",
    "assert",
    "asr",
    "begin",
    "class",
    "constraint",
    "do",
    "done",
    "downto",
    "else",
    "end",
    "exception",
    "external",
    "false",
    "for",
    "fun",
    "function",
    "functor",
    "if",
    "in",
    "include",
    "inherit",
    "initializer",
    "land",
    "lazy",
    "let",
    "lor",
    "lsl",
    "lsr",
    "lxor",
    "match",
    "method",
    "mod",
    "module",
    "mutable",
    "new",
    "nonrec",
    "object",
    "of",
    "open",
    "or",
    "private",
    "rec",
    "sig",
    "struct",
    "then",
    "to",
    "true",
    "try",
    "type",
    "val",
    "virtual",
    "when",
    "while",
    "with",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).expect("gen")
}

/// Names declared at column 0 for a given `keyword ` prefix (`module `,
/// `type `, `let `), taking the first whitespace-delimited token after it.
fn top_level_named(ml: &str, kw: &str) -> Vec<String> {
    ml.lines()
        // `module type X` is a distinct OCaml namespace (module-type / signature
        // names) — not a `module X`. Don't let the `"module "` prefix swallow a
        // `"module type "` line; those are counted under their own keyword.
        .filter(|l| !(kw == "module " && l.starts_with("module type ")))
        .filter_map(|l| l.strip_prefix(kw))
        .map(|rest| rest.split_whitespace().next().unwrap_or("").to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

fn assert_no_duplicate_top_level(ml: &str) {
    // OCaml has separate namespaces for modules / types / values, so a `type t`
    // and a `module T` can coexist — dedupe within each namespace.
    for kw in ["module ", "module type ", "type ", "let "] {
        let mut seen = HashSet::new();
        for n in top_level_named(ml, kw) {
            assert!(
                seen.insert(n.clone()),
                "duplicate top-level `{kw}{n}` in:\n{ml}"
            );
        }
    }
}

/// Record-field labels: the first token of a `    <name> : <type>;` line inside
/// a module's record body (indented four spaces, contains ` : `).
fn record_field_names(ml: &str) -> Vec<String> {
    ml.lines()
        .filter(|l| l.starts_with("    ") && l.contains(" : ") && l.trim_end().ends_with(';'))
        .filter_map(|l| l.trim_start().split(" : ").next())
        .map(str::to_string)
        .collect()
}

fn assert_no_reserved_identifiers(ml: &str) {
    for f in record_field_names(ml) {
        assert!(
            !OCAML_KEYWORDS.contains(&f.as_str()),
            "reserved word `{f}` used as a record field label in:\n{ml}"
        );
    }
    for kw in ["type ", "let "] {
        for n in top_level_named(ml, kw) {
            assert!(
                !OCAML_KEYWORDS.contains(&n.as_str()),
                "reserved word `{n}` used as a top-level `{kw}` name in:\n{ml}"
            );
        }
    }
}

fn assert_no_noncompile_patterns(ml: &str) {
    // A bare IDL boolean keyword must never survive into the OCaml (it is not an
    // OCaml value — `TRUE`/`FALSE` normalize to `true`/`false`).
    for tok in ["TRUE", "FALSE"] {
        assert!(
            !ml.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .any(|w| w == tok),
            "bare `{tok}` token leaked into the output:\n{ml}"
        );
    }
    // A wide-literal `L"…"` / `L'…'` prefix is not valid OCaml.
    assert!(
        !ml.contains("L\"") && !ml.contains("L'"),
        "L-prefixed wide literal:\n{ml}"
    );
    // The illegal empty record — an empty struct must be `type t = unit`
    // (#A15), never `type t = {}` / `type t = { }`.
    assert!(!ml.contains("= {}"), "empty record literal `= {{}}`:\n{ml}");
    for line in ml.lines() {
        let t = line.trim();
        assert!(
            t != "type t = {" || !line.contains("}"),
            "empty record type:\n{ml}"
        );
    }
    assert!(!ml.contains("{ }"), "empty record `{{ }}`:\n{ml}");
}

fn check_all(ml: &str) {
    assert_no_duplicate_top_level(ml);
    assert_no_reserved_identifiers(ml);
    assert_no_noncompile_patterns(ml);
}

#[test]
fn struct_inheritance_no_duplicate_and_carries_base_fields() {
    // #A10: base fields are inlined base-first; no duplicate top-level module.
    let ml = emit(
        "@final struct Base { long a; long b; };
         @final struct Derived : Base { long c; };",
    );
    check_all(&ml);
    let der = ml.split("module Derived = struct").nth(1).expect("Derived");
    let body = der.split("  }").next().unwrap();
    // Inherited a, b precede the derived c.
    let ia = body.find("a :").expect("a");
    let ib = body.find("b :").expect("b");
    let ic = body.find("c :").expect("c");
    assert!(ia < ib && ib < ic, "base-first order broken:\n{ml}");
}

#[test]
fn empty_struct_is_unit_not_empty_record() {
    // #A15: `type t = {}` is illegal OCaml — a member-less struct is `unit`.
    let ml = emit("@final struct Empty { };");
    check_all(&ml);
    assert!(ml.contains("type t = unit"), "{ml}");
}

#[test]
fn const_all_types_emit_and_normalize() {
    // #A5/P1 + #A13/#A7: every const type emits, booleans normalized, no L".
    let ml = emit(
        "enum Color { RED, GREEN, BLUE };
         const long SHIFT = 1 << 4;
         const long MASK = (1 << 3) | 2;
         const boolean FLAG = TRUE;
         const boolean OFF = FALSE;
         const string GREETING = \"hi\";
         const double PI = 3.14;
         const octet BYTE = 7;
         const char CH = 'x';
         const long long BIG = 100;
         const Color FAV = GREEN;
         const wstring WS = L\"hello\";",
    );
    check_all(&ml);
    // Const names are lower-cased-initial (OCaml `let` names must be), matching
    // the `type_ident` rule the rest of the backend uses.
    assert!(ml.contains("let sHIFT = (1 lsl 4)"), "{ml}");
    assert!(ml.contains("let mASK = ((1 lsl 3) lor 2)"), "{ml}");
    assert!(ml.contains("let fLAG = true"), "{ml}");
    assert!(ml.contains("let oFF = false"), "{ml}");
    assert!(ml.contains("let bIG = 100L"), "{ml}");
    assert!(ml.contains("let fAV = GREEN"), "{ml}");
    assert!(ml.contains("let wS = \"hello\""), "{ml}");
}

#[test]
fn union_enum_char_bool_discriminators_resolve() {
    // #A11/A12/A13/P4: enum / char / boolean labels resolve, no early error,
    // and the match subject is normalized per discriminator kind.
    let ml = emit(
        "enum Color { RED, GREEN, BLUE };
         @final union EU switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
         @final union CU switch (char) { case 'A': long a; case 'B': short b; };
         @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };",
    );
    check_all(&ml);
    assert!(
        ml.contains("(color_to_int v.disc)"),
        "enum disc subject:\n{ml}"
    );
    assert!(
        ml.contains("(Char.code v.disc)"),
        "char disc subject:\n{ml}"
    );
    assert!(
        ml.contains("| 65 ->") && ml.contains("| 66 ->"),
        "char labels:\n{ml}"
    );
    assert!(
        ml.contains("| true ->") && ml.contains("| false ->"),
        "bool labels:\n{ml}"
    );
}

#[test]
fn mutable_must_understand_sets_emheader_bit31() {
    // #A17: a @must_understand member's EMHEADER carries bit 31 (0x8000_0000);
    // a plain member keeps LC4 (0x4000_0000) only.
    let ml = emit("@mutable struct M { @must_understand long x; long y; };");
    check_all(&ml);
    assert!(
        ml.contains("0xc0000000"),
        "must-understand bit missing:\n{ml}"
    );
    assert!(
        ml.contains("0x40000001"),
        "plain member EMHEADER changed:\n{ml}"
    );
}

#[test]
fn array_of_struct_decodes_without_reject() {
    // #A27: a fixed array of a non-primitive element must generate a decoder
    // (nested Array.init), not the former loud reject.
    let ml = emit(
        "@final struct Item { long v; };
         @final struct Holder { Item items[3]; };",
    );
    check_all(&ml);
    assert!(ml.contains("Array.init 3 (fun _ -> (Item.read r))"), "{ml}");
}

#[test]
fn interface_nested_types_promoted_before_use() {
    // #A39: an interface-nested type survives, and a module-level type that
    // references it is emitted AFTER it (OCaml definition-before-use).
    let ml = emit(
        "module m {
           interface Svc { struct Req { long id; }; };
           @final struct Uses { m::Svc::Req r; };
         };",
    );
    check_all(&ml);
    let def = ml.find("module M_sSvc_sReq = struct").expect("Req def");
    let use_ = ml.find("module M_sUses = struct").expect("Uses def");
    assert!(
        def < use_,
        "interface-nested type emitted after its use:\n{ml}"
    );
    assert!(
        ml.contains("r : M_sSvc_sReq.t;"),
        "reference not resolved:\n{ml}"
    );
}

#[test]
fn reserved_words_at_every_position_are_escaped() {
    // OCaml keywords as IDL identifiers (member / struct / enum-enumerator are
    // uppercase-safe; the lowercase-initial member/field positions escape).
    let ml = emit(
        "enum E { AND_V, LET_V };
         @final struct S { long let; long type; long match; long end; long mod; long val; };",
    );
    check_all(&ml);
    // The record labels are the escaped forms.
    assert!(ml.contains("let_ :"), "{ml}");
    assert!(ml.contains("type_ :"), "{ml}");
    assert!(ml.contains("match_ :"), "{ml}");
}

#[test]
fn module_flatten_and_reopen_are_injective() {
    // Reopened + nested modules flatten to distinct, unique module names.
    let ml = emit(
        "module A { struct C { long x; }; };
         module A { struct D { double y; }; };
         module B { module A { struct C { long z; }; }; };",
    );
    check_all(&ml);
    assert!(ml.contains("module A_sC = struct"), "{ml}");
    assert!(ml.contains("module A_sD = struct"), "{ml}");
    assert!(ml.contains("module B_sA_sC = struct"), "{ml}");
}
