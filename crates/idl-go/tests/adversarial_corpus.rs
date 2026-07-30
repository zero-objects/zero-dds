// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial IDL corpus compiled with the real Go toolchain (gated on `go`
//! being on PATH). Three corpora:
//!
//! 1. **reserved-keyword** — every Go keyword that is a legal IDL identifier,
//!    placed at member / struct / enum-enumerator / module / const / union-branch
//!    positions, must generate Go that compiles (`exported()` capitalization
//!    dodges the lowercase Go keywords).
//! 2. **construct** — each IDL construct emitted minimally and compiled, with a
//!    wire-size / wire-byte assertion where it is statically checkable
//!    (`long double` = 16 B, `wchar` = 4 B, `fixed<P,S>`, enum `@value`,
//!    struct inheritance, primitive alignment).
//! 3. **compose-multifile** — two IDLs generated separately (one with the wire
//!    prelude, one as a prelude-less fragment) and merged into one Go package,
//!    which must build (the former per-file prelude duplication — #C-go — would
//!    have re-declared `Writer`/`Reader`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_fragment, generate_go_module};

/// Go keywords that are also legal IDL identifiers (i.e. not themselves IDL
/// keywords such as `map`/`switch`/`case`/`default`/`const`/`interface`/
/// `struct`/`enum`/`union`/`module`/`typedef`/`import`).
const SAFE_KEYWORDS: &[&str] = &[
    "break",
    "chan",
    "continue",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "package",
    "range",
    "return",
    "select",
    "type",
    "var",
];

fn go_available() -> bool {
    Command::new("go").arg("version").output().is_ok()
}

