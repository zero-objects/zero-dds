// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-transport-uds` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/lib.rs`,
//! `src/abstract_dgram.rs`): `wire_types::Locator::uds(id)` for its own and
//! a source locator, the 16-octet `address` as the id it turns into a
//! socket path, `LocatorKind::Uds` to refuse every other destination, and
//! `Locator::INVALID` as the source of a datagram whose sender it cannot
//! name. Oracle: DDSI-RTPS 2.5 §9.3.2.1 (`LOCATOR_INVALID`) and §9.3.2.4
//! (vendor range of locator kinds). No socket is opened.
#![allow(clippy::unwrap_used)]

use zerodds_rtps::wire_types::{Locator, LocatorKind};

#[test]
fn uds_locator_carries_the_id_and_no_port() {
    let id = [0xa5; 16];
    let locator = Locator::uds(id);
    assert_eq!(locator.kind, LocatorKind::Uds);
    assert_eq!(locator.address, id);
    assert_eq!(locator.port, 0);
}

#[test]
fn rtps_9_3_2_4_uds_kind_is_vendor_specific() {
    let value = LocatorKind::Uds.as_i32();
    assert!((0x0000_0003..=0x01ff_ffff).contains(&value));
    assert_eq!(LocatorKind::from_i32(value).unwrap(), LocatorKind::Uds);
}

#[test]
fn rtps_9_3_2_1_an_unnamed_source_is_locator_invalid_not_a_uds_locator() {
    assert_ne!(Locator::INVALID.kind, LocatorKind::Uds);
    assert_eq!(Locator::INVALID.kind.as_i32(), -1);
    assert_ne!(
        Locator::INVALID,
        Locator::uds([0; 16]),
        "the all-zero id is a real id"
    );
}
