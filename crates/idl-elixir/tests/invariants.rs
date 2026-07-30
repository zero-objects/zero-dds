// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the generated Elixir source.
//!
//! These never launch `elixir`; they assert properties of the emitted string
//! that are necessary (if not sufficient) for it to compile:
//!
//! 1. no duplicate top-level `defmodule <Name> do` (a collision would make the
//!    second definition silently shadow the first — #A35 non-injective flatten,
//!    reopened modules);
//! 2. every top-level module alias segment is uppercase-initial (a lowercase or
//!    reserved-word-derived IDL type/const name must not flatten to an invalid
//!    `Zdgen.end`/`Zdgen.max`);
//! 3. an IDL identifier that is an Elixir reserved word is escaped everywhere it
//!    becomes a bare Elixir identifier (`:do_`, `v.do_`, `{do_, r}`), never left
//!    as a bare `:do` / `v.do`;
//! 4. no non-compilable token leaks through: a bare `TRUE`/`FALSE` boolean
//!    keyword, or an IDL wide-literal `L"…"`/`L'…'` prefix;
//! 5. no empty `defmodule … do` immediately closed by `end`.
//!
//! The exhaustive real-toolchain corpus lives in `adversarial_corpus.rs`
//! (gated on `elixirc`).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

/// The Elixir reserved words the emitter must escape (mirrors
/// `keywords::ELIXIR_RESERVED`).
const RESERVED: &[&str] = &[
    "true", "false", "nil", "when", "and", "or", "not", "in", "fn", "do", "end", "catch", "rescue",
    "after", "else",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

/// Emits, returning `None` if the IDL itself rejects the source (some Elixir
/// reserved words — `in`, `true`, `false` — are also IDL keywords/literals and
/// can never reach the emitter, so escaping them is moot).
fn try_emit(src: &str) -> Option<String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).ok()?;
    generate_elixir_module(&ast, &ElixirGenOptions::default()).ok()
}

/// Top-level (column-0) `defmodule <Name> do` names, in order.
fn top_level_module_names(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("defmodule ")?;
            let name = rest.strip_suffix(" do")?;
            Some(name.to_string())
        })
        .collect()
}

/// A representative spec that exercises every construct the backend emits.
const KITCHEN_SINK: &str = "\
const long MAX = 10;
const boolean FLAG = TRUE;
const double PI = 3.14;
enum Color { RED, GREEN, BLUE };
@final struct Base { long a; };
@appendable struct Derived : Base { long b; string s; };
@mutable struct Mut { @must_understand long id; @optional long opt; };
union UE switch (Color) { case RED: long r; case GREEN: double g; default: octet o; };
union UC switch (char) { case 'A': long a; case 'B': double b; };
union UB switch (boolean) { case TRUE: long t; case FALSE: double f; };
@mutable union UM switch (long) { case 1: long a; case 2: double b; };
bitmask Flags { fa, fb, fc };
bitset Bits { bitfield<3> lo; bitfield<5> hi; };
typedef sequence<long> LongSeq;
@final struct Coll { LongSeq xs; sequence<double> ys; long grid[2][3]; map<long, string> m; };
module outer { module inner { @final struct Deep { long v; }; }; };
interface Svc { struct Ping { long seq; }; };
";

#[test]
fn no_duplicate_top_level_modules() {
    let out = emit(KITCHEN_SINK);
    let names = top_level_module_names(&out);
    let mut seen = std::collections::HashSet::new();
    for n in &names {
        assert!(
            seen.insert(n.clone()),
            "duplicate top-level `defmodule {n} do` — collision would shadow a type\n{out}"
        );
    }
    // Sanity: the sink really did emit a spread of modules.
    assert!(names.len() > 10, "expected many modules, got {names:?}");
}

#[test]
fn every_module_segment_is_uppercase_initial() {
    let out = emit(KITCHEN_SINK);
    for n in top_level_module_names(&out) {
        for seg in n.split('.') {
            let first = seg.chars().next().expect("non-empty module segment");
            assert!(
                first.is_uppercase(),
                "module segment `{seg}` in `{n}` is not uppercase-initial (invalid Elixir alias)"
            );
        }
    }
}

#[test]
fn injective_module_flatten_across_underscore_and_nesting() {
    // `module a { module b_c { struct D; }; }` and
    // `module a_b { module c { struct D; }; }` must NOT collapse to one module
    // name (#A35). Both `D`s in one spec → two distinct top-level modules.
    let out = emit(
        "module a { module b_c { @final struct D { long x; }; }; }; \
         module a_b { module c { @final struct D { long y; }; }; };",
    );
    let names = top_level_module_names(&out);
    let ds: Vec<&String> = names.iter().filter(|n| n.ends_with("_D")).collect();
    assert_eq!(ds.len(), 2, "expected two distinct D modules, got {ds:?}");
    assert_ne!(
        ds[0], ds[1],
        "the two D modules collapsed to one name (#A35)"
    );
}

