// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants for the Zig emitter — no `zig` binary
//! required, so they run on every host and in the base CI gate. They guard the
//! properties a compile would otherwise catch, over the full construct set the
//! `idl-construct-fix-campaign` covers for this backend:
//!
//!  1. no doubled top-level symbol (module-flatten injectivity — #A35),
//!  2. no unescaped Zig reserved word used as an identifier (keyword escaping),
//!  3. none of the cross-backend non-compile patterns leak into the output
//!     (`TRUE`/`FALSE`, an `L"`/`L'` wide-literal prefix, a Java-style generic
//!     array construction, or an empty aggregate body).

#![allow(clippy::expect_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

/// Emits Zig for `src`, panicking on parse/generation failure.
fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_zig_module(&ast, &ZigGenOptions::default()).expect("gen")
}

/// Emits Zig for `src`, returning `None` when the IDL does not parse (used by
/// the reserved-word sweep to skip words that are themselves IDL keywords).
fn try_emit(src: &str) -> Option<String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).ok()?;
    generate_zig_module(&ast, &ZigGenOptions::default()).ok()
}

/// A single IDL specification exercising every construct the Zig backend emits.
const FULL_IDL: &str = "\
const long MAXN = 42;
const boolean FLAG = TRUE;
const string GREETING = \"hi\";
const double RATE = 2.5;
enum Color { RED, @value(5) GREEN, BLUE };
@bit_bound(16) bitmask Perm { READ, WRITE, EXEC };
bitset Flags { bitfield<1> ready; bitfield<3> level; };
@final struct Base { @key long x; long w; };
@final struct Derived : Base { @key long y; };
@final union UC switch (Color) { case RED: long r; case GREEN: unsigned long g; default: long d; };
@final union CH switch (char) { case 'A': long a; default: octet b; };
@final union BU switch (boolean) { case TRUE: long t; case FALSE: short f; };
@mutable union MU switch (long) { case 1: unsigned long a; case 2: unsigned short b; };
@mutable struct MoptS { @id(1) @optional unsigned long a; @id(2) @must_understand string s; };
@appendable struct Coll { sequence<long> nums; long grid[2][3]; map<long, unsigned long> m; };
@final struct NM { map<long, map<long, unsigned long>> mm; };
interface Svc { struct Nested { long n; }; };
module a_b { @final struct C { long v; }; };
module a { module b { @final struct C { double w; }; }; };";

/// Collects every top-level `pub const <NAME> = ...` symbol name (the emitter's
/// only top-level declaration form: struct/enum/union/bitset/bitmask/const).
fn top_level_symbols(z: &str) -> Vec<String> {
    z.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("pub const ")?;
            // Name ends at the first ` =`, ` :`, or `(` boundary.
            let end = rest.find([' ', ':', '(']).unwrap_or(rest.len());
            Some(rest[..end].to_string())
        })
        .collect()
}

#[test]
fn no_doubled_top_level_symbol() {
    let z = emit(FULL_IDL);
    let syms = top_level_symbols(&z);
    let mut seen = std::collections::HashSet::new();
    for s in &syms {
        assert!(
            seen.insert(s.clone()),
            "duplicate top-level symbol `{s}` in generated Zig:\n{z}"
        );
    }
    // #A35: the two `C` structs in disjoint module paths (`a_b::C` and
    // `a::b::C`) must be distinct injective names, never a single collision.
    assert!(syms.iter().any(|s| s == "a__b_C"), "{z}");
    assert!(syms.iter().any(|s| s == "a_b_C"), "{z}");
}

