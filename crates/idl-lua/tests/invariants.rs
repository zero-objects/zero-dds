// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the Lua emitter's output. These run
//! everywhere (no `lua`/`luac` needed) and guard the classes of defect the
//! IDL-construct fix campaign targeted for the Lua backend:
//!
//! * no duplicate top-level symbol (a type/const emitted twice would make the
//!   second `local`/`function` silently shadow the first);
//! * no unescaped Lua reserved word spliced in as an identifier (`end` →
//!   `end_`) — Lua has no stropping syntax, so a bare keyword is a syntax error;
//! * no non-compiling token forms: the IDL boolean keywords `TRUE`/`FALSE`
//!   (must lower to `true`/`false`), and the wide-literal `L"…"` / `L'…'`
//!   prefixes (not valid Lua).
//!
//! The heavier "generate + actually compile with the target toolchain" checks
//! live in `adversarial_corpus.rs` (gated on a Lua interpreter on PATH).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};

/// The 22 Lua 5.4 reserved words (mirrors `keywords::LUA_RESERVED`, which is
/// crate-private). A subset (`in`, `local`) are *also* IDL keywords and so can
/// never reach the emitter as an identifier — those specs fail to parse and are
/// skipped by [`try_emit`].
const LUA_RESERVED: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_lua_module(&ast, &LuaGenOptions::default()).expect("gen")
}

/// Emits `src`, returning `None` if it does not even parse (used for the
/// reserved-word sweep: words that are IDL keywords too never parse).
fn try_emit(src: &str) -> Option<String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).ok()?;
    generate_lua_module(&ast, &LuaGenOptions::default()).ok()
}

/// A broad single-spec corpus touching every construct the campaign covered, so
/// the whole-file scans below see them all at once.
const CORPUS: &str = "
enum Color { RED, GREEN, @value(9) BLUE };
@bit_bound(8) bitmask Flags { A, B, @position(5) C };
bitset Packed { bitfield<3> lo; bitfield<5> hi; };
@final struct Base { @key long id; long a; };
@appendable struct Derived : Base { double d; };
@mutable struct Mut { @must_understand long must; @optional long maybe; };
union UInt switch (long) { case 1: long a; case 2: short b; default: octet o; };
union UEnum switch (Color) { case RED: long r; case BLUE: double d; };
union UChar switch (char) { case 'A': long a; case 'Z': short z; };
union UBool switch (boolean) { case TRUE: long t; default: short f; };
@mutable union UMut switch (long) { case 1: long a; case 2: double b; };
@final struct Coll {
    sequence<long> s;
    long grid[2][3];
    map<long, string> m;
    string<8> name;
    fixed<5,2> price;
};
typedef long Score;
@final struct Uses { Score sc; Color col; };
const long MAXN = 0x20;
const boolean ENABLED = TRUE;
const double RATE = 2.5;
const string TITLE = \"z\";
const char INITIAL = 'Q';
module Outer { module Inner { @final struct Leaf { long v; }; }; };
module Outer_X { @final struct Leaf { long v; }; };
module Reop { @final struct P { long a; }; };
module Reop { @final struct Q { long b; }; };
interface Svc { struct Nested { long z; }; };
";

/// Every top-level (column-0) `function NAME(` and `local NAME =` symbol must be
/// unique: a duplicate means one declaration silently shadows another, dropping
/// a whole type or const from the module.
#[test]
fn no_duplicate_top_level_symbol() {
    let lua = emit(CORPUS);
    let mut seen = std::collections::HashSet::new();
    for line in lua.lines() {
        // Only column-0 declarations are module-level; indented ones are locals
        // inside function bodies (legitimately repeated, e.g. `local zdMem`).
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let sym = if let Some(rest) = line.strip_prefix("function ") {
            rest.split('(').next().map(str::to_string)
        } else if let Some(rest) = line.strip_prefix("local ") {
            let head = rest.split('=').next().unwrap_or("").trim();
            // Skip the multi-name prelude binding `local LE, BE = …`.
            if head.contains(',') {
                None
            } else {
                Some(head.to_string())
            }
        } else {
            None
        };
        if let Some(name) = sym {
            assert!(
                seen.insert(name.clone()),
                "duplicate top-level symbol `{name}`:\n{lua}"
            );
        }
    }
}

