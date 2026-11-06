// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-transport-shm` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/posix.rs`):
//! `wire_types::Locator::shm` for its own and its peer's locator,
//! `LocatorKind::Shm` to refuse every other destination, and the 16-octet
//! `address` as the segment token. Oracle: DDSI-RTPS 2.5 §9.3.2.4 (the
//! vendor range of locator kinds) and the ZeroDDS vendor id `0x01F0` in the
//! upper half of the kind. No segment is created.
#![allow(clippy::unwrap_used)]

use zerodds_rtps::wire_types::{Locator, LocatorKind};

#[test]
fn shm_locator_carries_the_token_and_no_port() {
    let token = *b"zerodds-shm-0001";
    let locator = Locator::shm(token);
    assert_eq!(locator.kind, LocatorKind::Shm);
    assert_eq!(locator.address, token);
    assert_eq!(locator.port, 0);
}

#[test]
fn rtps_9_3_2_4_shm_kind_is_zerodds_vendor_specific() {
    let value = LocatorKind::Shm.as_i32();
    assert!((0x0000_0003..=0x01ff_ffff).contains(&value));
    assert_eq!(
        value, 0x01F0_0001,
        "ZeroDDS vendor id 0x01F0 in the upper half"
    );
    assert_eq!(LocatorKind::from_i32(value).unwrap(), LocatorKind::Shm);
}

#[test]
fn other_kinds_are_told_apart_from_shm() {
    assert_ne!(Locator::udp_v4([127, 0, 0, 1], 7400).kind, LocatorKind::Shm);
    assert_ne!(Locator::uds([0; 16]).kind, LocatorKind::Shm);
    assert_ne!(Locator::INVALID.kind, LocatorKind::Shm);
    assert_ne!(
        Locator::shm([1; 16]),
        Locator::shm([2; 16]),
        "the token tells peers apart"
    );
}
