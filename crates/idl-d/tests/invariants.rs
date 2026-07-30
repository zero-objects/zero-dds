// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Toolchain-free structural invariants of the generated D (no `gdc` needed).
//!
//! These guard the whole class of defects the IDL-construct fix campaign found
//! in the thin backends, checked purely on the emitted source string:
//!
//! - **No duplicate top-level symbol** — every top-level `struct X`,
//!   `enum X : int` and manifest-const `enum T NAME = …;` name is unique.
//!   Catches the non-injective module flatten (#A35), interface-nested
//!   promotion (#A39) and struct-inheritance duplication (#A10).
//! - **No unescaped reserved word** as a generated field / type identifier (a
//!   bare D keyword — the emitter must append the trailing-underscore escape).
//! - **No non-compile literal**: a bare `TRUE`/`FALSE` token (#A13), an
//!   `L"…"`/`L'…'` wide-literal prefix (#A7 family), or a doubled `int[][]`-style
//!   empty temporary loop that reuses a shadowed counter (#A22).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::HashSet;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_d::{DGenOptions, generate_d_module};

/// D keywords (D Language Reference, "Lexical") — a generated identifier must
/// never appear as a bare one of these (the emitter escapes with a trailing
/// underscore).
const D_KEYWORDS: &[&str] = &[
    "abstract",
    "alias",
    "align",
    "asm",
    "assert",
    "auto",
    "body",
    "bool",
    "break",
    "byte",
    "case",
    "cast",
    "catch",
    "cdouble",
    "cent",
    "cfloat",
    "char",
    "class",
    "const",
    "continue",
    "creal",
    "dchar",
    "debug",
    "default",
    "delegate",
    "delete",
    "deprecated",
    "do",
    "double",
    "else",
    "enum",
    "export",
    "extern",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "foreach",
    "foreach_reverse",
    "function",
    "goto",
    "idouble",
    "if",
    "ifloat",
    "immutable",
    "import",
    "in",
    "inout",
    "int",
    "interface",
    "invariant",
    "ireal",
    "is",
    "lazy",
    "long",
    "macro",
    "mixin",
    "module",
    "new",
    "nothrow",
    "null",
    "out",
    "override",
    "package",
    "pragma",
    "private",
    "protected",
    "public",
    "pure",
    "real",
    "ref",
    "return",
    "scope",
    "shared",
    "short",
    "static",
    "struct",
    "super",
    "switch",
    "synchronized",
    "template",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typeof",
    "ubyte",
    "ucent",
    "uint",
    "ulong",
    "union",
    "unittest",
    "ushort",
    "version",
    "void",
    "wchar",
    "while",
    "with",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_d_module(&ast, &DGenOptions::default()).expect("gen")
}

/// Top-level declared symbol names: `struct X {`, `enum X : int {` (a real
/// enum), and `enum T NAME = …;` (a manifest const). The wire prelude's own
/// `struct Writer`/`struct Reader` are skipped (fixed, always present once).
fn top_level_symbols(d: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in d.lines() {
        // Only truly top-level (column-0) declarations; struct bodies and the
        // prelude's helpers are indented or are the fixed Writer/Reader.
        if let Some(rest) = line.strip_prefix("struct ") {
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            if name != "Writer" && name != "Reader" {
                names.push(name);
            }
        } else if let Some(rest) = line.strip_prefix("enum ") {
            // `enum Color : int {`  → type name is the token before `:`/`{`.
            // `enum int NAME = …;`  → const name is the 2nd token.
            if rest.contains('=') {
                // manifest const: `<type> <name> = …;`
                let mut it = rest.split_whitespace();
                let _ty = it.next();
                if let Some(name) = it.next() {
                    names.push(name.to_string());
                }
            } else {
                names.push(
                    rest.split([':', ' ', '{'])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                );
            }
        }
    }
    names.retain(|n| !n.is_empty());
    names
}

fn assert_no_duplicate_top_level(d: &str) {
    let mut seen = HashSet::new();
    for n in top_level_symbols(d) {
        assert!(
            seen.insert(n.clone()),
            "duplicate top-level symbol `{n}` in:\n{d}"
        );
    }
}

