// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial IDL corpus compiled with the real D toolchain (gated on `gdc`
//! being on PATH). Three corpora:
//!
//! 1. **reserved-keyword** — every D keyword that is a legal IDL identifier,
//!    placed at member / struct / enum-enumerator / module / const /
//!    union-branch positions, must generate D that compiles (`escape_d_ident`
//!    appends the trailing-underscore escape).
//! 2. **construct** — each IDL construct emitted minimally and compiled, with a
//!    wire-size / wire-byte assertion where it is statically checkable
//!    (`long double` = 16 B, `wchar` = 4 B, `fixed<P,S>`, enum `@value`, struct
//!    inheritance, enum/char/bool + `@mutable` unions, nested seq/map, bitset,
//!    bitmask, `@optional`, module nested + reopened).
//! 3. **compose-multifile** — two IDLs generated separately, each wrapped in its
//!    own D `module` (the idiomatic D multi-file unit — every generated file
//!    carries the full wire prelude, so distinct module namespaces keep the two
//!    `Writer`/`Reader` copies from colliding), then compiled together.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_d::{DGenOptions, generate_d_module};

/// D keywords that are also legal IDL identifiers (i.e. not themselves an IDL
/// keyword such as `long`/`short`/`const`/`enum`/`union`/`struct`/`module`/
/// `switch`/`case`/`default`/`in`/`out`/`interface`/`typedef`/`import`/`char`/
/// `void`/`wchar`/`double`/`float`/`public`/`private`). Verified accepted by the
/// IDL front-end at every declaration position.
const SAFE_KEYWORDS: &[&str] = &[
    "asm",
    "assert",
    "auto",
    "body",
    "bool",
    "break",
    "byte",
    "cast",
    "catch",
    "cdouble",
    "cent",
    "cfloat",
    "class",
    "continue",
    "creal",
    "dchar",
    "debug",
    "delegate",
    "delete",
    "deprecated",
    "do",
    "else",
    "export",
    "extern",
    "final",
    "finally",
    "for",
    "foreach",
    "foreach_reverse",
    "function",
    "goto",
    "idouble",
    "if",
    "ifloat",
    "immutable",
    "invariant",
    "ireal",
    "is",
    "lazy",
    "macro",
    "mixin",
    "new",
    "nothrow",
    "null",
    "override",
    "package",
    "pragma",
    "protected",
    "pure",
    "real",
    "ref",
    "return",
    "scope",
    "shared",
    "static",
    "super",
    "synchronized",
    "template",
    "this",
    "throw",
    "try",
    "typeof",
    "ubyte",
    "ucent",
    "uint",
    "ulong",
    "unittest",
    "ushort",
    "version",
    "while",
    "with",
];

fn gdc_available() -> bool {
    Command::new("gdc").arg("--version").output().is_ok()
}

fn emit(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    generate_d_module(&ast, &DGenOptions::default()).expect("gen")
}

/// Compiles `files` (`(name, source)`) together with `gdc`, runs the resulting
/// binary, and returns `(ok, stderr, stdout)`. A unique tag keeps parallel test
/// cases from colliding on disk.
fn gdc_build_run(tag: &str, files: &[(&str, &str)]) -> (bool, String, String) {
    let dir = std::env::temp_dir().join(format!("idld_corpus_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    for (name, src) in files {
        std::fs::write(dir.join(name), src).expect("write .d");
    }
    let mut args: Vec<String> = files.iter().map(|(n, _)| (*n).to_string()).collect();
    args.push("-o".to_string());
    args.push("corpus_bin".to_string());
    let build = Command::new("gdc")
        .args(&args)
        .current_dir(&dir)
        .output()
        .expect("gdc");
    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr).into_owned();
        let _ = std::fs::remove_dir_all(&dir);
        return (false, stderr, String::new());
    }
    let run = Command::new("./corpus_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
    let _ = std::fs::remove_dir_all(&dir);
    (
        run.status.success(),
        String::from_utf8_lossy(&run.stderr).into_owned(),
        String::from_utf8_lossy(&run.stdout).into_owned(),
    )
}

