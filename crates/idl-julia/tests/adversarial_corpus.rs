// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial IDL corpus run through the real Julia toolchain (gated on
//! `julia` being on PATH). Three corpora:
//!
//! 1. **reserved-keyword** — every Julia keyword that is a legal IDL identifier,
//!    placed at member / struct / enum-enumerator / module / const / union-branch
//!    positions, must generate Julia that loads (`escape_julia_ident` appends a
//!    trailing `_` on collision).
//! 2. **construct** — each IDL construct emitted minimally and executed, with a
//!    wire-size / wire-byte assertion where it is statically checkable
//!    (`long double` = 16 B, `wchar` = 4 B, `fixed<P,S>`, enum `@value`,
//!    struct inheritance, bitmask width, nested seq/map round-trip, `@mutable`
//!    union round-trip).
//! 3. **compose-multifile** — two IDLs generated separately (one with the wire
//!    prelude, one as a prelude-less fragment) and merged into one Julia source,
//!    which must load and run (the former per-file prelude duplication —
//!    #C-julia — would have re-declared `Writer`/`Reader`/`Endian`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_fragment, generate_julia_module};

/// Julia keywords that are also legal IDL identifiers (i.e. not `struct` /
/// `const` / `module` / `import`, which the IDL grammar reserves, nor `local`,
/// which IDL reserves for local interfaces).
const SAFE_KEYWORDS: &[&str] = &[
    "baremodule",
    "begin",
    "break",
    "catch",
    "continue",
    "do",
    "else",
    "elseif",
    "end",
    "export",
    "finally",
    "for",
    "function",
    "global",
    "if",
    "let",
    "macro",
    "quote",
    "return",
    "try",
    "using",
    "while",
];

fn julia_available() -> bool {
    Command::new("julia").arg("--version").output().is_ok()
}

