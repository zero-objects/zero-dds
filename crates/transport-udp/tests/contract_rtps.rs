// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-transport-udp` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/udp_transport.rs`,
//! `src/recv_batch.rs`): `wire_types::Locator` built with `udp_v4` and
//! `udp_v6` for the local and the source address, its fields `kind`, `port`
//! and `address` read to build a socket address (`address[12..16]` for
//! UDPv4, all 16 octets for UDPv6), `LocatorKind::UdpV4`/`UdpV6` to tell the
//! families apart, and `Locator::INVALID`, which `send` refuses.
//! Oracle: DDSI-RTPS 2.5 §9.3.2.1 (the locator constants) and §9.3.2.4 (the
//! address layout). No socket is opened.
#![allow(clippy::unwrap_used)]

use zerodds_rtps::wire_types::{Locator, LocatorKind};

#[test]
fn rtps_9_3_2_1_udp_locator_kinds_are_1_and_2() {
    assert_eq!(LocatorKind::UdpV4.as_i32(), 1);
    assert_eq!(LocatorKind::UdpV6.as_i32(), 2);
    assert_eq!(LocatorKind::from_i32(1).unwrap(), LocatorKind::UdpV4);
    assert_eq!(LocatorKind::from_i32(2).unwrap(), LocatorKind::UdpV6);
}

#[test]
fn rtps_9_3_2_4_udpv4_address_sits_in_the_last_four_octets() {
    let locator = Locator::udp_v4([192, 168, 7, 9], 7411);
    assert_eq!(locator.kind, LocatorKind::UdpV4);
    assert_eq!(locator.port, 7411);
    assert_eq!(locator.address[..12], [0; 12], "leading 12 octets zero");
    assert_eq!(locator.address[12..], [192, 168, 7, 9]);
}

#[test]
fn rtps_9_3_2_4_udpv6_address_is_all_sixteen_octets() {
    // The example of §9.3.2.4: FF00:4501:0:0:0:0:0:32.
    let octets = [0xff, 0, 0x45, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x32];
    let locator = Locator::udp_v6(octets, 7412);
    assert_eq!(locator.kind, LocatorKind::UdpV6);
    assert_eq!(locator.port, 7412);
    assert_eq!(locator.address, octets);
    assert_eq!(
        std::net::Ipv6Addr::from(locator.address),
        "ff00:4501::32".parse::<std::net::Ipv6Addr>().unwrap()
    );
}

#[test]
fn rtps_9_3_2_1_locator_invalid_is_kind_minus_one_port_zero_address_zero() {
    let invalid = Locator::INVALID;
    assert_eq!(invalid.kind.as_i32(), -1);
    assert_eq!(invalid.port, 0);
    assert_eq!(invalid.address, [0; 16]);
    assert_ne!(invalid.kind, LocatorKind::UdpV4);
    assert_ne!(invalid.kind, LocatorKind::UdpV6);
}

#[test]
fn a_udp_port_above_u16_stays_visible_to_the_transport() {
    // `send` converts `port` with `u16::try_from` and refuses what does
    // not fit; the locator must carry the value unchanged for that.
    let locator = Locator::udp_v4([127, 0, 0, 1], 70_000);
    assert_eq!(locator.port, 70_000);
    assert!(u16::try_from(locator.port).is_err());
}

#[test]
fn locator_is_a_plain_value_the_transport_can_copy_and_compare() {
    fn value<T: Copy + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync + 'static>() {}
    value::<Locator>();
    let a = Locator::udp_v4([10, 0, 0, 1], 7400);
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, Locator::udp_v4([10, 0, 0, 1], 7401));
}
