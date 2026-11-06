// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `safe-crates-only` with `zerodds-transport-udp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-udp` (`src/lib.rs`,
//! `Cargo.toml`): it re-exports the crate as
//! `safe_crates_only::zerodds_transport_udp`, depends on it with
//! `default-features = false` and, with its `safety` feature, forwards
//! `zerodds-transport-udp/safety` and `zerodds-transport-udp/alloc` for the
//! no_std safe-profile build (the target needs `safety`, see `Cargo.toml`).
//! Without `std` the crate exports no item — the socket transport, the
//! interface list and the batch receive are all behind `std` — so the
//! contract is the build: `zerodds-transport-udp` compiles as `no_std` with
//! `alloc` and `safety`, and the re-export resolves.

#[test]
fn the_reexport_resolves_in_the_no_std_safe_profile() {
    #[allow(unused_imports)]
    use safe_crates_only::zerodds_transport_udp as _;
}
