// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The SEDP topic announcement (DDSI-RTPS 2.5 §8.5.4.2) read from what
//! Cyclone DDS sends.
//!
//! [`CYCLONE`] is the `serializedPayload` of the DATA Cyclone DDS 11.0.1,
//! built with topic discovery and run with
//! `Discovery/EnableTopicDiscoveryEndpoints`, sent from
//! `ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER` (`0x000002c2`) to
//! `ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR` (`0x000002c7`), captured on
//! the Linux test host. The topic was created from Python with
//! `TransientLocal`, `KeepLast(7)`, resource limits 100/10/10, deadline
//! 2 s, latency budget 5 ms, transport priority 3, lifespan 30 s,
//! `Reliable(100 ms)`, `Exclusive` and topic data `abc`.

use zerodds_qos::{DurabilityKind, Duration, HistoryKind, OwnershipKind, ReliabilityKind};
use zerodds_rtps::parameter_list::{ParameterList, pid};
use zerodds_rtps::topic_data::TopicBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};

const CYCLONE: [u8; 316] = [
    0x00, 0x03, 0x00, 0x00, 0x05, 0x00, 0x10, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x54, 0x6f, 0x70, 0x69,
    0x63, 0x50, 0x72, 0x6f, 0x62, 0x65, 0x00, 0x00, 0x07, 0x00, 0x10, 0x00, 0x0a, 0x00, 0x00, 0x00,
    0x74, 0x70, 0x3a, 0x3a, 0x50, 0x72, 0x6f, 0x62, 0x65, 0x00, 0x00, 0x00, 0x2e, 0x00, 0x08, 0x00,
    0x03, 0x00, 0x00, 0x00, 0x61, 0x62, 0x63, 0x00, 0x1d, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x23, 0x00, 0x08, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x27, 0x00, 0x08, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x15, 0xae, 0x47, 0x01, 0x1a, 0x00, 0x0c, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x9a, 0x99, 0x99, 0x19, 0x2b, 0x00, 0x08, 0x00, 0x1e, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00,
    0x41, 0x00, 0x0c, 0x00, 0x64, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00,
    0x1f, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x49, 0x00, 0x04, 0x00, 0x03, 0x00, 0x00, 0x00,
    0x73, 0x00, 0x08, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x75, 0x00, 0x64, 0x00,
    0x60, 0x00, 0x00, 0x00, 0x01, 0x10, 0x00, 0x40, 0x28, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0xf1, 0x6c, 0xf4, 0x60, 0x91, 0x9c, 0xfd, 0x86, 0xa6, 0x7f, 0xf3, 0x79,
    0x73, 0x6c, 0x6a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x02, 0x10, 0x00, 0x40, 0x28, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0xf2, 0x94, 0x6d, 0x3e, 0xf1, 0xe4, 0x1e, 0x1c, 0xbd, 0x5b, 0x4d, 0x1c,
    0x51, 0xb3, 0x5c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x15, 0x00, 0x04, 0x00, 0x02, 0x05, 0x00, 0x00, 0x16, 0x00, 0x04, 0x00,
    0x01, 0x10, 0x00, 0x00, 0x1b, 0x80, 0x10, 0x00, 0x01, 0x10, 0xd5, 0xff, 0x26, 0x58, 0x3a, 0xe1,
    0xaf, 0x16, 0x5a, 0xb9, 0x00, 0x00, 0x01, 0x4d, 0x01, 0x00, 0x00, 0x00,
];

