// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XCDR1 / classic-CDR conformance for the Lua backend (#11 parity with
//! idl-rust). Two layers:
//!
//! 1. Emit-string asserts (always run): the generated Lua carries the XCDR1
//!    branches — 8-byte alignment, no @appendable DHEADER, PL_CDR1 @mutable
//!    framing with the 0x3F02 sentinel — plus the shared member-id resolver
//!    (@hashid / @autoid(HASH), MD5 vectors) and const-expr bounds.
//!
//! 2. Byte-identity asserts (gated on a Lua interpreter on PATH; skip loudly):
//!    the generated `marshalCdr1_<T>` output is compared BYTE-FOR-BYTE against
//!    the authoritative `zerodds_cdr` XCDR1 writer (`BufferWriter` defaults to
//!    XCDR1 cap-8) and its PL_CDR1 framing (`xcdr1::encode_pl_cdr1_member` /
//!    `write_pl_cdr1_sentinel`), and each construct is round-tripped through
//!    `unmarshalCdr1_<T>`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_cdr::xcdr1::{encode_pl_cdr1_member, write_pl_cdr1_sentinel};
use zerodds_cdr::{BufferWriter, Endianness};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_lua_module(&ast, &LuaGenOptions::default()).expect("gen")
}

/// First Lua interpreter on PATH (the CI image ships `lua5.4`; a dev box may
/// have `lua5.5` / `lua`). `None` → the byte-identity layer skips loudly.
fn find_lua() -> Option<String> {
    for cand in ["lua5.4", "lua5.5", "lua"] {
        if Command::new(cand).arg("-v").output().is_ok() {
            return Some(cand.to_string());
        }
    }
    None
}

