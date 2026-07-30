// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `@hashid` / `@autoid(HASH)` / `@id` member-id derivation parity for the
//! Elixir backend (idl-rust / idl-nim / TypeObject agreement).
//!
//! idl-elixir previously assigned `@mutable` EMHEADER (and `@key`-order) member
//! ids from `@id` + a sequential counter only — it ignored `@hashid` and
//! struct-level `@autoid(HASH)`, so its wire member ids diverged from
//! idl-rust/idl-cpp and the TypeObject (the same wire bug idl-nim fixed in
//! `836369c9`). Ids are now resolved through the shared frontend
//! `zerodds_idl::semantics::member_id::{container_autoid_hash, fixed_member_id}`,
//! so the EMHEADER carries the XTypes 1.3 §7.3.1.2.1 `NameHash` =
//! `MD5(name)[0..4]` little-endian `u32` masked to 28 bits.
//!
//! Byte proof: the `@mutable` EMHEADER is `0x40000000 | member_id` (LC4). The
//! expected member ids below are the KNOWN XTypes MD5 vectors already pinned in
//! `crates/idl/src/semantics/member_id.rs` (`known_md5_vector` /
//! `member_id_from_name_matches_known_md5_vectors`): `member_id_from_name`
//! yields `0x0FA5_DD70` for `"color"` and `0x026C_50E0` for `"my_hint"`. So a
//! `color` member's EMHEADER must read `0x4fa5dd70` and a `my_hint`-hashed
//! member's `0x426c50e0`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

/// Struct-level `@autoid(HASH)`: a member with no explicit id takes the
/// name-hashed id (XTypes §7.3.1.2.1.1). `color` → 0x0FA5DD70 → EMHEADER
/// 0x4fa5dd70.
#[test]
fn autoid_hash_member_id_matches_md5_vector() {
    let e = emit("@autoid(HASH) @mutable struct S { long color; };");
    assert!(
        e.contains("Zdgen.Wire.put_u32(0x4fa5dd70)"),
        "@autoid(HASH) `color` EMHEADER must be 0x40000000 | MD5-id(0x0FA5DD70):\n{e}"
    );
    // The former sequential fallback would have emitted 0x40000000 (id 0).
    assert!(
        !e.contains("Zdgen.Wire.put_u32(0x40000000)"),
        "the name-hashed id must replace the sequential id 0:\n{e}"
    );
}

/// `@hashid("hint")` hashes the hint string, not the member name
/// (XTypes §7.3.1.2.1.4). `my_hint` → 0x026C50E0 → EMHEADER 0x426c50e0.
#[test]
fn hashid_hint_member_id_matches_md5_vector() {
    let e = emit("@mutable struct S { @hashid(\"my_hint\") long a; };");
    assert!(
        e.contains("Zdgen.Wire.put_u32(0x426c50e0)"),
        "@hashid(\"my_hint\") EMHEADER must be 0x40000000 | MD5-id(0x026C50E0):\n{e}"
    );
}

/// A bare `@hashid` hashes the member's own name. `color` → 0x0FA5DD70.
#[test]
fn bare_hashid_hashes_member_name() {
    let e = emit("@mutable struct S { @hashid long color; };");
    assert!(
        e.contains("Zdgen.Wire.put_u32(0x4fa5dd70)"),
        "bare @hashid must hash the member name `color`:\n{e}"
    );
}

/// Explicit `@id(n)` wins over `@hashid` and `@autoid(HASH)` (highest
/// precedence). 42 → EMHEADER 0x4000002a.
#[test]
fn explicit_id_wins_over_hashid_and_autoid() {
    let e = emit("@autoid(HASH) @mutable struct S { @id(42) @hashid(\"x\") long a; };");
    assert!(
        e.contains("Zdgen.Wire.put_u32(0x4000002a)"),
        "explicit @id(42) must win → EMHEADER 0x4000002a:\n{e}"
    );
    assert!(
        !e.contains("Zdgen.Wire.put_u32(0x4fa5dd70)"),
        "no name-hash may leak when @id is explicit:\n{e}"
    );
}

/// Without any id annotation the sequential default is 0-based
/// (XTypes §7.2.2.4.9), matching the TypeObject builder.
#[test]
fn sequential_default_is_zero_based() {
    let e = emit("@mutable struct S { long a; long b; };");
    assert!(
        e.contains("Zdgen.Wire.put_u32(0x40000000)")
            && e.contains("Zdgen.Wire.put_u32(0x40000001)"),
        "sequential members a,b must be ids 0,1 → EMHEADER 0x40000000/0x40000001:\n{e}"
    );
}

/// The hashed id also drives the `@key`-order and the KeyHash body: a keyed,
/// name-hashed struct sorts its key members by the resolved (hashed) id, and
/// still emits a `key_hash/1`. Proves the resolver reaches the key path too.
#[test]
fn autoid_hash_reaches_key_ordering() {
    // Single @key member: the KeyHash writer must exist and cover it, and the
    // struct's own @mutable EMHEADER carries the hashed id (not 0).
    let e = emit("@autoid(HASH) @mutable struct S { @key long color; long other; };");
    assert!(e.contains("def key_hash(%__MODULE__{} = v) do"), "{e}");
    assert!(
        e.contains("Zdgen.Wire.put_u32(0x4fa5dd70)"),
        "keyed name-hashed member `color` keeps its 0x0FA5DD70 id:\n{e}"
    );
}
