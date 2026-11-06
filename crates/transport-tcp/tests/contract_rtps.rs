// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-transport-tcp` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/tcp_transport.rs`,
//! `src/handshake.rs`): `wire_types::Locator` built with `tcp_v4` and
//! `tcp_v6`, its `kind`, `port` and `address` read back into a socket
//! address (`address[12..16]` for TCPv4, all 16 octets for TCPv6),
//! `LocatorKind::Tcpv4`/`Tcpv6`, and `VendorId::ZERODDS`, which the
//! handshake announces as `handshake::VENDOR_ID_ZERODDS`.
//! Oracle: DDSI-RTPS 2.5 §9.3.2.4 (vendor range of locator kinds, address
//! layout); the Fast DDS TCP kinds 4 and 8 for the compatibility switch.
//! No socket is opened.
#![allow(clippy::unwrap_used)]

use zerodds_rtps::wire_types::{
    Locator, LocatorKind, TCP_V4_FAST_DDS, TCP_V6_FAST_DDS, VendorId, set_tcp_kinds_fast_dds,
};

#[test]
fn the_handshake_announces_the_zerodds_vendor_id() {
    assert_eq!(
        zerodds_transport_tcp::handshake::VENDOR_ID_ZERODDS,
        VendorId::ZERODDS.to_bytes()
    );
    assert_ne!(
        zerodds_transport_tcp::handshake::VENDOR_ID_ZERODDS,
        VendorId::FASTDDS.to_bytes(),
        "the handshake must not claim to be Fast DDS"
    );
}

#[test]
fn tcpv4_locator_keeps_the_ipv4_address_in_the_last_four_octets() {
    let locator = Locator::tcp_v4([10, 1, 2, 3], 7410);
    assert_eq!(locator.kind, LocatorKind::Tcpv4);
    assert_eq!(locator.port, 7410);
    assert_eq!(locator.address[..12], [0; 12]);
    assert_eq!(locator.address[12..], [10, 1, 2, 3]);
}

#[test]
fn tcpv6_locator_keeps_all_sixteen_octets() {
    let octets: [u8; 16] = std::net::Ipv6Addr::LOCALHOST.octets();
    let locator = Locator::tcp_v6(octets, 7411);
    assert_eq!(locator.kind, LocatorKind::Tcpv6);
    assert_eq!(locator.port, 7411);
    assert_eq!(
        std::net::Ipv6Addr::from(locator.address),
        std::net::Ipv6Addr::LOCALHOST
    );
}

#[test]
fn rtps_9_3_2_4_tcp_kinds_lie_in_the_vendor_range_and_read_both_spellings() {
    // §9.3.2.4: 0x00000003..=0x01ffffff are vendor-specific kinds.
    for kind in [LocatorKind::Tcpv4, LocatorKind::Tcpv6] {
        let value = kind as i32;
        assert!((0x0000_0003..=0x01ff_ffff).contains(&value), "{kind:?}");
    }
    assert_eq!(
        LocatorKind::from_i32(TCP_V4_FAST_DDS).unwrap(),
        LocatorKind::Tcpv4
    );
    assert_eq!(
        LocatorKind::from_i32(TCP_V6_FAST_DDS).unwrap(),
        LocatorKind::Tcpv6
    );
    assert_eq!(
        LocatorKind::from_i32(LocatorKind::Tcpv4 as i32).unwrap(),
        LocatorKind::Tcpv4
    );
    assert_eq!(TCP_V4_FAST_DDS, 4);
    assert_eq!(TCP_V6_FAST_DDS, 8);
}

#[test]
fn the_fast_dds_switch_changes_the_wire_value_not_the_kind() {
    // Process-wide; this binary holds only this test that sets it, and
    // it puts the switch back before returning.
    set_tcp_kinds_fast_dds(true);
    let on = (LocatorKind::Tcpv4.as_i32(), LocatorKind::Tcpv6.as_i32());
    set_tcp_kinds_fast_dds(false);
    let off = (LocatorKind::Tcpv4.as_i32(), LocatorKind::Tcpv6.as_i32());
    assert_eq!(on, (4, 8));
    assert_eq!(off, (LocatorKind::Tcpv4 as i32, LocatorKind::Tcpv6 as i32));
    assert_eq!(Locator::tcp_v4([1, 2, 3, 4], 1).kind, LocatorKind::Tcpv4);
}

#[test]
fn a_udp_locator_is_not_a_tcp_locator() {
    let udp = Locator::udp_v4([10, 1, 2, 3], 7410);
    assert_ne!(udp.kind, LocatorKind::Tcpv4);
    assert_ne!(udp.kind, LocatorKind::Tcpv6);
    assert_ne!(Locator::INVALID.kind, LocatorKind::Tcpv4);
}
