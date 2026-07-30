// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the generated Nim source.
//!
//! These run everywhere (no `nim` needed): they scan the emitted text for the
//! defect classes the idl-construct fix campaign flagged across backends —
//! a duplicated top-level symbol (`type`/`const` redefinition — #A35/#A37/#21),
//! an unescaped Nim reserved word used as an identifier, and known
//! non-compilable tokens (`TRUE`/`FALSE` instead of `true`/`false` — #A13; a
//! wide-literal `L"…"`/`L'…'` prefix — #A7; an empty `case` dispatch). The
//! byte-level and real-compile guarantees live in `adversarial_corpus.rs`
//! (gated on the Nim toolchain).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_nim_module(&ast, &NimGenOptions::default()).expect("gen")
}

/// The Nim reserved keywords (mirrors `crates/idl-nim/src/keywords.rs`), used to
/// assert none appears bare (un-backtick-stropped) as a generated identifier.
const NIM_RESERVED: &[&str] = &[
    "addr",
    "and",
    "as",
    "asm",
    "bind",
    "block",
    "break",
    "case",
    "cast",
    "concept",
    "const",
    "continue",
    "converter",
    "defer",
    "discard",
    "distinct",
    "div",
    "do",
    "elif",
    "else",
    "end",
    "enum",
    "except",
    "export",
    "finally",
    "for",
    "from",
    "func",
    "if",
    "import",
    "in",
    "include",
    "interface",
    "is",
    "isnot",
    "iterator",
    "let",
    "macro",
    "method",
    "mixin",
    "mod",
    "nil",
    "not",
    "notin",
    "object",
    "of",
    "or",
    "out",
    "proc",
    "ptr",
    "raise",
    "ref",
    "return",
    "shl",
    "shr",
    "static",
    "template",
    "try",
    "tuple",
    "type",
    "using",
    "var",
    "when",
    "while",
    "xor",
    "yield",
];

/// Every IDL construct the campaign exercises, minimally. Each entry must emit
/// Nim that passes all structural invariants below.
fn corpus() -> Vec<(&'static str, String)> {
    vec![
        ("final_struct", "@final struct S { long a; double b; string s; };".into()),
        ("appendable_struct", "@appendable struct S { long a; };".into()),
        ("mutable_struct", "@mutable struct S { @must_understand long a; long b; };".into()),
        (
            "struct_inheritance",
            "@final struct Base { long a; }; @final struct Mid : Base { long b; }; \
             @final struct Der : Mid { long c; };"
                .into(),
        ),
        ("enum_value", "enum E { A, @value(7) B, C };".into()),
        (
            "const_all",
            "const long CL = 1 + 2; const unsigned long long CULL = 42; const float CF = 1.5; \
             const double CD = 3.14; const boolean CB = TRUE; const char CC = 'x'; \
             const string CS = \"hi\"; const octet CO = 3;"
                .into(),
        ),
        (
            "union_integer",
            "@final union U switch(long) { case 1: long a; case 2: double b; default: octet d; };".into(),
        ),
        (
            "union_enum",
            "enum Color { RED, GREEN, BLUE }; \
             @final union U switch(Color) { case RED: long r; case GREEN: long g; default: long d; };"
                .into(),
        ),
        (
            "union_enum_exhaustive",
            "enum Two { LO, HI }; @final union U switch(Two) { case LO: long a; case HI: long b; };".into(),
        ),
        (
            "union_char",
            "@final union U switch(char) { case 'A': long a; case 'B': long b; };".into(),
        ),
        (
            "union_bool",
            "@final union U switch(boolean) { case TRUE: long t; case FALSE: long f; };".into(),
        ),
        ("union_mutable", "@mutable union U switch(long) { case 1: long a; case 2: double b; };".into()),
        ("bitset", "bitset BS { bitfield<3> a; bitfield<5> b; };".into()),
        ("bitmask", "@bit_bound(16) bitmask BM { FLAG_A, FLAG_B, FLAG_C };".into()),
        ("optional", "@final struct S { @optional long a; long b; };".into()),
        ("fixed", "@final struct S { fixed<9,2> price; };".into()),
        ("seq", "@final struct S { sequence<long> xs; sequence<octet,4> raw; };".into()),
        ("array_multidim", "@final struct S { long grid[2][3]; };".into()),
        ("map", "@final struct S { map<long, double> m; };".into()),
        (
            "module_nested",
            "module a { module b { @final struct C { long x; }; }; };".into(),
        ),
        (
            "module_reopened",
            "module M { @final struct A { long x; }; }; module M { @final struct B { long y; }; };".into(),
        ),
        (
            "module_flatten_injective",
            "module A_B { @final struct C { long x; }; }; \
             module A { module B { @final struct C { long y; }; }; };"
                .into(),
        ),
        (
            "member_case_fold",
            "@final struct S { long my_field; long myField; long myfield; };".into(),
        ),
        ("interface_nested_type", "interface I { struct Nested { long n; }; };".into()),
        (
            "reserved_word_members",
            "@final struct S { long type; long proc; long var; long object; };".into(),
        ),
    ]
}

