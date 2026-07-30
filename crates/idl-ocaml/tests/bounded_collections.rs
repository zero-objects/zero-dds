// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated OCaml
//! codec: idl-ocaml previously had NO enforcement on either side — a
//! `sequence<T, N>` / `string<N>` / `wstring<N>` / `map<K,V,N>` value longer
//! than its declared bound silently encoded and decoded. XTypes 1.3 §7.4.3
//! requires the bound enforced on BOTH sides (decode especially: a
//! well-formed-but-oversized wire value is otherwise an untrusted-input DoS
//! vector). Both `marshal_into`/`marshal` (encode) and `read`/`unmarshal`
//! (decode) now `failwith` on an over-bound value; every representation
//! (`@final`/`@appendable`, union members, array elements) routes through
//! the shared `map_type`/`map_get`, so the fix covers all of them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};

fn gen_ml(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).expect("gen")
}

#[test]
fn bounded_string_encode_and_decode_checks() {
    let ml = gen_ml("@final struct Named { string<16> name; };");
    assert!(
        ml.contains("String.length v.name > 16")
            && ml.contains("bounded string length exceeds its IDL bound (16)"),
        "bounded string<16> must throw on over-bound encode:\n{ml}"
    );
    assert!(
        ml.contains("decoded string length exceeds its IDL bound (16)"),
        "bounded string<16> must throw on over-bound decode:\n{ml}"
    );
}

#[test]
fn bounded_wstring_encode_and_decode_checks() {
    let ml = gen_ml("@final struct Named { wstring<8> name; };");
    assert!(
        ml.contains("bounded wstring length exceeds its IDL bound (8)"),
        "bounded wstring<8> must throw on over-bound encode:\n{ml}"
    );
    assert!(
        ml.contains("decoded wstring length exceeds its IDL bound (8)"),
        "bounded wstring<8> must throw on over-bound decode:\n{ml}"
    );
}

#[test]
fn bounded_octet_sequence_encode_and_decode_checks() {
    let ml = gen_ml("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        ml.contains("List.length v.data > 4")
            && ml.contains("bounded sequence length exceeds its IDL bound (4)"),
        "bounded sequence<octet,4> must throw on over-bound encode:\n{ml}"
    );
    assert!(
        ml.contains("decoded sequence length exceeds its IDL bound (4)"),
        "bounded sequence<octet,4> must throw on over-bound decode:\n{ml}"
    );
}

#[test]
fn bounded_struct_sequence_encode_and_decode_checks() {
    let ml =
        gen_ml("@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };");
    assert!(
        ml.contains("bounded sequence length exceeds its IDL bound (3)"),
        "bounded sequence<Pt,3> must throw on over-bound encode:\n{ml}"
    );
    assert!(
        ml.contains("decoded sequence length exceeds its IDL bound (3)"),
        "bounded sequence<Pt,3> must throw on over-bound decode:\n{ml}"
    );
}

#[test]
fn bounded_map_encode_and_decode_checks() {
    let ml = gen_ml("@final struct M { map<string, long, 2> vals; };");
    assert!(
        ml.contains("bounded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must throw on over-bound encode:\n{ml}"
    );
    assert!(
        ml.contains("decoded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must throw on over-bound decode:\n{ml}"
    );
}

#[test]
fn bounded_string_array_element_checks() {
    // Array elements route through the same `map_type`/`map_get` as scalar
    // members (see `emit_struct`'s `Declarator::Array` arm calling
    // `map_type`/`map_get` with the resolved element type) — no separate
    // manual-array-decode path like idl-rust has, so array-of-bounded
    // elements is covered for free.
    let ml = gen_ml("@final struct A { string<4> names[3]; };");
    assert!(
        ml.contains("bounded string length exceeds its IDL bound (4)"),
        "array-of-bounded-string element encode must carry the check:\n{ml}"
    );
    assert!(
        ml.contains("decoded string length exceeds its IDL bound (4)"),
        "array-of-bounded-string element decode must carry the check:\n{ml}"
    );
}

#[test]
fn bounded_string_union_member_checks() {
    let ml = gen_ml("union U switch (long) { case 1: string<8> s; };");
    assert!(
        ml.contains("bounded string length exceeds its IDL bound (8)"),
        "union member encode must carry the check:\n{ml}"
    );
    assert!(
        ml.contains("decoded string length exceeds its IDL bound (8)"),
        "union member decode must carry the check:\n{ml}"
    );
}

#[test]
fn unbounded_no_checks_either_side() {
    // sequence<long> / sequence<non-octet, non-struct> is unsupported by
    // this backend regardless of bound (see `map_sequence`'s error arm), so
    // this only exercises the type/collection shapes idl-ocaml supports.
    let ml = gen_ml(
        "@final struct Free { string name; wstring wname; sequence<octet> vals; map<string, long> m; };",
    );
    assert!(
        !ml.contains("exceeds its IDL bound"),
        "unbounded string/wstring/sequence/map must NOT get a bound check:\n{ml}"
    );
}

/// Real-compile proof (gated on `ocamlfind`, same gate as `golden.rs`):
/// within-bound roundtrips normally; an over-bound VALUE constructed
/// in-process (bypassing the codegen'd encode check by hand-building the
/// wire bytes) is rejected on decode via `Failure`, and an over-bound value
/// passed to `marshal` is rejected on encode the same way.
#[test]
fn ocaml_runtime_rejects_over_bound_string() {
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP ocaml_runtime_rejects_over_bound_string: `ocamlfind` not on PATH");
        return;
    }
    let idl = "@final struct S { string<8> label; };";
    let mut src = gen_ml(idl);
    src.push_str(
        r#"
let () =
  (* Within-bound roundtrip must still work (no false positives). *)
  let ok : S.t = { S.label = "short" } in
  let bytes = S.marshal ok Wire.LE in
  let back = S.unmarshal bytes Wire.LE in
  assert (back.S.label = "short");

  (* Over-bound value on ENCODE must raise Failure. *)
  (try
     let _ = S.marshal { S.label = "this-is-way-over-eight-chars" } Wire.LE in
     print_endline "FAIL: encode did not reject over-bound value"
   with Failure _ -> print_endline "encode-rejected");

  (* Forge a well-formed wire payload whose label exceeds the IDL bound (8)
     via the raw Wire.put_string primitive (no bound awareness — same
     adversarial-sender shape as the other backends' decode tests), and
     confirm decode rejects it too. *)
  let w = Wire.writer Wire.LE in
  Wire.put_string w "this-is-way-over-eight-chars";
  let forged = Wire.bytes w in
  (try
     let _ = S.unmarshal forged Wire.LE in
     print_endline "FAIL: decode did not reject over-bound value"
   with Failure _ -> print_endline "decode-rejected")
"#,
    );

    let dir = std::env::temp_dir().join(format!("idlocaml_bound_string_{}", std::process::id()));
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
    let run = Command::new(dir.join("main_bin")).output().expect("run");
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    assert!(
        stdout.contains("encode-rejected"),
        "encode must reject the over-bound value:\n{stdout}"
    );
    assert!(
        stdout.contains("decode-rejected"),
        "decode must reject the over-bound forged wire value:\n{stdout}"
    );
    assert!(!stdout.contains("FAIL"), "unexpected acceptance:\n{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}
