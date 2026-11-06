// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-transport-tsn`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-tsn`, with the feature
//! `tsn-live` on Linux, in `runtime.rs` (`select_user_transport`,
//! `transport.user_transport = TSN`):
//! `socket::TsnTransport::bind(interface, tsn_vlan, tsn_pcp, Some(1 s))`
//! and the transport behind `Arc<dyn Transport + Send + Sync>`. The runtime
//! validates `tsn.vlan` as `0..=4094` and `tsn.pcp` as `0..=7` and passes
//! both on unchanged; every `send` then tags the frame through
//! `live_frame::outbound_vlan` and builds it with `live_frame::build_frame`,
//! the two helpers `socket.rs` calls for that and nothing else.
//!
//! Oracles: DDS-TSN 1.0 Annex A (the RTPS message directly in the Ethernet
//! payload), IEEE Std 802 (`0x88B5`, Local Experimental EtherType 1),
//! IEEE 802.1Q §9.6 (TPID `0x8100`, TCI = PCP(3) DEI(1) VID(12); VID 0 is
//! the null VID of a priority-tagged frame, Table 9-2, 4095 reserved),
//! IEEE 802.3 §4.4.2 (a minimum frame of 64
//! octets with the 4-octet FCS). No socket is opened: `bind` needs
//! `AF_PACKET` and `CAP_NET_RAW`.

use zerodds_transport_tsn::live_frame::{ETH_MIN_FRAME, build_frame, outbound_vlan};
use zerodds_transport_tsn::vlan_tag::{Ieee802VlanTag, TPID_8021Q};

const LOCAL: [u8; 6] = [0x02, 0x00, 0x5e, 0x00, 0x00, 0x01];
const PEER: [u8; 6] = [0x02, 0x00, 0x5e, 0x00, 0x00, 0x02];

/// An RTPS message of `len` octets: the 20-octet header of DDSI-RTPS 2.5
/// §8.3.3, then filler.
fn rtps_message(len: usize) -> Vec<u8> {
    let mut out = b"RTPS".to_vec();
    out.extend_from_slice(&[2, 5, 0x01, 0xF0]);
    out.extend_from_slice(&[0x01, 0xF0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    out.resize(len, 0xA5);
    out
}

#[test]
fn every_vlan_and_priority_the_runtime_admits_tags_without_refusal() {
    for pcp in 0..=7_u8 {
        for vid in 1..=4094_u16 {
            let tag = Ieee802VlanTag {
                tpid: 0x8100,
                pcp,
                dei: false,
                vid,
            };
            assert_eq!(outbound_vlan(vid, pcp, 0), Ok(Some(tag)));
        }
    }
    assert_eq!(TPID_8021Q, 0x8100);
    // A destination locator that carries a VID wins over `tsn.vlan`.
    let tag = outbound_vlan(5, 3, 9)
        .ok()
        .flatten()
        .map(|t| (t.vid, t.pcp));
    assert_eq!(tag, Some((9, 3)));
}

#[test]
fn with_vlan_zero_a_priority_goes_in_a_priority_tagged_frame() {
    // `tsn.vlan = 0` (the default) and a destination without a VID: with
    // `tsn.pcp = 0` the frame goes untagged; with a priority it carries a
    // tag with the null VID 0 and that priority (IEEE 802.1Q §3
    // "priority-tagged frame", §9.6 Table 9-2), TCI = PCP << 13.
    assert_eq!(outbound_vlan(0, 0, 0), Ok(None));
    for pcp in 1..=7_u8 {
        let tag = outbound_vlan(0, pcp, 0).ok().flatten();
        assert_eq!(
            tag.map(|t| (t.vid, t.pcp, t.dei)),
            Some((0, pcp, false)),
            "pcp {pcp}"
        );
        let frame = build_frame(LOCAL, PEER, tag, &rtps_message(24));
        let tci = u16::from(pcp) << 13;
        assert_eq!(
            frame[12..16],
            [0x81, 0x00, (tci >> 8) as u8, tci as u8],
            "pcp {pcp}"
        );
        assert_eq!(frame[16..18], [0x88, 0xB5]);
    }
}

#[test]
fn rtps_messages_ride_in_an_88b5_frame_unchanged() {
    let message = rtps_message(64);

    let tagged = build_frame(LOCAL, PEER, outbound_vlan(5, 3, 0).ok().flatten(), &message);
    let mut expected = PEER.to_vec();
    expected.extend_from_slice(&LOCAL);
    // TPID, then TCI: PCP 3 = 011, DEI 0, VID 5 → 0x6005.
    expected.extend_from_slice(&[0x81, 0x00, 0x60, 0x05]);
    expected.extend_from_slice(&[0x88, 0xB5]);
    expected.extend_from_slice(&message);
    assert_eq!(tagged, expected);

    let untagged = build_frame(LOCAL, PEER, None, &message);
    assert_eq!(untagged[..12], expected[..12]);
    assert_eq!(untagged[12..14], [0x88, 0xB5]);
    assert_eq!(untagged[14..], message[..]);

    // A message shorter than the 46-octet minimum payload is padded with
    // zeros to 60 octets (64 less the FCS); the message itself is intact.
    assert_eq!(ETH_MIN_FRAME, 60);
    let short = rtps_message(24);
    let padded = build_frame(LOCAL, PEER, None, &short);
    assert_eq!(padded.len(), 60);
    assert_eq!(padded[14..14 + 24], short[..]);
    assert!(padded[14 + 24..].iter().all(|&octet| octet == 0));
}

#[cfg(target_os = "linux")]
#[test]
fn the_transport_goes_behind_the_shared_transport_trait() {
    fn behind_dyn<T: zerodds_transport::Transport + Send + Sync + 'static>() {}
    behind_dyn::<zerodds_transport_tsn::socket::TsnTransport>();
}
