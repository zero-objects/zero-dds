// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Broad-audit P1: `@bit_bound(N)` selects the enum wire width — 1..8 → 1
//! octet, 9..16 → 2 octets, 17..32 / default → 4 octets (XTypes 1.3
//! §7.3.1.2.1.9 + §7.4.5.1). Toolchain-free string assertions over the real
//! emit path; the runtime byte-roundtrip verify runs on Linux/codepit.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ada::{AdaGenOptions, generate_ada_module};

const REPRO: &str = "@bit_bound(8) enum Small { SA, SB, SC };\n\
@bit_bound(16) enum Med { MX, MY };\n\
enum Def { DP, DQ };\n\
@final struct Holder { Small s; Med m; Def d; };\n";

fn emit_body() -> String {
    let ast = zerodds_idl::parse(REPRO, &ParserConfig::default()).expect("parse");
    generate_ada_module(&ast, &AdaGenOptions::default())
        .expect("gen")
        .body
}

#[test]
fn bit_bound_selects_enum_wire_width() {
    let src = emit_body();
    // Encode: narrow the Unsigned_32 codec value to 1/2 octets by mask.
    assert!(
        src.contains("Unsigned_8 (Small_To_U32 (V.s) and 16#FF#)"),
        "1-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("Unsigned_16 (Med_To_U32 (V.m) and 16#FFFF#)"),
        "2-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("Put_U32 (W, Def_To_U32 (V.d))"),
        "4-byte enum encode missing:\n{src}"
    );
    // Decode: read the matching narrow holder (Get_U8 takes no Endian) and
    // SIGN-extend it to the 32-bit wire value — the enum holder is signed
    // (§7.4.5.1), so a negative enumerator under a narrow bound roundtrips
    // (mirrors idl-rust's `i32::from(i8)`).
    assert!(
        src.contains("Small_Of_U32 (Unsigned_32'Mod (Integer_32 (U8_I8 (Get_U8 (Data, Pos)))))"),
        "1-byte enum sign-extended decode missing:\n{src}"
    );
    assert!(
        src.contains(
            "Med_Of_U32 (Unsigned_32'Mod (Integer_32 (U16_I16 (Get_U16 (Data, Pos, Endian)))))"
        ),
        "2-byte enum sign-extended decode missing:\n{src}"
    );
    assert!(
        src.contains("Def_Of_U32 (Get_U32 (Data, Pos, Endian))"),
        "4-byte enum decode missing:\n{src}"
    );
}
