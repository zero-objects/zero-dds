// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Toolchain-free structural invariants of the generated Rust source.
//!
//! These run without a Rust toolchain (no `cargo check`): they parse IDL,
//! generate the module, and assert textual properties that must hold for the
//! output to be well-formed Rust — a fast merge gate that catches the whole
//! family of "emits a non-compiling token" regressions the construct-fix
//! campaign targets, without paying for a compile per case. The heavier,
//! actually-compiling corpus lives in `adversarial_corpus.rs` (toolchain-gated).
//!
//! Invariants checked:
//!   * no duplicate top-level item symbol (would be E0428);
//!   * every Rust reserved word used as an IDL identifier is escaped — as a raw
//!     identifier `r#kw`, or, for the four keywords that cannot be raw
//!     (`crate`/`self`/`super`/`Self`), mangled to `kw_`;
//!   * no known non-compiling token leaks: an unusable raw identifier
//!     (`r#self`/`r#Self`/`r#crate`/`r#super`), a bare IDL boolean literal
//!     (`TRUE`/`FALSE`), or a C wide-literal prefix (`L"`/`L'`).

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Generates the Rust module for `idl`, panicking with the IDL on parse/gen
/// failure.
fn gen_src(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default())
        .unwrap_or_else(|e| panic!("parse failed for `{idl}`: {e:?}"));
    generate_rust_module(&ast, &RustGenOptions::default())
        .unwrap_or_else(|e| panic!("gen failed for `{idl}`: {e:?}"))
}

/// Simple names of items declared at the file's top level (column 0):
/// `pub struct`/`enum`/`mod`/`const`/`type`. Nested (indented) items are
/// intentionally excluded — they live in their own module scope.
fn top_level_symbols(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        // Top-level items start at column 0 (module bodies are indented).
        for kw in [
            "pub struct ",
            "pub enum ",
            "pub mod ",
            "pub const ",
            "pub type ",
        ] {
            if let Some(rest) = line.strip_prefix(kw) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '#')
                    .collect();
                if !name.is_empty() {
                    out.push(name);
                }
            }
        }
    }
    out
}

/// The Rust reserved words the codegen must be able to accept as IDL
/// identifiers. Excludes the words that are ALSO IDL keywords (the IDL parser
/// rejects those as identifiers: `const`/`enum`/`struct`/`union`/`in`/
/// `abstract`), which is verified by [`reserved_idl_keywords_do_not_parse`].
const RUST_KEYWORDS_AS_IDL_IDENTS: &[&str] = &[
    "as", "break", "continue", "crate", "else", "extern", "fn", "for", "if", "impl", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "super",
    "trait", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "gen", "become",
    "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield",
    "try",
];

/// The four Rust keywords that cannot be written as raw identifiers and must be
/// mangled with a trailing underscore instead.
const NON_RAW_KEYWORDS: &[&str] = &["crate", "self", "super", "Self"];

#[test]
fn every_reserved_word_identifier_is_escaped() {
    for kw in RUST_KEYWORDS_AS_IDL_IDENTS {
        let idl = format!("struct Holder_{kw} {{ long {kw}; }};");
        let src = gen_src(&idl);
        let expected_field = if NON_RAW_KEYWORDS.contains(kw) {
            format!("pub {kw}_: i32,")
        } else {
            format!("pub r#{kw}: i32,")
        };
        assert!(
            src.contains(&expected_field),
            "reserved word `{kw}` field must be emitted as `{expected_field}`:\n{src}"
        );
        // Never the bare, unescaped form.
        assert!(
            !src.contains(&format!("pub {kw}: i32,")),
            "reserved word `{kw}` must never be emitted unescaped:\n{src}"
        );
    }
}

