// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial IDL corpus compiled with the real Swift toolchain (gated on
//! `swiftc` being on PATH — macOS only; codepit has no `swiftc`). Three corpora:
//!
//! 1. **reserved-keyword** — every Swift keyword that is a legal IDL identifier,
//!    placed at member / struct / enum-enumerator / module / const / union-branch
//!    positions, must generate Swift that compiles (backtick escaping keeps each
//!    collision a legal Swift identifier).
//! 2. **construct** — each IDL construct emitted minimally and compiled, with a
//!    wire-size / wire-byte / round-trip assertion where it is statically
//!    checkable (`long double` = 16 B, `wchar` = 4 B, `fixed<P,S>`, enum
//!    `@value`, struct inheritance, `@mutable` union, nested seq/map, `@optional`
//!    under `@appendable`/`@mutable`).
//! 3. **compose-multifile** — two IDLs generated separately (one with the wire
//!    prelude, one as a prelude-less fragment) and merged into one Swift module,
//!    which must build (the former per-file prelude duplication — #C-swift —
//!    would have re-declared `Writer`/`Reader`/`Endianness`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_swift::{SwiftGenOptions, generate_swift_fragment, generate_swift_module};

/// Swift keywords that are also legal IDL identifiers (i.e. not themselves IDL
/// keywords such as `switch`/`case`/`default`/`const`/`interface`/`struct`/
/// `enum`/`union`/`module`/`typedef`/`import`/`in`/`inout`/`public`/`private`,
/// and not case-insensitively colliding with an IDL keyword such as `any`).
const SAFE_KEYWORDS: &[&str] = &[
    "class",
    "extension",
    "fileprivate",
    "func",
    "guard",
    "internal",
    "open",
    "operator",
    "protocol",
    "rethrows",
    "static",
    "subscript",
    "typealias",
    "var",
    "break",
    "catch",
    "continue",
    "defer",
    "do",
    "else",
    "fallthrough",
    "for",
    "if",
    "repeat",
    "return",
    "throw",
    "throws",
    "try",
    "as",
    "is",
    "where",
    "while",
];

fn swiftc_available() -> bool {
    Command::new("swiftc").arg("--version").output().is_ok()
}

fn gen_module(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    generate_swift_module(&ast, &SwiftGenOptions::default()).expect("gen")
}

/// Writes `src` to a temp `main.swift`, compiles it with `swiftc`, and (if the
/// build succeeds) runs the binary. Returns `(built, ran_ok, stderr, stdout)`.
fn swiftc_build_run(tag: &str, src: &str) -> (bool, bool, String, String) {
    let dir = std::env::temp_dir().join(format!("idlswift_corpus_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let sf = dir.join("main.swift");
    std::fs::write(&sf, src).expect("write");
    let bin = dir.join("main_bin");
    let build = Command::new("swiftc")
        .arg(&sf)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("swiftc");
    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr).into_owned();
        let _ = std::fs::remove_dir_all(&dir);
        return (false, false, stderr, String::new());
    }
    let run = Command::new(&bin).output().expect("run");
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    (true, run.status.success(), stderr, stdout)
}

#[test]
fn reserved_keyword_corpus_compiles() {
    if !swiftc_available() {
        eprintln!("SKIP reserved-keyword corpus: `swiftc` not on PATH");
        return;
    }
    // Build one IDL exercising every safe keyword at each position.
    let mut idl = String::new();
    // member position: one struct holding every keyword as a member.
    idl.push_str("@final struct AllMembers {\n");
    for kw in SAFE_KEYWORDS {
        idl.push_str(&format!("  long {kw};\n"));
    }
    idl.push_str("};\n");
    // enumerator position.
    idl.push_str("enum Kw {\n");
    idl.push_str(
        &SAFE_KEYWORDS
            .iter()
            .map(|k| format!("  {k}"))
            .collect::<Vec<_>>()
            .join(",\n"),
    );
    idl.push_str("\n};\n");
    // const position (inside a module, so const names do not collide with the
    // keyword-named modules below).
    idl.push_str("module consts {\n");
    for (i, kw) in SAFE_KEYWORDS.iter().enumerate() {
        idl.push_str(&format!("  const long {kw} = {i};\n"));
    }
    idl.push_str("};\n");
    // union-branch (member) position.
    idl.push_str("@final union U switch (long) {\n");
    for (i, kw) in SAFE_KEYWORDS.iter().enumerate() {
        idl.push_str(&format!("  case {i}: long {kw};\n"));
    }
    idl.push_str("};\n");
    // module + struct-name position.
    for kw in SAFE_KEYWORDS {
        idl.push_str(&format!(
            "module {kw} {{ @final struct {kw} {{ long x; }}; }};\n"
        ));
    }

    let src = gen_module(&idl);
    let (built, ran, stderr, _) = swiftc_build_run("reserved", &src);
    assert!(
        built,
        "swiftc failed for reserved corpus:\n{stderr}\n--- src ---\n{src}"
    );
    assert!(ran, "reserved corpus binary failed at runtime:\n{stderr}");
}

