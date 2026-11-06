// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-transport-tsn` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/socket.rs`):
//! `wire_types::Locator::tsn(mac, vid)` for its own and a source locator,
//! `LocatorKind::Tsn` to refuse every other destination, and the address
//! layout it reads back in `send`: MAC in octets 0..6, the 802.1Q VLAN id
//! big-endian in octets 6..8. Oracle: DDSI-RTPS 2.5 §9.3.2.4 (vendor range
//! of locator kinds), IEEE 802.1Q (a VID is 12 bits). No socket is opened.
#![allow(clippy::unwrap_used)]

use zerodds_rtps::wire_types::{Locator, LocatorKind};

#[test]
fn tsn_locator_puts_mac_then_vlan_id_big_endian() {
    let mac = [0x02, 0x00, 0x5e, 0x10, 0x20, 0x30];
    let locator = Locator::tsn(mac, 0x0abc);
    assert_eq!(locator.kind, LocatorKind::Tsn);
    assert_eq!(locator.port, 0);
    assert_eq!(locator.address[..6], mac);
    assert_eq!(
        u16::from_be_bytes([locator.address[6], locator.address[7]]),
        0x0abc
    );
    assert_eq!(locator.address[8..], [0; 8]);
}

#[test]
fn an_untagged_endpoint_has_vlan_id_zero() {
    let locator = Locator::tsn([1, 2, 3, 4, 5, 6], 0);
    assert_eq!(locator.address[6..8], [0, 0]);
}

#[test]
fn rtps_9_3_2_4_tsn_kind_is_vendor_specific_and_distinct() {
    let value = LocatorKind::Tsn.as_i32();
    assert!((0x0000_0003..=0x01ff_ffff).contains(&value));
    assert_eq!(LocatorKind::from_i32(value).unwrap(), LocatorKind::Tsn);
    for other in [
        LocatorKind::UdpV4,
        LocatorKind::UdpV6,
        LocatorKind::Shm,
        LocatorKind::Uds,
    ] {
        assert_ne!(other, LocatorKind::Tsn);
    }
}