/// Collects every top-level `type NAME* =` and `const NAME* …` declaration name.
fn top_level_symbols(nim: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in nim.lines() {
        // A top-level declaration starts at column 0.
        if let Some(rest) = line.strip_prefix("type ") {
            if let Some(name) = rest.split(['*', ' ', '=']).next() {
                if !name.is_empty() {
                    out.push(format!("type {name}"));
                }
            }
        } else if let Some(rest) = line.strip_prefix("const ") {
            if let Some(name) = rest.split(['*', ':', ' ', '=']).next() {
                if !name.is_empty() {
                    out.push(format!("const {name}"));
                }
            }
        }
    }
    out
}

/// A generated identifier appearing in declaration position (type name, object
/// field, const name, proc name). A reserved word here must be backtick-stropped
/// — a bare one would be a keyword token and fail to compile.
fn declared_identifiers(nim: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in nim.lines() {
        let line = raw.trim_start();
        // Object field: `name*: type` (indented). Skip backticked (`` `name` ``).
        if let Some((lhs, _)) = line.split_once("*:") {
            if !lhs.starts_with('`') && lhs.chars().all(|c| c.is_alphanumeric() || c == '_') {
                out.push(lhs.to_string());
            }
        }
        // Type declaration head: `type name* = …`.
        if let Some(rest) = line.strip_prefix("type ") {
            if let Some(head) = rest.split('*').next() {
                if !head.starts_with('`') && head.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    out.push(head.to_string());
                }
            }
        }
    }
    out
}

#[test]
fn no_duplicate_top_level_symbol() {
    for (name, src) in corpus() {
        let nim = emit(&src);
        let syms = top_level_symbols(&nim);
        let mut seen = std::collections::HashSet::new();
        for s in &syms {
            assert!(
                seen.insert(s.clone()),
                "duplicate top-level symbol `{s}` in `{name}`:\n{nim}"
            );
        }
    }
}

#[test]
fn no_unescaped_reserved_word_identifier() {
    for (name, src) in corpus() {
        let nim = emit(&src);
        for ident in declared_identifiers(&nim) {
            assert!(
                !NIM_RESERVED.contains(&ident.as_str()),
                "bare reserved word `{ident}` used as an identifier in `{name}`:\n{nim}"
            );
        }
    }
}

#[test]
fn no_non_compilable_tokens() {
    for (name, src) in corpus() {
        let nim = emit(&src);
        for line in nim.lines() {
            // #A13: the IDL boolean keyword must be lowered to Nim `true`/`false`.
            assert!(
                !contains_word(line, "TRUE") && !contains_word(line, "FALSE"),
                "bare TRUE/FALSE token in `{name}`: `{line}`"
            );
            // #A7: a wide-literal `L\"…\"`/`L'…'` prefix is not valid Nim.
            assert!(
                !line.contains("L\"") && !line.contains("L'"),
                "wide-literal L-prefix in `{name}`: `{line}`"
            );
        }
    }
}

