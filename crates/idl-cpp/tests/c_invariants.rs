//! Toolchain-free structural invariants for the C backend (`src/c_mode.rs`).
//!
//! These run without gcc — they inspect the generated C *source text* for the
//! failure shapes the IDL-construct fix campaign enumerated:
//!
//! - no duplicate top-level symbol (each `<T>_t` typedef / include guard once);
//! - no un-escaped C reserved word in identifier position (`int` -> `int_`);
//! - none of the cross-backend non-compilable patterns (`TRUE`/`FALSE` bare
//!   literals, an `L"…"` wide-string prefix, a generic-array `new`, an empty
//!   aggregate body `{}`);
//! - the compose invariant behind bug **C-c**: two separately generated
//!   headers must carry *distinct* include guards, so the second is not
//!   silently swallowed when both land in one translation unit; regenerating
//!   the *same* IDL must reproduce the *same* guard (correct dedup).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, generate_c_header};

fn gen_c(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_c_header(&ast, &CGenOptions::default()).expect("c-gen")
}

/// Extract the `#ifndef <GUARD>` token from a generated header.
fn guard_of(header: &str) -> String {
    header
        .lines()
        .find_map(|l| l.strip_prefix("#ifndef ").map(str::to_string))
        .expect("header has an #ifndef guard")
}

// ------------------------------------------------------------------ compose C-c

/// Bug C-c: the guard is derived per-header, so two distinct IDL inputs yield
/// two distinct guards. Without this the fixed `ZERODDS_GENERATED_H` made the
/// second header a no-op in a shared translation unit.
#[test]
fn distinct_inputs_get_distinct_guards() {
    let a = gen_c("@final struct Alpha { long a; };");
    let b = gen_c("@final struct Beta { double b; };");
    let ga = guard_of(&a);
    let gb = guard_of(&b);
    assert_ne!(
        ga, gb,
        "two different IDLs must not share an include guard (C-c):\n{ga}\n{gb}"
    );
    // Both are `#define`d exactly once and closed exactly once.
    assert_eq!(a.matches(&format!("#define {ga}")).count(), 1);
    assert_eq!(b.matches(&format!("#define {gb}")).count(), 1);
}

/// Regenerating the same IDL reproduces the same guard — the legitimate
/// include-guard dedup (re-`#include` of one header is a no-op) still works,
/// and codegen stays reproducible for snapshot tests.
#[test]
fn identical_input_is_guard_stable() {
    let src = "module m { @final struct S { long x; wchar c; }; enum E { A, B }; };";
    assert_eq!(guard_of(&gen_c(src)), guard_of(&gen_c(src)));
}

/// An explicit guard from `CGenOptions` still wins verbatim.
#[test]
fn explicit_guard_is_honoured() {
    let ast = zerodds_idl::parse("@final struct S { long x; };", &ParserConfig::default())
        .expect("parse");
    let h = generate_c_header(
        &ast,
        &CGenOptions {
            include_guard: Some("MY_FIXED_GUARD_H".into()),
            file_header: None,
        },
    )
    .expect("gen");
    assert_eq!(guard_of(&h), "MY_FIXED_GUARD_H");
}

// -------------------------------------------------------- no duplicate symbols

/// No top-level type typedef (`} <T>_t;`) is emitted twice, and the guard is
/// defined exactly once, across a header carrying many distinct constructs.
#[test]
fn no_duplicate_top_level_symbol() {
    let src = "\
module g {
  enum Color { RED, GREEN, BLUE };
  @final struct Point { long x; long y; };
  @appendable struct Line { Point a; Point b; };
  union Var switch (long) { case 0: long i; default: double d; };
};";
    let h = gen_c(src);

    // Include guard: defined once.
    let guard = guard_of(&h);
    assert_eq!(h.matches(&format!("#define {guard}")).count(), 1);

    // Each `} <name>_t;` closing typedef occurs exactly once.
    let mut typedefs: Vec<&str> = Vec::new();
    for line in h.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("} ") {
            if let Some(name) = rest.strip_suffix("_t;") {
                typedefs.push(name);
            }
        }
    }
    assert!(
        typedefs.contains(&"g_sPoint"),
        "expected g_sPoint typedef, got {typedefs:?}"
    );
    let mut sorted = typedefs.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        typedefs.len(),
        "duplicate top-level typedef emitted: {typedefs:?}"
    );
}

// ------------------------------------------------------- reserved-word escapes

/// Every C keyword used as an IDL member name reaches C source in escaped
/// form (`int_`, `while_`), never as the bare keyword in declarator position.
#[test]
fn reserved_member_names_are_escaped() {
    // C keywords that are NOT also IDL keywords (so the frontend accepts them
    // as identifiers) — the collision cases the escaper must catch.
    let kws = [
        "int", "auto", "register", "volatile", "extern", "static", "goto", "return", "sizeof",
        "restrict", "inline", "while", "for", "signed",
    ];
    for kw in kws {
        let src = format!("@final struct S {{ long {kw}; }};");
        let h = gen_c(&src);
        assert!(
            h.contains(&format!("int32_t {kw}_;")),
            "member `{kw}` not escaped to `{kw}_` in:\n{h}"
        );
        // The bare keyword must not appear as a struct field declarator.
        assert!(
            !h.contains(&format!("int32_t {kw};")),
            "un-escaped reserved word `{kw}` reached declarator position:\n{h}"
        );
    }
}

/// A C keyword used as the IDL *type* name is escaped in the typedef too.
#[test]
fn reserved_type_names_are_escaped() {
    let h = gen_c("@final struct int { long a; };");
    assert!(
        h.contains("} int__t;"),
        "struct name `int` not escaped:\n{h}"
    );
    assert!(
        !h.contains("} int_t;") && !h.contains("struct int "),
        "un-escaped reserved type name reached C:\n{h}"
    );
}

// ---------------------------------------------------- non-compilable patterns

/// None of the cross-backend non-compilable shapes appear in C output over a
/// representative corpus.
#[test]
fn no_non_compilable_patterns() {
    let corpus = [
        "@final struct E {};",
        "module m { enum Col { R, G, B }; @final struct S { long x; wchar c; wstring w; }; };",
        "const boolean FLAG = TRUE; @final struct B { boolean b; };",
        "@mutable struct M { @optional long maybe; long arr[2][3]; sequence<long> s; };",
        "@final struct Keyed { @key long id; string label; };",
    ];
    for src in corpus {
        let h = gen_c(src);

        // No bare TRUE/FALSE literal (C has no such constants without <stdbool.h>
        // spelling; the backend uses 0/1 `uint8_t`).
        for bad in ["TRUE", "FALSE"] {
            assert!(
                !h.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .any(|tok| tok == bad),
                "bare `{bad}` literal in C output for `{src}`:\n{h}"
            );
        }
        // No wide-string / wide-char literal prefix — wstring/wchar are emitted
        // as `uint16_t`, never as an `L"…"` literal.
        assert!(
            !h.contains("L\"") && !h.contains("L'"),
            "wide-literal `L` prefix in C output for `{src}`"
        );
        // No generic-array `new` (a Java/TS shape, never valid C).
        assert!(
            !h.split_whitespace().any(|t| t == "new"),
            "`new` token in C output for `{src}`"
        );
        // No empty aggregate body: a `typedef struct <x> {` is never immediately
        // closed by `}` (C forbids empty structs — a placeholder member is used).
        assert!(
            !h.contains("{\n}") && !h.contains("{ }") && !h.contains("{}"),
            "empty aggregate body in C output for `{src}`:\n{h}"
        );
    }
}