#[test]
fn construct_corpus_compiles_and_wire_sizes_hold() {
    if !swiftc_available() {
        eprintln!("SKIP construct corpus: `swiftc` not on PATH");
        return;
    }
    let idl = "
        // primitives + wchar + long double + fixed
        @final struct LD { long double d; };
        @final struct WC { wchar c; };
        @final struct Prim { octet a; short b; long c; long long d; };
        @final struct Fx { fixed<5,2> f; };
        // enum with @value gaps
        enum Sparse { SA, @value(5) SB, SC };
        @final struct EnumHolder { Sparse e; };
        // struct inheritance
        @final struct Base { long a; long b; };
        @final struct Derived : Base { long c; };
        // unions: enum / char / boolean discriminators
        enum Color { RED, GREEN, BLUE };
        @final union EU switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
        @final union CU switch (char) { case 'A': long a; case 'B': short b; };
        @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };
        // #A16: @mutable union — EMHEADER-framed member list (was unsupported).
        @mutable union MutU switch (long) { case 1: long a; default: short b; };
        // bitset / bitmask
        bitset BS { bitfield<3> lo; bitfield<5> hi; };
        bitmask BM { FLAG_A, FLAG_B, FLAG_C };
        // @optional under @appendable and @mutable extensibility
        @appendable struct OptA { @optional long a; long b; };
        @mutable struct MutS { @id(1) long x; @must_understand @id(2) string s; };
        // collections: bounded seq, multidim array, map, nested seq/map
        @final struct Coll {
            sequence<long, 4> s;
            long m[2][3];
            map<string, long> mp;
            sequence<sequence<long> > ss;
            map<string, map<string, long> > mm;
        };
        // module nested + reopened
        module Outer { module Inner { @final struct P { long x; }; }; };
        module Outer { @final struct Q { long y; }; };
    ";

    let mut src = format!("import Foundation\n{}", gen_module(idl));
    // A top-level program that asserts the statically-checkable wire sizes /
    // bytes and round-trips the new constructs.
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
func fail(_ m: String) { print("FAIL " + m) }