#[test]
fn every_case_dispatch_has_a_branch() {
    // A `case x` line must be followed (ignoring blank lines) by an `of`/`else`
    // branch — an empty dispatch does not compile.
    for (name, src) in corpus() {
        let nim = emit(&src);
        let lines: Vec<&str> = nim.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            if t == "case self.disc" || t == "case result.disc" {
                let next = lines[i + 1..]
                    .iter()
                    .map(|l| l.trim_start())
                    .find(|l| !l.is_empty())
                    .unwrap_or("");
                assert!(
                    next.starts_with("of ") || next.starts_with("else"),
                    "empty case dispatch in `{name}` at line {i}:\n{nim}"
                );
            }
        }
    }
}

/// Whole-word containment (so `TRUE` matches but `myTRUEval` does not).
fn contains_word(hay: &str, word: &str) -> bool {
    hay.match_indices(word).any(|(idx, _)| {
        let before = hay[..idx].chars().next_back();
        let after = hay[idx + word.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        boundary(before) && boundary(after)
    })
}

/// #A10: an inherited member appears in the derived type and its wire encode.
#[test]
fn inheritance_carries_base_members() {
    let nim = emit(
        "@final struct Base { long a; }; @final struct Mid : Base { long b; }; \
         @final struct Der : Mid { long c; };",
    );
    // Der carries a, b, c — base-first.
    let der = nim.split("type Der* = object").nth(1).expect("Der type");
    let body = der.split("proc marshalInto").next().unwrap();
    assert!(body.contains("a*: int32"), "{nim}");
    assert!(body.contains("b*: int32"), "{nim}");
    assert!(body.contains("c*: int32"), "{nim}");
}

/// #A5: a top-level const is emitted, not dropped, and lowered to Nim syntax.
#[test]
fn const_is_emitted_and_lowered() {
    let nim = emit("const boolean FLAG = TRUE; const long MAX = 5;");
    assert!(nim.contains("const FLAG*: bool = true"), "{nim}");
    assert!(nim.contains("const MAX*: int32 = 5"), "{nim}");
}

/// #A35: distinct module paths flattening to the same `join(\"_\")` string now
/// map to distinct Nim types.
#[test]
fn module_flatten_is_injective() {
    let nim = emit(
        "module A_B { @final struct C { long x; }; }; \
         module A { module B { @final struct C { long y; }; }; };",
    );
    assert!(nim.contains("type A__B_C* = object"), "{nim}");
    assert!(nim.contains("type A_B_C* = object"), "{nim}");
}

/// #A37: Nim-style-insensitive member names are disambiguated.
#[test]
fn style_insensitive_members_are_disambiguated() {
    let nim = emit("@final struct S { long my_field; long myField; };");
    let body = nim.split("type S* = object").nth(1).unwrap();
    let body = body.split("proc marshalInto").next().unwrap();
    assert!(body.contains("my_field*: int32"), "{nim}");
    assert!(body.contains("myField_2*: int32"), "{nim}");
}

/// #A17: a `@must_understand` member sets EMHEADER bit 31 in the mutable framing.
#[test]
fn must_understand_sets_emheader_bit31() {
    let nim = emit("@mutable struct S { @must_understand long a; long b; };");
    assert!(nim.contains("body.putU32(uint32(0xc0000000))"), "{nim}");
    assert!(nim.contains("body.putU32(uint32(0x40000001))"), "{nim}");
}

/// #A39: a type declared inside an interface body survives.
#[test]
fn interface_nested_type_survives() {
    let nim = emit("interface I { struct Nested { long n; }; };");
    assert!(nim.contains("type I_Nested* = object"), "{nim}");
    assert!(nim.contains("n*: int32"), "{nim}");
}
