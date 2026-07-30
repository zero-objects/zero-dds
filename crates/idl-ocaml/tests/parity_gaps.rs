// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCaml backend feature-parity gaps closed against idl-rust (#11):
//! `@hashid`/`@autoid(HASH)` member ids (XTypes MD5 vectors), const-expression
//! collection/array bounds and union labels, narrow/negative enum sign-extension
//! on decode, and `@mutable` union framing. String-level over the real emit path
//! (the runtime byte round-trip runs in `xcdr1.rs` under `ocamlfind`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};
use zerodds_types::type_object::common::NameHash;

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).expect("gen")
}

// ---- @hashid / @autoid(HASH) member ids (findings A31/A32) -------------------

#[test]
fn autoid_hash_member_id_matches_md5_vector() {
    // XTypes §7.3.1.2.1.1: id = MD5(name)[0..4] LE u32 & 0x0FFFFFFF.
    assert_eq!(NameHash::member_id_from_name("color"), 0x0FA5_DD70);
    let s = emit("@autoid(HASH) @mutable struct H { long color; };");
    // PL_CDR1 uses the raw member id; the EMHEADER carries LC4 | id.
    assert!(s.contains("Wire.put_pl_cdr1_member w 0x0fa5dd70"), "{s}");
    assert!(s.contains("Wire.put_u32 body 0x4fa5dd70"), "{s}");
}

#[test]
fn hashid_hint_member_id_matches_md5_vector() {
    assert_eq!(NameHash::member_id_from_name("my_hint"), 0x026C_50E0);
    let s = emit("@mutable struct H { @hashid(\"my_hint\") long x; };");
    assert!(s.contains("Wire.put_pl_cdr1_member w 0x026c50e0"), "{s}");
    assert!(s.contains("Wire.put_u32 body 0x426c50e0"), "{s}");
}

#[test]
fn bare_hashid_hashes_member_name() {
    let id = NameHash::member_id_from_name("color");
    let s = emit("@mutable struct H { @hashid long color; };");
    assert!(
        s.contains(&format!("Wire.put_pl_cdr1_member w 0x{id:08x}")),
        "{s}"
    );
}

#[test]
fn explicit_id_wins_over_autoid_and_hashid() {
    let s = emit("@autoid(HASH) @mutable struct H { @id(42) @hashid(\"x\") long a; };");
    // id 42 = 0x2a; PL_CDR1 raw id, EMHEADER LC4 | 42.
    assert!(s.contains("Wire.put_pl_cdr1_member w 0x0000002a"), "{s}");
    assert!(s.contains("Wire.put_u32 body 0x4000002a"), "{s}");
}

// ---- const-expression bounds (array / sequence / string / map / labels) -----

#[test]
fn const_expr_array_and_sequence_bounds_resolve() {
    let s = emit("const long N = 3; @final struct A { long arr[N+1]; sequence<long,N*2> s; };");
    // arr[N+1] = arr[4] → row-major loop 0..3.
    assert!(s.contains("for zdi0 = 0 to 3 do"), "{s}");
    // sequence<long, N*2> → bound 6 enforced on encode + decode.
    assert!(
        s.contains("bounded sequence length exceeds its IDL bound (6)"),
        "{s}"
    );
    assert!(
        s.contains("decoded sequence length exceeds its IDL bound (6)"),
        "{s}"
    );
}

#[test]
fn const_expr_string_and_map_bounds_resolve() {
    let s = emit("const long K = 4; @final struct B { string<K*2> s; map<long,long,K+1> m; };");
    assert!(
        s.contains("bounded string length exceeds its IDL bound (8)"),
        "{s}"
    );
    assert!(
        s.contains("bounded map length exceeds its IDL bound (5)"),
        "{s}"
    );
}

#[test]
fn const_expr_union_label_resolves() {
    let s = emit(
        "const long BASE = 10; \
         union U switch(long) { case BASE + 1: long a; default: octet b; };",
    );
    // The label `BASE + 1` resolves to 11.
    assert!(s.contains("| 11 ->"), "{s}");
}

// ---- narrow / negative enum sign-extension on decode ------------------------

#[test]
fn negative_enumerator_sign_extends_on_decode() {
    // A 4-byte enum with a negative enumerator: read unsigned, then fold into
    // the signed range before mapping (else `_of_int` never matches -1).
    let s = emit(
        "enum Sign { @value(-1) NEG, @value(0) ZERO, @value(1) POS };\n\
         @final struct S { Sign a; };",
    );
    assert!(
        s.contains("if zdv >= 2147483648 then zdv - 4294967296"),
        "{s}"
    );
}

#[test]
fn narrow_negative_enum_sign_extends_at_holder_width() {
    let s = emit(
        "@bit_bound(8) enum SN { @value(-2) A, @value(3) B };\n\
         @final struct S { SN b; };",
    );
    // 1-byte holder: fold at 0x80 over 0x100.
    assert!(
        s.contains("Wire.get_u8 r in if zdv >= 128 then zdv - 256 else zdv"),
        "{s}"
    );
}

#[test]
fn non_negative_enum_keeps_unsigned_read() {
    // All-non-negative enum: no sign-extension (byte-identical to before).
    let s = emit("enum E { A, B, C };\n@final struct S { E e; };");
    assert!(s.contains("(e_of_int (Wire.get_u32 r))"), "{s}");
    assert!(!s.contains("zdv - 4294967296"), "{s}");
}

// ---- @mutable union framing (previously rejected) ---------------------------

#[test]
fn mutable_union_is_emitted_not_rejected() {
    let s = emit("@mutable union U switch(long) { case 1: unsigned long a; default: octet c; };");
    // Discriminator = PL_CDR1 member id 0; selected branch = member id (idx+1).
    assert!(s.contains("Wire.put_pl_cdr1_member w 0x00000000"), "{s}");
    assert!(s.contains("Wire.put_pl_cdr1_member w 0x00000001"), "{s}");
    assert!(s.contains("Wire.put_pl_cdr1_sentinel w"), "{s}");
    // XCDR2 PL_CDR2: disc EMHEADER carries the must-understand bit (0xc0000000).
    assert!(s.contains("Wire.put_u32 zdBody 0xc0000000"), "{s}");
    // Decode is table-based (id → body), for both representations.
    assert!(s.contains("Wire.read_pl_cdr1_member r"), "{s}");
    assert!(s.contains("Wire.read_emheader_member r"), "{s}");
    // Both entry points present.
    assert!(s.contains("let marshal_xcdr1 (v : t)"), "{s}");
    assert!(s.contains("let unmarshal_xcdr1 (b : bytes)"), "{s}");
}