#[test]
fn reserved_keyword_corpus_compiles() {
    if !gdc_available() {
        eprintln!("SKIP reserved-keyword corpus: `gdc` not on PATH");
        return;
    }
    let mut idl = String::new();
    // member position.
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
    // const position (global scope, so the bare keyword is escaped to `<kw>_`).
    for (i, kw) in SAFE_KEYWORDS.iter().enumerate() {
        idl.push_str(&format!("const long {kw} = {i};\n"));
    }
    // union-branch position.
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

    let mut src = emit(&idl);
    src.push_str("\nvoid main() {}\n");
    let (ok, stderr, _) = gdc_build_run("reserved", &[("main.d", &src)]);
    assert!(ok, "gdc build/run failed:\n{stderr}\n--- src ---\n{src}");
}

#[test]
fn construct_corpus_compiles_and_wire_sizes_hold() {
    if !gdc_available() {
        eprintln!("SKIP construct corpus: `gdc` not on PATH");
        return;
    }
    let idl = "
        @final struct LD { long double d; };
        @final struct WC { wchar c; };
        @final struct Prim { octet a; short b; long c; long long d; };
        @final struct Fx { fixed<5,2> f; };
        enum Sparse { SA, @value(5) SB, SC };
        @final struct EnumHolder { Sparse e; };
        @final struct Base { long a; long b; };
        @final struct Derived : Base { long c; };
        enum Color { RED, GREEN, BLUE };
        @final union EU switch (Color) { case RED: long r; case GREEN: short g; default: octet o; };
        @final union CU switch (char) { case 'A': long a; case 'B': short b; };
        @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };
        @mutable union MutU switch (long) { case 1: long a; default: short b; };
        bitset BS { bitfield<3> lo; bitfield<5> hi; };
        bitmask BM { FLAG_A, FLAG_B, FLAG_C };
        @appendable struct OptA { @optional long a; long b; };
        @mutable struct MutS { @id(1) long x; @must_understand @id(2) string s; };
        @final struct Coll {
            sequence<long, 4> s;
            long m[2][2];
            map<string, long> mp;
            sequence<sequence<long> > ss;
            map<string, map<string, long> > mm;
        };
        module Outer { module Inner { @final struct P { long x; }; }; };
        module Outer { @final struct Q { long y; }; };
    ";
    let mut src = emit(idl);
    src.push_str(
        r#"
void main() {
    import std.stdio : writeln;
    alias E = Endian;
    // long double is 16 bytes on the wire.
    { LD v; if ((v).marshalXCDR(E.LE).length != 16) { writeln("FAIL LD"); return; } }
    // wchar is the established 4-byte ZeroDDS reference wire (A1 unchanged).
    { WC v; if ((v).marshalXCDR(E.LE).length != 4) { writeln("FAIL WC"); return; } }
    // octet+short+long+longlong with XCDR2 alignment = 16 bytes.
    { Prim v; if ((v).marshalXCDR(E.LE).length != 16) { writeln("FAIL Prim"); return; } }
    // fixed<5,2> packs to (5+2)/2 = 3 BCD octets.
    { Fx v; v.f = zdFixedEnc("123.45", 5, 2); if ((v).marshalXCDR(E.LE).length != 3) { writeln("FAIL Fx"); return; } }
    // enum @value: SB == 5, so its 4-byte LE encoding is 05 00 00 00.
    { EnumHolder v; v.e = Sparse.SB; auto b = v.marshalXCDR(E.LE);
      if (b != [cast(ubyte)5, 0, 0, 0]) { writeln("FAIL enum @value"); return; } }
    // struct inheritance: Derived carries a,b (base) then c, base-first.
    { Derived v; v.a = 1; v.b = 2; v.c = 3; auto b = v.marshalXCDR(E.LE);
      if (b != [cast(ubyte)1,0,0,0, 2,0,0,0, 3,0,0,0]) { writeln("FAIL inheritance"); return; } }
    // enum-discriminated union round-trips.
    { EU v; v.disc = Color.GREEN; v.g = 7; auto back = UnmarshalXCDREU(v.marshalXCDR(E.LE), E.LE);
      if (back.disc != Color.GREEN || back.g != 7) { writeln("FAIL EU"); return; } }
    // char-discriminated union round-trips ('B' == 66).
    { CU v; v.disc = 'B'; v.b = 9; auto back = UnmarshalXCDRCU(v.marshalXCDR(E.LE), E.LE);
      if (back.disc != 'B' || back.b != 9) { writeln("FAIL CU"); return; } }
    // boolean-discriminated union round-trips.
    { BU v; v.disc = false; v.no = 5; auto back = UnmarshalXCDRBU(v.marshalXCDR(E.LE), E.LE);
      if (back.disc != false || back.no != 5) { writeln("FAIL BU"); return; } }
    // @mutable union round-trips through its EMHEADER-framed member list.
    { MutU v; v.disc = 1; v.a = 42; auto back = UnmarshalXCDRMutU(v.marshalXCDR(E.LE), E.LE);
      if (back.disc != 1 || back.a != 42) { writeln("FAIL MutU"); return; } }
    // bitmask default backing integer is uint32 (4 bytes).
    { BM v; if ((v).marshalXCDR(E.LE).length != 4) { writeln("FAIL BM"); return; } }
    // @optional round-trips present and absent.
    { OptA v; v.a_present = true; v.a = 3; v.b = 4; auto back = UnmarshalXCDROptA(v.marshalXCDR(E.LE), E.LE);
      if (!back.a_present || back.a != 3 || back.b != 4) { writeln("FAIL OptA present"); return; }
      OptA w; w.a_present = false; w.b = 9; auto back2 = UnmarshalXCDROptA(w.marshalXCDR(E.LE), E.LE);
      if (back2.a_present || back2.b != 9) { writeln("FAIL OptA absent"); return; } }
    // @mutable struct with a @must_understand member round-trips.
    { MutS v; v.x = 11; v.s = "hi"; auto back = UnmarshalXCDRMutS(v.marshalXCDR(E.LE), E.LE);
      if (back.x != 11 || back.s != "hi") { writeln("FAIL MutS"); return; } }
    // #A22: nested sequence<sequence<long>> must round-trip (the former
    // shadowed `zdn`/`i` would not compile / re-indexed with the outer counter).
    { Coll c; c.ss = [[1,2,3],[4,5]]; c.mm = ["a":["x":10,"y":11],"b":["z":12]];
      auto back = UnmarshalXCDRColl(c.marshalXCDR(E.LE), E.LE);
      if (back.ss.length != 2 || back.ss[0][2] != 3 || back.ss[1][1] != 5) { writeln("FAIL nested seq"); return; }
      // #A22: nested map<string,map<string,long>> must round-trip.
      if (back.mm["a"]["y"] != 11 || back.mm["b"]["z"] != 12) { writeln("FAIL nested map"); return; } }
    writeln("ok");
}
"#,
    );
    let (ok, stderr, stdout) = gdc_build_run("construct", &[("main.d", &src)]);
    assert!(ok, "gdc build/run failed:\n{stderr}\n--- src ---\n{src}");
    assert_eq!(stdout.trim(), "ok", "wire assertion failed:\n{src}");
}

