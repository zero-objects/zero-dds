// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Go
//! XCDR2 encode AND decode: a `string<N>` / `wstring<N>` / `sequence<T, N>` /
//! `map<K,V,N>` value longer than its declared bound is rejected (panics) on
//! BOTH sides — idl-go previously had NO bound enforcement at all (neither
//! side), the widest gap among the idl-* backends surveyed alongside the
//! idl-rust decode-side fix (#22).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

fn gen_go(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_go_module(&ast, &GoGenOptions::default()).expect("gen")
}

#[test]
fn bounded_string_encode_and_decode_checks() {
    let go = gen_go("@final struct Named { string<16> name; };");
    assert!(
        go.contains("len(v.Name) > 16")
            && go.contains("encoded string length exceeds its IDL bound (16)"),
        "bounded string<16> must panic on over-bound encode:\n{go}"
    );
    assert!(
        go.contains("decoded string length exceeds its IDL bound (16)"),
        "bounded string<16> must panic on over-bound decode:\n{go}"
    );
}

#[test]
fn bounded_wstring_uses_utf16_unit_count() {
    let go = gen_go("@final struct Named { wstring<8> name; };");
    assert!(
        go.contains("wstringUnitLen(v.Name) > 8")
            && go.contains("encoded wstring length exceeds its IDL bound (8)"),
        "bounded wstring<8> must panic on over-bound encode using UTF-16 unit count:\n{go}"
    );
    assert!(
        go.contains("decoded wstring length exceeds its IDL bound (8)"),
        "bounded wstring<8> must panic on over-bound decode:\n{go}"
    );
}

#[test]
fn bounded_sequence_octet_fast_path_checks() {
    let go = gen_go("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        go.contains("len(v.Data) > 4")
            && go.contains("encoded sequence length exceeds its IDL bound (4)"),
        "bounded sequence<octet,4> (fast byte-slice path) must panic on over-bound encode:\n{go}"
    );
    assert!(
        go.contains("decoded sequence length exceeds its IDL bound (4)"),
        "bounded sequence<octet,4> must panic on over-bound decode:\n{go}"
    );
}

#[test]
fn bounded_sequence_of_struct_checks() {
    // Non-fast-path (struct-element, DHEADER-framed) sequence must also carry
    // the bound check, not just the sequence<octet> fast path.
    let go =
        gen_go("@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };");
    assert!(
        go.contains("encoded sequence length exceeds its IDL bound (3)"),
        "bounded sequence<Pt,3> (struct-element path) must panic on over-bound encode:\n{go}"
    );
    assert!(
        go.contains("decoded sequence length exceeds its IDL bound (3)"),
        "bounded sequence<Pt,3> must panic on over-bound decode:\n{go}"
    );
}

#[test]
fn bounded_map_checks() {
    let go = gen_go("@final struct M { map<string, long, 2> vals; };");
    assert!(
        go.contains("encoded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must panic on over-bound encode:\n{go}"
    );
    assert!(
        go.contains("decoded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must panic on over-bound decode:\n{go}"
    );
}

#[test]
fn bounded_string_mutable_and_appendable_checks() {
    // marshalInto/unmarshalFrom build ONE put/get statement per field and
    // reuse it across Final/Appendable/Mutable extensibility bodies — proves
    // the @mutable and @appendable emit paths inherit the check for free.
    let appendable = gen_go("@appendable struct Named { string<8> name; };");
    assert!(
        appendable.contains("encoded string length exceeds its IDL bound (8)")
            && appendable.contains("decoded string length exceeds its IDL bound (8)"),
        "@appendable struct must carry both checks:\n{appendable}"
    );
    let mutable = gen_go("@mutable struct Named { string<8> name; };");
    assert!(
        mutable.contains("encoded string length exceeds its IDL bound (8)")
            && mutable.contains("decoded string length exceeds its IDL bound (8)"),
        "@mutable struct must carry both checks:\n{mutable}"
    );
}

#[test]
fn bounded_string_union_member_checks() {
    let go = gen_go("union U switch (long) { case 1: string<8> s; };");
    assert!(
        go.contains("encoded string length exceeds its IDL bound (8)")
            && go.contains("decoded string length exceeds its IDL bound (8)"),
        "union member (routes through the same map_type/map_get) must carry both checks:\n{go}"
    );
}

#[test]
fn bounded_string_array_element_checks() {
    // Array decode (build_array_get) calls map_get at the leaf for every
    // dimension, so array-of-bounded-element is covered for free (no
    // separate manual-array-decode path like idl-rust has).
    let go = gen_go("@final struct A { string<4> names[3]; };");
    assert!(
        go.contains("encoded string length exceeds its IDL bound (4)")
            && go.contains("decoded string length exceeds its IDL bound (4)"),
        "array-of-bounded-string element must carry both checks:\n{go}"
    );
}

#[test]
fn unbounded_no_check_emitted() {
    let go = gen_go(
        "@final struct Free { string name; wstring wname; sequence<long> vals; map<string, long> m; };",
    );
    assert!(
        !go.contains("exceeds its IDL bound"),
        "unbounded string/wstring/sequence/map must NOT get a bound check:\n{go}"
    );
}

/// Real `go run` execution: constructs an over-bound value in Go, calls
/// `MarshalXCDR`, and confirms it panics with the expected message (recovered
/// in a deferred handler so the process exits 0 either way, and the outcome
/// is reported over stdout) — proves the emitted Go actually compiles AND
/// the check fires at runtime, not just that the string is present in the
/// generated source. Gated on `go` being on PATH (matches golden.rs style).
#[test]
fn runtime_encode_panics_on_over_bound_string() {
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP runtime_encode_panics_on_over_bound_string: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(
            "@final struct S { string<4> label; };",
            &ParserConfig::default(),
        )
        .expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	defer func() {
		if r := recover(); r != nil {
			fmt.Println("PANICKED:", r)
			return
		}
		fmt.Println("NO PANIC")
	}()
	s := S{Label: "this-is-way-over-four-chars"}
	_ = s.MarshalXCDR(Little)
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"fmt\"\n)\n",
        1,
    );

    let dir = std::env::temp_dir().join(format!("idlgo_bound_encode_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");

    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("PANICKED") && stdout.contains("exceeds its IDL bound (4)"),
        "expected a recovered panic reporting the IDL bound, got:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Same as above but for decode: forges an over-bound wire value using the
/// runtime `Writer`/`PutString` directly (no bound awareness — mirrors an
/// adversarial/foreign sender), then calls `unmarshalFromS` and confirms it
/// panics instead of silently accepting the oversized value (the untrusted-
/// input DoS vector XTypes 1.3 §7.4.3 calls out).
#[test]
fn runtime_decode_panics_on_over_bound_string() {
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP runtime_decode_panics_on_over_bound_string: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(
            "@final struct S { string<4> label; };",
            &ParserConfig::default(),
        )
        .expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	defer func() {
		if r := recover(); r != nil {
			fmt.Println("PANICKED:", r)
			return
		}
		fmt.Println("NO PANIC")
	}()
	w := NewWriter(Little)
	w.PutString("this-is-way-over-four-chars")
	r := NewReader(w.Bytes(), Little)
	_ = unmarshalFromS(r)
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"fmt\"\n)\n",
        1,
    );

    let dir = std::env::temp_dir().join(format!("idlgo_bound_decode_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");

    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("PANICKED")
            && stdout.contains("decoded string length exceeds its IDL bound (4)"),
        "expected a recovered panic reporting the IDL bound, got:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
