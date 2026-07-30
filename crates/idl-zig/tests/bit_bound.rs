// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Broad-audit P1: `@bit_bound(N)` selects the enum wire width — 1..8 → 1
//! octet, 9..16 → 2 octets, 17..32 / default → 4 octets (XTypes 1.3
//! §7.3.1.2.1.9 + §7.4.5.1). Toolchain-free string assertions over the real
//! emit path; the runtime byte-roundtrip verify runs on Linux/codepit.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

const REPRO: &str = "@bit_bound(8) enum Small { SA, SB, SC };\n\
@bit_bound(16) enum Med { MX, MY };\n\
enum Def { DP, DQ };\n\
@final struct Holder { Small s; Med m; Def d; };\n";

fn emit() -> String {
    let ast = zerodds_idl::parse(REPRO, &ParserConfig::default()).expect("parse");
    generate_zig_module(&ast, &ZigGenOptions::default()).expect("gen")
}

#[test]
fn bit_bound_selects_enum_wire_width() {
    let src = emit();
    // Encode: i32 enum tag truncated to the @bit_bound holder.
    assert!(
        src.contains("putU8(@bitCast(@as(i8, @truncate("),
        "1-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("putU16(@bitCast(@as(i16, @truncate("),
        "2-byte enum encode missing:\n{src}"
    );
    assert!(
        src.contains("putU32(@bitCast(@intFromEnum("),
        "4-byte enum encode missing:\n{src}"
    );
    // Decode: sign-extend the narrow holder to the i32 tag.
    assert!(
        src.contains("@as(i8, @bitCast(r.getU8()))"),
        "1-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("@as(i16, @bitCast(r.getU16()))"),
        "2-byte enum decode missing:\n{src}"
    );
    assert!(
        src.contains("@as(i32, @bitCast(r.getU32()))"),
        "4-byte enum decode missing:\n{src}"
    );
}