#[test]
fn reserved_word_at_every_declaration_position_is_escaped() {
    // struct name, enum name, module name, const name, enumerator name.
    // (A union BRANCH name is PascalCased into a Rust enum variant — e.g.
    // `move` → `Move` — which is not a keyword, so it needs no escaping; that
    // is checked separately below.)
    let cases = [
        ("struct r#match", "struct match { long a; };"),
        ("pub enum r#loop", "enum loop { A, B };"),
        ("pub mod r#type", "module type { struct S { long a; }; };"),
        ("pub const r#static", "const long static = 3;"),
        ("r#async", "enum En { A, async };"),
    ];
    for (needle, idl) in cases {
        let src = gen_src(idl);
        assert!(
            src.contains(needle),
            "expected escaped `{needle}` for `{idl}`:\n{src}"
        );
    }
    // Union branch name that is a reserved word: PascalCased to a safe variant.
    let src = gen_src("union U switch(long) { case 0: long move; default: long other; };");
    assert!(
        src.contains("Move(i32)"),
        "union branch `move` must PascalCase to variant `Move`:\n{src}"
    );
}

#[test]
fn no_duplicate_top_level_symbol() {
    // Reopened modules (which merge to one `pub mod`), plus a mix of item
    // kinds, must yield distinct top-level symbols.
    let src = gen_src(
        "module reo { struct X { long a; }; }; \
         module reo { struct Y { long b; }; }; \
         enum Color { RED, GREEN }; \
         const long LIMIT = 4; \
         struct Top { long v; };",
    );
    let syms = top_level_symbols(&src);
    let mut seen = std::collections::BTreeSet::new();
    for s in &syms {
        assert!(
            seen.insert(s.clone()),
            "duplicate top-level symbol `{s}` (would be E0428):\n{src}"
        );
    }
    // The reopened module must appear exactly once.
    assert_eq!(
        src.matches("pub mod reo").count(),
        1,
        "reopened module must be merged into a single `pub mod`:\n{src}"
    );
}

#[test]
fn no_noncompiling_token_patterns() {
    // A broad corpus covering the constructs whose emitters historically leaked
    // a non-compiling token.
    let corpus = [
        "struct S1 { long self; long crate; long super; long Self; };",
        "const boolean B = TRUE; const boolean C = FALSE;",
        "union U switch(char) { case 'A': long a; case 'B': short b; default: octet o; };",
        r#"const wstring W = L"hi"; const wchar C = L'x';"#,
        "@mutable struct M { @default(7) long a; @id(1) long b; };",
        "enum E { @value(5) A, B, @value(9) C };",
    ];
    for idl in corpus {
        let src = gen_src(idl);
        // The four keywords that cannot be raw identifiers must never appear in
        // the `r#` form.
        for bad in ["r#self", "r#Self", "r#crate", "r#super", "r#_"] {
            assert!(
                !src.contains(bad),
                "`{bad}` is not a valid Rust raw identifier, for `{idl}`:\n{src}"
            );
        }
        // IDL boolean literals must be lowered, never leaked verbatim.
        for bad in ["TRUE", "FALSE"] {
            assert!(
                !src.contains(bad),
                "IDL boolean literal `{bad}` leaked into Rust for `{idl}`:\n{src}"
            );
        }
        // C wide-literal prefixes must be stripped.
        assert!(
            !src.contains("L\"") && !src.contains("L'"),
            "C wide-literal prefix leaked for `{idl}`:\n{src}"
        );
    }
}

#[test]
fn reserved_idl_keywords_do_not_parse_as_identifiers() {
    // Documents why the reserved-word corpus excludes these: they are IDL
    // keywords, so the FRONTEND rejects them as identifiers — they can never
    // reach the Rust emitter.
    for kw in ["const", "enum", "struct", "union", "in", "abstract"] {
        let idl = format!("struct H {{ long {kw}; }};");
        assert!(
            zerodds_idl::parse(&idl, &ParserConfig::default()).is_err(),
            "`{kw}` unexpectedly parsed as an IDL identifier"
        );
    }
}
