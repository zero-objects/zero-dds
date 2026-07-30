// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Broad-audit P1: `@bit_bound(N)` selects the enum wire width — 1..8 → 1
//! octet, 9..16 → 2 octets, 17..32 / default → 4 octets (XTypes 1.3
//! §7.3.1.2.1.9 + §7.4.5.1). Toolchain-free string assertions over the real
//! emit path. A runtime byte-roundtrip of the Go output is exercised locally
//! (`go test`) and confirms `@bit_bound(8)`→1 byte, `@bit_bound(16)`→2 bytes.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

const REPRO: &str = "@bit_bound(8) enum Small { SA, SB, SC };\n\
@bit_bound(16) enum Med { MX, MY };\n\
enum Def { DP, DQ };\n\
@final struct Holder { Small s; Med m; Def d; };\n";

fn emit() -> String {
    let ast = zerodds_idl::parse(REPRO, &ParserConfig::default()).expect("parse");
    generate_go_module(&ast, &GoGenOptions::default()).expect("gen")
}

#[test]
fn bit_bound_selects_enum_wire_width() {
    let src = emit();
    // Encode: @bit_bound(8) → PutU8, @bit_bound(16) → PutU16, default → PutU32.
    assert!(
        src.contains("PutU8(uint8("),
        "1-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("PutU16(uint16("),
        "2-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("PutU32(uint32("),
        "4-byte enum encode missing:\n{src}"
    );
    // Decode: sign-extend from the narrow signed holder to the enum's int32.
    assert!(
        src.contains("int8(r.GetU8())"),
        "1-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("int16(r.GetU16())"),
        "2-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("int32(r.GetU32())"),
        "4-byte enum decode missing:\n{src}"
    );
}
