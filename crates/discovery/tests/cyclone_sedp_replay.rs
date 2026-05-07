//! WP 1.4 T6a — Cyclone-Fixture-Replay durch den SedpStack.
//!
//! Wir nehmen die echte Cyclone-0.10.2-SEDP-Publication-Capture aus
//! `crates/rtps/tests/fixtures/cyclone/sedp_publication.hex` und
//! fuettern sie in einen `SedpStack`. Erwartung: der Stack decoded alle
//! 4 Publications und legt sie byte-genau im Cache ab.
//!
//! Dieser Test ist **deterministisch** und CI-fähig — braucht keine
//! Live-Cyclone-Instanz. Der echte Live-Interop-Test steht als
//! `#[ignore]` in `tests/cyclone_live_sedp.rs`.

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

/// Cyclone-Capture wurde mit GuidPrefix 0110_8fb9_be8f_4e1e_9ec2_b735
/// aufgenommen — hart kodiert fuer den Replay-Test.
const CYCLONE_PREFIX: [u8; 12] = [
    0x01, 0x10, 0x8f, 0xb9, 0xbe, 0x8f, 0x4e, 0x1e, 0x9e, 0xc2, 0xb7, 0x35,
];

/// Local-Capture-Prefix — das ist der DESTINATION-Prefix (INFO_DST)
/// im Fixture-Datagramm: Cyclone hat unsere lokale GuidPrefix gesetzt,
/// damit wir sie matchen. Wir bauen unseren Stack mit diesem Prefix,
/// damit die DATA-Submessages an uns adressiert sind.
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

/// Konstruiert eine synthetische `DiscoveredParticipant` fuer Cyclone
/// mit allen SEDP-Builtin-Endpoint-Flags gesetzt. In Realitaet kaeme
/// das aus SPDP.
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
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
        },
    }
}

#[test]
fn cyclone_sedp_publication_fixture_flows_through_stack() {
    // Lokaler Stack mit dem Prefix, auf den Cyclone im Fixture zielt.
    let mut stack = SedpStack::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX_FROM_FIXTURE),
        VendorId::ZERODDS,
    );
    // Cyclone als Remote-Participant "entdecken" — damit unser
    // SedpPublicationsReader einen WriterProxy fuer Cyclone haelt.
    stack.on_participant_discovered(&cyclone_participant());

    // Das echte Cyclone-Datagramm reinspielen.
    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let now = Duration::from_secs(1);
    let events = stack.handle_datagram(&bytes, now).expect("handle_datagram");

    // Erwartet: 4 Publications (DDSPerfCPUStats/RPingKS/RDataKS/RPongKS)
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

    // T10: mindestens eine Publication muss ein nicht-leeres
    // type_information-Feld haben (Cyclone schickt PID_TYPE_INFORMATION
    // ab 0.10.x fuer jeden Publication-Typ, der als XTypes deklariert ist).
    let with_ti = events
        .new_publications
        .iter()
        .filter(|p| p.type_information.is_some())
        .count();
    assert!(
        with_ti > 0,
        "expected at least one Cyclone publication with PID_TYPE_INFORMATION"
    );

    // Cache muss alle 4 enthalten
    assert_eq!(stack.cache().publications_len(), 4);
    // Alle Publications gehoeren zum Cyclone-Prefix
    let cyclone_prefix = GuidPrefix::from_bytes(CYCLONE_PREFIX);
    for pub_data in stack.cache().publications() {
        assert_eq!(pub_data.data.key.prefix, cyclone_prefix);
    }
}

#[test]
fn cyclone_fixture_without_discovery_is_dropped_as_unknown_src() {
    // Ohne vorherige on_participant_discovered kennt unser Reader den
    // Cyclone-Writer nicht → Submessages werden als unknown_src
    // verworfen, nichts landet im Cache.
    let mut stack = SedpStack::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX_FROM_FIXTURE),
        VendorId::ZERODDS,
    );
    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let events = stack.handle_datagram(&bytes, Duration::ZERO).unwrap();
    assert_eq!(events.new_publications.len(), 0);
    assert_eq!(stack.cache().publications_len(), 0);
    // Der pub_reader muss den unknown_src-Zaehler hochsetzen
    // (4 DATAs mit writer_id aus Cyclone-Prefix, kein Proxy registriert).
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

    // Erster Replay: alle 4 kommen durch
    let events1 = stack.handle_datagram(&bytes, now).unwrap();
    assert_eq!(events1.new_publications.len(), 4);

    // Participant lost — Cache cleared, Proxies entfernt
    let cyclone_prefix = GuidPrefix::from_bytes(CYCLONE_PREFIX);
    let (pubs_removed, _) = stack.on_participant_lost(cyclone_prefix);
    assert_eq!(pubs_removed, 4);
    assert_eq!(stack.cache().publications_len(), 0);

    // Zweiter Replay: kein Proxy mehr → nichts kommt durch
    let events2 = stack.handle_datagram(&bytes, now).unwrap();
    assert_eq!(events2.new_publications.len(), 0);
}
