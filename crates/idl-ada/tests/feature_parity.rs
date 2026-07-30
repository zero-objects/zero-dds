// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Feature-parity coverage of the Ada backend against `idl-rust` (#11): the
//! XCDR1 / PL_CDR1 wire path, const-expression array/sequence/map bounds, and
//! `@hashid` / `@autoid(HASH)` member ids. Assertions are on the emitted Ada
//! source (the wire semantics of each `Put_*`/PID/EMHEADER are byte-anchored to
//! `zerodds_cdr::xcdr1` and the XTypes MD5 member-id vectors); gnat-compiled
//! golden byte comparison is the Linux/CI boundary — GNAT is usually absent on
//! the macOS dev host, so no runtime `Marshal`/`Unmarshal` is exercised here.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ada::{AdaGenOptions, AdaModule, generate_ada_module};

fn emit(src: &str, xcdr1: bool) -> AdaModule {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ada_module(
        &ast,
        &AdaGenOptions {
            package_name: "Pgen".to_string(),
            xcdr1,
        },
    )
    .expect("gen")
}

// ---------------------------------------------------------------------------
// Gap 1 — XCDR1 / PL_CDR1 wire path
// ---------------------------------------------------------------------------

#[test]
fn xcdr1_sets_stream_max_align_8_xcdr2_stays_4() {
    // The reader alignment cap comes from the module constant; XCDR1 = 8
    // (full natural alignment), XCDR2 = 4 (§7.4.2 cap).
    let x1 = emit("struct S { long long a; };", true);
    assert!(
        x1.body
            .contains("Stream_Max_Align : constant Positive := 8;"),
        "{}",
        x1.body
    );
    let x2 = emit("struct S { long long a; };", false);
    assert!(
        x2.body
            .contains("Stream_Max_Align : constant Positive := 4;"),
        "{}",
        x2.body
    );
    // The writer buffer carries a per-buffer Max_Align field (default 4) so the
    // KeyHash buffer stays cap-4 even in an XCDR1 module.
    assert!(
        x1.body.contains("Max_Align : Positive := 4;"),
        "{}",
        x1.body
    );
    // 8-byte values align to 8 (capped by Max_Align); XCDR2 caps that to 4.
    assert!(x1.body.contains("Put_LE (W, 8,"), "{}", x1.body);
}

#[test]
fn xcdr1_mutable_struct_emits_pl_cdr1_short_header_and_sentinel() {
    // @id(7) unsigned long: id 7 < 0x3F00 → standard 16-bit PID header; the
    // parameter list ends with the 0x3F02 sentinel; body padded to 4.
    let m = emit("@mutable struct S { @id(7) unsigned long x; };", true);
    let b = &m.body;
    assert!(
        b.contains("Put_U16 (W, Unsigned_16 (7));"),
        "short id:\n{b}"
    );
    assert!(
        b.contains("Put_U16 (W, Unsigned_16 (M2.Len));"),
        "short len:\n{b}"
    );
    // Extended header is still emitted behind the runtime length guard.
    assert!(b.contains("if M2.Len > 16#FFFF# then"), "ext guard:\n{b}");
    assert!(b.contains("Put_U16 (W, 16#3F01#);"), "PID_EXTENDED:\n{b}");
    // Sentinel PID_LIST_END 0x3F02 + length 0.
    assert!(b.contains("Put_U16 (W, 16#3F02#);"), "sentinel:\n{b}");
    // 4-byte pad after the member body.
    assert!(
        b.contains("while (W.Len mod 4) /= 0 loop Put_U8 (W, 0); end loop;"),
        "pad:\n{b}"
    );
    // No XCDR2 DHEADER/EMHEADER framing in the XCDR1 mutable path.
    assert!(!b.contains("16#40000007#"), "no EMHEADER:\n{b}");
    // Decode is a PID dispatch loop keyed by member id, re-basing each body.
    assert!(
        b.contains("Zpid  : constant Unsigned_16"),
        "decode loop:\n{b}"
    );
    assert!(b.contains("when 7 =>"), "decode arm id 7:\n{b}");
    assert!(
        b.contains("Data : Byte_Array renames MB;"),
        "body re-base:\n{b}"
    );
}