/// Runs `module ++ driver` and returns trimmed stdout. `driver` prints via the
/// `toHex` helper injected here.
fn run_lua(lua: &str, module: &str, driver: &str) -> String {
    let mut src = String::from(module);
    src.push_str(
        "\nlocal function toHex(s) local t={} for i=1,#s do t[i]=string.format('%02x', string.byte(s,i)) end return table.concat(t) end\n",
    );
    src.push_str(driver);
    let dir = std::env::temp_dir().join(format!(
        "idllua_xcdr1_{}_{}",
        std::process::id(),
        module.len()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new(lua).arg(&lf).output().expect("run lua");
    assert!(
        out.status.success(),
        "lua failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ------------------------------------------------------------------------
// Byte-identity: @final 8-byte alignment (XCDR1 cap 8 vs XCDR2 cap 4)
// ------------------------------------------------------------------------

#[test]
fn final_u64_alignment_xcdr1_is_eight_xcdr2_is_four() {
    let module = emit("@final struct S { octet a; uint64 b; };");
    let Some(lua) = find_lua() else {
        eprintln!("SKIP final_u64_alignment: no Lua interpreter on PATH (CI provides lua5.4)");
        return;
    };
    // Oracle: `octet` at 0, then `uint64` — XCDR1 aligns it to 8, XCDR2 to 4.
    let mut w1 = BufferWriter::new(Endianness::Little);
    w1.write_u8(0x11).unwrap();
    w1.write_u64(0x2233_4455_6677_8899).unwrap();
    let x1 = w1.into_bytes();
    let mut w2 = BufferWriter::new(Endianness::Little).xcdr2();
    w2.write_u8(0x11).unwrap();
    w2.write_u64(0x2233_4455_6677_8899).unwrap();
    let x2 = w2.into_bytes();
    assert_eq!(x1.len(), 16, "XCDR1: 1 + 7 pad + 8");
    assert_eq!(x2.len(), 12, "XCDR2: 1 + 3 pad + 8");

    let got1 = run_lua(
        &lua,
        &module,
        "print(toHex(marshalCdr1_S({ a = 0x11, b = 0x2233445566778899 }, LE)))",
    );
    let got2 = run_lua(
        &lua,
        &module,
        "print(toHex(marshal_S({ a = 0x11, b = 0x2233445566778899 }, LE)))",
    );
    assert_eq!(got1, hex(&x1), "XCDR1 bytes must match zerodds_cdr");
    assert_eq!(got2, hex(&x2), "XCDR2 bytes must stay unchanged");
    assert_ne!(got1, got2, "the two reps must differ (alignment)");
}

// ------------------------------------------------------------------------
// Byte-identity: @appendable — XCDR2 leads with a DHEADER, XCDR1 has none
// ------------------------------------------------------------------------

#[test]
fn appendable_xcdr1_has_no_dheader() {
    let module = emit("@appendable struct S { uint32 a; uint32 b; };");
    let Some(lua) = find_lua() else {
        eprintln!("SKIP appendable_xcdr1_has_no_dheader: no Lua interpreter on PATH");
        return;
    };
    // XCDR1: inline `a,b`, no framing.
    let mut w1 = BufferWriter::new(Endianness::Little);
    w1.write_u32(0xAABB_CCDD).unwrap();
    w1.write_u32(0x0011_2233).unwrap();
    let x1 = w1.into_bytes();
    // XCDR2: [DHEADER = body length = 8][a][b].
    let mut w2 = BufferWriter::new(Endianness::Little).xcdr2();
    w2.write_u32(8).unwrap();
    w2.write_u32(0xAABB_CCDD).unwrap();
    w2.write_u32(0x0011_2233).unwrap();
    let x2 = w2.into_bytes();

    let got1 = run_lua(
        &lua,
        &module,
        "print(toHex(marshalCdr1_S({ a = 0xAABBCCDD, b = 0x00112233 }, LE)))",
    );
    let got2 = run_lua(
        &lua,
        &module,
        "print(toHex(marshal_S({ a = 0xAABBCCDD, b = 0x00112233 }, LE)))",
    );
    assert_eq!(got1, hex(&x1), "XCDR1 appendable = inline, no DHEADER");
    assert_eq!(got2, hex(&x2), "XCDR2 appendable keeps the DHEADER");
    assert_eq!(
        x1.len() + 4,
        x2.len(),
        "XCDR2 carries 4 extra DHEADER bytes"
    );
}

// ------------------------------------------------------------------------
// Byte-identity: @mutable — PL_CDR1 PID list + 0x3F02 sentinel
// ------------------------------------------------------------------------

#[test]
fn mutable_xcdr1_is_pl_cdr1_with_sentinel() {
    let module = emit("@mutable struct S { uint32 a; uint16 b; };");
    let Some(lua) = find_lua() else {
        eprintln!("SKIP mutable_xcdr1_is_pl_cdr1_with_sentinel: no Lua interpreter on PATH");
        return;
    };
    // Oracle: two PL_CDR1 members (ids 0,1, each body member-relative) + sentinel.
    let mut w = BufferWriter::new(Endianness::Little);
    encode_pl_cdr1_member(&mut w, 0, |m| m.write_u32(0x1122_3344)).unwrap();
    encode_pl_cdr1_member(&mut w, 1, |m| m.write_u16(0x5566)).unwrap();
    write_pl_cdr1_sentinel(&mut w).unwrap();
    let x1 = w.into_bytes();

    let got1 = run_lua(
        &lua,
        &module,
        "print(toHex(marshalCdr1_S({ a = 0x11223344, b = 0x5566 }, LE)))",
    );
    assert_eq!(
        got1,
        hex(&x1),
        "PL_CDR1 member list + sentinel byte-identical"
    );
    // The sentinel PID_LIST_END (0x3F02) closes the list (LE: 02 3f 00 00).
    assert!(
        got1.ends_with("023f0000"),
        "must end with the sentinel: {got1}"
    );
}

// ------------------------------------------------------------------------
// Round-trip through unmarshalCdr1_<T> for the main constructs
// ------------------------------------------------------------------------

#[test]
fn xcdr1_round_trips_all_constructs() {
    let Some(lua) = find_lua() else {
        eprintln!("SKIP xcdr1_round_trips_all_constructs: no Lua interpreter on PATH");
        return;
    };
    // (idl, ctor, re-encode check on the decoded value)
    let cases: &[(&str, &str, &str)] = &[
        (
            "@final struct S { octet a; uint64 b; string s; };",
            "{ a = 3, b = 0x0102030405060708, s = 'hi' }",
            "S",
        ),
        (
            "@appendable struct S { uint32 a; sequence<uint32> xs; };",
            "{ a = 9, xs = { 1, 2, 3 } }",
            "S",
        ),
        (
            "@mutable struct S { uint32 a; uint16 b; string s; };",
            "{ a = 7, b = 0x2211, s = 'lua' }",
            "S",
        ),
        (
            "@final union U switch(long) { case 1: uint32 x; case 2: uint16 y; default: octet z; };",
            "{ disc = 1, x = 0xdeadbeef }",
            "U",
        ),
        (
            "@mutable union U switch(long) { case 1: uint32 x; default: octet z; };",
            "{ disc = 1, x = 0x44332211 }",
            "U",
        ),
    ];
    for (idl, ctor, ty) in cases {
        let module = emit(idl);
        // Decode the freshly-encoded bytes, then re-encode: byte-stable = ok.
        let driver = format!(
            "local v = {ctor}\nlocal enc = marshalCdr1_{ty}(v, LE)\nlocal dec = unmarshalCdr1_{ty}(enc, LE)\nlocal re = marshalCdr1_{ty}(dec, LE)\nprint(toHex(enc) == toHex(re) and 'ok' or ('MISMATCH ' .. toHex(enc) .. ' vs ' .. toHex(re)))"
        );
        let got = run_lua(&lua, &module, &driver);
        assert_eq!(got, "ok", "XCDR1 round-trip for `{idl}` failed: {got}");
    }
}

#[test]
fn xcdr1_big_endian_round_trips() {
    let Some(lua) = find_lua() else {
        eprintln!("SKIP xcdr1_big_endian_round_trips: no Lua interpreter on PATH");
        return;
    };
    let module = emit("@final struct S { octet a; uint64 b; };");
    // Oracle in big-endian.
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u8(0x11).unwrap();
    w.write_u64(0x0102_0304_0506_0708).unwrap();
    let x = w.into_bytes();
    let got = run_lua(
        &lua,
        &module,
        "print(toHex(marshalCdr1_S({ a = 0x11, b = 0x0102030405060708 }, BE)))",
    );
    assert_eq!(got, hex(&x), "XCDR1 BE bytes must match zerodds_cdr");
}

// ------------------------------------------------------------------------
// Emit-string: XCDR1 branches present (always run)
// ------------------------------------------------------------------------

#[test]
fn xcdr1_entry_points_and_prelude_emitted() {
    let l = emit("@mutable struct S { uint32 a; };");
    assert!(l.contains("function marshalCdr1_S(v, endian)"), "{l}");
    assert!(l.contains("function unmarshalCdr1_S(buf, endian)"), "{l}");
    assert!(l.contains("Writer.new(endian, true)"), "{l}");
    assert!(l.contains("Reader.new(buf, endian, true)"), "{l}");
    // PL_CDR1 helpers + sentinel present in the prelude / mutable path.
    assert!(l.contains("writePlCdr1Member"), "{l}");
    assert!(l.contains("writePlCdr1Sentinel"), "{l}");
    assert!(l.contains("0x3F02"), "{l}");
    // 8-byte alignment for 8-byte primitives (XCDR1) vs cap-4 (XCDR2).
    assert!(l.contains("self.xcdr1 and 8 or 4"), "{l}");
}

#[test]
fn appendable_dheader_is_xcdr1_conditional() {
    let l = emit("@appendable struct S { uint32 a; };");
    // Encode side: XCDR1 inline branch, XCDR2 DHEADER branch.
    assert!(l.contains("if w.xcdr1 then"), "{l}");
    assert!(l.contains("local bb = body:bytes()"), "{l}");
    // Decode side: DHEADER only skipped when not xcdr1.
    assert!(l.contains("if not r.xcdr1 then r:getU32() end"), "{l}");
}

// ------------------------------------------------------------------------
// Emit-string: shared member-id resolver (@hashid / @autoid(HASH))
// ------------------------------------------------------------------------

// XTypes 1.3 §7.3.1.2.1.1: NameHash("color") = MD5("color")[0..4] LE & 0x0FFFFFFF
// = 0x0FA5DD70 (the vector asserted in `zerodds_idl::semantics::member_id`).
const COLOR_ID: u32 = 0x0FA5_DD70;

fn emheader_hex(id: u32) -> String {
    format!("0x{:08x}", 0x4000_0000 | (id & 0x0FFF_FFFF))
}

#[test]
fn hashid_hint_member_id_matches_md5_vector() {
    // `@hashid("color")` hashes the hint string "color".
    let l = emit("@mutable struct S { @hashid(\"color\") uint32 x; };");
    assert!(
        l.contains(&format!("body:putU32({})", emheader_hex(COLOR_ID))),
        "{l}"
    );
    assert!(
        l.contains(&format!("writePlCdr1Member({COLOR_ID}",)),
        "PL_CDR1 must key the member by the hashed id {COLOR_ID}: {l}"
    );
}

#[test]
fn bare_hashid_hashes_member_name() {
    // Bare `@hashid` hashes the member's own name — here "color".
    let l = emit("@mutable struct S { @hashid uint32 color; };");
    assert!(
        l.contains(&format!("body:putU32({})", emheader_hex(COLOR_ID))),
        "{l}"
    );
}

#[test]
fn autoid_hash_container_hashes_member_names() {
    // Container `@autoid(HASH)`: every un-annotated member takes a name hash.
    let l = emit("@mutable @autoid(HASH) struct S { uint32 color; };");
    assert!(
        l.contains(&format!("body:putU32({})", emheader_hex(COLOR_ID))),
        "{l}"
    );
}

#[test]
fn explicit_id_wins_over_hashid_and_autoid() {
    // Precedence @id → @hashid → @autoid(HASH): explicit @id(42) wins.
    let l = emit("@mutable @autoid(HASH) struct S { @id(42) @hashid(\"color\") uint32 x; };");
    assert!(
        l.contains(&format!("body:putU32({})", emheader_hex(42))),
        "{l}"
    );
    assert!(
        !l.contains(&emheader_hex(COLOR_ID)),
        "hashed id must not appear: {l}"
    );
}

// ------------------------------------------------------------------------
// Emit-string: const-expr bounds (named consts + arithmetic)
// ------------------------------------------------------------------------

#[test]
fn array_bound_resolves_named_const() {
    let l = emit("const long MAX = 4; @final struct S { octet a[MAX]; };");
    assert!(
        l.contains("for zdi0 = 1, 4 do"),
        "array bound MAX=4 must fold: {l}"
    );
}

#[test]
fn array_bound_resolves_const_arithmetic() {
    let l = emit("const long N = 3; @final struct S { octet a[N * 2]; };");
    assert!(
        l.contains("for zdi0 = 1, 6 do"),
        "array bound N*2=6 must fold: {l}"
    );
}

#[test]
fn sequence_bound_resolves_const_arithmetic() {
    let l = emit("const long N = 3; @final struct S { sequence<octet, N * 2> s; };");
    assert!(
        l.contains("putSeqU8(v.s, 6)"),
        "seq bound N*2=6 must fold: {l}"
    );
}

#[test]
fn string_bound_resolves_named_const() {
    let l = emit("const long CAP = 8; @final struct S { string<CAP> s; };");
    assert!(
        l.contains("putString(v.s, 8)"),
        "string bound CAP=8 must fold: {l}"
    );
}

#[test]
fn union_label_resolves_named_const_and_arithmetic() {
    let l = emit(
        "const long SEL = 5; @final union U switch(long) { case SEL: uint32 x; case 1 + 1: uint16 y; default: octet z; };",
    );
    assert!(
        l.contains("v.disc == 5"),
        "named-const label SEL=5 must fold: {l}"
    );
    assert!(
        l.contains("v.disc == 2"),
        "arithmetic label 1+1=2 must fold: {l}"
    );
}
