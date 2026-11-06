// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-isolation-smoke` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`): it builds the peer
//! locator for each transport it probes with `wire_types::Locator::udp_v4`,
//! `Locator::shm` and `Locator::uds`, and hands it to that transport's
//! `send`. The contract is that each constructor yields the kind its
//! transport accepts. No socket is opened.

use zerodds_rtps::wire_types::{Locator, LocatorKind};

#[test]
fn each_peer_constructor_yields_the_kind_of_its_transport() {
    assert_eq!(
        Locator::udp_v4([127, 0, 0, 1], 7400).kind,
        LocatorKind::UdpV4
    );
    assert_eq!(Locator::shm([7; 16]).kind, LocatorKind::Shm);
    assert_eq!(Locator::uds([7; 16]).kind, LocatorKind::Uds);
}

#[test]
fn the_peer_id_reaches_the_transport_unchanged() {
    let id = *b"isolation-peer-1";
    assert_eq!(Locator::shm(id).address, id);
    assert_eq!(Locator::uds(id).address, id);
    let udp = Locator::udp_v4([10, 0, 0, 2], 7411);
    assert_eq!(udp.address[12..], [10, 0, 0, 2]);
    assert_eq!(udp.port, 7411);
}
