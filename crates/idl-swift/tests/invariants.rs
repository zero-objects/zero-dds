// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the generated Swift (no `swiftc`
//! needed).
//!
//! These guard the whole class of defects the IDL-construct fix campaign found
//! in the thin backends, checked purely on the emitted source string:
//!
//! - **No duplicate top-level symbol** — every `public struct X`,
//!   `public enum X` and `public let X` name is unique. Catches an
//!   interface-nested promotion (#A39) or struct-inheritance (#A10) collision.
//! - **No unescaped reserved word** as a generated identifier (a Swift keyword
//!   used bare as a type / field / const name; the escape wraps it in
//!   backticks).
//! - **No non-compile literal**: a bare `TRUE`/`FALSE` token (#A13) or an
//!   `L"…"`/`L'…'` wide-literal prefix (#A7 family) in a `const` value.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::HashSet;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_swift::{SwiftGenOptions, generate_swift_module};

/// A representative subset of Swift keywords that would break the build if a
/// generated identifier used them bare (the emitter must backtick-escape them).
const SWIFT_KEYWORDS: &[&str] = &[
    "class",
    "enum",
    "struct",
    "func",
    "let",
    "var",
    "switch",
    "case",
    "default",
    "for",
    "while",
    "if",
    "else",
    "return",
    "public",
    "private",
    "internal",
    "static",
    "import",
    "protocol",
    "extension",
    "in",
    "where",
    "guard",
    "defer",
    "repeat",
    "throw",
    "throws",
    "try",
    "as",
    "is",
    "self",
    "super",
    "init",
    "deinit",
    "operator",
    "subscript",
    "typealias",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_swift_module(&ast, &SwiftGenOptions::default()).expect("gen")
}

/// Top-level declared symbol names: `public struct <name>`, `public enum
/// <name>` and `public let <name>` (all emitted at column 0; methods live
/// inside a struct/enum body, indented, and are skipped).
fn top_level_symbols(swift: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in swift.lines() {
        let name = if let Some(rest) = line.strip_prefix("public struct ") {
            rest.split_whitespace().next()
        } else if let Some(rest) = line.strip_prefix("public enum ") {
            // `public enum Name: Int32 {` — the name ends at `:` or whitespace.
            rest.split([':', ' ']).next()
        } else if let Some(rest) = line.strip_prefix("public let ") {
            rest.split([':', ' ']).next()
        } else {
            None
        };
        if let Some(n) = name {
            if !n.is_empty() {
                names.push(n.to_string());
            }
        }
    }
    names
}

/// Struct/enum member identifiers: the token after `public var ` /
/// `public static let ` / `case `, with any trailing `_present` companion and
/// enum `= value` suffix stripped.
fn member_identifiers(swift: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in swift.lines() {
        let t = line.trim_start();
        let ident = if let Some(rest) = t.strip_prefix("public var ") {
            rest.split([':', ' ']).next()
        } else if let Some(rest) = t.strip_prefix("case ") {
            // Enum case (`case FOO = 0`) — but NOT a switch `case 3:` (those
            // start with a digit / backtick-free integer and are skipped by the
            // reserved-word check anyway).
            rest.split([' ', ':', ',']).next()
        } else {
            None
        };
        if let Some(id) = ident {
            let id = id.trim_end_matches("_present");
            if !id.is_empty() {
                ids.push(id.to_string());
            }
        }
    }
    ids
}

fn assert_no_duplicate_top_level(swift: &str) {
    let names = top_level_symbols(swift);
    let mut seen = HashSet::new();
    for n in &names {
        assert!(
            seen.insert(n.clone()),
            "duplicate top-level symbol `{n}` in:\n{swift}"
        );
    }
}

fn assert_no_unescaped_reserved(swift: &str) {
    // A generated identifier that equals a Swift keyword must be backtick-
    // wrapped; a bare keyword identifier would not compile.
    for id in top_level_symbols(swift)
        .into_iter()
        .chain(member_identifiers(swift))
    {
        // Backtick-wrapped identifiers carry the leading backtick and are safe.
        if id.starts_with('`') {
            continue;
        }
        assert!(
            !SWIFT_KEYWORDS.contains(&id.as_str()),
            "reserved word `{id}` used bare as a generated identifier in:\n{swift}"
        );
    }
}

fn assert_no_noncompile_literals(swift: &str) {
    // A `const` value must never carry a bare TRUE/FALSE token or an L-prefixed
    // wide literal (both invalid Swift).
    for line in swift.lines() {
        if line.starts_with("public let ") {
            assert!(
                !line.contains(" TRUE") && !line.contains("=TRUE") && !line.contains(" FALSE"),
                "TRUE/FALSE literal leaked into a const:\n{line}"
            );
            assert!(
                !line.contains("L\"") && !line.contains("L'"),
                "L-prefixed wide literal leaked into a const:\n{line}"
            );
        }
    }
}

