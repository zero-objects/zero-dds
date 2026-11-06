// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-discovery`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-discovery`, from its non-test sources
//! (`runtime.rs`, `participant.rs`):
//! - `spdp`: `SpdpBeacon` (`new`, `set_new_sn_each_period`, `data`,
//!   `serialize`, `serialize_departure`, `serialize_secure`,
//!   `serialize_secure_departure`, `secure_sequence_number`), `SpdpReader`
//!   (`samples_with_channel_in`, `sample_datagram_in`), `SpdpSample::
//!   {Announced, Departed}`, `SpdpChannel::{Plain, Secure}`,
//!   `DiscoveredParticipant`, `DiscoveredParticipantsCache` (`observe`,
//!   `touch`, `drain_expired`, `get`, `remove`, `iter`, `len`).
//! - `sedp`: `SedpStack` (`new`, `on_participant_discovered`,
//!   `announce_publication`, `announce_subscription`, `announce_topic`,
//!   `dispose_publication`, `handle_datagram_in`, `cache`, `topics_of`,
//!   `discovered_topics`, `on_participant_lost`), `SedpEvents`,
//!   `topics_correspond`.
//! - `type_lookup`: `TypeLookupEndpoints` (the four GUIDs,
//!   `service_instance_name`), `TypeLookupClient` (`request_types`,
//!   `request_type_dependencies`, `bind_peer`, `accepts_reply`,
//!   `handle_reply`, `dependency_query`, `pending_count`),
//!   `TypeLookupReply`, `RequestId`, `TypeLookupServer` (`registry`,
//!   `handle_get_types`, `handle_get_type_dependencies`).
//! - `PeerCapabilities::from_bits` (`DcpsRuntime::peer_capabilities`).
//! - `security::SecurityBuiltinStack` without an authentication plugin
//!   (`enable_security_builtins`, `spdp_security_handshake` in a build
//!   without the `security` feature): `handle_remote_stateless_endpoints`,
//!   `handle_remote_volatile_endpoints`, `volatile_matched`, `peer_secret`,
//!   `begin_handshake_with`, `mark_unauthenticated` with
//!   `UnauthenticatedReason::NoIdentityToken`, `poll_stateless`.
//!
//! Oracles: DDSI-RTPS 2.5 §8.5.3.1 (one data-object per SPDP writer, its
//! sequence number changing with the data; the lease), §8.5.4 and §8.5.5.1
//! (SEDP endpoints matched by the builtin endpoint set), §9.3.1.3 Table 9.2
//! (builtin entity ids), §9.3.2 (the builtin endpoint set,
//! `DURATION_INFINITE`), §9.4.5.3 (DATA
//! layout), §9.6.2.2 Table 9.13 (parameter ids), §9.6.4.8 and §9.6.4.9
//! (KeyHash, StatusInfo); DDS-XTypes 1.3 §7.6.3.3.3 (the TypeLookup
//! operation hash ids) and §7.6.3.3.4 (endpoint ids, the service instance
//! name, `getTypes`); DDS-RPC 1.0 §7.4.3 (topic aliases); DDS-Security 1.2
//! §7.4.7.1 (the secure bits of the builtin endpoint set), §7.5.1.6 and
//! §9.3.2.2 (the IdentityStatusToken only on `DCPSParticipantsSecure`),
//! §9.3.2.10 (unauthenticated participants), §9.8.3.2 (stateless endpoints
//! before, volatile ones after authentication). Parameter lists and the
//! departure datagram are written out by hand; the TypeObjects are Cyclone
//! DDS 11.0.1's `Robot` (hash announced by Cyclone) and types read from IDL
//! through `zerodds-wire-probe`. No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use core::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds_discovery::PeerCapabilities;
use zerodds_discovery::security::SecurityBuiltinStack;
use zerodds_discovery::security::stack::{PeerAuthentication, UnauthenticatedReason};
use zerodds_discovery::sedp::{SedpEvents, SedpStack, topics_correspond};
use zerodds_discovery::spdp::{
    DiscoveredParticipant, DiscoveredParticipantsCache, SpdpBeacon, SpdpChannel, SpdpReader,
    SpdpSample,
};
use zerodds_discovery::type_lookup::{
    RequestId, TypeLookupClient, TypeLookupEndpoints, TypeLookupReply, TypeLookupServer,
};
use zerodds_rtps::datagram::ReceiveContext;
use zerodds_rtps::participant_data::ParticipantBuiltinTopicData;
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::topic_data::TopicBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, VendorId};
use zerodds_types::type_lookup::{
    ContinuationPoint, GetTypesReply, GetTypesRequest, OPERATION_GET_DEPENDENCIES,
    OPERATION_GET_TYPES,
};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