fn gen_julia(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

/// Writes `src` to a temp `main.jl` and runs it, returning `(ok, stderr,
/// stdout)`. A unique tag keeps parallel test cases from colliding on disk.
fn julia_run(tag: &str, src: &str) -> (bool, String, String) {
    let dir = std::env::temp_dir().join(format!("idljulia_corpus_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, src).expect("write jl");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    let _ = std::fs::remove_dir_all(&dir);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn reserved_keyword_corpus_loads() {
    if !julia_available() {
        eprintln!("SKIP reserved-keyword corpus: `julia` not on PATH");
        return;
    }
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

    let mut src = gen_julia(&idl);
    src.push_str("\nprintln(\"OK\")\n");
    let (ok, stderr, stdout) = julia_run("reserved", &src);
    assert!(ok, "julia failed:\n{stderr}\n--- src ---\n{src}");
    assert_eq!(stdout.trim(), "OK", "reserved corpus did not run clean");
}

#[test]
fn construct_corpus_loads_and_wire_sizes_hold() {
    if !julia_available() {
        eprintln!("SKIP construct corpus: `julia` not on PATH");
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
        @final union EU switch (Color) { case RED: long red; case GREEN: short grn; default: octet oth; };
        @final union CU switch (char) { case 'A': long a; case 'B': short b; };
        @final union BU switch (boolean) { case TRUE: long yes; case FALSE: short no; };
        // #A16/F14: @mutable union — EMHEADER-framed member list (was unsupported).
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

    let mut src = gen_julia(idl);
    src.push_str(
        r#"
function main()
    if length(marshal_xcdr(LD(0.0), LE)) != 16
        println("FAIL LD"); return
    end
    if length(marshal_xcdr(WC(UInt32(0)), LE)) != 4
        println("FAIL WC"); return
    end
    if length(marshal_xcdr(Prim(0, 0, 0, 0), LE)) != 16
        println("FAIL Prim"); return
    end
    if length(marshal_xcdr(Fx(zd_fixed_enc("123.45", 5, 2)), LE)) != 3
        println("FAIL Fx"); return
    end
    if bytes2hex(marshal_xcdr(EnumHolder(SB), LE)) != "05000000"
        println("FAIL enum @value"); return
    end
    if bytes2hex(marshal_xcdr(Derived(1, 2, 3), LE)) != "010000000200000003000000"
        println("FAIL inheritance"); return
    end
    if length(marshal_xcdr(BM(0), LE)) != 4
        println("FAIL BM"); return
    end
    # #A11/A12/A13/F11/F12/F13: enum / char / bool union labels round-trip with
    # the discriminator typed correctly (enum value / Char / Bool, not integer).
    eu = unmarshal_xcdr_EU(marshal_xcdr(EU(GREEN, Int32(0), Int16(77), UInt8(0)), LE), LE)
    if eu.disc != GREEN || eu.grn != 77
        println("FAIL enum union"); return
    end
    cu = unmarshal_xcdr_CU(marshal_xcdr(CU(Char(66), Int32(0), Int16(9)), LE), LE)
    if cu.disc != Char(66) || cu.b != 9
        println("FAIL char union"); return
    end
    bu = unmarshal_xcdr_BU(marshal_xcdr(BU(false, Int32(0), Int16(5)), LE), LE)
    if bu.disc != false || bu.no != 5
        println("FAIL bool union"); return
    end
    _ = marshal_xcdr(OptA(true, Int32(0), Int32(0)), LE)
    _ = marshal_xcdr(MutS(Int32(0), ""), LE)
    # Collections: bounded sequence, 2x3 fixed array, and map<string,long>
    # round-trip (Julia is 1-based). The nested `ss`/`mm` fields are left empty:
    # they DEFINE fine, but the nested-collection temp-name reuse (#A21/A22/P9 —
    # audited for go/d, out of the idl-julia finding set) is a pre-existing hole
    # not fixed here, so they are only exercised at zero depth.
    c = Coll(Int32[1, 2, 3, 4],
             [Int32[1, 2, 3], Int32[4, 5, 6]],
             Dict{String,Int32}("k" => 1),
             Vector{Vector{Int32}}(),
             Dict{String,Dict{String,Int32}}())
    back = unmarshal_xcdr_Coll(marshal_xcdr(c, LE), LE)
    if back.s != Int32[1, 2, 3, 4]
        println("FAIL bounded seq"); return
    end
    if length(back.m) != 2 || back.m[1][3] != 3 || back.m[2][3] != 6
        println("FAIL multidim array"); return
    end
    if back.mp["k"] != 1
        println("FAIL map"); return
    end
    # #A16/F14: @mutable union round-trips through its EMHEADER-framed member list.
    u = MutU(Int32(1), Int32(42), Int16(0))
    ub = unmarshal_xcdr_MutU(marshal_xcdr(u, LE), LE)
    if ub.disc != 1 || ub.a != 42
        println("FAIL mutable union"); return
    end
    println("OK")
end
main()
"#,
    );

    let (ok, stderr, stdout) = julia_run("construct", &src);
    assert!(ok, "julia failed:\n{stderr}\n--- src ---\n{src}");
    assert_eq!(stdout.trim(), "OK", "wire-size assertion failed:\n{src}");
}

#[test]
fn compose_multifile_loads() {
    if !julia_available() {
        eprintln!("SKIP compose corpus: `julia` not on PATH");
        return;
    }
    // File A carries the shared wire prelude; file B is a prelude-less fragment.
    let opts = JuliaGenOptions::default();
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
    let file_a = generate_julia_module(&ast_a, &opts).expect("gen A");
    let file_b = generate_julia_fragment(&ast_b, &opts).expect("gen B fragment");

    // The fragment must NOT re-declare the wire prelude.
    assert!(
        !file_b.contains("mutable struct Writer"),
        "fragment re-declared the wire prelude:\n{file_b}"
    );
    assert!(
        file_a.contains("mutable struct Writer"),
        "file A missing prelude"
    );

    // Merge: file A whole, then B's body with its leading comment header lines
    // stripped (so `Writer`/`Reader`/`Endian` are defined exactly once).
    let b_body: String = file_b
        .lines()
        .skip_while(|l| l.trim_start().starts_with('#') || l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let mut merged = file_a;
    merged.push('\n');
    merged.push_str(&b_body);
    merged.push_str(
        "\nfunction main()\n    _ = marshal_xcdr(Alpha(1, \"a\"), LE)\n    _ = marshal_xcdr(Beta(UInt16(2), Int32[3]), LE)\n    println(\"OK\")\nend\nmain()\n",
    );

    let (ok, stderr, stdout) = julia_run("compose", &merged);
    assert!(
        ok,
        "composed source failed to load:\n{stderr}\n--- src ---\n{merged}"
    );
    assert_eq!(stdout.trim(), "OK", "composed source did not run clean");
}

/// Julia-toolchain-free (string) verification of the builtin/keyword mangling
/// finding: an IDL type named `Base` (or `Module`), or a field named `end`,
/// must never reach the emitted source as a bare `struct Base` / `::Base` /
/// `end::…` — those redefine an auto-imported `Main` constant (`Base`,
/// `Module`) or reuse a keyword and make `main.jl` fail to load
/// (`ERROR: invalid redefinition of constant Main.Base`). This asserts the
/// mangled forms on the real emit path; the runtime `main.jl`-loads proof lives
/// in [`construct_corpus_loads_and_wire_sizes_hold`], which runs only where the
/// `julia` toolchain is on PATH (Linux/CI, not macOS-local).
#[test]
fn builtin_and_keyword_type_names_are_mangled() {
    // `Base`/`Module` are auto-imported constants; `end` is a keyword. `Holder`
    // references `Base` as a field type so the reference path is exercised too.
    let idl = "
        @final struct Base { long a; };
        @final struct Module { long b; };
        @final struct Ender { long end; };
        @final struct Holder { Base ref; };
    ";
    let src = gen_julia(idl);

    // Definitions are mangled with the trailing-underscore convention.
    assert!(src.contains("struct Base_"), "Base not mangled:\n{src}");
    assert!(src.contains("struct Module_"), "Module not mangled:\n{src}");
    // No bare (unmangled) definition survives. Match `struct Base` / `struct
    // Module` NOT followed by `_` (so `struct Base_` does not trip the guard).
    assert!(
        !has_bare_ident(&src, "struct Base"),
        "bare `struct Base` present — would redefine Main.Base:\n{src}"
    );
    assert!(
        !has_bare_ident(&src, "struct Module"),
        "bare `struct Module` present — would redefine Main.Module:\n{src}"
    );

    // The `end` field is mangled to `end_` (a bare `end::` would parse as the
    // block terminator, not a field).
    assert!(src.contains("end_::"), "field `end` not mangled:\n{src}");

    // The reference to `Base` (field type of `Holder`) resolves to the mangled
    // name, so the definition and its use stay consistent.
    assert!(
        src.contains("ref::Base_"),
        "reference to Base not mangled consistently:\n{src}"
    );
    assert!(
        !has_bare_ident(&src, "::Base"),
        "bare `::Base` reference present:\n{src}"
    );
}

/// True if `needle` occurs in `hay` not immediately followed by an identifier
/// continuation char (`_`/alnum) — i.e. as a *bare* token, so `"struct Base"`
/// does not match inside `"struct Base_"`.
fn has_bare_ident(hay: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(rel) = hay[from..].find(needle) {
        let end = from + rel + needle.len();
        let next = hay[end..].chars().next();
        if !matches!(next, Some(c) if c == '_' || c.is_alphanumeric()) {
            return true;
        }
        from = end;
    }
    false
}
