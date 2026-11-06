// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `safe-crates-only` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/lib.rs`, `Cargo.toml`): it
//! re-exports the crate as `safe_crates_only::zerodds_rtps` and forwards its
//! `safety` and `alloc` features for the no_std safe-profile build. Cargo
//! itself refuses the manifest if a forwarded feature is gone; what is left
//! for a test is that the re-export is the crate.

#[test]
fn the_reexport_is_the_rtps_crate() {
    assert_eq!(
        core::any::TypeId::of::<safe_crates_only::zerodds_rtps::wire_types::GuidPrefix>(),
        core::any::TypeId::of::<zerodds_rtps::wire_types::GuidPrefix>()
    );
}