// ---------------------------------------------------------------------
// Hand-built discovery data
// ---------------------------------------------------------------------

fn prefix(tag: u8) -> GuidPrefix {
    GuidPrefix::zerodds([tag, 0, 0, tag], [0, 0, 0, 0, 0, tag])
}

/// A PL_CDR_LE payload: `{0x00, 0x03}`, options, each parameter padded to
/// four octets, `PID_SENTINEL` (§9.4.2.11, §10.5).
fn pl_cdr_le(params: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0x00, 0x03, 0x00, 0x00];
    for (pid, value) in params {
        let mut value = value.clone();
        while value.len() % 4 != 0 {
            value.push(0);
        }
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&value);
    }
    out.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    out
}

fn cdr_string(s: &str) -> Vec<u8> {
    let mut out = u32::try_from(s.len() + 1).unwrap().to_le_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    out
}

/// `Locator_t` (§9.3.2): kind, port, sixteen address octets, little-endian.
fn udp_v4(ip: [u8; 4], port: u32) -> Vec<u8> {
    let mut out = 1_i32.to_le_bytes().to_vec();
    out.extend_from_slice(&port.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&ip);
    out
}

/// §9.3.2 builtin endpoint set: SPDP, SEDP publications and
/// subscriptions (bits 0-5), SEDP topics (28, 29).
const SEDP_SET: u32 = 0b11_1111 | (1 << 28) | (1 << 29);

/// Table 9.13: GUID (0x0050), version (0x0015), vendor (0x0016),
/// metatraffic (0x0032) and default (0x0031) unicast locators, endpoint set
/// (0x0058), lease (0x0002) as `Duration_t` seconds and fraction.
fn participant_data(
    p: GuidPrefix,
    endpoints: u32,
    lease: (i32, u32),
    ip: [u8; 4],
) -> ParticipantBuiltinTopicData {
    let mut lease_octets = lease.0.to_le_bytes().to_vec();
    lease_octets.extend_from_slice(&lease.1.to_le_bytes());
    ParticipantBuiltinTopicData::from_pl_cdr_le(&pl_cdr_le(&[
        (
            0x0050,
            Guid::new(p, EntityId::PARTICIPANT).to_bytes().to_vec(),
        ),
        (0x0015, vec![2, 5]),
        (0x0016, vec![0x01, 0xF0]),
        (0x0032, udp_v4(ip, 7410)),
        (0x0031, udp_v4(ip, 7411)),
        (0x0058, endpoints.to_le_bytes().to_vec()),
        (0x0002, lease_octets),
    ]))
    .unwrap()
}

fn discovered(p: GuidPrefix, endpoints: u32, ip: [u8; 4]) -> DiscoveredParticipant {
    DiscoveredParticipant {
        sender_prefix: p,
        sender_vendor: VendorId::ZERODDS,
        data: participant_data(p, endpoints, (100, 0), ip),
    }
}

/// An endpoint: GUID (0x005a), participant (0x0050), topic (0x0005),
/// type (0x0007).
fn endpoint_pl(key: Guid, topic: &str, type_name: &str) -> Vec<u8> {
    pl_cdr_le(&[
        (0x005a, key.to_bytes().to_vec()),
        (
            0x0050,
            Guid::new(key.prefix, EntityId::PARTICIPANT)
                .to_bytes()
                .to_vec(),
        ),
        (0x0005, cdr_string(topic)),
        (0x0007, cdr_string(type_name)),
    ])
}

fn spdp(datagram: &[u8]) -> Vec<(SpdpChannel, SpdpSample)> {
    SpdpReader::new()
        .samples_with_channel_in(datagram, &ReceiveContext::default())
        .unwrap()
}

// ---------------------------------------------------------------------
// SPDP
// ---------------------------------------------------------------------

