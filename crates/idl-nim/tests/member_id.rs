// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Member-ID derivation parity with `idl-rust` (findings A31/A32): explicit
//! `@id`, per-member `@hashid`, and container `@autoid(HASH)` all resolve
//! through the shared `zerodds_idl::semantics::member_id` frontend, so the
//! `@mutable` EMHEADER member ids on the wire match the TypeObject and the
//! Rust/C++/Java bindings.
//!
//! The expected hash ids are the independently-computed MD5 vectors from
//! XTypes 1.3 §7.3.1.2.1.1 (`id = MD5(name)[0..4] as LE u32, & 0x0FFF_FFFF`):
//! `"color"` → `0x0FA5_DD70`, `"my_hint"` → `0x026C_50E0`. The `@mutable`
//! EMHEADER wraps that id as `0x4000_0000 | id` (no `@must_understand` here).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_nim_module(&ast, &NimGenOptions::default()).expect("gen")
}

/// `@autoid(HASH)` hashes the member NAME into the EMHEADER member id, instead
/// of the sequential default. `"color"` must serialize under id `0x0FA5_DD70`
/// (EMHEADER `0x4FA5_DD70`), and no member may keep the sequential `0x4000_0001`
/// slot — every member of a `@autoid(HASH)` container is name-hashed.
#[test]
fn autoid_hash_member_id_is_name_hashed_ok() {
    let out = emit("@mutable @autoid(HASH) struct H { long color; unsigned short size; };");
    assert!(
        out.contains("putU32(uint32(0x4fa5dd70))"),
        "expected name-hashed EMHEADER for `color`, got:\n{out}"
    );
    assert!(
        !out.contains("putU32(uint32(0x40000001))"),
        "member `size` must be name-hashed under @autoid(HASH), not sequential id 1"
    );
}

/// A bare/annotated `@hashid("hint")` hashes the HINT string, overriding the
/// positional id for that member only; unannotated siblings keep the sequential
/// default. `@hashid("my_hint")` → `0x026C_50E0` (EMHEADER `0x426C_50E0`);
/// `plain` stays sequential id 0 (EMHEADER `0x4000_0000`).
#[test]
fn hashid_member_id_uses_hint_hash_ok() {
    let out = emit("@mutable struct HH { @hashid(\"my_hint\") long field; long plain; };");
    assert!(
        out.contains("putU32(uint32(0x426c50e0))"),
        "expected hint-hashed EMHEADER for @hashid(\"my_hint\"), got:\n{out}"
    );
    assert!(
        out.contains("putU32(uint32(0x40000000))"),
        "sibling `plain` must keep sequential id 0"
    );
}

/// Explicit `@id(n)` wins over a container `@autoid(HASH)` (XTypes precedence
/// order: `@id` → `@hashid` → `@autoid(HASH)` → sequential). The member takes
/// id 7 (EMHEADER `0x4000_0007`), NOT the name hash of `color`.
#[test]
fn explicit_id_wins_over_autoid_hash_ok() {
    let out = emit("@mutable @autoid(HASH) struct HX { @id(7) long color; };");
    assert!(
        out.contains("putU32(uint32(0x40000007))"),
        "explicit @id(7) must win over @autoid(HASH), got:\n{out}"
    );
    assert!(
        !out.contains("putU32(uint32(0x4fa5dd70))"),
        "the name hash of `color` must not appear once @id(7) is explicit"
    );
}

/// A plain `@mutable` struct (no `@autoid`, no `@id`/`@hashid`) keeps the
/// 0-based sequential member ids — the pre-existing behavior must not regress.
#[test]
fn sequential_autoid_stays_zero_based_ok() {
    let out = emit("@mutable struct S { long a; long b; long c; };");
    for emh in ["0x40000000", "0x40000001", "0x40000002"] {
        assert!(
            out.contains(&format!("putU32(uint32({emh}))")),
            "expected sequential EMHEADER {emh}, got:\n{out}"
        );
    }
}

/// KeyHash: a nested `@key` struct with its own `@autoid(HASH)` serializes its
/// key members in ascending HASHED member-id order (XTypes 1.3 §7.6.8), not
/// declaration order. `hash("my_hint")=0x026C_50E0 < hash("color")=0x0FA5_DD70`,
/// so `my_hint` must be written before `color`. Mirrors idl-rust
/// `compute_key_holder`/`encode_key_holder`.
#[test]
fn autoid_hash_orders_nested_key_members_by_hashed_id_ok() {
    let src = "@autoid(HASH) struct N { long color; long my_hint; };\n\
               struct Outer { @key N n; };";
    let out = emit(src);
    // Scope the search to the `keyHash` proc — the regular `marshalXCDR` emits
    // members in declaration order (color first), which is not the key order.
    let key_start = out
        .find("proc keyHash*(self: Outer)")
        .expect("keyHash proc for `Outer` missing");
    let key_body = &out[key_start..];
    let my_hint = key_body
        .find(".my_hint")
        .expect("key put for `my_hint` missing");
    let color = key_body
        .find(".color")
        .expect("key put for `color` missing");
    assert!(
        my_hint < color,
        "nested @autoid(HASH) key members must serialize my_hint (0x026c50e0) before color (0x0fa5dd70)"
    );
}