#[test]
fn cyclones_announcement_reads_as_the_topic_it_created() {
    let topic = TopicBuiltinTopicData::from_pl_cdr(&CYCLONE, VendorId::CYCLONE).unwrap();
    // PID_CYCLONE_TOPIC_GUID: the participant prefix and Cyclone's topic
    // entity id 0x0000014d.
    assert_eq!(
        topic.key.to_bytes(),
        [
            0x01, 0x10, 0xd5, 0xff, 0x26, 0x58, 0x3a, 0xe1, 0xaf, 0x16, 0x5a, 0xb9, 0x00, 0x00,
            0x01, 0x4d
        ]
    );
    assert_eq!(topic.topic_name, "TopicProbe");
    assert_eq!(topic.type_name, "tp::Probe");
    assert_eq!(topic.durability.kind, DurabilityKind::TransientLocal);
    assert_eq!(topic.history.kind, HistoryKind::KeepLast);
    assert_eq!(topic.history.depth, 7);
    assert_eq!(topic.resource_limits.max_samples, 100);
    assert_eq!(topic.resource_limits.max_instances, 10);
    assert_eq!(topic.resource_limits.max_samples_per_instance, 10);
    assert_eq!(topic.deadline.period, Duration::from_secs(2));
    assert_eq!(
        topic.latency_budget.duration,
        Duration {
            seconds: 0,
            fraction: 0x0147_ae15
        }
    );
    assert_eq!(topic.transport_priority.value, 3);
    assert_eq!(topic.lifespan.duration, Duration::from_secs(30));
    assert_eq!(topic.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(
        topic.reliability.max_blocking_time,
        Duration {
            seconds: 0,
            fraction: 0x1999_999a
        }
    );
    assert_eq!(topic.ownership.kind, OwnershipKind::Exclusive);
    assert_eq!(topic.topic_data.value, b"abc");
    assert_eq!(topic.data_representation, [0, 2]);
    assert_eq!(
        topic.type_information.as_deref().map(<[u8]>::len),
        Some(100)
    );
    assert_eq!(topic.vendor_id, Some(VendorId([0x01, 0x10])));
}

#[test]
fn the_policies_cyclone_leaves_out_read_as_their_defaults() {
    let topic = TopicBuiltinTopicData::from_pl_cdr(&CYCLONE, VendorId::CYCLONE).unwrap();
    let defaults = TopicBuiltinTopicData::new(topic.key, "TopicProbe", "tp::Probe");
    assert_eq!(topic.durability_service, defaults.durability_service);
    assert_eq!(topic.liveliness, defaults.liveliness);
    assert_eq!(topic.destination_order, defaults.destination_order);
}

#[test]
fn what_we_write_reads_back_as_the_same_topic() {
    let mut topic = TopicBuiltinTopicData::from_pl_cdr(&CYCLONE, VendorId::CYCLONE).unwrap();
    topic.vendor_id = Some(VendorId::ZERODDS);
    let bytes = topic.to_pl_cdr_le().unwrap();
    assert_eq!(
        TopicBuiltinTopicData::from_pl_cdr(&bytes, VendorId::ZERODDS).unwrap(),
        topic
    );
}

#[test]
fn cyclones_key_parameter_is_read_only_from_cyclone() {
    // 0x801b is vendor-specific: from another vendor it means something
    // else or nothing, and the announcement has no key.
    assert!(TopicBuiltinTopicData::from_pl_cdr(&CYCLONE, VendorId::FASTDDS).is_err());
    assert!(TopicBuiltinTopicData::from_pl_cdr(&CYCLONE, VendorId::ZERODDS).is_err());
}

/// The policy parameters of a topic announcement.
const POLICIES: [u16; 13] = [
    pid::TOPIC_DATA,
    pid::DURABILITY,
    pid::DURABILITY_SERVICE,
    pid::DEADLINE,
    pid::LATENCY_BUDGET,
    pid::LIVELINESS,
    pid::RELIABILITY,
    pid::TRANSPORT_PRIORITY,
    pid::LIFESPAN,
    pid::DESTINATION_ORDER,
    pid::HISTORY,
    pid::RESOURCE_LIMITS,
    pid::OWNERSHIP,
];

#[test]
fn a_topic_at_every_default_sends_its_key_names_and_vendor_only() {
    let key = Guid::new(GuidPrefix::from_bytes([7; 12]), EntityId::topic([0, 0, 1]));
    let bytes = TopicBuiltinTopicData::new(key, "T", "M::T")
        .to_pl_cdr_le()
        .unwrap();
    assert_eq!(&bytes[..4], [0x00, 0x03, 0x00, 0x00]);
    let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
    assert_eq!(
        pl.find(pid::ENDPOINT_GUID).map(|p| p.value.clone()),
        Some(key.to_bytes().to_vec())
    );
    for id in [pid::TOPIC_NAME, pid::TYPE_NAME, pid::VENDOR_ID] {
        assert!(pl.find(id).is_some(), "parameter {id:#06x} missing");
    }
    // Dust DDS 0.15.0 stops on any PID_HISTORY; a default topic sends
    // none of the policies.
    for id in POLICIES {
        assert!(pl.find(id).is_none(), "default parameter {id:#06x} sent");
    }
}

#[test]
fn a_policy_off_its_default_is_sent_and_reads_back() {
    // Cyclone's topic: TransientLocal, KeepLast(7), Reliable, Exclusive
    // and the rest it names off the default. What it leaves out stays
    // out, what it sends is sent again.
    let mut topic = TopicBuiltinTopicData::from_pl_cdr(&CYCLONE, VendorId::CYCLONE).unwrap();
    topic.vendor_id = Some(VendorId::ZERODDS);
    let bytes = topic.to_pl_cdr_le().unwrap();
    let ours = ParameterList::from_bytes(&bytes[4..], true).unwrap();
    let cyclones = ParameterList::from_bytes(&CYCLONE[4..], true).unwrap();
    for id in POLICIES {
        assert_eq!(
            ours.find(id).is_some(),
            cyclones.find(id).is_some(),
            "parameter {id:#06x}"
        );
    }
    assert_eq!(
        TopicBuiltinTopicData::from_pl_cdr(&bytes, VendorId::ZERODDS).unwrap(),
        topic
    );
}

/// The offset of a parameter's header in [`CYCLONE`].
fn header_of(id: u16) -> usize {
    let mut at = 4;
    loop {
        let pid = u16::from_le_bytes([CYCLONE[at], CYCLONE[at + 1]]);
        let len = usize::from(u16::from_le_bytes([CYCLONE[at + 2], CYCLONE[at + 3]]));
        if pid == id {
            return at;
        }
        assert_ne!(pid, pid::SENTINEL, "{id:#06x} is not in the fixture");
        at += 4 + len;
    }
}

#[test]
fn a_policy_cut_short_is_a_decode_error() {
    // RELIABILITY's twelve octets cut to four: the kind is there, the
    // blocking time is not. The parameter after it moves up, so the
    // list stays well-formed and only the policy is malformed.
    let at = header_of(pid::RELIABILITY);
    let mut bytes = CYCLONE[..at].to_vec();
    bytes.extend_from_slice(&[0x1a, 0x00, 0x04, 0x00, 0x02, 0x00, 0x00, 0x00]);
    bytes.extend_from_slice(&CYCLONE[at + 16..]);
    assert!(TopicBuiltinTopicData::from_pl_cdr(&bytes, VendorId::CYCLONE).is_err());
}

#[test]
fn an_announcement_without_its_topic_name_is_a_decode_error() {
    let at = header_of(pid::TOPIC_NAME);
    let mut bytes = CYCLONE[..at].to_vec();
    bytes.extend_from_slice(&CYCLONE[header_of(pid::TYPE_NAME)..]);
    assert!(TopicBuiltinTopicData::from_pl_cdr(&bytes, VendorId::CYCLONE).is_err());
}