#[test]
fn rtps_8_5_3_1_the_beacon_is_one_data_object_from_the_spdp_writer() {
    let p = prefix(1);
    let data = participant_data(p, SEDP_SET, (100, 0), [10, 0, 0, 1]);
    let mut beacon = SpdpBeacon::new(data.clone());
    let first = beacon.serialize().unwrap();
    // Header: RTPS 2.5, vendor 0x01F0, the participant's prefix.
    assert_eq!(first[..8], *b"RTPS\x02\x05\x01\xF0");
    assert_eq!(first[8..20], p.to_bytes());
    // DATA (0x15), E|D; reader 000100c7, writer 000100c2 (Table 9.2);
    // writerSN 1; the payload is PL_CDR_LE.
    assert_eq!(first[20..22], [0x15, 0x05]);
    assert_eq!(first[28..32], [0x00, 0x01, 0x00, 0xc7]);
    assert_eq!(first[32..36], [0x00, 0x01, 0x00, 0xc2]);
    assert_eq!(first[36..44], [0, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(first[44..48], [0x00, 0x03, 0x00, 0x00]);
    // The same data sent again is the same change.
    assert_eq!(beacon.serialize().unwrap(), first);
    // A change of the participant data is the next change.
    beacon.data.user_data = vec![1, 2, 3];
    assert_eq!(
        beacon.serialize().unwrap()[36..44],
        [0, 0, 0, 0, 2, 0, 0, 0]
    );
    // `spdp.new_sequence_per_period`: every announcement a new number.
    beacon.set_new_sn_each_period(true);
    assert_eq!(
        beacon.serialize().unwrap()[36..44],
        [0, 0, 0, 0, 3, 0, 0, 0]
    );

    // The reader gives the participant back, from the plain writer.
    let samples = spdp(&first);
    assert_eq!(samples.len(), 1);
    match &samples[0] {
        (SpdpChannel::Plain, SpdpSample::Announced(dp)) => {
            assert_eq!(dp.sender_prefix, p);
            assert_eq!(dp.sender_vendor, VendorId::ZERODDS);
            assert_eq!(dp.data, data);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn security_9_3_2_2_the_identity_status_token_goes_on_the_secure_beacon_only() {
    let p = prefix(2);
    let mut data = participant_data(p, SEDP_SET, (100, 0), [10, 0, 0, 2]);
    data.identity_status_token = Some(vec![0xAB; 8]);
    let mut beacon = SpdpBeacon::new(data);

    let plain = beacon.serialize().unwrap();
    match &spdp(&plain)[0] {
        (SpdpChannel::Plain, SpdpSample::Announced(dp)) => {
            assert_eq!(dp.data.identity_status_token, None);
        }
        other => panic!("{other:?}"),
    }

    // DCPSParticipantsSecure: writer ff0101c2, reader ff0101c7, its own
    // sequence numbers.
    let secure = beacon.serialize_secure().unwrap();
    assert_eq!(secure[28..32], [0xff, 0x01, 0x01, 0xc7]);
    assert_eq!(secure[32..36], [0xff, 0x01, 0x01, 0xc2]);
    assert_eq!(secure[36..44], [0, 0, 0, 0, 1, 0, 0, 0]);
    match &spdp(&secure)[0] {
        (SpdpChannel::Secure, SpdpSample::Announced(dp)) => {
            assert_eq!(dp.data.identity_status_token, Some(vec![0xAB; 8]));
        }
        other => panic!("{other:?}"),
    }
    // A reliable writer: the same data keeps its number, other data
    // takes the next.
    assert_eq!(beacon.secure_sequence_number().unwrap(), 1);
    beacon.data.identity_status_token = Some(vec![0xCD; 8]);
    assert_eq!(beacon.secure_sequence_number().unwrap(), 2);
    assert_eq!(beacon.secure_sequence_number().unwrap(), 2);
}

#[test]
fn rtps_9_6_4_9_a_departure_is_a_key_only_data_disposed_and_unregistered() {
    let p = prefix(3);
    let guid = Guid::new(p, EntityId::PARTICIPANT).to_bytes();
    let mut beacon = SpdpBeacon::new(participant_data(p, SEDP_SET, (100, 0), [10, 0, 0, 3]));
    let _ = beacon.serialize().unwrap(); // SN 1
    let departure = beacon.serialize_departure().unwrap();

    let mut expected = b"RTPS\x02\x05\x01\xF0".to_vec();
    expected.extend_from_slice(&p.to_bytes());
    // DATA, E|Q|K, 80 octets: 20 fixed, 32 inline QoS, 28 payload.
    expected.extend_from_slice(&[0x15, 0x0B, 80, 0]);
    expected.extend_from_slice(&[0, 0, 16, 0]);
    expected.extend_from_slice(&[0x00, 0x01, 0x00, 0xc7, 0x00, 0x01, 0x00, 0xc2]);
    expected.extend_from_slice(&[0, 0, 0, 0, 2, 0, 0, 0]);
    // PID_KEY_HASH (0x0070): the participant GUID.
    expected.extend_from_slice(&[0x70, 0x00, 16, 0]);
    expected.extend_from_slice(&guid);
    // PID_STATUS_INFO (0x0071), big-endian: DISPOSED | UNREGISTERED.
    expected.extend_from_slice(&[0x71, 0x00, 4, 0, 0, 0, 0, 0x03]);
    expected.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    // serializedKey: PL_CDR_LE with PID_PARTICIPANT_GUID.
    expected.extend_from_slice(&[0x00, 0x03, 0x00, 0x00, 0x50, 0x00, 16, 0]);
    expected.extend_from_slice(&guid);
    expected.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    assert_eq!(departure, expected);

    assert!(matches!(
        spdp(&departure)[..],
        [(SpdpChannel::Plain, SpdpSample::Departed(q))] if q == p
    ));
    let secure = beacon.serialize_secure_departure().unwrap();
    assert!(matches!(
        spdp(&secure)[..],
        [(SpdpChannel::Secure, SpdpSample::Departed(q))] if q == p
    ));
}

#[test]
fn a_datagram_without_spdp_is_refused_and_one_announcement_can_be_cut_out() {
    // A header and a PAD only.
    let mut not_spdp = b"RTPS\x02\x05\x01\xF0".to_vec();
    not_spdp.extend_from_slice(&prefix(4).to_bytes());
    not_spdp.extend_from_slice(&[0x01, 0x01, 0x00, 0x00]);
    assert!(
        SpdpReader::new()
            .samples_with_channel_in(&not_spdp, &ReceiveContext::default())
            .is_err()
    );

    // The discovery-server relay forwards one participant's announcement
    // as a datagram of its own.
    let p = prefix(5);
    let mut beacon = SpdpBeacon::new(participant_data(p, SEDP_SET, (100, 0), [10, 0, 0, 5]));
    let datagram = beacon.serialize().unwrap();
    let ctx = ReceiveContext::default();
    let relayed = SpdpReader::new()
        .sample_datagram_in(&datagram, p, &ctx)
        .unwrap();
    assert_eq!(relayed, datagram);
    assert!(
        SpdpReader::new()
            .sample_datagram_in(&datagram, prefix(6), &ctx)
            .is_err()
    );
}

#[test]
fn rtps_8_5_3_1_a_participant_expires_when_its_lease_runs_out() {
    let mut cache = DiscoveredParticipantsCache::new();
    let short = DiscoveredParticipant {
        sender_prefix: prefix(7),
        sender_vendor: VendorId::ZERODDS,
        data: participant_data(prefix(7), SEDP_SET, (10, 0), [10, 0, 0, 7]),
    };
    // §9.3.2: DURATION_INFINITE = {0x7fffffff, 0xffffffff}.
    let forever = DiscoveredParticipant {
        sender_prefix: prefix(8),
        sender_vendor: VendorId::ZERODDS,
        data: participant_data(
            prefix(8),
            SEDP_SET,
            (0x7fff_ffff, 0xffff_ffff),
            [10, 0, 0, 8],
        ),
    };
    let t = |s| Duration::from_secs(s);
    assert!(cache.observe(short.clone(), t(1)), "new");
    assert!(!cache.observe(short.clone(), t(2)), "known");
    assert!(cache.observe(forever.clone(), t(1)));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&prefix(7)), Some(&short));
    // Renewed at 5: alive until 15.
    assert!(cache.touch(&prefix(7), t(5)));
    assert!(!cache.touch(&prefix(9), t(5)), "unknown participant");
    assert!(cache.drain_expired(t(14)).is_empty());
    let expired = cache.drain_expired(t(15));
    assert_eq!(expired, vec![short]);
    assert!(cache.get(&prefix(7)).is_none());
    // The infinite lease never runs out.
    assert!(cache.drain_expired(t(u64::from(u32::MAX))).is_empty());
    assert_eq!(cache.iter().count(), 1);
    assert_eq!(cache.remove(&prefix(8)), Some(forever));
    assert_eq!(cache.len(), 0);
}

#[test]
fn rtps_9_3_2_and_security_7_4_7_1_peer_capabilities_from_the_endpoint_set() {
    let bit = |n: u32| 1_u32 << n;
    let caps = PeerCapabilities::from_bits(
        bit(0) | bit(1) | bit(2) | bit(3) | bit(4) | bit(5) | bit(10) | bit(11),
    );
    assert!(caps.has_spdp && caps.has_sedp_publications && caps.has_sedp_subscriptions);
    assert!(caps.has_wlp);
    assert!(!caps.has_type_lookup && !caps.has_topics_discovery);
    assert!(!caps.supports_security());
    // An announcer without its detector is not the pair.
    assert!(!PeerCapabilities::from_bits(bit(2)).has_sedp_publications);
    // TypeLookup: request writer/reader and reply writer/reader, 12-15.
    assert!(PeerCapabilities::from_bits(bit(12) | bit(13) | bit(14) | bit(15)).has_type_lookup);
    assert!(!PeerCapabilities::from_bits(bit(12) | bit(13) | bit(14)).has_type_lookup);
    assert!(PeerCapabilities::from_bits(bit(28) | bit(29)).has_topics_discovery);
    // DDS-Security 1.2: secure publications 16/17, subscriptions 18/19,
    // participant message 20/21, stateless 22/23, volatile 24/25,
    // secure participant 26/27.
    let secure = PeerCapabilities::from_bits((16..=27).map(bit).fold(0, |a, b| a | b));
    assert!(secure.supports_security());
    assert!(
        secure.has_secure_publications
            && secure.has_secure_subscriptions
            && secure.has_secure_wlp
            && secure.has_stateless_auth
            && secure.has_volatile_secure
            && secure.has_secure_participant
    );
}

// ---------------------------------------------------------------------
// SEDP
// ---------------------------------------------------------------------

/// Hands every datagram to `to` and gathers the events.
fn deliver(
    datagrams: &[zerodds_rtps::message_builder::OutboundDatagram],
    to: &mut SedpStack,
    target: Locator,
) -> Vec<SedpEvents> {
    assert!(!datagrams.is_empty());
    datagrams
        .iter()
        .map(|dg| {
            assert!(dg.targets.contains(&target), "{:?}", dg.targets);
            to.handle_datagram_in(
                &dg.bytes,
                &ReceiveContext::default(),
                Duration::from_secs(1),
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn rtps_8_5_4_endpoints_announced_on_one_participant_are_discovered_on_the_other() {
    let (pa, pb) = (prefix(10), prefix(11));
    let (ip_a, ip_b) = ([10, 0, 0, 10], [10, 0, 0, 11]);
    // The metatraffic locator of each, where SEDP goes.
    let (meta_a, meta_b) = (Locator::udp_v4(ip_a, 7410), Locator::udp_v4(ip_b, 7410));
    let mut a = SedpStack::new(pa, VendorId::ZERODDS);
    let mut b = SedpStack::new(pb, VendorId::ZERODDS);
    a.on_participant_discovered(&discovered(pb, SEDP_SET, ip_b));
    b.on_participant_discovered(&discovered(pa, SEDP_SET, ip_a));

    let writer = Guid::new(pa, EntityId::user_writer_with_key([0, 0, 1]));
    let publication =
        PublicationBuiltinTopicData::from_pl_cdr_le(&endpoint_pl(writer, "Square", "ShapeType"))
            .unwrap();
    let events = deliver(
        &a.announce_publication(&publication).unwrap(),
        &mut b,
        meta_b,
    );
    let found: Vec<_> = events.iter().flat_map(|e| &e.new_publications).collect();
    assert_eq!(found.len(), 1);
    assert_eq!(
        (
            found[0].key,
            found[0].topic_name.as_str(),
            found[0].type_name.as_str()
        ),
        (writer, "Square", "ShapeType")
    );
    assert!(b.cache().publication(writer).is_some());

    let reader = Guid::new(pb, EntityId::user_reader_with_key([0, 0, 2]));
    let subscription =
        SubscriptionBuiltinTopicData::from_pl_cdr_le(&endpoint_pl(reader, "Square", "ShapeType"))
            .unwrap();
    let events = deliver(
        &b.announce_subscription(&subscription).unwrap(),
        &mut a,
        meta_a,
    );
    let found: Vec<_> = events.iter().flat_map(|e| &e.new_subscriptions).collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].key, reader);
    assert!(a.cache().subscription(reader).is_some());

    // DCPSTopic (§8.5.4.2).
    let topic = TopicBuiltinTopicData::new(
        Guid::new(pa, EntityId::topic([0, 0, 3])),
        "Square",
        "ShapeType",
    );
    let events = deliver(&a.announce_topic(&topic).unwrap(), &mut b, meta_b);
    assert_eq!(events.iter().map(|e| e.new_topics.len()).sum::<usize>(), 1);
    assert_eq!(b.topics_of(pa).len(), 1);
    assert_eq!(b.discovered_topics().count(), 1);

    // The publication deleted: disposed on the other side.
    let events = deliver(&a.dispose_publication(writer).unwrap(), &mut b, meta_b);
    let removed: Vec<_> = events
        .iter()
        .flat_map(|e| &e.removed_publications)
        .collect();
    assert_eq!(removed, vec![&writer]);
    assert!(b.cache().publication(writer).is_none());

    // A participant lost (§8.5.5.2): what it announced leaves the cache
    // and its topics go; the count is that of the cache entries, one
    // publication still announced and no subscription.
    let second = Guid::new(pa, EntityId::user_writer_with_key([0, 0, 4]));
    let publication =
        PublicationBuiltinTopicData::from_pl_cdr_le(&endpoint_pl(second, "Circle", "ShapeType"))
            .unwrap();
    deliver(
        &a.announce_publication(&publication).unwrap(),
        &mut b,
        meta_b,
    );
    assert!(b.cache().publication(second).is_some());
    assert_eq!(b.on_participant_lost(pa), (1, 0));
    assert!(b.cache().publication(second).is_none());
    assert!(b.topics_of(pa).is_empty());
}

#[test]
fn rtps_8_5_5_1_a_data_from_an_sedp_writer_not_yet_matched_is_reported() {
    // B has not seen A's SPDP: A's announcement names a writer B has no
    // proxy for, which the runtime answers by announcing itself again.
    let (pa, pb) = (prefix(12), prefix(13));
    let mut a = SedpStack::new(pa, VendorId::ZERODDS);
    let mut b = SedpStack::new(pb, VendorId::ZERODDS);
    a.on_participant_discovered(&discovered(pb, SEDP_SET, [10, 0, 0, 13]));
    let writer = Guid::new(pa, EntityId::user_writer_with_key([0, 0, 1]));
    let publication =
        PublicationBuiltinTopicData::from_pl_cdr_le(&endpoint_pl(writer, "T", "U")).unwrap();
    let events = deliver(
        &a.announce_publication(&publication).unwrap(),
        &mut b,
        Locator::udp_v4([10, 0, 0, 13], 7410),
    );
    assert!(events.iter().all(|e| e.new_publications.is_empty()));
    assert!(events.iter().any(|e| {
        e.unmatched_writers
            .contains(&Guid::new(pa, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER))
    }));
}

#[test]
fn rpc_7_4_3_topic_aliases_name_the_same_topic() {
    let aliases = vec![String::from("RobotService_Request")];
    assert!(topics_correspond("Square", &[], "Square", None));
    assert!(!topics_correspond("Square", &[], "Circle", None));
    assert!(topics_correspond(
        "RobotService_Request",
        &[],
        "Robot_Request",
        Some(aliases.as_slice())
    ));
    assert!(topics_correspond(
        "Robot_Request",
        &aliases,
        "RobotService_Request",
        None
    ));
    assert!(!topics_correspond(
        "Other",
        &[],
        "Robot_Request",
        Some(aliases.as_slice())
    ));
}

// ---------------------------------------------------------------------
// TypeLookup
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_3_4_type_lookup_endpoints_and_service_name() {
    let p = GuidPrefix([0x01, 0x0f, 0xab, 0xcd, 0, 0, 0, 1, 0, 0, 0, 0x2a]);
    let endpoints = TypeLookupEndpoints::new(p);
    for (guid, id) in [
        (endpoints.request_writer(), [0x00, 0x03, 0x00, 0xc3]),
        (endpoints.request_reader(), [0x00, 0x03, 0x00, 0xc4]),
        (endpoints.reply_writer(), [0x00, 0x03, 0x01, 0xc3]),
        (endpoints.reply_reader(), [0x00, 0x03, 0x01, 0xc4]),
    ] {
        assert_eq!(guid.prefix, p);
        assert_eq!(guid.entity_id.to_bytes(), id);
    }
    // "dds.builtin.TOS." and the participant GUID in hexadecimal.
    assert_eq!(
        endpoints.service_instance_name(),
        "dds.builtin.TOS.010fabcd000000010000002a000001c1"
    );
}

#[test]
fn xtypes_7_6_3_3_3_a_reply_reaches_the_callback_of_its_own_request_only() {
    assert_eq!(OPERATION_GET_TYPES, 0x0182_52d3);
    assert_eq!(OPERATION_GET_DEPENDENCIES, 0x05aa_fb31);
    let hits = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&hits);
    let mut client = TypeLookupClient::new();
    let hash = TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([7; 14]));
    let id: RequestId = client.request_types(
        vec![hash.clone()],
        Box::new(move |reply: TypeLookupReply| {
            assert!(matches!(reply, TypeLookupReply::Types(_)));
            seen.fetch_add(1, Ordering::SeqCst);
        }),
    );
    assert_eq!(client.pending_count(), 1);
    let peer = prefix(20).to_bytes();
    client.bind_peer(id, peer);
    // Another peer, or a getTypeDependencies answer, is not this reply.
    assert!(!client.accepts_reply(id, prefix(21).to_bytes(), OPERATION_GET_TYPES));
    assert!(!client.accepts_reply(id, peer, OPERATION_GET_DEPENDENCIES));
    assert!(client.accepts_reply(id, peer, OPERATION_GET_TYPES));
    assert!(client.handle_reply(id, TypeLookupReply::Types(GetTypesReply::default())));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // Answered once: a repetition finds nothing pending.
    assert!(!client.handle_reply(id, TypeLookupReply::Types(GetTypesReply::default())));
    assert_eq!(client.pending_count(), 0);

    // A dependencies request keeps its query for the next page.
    let cursor = ContinuationPoint(vec![1, 2, 3]);
    let deps = client.request_type_dependencies(
        vec![hash.clone()],
        cursor.clone(),
        Box::new(|_: TypeLookupReply| {}),
    );
    assert_ne!(deps, id);
    assert_eq!(client.dependency_query(deps), Some((vec![hash], cursor)));
    assert!(!client.handle_reply(deps, TypeLookupReply::Types(GetTypesReply::default())));
}

#[test]
fn xtypes_7_6_3_3_4_get_types_answers_what_the_registry_holds() {
    use zerodds_types::framing::xcdr2_reader;
    use zerodds_types::type_object::TypeObject;
    // `@final struct Robot { uint32 id; uint32 label; }`, the MINIMAL
    // TypeObject written out from Annex B (crates/discovery/tests/
    // contract_types.rs); Cyclone DDS 11.0.1 announces its hash.
    #[rustfmt::skip]
    let robot: [u8; 55] = [
        0x33, 0x00, 0x00, 0x00, 0xf1, 0x51, 0x01, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x23, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
        0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07,
        0xb8, 0x0b, 0xb7, 0x74, 0x00,
        0x0b, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07,
        0xd3, 0x04, 0xba, 0x20,
    ];
    let cyclone = EquivalenceHash([
        0x62, 0x54, 0x36, 0xec, 0xee, 0x22, 0xcc, 0x29, 0x43, 0xeb, 0xc5, 0x98, 0xc4, 0x3e,
    ]);
    let object = TypeObject::decode_from(&mut xcdr2_reader(&robot)).unwrap();
    let mut server = TypeLookupServer::new();
    assert_eq!(
        server.registry.insert_object(object.clone()).unwrap(),
        cyclone
    );

    let asked = TypeIdentifier::EquivalenceHashMinimal(cyclone);
    let reply = server.handle_get_types(&GetTypesRequest {
        type_ids: vec![asked.clone()],
    });
    assert_eq!(reply.types.len(), 1);
    assert_eq!(reply.types[0].type_identifier, asked);
    assert_eq!(reply.types[0].type_object, object);
    // A type the registry does not hold is left out, not invented.
    let unknown = server.handle_get_types(&GetTypesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            [0; 14],
        ))],
    });
    assert!(unknown.types.is_empty());
}

