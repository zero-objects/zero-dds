// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCaml backend XCDR1 (classic CDR) parity with idl-rust's `encode_xcdr1` /
//! `decode_xcdr1`. String-level tests always run; the byte-identity + round-trip
//! tests compile and run the generated OCaml and are gated on `ocamlfind`
//! (absent on this dev host / codepit CI without the OCaml toolchain — they
//! skip loud there).
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
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).expect("gen")
}

fn have_ocaml() -> bool {
    Command::new("ocamlfind").arg("printconf").output().is_ok()
}

/// Compiles `emit(idl)` plus `main` (which must `print_endline` its output
/// lines) and returns the program's stdout lines.
fn run_ocaml(tag: &str, idl: &str, main: &str) -> Vec<String> {
    let mut src = emit(idl);
    src.push_str(
        "\nlet zd_hex (b : bytes) : string =\n  \
         let buf = Buffer.create (Bytes.length b * 2) in\n  \
         Bytes.iter (fun c -> Buffer.add_string buf (Printf.sprintf \"%02x\" (Char.code c))) b;\n  \
         Buffer.contents buf\n",
    );
    src.push_str(main);
    let dir = std::env::temp_dir().join(format!("idlocaml_x1_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
        s.contains("let marshal_xcdr1 (v : t) (endian : Wire.endian) : bytes ="),
        "{s}"
    );
    assert!(
        s.contains("let unmarshal_xcdr1 (b : bytes) (endian : Wire.endian) : t ="),
        "{s}"
    );
    // The writer/reader carry the representation flag.
    assert!(s.contains("let writer_x1 endian ="), "{s}");
    assert!(s.contains("let reader_x1 (b : bytes) endian ="), "{s}");
}

#[test]
fn pl_cdr1_helpers_present_for_mutable() {
    let s = emit("@mutable struct M { unsigned long a; unsigned short b; };");
    assert!(s.contains("put_pl_cdr1_member"), "{s}");
    assert!(s.contains("put_pl_cdr1_sentinel"), "{s}");
    assert!(s.contains("read_pl_cdr1_member"), "{s}");
    // Mutable marshal branches on the writer representation.
    assert!(s.contains("if Wire.is_x1 w then begin"), "{s}");
}

#[test]
fn appendable_branches_on_dheader() {
    let s = emit("@appendable struct A { unsigned long id; unsigned short k; };");
    // Encode: XCDR1 inline (no DHEADER) vs XCDR2 length-prefixed body.
    assert!(s.contains("if Wire.is_x1 w then begin"), "{s}");
    // Decode: DHEADER skipped only when not XCDR1.
    assert!(
        s.contains("if not (Wire.r_is_x1 r) then ignore (Wire.get_u32 r)"),
        "{s}"
    );
}

#[test]
fn eight_byte_types_align_to_eight_under_xcdr1() {
    // `put_u64`/`get_u64`/`put_long_double` carry alignment 8; the `align` cap
    // reduces it to 4 under XCDR2, keeps 8 under XCDR1.
    let s = emit("@final struct F { octet a; double v; };");
    assert!(s.contains("let put_u64 w"), "{s}");
    assert!(s.contains("let cap = min a (if w.x1 then 8 else 4)"), "{s}");
}

// ---- byte identity (ocamlfind-gated) ---------------------------------------

/// `@final` with an 8-byte member: classic CDR aligns it to 8 (seven pad bytes),
/// whereas XCDR2 caps alignment at 4 (three pad bytes).
#[test]
fn cdr1_final_u64_aligns_to_eight() {
    if !have_ocaml() {
        eprintln!("SKIP cdr1_final_u64_aligns_to_eight: `ocamlfind` not on PATH");
        return;
    }
    let idl = "@final struct F { octet a; unsigned long long v; };";
    let out = run_ocaml(
        "final64",
        idl,
        "let () =\n  \
         let f : F.t = { a = 0x01; v = 0x0203040506070809L } in\n  \
         print_endline (zd_hex (F.marshal_xcdr1 f Wire.LE));\n  \
         print_endline (zd_hex (F.marshal f Wire.LE))\n",
    );
    // XCDR1: a, then 7 pad, then v LE.
    assert_eq!(out[0], "01000000000000000908070605040302", "xcdr1 le");
    // XCDR2: a, then 3 pad, then v LE.
    assert_eq!(out[1], "010000000908070605040302", "xcdr2 le");
}

/// `@appendable`: classic CDR is inline (no DHEADER); XCDR2 prepends the body's
/// byte-length DHEADER.
#[test]
fn cdr1_appendable_has_no_dheader() {
    if !have_ocaml() {
        eprintln!("SKIP cdr1_appendable_has_no_dheader: `ocamlfind` not on PATH");
        return;
    }
    let idl = "@appendable struct A { unsigned long id; unsigned short k; };";
    let out = run_ocaml(
        "app",
        idl,
        "let () =\n  \
         let a : A.t = { id = 0x11223344; k = 0x5566 } in\n  \
         print_endline (zd_hex (A.marshal_xcdr1 a Wire.LE));\n  \
         print_endline (zd_hex (A.marshal a Wire.LE))\n",
    );
    assert_eq!(out[0], "443322116655", "xcdr1 le (no dheader)");
    assert_eq!(out[1], "06000000443322116655", "xcdr2 le (dheader)");
}

/// `@mutable`: XCDR1 is a PL_CDR1 `[PID][len][body][pad]` list terminated by the
/// sentinel `02 3f 00 00` (PID_LIST_END), no outer DHEADER.
#[test]
fn cdr1_mutable_is_pl_cdr1() {
    if !have_ocaml() {
        eprintln!("SKIP cdr1_mutable_is_pl_cdr1: `ocamlfind` not on PATH");
        return;
    }
    let idl = "@mutable struct M { @id(1) unsigned long a; };";
    let out = run_ocaml(
        "mut",
        idl,
        "let () =\n  \
         let m : M.t = { a = 0xAABBCCDD } in\n  \
         print_endline (zd_hex (M.marshal_xcdr1 m Wire.LE))\n",
    );
    // PID=1 (01 00), len=4 (04 00), body dd cc bb aa, sentinel 02 3f 00 00.
    assert_eq!(out[0], "01000400ddccbbaa023f0000", "pl_cdr1 le");
}

/// Round-trip every extensibility through the XCDR1 entry points (and confirm
/// the XCDR2 entry points still round-trip too).
#[test]
fn cdr1_roundtrips_all_extensibilities() {
    if !have_ocaml() {
        eprintln!("SKIP cdr1_roundtrips_all_extensibilities: `ocamlfind` not on PATH");
        return;
    }
    let idl = "@final struct Fi { octet a; unsigned long long v; long long w; };\n\
               @appendable struct Ap { unsigned long id; sequence<long> s; };\n\
               @mutable struct Mu { @id(3) unsigned long x; @id(7) unsigned short k; string label; };\n\
               @mutable union Un switch(long) { case 1: unsigned long a; case 2: unsigned short b; default: octet c; };";
    let out = run_ocaml(
        "rt",
        idl,
        "let ok name b = print_endline (name ^ \": \" ^ (if b then \"ok\" else \"FAIL\"))\n\
         let () =\n  \
         let fi : Fi.t = { a = 9; v = 0x1122334455667788L; w = -3L } in\n  \
         ok \"fi_x1\" (Fi.unmarshal_xcdr1 (Fi.marshal_xcdr1 fi Wire.LE) Wire.LE = fi);\n  \
         ok \"fi_x1_be\" (Fi.unmarshal_xcdr1 (Fi.marshal_xcdr1 fi Wire.BE) Wire.BE = fi);\n  \
         ok \"fi_x2\" (Fi.unmarshal (Fi.marshal fi Wire.LE) Wire.LE = fi);\n  \
         let ap : Ap.t = { id = 5; s = [1;2;3] } in\n  \
         ok \"ap_x1\" (Ap.unmarshal_xcdr1 (Ap.marshal_xcdr1 ap Wire.LE) Wire.LE = ap);\n  \
         ok \"ap_x2\" (Ap.unmarshal (Ap.marshal ap Wire.LE) Wire.LE = ap);\n  \
         let mu : Mu.t = { x = 111; k = 222; label = \"hi\" } in\n  \
         ok \"mu_x1\" (Mu.unmarshal_xcdr1 (Mu.marshal_xcdr1 mu Wire.LE) Wire.LE = mu);\n  \
         ok \"mu_x1_be\" (Mu.unmarshal_xcdr1 (Mu.marshal_xcdr1 mu Wire.BE) Wire.BE = mu);\n  \
         ok \"mu_x2\" (Mu.unmarshal (Mu.marshal mu Wire.LE) Wire.LE = mu);\n  \
         let un : Un.t = { disc = 2; a = 0; b = 4242; c = 0 } in\n  \
         ok \"un_x1\" (Un.unmarshal_xcdr1 (Un.marshal_xcdr1 un Wire.LE) Wire.LE = un);\n  \
         ok \"un_x2\" (Un.unmarshal (Un.marshal un Wire.LE) Wire.LE = un)\n",
    );
    for line in &out {
        assert!(
            line.ends_with(": ok"),
            "roundtrip failed: {line}\nall: {out:?}"
        );
    }
    assert_eq!(out.len(), 10, "expected 10 roundtrip lines: {out:?}");
}