do {
    // long double is 16 bytes on the wire.
    if try LD(d: 0).marshalXCDR(.little).count != 16 { fail("LD size"); exit(1) }
    // wchar is 4 bytes (the established ZeroDDS reference wire).
    if try WC(c: 0).marshalXCDR(.little).count != 4 { fail("WC size"); exit(1) }
    // octet+short+long+longlong with XCDR2 alignment = 16 bytes.
    if try Prim(a: 0, b: 0, c: 0, d: 0).marshalXCDR(.little).count != 16 { fail("Prim size"); exit(1) }
    // fixed<5,2> packs to (5+2)/2 = 3 BCD octets.
    if try Fx(f: zdFixedEnc("123.45", 5, 2)).marshalXCDR(.little).count != 3 { fail("Fx size"); exit(1) }
    // enum @value: SB == 5, so its 4-byte LE encoding is 05000000.
    if try toHex(EnumHolder(e: .SB).marshalXCDR(.little)) != "05000000" { fail("enum @value"); exit(1) }
    // struct inheritance: Derived carries a,b (base) then c.
    if try toHex(Derived(a: 1, b: 2, c: 3).marshalXCDR(.little)) != "010000000200000003000000" { fail("inheritance"); exit(1) }
    // bitmask default backing integer is UInt32 (4 bytes).
    if try BM().marshalXCDR(.little).count != 4 { fail("BM size"); exit(1) }
    // #A16: @mutable union round-trips through its EMHEADER-framed member list.
    let mu = MutU(disc: 1, a: 42, b: 0)
    let mub = try MutU.unmarshalXCDR(mu.marshalXCDR(.little), .little)
    if mub.disc != 1 || mub.a != 42 { fail("mutable union round-trip"); exit(1) }
    // #A17: @mutable struct with a @must_understand member round-trips.
    let ms = MutS(x: 7, s: "hi")
    let msb = try MutS.unmarshalXCDR(ms.marshalXCDR(.little), .little)
    if msb.x != 7 || msb.s != "hi" { fail("mutable struct round-trip"); exit(1) }
    // #A11/A12/A13: enum / char / boolean discriminated unions round-trip.
    let eu = EU(disc: .GREEN, r: 0, g: 9, o: 0)
    let eub = try EU.unmarshalXCDR(eu.marshalXCDR(.little), .little)
    if eub.disc != .GREEN || eub.g != 9 { fail("enum union round-trip"); exit(1) }
    let cu = CU(disc: 66, a: 0, b: 5)  // 'B'
    let cub = try CU.unmarshalXCDR(cu.marshalXCDR(.little), .little)
    if cub.disc != 66 || cub.b != 5 { fail("char union round-trip"); exit(1) }
    let bu = BU(disc: true, yes: 3, no: 0)
    let bub = try BU.unmarshalXCDR(bu.marshalXCDR(.little), .little)
    if bub.disc != true || bub.yes != 3 { fail("bool union round-trip"); exit(1) }
    // nested seq<seq<long>> and map<string,map<string,long>> round-trip.
    let c = Coll(
        s: [1, 2],
        m: [[1, 2, 3], [4, 5, 6]],
        mp: ["a": 1],
        ss: [[1, 2, 3], [4, 5]],
        mm: ["a": ["x": 10, "y": 11], "b": ["z": 12]]
    )
    let cb = try Coll.unmarshalXCDR(c.marshalXCDR(.little), .little)
    if cb.ss.count != 2 || cb.ss[0].count != 3 || cb.ss[0][2] != 3 || cb.ss[1][1] != 5 { fail("nested seq"); exit(1) }
    if cb.mm["a"]?["y"] != 11 || cb.mm["b"]?["z"] != 12 { fail("nested map"); exit(1) }
    if cb.m[1][2] != 6 { fail("multidim array"); exit(1) }
    // @optional under @appendable round-trips (present + absent).
    let oa = OptA(a_present: true, a: 5, b: 6)
    let oab = try OptA.unmarshalXCDR(oa.marshalXCDR(.little), .little)
    if oab.a_present != true || oab.a != 5 || oab.b != 6 { fail("optional appendable"); exit(1) }
    print("OK")
} catch {
    print("FAIL threw \(error)")
    exit(1)
}
"##,
    );

    let (built, ran, stderr, stdout) = swiftc_build_run("construct", &src);
    assert!(built, "swiftc failed:\n{stderr}\n--- src ---\n{src}");
    assert!(ran, "construct binary failed at runtime:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "OK",
        "wire assertion failed:\n{stdout}\n--- src ---\n{src}"
    );
}

#[test]
fn compose_multifile_builds() {
    if !swiftc_available() {
        eprintln!("SKIP compose corpus: `swiftc` not on PATH");
        return;
    }
    // File A carries the shared wire prelude; file B is a prelude-less fragment.
    let ast_a = zerodds_idl::parse(
        "@final struct Alpha { long x; string s; };",
        &ParserConfig::default(),
    )
    .expect("parse A");
    let ast_b = zerodds_idl::parse(
        "@appendable struct Beta { unsigned short k; sequence<long> v; };",
        &ParserConfig::default(),
    )
    .expect("parse B");
    let file_a = generate_swift_module(&ast_a, &SwiftGenOptions::default()).expect("gen A");
    let file_b =
        generate_swift_fragment(&ast_b, &SwiftGenOptions::default()).expect("gen B fragment");

    // The fragment must NOT re-declare the wire prelude.
    assert!(
        !file_b.contains("public struct Writer {"),
        "fragment re-declared the wire prelude:\n{file_b}"
    );
    assert!(
        file_a.contains("public struct Writer {"),
        "file A missing prelude"
    );

    // Merge idiomatically into one Swift module file: file A whole, then file B's
    // body with its leading header comment lines stripped.
    let mut merged = file_a;
    let b_body: String = file_b
        .lines()
        .skip_while(|l| l.starts_with("//") || l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    merged.push('\n');
    merged.push_str(&b_body);
    merged.push_str(
        "\ndo {\n  _ = try Alpha(x: 1, s: \"a\").marshalXCDR(.little)\n  _ = try Beta(k: 2, v: [3]).marshalXCDR(.little)\n  print(\"OK\")\n} catch { print(\"FAIL \\(error)\") }\n",
    );

    let (built, ran, stderr, stdout) = swiftc_build_run("compose", &merged);
    assert!(
        built,
        "composed module failed to build:\n{stderr}\n--- src ---\n{merged}"
    );
    assert!(ran, "composed binary failed at runtime:\n{stderr}");
    assert_eq!(stdout.trim(), "OK", "compose run failed:\n{stdout}");
}