#[test]
fn xcdr1_mutable_large_id_forces_extended_pid_unconditionally() {
    // A member id in the reserved 0x3FXX band (here via @id) is always extended.
    let m = emit("@mutable struct S { @id(16256) long x; };", true);
    let b = &m.body;
    // 16256 = 0x3F80 >= 0x3F00 → no runtime guard, unconditional extended PID.
    assert!(b.contains("Put_U16 (W, 16#3F01#);"), "{b}");
    assert!(b.contains("Put_U32 (W, 16256);"), "{b}");
    // The standard-form short id must NOT appear for this member.
    assert!(
        !b.contains("Put_U16 (W, Unsigned_16 (16256));"),
        "no short form:\n{b}"
    );
}

#[test]
fn xcdr1_appendable_has_no_dheader_but_xcdr2_does() {
    let src = "@appendable struct S { long a; long b; };";
    let x1 = emit(src, true);
    // XCDR1 @appendable serializes like @final: no B/DHEADER framing.
    assert!(
        !x1.body.contains("Put_U32 (W, Unsigned_32 (B.Len));"),
        "xcdr1 appendable has no DHEADER:\n{}",
        x1.body
    );
    assert!(!x1.body.contains("B : Buf_T;"), "no B buffer:\n{}", x1.body);
    let x2 = emit(src, false);
    // XCDR2 @appendable keeps its DHEADER-framed body.
    assert!(
        x2.body.contains("Put_U32 (W, Unsigned_32 (B.Len));"),
        "xcdr2 appendable keeps DHEADER:\n{}",
        x2.body
    );
}

#[test]
fn xcdr1_mutable_union_emits_pl_cdr1_disc_and_branch_ids() {
    let m = emit(
        "@mutable union U switch (long) { case 0: long a; case 1: short b; };",
        true,
    );
    let b = &m.body;
    // Discriminator is PL_CDR1 member id 0; branch ids are 1 and 2.
    assert!(
        b.contains("Put_U16 (W, Unsigned_16 (0));"),
        "disc id 0:\n{b}"
    );
    assert!(
        b.contains("Put_U16 (W, Unsigned_16 (1));"),
        "branch id 1:\n{b}"
    );
    assert!(
        b.contains("Put_U16 (W, Unsigned_16 (2));"),
        "branch id 2:\n{b}"
    );
    assert!(b.contains("Put_U16 (W, 16#3F02#);"), "sentinel:\n{b}");
    // Decode dispatch loop: id 0 → disc, ids 1/2 → branches.
    assert!(b.contains("when 0 =>"), "decode disc arm:\n{b}");
    assert!(b.contains("when 1 =>"), "decode branch arm:\n{b}");
}

#[test]
fn xcdr2_output_is_byte_stable_final_struct() {
    // A @final struct with 8-byte and 4-byte members: the XCDR2 emission must be
    // unchanged by the XCDR1 additions (the alignment field defaults to 4).
    let m = emit("@final struct S { long long a; long b; };", false);
    // 8-byte align arg is now 8, but capped to 4 by the default Max_Align, so
    // the XCDR2 bytes are identical to before.
    assert!(m.body.contains("Put_LE (W, 8,"), "{}", m.body);
    assert!(
        m.body
            .contains("Cap : constant Positive := (if A > W.Max_Align then W.Max_Align else A);"),
        "{}",
        m.body
    );
}

// ---------------------------------------------------------------------------
// Gap 2 — const-expression array / sequence / map bounds
// ---------------------------------------------------------------------------

#[test]
fn named_const_sequence_bound_is_resolved() {
    let m = emit(
        "const long MAX = 4; struct S { sequence<long, MAX> s; };",
        false,
    );
    assert!(
        m.body.contains("sequence length exceeds its IDL bound (4)"),
        "{}",
        m.body
    );
}

#[test]
fn binary_const_expr_array_size_is_folded() {
    // 2 + 2 = 4 → Ada index range 0 .. 3.
    let m = emit("struct S { long a[2 + 2]; };", false);
    assert!(m.spec.contains("array (0 .. 3)"), "{}", m.spec);
}

#[test]
fn shift_const_expr_octet_sequence_bound_is_folded() {
    // 1 << 3 = 8.
    let m = emit("struct S { sequence<octet, 1 << 3> s; };", false);
    assert!(
        m.body.contains("sequence length exceeds its IDL bound (8)"),
        "{}",
        m.body
    );
}

