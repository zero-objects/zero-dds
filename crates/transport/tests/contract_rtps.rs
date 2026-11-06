// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-transport` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/lib.rs`): it re-exports
//! `wire_types::Locator` as `zerodds_transport::Locator`, the address type of
//! the `Transport` trait every transport implements. The contract is that
//! the two are one type, so a locator built with `zerodds_rtps` is accepted
//! where the trait asks for one.

use zerodds_rtps::wire_types::{Locator, LocatorKind};

#[test]
fn the_transport_locator_is_the_rtps_locator() {
    let built_by_rtps: Locator = Locator::udp_v4([127, 0, 0, 1], 7400);
    let seen_by_transport: zerodds_transport::Locator = built_by_rtps;
    assert_eq!(seen_by_transport.kind, LocatorKind::UdpV4);
    assert_eq!(
        core::any::TypeId::of::<zerodds_transport::Locator>(),
        core::any::TypeId::of::<Locator>()
    );
}
