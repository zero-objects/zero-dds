// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `safe-crates-only` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/lib.rs`, `Cargo.toml`): it
//! re-exports the crate as `safe_crates_only::zerodds_wire`. `zerodds-wire`
//! has no features, so `Cargo.toml` forwards none; the edge is the
//! safe-profile build gate. What is left for a test is that the re-export
//! is the crate.

#[test]
fn the_reexport_is_the_wire_crate() {
    assert_eq!(
        core::any::TypeId::of::<safe_crates_only::zerodds_wire::Writer>(),
        core::any::TypeId::of::<zerodds_wire::Writer>()
    );
    assert_eq!(
        core::any::TypeId::of::<safe_crates_only::zerodds_wire::Reader<'static>>(),
        core::any::TypeId::of::<zerodds_wire::Reader<'static>>()
    );
}