fn check_all(swift: &str) {
    assert_no_duplicate_top_level(swift);
    assert_no_unescaped_reserved(swift);
    assert_no_noncompile_literals(swift);
}

#[test]
fn struct_inheritance_no_duplicate_and_carries_base_fields() {
    // #A10: base fields are inlined base-first; no duplicate `struct`.
    let s = emit(
        "@final struct Base { long a; long b; };
         @final struct Derived : Base { long c; };",
    );
    check_all(&s);
    assert!(s.contains("public struct Derived {"), "{s}");
    // Derived carries a, b (inherited) then c.
    let der = s
        .split("public struct Derived {")
        .nth(1)
        .expect("Derived body");
    let body = der.split("\n    public func").next().unwrap();
    assert!(body.contains("public var a: Int32"), "{s}");
    assert!(body.contains("public var b: Int32"), "{s}");
    assert!(body.contains("public var c: Int32"), "{s}");
}

#[test]
fn const_all_types_emit_valid_swift_no_bad_literals() {
    // #A5/P1 + #A13/#A7: every const type emits, booleans normalized, no L"/L'.
    let s = emit(
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
    check_all(&s);
    assert!(s.contains("public let SHIFT: Int32 = (1 << 4)"), "{s}");
    assert!(s.contains("public let FLAG: Bool = true"), "{s}");
    assert!(s.contains("public let OFF: Bool = false"), "{s}");
    assert!(s.contains("public let GREETING: String = \"hi\""), "{s}");
    assert!(s.contains("public let WS: String = \"hello\""), "{s}");
    // char/wchar consts render as their code point (they map to Swift ints).
    assert!(s.contains("public let CH: UInt8 = 120"), "{s}"); // 'x'
    assert!(s.contains("public let WC: UInt32 = 121"), "{s}"); // 'y'
}

#[test]
fn union_enum_char_bool_discriminators_do_not_abort() {
    // #A11/A12/A13/P4: enum / char / boolean labels resolve, no early error.
    let s = emit(
        "enum Color { RED, GREEN, BLUE };
         @final union EU switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
         @final union CU switch (char) { case 'A': long a; case 'B': short b; };
         @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };",
    );
    check_all(&s);
    // Enum labels resolve to ordinals, dispatched on the enum's rawValue.
    assert!(s.contains("switch disc.rawValue {"), "{s}");
    assert!(s.contains("case 0:") && s.contains("case 1:"), "{s}");
    // Char label 'A' -> 65.
    assert!(s.contains("case 65:"), "{s}");
    // Boolean labels -> Swift true/false, not integers.
    assert!(s.contains("case true:") && s.contains("case false:"), "{s}");
}

#[test]
fn interface_nested_types_survive() {
    // #A39: the interface body's nested struct must be emitted, not dropped.
    let s = emit(
        "interface Calculator { struct Config { long precision; }; long add(in long a, in long b); };",
    );
    check_all(&s);
    assert!(s.contains("public struct Calculator_sConfig {"), "{s}");
    assert!(s.contains("public var precision: Int32"), "{s}");
}

#[test]
fn mutable_member_sets_must_understand_bit() {
    // #A17: a @must_understand member's EMHEADER carries bit 31 (0x8...); a
    // plain member keeps the LC4-only 0x4000_0000|id form.
    let s = emit("@mutable struct M { @id(1) long a; @must_understand @id(2) long b; };");
    check_all(&s);
    // id=1, no MU: 0x40000001. id=2, MU: 0x80000000|0x40000000|2 = 0xc0000002.
    assert!(s.contains("body.putU32(0x40000001)"), "{s}");
    assert!(s.contains("body.putU32(0xc0000002)"), "{s}");
}

#[test]
fn mutable_union_emits_emheader_framing() {
    // #A16: a @mutable union frames its discriminator (member id 0) and each
    // branch (1-based id) as LC4 EMHEADER + NEXTINT, wrapped in a DHEADER.
    let s = emit("@mutable union MutU switch (long) { case 1: long a; default: short b; };");
    check_all(&s);
    // Discriminator = member id 0 (LC4): 0x40000000.
    assert!(s.contains("body.putU32(0x40000000)"), "{s}");
    // First branch = member id 1: 0x40000001.
    assert!(s.contains("body.putU32(0x40000001)"), "{s}");
    // DHEADER wrap.
    assert!(s.contains("w.putU32(UInt32(zdBB.count))"), "{s}");
}

#[test]
fn keyword_named_members_are_backtick_escaped() {
    // Swift keywords that are legal IDL identifiers (i.e. not themselves IDL
    // keywords) must be backtick-wrapped as Swift member names, never bare.
    let s = emit("@final struct S { long class; long func; long guard; long defer; };");
    check_all(&s);
    assert!(s.contains("public var `class`: Int32"), "{s}");
    assert!(s.contains("public var `func`: Int32"), "{s}");
    assert!(s.contains("public var `guard`: Int32"), "{s}");
    assert!(s.contains("public var `defer`: Int32"), "{s}");
}
