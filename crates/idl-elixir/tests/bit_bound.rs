// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Broad-audit P1: `@bit_bound(N)` selects the enum wire width — 1..8 → 1
//! octet, 9..16 → 2 octets, 17..32 / default → 4 octets (XTypes 1.3
//! §7.3.1.2.1.9 + §7.4.5.1). Toolchain-free string assertions over the real
//! emit path; the runtime byte-roundtrip verify runs on Linux/codepit.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

const REPRO: &str = "@bit_bound(8) enum Small { SA, SB, SC };\n\
@bit_bound(16) enum Med { MX, MY };\n\
enum Def { DP, DQ };\n\
@final struct Holder { Small s; Med m; Def d; };\n";

fn emit() -> String {
    let ast = zerodds_idl::parse(REPRO, &ParserConfig::default()).expect("parse");
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

#[test]
fn bit_bound_selects_enum_wire_width() {
    let src = emit();
    // Encode: Wire.put_u8 / put_u16 / put_u32 by @bit_bound.
    assert!(
        src.contains("Wire.put_u8(v.s)"),
        "1-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("Wire.put_u16(v.m)"),
        "2-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("Wire.put_u32(v.d)"),
        "4-byte enum encode missing:\n{src}"
    );
    // Decode: matching narrow reader.
    assert!(
        src.contains("Wire.get_u8(r)"),
        "1-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("Wire.get_u16(r)"),
        "2-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("Wire.get_u32(r)"),
        "4-byte enum decode missing:\n{src}"
    );
}