#[test]
fn no_noncompile_patterns_leak() {
    let z = emit(FULL_IDL);
    // A bare IDL boolean keyword must never survive into Zig source (it is not
    // a Zig token) — the const `FLAG = TRUE` and the `case TRUE/FALSE:` union
    // labels must both be normalized (#A13).
    assert!(!z.contains("TRUE"), "bare TRUE token leaked:\n{z}");
    assert!(!z.contains("FALSE"), "bare FALSE token leaked:\n{z}");
    // A wide `L"…"` / `L'…'` literal prefix is not valid Zig.
    assert!(!z.contains("L\""), "wide-string L-prefix leaked:\n{z}");
    assert!(!z.contains("L'"), "wide-char L-prefix leaked:\n{z}");
    // Java-style generic array construction (`new T[`) is a foreign-backend
    // pattern that must never appear here.
    assert!(!z.contains("new "), "generic-array `new` leaked:\n{z}");
    // No empty aggregate body: `= struct {` is always immediately followed by
    // fields/methods, never by a closing `};` on the next line.
    assert!(
        !z.contains("= struct {\n};"),
        "empty struct body emitted:\n{z}"
    );
}

#[test]
fn boolean_const_and_labels_are_normalized() {
    let z = emit(FULL_IDL);
    assert!(z.contains("pub const FLAG: bool = true;"), "{z}");
    // Bool-discriminated union switches on `true`/`false`, not integers.
    assert!(z.contains("true => {"), "{z}");
    assert!(z.contains("false => {"), "{z}");
}

#[test]
fn inheritance_carries_base_fields_before_derived() {
    let z = emit(FULL_IDL);
    // #A10: `Derived : Base` carries `x`, `w` (base, first) then `y` (own).
    let d = z
        .split("pub const Derived = struct {")
        .nth(1)
        .expect("Derived struct");
    let x = d.find("x: i32,").expect("base field x");
    let w = d.find("w: i32,").expect("base field w");
    let y = d.find("y: i32,").expect("own field y");
    assert!(
        x < y && w < y,
        "base fields must precede derived field:\n{z}"
    );
}

/// Zig reserved words that are also legal IDL identifiers, one per identifier
/// position we escape. Words that are themselves IDL keywords (`const`, `enum`,
/// `struct`, `union`, `switch`, …) are excluded — `try_emit` also skips any
/// that fail to parse, so the set is self-correcting.
const RESERVED_IDENT_WORDS: &[&str] = &[
    "align",
    "try",
    "error",
    "for",
    "while",
    "break",
    "defer",
    "comptime",
    "test",
    "var",
    "fn",
    "catch",
    "inline",
    "volatile",
    "resume",
    "unreachable",
    "undefined",
    "threadlocal",
];

#[test]
fn reserved_words_are_escaped_at_every_identifier_position() {
    let mut checked = 0usize;
    for w in RESERVED_IDENT_WORDS {
        // Word as: struct name, member name, enumerator name, union branch.
        let idl = format!(
            "enum E{w} {{ {w}, ZZ }};\n\
             @final struct {w} {{ long {w}; }};\n\
             @final union U{w} switch (long) {{ case 1: long {w}; }};"
        );
        let Some(z) = try_emit(&idl) else {
            continue;
        };
        let esc = format!("@\"{w}\"");
        assert!(
            z.contains(&esc),
            "reserved word `{w}` was not escaped as `{esc}`:\n{z}"
        );
        // No bare (unescaped) declaration/field token for the word.
        assert!(
            !z.contains(&format!("pub const {w} =")),
            "bare `pub const {w}` leaked:\n{z}"
        );
        assert!(
            !z.contains(&format!("    {w}:")),
            "bare field `{w}:` leaked:\n{z}"
        );

        // Word as a const name (separate spec — a const `<w>` would otherwise
        // collide with the struct `<w>` above).
        let cidl = format!("const long {w} = 1;");
        if let Some(cz) = try_emit(&cidl) {
            assert!(
                cz.contains(&esc) && !cz.contains(&format!("pub const {w}:")),
                "reserved const name `{w}` not escaped:\n{cz}"
            );
        }
        checked += 1;
    }
    assert!(checked >= 10, "too few reserved words exercised: {checked}");
}