#[test]
fn reserved_word_members_are_escaped() {
    for &w in RESERVED {
        let Some(out) = try_emit(&format!("@final struct S {{ long {w}; }};")) else {
            eprintln!("SKIP reserved `{w}`: not a legal IDL identifier");
            continue;
        };
        // Escaped atom in `defstruct`, escaped struct access, escaped decode var.
        assert!(
            out.contains(&format!(":{w}_")),
            "field `{w}` not escaped to `:{w}_` in defstruct\n{out}"
        );
        assert!(
            out.contains(&format!("v.{w}_")),
            "field access for `{w}` not escaped to `v.{w}_`\n{out}"
        );
        // No bare reserved atom / access survives (would not compile).
        for bad in [
            format!(":{w}]"),
            format!(":{w},"),
            format!("v.{w} "),
            format!("v.{w})"),
        ] {
            assert!(
                !out.contains(&bad),
                "bare reserved identifier `{bad}` leaked for word `{w}`\n{out}"
            );
        }
    }
}

#[test]
fn reserved_word_type_names_yield_valid_modules() {
    // A reserved word as struct / enum / const / module name must flatten to a
    // valid (uppercase-initial) Elixir module alias, never a bare `Zdgen.end`.
    for &w in RESERVED {
        let specs = [
            format!("@final struct {w} {{ long v; }};"),
            format!("enum {w} {{ {w}_a, {w}_b }};"),
            format!("const long {w} = 1;"),
            format!("module {w} {{ @final struct S {{ long v; }}; }};"),
        ];
        for spec in specs {
            let Some(out) = try_emit(&spec) else {
                continue;
            };
            for n in top_level_module_names(&out) {
                for seg in n.split('.') {
                    let first = seg.chars().next().expect("non-empty segment");
                    assert!(
                        first.is_uppercase(),
                        "reserved-word spec `{spec}` produced invalid module segment `{seg}`\n{out}"
                    );
                }
            }
        }
    }
}

#[test]
fn no_bare_boolean_keyword_or_wide_literal_prefix() {
    let out = emit(KITCHEN_SINK);
    // Elixir has no `TRUE`/`FALSE`; the emitter must render `true`/`false`.
    assert!(!out.contains("TRUE"), "bare `TRUE` keyword leaked\n{out}");
    assert!(!out.contains("FALSE"), "bare `FALSE` keyword leaked\n{out}");
    // An IDL wide literal `L"…"`/`L'…'` is not valid Elixir.
    assert!(
        !out.contains("L\""),
        "wide-string `L\"` prefix leaked\n{out}"
    );
    assert!(!out.contains("L'"), "wide-char `L'` prefix leaked\n{out}");
}

#[test]
fn no_empty_module_body() {
    let out = emit(KITCHEN_SINK);
    // A `defmodule X do` immediately followed by `end` is an empty (useless and,
    // for record-like backends, non-compiling) module. Every emitted module must
    // carry at least one member.
    let lines: Vec<&str> = out.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("defmodule ") && l.ends_with(" do") {
            let next = lines.get(i + 1).map(|s| s.trim()).unwrap_or("");
            assert_ne!(next, "end", "empty module body at line {}: {l}", i + 1);
        }
    }
}

#[test]
fn const_all_scalar_types_render_literals() {
    // Every `const` scalar renders a bare Elixir literal (no dropped decl, no
    // `TRUE`, no ill-formed token) — #A5/P1.
    let out = emit(
        "const long I = -7; const unsigned long U = 0x10; const boolean B = FALSE; \
         const float F = 1.5; const double D = 2.0; const char C = 'Z'; \
         const string S = \"hi\"; const octet O = 3;",
    );
    for (name, lit) in [
        ("I", "-7"),
        ("U", "0x10"),
        ("B", "false"),
        ("F", "1.5"),
        ("D", "2.0"),
        ("C", "?Z"),
        ("S", "\"hi\""),
        ("O", "3"),
    ] {
        assert!(
            out.contains(&format!("defmodule Zdgen.{name} do")),
            "const {name} module missing\n{out}"
        );
        assert!(
            out.contains(&format!("def value, do: {lit}")),
            "const {name} did not render `{lit}`\n{out}"
        );
    }
}

#[test]
fn interface_nested_type_is_emitted() {
    // A type declared inside `interface { ... }` must not be dropped (#A39): it
    // is promoted under the interface's own scope segment.
    let out = emit("interface Svc { struct Ping { long seq; }; };");
    assert!(
        out.contains("defmodule Zdgen.Svc_Ping do"),
        "interface-nested struct dropped\n{out}"
    );
}

#[test]
fn struct_inheritance_lists_base_fields_first() {
    // Base members precede derived members in both `defstruct` and the wire
    // pipe (#A10).
    let out = emit(
        "@final struct Base { long a; long b; }; \
         @final struct Derived : Base { long c; };",
    );
    let ds = out
        .split("defmodule Zdgen.Derived do")
        .nth(1)
        .expect("Derived module");
    assert!(
        ds.contains("defstruct [:a, :b, :c]"),
        "derived struct missing base-first fields\n{ds}"
    );
}
