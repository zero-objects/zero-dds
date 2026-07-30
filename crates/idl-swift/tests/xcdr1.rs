// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Swift backend XCDR1 (classic CDR) parity with idl-rust's `encode_xcdr1` /
//! `decode_xcdr1`. String smoke tests always run; the byte-identity + round-trip
//! tests compile and run the generated Swift and are gated on `swiftc` (present
//! on local macOS; codepit CI has none).
//!
//! The expected byte strings are hand-derived classic CDR (max alignment 8, no
//! collection/aggregate DHEADER, PL_CDR1 `@mutable` framing) — the same wire
//! `zerodds_cdr::xcdr1` produces for the Rust backend.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_swift::{SwiftGenOptions, generate_swift_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_swift_module(&ast, &SwiftGenOptions::default()).expect("gen")
}

fn have_swiftc() -> bool {
    Command::new("swiftc").arg("--version").output().is_ok()
}

/// Compiles `emit(idl)` plus `main` and returns the program's stdout lines.
fn run_swift(tag: &str, idl: &str, main: &str) -> Vec<String> {
    let mut src = emit(idl);
    src.push_str(
        "\nfunc toHex(_ b: [UInt8]) -> String { b.map { String(format: \"%02x\", $0) }.joined() }\n",
    );
    src.push_str(main);
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_x1_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let sf = dir.join("main.swift");
    std::fs::write(&sf, &src).expect("write");
    let build = Command::new("swiftc")
        .arg(&sf)
        .arg("-o")
        .arg(dir.join("main_bin"))
        .output()
        .expect("swiftc");
    assert!(
        build.status.success(),
        "swiftc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(dir.join("main_bin")).output().expect("run");
    assert!(
        run.status.success(),
        "run failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let out = String::from_utf8(run.stdout).expect("utf8");
    let lines: Vec<String> = out.lines().map(|l| l.trim().to_string()).collect();
    let _ = std::fs::remove_dir_all(&dir);
    lines
}

// ---- string-level (always) --------------------------------------------------

#[test]
fn cdr1_entry_points_are_emitted() {
    let s = emit("@final struct F { octet a; unsigned long long v; };");
    assert!(
        s.contains("public func marshalCDR1(_ endian: Endianness)"),
        "{s}"
    );
    assert!(
        s.contains("public static func unmarshalCDR1(_ buf: [UInt8], _ endian: Endianness)"),
        "{s}"
    );
    // The writer/reader carry the representation flag.
    assert!(s.contains("public var isXcdr1 = false"), "{s}");
    assert!(s.contains("w.isXcdr1 = true"), "{s}");
}

#[test]
fn pl_cdr1_helpers_present_for_mutable() {
    let s = emit("@mutable struct M { unsigned long a; unsigned short b; };");
    assert!(s.contains("writePlCdr1Member"), "{s}");
    assert!(s.contains("writePlCdr1Sentinel"), "{s}");
    assert!(s.contains("readPlCdr1Member"), "{s}");
}

// ---- byte identity (swiftc-gated) -------------------------------------------

/// `@final` with an 8-byte member: classic CDR aligns it to 8 (seven pad bytes),
/// whereas XCDR2 caps alignment at 4 (three pad bytes).
#[test]
fn cdr1_final_u64_aligns_to_eight() {
    if !have_swiftc() {
        eprintln!("SKIP cdr1_final_u64_aligns_to_eight: `swiftc` not on PATH");
        return;
    }
    let idl = "@final struct F { octet a; unsigned long long v; };";
    let out = run_swift(
        "final64",
        idl,
        "let f = F(a: 0x01, v: 0x0203040506070809)\n\
         print(toHex(try f.marshalCDR1(.little)))\n\
         print(toHex(try f.marshalXCDR(.little)))\n",
    );
    // XCDR1: a, then 7 pad, then v LE.
    assert_eq!(out[0], "01000000000000000908070605040302", "xcdr1 le");
    // XCDR2: a, then 3 pad, then v LE.
    assert_eq!(out[1], "010000000908070605040302", "xcdr2 le");
}

/// `@appendable`: classic CDR is inline (no DHEADER); XCDR2 prepends a DHEADER
/// byte-length of the member block.
#[test]
fn cdr1_appendable_has_no_dheader() {
    if !have_swiftc() {
        eprintln!("SKIP cdr1_appendable_has_no_dheader: `swiftc` not on PATH");
        return;
    }
    let idl = "@appendable struct A { unsigned long id; unsigned short k; };";
    let out = run_swift(
        "app",
        idl,
        "let a = A(id: 0x11223344, k: 0x5566)\n\
         print(toHex(try a.marshalCDR1(.little)))\n\
         print(toHex(try a.marshalXCDR(.little)))\n",
    );
    assert_eq!(out[0], "443322116655", "xcdr1 le (no dheader)");
    assert_eq!(out[1], "06000000443322116655", "xcdr2 le (dheader)");
}

/// `sequence<struct>`: classic CDR is `count + elements` stream-relative (no
/// collection DHEADER); XCDR2 wraps it in a byte-length DHEADER.
#[test]
fn cdr1_sequence_of_struct_no_collection_dheader() {
    if !have_swiftc() {
        eprintln!("SKIP cdr1_sequence_of_struct_no_collection_dheader: `swiftc` not on PATH");
        return;
    }
    let idl = "@final struct P { unsigned short x; };\n\
               @final struct Q { sequence<P> items; };";
    let out = run_swift(
        "seqstruct",
        idl,
        "let q = Q(items: [P(x: 0xAABB), P(x: 0xCCDD)])\n\
         print(toHex(try q.marshalCDR1(.little)))\n\
         print(toHex(try q.marshalXCDR(.little)))\n",
    );
    assert_eq!(
        out[0], "02000000bbaaddcc",
        "xcdr1 le (no collection dheader)"
    );
    assert_eq!(
        out[1], "0800000002000000bbaaddcc",
        "xcdr2 le (collection dheader)"
    );
}

/// `@mutable`: classic CDR is a PL_CDR1 `[PID][len]` list terminated by the
/// sentinel `0x3F02` — disc/member ids in the low PID, unpadded length, member
/// bodies padded to 4.
#[test]
fn cdr1_mutable_pl_cdr1_framing() {
    if !have_swiftc() {
        eprintln!("SKIP cdr1_mutable_pl_cdr1_framing: `swiftc` not on PATH");
        return;
    }
    let idl = "@mutable struct M { unsigned long a; unsigned short b; };";
    let out = run_swift(
        "mut",
        idl,
        "let m = M(a: 0x11223344, b: 0x5566)\n\
         print(toHex(try m.marshalCDR1(.little)))\n",
    );
    // member a (id0): [00 00][04 00] 44332211 ; member b (id1): [01 00][02 00]
    // 6655 [00 00 pad] ; sentinel [02 3f][00 00].
    assert_eq!(
        out[0], "00000400443322110100020066550000023f0000",
        "xcdr1 pl_cdr1"
    );
}

// ---- round trips (swiftc-gated) ---------------------------------------------

/// Exercises every extensibility + a nested struct, sequence, string, map and
/// union through `marshalCDR1`/`unmarshalCDR1` and asserts the value survives.
#[test]
fn cdr1_round_trips_all_shapes() {
    if !have_swiftc() {
        eprintln!("SKIP cdr1_round_trips_all_shapes: `swiftc` not on PATH");
        return;
    }
    let idl = "\
@final struct Inner { unsigned short a; unsigned long long b; };
@appendable struct App { unsigned long id; sequence<Inner> kids; string label; };
@mutable struct Mut { unsigned long a; string s; sequence<octet> raw; };
@appendable union U switch (long) { case 1: unsigned long long big; case 2: unsigned short small; };
@final struct WithMap { map<unsigned long, Inner> m; };";
    let main = "\
func chk(_ ok: Bool, _ n: String) { if !ok { print(\"FAIL \\(n)\"); } else { print(\"ok \\(n)\") } }
for e in [Endianness.little, Endianness.big] {
    let app = App(id: 0xCAFEBABE, kids: [Inner(a: 1, b: 0xAABBCCDDEEFF0011), Inner(a: 2, b: 7)], label: \"hi\")
    let a2 = try App.unmarshalCDR1(try app.marshalCDR1(e), e)
    chk(a2.id == app.id && a2.kids.count == 2 && a2.kids[0].b == 0xAABBCCDDEEFF0011 && a2.label == \"hi\", \"app\")

    var mut = Mut(a: 42, s: \"world\", raw: [9, 8, 7])
    mut.a = 0x01020304
    let m2 = try Mut.unmarshalCDR1(try mut.marshalCDR1(e), e)
    chk(m2.a == 0x01020304 && m2.s == \"world\" && m2.raw == [9, 8, 7], \"mut\")

    var u = U(disc: 1, big: 0, small: 0); u.big = 0x1122334455667788
    let u2 = try U.unmarshalCDR1(try u.marshalCDR1(e), e)
    chk(u2.disc == 1 && u2.big == 0x1122334455667788, \"union\")

    let wm = WithMap(m: [7: Inner(a: 3, b: 0x99)])
    let wm2 = try WithMap.unmarshalCDR1(try wm.marshalCDR1(e), e)
    chk(wm2.m[7]?.b == 0x99, \"map\")
}
";
    let out = run_swift("roundtrip", idl, main);
    let fails: Vec<&String> = out.iter().filter(|l| l.starts_with("FAIL")).collect();
    assert!(
        fails.is_empty(),
        "round-trip failures: {fails:?}\nall: {out:?}"
    );
    // 4 checks x 2 endiannesses.
    assert_eq!(
        out.iter().filter(|l| l.starts_with("ok ")).count(),
        8,
        "{out:?}"
    );
}
