// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `safe-crates-only` with `zerodds-discovery`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-discovery` (`src/lib.rs`, `Cargo.toml`):
//! it re-exports the crate as `safe_crates_only::zerodds_discovery`,
//! depends on it with `default-features = false` and, with its `safety`
//! feature, forwards `zerodds-discovery/safety` and `zerodds-discovery/alloc`
//! for the no_std safe-profile build. Every item of `zerodds-discovery` is
//! behind its `alloc`, so the target needs `safety` (see `Cargo.toml`).
//! What is left for a test is that the re-export is the crate and that its
//! no_std `alloc` surface is there.

#[test]
fn the_reexport_is_the_discovery_crate_with_its_alloc_surface() {
    assert_eq!(
        core::any::TypeId::of::<safe_crates_only::zerodds_discovery::PeerCapabilities>(),
        core::any::TypeId::of::<zerodds_discovery::PeerCapabilities>()
    );
    assert_eq!(
        core::any::TypeId::of::<safe_crates_only::zerodds_discovery::DiscoveredParticipantsCache>(),
        core::any::TypeId::of::<zerodds_discovery::DiscoveredParticipantsCache>()
    );
}