/// Field names inside every `struct X { … }` body: the token before `;` on a
/// simple `    <type> <name>;` line (skips methods `(`, comments and blanks).
/// Struct bodies here are flat (no nested braces at field level).
fn struct_field_names(d: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_struct = false;
    for line in d.lines() {
        if line.starts_with("struct ") && line.trim_end().ends_with('{') {
            in_struct = true;
            continue;
        }
        if in_struct {
            if line.starts_with('}') {
                in_struct = false;
                continue;
            }
            let t = line.trim();
            if !t.ends_with(';') || t.contains('(') || t.starts_with("//") {
                continue;
            }
            // `<type…> <name>;` — the field name is the last whitespace token
            // (stripped of the trailing `;`).
            let name = t
                .trim_end_matches(';')
                .split_whitespace()
                .last()
                .unwrap_or("");
            if !name.is_empty() {
                fields.push(name.to_string());
            }
        }
    }
    fields
}

fn assert_no_reserved_identifiers(d: &str) {
    let kw: HashSet<&str> = D_KEYWORDS.iter().copied().collect();
    for f in struct_field_names(d) {
        assert!(
            !kw.contains(f.as_str()),
            "reserved word `{f}` used as a struct field name in:\n{d}"
        );
    }
    for n in top_level_symbols(d) {
        assert!(
            !kw.contains(n.as_str()),
            "reserved word `{n}` used as a top-level symbol in:\n{d}"
        );
    }
}