#[test]
fn xtypes_7_6_3_3_4_get_type_dependencies_names_a_complete_roots_dependencies_complete() {
    use zerodds_types::type_lookup::GetTypeDependenciesRequest;
    use zerodds_types::type_object::TypeObject;
    let ast = zerodds_wire_probe::encode::model_from_idl(
        "@final struct Point { long x; long y; }; @final struct Path { Point head; Point tail; };",
        "contract_discovery.idl",
    )
    .expect("the model");
    let complete = |name: &str| {
        let id = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![name.into()]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect(name);
        TypeObject::Complete(zerodds_types::from_ast::complete_type_object(&ast, id).unwrap())
    };
    let mut server = TypeLookupServer::new();
    let point = server.registry.insert_object(complete("Point")).unwrap();
    let path = server.registry.insert_object(complete("Path")).unwrap();

    let reply = server.handle_get_type_dependencies(&GetTypeDependenciesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashComplete(path)],
        continuation_point: ContinuationPoint::default(),
    });
    // Point once, though Path names it twice, in the form it was asked in;
    // one page, so no continuation.
    let named: Vec<_> = reply
        .dependent_typeids
        .iter()
        .map(|d| d.type_id.clone())
        .collect();
    assert_eq!(named, vec![TypeIdentifier::EquivalenceHashComplete(point)]);
    assert_eq!(reply.continuation_point, ContinuationPoint::default());
    // A type without dependencies has none to list.
    let leaf = server.handle_get_type_dependencies(&GetTypeDependenciesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashComplete(point)],
        continuation_point: ContinuationPoint::default(),
    });
    assert!(leaf.dependent_typeids.is_empty());
}

