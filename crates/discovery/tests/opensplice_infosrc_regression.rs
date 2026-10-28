//! Regression for the INFO_SRC receiver-state bug (DDSI-RTPS 2.5 §8.3.4 /
//! §8.3.7.9), replaying the real datagrams that exposed it.
//!
//! `crates/rtps/tests/fixtures/opensplice/infosrc_bundling.hex` holds
//! twelve datagrams OpenSplice Community Edition 6.9's DDSI2 service sent
//! to a ZeroDDS publisher's metatraffic sockets (captured on the
//! measurement host, provenance and per-frame annotation in the fixture
//! file itself). Frames 3 and 7 each bundle the submessages of *several*
//! source participants into one datagram and switch the attributed source
//! with INFO_SRC — exactly what `ParsedDatagram::receiver_walk` now
//! tracks per submessage instead of the old code's single
//! `parsed.header.guid_prefix` read once for the whole datagram.
//!
//! What this test checks, replaying the twelve frames through a real
//! `SedpStack` in order:
//!
//! * the application participant's own "Square" DataReader (frame 4) is
//!   discovered and attributed to the application participant's prefix,
//!   not to whichever participant the durability service's samples in
//!   frame 3 were misattributed to under the old bug;
//! * the durability-service participant's nine internal subscriptions
//!   (frame 3, behind INFO_SRC) and a second internal participant's three
//!   subscriptions (frame 7, behind INFO_SRC) are each attributed to
//!   their own INFO_SRC prefix;
//! * no writer's sequence-number state crosses into another writer's proxy
//!   — the durability participant's SN 1..9 never touch the application
//!   participant's own SN 2 "Square" sample, and vice versa;
//! * the ACKNACKs the stack produces afterwards are bounded — the old bug
//!   left a proxy's sequence state stuck below the writer's real firstSN,
//!   which (measured on the same host) produced 33,464 outbound ACKNACK
//!   packets in about 15 seconds against a single peer.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use core::time::Duration;

use zerodds_discovery::sedp::SedpStack;
use zerodds_discovery::spdp::{DiscoveredParticipant, SpdpReader};
use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
use zerodds_rtps::participant_data::{
    Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};

const FIXTURE: &str = include_str!("../../rtps/tests/fixtures/opensplice/infosrc_bundling.hex");

/// Splits the fixture into its twelve datagrams: `#`-comments are
/// stripped, and a blank line ends one datagram's hex block and starts
/// the next.
fn parse_frames(text: &str) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            if !cur.is_empty() {
                frames.push(hex_to_bytes(&cur));
                cur.clear();
            }
            continue;
        }
        cur.push_str(&line.replace(' ', ""));
    }
    if !cur.is_empty() {
        frames.push(hex_to_bytes(&cur));
    }
    frames
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd-length hex string in fixture");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("valid hex"))
        .collect()
}