fn assert_no_noncompile_literals(d: &str) {
    // A bare TRUE/FALSE token or an L-prefixed wide literal is not valid D.
    for line in d.lines() {
        // The manifest consts are the only place a literal value is emitted.
        if line.starts_with("enum ") && line.contains('=') {
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

fn check_all(d: &str) {
    assert_no_duplicate_top_level(d);
    assert_no_reserved_identifiers(d);
    assert_no_noncompile_literals(d);
}

#[test]
fn module_underscore_flatten_is_injective() {
    // #A35/F35: `module A_B { struct C }` and `module A { module B { struct C }}`
    // must NOT both flatten to `struct A_B_C`.
    let d = emit(
        "module A_B { struct C { long x; }; };
         module A { module B { struct C { double y; }; }; };",
    );
    check_all(&d);
    assert!(d.contains("struct A__B_C {"), "{d}");
    assert!(d.contains("struct A_B_C {"), "{d}");
}

#[test]
fn struct_inheritance_carries_base_fields_base_first() {
    // #A10/F10: base fields are inlined base-first; no duplicate `struct`.
    let d = emit(
        "@final struct Base { long a; long b; };
         @final struct Derived : Base { long c; };",
    );
    check_all(&d);
    let der = d
        .split("struct Derived {")
        .nth(1)
        .expect("Derived body")
        .split('}')
        .next()
        .unwrap();
    // a, b (inherited, base-first) then c.
    let ia = der.find("int a;").expect("a");
    let ib = der.find("int b;").expect("b");
    let ic = der.find("int c;").expect("c");
    assert!(ia < ib && ib < ic, "base-first order wrong:\n{der}");
}

#[test]
fn const_all_types_emit_valid_d_no_bad_literals() {
    // #A5/F5 + #A13/#A7: every const type emits, booleans normalized, no L".
    let d = emit(
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
    check_all(&d);
    assert!(d.contains("enum int SHIFT = (1 << 4);"), "{d}");
    assert!(d.contains("enum bool FLAG = true;"), "{d}");
    assert!(d.contains("enum bool OFF = false;"), "{d}");
    assert!(d.contains("enum string GREETING = \"hi\";"), "{d}");
    assert!(d.contains("enum ubyte BYTE = 7;"), "{d}");
    assert!(d.contains("enum wstring WS = \"hello\";"), "{d}");
    assert!(d.contains("enum wchar WC = 'y';"), "{d}");
}

#[test]
fn union_enum_char_bool_discriminators_do_not_abort() {
    // #A11/A12/A13/F11/F12/F13/P4: enum / char / boolean labels resolve.
    let d = emit(
        "enum Color { RED, GREEN, BLUE };
         @final union EU switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
         @final union CU switch (char) { case 'A': long a; case 'B': short b; };
         @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };",
    );
    check_all(&d);
    // Enum labels resolve to ordinals.
    assert!(
        d.contains("if (disc == 0)") && d.contains("else if (disc == 1)"),
        "{d}"
    );
    // Char label 'A' -> 65, 'B' -> 66.
    assert!(
        d.contains("if (disc == 65)") && d.contains("else if (disc == 66)"),
        "{d}"
    );
    // Boolean labels -> D true/false, not integers (`disc` is a `bool`).
    assert!(
        d.contains("if (disc == true)") && d.contains("else if (disc == false)"),
        "{d}"
    );
}

#[test]
fn mutable_union_emits_emheader_framing() {
    // #A16/F14: a @mutable union is no longer rejected — it emits an
    // EMHEADER-framed member list (disc = member id 0, branches 1-based).
    let d = emit("@mutable union MutU switch (long) { case 1: long a; default: short b; };");
    check_all(&d);
    // Discriminator frame = member id 0 with LC4.
    assert!(d.contains("zdBody.putU32(0x40000000u);"), "{d}");
    // Branches are 1-based member ids.
    assert!(d.contains("zdBody.putU32(0x40000001u);"), "{d}");
    assert!(d.contains("zdBody.putU32(0x40000002u);"), "{d}");
    // Decode reads a per-member EMHEADER+NEXTINT.
    assert!(d.contains("r.getU32(); // EMHEADER"), "{d}");
}

#[test]
fn mutable_struct_sets_must_understand_bit() {
    // #A17/F17: a @must_understand member's EMHEADER carries bit 31 (0x8…),
    // while a plain member keeps LC4 only (0x4…).
    let d = emit("@mutable struct S { @id(1) long x; @must_understand @id(2) string s; };");
    check_all(&d);
    assert!(
        d.contains("zdBody.putU32(0x40000001u);"),
        "plain member: {d}"
    );
    assert!(
        d.contains("zdBody.putU32(0xc0000002u);"),
        "must-understand member (0x80000000|0x40000000|2): {d}"
    );
}

#[test]
fn nested_sequence_and_map_temporaries_are_depth_unique() {
    // #A22/F22: nested sequence/map decoders and encoders must not reuse the
    // same D counter/length/key/value name (a re-declaration shadows and the
    // inner loop would re-index with the outer counter — a D compile error).
    let d = emit(
        "@final struct S { sequence<sequence<long> > ss; map<string, map<string, long> > mm; };",
    );
    check_all(&d);
    // Decode: distinct loop counter + length per depth.
    assert!(
        d.contains("foreach (i; 0 .. zdn)") && d.contains("foreach (i1; 0 .. zdn1)"),
        "{d}"
    );
    // Decode map: distinct key/value holders per depth.
    assert!(d.contains("string zk;") && d.contains("string zk1;"), "{d}");
    // Encode: distinct element / sub-writer names per depth.
    assert!(
        d.contains("foreach (zdElem; ss)") && d.contains("foreach (zdElem1; zdElem)"),
        "{d}"
    );
    assert!(
        d.contains("auto zdKeys = ") && d.contains("auto zdKeys1 = "),
        "{d}"
    );
}

#[test]
fn interface_nested_types_survive() {
    // #A39/F39: the interface body's nested struct must be emitted, not dropped
    // with the interface body.
    let d = emit(
        "interface Calculator { struct Config { long precision; }; long add(in long a, in long b); };",
    );
    check_all(&d);
    assert!(d.contains("struct Calculator_Config {"), "{d}");
    assert!(d.contains("int precision;"), "{d}");
}

#[test]
fn keyword_named_members_are_escaped() {
    // D keywords as IDL member names get the trailing-underscore escape; no
    // bare reserved identifier survives.
    let d = emit("@final struct S { long scope; long version; long body; long ref; };");
    check_all(&d);
    assert!(d.contains("int scope_;"), "{d}");
    assert!(d.contains("int version_;"), "{d}");
    assert!(d.contains("int body_;"), "{d}");
    assert!(d.contains("int ref_;"), "{d}");
}
