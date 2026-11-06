// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-transport-udp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-udp`, all in `runtime.rs`:
//! - `UdpTransport`: the `bind_v4_with_config`, `bind_v6_with_config`,
//!   `bind_v4_reuse_with_config`, `bind_multicast_v4_on_with_config` and
//!   `bind_multicast_sender_v4_with_config` binds, `with_timeout`, and the
//!   `zerodds_transport::Transport` impl behind `Arc<dyn Transport + Send +
//!   Sync>` (`select_user_transport`, the SPDP sockets, `InterfaceSockets`).
//! - `UdpTransportConfig`: built field by field from the `transport.udp.*`
//!   settings over `UdpTransportConfig::default()` (`udp_config_from`); its
//!   default `max_message_size` is taken as the 1472 octets the writer's
//!   fragment size follows from.
//! - `MAX_DATAGRAM_SIZE`: the upper bound of `transport.udp.max_message_size`
//!   (`20..=MAX_DATAGRAM_SIZE`, help text "20..=65507").
//! - `eligible_ipv4_interfaces`: the announced unicast locators
//!   (`announce_locators`, which takes the list as sorted by address and free
//!   of loopback and link-local addresses), the SPDP multicast interfaces and
//!   `local_ipv4_addresses`.
//!
//! Oracles: RFC 768 (UDP header, 8 octets), RFC 791 (IPv4 header of 20
//! octets, 16-bit total length), RFC 894 (Ethernet payload of 1500 octets),
//! RFC 1122 §3.2.1.3 and RFC 3927 (loopback, unspecified, link-local),
//! RFC 5771 (multicast), DDSI-RTPS 2.5 §8.3.3 (the 20-octet message header),
//! and the documented order of `eligible_ipv4_interfaces`. No socket is
//! opened: every other use of `UdpTransport` binds one and is left to the
//! integration pillar.

use zerodds_transport::Transport;
use zerodds_transport_udp::{
    EligibleInterface, MAX_DATAGRAM_SIZE, UdpTransport, UdpTransportConfig,
    eligible_ipv4_interfaces,
};

/// RFC 791: the IPv4 header without options.
const IPV4_HEADER: usize = 20;
/// RFC 768: the UDP header.
const UDP_HEADER: usize = 8;
/// DDSI-RTPS 2.5 §8.3.3: the RTPS message header, the smallest message.
const RTPS_HEADER: usize = 20;

#[test]
fn the_largest_udp_datagram_over_ipv4_is_65507_octets() {
    // A 16-bit total length less the two headers.
    assert_eq!(MAX_DATAGRAM_SIZE, 65_535 - IPV4_HEADER - UDP_HEADER);
    assert_eq!(
        MAX_DATAGRAM_SIZE, 65_507,
        "the runtime's help text names it"
    );
}

#[test]
fn the_defaults_pass_the_runtimes_validation_and_fit_an_ethernet_frame() {
    let d = UdpTransportConfig::default();
    // RFC 894: 1500 octets of Ethernet payload carry IPv4 + UDP + data.
    assert_eq!(d.max_message_size, 1500 - IPV4_HEADER - UDP_HEADER);
    // `transport.udp.max_message_size` is validated against
    // `20..=MAX_DATAGRAM_SIZE`, `transport.udp.cache_max_entries` against
    // `> 0`; the compiled defaults the runtime reports have to pass both.
    assert!((RTPS_HEADER..=MAX_DATAGRAM_SIZE).contains(&d.max_message_size));
    assert!(d.connected_cache_max_entries > 0);
}

#[test]
fn eligible_interfaces_are_routable_unicast_and_in_ascending_order() {
    let found = eligible_ipv4_interfaces();
    for e in &found {
        let ip = e.ipv4;
        assert!(!ip.is_loopback(), "{e:?}: 127/8 (RFC 1122)");
        assert!(!ip.is_unspecified(), "{e:?}: 0.0.0.0 (RFC 1122)");
        assert!(!ip.is_link_local(), "{e:?}: 169.254/16 (RFC 3927)");
        assert!(!ip.is_broadcast(), "{e:?}: 255.255.255.255");
        assert!(!ip.is_multicast(), "{e:?}: 224/4 (RFC 5771)");
    }
    // Strictly ascending by (address, name, index): sorted, and no tuple
    // twice. `announce_locators` keeps this order for the SPDP announcement.
    let key = |e: &EligibleInterface| (e.ipv4.octets(), e.name.clone(), e.index);
    for pair in found.windows(2) {
        assert!(key(&pair[0]) < key(&pair[1]), "{pair:?}");
    }
    // Independent of the enumeration order of the OS: a second call agrees.
    assert_eq!(eligible_ipv4_interfaces(), found);
}

#[test]
fn the_transport_goes_behind_the_shared_transport_trait() {
    // `Arc<UdpTransport>` is coerced to `Arc<dyn Transport + Send + Sync>`
    // and moved into the receive threads.
    fn behind_dyn<T: Transport + Send + Sync + 'static>() {}
    behind_dyn::<UdpTransport>();
}