#[test]
fn enumerator_as_array_bound_is_resolved() {
    // enum C is the 3rd enumerator → value 2 → range 0 .. 1.
    let m = emit("enum E { A, B, C }; struct S { long a[C]; };", false);
    assert!(m.spec.contains("array (0 .. 1)"), "{}", m.spec);
}

#[test]
fn chained_const_bound_resolves_regardless_of_order() {
    let m = emit(
        "const long A = 3; const long B = A + 1; struct S { sequence<long, B> s; };",
        false,
    );
    assert!(
        m.body.contains("sequence length exceeds its IDL bound (4)"),
        "{}",
        m.body
    );
}

#[test]
fn named_const_map_bound_is_resolved() {
    let m = emit(
        "const long CAP = 5; struct S { map<long, long, CAP> m; };",
        false,
    );
    assert!(
        m.body.contains("map length exceeds its IDL bound (5)"),
        "{}",
        m.body
    );
}

// ---------------------------------------------------------------------------
// Gap 3 — @hashid / @autoid(HASH) member ids (XTypes 1.3 §7.3.1.2.1)
// ---------------------------------------------------------------------------

#[test]
fn autoid_hash_member_id_matches_md5_vector_xcdr2() {
    // NameHash("color") = 0x0FA5DD70 (XTypes §7.3.1.2.1.1). In a @mutable XCDR2
    // struct the EMHEADER (LC4) is 0x40000000 | id = 0x4FA5DD70.
    let m = emit("@autoid(HASH) @mutable struct S { long color; };", false);
    assert!(m.body.contains("Put_U32 (B, 16#4FA5DD70#);"), "{}", m.body);
}

#[test]
fn hashid_hint_member_id_matches_md5_vector_xcdr2() {
    // NameHash("my_hint") = 0x026C50E0 → EMHEADER 0x426C50E0.
    let m = emit("@mutable struct S { @hashid(\"my_hint\") long a; };", false);
    assert!(m.body.contains("Put_U32 (B, 16#426C50E0#);"), "{}", m.body);
}

#[test]
fn explicit_id_wins_over_autoid_and_hashid() {
    // @id(42) has highest precedence → EMHEADER 0x4000002A.
    let m = emit(
        "@autoid(HASH) @mutable struct S { @id(42) @hashid(\"x\") long a; };",
        false,
    );
    assert!(m.body.contains("Put_U32 (B, 16#4000002A#);"), "{}", m.body);
}

#[test]
fn autoid_hash_member_id_in_xcdr1_pl_cdr1_extended_pid() {
    // The hash id 0x0FA5DD70 (262528368) is >= 0x3F00, so PL_CDR1 must use the
    // extended PID with the full 32-bit member id.
    let m = emit("@autoid(HASH) @mutable struct S { long color; };", true);
    assert!(m.body.contains("Put_U32 (W, 262528368);"), "{}", m.body);
    assert!(m.body.contains("Put_U16 (W, 16#3F01#);"), "{}", m.body);
}

#[test]
fn bare_hashid_hashes_member_name() {
    // A bare @hashid hashes the member's own name; "color" → 0x0FA5DD70.
    let m = emit("@mutable struct S { @hashid long color; };", false);
    assert!(m.body.contains("Put_U32 (B, 16#4FA5DD70#);"), "{}", m.body);
}

#[test]
fn sequential_ids_unaffected_by_hash_additions() {
    // Without @autoid/@hashid, members keep the sequential positional ids.
    let m = emit("@mutable struct S { long a; long b; };", false);
    assert!(m.body.contains("Put_U32 (B, 16#40000000#);"), "{}", m.body);
    assert!(m.body.contains("Put_U32 (B, 16#40000001#);"), "{}", m.body);
}

// ---------------------------------------------------------------------------
// Narrow-enum sign extension (flagged idl-ada open point, now closed)
// ---------------------------------------------------------------------------

#[test]
fn narrow_signed_enum_decode_sign_extends() {
    // A @bit_bound(8) enum with a negative enumerator: decode must sign-extend
    // the 1-octet holder to the 32-bit wire value (mirrors idl-rust i32::from).
    let m = emit(
        "@bit_bound(8) enum E { @value(-1) NEG, ZERO, ONE }; struct S { E e; };",
        false,
    );
    assert!(
        m.body
            .contains("Unsigned_32'Mod (Integer_32 (U8_I8 (Get_U8 (Data, Pos))))"),
        "{}",
        m.body
    );
}