/// The IDL boolean keywords must never survive as bare `TRUE`/`FALSE` tokens —
/// they are not Lua identifiers. They must lower to `true`/`false`.
#[test]
fn no_bare_boolean_keyword() {
    let lua = emit(CORPUS);
    assert!(
        !contains_word(&lua, "TRUE"),
        "bare `TRUE` token in output:\n{lua}"
    );
    assert!(
        !contains_word(&lua, "FALSE"),
        "bare `FALSE` token in output:\n{lua}"
    );
}

/// A wide char/string literal keeps no `L` prefix (`L\"x\"` / `L'x'` is not
/// valid Lua). Exercised via a `wchar`/`wstring` const and union char labels.
#[test]
fn no_wide_literal_prefix() {
    let lua = emit(
        "
const wchar WC = L'w';
const wstring WS = L\"ws\";
@final struct S { wchar c; wstring t; };
",
    );
    assert!(
        !lua.contains("L\""),
        "wide-string `L\"` prefix leaked:\n{lua}"
    );
    assert!(!lua.contains("L'"), "wide-char `L'` prefix leaked:\n{lua}");
}

/// Every Lua reserved word placed in an IDL identifier position (struct name,
/// member, enum, enumerator, const, union-branch field) must come out escaped
/// (`end` → `end_`), never as a bare `.end`/`local end`/`function …end(` token.
/// Words that are IDL keywords too (`in`, `local`) fail to parse and are
/// skipped — they can never reach the emitter as an identifier.
#[test]
fn reserved_words_are_escaped_in_every_position() {
    let mut exercised = 0;
    for &kw in LUA_RESERVED {
        // struct name + member + enumerator + const + union-branch field, all
        // named with the reserved word.
        let src = format!(
            "enum E_{kw} {{ {kw} }};
@final struct {kw} {{ long {kw}; }};
const long {kw}_c = 1;
union U_{kw} switch (long) {{ case 1: long {kw}; }};"
        );
        let Some(lua) = try_emit(&src) else {
            continue;
        };
        exercised += 1;

        // The escaped form must appear (proof the identifier was emitted).
        assert!(
            lua.contains(&format!("{kw}_")),
            "reserved word `{kw}` was not escaped to `{kw}_`:\n{lua}"
        );
        // No bare reserved word as a field access `.<kw>` (word-bounded).
        assert!(
            !contains_after(&lua, ".", kw),
            "bare reserved word `.{kw}` (unescaped field access):\n{lua}"
        );
        // No bare reserved word as a top-level `local <kw> …` binding.
        assert!(
            !contains_after(&lua, "local ", kw),
            "bare reserved word `local {kw}` (unescaped binding):\n{lua}"
        );
    }
    // The bulk of the keyword set is expressible as an IDL identifier; guard
    // against the sweep silently degenerating to a no-op.
    assert!(
        exercised >= 18,
        "only {exercised} reserved words exercised (parser regression?)"
    );
}

/// True if `word` occurs in `s` bounded by non-identifier characters on both
/// sides (so `end` does not match inside `endian`).
fn contains_word(s: &str, word: &str) -> bool {
    s.match_indices(word).any(|(i, _)| {
        let before = s[..i].chars().next_back();
        let after = s[i + word.len()..].chars().next();
        !before.is_some_and(is_ident_char) && !after.is_some_and(is_ident_char)
    })
}

/// True if `prefix` immediately followed by `word` occurs in `s`, with `word`
/// bounded by a non-identifier character on its right (so `.end` does not match
/// inside `.endian`).
fn contains_after(s: &str, prefix: &str, word: &str) -> bool {
    let needle = format!("{prefix}{word}");
    s.match_indices(&needle).any(|(i, _)| {
        let after = s[i + needle.len()..].chars().next();
        !after.is_some_and(is_ident_char)
    })
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
