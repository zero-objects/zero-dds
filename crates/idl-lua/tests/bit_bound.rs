// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Broad-audit P1: `@bit_bound(N)` selects the enum wire width — 1..8 → 1
//! octet, 9..16 → 2 octets, 17..32 / default → 4 octets (XTypes 1.3
//! §7.3.1.2.1.9 + §7.4.5.1). Toolchain-free string assertions over the real
//! emit path; the runtime byte-roundtrip verify runs on Linux/codepit.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};

const REPRO: &str = "@bit_bound(8) enum Small { SA, SB, SC };\n\
@bit_bound(16) enum Med { MX, MY };\n\
enum Def { DP, DQ };\n\
@final struct Holder { Small s; Med m; Def d; };\n";

fn emit() -> String {
    let ast = zerodds_idl::parse(REPRO, &ParserConfig::default()).expect("parse");
    generate_lua_module(&ast, &LuaGenOptions::default()).expect("gen")
}

#[test]
fn bit_bound_selects_enum_wire_width() {
    let src = emit();
    // Encode: 1-octet mask 0xff, 2-octet mask 0xffff, 4-octet mask 0xffffffff.
    assert!(
        src.contains(":putU8(v.s & 0xff)"),
        "1-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains(":putU16(v.m & 0xffff)"),
        "2-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains(":putU32(v.d & 0xffffffff)"),
        "4-byte enum encode missing:\n{src}"
    );
    // Decode: read the matching narrow holder.
    assert!(
        src.contains("v.s = r:getU8()"),
        "1-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("v.m = r:getU16()"),
        "2-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("v.d = r:getU32()"),
        "4-byte enum decode missing:\n{src}"
    );
}