/// A synthetic `DiscoveredParticipant` for one of OpenSplice's internal
/// (non-application) participants — these never send their own SPDP
/// beacon in the fixture (only the application participant does, frame
/// 2), so the test wires their SEDP builtin endpoints directly, the way a
/// real SPDP announcement from them would.
fn internal_participant(prefix: GuidPrefix) -> DiscoveredParticipant {
    let flags = endpoint_flag::PUBLICATIONS_ANNOUNCER
        | endpoint_flag::PUBLICATIONS_DETECTOR
        | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
        | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
    DiscoveredParticipant {
        sender_prefix: prefix,
        sender_vendor: VendorId([0x01, 0x02]),
        data: ParticipantBuiltinTopicData {
            guid: Guid::new(prefix, EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_1,
            vendor_id: VendorId([0x01, 0x02]),
            default_unicast_locators: vec![Locator::udp_v4([10, 0, 0, 1], 7410)],
            default_multicast_locators: Vec::new(),
            metatraffic_unicast_locators: vec![Locator::udp_v4([10, 0, 0, 1], 7411)],
            metatraffic_multicast_locators: Vec::new(),
            domain_id: None,
            domain_tag: Default::default(),
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
            available_builtin_endpoints_ext: 0,
            participant_security_info: None,
            expects_inline_qos: false,
            manual_liveliness_count: None,
            builtin_endpoint_qos: None,
            entity_name: None,
        },
    }
}

#[test]
fn opensplice_bundled_infosrc_datagrams_attribute_to_the_real_source() {
    let frames = parse_frames(FIXTURE);
    assert_eq!(
        frames.len(),
        12,
        "fixture must hold all twelve captured frames"
    );

    // The application participant's own prefix — read straight off frame
    // 2, the SPDP announcement it sends for itself (not hard-coded, so a
    // change to the fixture cannot silently desync the two).
    let spdp = SpdpReader::new();
    let app = spdp
        .parse_datagram(&frames[1])
        .expect("frame 2 is the application participant's SPDP announcement");
    let app_prefix = app.sender_prefix;
    assert_eq!(
        app_prefix,
        GuidPrefix::from_bytes([
            0x7c, 0x6b, 0x09, 0xd4, 0x00, 0x00, 0x02, 0xd0, 0x00, 0x00, 0x00, 0x01
        ])
    );

    let services_prefix = GuidPrefix::from_bytes([
        0x7c, 0x6b, 0x09, 0xd4, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01,
    ]);
    let durability_prefix = GuidPrefix::from_bytes([
        0x7c, 0x6b, 0x09, 0xd4, 0x00, 0x00, 0x00, 0x7d, 0x00, 0x00, 0x00, 0x01,
    ]);
    // frame 7's header participant (a second application-participant
    // instance) and the internal participant behind its second INFO_SRC.
    let app2_prefix = GuidPrefix::from_bytes([
        0x7c, 0x6b, 0x09, 0xd4, 0x00, 0x00, 0x00, 0x81, 0x00, 0x00, 0x00, 0x01,
    ]);
    let monitor_prefix = GuidPrefix::from_bytes([
        0x7c, 0x6b, 0x09, 0xd4, 0x00, 0x00, 0x00, 0x58, 0x00, 0x00, 0x00, 0x01,
    ]);

    // The local participant is whoever frame 3/7's INFO_DST names — the
    // ZeroDDS publisher these datagrams were sent to.
    let local_prefix = {
        let parsed = decode_datagram(&frames[2]).expect("frame 3 decodes");
        let Some(ParsedSubmessage::InfoDestination(gp)) = parsed.contents().next() else {
            panic!("frame 3 must open with INFO_DST");
        };
        GuidPrefix::from_bytes(*gp)
    };

    let mut stack = SedpStack::new(local_prefix, VendorId::ZERODDS);
    stack.on_participant_discovered(&app);
    stack.on_participant_discovered(&internal_participant(services_prefix));
    stack.on_participant_discovered(&internal_participant(durability_prefix));
    stack.on_participant_discovered(&internal_participant(app2_prefix));
    stack.on_participant_discovered(&internal_participant(monitor_prefix));

    let mut all_new_subscriptions = Vec::new();
    let mut total_acknacks_seen_inbound = 0usize;
    for (i, frame) in frames.iter().enumerate() {
        let now = Duration::from_millis(i as u64 * 100);
        // Frames the stack does not recognize as SEDP (WLP heartbeats,
        // frames 1/8..12) return an empty event set and must not error.
        let events = stack
            .handle_datagram(frame, now)
            .unwrap_or_else(|e| panic!("frame {} failed to decode: {e:?}", i + 1));
        all_new_subscriptions.extend(events.new_subscriptions);
        if let Ok(parsed) = decode_datagram(frame) {
            total_acknacks_seen_inbound += parsed
                .contents()
                .filter(|s| matches!(s, ParsedSubmessage::AckNack(_)))
                .count();
        }
    }
    assert_eq!(
        total_acknacks_seen_inbound, 0,
        "none of the twelve captured frames carry an ACKNACK — sanity check on the fixture"
    );

    // --- the real "Square" subscription reaches the app participant ---
    let square: Vec<_> = all_new_subscriptions
        .iter()
        .filter(|s| s.topic_name == "Square")
        .collect();
    assert_eq!(
        square.len(),
        1,
        "the Square subscription must be discovered exactly once"
    );
    assert_eq!(
        square[0].key.prefix, app_prefix,
        "Square must be attributed to the application participant, not to \
         whatever the durability service's INFO_SRC-bundled samples left behind"
    );

    // --- the durability service's nine internal subscriptions ---
    let durability_topics = [
        "d_capability",
        "d_nameSpaces",
        "d_nameSpacesRequest",
        "d_status",
        "d_groupsRequest",
        "d_deleteData",
        "d_newGroup",
        "d_sampleRequest",
        "d_sampleChain",
    ];
    for topic in durability_topics {
        let found: Vec<_> = all_new_subscriptions
            .iter()
            .filter(|s| s.topic_name == topic)
            .collect();
        assert_eq!(
            found.len(),
            1,
            "durability topic {topic} discovered exactly once"
        );
        assert_eq!(
            found[0].key.prefix, durability_prefix,
            "durability topic {topic} must be attributed to the durability \
             participant's own INFO_SRC prefix"
        );
    }

    // --- frame 7's second bundled datagram: three more topics, a third source ---
    for topic in ["DCPSType", "DCPSDelivery", "DCPSCandMCommand"] {
        let found: Vec<_> = all_new_subscriptions
            .iter()
            .filter(|s| s.topic_name == topic)
            .collect();
        assert_eq!(found.len(), 1, "{topic} discovered exactly once");
        assert_eq!(
            found[0].key.prefix, monitor_prefix,
            "{topic} must be attributed to its own INFO_SRC prefix, not frame 7's header"
        );
    }

    // --- no sequence state crossed between writers sharing the SEDP
    //     SUBSCRIPTIONS EntityId (0x000004c2) ---
    let sub_reader = stack.sub_reader().inner();
    let proxy_of = |prefix: GuidPrefix| {
        sub_reader
            .writer_proxies()
            .iter()
            .find(|s| {
                s.proxy.remote_writer_guid
                    == Guid::new(prefix, EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER)
            })
            .unwrap_or_else(|| panic!("no SEDP_BUILTIN_SUBSCRIPTIONS_WRITER proxy for {prefix:?}"))
    };
    let app_proxy = proxy_of(app_prefix);
    let durability_proxy = proxy_of(durability_prefix);
    let services_proxy = proxy_of(services_prefix);
    assert_eq!(
        durability_proxy.delivered_up_to,
        zerodds_rtps::wire_types::SequenceNumber(9),
        "the durability writer's own nine samples, and only those"
    );
    assert_eq!(
        app_proxy.delivered_up_to,
        zerodds_rtps::wire_types::SequenceNumber(2),
        "the application writer's own SN 2 (Square), untouched by the \
         durability writer's SN 1..9 sharing the same EntityId"
    );
    assert_eq!(
        services_proxy.delivered_up_to,
        zerodds_rtps::wire_types::SequenceNumber(1),
        "the services writer's GAP(1)+HEARTBEAT(1,1) says SN 1 is \
         irrelevant and nothing else exists — delivered_up_to correctly \
         advances to 1 with zero real samples, not to 9 (the durability \
         writer's high-water mark sharing the same EntityId)"
    );

    // --- bounded ACKNACK output ---
    //
    // The measured failure (same host, OMG dds-rtps Test_Reliability_0):
    // 33,464 outbound ACKNACK packets in ~15s against this one peer,
    // stuck at bitmapBase=1 while the writer's HEARTBEAT already said
    // firstAvailableSeqNumber=2. Code review of
    // `zerodds_rtps::reliable_reader::ReliableReader::tick_outbound` finds
    // the mechanism itself already spec-conformant — an ACKNACK is armed
    // only by an incoming HEARTBEAT/fragment-gap
    // (`pending_acknack_since.get_or_insert`) and cleared the moment it is
    // sent, never re-armed by ticking alone — so the storm was a
    // consequence of the misattribution bug: HEARTBEATs and GAPs for
    // three distinct writers landing on one proxy, and a stale, wrongly
    // advanced firstAvailableSeqNumber the real writer's own HEARTBEAT
    // could never again match. With attribution fixed, ticking well past
    // every HEARTBEAT's response delay produces at most one settling
    // ACKNACK per matched writer proxy.
    let outbound = stack
        .tick(Duration::from_secs(10))
        .expect("tick encodes cleanly");
    let acknacks: usize = outbound
        .iter()
        .filter_map(|dg| decode_datagram(&dg.bytes).ok())
        .map(|parsed| {
            parsed
                .contents()
                .filter(|s| matches!(s, ParsedSubmessage::AckNack(_)))
                .count()
        })
        .sum();
    let matched_writer_proxies = stack.sub_reader().inner().writer_proxy_count()
        + stack.pub_reader().inner().writer_proxy_count();
    assert!(
        acknacks <= matched_writer_proxies,
        "expected at most one settling ACKNACK per matched writer proxy \
         ({matched_writer_proxies}), got {acknacks} — that is the shape of the \
         33,464-packet storm, not a bound"
    );
}