#[test]
fn compose_multifile_builds() {
    if !gdc_available() {
        eprintln!("SKIP compose corpus: `gdc` not on PATH");
        return;
    }
    // Two IDLs generated separately. Each generated file carries the full wire
    // prelude, so they are placed in distinct D modules (the idiomatic D
    // compilation unit) — the two `Writer`/`Reader` copies then live in
    // `zdgen_a.*` / `zdgen_b.*` and never collide.
    let gen_a = emit("@final struct Alpha { long x; string s; };");
    let gen_b = emit("@appendable struct Beta { unsigned short k; sequence<long> v; };");
    let file_a = format!("module zdgen_a;\n{gen_a}");
    let file_b = format!("module zdgen_b;\n{gen_b}");
    let main = r#"module corpus_main;
import zdgen_a;
import zdgen_b;
void main() {
    import std.stdio : writeln;
    zdgen_a.Alpha a; a.x = 1; a.s = "a";
    auto ba = a.marshalXCDR(zdgen_a.Endian.LE);
    zdgen_b.Beta b; b.k = 2; b.v = [3];
    auto bb = b.marshalXCDR(zdgen_b.Endian.LE);
    if (ba.length == 0 || bb.length == 0) { writeln("FAIL empty"); return; }
    writeln("ok");
}
"#;
    let (ok, stderr, stdout) = gdc_build_run(
        "compose",
        &[
            ("zdgen_a.d", &file_a),
            ("zdgen_b.d", &file_b),
            ("corpus_main.d", main),
        ],
    );
    assert!(ok, "composed build/run failed:\n{stderr}");
    assert_eq!(stdout.trim(), "ok");
}
