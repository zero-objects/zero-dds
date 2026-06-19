//! WP 1.4 T6a — Cyclone fixture replay through the SedpStack.
//!
//! We take the real Cyclone 0.10.2 SEDP publication capture from
//! `crates/rtps/tests/fixtures/cyclone/sedp_publication.hex` and feed
//! it into a `SedpStack`. Expectation: the stack decodes all 4
//! publications and stores them byte-exact in the cache.
//!
//! This test is **deterministic** and CI-capable — needs no live
//! Cyclone instance. The real live-interop test lives as `#[ignore]`
//! in `tests/cyclone_live_sedp.rs`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use core::time::Duration;

use zerodds_discovery::sedp::SedpStack;
use zerodds_discovery::spdp::DiscoveredParticipant;
use zerodds_rtps::participant_data::{
    Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};

const FRAME_SEDP_PUBLICATION: &str =
    include_str!("../../rtps/tests/fixtures/cyclone/sedp_publication.hex");

/// The Cyclone capture was recorded with GuidPrefix
/// 0110_8fb9_be8f_4e1e_9ec2_b735 — hard-coded for the replay test.
const CYCLONE_PREFIX: [u8; 12] = [
    0x01, 0x10, 0x8f, 0xb9, 0xbe, 0x8f, 0x4e, 0x1e, 0x9e, 0xc2, 0xb7, 0x35,
];

/// Local capture prefix — this is the DESTINATION prefix (INFO_DST) in
/// the fixture datagram: Cyclone set our local GuidPrefix so that we
/// match it. We build our stack with this prefix so the DATA
/// submessages are addressed to us.
const LOCAL_PREFIX_FROM_FIXTURE: [u8; 12] = [
    0x01, 0x10, 0xa0, 0x3a, 0xe0, 0x39, 0x90, 0xf0, 0xb4, 0xb1, 0x80, 0x10,
];

fn parse_hex(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for chunk in line.split_whitespace() {
            for pair in chunk.as_bytes().chunks(2) {
                let hex = std::str::from_utf8(pair).expect("ascii hex");
                bytes.push(u8::from_str_radix(hex, 16).expect("valid hex"));
            }
        }
    }
    bytes
}

/// Constructs a synthetic `DiscoveredParticipant` for Cyclone with all
/// SEDP builtin-endpoint flags set. In reality this would come from
/// SPDP.
fn cyclone_participant() -> DiscoveredParticipant {
    let flags = endpoint_flag::PUBLICATIONS_ANNOUNCER
        | endpoint_flag::PUBLICATIONS_DETECTOR
        | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
        | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
    DiscoveredParticipant {
        sender_prefix: GuidPrefix::from_bytes(CYCLONE_PREFIX),
        sender_vendor: VendorId([0x01, 0x10]),
        data: ParticipantBuiltinTopicData {
            guid: Guid::new(
                GuidPrefix::from_bytes(CYCLONE_PREFIX),
                EntityId::PARTICIPANT,
            ),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId([0x01, 0x10]),
            default_unicast_locator: Some(Locator::udp_v4([192, 168, 178, 60], 46133)),
            default_multicast_locator: None,
            metatraffic_unicast_locator: None,
            metatraffic_multicast_locator: None,
            domain_id: None,
            builtin_endpoint_set: flags,
            lease_duration: DdsDuration::from_secs(30),
            user_data: Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            participant_security_info: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
        },
    }
}

#[test]
fn cyclone_sedp_publication_fixture_flows_through_stack() {
    // Local stack with the prefix Cyclone targets in the fixture.
    let mut stack = SedpStack::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX_FROM_FIXTURE),
        VendorId::ZERODDS,
    );
    // "Discover" Cyclone as a remote participant — so our
    // SedpPublicationsReader holds a WriterProxy for Cyclone.
    stack.on_participant_discovered(&cyclone_participant());

    // Replay the real Cyclone datagram.
    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let now = Duration::from_secs(1);
    let events = stack.handle_datagram(&bytes, now).expect("handle_datagram");

    // Expected: 4 publications (DDSPerfCPUStats/RPingKS/RDataKS/RPongKS)
    assert_eq!(events.new_publications.len(), 4);
    let topic_names: Vec<_> = events
        .new_publications
        .iter()
        .map(|p| p.topic_name.as_str())
        .collect();
    assert!(topic_names.contains(&"DDSPerfCPUStats"));
    assert!(topic_names.contains(&"DDSPerfRPingKS"));
    assert!(topic_names.contains(&"DDSPerfRDataKS"));
    assert!(topic_names.contains(&"DDSPerfRPongKS"));

    // T10: at least one publication must have a non-empty
    // type_information field (Cyclone sends PID_TYPE_INFORMATION from
    // 0.10.x for every publication type declared as XTypes).
    let with_ti = events
        .new_publications
        .iter()
        .filter(|p| p.type_information.is_some())
        .count();
    assert!(
        with_ti > 0,
        "expected at least one Cyclone publication with PID_TYPE_INFORMATION"
    );

    // Cache must contain all 4
    assert_eq!(stack.cache().publications_len(), 4);
    // All publications belong to the Cyclone prefix
    let cyclone_prefix = GuidPrefix::from_bytes(CYCLONE_PREFIX);
    for pub_data in stack.cache().publications() {
        assert_eq!(pub_data.data.key.prefix, cyclone_prefix);
    }
}

#[test]
fn cyclone_fixture_without_discovery_is_dropped_as_unknown_src() {
    // Without a prior on_participant_discovered, our reader doesn't know
    // the Cyclone writer → submessages are discarded as unknown_src,
    // nothing lands in the cache.
    let mut stack = SedpStack::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX_FROM_FIXTURE),
        VendorId::ZERODDS,
    );
    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let events = stack.handle_datagram(&bytes, Duration::ZERO).unwrap();
    assert_eq!(events.new_publications.len(), 0);
    assert_eq!(stack.cache().publications_len(), 0);
    // The pub_reader must increment the unknown_src counter
    // (4 DATAs with writer_id from the Cyclone prefix, no proxy
    // registered).
    assert!(stack.pub_reader().inner().unknown_src_count() >= 4);
}

#[test]
fn replay_after_participant_lost_stops_accepting_publications() {
    let mut stack = SedpStack::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX_FROM_FIXTURE),
        VendorId::ZERODDS,
    );
    stack.on_participant_discovered(&cyclone_participant());
    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let now = Duration::from_secs(1);

    // First replay: all 4 come through
    let events1 = stack.handle_datagram(&bytes, now).unwrap();
    assert_eq!(events1.new_publications.len(), 4);

    // Participant lost — cache cleared, proxies removed
    let cyclone_prefix = GuidPrefix::from_bytes(CYCLONE_PREFIX);
    let (pubs_removed, _) = stack.on_participant_lost(cyclone_prefix);
    assert_eq!(pubs_removed, 4);
    assert_eq!(stack.cache().publications_len(), 0);

    // Second replay: no proxy left → nothing comes through
    let events2 = stack.handle_datagram(&bytes, now).unwrap();
    assert_eq!(events2.new_publications.len(), 0);
}
