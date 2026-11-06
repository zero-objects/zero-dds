// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `safe-crates-only` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/lib.rs`, `Cargo.toml`): it
//! re-exports the crate as `safe_crates_only::zerodds_types`, depends on it
//! with `default-features = false` (no `std`, no `dynamic`) and forwards its
//! `safety` and `alloc` features for the no_std safe-profile build. Cargo
//! refuses the manifest if a forwarded feature is gone; what is left for a
//! test is that the re-export is the crate, with the type identifier
//! present without any feature.

#[test]
fn the_reexport_is_the_types_crate() {
    assert_eq!(
        core::any::TypeId::of::<safe_crates_only::zerodds_types::TypeIdentifier>(),
        core::any::TypeId::of::<zerodds_types::TypeIdentifier>()
    );
}
