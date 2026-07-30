// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Julia backend XCDR1 (classic CDR) parity with idl-rust's `encode_xcdr1` /
//! `decode_xcdr1`. String smoke tests always run; the byte-identity + round-trip
//! tests compile and run the generated Julia and are gated on `julia` (absent on
//! codepit CI / this macOS host — marked as a Linux/CI boundary, not skipped
//! silently).
//!
//! The expected byte strings are hand-derived classic CDR (max alignment 8, no
//! aggregate/collection DHEADER, PL_CDR1 `@mutable` framing) — the same wire
//! `zerodds_cdr::xcdr1` produces for the Rust backend.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

fn have_julia() -> bool {
    Command::new("julia").arg("--version").output().is_ok()
}

/// Emits `idl` + a `main` and returns the program's stdout lines.
fn run_julia(tag: &str, idl: &str, main: &str) -> Vec<String> {
    let mut src = emit(idl);
    src.push_str(main);
    let dir = std::env::temp_dir().join(format!("idljulia_x1_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = String::from_utf8(out.stdout)
        .expect("utf8")
        .lines()
        .map(str::to_string)
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    lines
}

// ---- string-path (always run) --------------------------------------------

#[test]
fn prelude_carries_the_xcdr1_writer_mode() {
    let j = emit("@final struct P { uint32 a; };");
    // Writer/Reader carry the mode flag; alignment caps at 8 under XCDR1, 4 else.
    assert!(j.contains("xcdr1::Bool"), "{j}");
    assert!(j.contains("cap = min(a, w.xcdr1 ? 8 : 4)"), "{j}");
    assert!(j.contains("cap = min(a, r.xcdr1 ? 8 : 4)"), "{j}");
    // PL_CDR1 helpers present.
    assert!(j.contains("function write_pl_cdr1_member!"), "{j}");
    assert!(j.contains("write_pl_cdr1_sentinel!"), "{j}");
    assert!(j.contains("function read_pl_cdr1_member!"), "{j}");
    // Every struct gains XCDR1 entry points.
    assert!(
        j.contains("function marshal_xcdr1(v::P, endian::Endian)::Vector{UInt8}"),
        "{j}"
    );
    assert!(j.contains("function unmarshal_xcdr1_P("), "{j}");
    // 8-byte types align to 8 (capped to 4 under XCDR2, so bytes unchanged).
    assert!(j.contains("put_u64!(w::Writer, v) = emit!(w, 8,"), "{j}");
    assert!(
        j.contains("get_u64!(r::Reader)::UInt64 = get_le(r, 8, 8)"),
        "{j}"
    );
}

#[test]
fn appendable_marshal_branches_on_the_writer_mode() {
    let j = emit("@appendable struct A { octet x; };");
    // XCDR1: inline (no DHEADER); XCDR2: length-prefixed member block.
    assert!(j.contains("if w.xcdr1"), "{j}");
    assert!(j.contains("bb = bytes(body)"), "{j}");
    // Decode skips the DHEADER only under XCDR2.
    assert!(j.contains("if !r.xcdr1"), "{j}");
}

#[test]
fn mutable_marshal_uses_pl_cdr1_framing() {
    let j = emit("@mutable struct M { @id(1) uint32 v; };");
    assert!(
        j.contains("write_pl_cdr1_member!(w, UInt32(0x00000001), bytes(zdMem))"),
        "{j}"
    );
    assert!(j.contains("write_pl_cdr1_sentinel!(w)"), "{j}");
    // Decode collects the PL member list.
    assert!(j.contains("zdm = read_pl_cdr1_member!(r)"), "{j}");
}

#[test]
fn sequence_of_struct_omits_dheader_under_xcdr1() {
    let j = emit("@final struct E { uint16 a; };\n@final struct C { sequence<E> es; };");
    // XCDR1 branch writes count + elements straight into the stream writer;
    // XCDR2 branch builds them in a DHEADER-framed sub-writer.
    assert!(j.contains("marshal_into!(e, w)"), "{j}");
    assert!(j.contains("marshal_into!(e, sub)"), "{j}");
}

// ---- runtime byte-identity + round-trip (gated on julia) ------------------

/// A `uint64` following a `uint32` is 8-aligned under XCDR1 (4 pad bytes),
/// 4-aligned under XCDR2 (none). Confirms the max-alignment-8 writer mode.
#[test]
fn final_struct_eight_byte_alignment() {
    if !have_julia() {
        eprintln!("SKIP final_struct_eight_byte_alignment: `julia` not on PATH (Linux/CI-only)");
        return;
    }
    let idl = "@final struct P { uint32 a; uint64 b; };";
    let main = r#"
function main()
    p = P(0x11223344, 0x1122334455667788)
    println(bytes2hex(marshal_xcdr(p, LE)))
    println(bytes2hex(marshal_xcdr1(p, LE)))
end
main()
"#;
    let out = run_julia("align8", idl, main);
    assert_eq!(out[0], "443322118877665544332211", "XCDR2 (max-align 4)");
    assert_eq!(
        out[1], "44332211000000008877665544332211",
        "XCDR1 must pad the u64 to an 8-byte boundary"
    );
}

/// `@appendable`: XCDR2 frames the member block with a DHEADER; XCDR1 does not.
#[test]
fn appendable_no_dheader_under_xcdr1() {
    if !have_julia() {
        eprintln!("SKIP appendable_no_dheader_under_xcdr1: `julia` not on PATH (Linux/CI-only)");
        return;
    }
    let idl = "@appendable struct A { octet x; };";
    let main = r#"
function main()
    a = A(0xAB)
    println(bytes2hex(marshal_xcdr(a, LE)))
    println(bytes2hex(marshal_xcdr1(a, LE)))
end
main()
"#;
    let out = run_julia("appd", idl, main);
    assert_eq!(out[0], "01000000ab", "XCDR2: DHEADER(=1) + octet");
    assert_eq!(out[1], "ab", "XCDR1: inline octet, no DHEADER");
}

/// `@mutable`: PL_CDR1 member list `[PID][len][body][pad]` + sentinel.
#[test]
fn mutable_pl_cdr1_bytes_and_roundtrip() {
    if !have_julia() {
        eprintln!("SKIP mutable_pl_cdr1_bytes_and_roundtrip: `julia` not on PATH (Linux/CI-only)");
        return;
    }
    let idl = "@mutable struct M { @id(1) uint32 v; };";
    let main = r#"
function main()
    m = M(0x04030201)
    println(bytes2hex(marshal_xcdr1(m, LE)))
    r = unmarshal_xcdr1_M(marshal_xcdr1(m, LE), LE)
    println(r.v == 0x04030201 ? "rt-ok" : "rt-bad")
    rb = unmarshal_xcdr1_M(marshal_xcdr1(m, BE), BE)
    println(rb.v == 0x04030201 ? "be-ok" : "be-bad")
end
main()
"#;
    let out = run_julia("mut", idl, main);
    // PID=1 (0100) len=4 (0400) body 01020304 sentinel 023f 0000.
    assert_eq!(out[0], "0100040001020304023f0000", "PL_CDR1 framing");
    assert_eq!(out[1], "rt-ok");
    assert_eq!(out[2], "be-ok");
}

/// End-to-end XCDR1 round-trip across the mixed feature surface: primitives,
/// string, bounded sequence, nested struct, sequence<struct>, for @final and
/// @appendable, LE and BE.
#[test]
fn mixed_xcdr1_roundtrip() {
    if !have_julia() {
        eprintln!("SKIP mixed_xcdr1_roundtrip: `julia` not on PATH (Linux/CI-only)");
        return;
    }
    let idl = "@final struct Inner { uint16 a; uint64 b; };\n\
               @appendable struct Outer { uint32 id; string label; Inner one; sequence<Inner> many; };";
    let main = r#"
function main()
    o = Outer(0xCAFEBABE, "hi", Inner(0x0102, 0x1122334455667788),
              [Inner(0x0001, 0x1), Inner(0x0002, 0x2)])
    for e in (LE, BE)
        r = unmarshal_xcdr1_Outer(marshal_xcdr1(o, e), e)
        ok = r.id == o.id && r.label == o.label && r.one.a == o.one.a &&
             r.one.b == o.one.b && length(r.many) == 2 &&
             r.many[1].a == 0x0001 && r.many[2].b == 0x2
        println(ok ? "ok" : "bad")
    end
end
main()
"#;
    let out = run_julia("mixed", idl, main);
    assert_eq!(out, vec!["ok", "ok"], "XCDR1 round-trip LE + BE");
}