fn gen_main(idl: &str) -> String {
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    generate_go_module(
        &ast,
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen")
}

/// Writes `src` to a temp `main.go` and `go run`s it, returning `(ok, stderr,
/// stdout)`. A unique tag keeps parallel test cases from colliding on disk.
fn go_run(tag: &str, src: &str) -> (bool, String, String) {
    let dir = std::env::temp_dir().join(format!("idlgo_corpus_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    let _ = std::fs::remove_dir_all(&dir);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// Appends an empty `main` (compile-only) and asserts the program builds/runs.
fn assert_compiles(tag: &str, mut src: String) {
    src.push_str("\nfunc main() {}\n");
    let (ok, stderr, _) = go_run(tag, &src);
    assert!(
        ok,
        "go run failed for `{tag}`:\n{stderr}\n--- src ---\n{src}"
    );
}

#[test]
fn reserved_keyword_corpus_compiles() {
    if !go_available() {
        eprintln!("SKIP reserved-keyword corpus: `go` not on PATH");
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

    assert_compiles("reserved", gen_main(&idl));
}

#[test]
fn construct_corpus_compiles_and_wire_sizes_hold() {
    if !go_available() {
        eprintln!("SKIP construct corpus: `go` not on PATH");
        return;
    }
    // One IDL covering the construct set; distinct type names avoid collisions.
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

    let mut src = gen_main(idl);
    // A main that asserts the statically-checkable wire sizes/bytes.
    src.push_str(
        r#"
func hexOf(b []byte) string { return hex.EncodeToString(b) }
func main() {
	// long double is 16 bytes on the wire.
	if n := len((LD{}).MarshalXCDR(Little)); n != 16 {
		fmt.Printf("FAIL LD len=%d want 16\n", n); return
	}
	// wchar is 4 bytes (the established ZeroDDS reference wire).
	if n := len((WC{}).MarshalXCDR(Little)); n != 4 {
		fmt.Printf("FAIL WC len=%d want 4\n", n); return
	}
	// octet+short+long+longlong with XCDR2 alignment = 16 bytes.
	if n := len((Prim{}).MarshalXCDR(Little)); n != 16 {
		fmt.Printf("FAIL Prim len=%d want 16\n", n); return
	}
	// fixed<5,2> packs to (5+2)/2 = 3 BCD octets.
	if n := len((Fx{F: zdFixedEnc("123.45", 5, 2)}).MarshalXCDR(Little)); n != 3 {
		fmt.Printf("FAIL Fx len=%d want 3\n", n); return
	}
	// enum @value: SB == 5, so its 4-byte LE encoding is 05000000.
	if h := hexOf((EnumHolder{E: SparseSB}).MarshalXCDR(Little)); h != "05000000" {
		fmt.Printf("FAIL enum @value hex=%s want 05000000\n", h); return
	}
	// struct inheritance: Derived carries a,b (base) then c.
	if h := hexOf((Derived{A: 1, B: 2, C: 3}).MarshalXCDR(Little)); h != "010000000200000003000000" {
		fmt.Printf("FAIL inheritance hex=%s\n", h); return
	}
	// bitmask default backing integer is uint32 (4 bytes).
	if n := len((BM{}).MarshalXCDR(Little)); n != 4 {
		fmt.Printf("FAIL BM len=%d want 4\n", n); return
	}
	// #A21: nested sequence<sequence<long>> must round-trip (the former literal
	// loop index corrupted/panicked). A 2x3 jagged value survives marshal/unmarshal.
	{
		c := Coll{
			Ss: [][]int32{{1, 2, 3}, {4, 5}},
			Mm: map[string]map[string]int32{"a": {"x": 10, "y": 11}, "b": {"z": 12}},
		}
		back := UnmarshalXCDRColl(c.MarshalXCDR(Little), Little)
		if len(back.Ss) != 2 || len(back.Ss[0]) != 3 || back.Ss[0][2] != 3 || back.Ss[1][1] != 5 {
			fmt.Printf("FAIL nested seq round-trip: %v\n", back.Ss); return
		}
		// #A21: nested map<string,map<string,long>> must round-trip (the former
		// literal temp produced `zv[zk] = zv`, a Go type error).
		if back.Mm["a"]["y"] != 11 || back.Mm["b"]["z"] != 12 {
			fmt.Printf("FAIL nested map round-trip: %v\n", back.Mm); return
		}
	}
	// #A16: @mutable union round-trips through its EMHEADER-framed member list.
	{
		u := MutU{Disc: 1, A: 42}
		back := UnmarshalXCDRMutU(u.MarshalXCDR(Little), Little)
		if back.Disc != 1 || back.A != 42 {
			fmt.Printf("FAIL mutable union round-trip: %+v\n", back); return
		}
	}
	fmt.Println("OK")
}
"#,
    );
    let src = src.replacen(
        "import (\n\t\"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n",
        1,
    );

    let (ok, stderr, stdout) = go_run("construct", &src);
    assert!(ok, "go run failed:\n{stderr}\n--- src ---\n{src}");
    assert_eq!(stdout.trim(), "OK", "wire-size assertion failed:\n{src}");
}

#[test]
fn compose_multifile_builds() {
    if !go_available() {
        eprintln!("SKIP compose corpus: `go` not on PATH");
        return;
    }
    // File A carries the shared wire prelude; file B is a prelude-less fragment.
    let opts = GoGenOptions {
        package_name: "main".to_string(),
    };
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
    let file_a = generate_go_module(&ast_a, &opts).expect("gen A");
    let file_b = generate_go_fragment(&ast_b, &opts).expect("gen B fragment");

    // The fragment must NOT re-declare the wire prelude.
    assert!(
        !file_b.contains("type Writer struct"),
        "fragment re-declared the wire prelude:\n{file_b}"
    );
    assert!(
        file_a.contains("type Writer struct"),
        "file A missing prelude"
    );

    // Merge idiomatically: package clause once (from A), then A's body, then B's
    // body (strip B's own header/package lines).
    let mut merged = file_a;
    let b_body: String = file_b
        .lines()
        .skip_while(|l| l.starts_with("//") || l.starts_with("package ") || l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    merged.push('\n');
    merged.push_str(&b_body);
    merged.push_str(
        "\nfunc main() {\n\t_ = Alpha{X: 1, S: \"a\"}.MarshalXCDR(Little)\n\t_ = Beta{K: 2, V: []int32{3}}.MarshalXCDR(Little)\n}\n",
    );

    let (ok, stderr, _) = go_run("compose", &merged);
    assert!(
        ok,
        "composed package failed to build:\n{stderr}\n--- src ---\n{merged}"
    );
}