// ---------------------------------------------------------------------
// The security builtin stack without an authentication plugin
// ---------------------------------------------------------------------

#[test]
fn security_9_8_3_2_stateless_endpoints_first_and_no_handshake_without_a_plugin() {
    let local = prefix(30);
    let peer = prefix(31);
    let mut stack = SecurityBuiltinStack::new(local, VendorId::ZERODDS);
    let stateless_and_volatile = (1 << 22) | (1 << 23) | (1 << 24) | (1 << 25);
    let dp = discovered(peer, SEDP_SET | stateless_and_volatile, [10, 0, 0, 31]);

    stack.handle_remote_stateless_endpoints(&dp);
    assert!(
        !stack.volatile_matched(peer),
        "volatile only once authenticated"
    );
    assert!(stack.peer_secret(peer).is_none());
    // Without a plugin nothing is sent for the peer's IdentityToken.
    let sent = stack
        .begin_handshake_with(peer, dp.data.guid.to_bytes(), &[1, 2, 3])
        .unwrap();
    assert!(sent.is_empty());
    assert!(
        stack
            .poll_stateless(Duration::from_secs(1))
            .unwrap()
            .is_empty()
    );
    // §9.3.2.10: a peer without an IdentityToken is unauthenticated.
    stack.mark_unauthenticated(peer, UnauthenticatedReason::NoIdentityToken);
    assert!(!stack.is_authenticated(peer));
    assert_eq!(
        stack.authentication_status(peer),
        PeerAuthentication::Unauthenticated(UnauthenticatedReason::NoIdentityToken)
    );
    // The volatile endpoints, when the runtime matches them.
    stack.handle_remote_volatile_endpoints(&dp);
    assert!(stack.volatile_matched(peer));
}
