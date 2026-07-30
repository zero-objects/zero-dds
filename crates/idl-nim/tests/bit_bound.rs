// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Broad-audit P1: `@bit_bound(N)` selects the enum wire width — 1..8 → 1
//! octet, 9..16 → 2 octets, 17..32 / default → 4 octets (XTypes 1.3
//! §7.3.1.2.1.9 + §7.4.5.1). Toolchain-free string assertions over the real
//! emit path; the runtime byte-roundtrip verify runs on Linux/codepit.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};

const REPRO: &str = "@bit_bound(8) enum Small { SA, SB, SC };\n\
@bit_bound(16) enum Med { MX, MY };\n\
enum Def { DP, DQ };\n\
@final struct Holder { Small s; Med m; Def d; };\n";

fn emit() -> String {
    let ast = zerodds_idl::parse(REPRO, &ParserConfig::default()).expect("parse");
    generate_nim_module(&ast, &NimGenOptions::default()).expect("gen")
}

#[test]
fn bit_bound_selects_enum_wire_width() {
    let src = emit();
    // Encode: putU8/putU16 mask internally; the ordinal is passed as-is.
    assert!(
        src.contains("putU8(int(ord("),
        "1-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("putU16(int(ord("),
        "2-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("putU32(cast[uint32](int32(ord("),
        "4-byte enum encode missing:\n{src}"
    );
    // Decode: sign-extend via a signed cast before widening to int.
    assert!(
        src.contains("cast[int8](uint8(r.getU8()))"),
        "1-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("cast[int16](uint16(r.getU16()))"),
        "2-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("int(r.getU32())"),
        "4-byte enum decode missing:\n{src}"
    );
}
