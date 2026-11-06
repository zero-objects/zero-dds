// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps`, from its non-test sources
//! (`runtime.rs` up to its test modules, `wlp.rs`, `type_lookup_transport.rs`,
//! `participant.rs`, `publisher.rs`, `subscriber.rs`, `builtin_topics.rs`,
//! `qos.rs`, `access_request.rs`, `time.rs`, `presentation.rs`,
//! `group_order.rs`, `contained.rs`, `sample_bytes.rs`, the transport glue):
//! - `wire_types`: `EntityId` (`PARTICIPANT`), `EntityKind` (the user writer
//!   and reader kinds, the group kinds), `TopicKind::WithKey`, `Guid`
//!   (`new`, `from_bytes`), `GuidPrefix::zerodds`, `VendorId::ZERODDS`,
//!   `ProtocolVersion`, `SequenceNumber`, `Locator`, `LocatorKind::{UdpV4,
//!   Shm}`, `PortMapping` (`SPEC`, `Default`), `spdp_unicast_port`,
//!   `SPDP_DEFAULT_MULTICAST_ADDRESS`, `set_tcp_kinds_fast_dds`.
//! - `header::RtpsHeader`, `submessage_header::{SubmessageHeader,
//!   SubmessageId::from_u8, FLAG_E_LITTLE_ENDIAN}`, `datagram::{decode_datagram,
//!   decode_datagram_shared, decode_datagram_shared_into, encode_datagram,
//!   ParsedDatagram, SubmessageFrame, ParsedSubmessage, ReceiveContext,
//!   ReceptionClock, ReceiverWalk, Rejection, set_rejection_hook,
//!   refused_messages, invalid_message_rests,
//!   skipped_must_understand_submessages}`, `receiver_state::ReceiverState`,
//!   `header_extension::{HeTimestamp, HeaderExtension::restate_for_rebuilt_message}`.
//! - `submessages::{DataSubmessage, DATA_FLAG_NON_STANDARD, HeartbeatSubmessage,
//!   HeartbeatGroupInfo, AckNackSubmessage, GapSubmessage, SequenceNumberSet,
//!   RTPS_BITMAP_MAX_BITS, set_accept_empty_acknack_base_zero}`,
//!   `payload::Payload` (`from`, `whole`).
//! - `parameter_list::{ParameterList, pid::{KEY_HASH, STATUS_INFO,
//!   TYPE_INFORMATION}}`, `inline_qos::{SampleIdentityBytes, status_info,
//!   reply_inline_qos, eprosima_related_sample_identity_param,
//!   original_writer_info_param}`, `history_cache::{ChangeKind, HistoryKind}`.
//! - `participant_data::{ParticipantBuiltinTopicData, Duration, endpoint_flag,
//!   builtin_endpoint_qos, builtin_endpoint_ext::ALL_TYPE_LOOKUP_SECURE,
//!   locator_looks_routable}`, `participant_message_data::{ParticipantMessageData,
//!   PARTICIPANT_MESSAGE_DATA_KIND_*}`, `property_list::{WireProperty,
//!   WireBinaryProperty, WirePropertyList, WireDataTags}`,
//!   `publication_data::{PublicationBuiltinTopicData, ReliabilityKind,
//!   STRING_256_MAX, data_representation::{XCDR, XCDR2, DEFAULT_OFFER,
//!   DataRepMatchMode, negotiate}}`, `subscription_data::{SubscriptionBuiltinTopicData,
//!   ContentFilterProperty, filter_class::DDSSQL}`, `topic_data::TopicBuiltinTopicData::new`.
//! - `reliable_writer::{ReliableWriter, ReliableWriterConfig, DEFAULT_FRAGMENT_SIZE,
//!   DEFAULT_HEARTBEAT_PERIOD, DEFAULT_NACK_RESPONSE_DELAY,
//!   DEFAULT_NACK_SUPPRESSION_DURATION, DEFAULT_UNACKED_WINDOW, LOOPBACK_MTU,
//!   fragment_size_for_message_size}`, `reliable_reader::{ReliableReader,
//!   ReliableReaderConfig, DeliveredSample, DEFAULT_HEARTBEAT_RESPONSE_DELAY,
//!   PayloadOpener, PayloadOpening, AsIs}`, `reader_proxy::ReaderProxy::new`,
//!   `writer_proxy::WriterProxy::new`, `fragment_assembler::AssemblerCaps`,
//!   `message_builder::{DEFAULT_MTU, OutboundDatagram, RTPS_HEADER_LEN,
//!   try_merge_message}`.
//! - `writer_group::{WriterGroup, ChangeGroupInfo, GroupSetEnd}`,
//!   `group_digest::GroupDigest::of_entities`.
//! - `dialect::{BehaviourTable, RtpsBehaviour, RtpsDialect, SpdpResponseTargets,
//!   GroupCoherentSetPid, GroupWireForm, HeartbeatGroupDigests}`.
//! - `endpoint_security_info::{EndpointSecurityInfo, attrs::IS_PAYLOAD_PROTECTED}`,
//!   `security_algo_info` (the participant and endpoint algorithm infos with
//!   `spec_default`, `AlgorithmRequirements`).
//!
//! Oracles: DDSI-RTPS 2.5 (Table 8.17, §8.3.6.3, §8.3.7.4, §8.4.7.1,
//! §8.4.10.1, §9.3.1.2 Table 9.1, §9.3.1.5, §9.3.2.1, §9.3.2.5, §9.4.2.6
//! Table 9.5, §9.4.4, §9.4.5.1, §9.4.5.2, §9.4.5.3, §9.4.5.4, §9.4.5.7,
//! §9.6.1.4.1, §9.6.2.3, §9.6.3.1, §9.6.4.3 Table 9.23, §9.6.4.7, §9.6.4.9,
//! §10.2, §10.6), DDS-XTypes 1.3 §7.4, §7.6.3.1.1, §7.6.3.1.2, §7.6.3.2.2,
//! DDS-RPC 1.0 §7.8.2, DDS-Security 1.2 §7.3.11.2, §7.3.12.2, §7.3.13.2,
//! §7.3.15.2, §8.1-§8.3, §9.4.2.8 Table 38 and §7.4.1.4 Table 12, Fast DDS
//! `ParameterTypes.hpp` (`PID_RELATED_SAMPLE_IDENTITY = 0x800f`), the MD5 of
//! RFC 1321 computed outside this code. No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use core::time::Duration as StdDuration;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds_rtps::datagram::{
    ParsedDatagram, ParsedSubmessage, ReceiveContext, ReceptionClock, Rejection, SubmessageFrame,
    decode_datagram, decode_datagram_shared, decode_datagram_shared_into, encode_datagram,
    invalid_message_rests, refused_messages, set_rejection_hook,
    skipped_must_understand_submessages,
};
use zerodds_rtps::dialect::{
    BehaviourTable, GroupCoherentSetPid, GroupWireForm, HeartbeatGroupDigests, RtpsBehaviour,
    RtpsDialect, SpdpResponseTargets,
};
use zerodds_rtps::endpoint_security_info::{EndpointSecurityInfo, attrs};
use zerodds_rtps::fragment_assembler::AssemblerCaps;
use zerodds_rtps::group_digest::GroupDigest;
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::header_extension::{HeTimestamp, HeaderExtension};
use zerodds_rtps::history_cache::{ChangeKind, HistoryKind};
use zerodds_rtps::inline_qos::{
    SampleIdentityBytes, eprosima_related_sample_identity_param, original_writer_info_param,
    reply_inline_qos, status_info,
};
use zerodds_rtps::message_builder::{
    DEFAULT_MTU, OutboundDatagram, RTPS_HEADER_LEN, try_merge_message,
};
use zerodds_rtps::parameter_list::{Parameter, ParameterList, pid};
use zerodds_rtps::participant_data::{
    Duration, ParticipantBuiltinTopicData, builtin_endpoint_ext, builtin_endpoint_qos,
    endpoint_flag, locator_looks_routable,
};
use zerodds_rtps::participant_message_data::{
    PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
    PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
    PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC, ParticipantMessageData,
};
use zerodds_rtps::payload::Payload;
use zerodds_rtps::property_list::{
    WireBinaryProperty, WireDataTags, WireProperty, WirePropertyList,
};
use zerodds_rtps::publication_data::data_representation::{
    self as dr, DEFAULT_OFFER, DataRepMatchMode, XCDR, XCDR2,
};
use zerodds_rtps::publication_data::{
    PublicationBuiltinTopicData, ReliabilityKind, STRING_256_MAX,
};
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_reader::{
    AsIs, DEFAULT_HEARTBEAT_RESPONSE_DELAY, DeliveredSample, PayloadOpener, PayloadOpening,
    ReliableReader, ReliableReaderConfig,
};
use zerodds_rtps::reliable_writer::{
    DEFAULT_FRAGMENT_SIZE, DEFAULT_HEARTBEAT_PERIOD, DEFAULT_NACK_RESPONSE_DELAY,
    DEFAULT_NACK_SUPPRESSION_DURATION, DEFAULT_UNACKED_WINDOW, LOOPBACK_MTU, ReliableWriter,
    ReliableWriterConfig, fragment_size_for_message_size,
};
use zerodds_rtps::security_algo_info::{
    AlgorithmRequirements, EndpointSecuritySymmetricCipherAlgorithmInfo,
    ParticipantSecurityDigitalSignatureAlgorithmInfo,
    ParticipantSecurityKeyEstablishmentAlgorithmInfo,
    ParticipantSecuritySymmetricCipherAlgorithmInfo,
};
use zerodds_rtps::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use zerodds_rtps::submessages::{
    AckNackSubmessage, DATA_FLAG_NON_STANDARD, DataSubmessage, HeartbeatGroupInfo,
    HeartbeatSubmessage, RTPS_BITMAP_MAX_BITS, SequenceNumberSet,
    set_accept_empty_acknack_base_zero,
};
use zerodds_rtps::subscription_data::{
    ContentFilterProperty, SubscriptionBuiltinTopicData, filter_class,
};
use zerodds_rtps::topic_data::TopicBuiltinTopicData;
use zerodds_rtps::wire_types::{
    EntityId, EntityKind, Guid, GuidPrefix, Locator, LocatorKind, PortMapping, ProtocolVersion,
    SPDP_DEFAULT_MULTICAST_ADDRESS, SequenceNumber, TopicKind, VendorId, set_tcp_kinds_fast_dds,
    spdp_unicast_port,
};
use zerodds_rtps::writer_group::WriterGroup;
use zerodds_rtps::writer_proxy::WriterProxy;

fn prefix(tag: u8) -> GuidPrefix {
    GuidPrefix::zerodds([tag, 0, 0, tag], [0, 0, 0, 0, 0, tag])
}

// ---------------------------------------------------------------------
// Identities, ports and locators
// ---------------------------------------------------------------------

#[test]
fn rtps_9_3_1_2_user_and_group_entity_kinds_follow_table_9_1() {
    for (kind, octet) in [
        (EntityKind::UserWriterWithKey, 0x02),
        (EntityKind::UserWriterNoKey, 0x03),
        (EntityKind::UserReaderNoKey, 0x04),
        (EntityKind::UserReaderWithKey, 0x07),
        (EntityKind::UserWriterGroup, 0x08),
        (EntityKind::UserReaderGroup, 0x09),
        (EntityKind::Participant, 0xc1),
    ] {
        assert_eq!(kind.to_byte(), octet, "{kind:?}");
        assert_eq!(EntityKind::from_byte(octet), kind);
    }
    assert_eq!(
        TopicKind::WithKey.writer_entity_kind(false),
        EntityKind::UserWriterWithKey
    );
    assert_eq!(
        TopicKind::WithKey.reader_entity_kind(false),
        EntityKind::UserReaderWithKey
    );
    assert_eq!(
        TopicKind::NoKey.writer_entity_kind(false),
        EntityKind::UserWriterNoKey
    );
    assert_eq!(
        TopicKind::NoKey.reader_entity_kind(false),
        EntityKind::UserReaderNoKey
    );
    assert_eq!(
        EntityKind::UserReaderWithKey.topic_kind(),
        Some(TopicKind::WithKey)
    );
    assert_eq!(EntityId::PARTICIPANT.to_bytes(), [0, 0, 1, 0xc1]);
}

#[test]
fn rtps_9_3_1_5_a_zerodds_prefix_starts_with_the_vendor_id() {
    let p = GuidPrefix::zerodds([1, 2, 3, 4], [5, 6, 7, 8, 9, 10]);
    assert_eq!(p.to_bytes(), [0x01, 0xF0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(p.to_bytes()[..2], VendorId::ZERODDS.to_bytes());
    assert_eq!(p.host_id(), [1, 2, 3, 4]);
    let g = Guid::new(p, EntityId::user_writer_with_key([0, 0, 1]));
    assert_eq!(Guid::from_bytes(g.to_bytes()), g);
    assert_eq!(ProtocolVersion::V2_5.to_bytes(), [2, 5]);
    assert_eq!(
        SequenceNumber::from_high_low(1, 2),
        SequenceNumber((1 << 32) | 2)
    );
}

#[test]
fn rtps_9_6_2_3_default_port_numbers_and_the_spdp_multicast_group() {
    let spec = PortMapping::SPEC;
    assert_eq!(
        (
            spec.port_base,
            spec.domain_gain,
            spec.participant_gain,
            spec.d0,
            spec.d1,
            spec.d2,
            spec.d3
        ),
        (7400, 250, 2, 0, 10, 1, 11)
    );
    assert_eq!(PortMapping::default(), spec);
    // PB + DG * domain + d1 + PG * participant.
    assert_eq!(spdp_unicast_port(0, 0), 7410);
    assert_eq!(spdp_unicast_port(1, 2), 7400 + 250 + 10 + 4);
    assert_eq!(spec.spdp_multicast_port(3), 7400 + 750);
    assert_eq!(spec.user_traffic_unicast_port(0, 1), 7400 + 11 + 2);
    assert_eq!(spec.user_traffic_multicast_port(0), 7401);
    // §9.6.1.4.1: DefaultMulticastLocator 239.255.0.1.
    assert_eq!(SPDP_DEFAULT_MULTICAST_ADDRESS, [239, 255, 0, 1]);
}

#[test]
fn the_locator_kinds_the_runtime_routes_by() {
    assert_eq!(LocatorKind::UdpV4.as_i32(), 1);
    assert_eq!(LocatorKind::Shm.as_i32(), 0x01F0_0001);
    assert_eq!(
        Locator::udp_v4([10, 0, 0, 1], 7411).kind,
        LocatorKind::UdpV4
    );
    assert_eq!(Locator::shm([3; 16]).kind, LocatorKind::Shm);
    // `transport.tcp_locator_kinds: fast-dds` switches the wire value.
    set_tcp_kinds_fast_dds(true);
    let fast = LocatorKind::Tcpv4.as_i32();
    set_tcp_kinds_fast_dds(false);
    assert_eq!(fast, 4);
    assert_ne!(LocatorKind::Tcpv4.as_i32(), 4);
}

#[test]
fn routable_locators_exclude_unspecified_and_link_local_udpv4() {
    assert!(!locator_looks_routable(&Locator::udp_v4(
        [0, 0, 0, 0],
        7410
    )));
    assert!(!locator_looks_routable(&Locator::udp_v4(
        [169, 254, 3, 4],
        7410
    )));
    assert!(locator_looks_routable(&Locator::udp_v4(
        [127, 0, 0, 1],
        7410
    )));
    assert!(locator_looks_routable(&Locator::udp_v4(
        [192, 168, 1, 7],
        7410
    )));
    assert!(locator_looks_routable(&Locator::shm([0; 16])));
}

// ---------------------------------------------------------------------
// Messages and the receiver
// ---------------------------------------------------------------------

#[test]
fn rtps_9_4_5_1_submessage_header_and_ids() {
    assert_eq!(FLAG_E_LITTLE_ENDIAN, 0x01);
    let header = SubmessageHeader {
        submessage_id: SubmessageId::Heartbeat,
        flags: FLAG_E_LITTLE_ENDIAN,
        octets_to_next_header: 28,
    };
    assert_eq!(header.to_bytes(), [0x07, 0x01, 28, 0]);
    for (id, byte) in [
        (SubmessageId::Pad, 0x01),
        (SubmessageId::AckNack, 0x06),
        (SubmessageId::Heartbeat, 0x07),
        (SubmessageId::Gap, 0x08),
        (SubmessageId::InfoTs, 0x09),
        (SubmessageId::InfoSrc, 0x0c),
        (SubmessageId::InfoReplyIp4, 0x0d),
        (SubmessageId::InfoDst, 0x0e),
        (SubmessageId::InfoReply, 0x0f),
        (SubmessageId::NackFrag, 0x12),
        (SubmessageId::HeartbeatFrag, 0x13),
        (SubmessageId::Data, 0x15),
        (SubmessageId::DataFrag, 0x16),
    ] {
        assert_eq!(SubmessageId::from_u8(byte).unwrap(), id);
        assert_eq!(id.as_u8(), byte);
    }
    assert!(
        SubmessageId::from_u8(0x80).is_err(),
        "0x80..=0xff are vendor-specific"
    );
}

fn header_bytes(p: GuidPrefix) -> Vec<u8> {
    RtpsHeader::new(VendorId::ZERODDS, p).to_bytes().to_vec()
}

/// A HEARTBEAT laid out by hand from §9.4.5.7, little-endian, F set.
fn heartbeat_bytes(last: u32) -> Vec<u8> {
    let mut out = vec![0x07, 0x01 | 0x02, 28, 0];
    out.extend_from_slice(&EntityId::UNKNOWN.to_bytes());
    out.extend_from_slice(&EntityId::user_writer_with_key([0, 0, 1]).to_bytes());
    out.extend_from_slice(&0_i32.to_le_bytes());
    out.extend_from_slice(&1_u32.to_le_bytes());
    out.extend_from_slice(&0_i32.to_le_bytes());
    out.extend_from_slice(&last.to_le_bytes());
    out.extend_from_slice(&1_i32.to_le_bytes());
    out
}

#[test]
fn rtps_9_4_4_shared_decodes_agree_with_the_plain_decode() {
    let mut bytes = header_bytes(prefix(1));
    bytes.extend(heartbeat_bytes(3));
    let plain = decode_datagram(&bytes).unwrap();
    let shared: Arc<[u8]> = Arc::from(bytes.clone().into_boxed_slice());
    assert_eq!(decode_datagram_shared(&shared).unwrap(), plain);
    let reused = decode_datagram_shared_into(&shared, plain.submessages.clone()).unwrap();
    assert_eq!(
        reused.submessages, plain.submessages,
        "the frames passed in are cleared first"
    );
    assert_eq!(plain.header.vendor_id, VendorId::ZERODDS);
    assert!(plain.invalid_rest.is_none());
}

#[test]
fn a_re_encoded_datagram_keeps_every_octet_including_unknown_submessages() {
    let mut bytes = header_bytes(prefix(2));
    bytes.extend(heartbeat_bytes(4));
    // A vendor-specific submessage (§9.4.5.1.3: 0x80..=0xff) with a body.
    bytes.extend([0x80, 0x01, 4, 0, 0xde, 0xad, 0xbe, 0xef]);
    let parsed = decode_datagram(&bytes).unwrap();
    assert!(parsed.contents().any(|s| matches!(
        s,
        ParsedSubmessage::Unknown { id: 0x80, body, .. } if body == &[0xde, 0xad, 0xbe, 0xef]
    )));
    assert_eq!(encode_datagram(&parsed).unwrap(), bytes);
}

#[test]
fn rtps_9_4_2_6_sequence_number_set_of_table_9_5() {
    // "1234/12:00110": 1236 and 1237 are in the set.
    let set = SequenceNumberSet {
        bitmap_base: SequenceNumber(1234),
        num_bits: 12,
        bitmap: vec![0x3000_0000],
    };
    let members: Vec<i64> = set.iter_set().map(|sn| sn.0).collect();
    assert_eq!(members, [1236, 1237]);
    let mut wire = Vec::new();
    set.write_to(&mut wire, true);
    let mut expected = Vec::new();
    expected.extend(0_i32.to_le_bytes());
    expected.extend(1234_u32.to_le_bytes());
    expected.extend(12_u32.to_le_bytes());
    expected.extend(0x3000_0000_u32.to_le_bytes());
    assert_eq!(wire, expected);
    let built = SequenceNumberSet::from_missing(
        SequenceNumber(1234),
        &[SequenceNumber(1236), SequenceNumber(1237)],
    );
    let built_members: Vec<i64> = built.iter_set().map(|sn| sn.0).collect();
    assert_eq!(built_members, [1236, 1237]);
    assert_eq!(RTPS_BITMAP_MAX_BITS, 256, "§9.4.2.6: 0 <= numBits <= 256");
}

fn acknack_frame(base: i64, missing: &[i64], count: i32) -> SubmessageFrame {
    let missing: Vec<SequenceNumber> = missing.iter().map(|&s| SequenceNumber(s)).collect();
    SubmessageFrame {
        little_endian: true,
        octets_elided: false,
        submessage: ParsedSubmessage::AckNack(AckNackSubmessage {
            reader_id: EntityId::user_reader_with_key([0, 0, 2]),
            writer_id: EntityId::user_writer_with_key([0, 0, 1]),
            reader_sn_state: SequenceNumberSet::from_missing(SequenceNumber(base), &missing),
            count,
            final_flag: true,
        }),
    }
}

#[test]
fn rtps_9_4_5_3_acknack_layout_through_encode_datagram() {
    let datagram = ParsedDatagram {
        header: RtpsHeader::new(VendorId::ZERODDS, prefix(3)),
        submessages: vec![acknack_frame(5, &[5, 7], 9)],
        invalid_rest: None,
    };
    let bytes = encode_datagram(&datagram).unwrap();
    let sub = &bytes[RTPS_HEADER_LEN..];
    assert_eq!(sub[0], 0x06, "ACKNACK");
    assert_eq!(sub[1], 0x01 | 0x02, "E and F");
    assert_eq!(sub[4..8], [0, 0, 2, 0x07]);
    assert_eq!(sub[8..12], [0, 0, 1, 0x02]);
    assert_eq!(sub[12..20], [0, 0, 0, 0, 5, 0, 0, 0], "bitmapBase 5");
    assert_eq!(sub[sub.len() - 4..], 9_i32.to_le_bytes(), "count last");
    let back = decode_datagram(&bytes).unwrap();
    assert_eq!(back.submessages, datagram.submessages);
}

#[test]
fn rtps_9_4_2_6_an_acknack_with_bitmap_base_zero_needs_the_interop_setting() {
    let mut bytes = header_bytes(prefix(4));
    // ACKNACK readerSNState 0/0: (OpenDDS, Connext), count 1.
    bytes.extend([0x06, 0x01, 24, 0]);
    bytes.extend(EntityId::user_reader_with_key([0, 0, 2]).to_bytes());
    bytes.extend(EntityId::user_writer_with_key([0, 0, 1]).to_bytes());
    bytes.extend([0; 8]);
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(1_i32.to_le_bytes());
    let rests = invalid_message_rests();
    let strict = decode_datagram(&bytes).unwrap();
    assert!(strict.invalid_rest.is_some(), "§9.4.2.6: bitmapBase >= 1");
    assert!(
        !strict
            .contents()
            .any(|s| matches!(s, ParsedSubmessage::AckNack(_)))
    );
    assert_eq!(invalid_message_rests(), rests + 1);
    set_accept_empty_acknack_base_zero(true);
    let tolerant = decode_datagram(&bytes).unwrap();
    set_accept_empty_acknack_base_zero(false);
    assert!(
        tolerant
            .contents()
            .any(|s| matches!(s, ParsedSubmessage::AckNack(_)))
    );
}

static HOOK_CALLS: AtomicUsize = AtomicUsize::new(0);

fn hook(rejection: &Rejection) {
    if rejection.vendor_id == VendorId::ZERODDS.to_bytes() {
        HOOK_CALLS.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn rtps_8_3_6_3_a_message_of_a_higher_major_version_is_refused_whole() {
    assert!(set_rejection_hook(hook), "first installation");
    assert!(!set_rejection_hook(hook), "set once per process");
    let mut bytes = header_bytes(prefix(5));
    bytes[4] = 3;
    bytes.extend(heartbeat_bytes(1));
    let refused = refused_messages();
    let outcome = decode_datagram(&bytes);
    if let Ok(parsed) = outcome {
        assert!(
            parsed.submessages.is_empty(),
            "nothing of a refused message is taken"
        );
    }
    assert_eq!(refused_messages(), refused + 1);
    assert_eq!(HOOK_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn rtps_9_6_2_2_1_a_data_with_an_unknown_must_understand_pid_is_skipped() {
    let mut qos = ParameterList::new();
    // 0x4000 is the must-understand bit; 0x0fff is no PID of the spec.
    qos.push(Parameter::new(0x4fff, vec![0, 0, 0, 0]));
    let data = DataSubmessage {
        extra_flags: 0,
        reader_id: EntityId::UNKNOWN,
        writer_id: EntityId::user_writer_with_key([0, 0, 1]),
        writer_sn: SequenceNumber(1),
        inline_qos: Some(qos.into()),
        data_flag: true,
        key_flag: false,
        non_standard_flag: false,
        serialized_payload: Payload::from(vec![0, 1, 0, 0]),
    };
    let datagram = ParsedDatagram {
        header: RtpsHeader::new(VendorId::ZERODDS, prefix(6)),
        submessages: vec![SubmessageFrame {
            little_endian: true,
            octets_elided: false,
            submessage: ParsedSubmessage::Data(data),
        }],
        invalid_rest: None,
    };
    let bytes = encode_datagram(&datagram).unwrap();
    let skipped = skipped_must_understand_submessages();
    let parsed = decode_datagram(&bytes).unwrap();
    assert!(
        !parsed
            .contents()
            .any(|s| matches!(s, ParsedSubmessage::Data(_)))
    );
    assert_eq!(skipped_must_understand_submessages(), skipped + 1);
}

#[test]
fn rtps_9_4_5_4_key_and_non_standard_flags() {
    assert_eq!(DATA_FLAG_NON_STANDARD, 0x10);
    let base = DataSubmessage {
        extra_flags: 0,
        reader_id: EntityId::UNKNOWN,
        writer_id: EntityId::user_writer_with_key([0, 0, 1]),
        writer_sn: SequenceNumber(2),
        inline_qos: None,
        data_flag: true,
        key_flag: false,
        non_standard_flag: true,
        serialized_payload: Payload::whole(Arc::from(vec![9_u8, 9, 9, 9].into_boxed_slice())),
    };
    let (_, flags) = base.write_body(true);
    assert_eq!(flags & 0x10, 0x10, "N");
    assert_eq!(flags & 0x04, 0x04, "D");
    let key = DataSubmessage {
        data_flag: false,
        key_flag: true,
        non_standard_flag: false,
        ..base.clone()
    };
    let (_, flags) = key.write_body(true);
    assert_eq!(flags & 0x0c, 0x08, "K without D");
    assert_eq!(&*base.serialized_payload, &[9, 9, 9, 9]);
    assert_eq!(&*Payload::from(vec![1_u8, 2]), &[1, 2]);
}

#[test]
fn rtps_9_4_5_7_heartbeat_group_info_sets_g() {
    let hb = HeartbeatSubmessage {
        reader_id: EntityId::UNKNOWN,
        writer_id: EntityId::user_writer_with_key([0, 0, 1]),
        first_sn: SequenceNumber(1),
        last_sn: SequenceNumber(3),
        count: 2,
        final_flag: false,
        liveliness_flag: false,
        group_info: Some(HeartbeatGroupInfo {
            current_gsn: SequenceNumber(14),
            first_gsn: SequenceNumber(11),
            last_gsn: SequenceNumber(13),
            // writerSet and secureWriterSet go out together after lastGSN
            // (Table 8.44, §9.4.2.19); a missing one is written as zeros.
            writer_set: Some(GroupDigest([1, 2, 3, 4])),
            secure_writer_set: Some(GroupDigest([5, 6, 7, 8])),
        }),
    };
    let (_, flags) = hb.write_body(true);
    assert_eq!(flags & 0x08, 0x08, "G");
    let datagram = ParsedDatagram {
        header: RtpsHeader::new(VendorId::ZERODDS, prefix(7)),
        submessages: vec![SubmessageFrame {
            little_endian: true,
            octets_elided: false,
            submessage: ParsedSubmessage::Heartbeat(hb.clone()),
        }],
        invalid_rest: None,
    };
    let back = decode_datagram(&encode_datagram(&datagram).unwrap()).unwrap();
    match back.contents().next() {
        Some(ParsedSubmessage::Heartbeat(got)) => assert_eq!(got, &hb),
        other => panic!("expected HEARTBEAT, got {other:?}"),
    }
}

/// A HeaderExtension (§9.4.5.2) with L and T: messageLength, then the
/// rtpsSendTimestamp.
fn header_extension(message_length: u32, sent: HeTimestamp) -> Vec<u8> {
    let mut out = vec![0x00, 0x01 | 0x02 | 0x04, 12, 0];
    out.extend(message_length.to_le_bytes());
    out.extend(sent.seconds.to_le_bytes());
    out.extend(sent.fraction.to_le_bytes());
    out
}

#[test]
fn rtps_8_3_7_4_the_reception_clock_and_the_send_timestamp_reach_the_receiver() {
    fn now() -> HeTimestamp {
        HeTimestamp {
            seconds: 1_000,
            fraction: 0,
        }
    }
    let sent = HeTimestamp {
        seconds: 999,
        fraction: 1 << 31,
    };
    let mut bytes = header_bytes(prefix(8));
    let total = (RTPS_HEADER_LEN + 16 + 32) as u32;
    bytes.extend(header_extension(total, sent));
    bytes.extend(heartbeat_bytes(1));
    assert_eq!(bytes.len() as u32, total);
    let source = Locator::udp_v4([192, 168, 0, 9], 40_000);
    let context = ReceiveContext {
        source: Some(source),
        clock: Some(ReceptionClock {
            now,
            skew_threshold_seconds: None,
        }),
    };
    let parsed = decode_datagram(&bytes).unwrap();
    let state = parsed
        .receiver_walk_in(prefix(9), &context)
        .find_map(|(state, sub)| matches!(sub, ParsedSubmessage::Heartbeat(_)).then_some(state))
        .unwrap();
    assert_eq!(state.rtps_send_timestamp, Some(sent));
    assert_eq!(state.rtps_reception_timestamp, Some(now()));
    // Table 8.17: the reply locators start from the transport and source
    // address, port LOCATOR_PORT_INVALID.
    assert_eq!(
        state.unicast_reply_locator_list,
        [Locator {
            kind: LocatorKind::UdpV4,
            port: 0,
            address: source.address
        }]
    );
    assert_eq!(state.dest_guid_prefix, prefix(9));
}

#[test]
fn rtps_8_3_7_4_a_rebuilt_message_gets_its_own_message_length() {
    let sent = HeTimestamp {
        seconds: 1,
        fraction: 0,
    };
    let mut received = header_bytes(prefix(10));
    let total = (RTPS_HEADER_LEN + 16 + 32) as u32;
    received.extend(header_extension(total, sent));
    received.extend(heartbeat_bytes(1));
    let mut rebuilt = received.clone();
    rebuilt.extend(heartbeat_bytes(2));
    HeaderExtension::restate_for_rebuilt_message(&received, &mut rebuilt).unwrap();
    let length = u32::from_le_bytes(
        rebuilt[RTPS_HEADER_LEN + 4..RTPS_HEADER_LEN + 8]
            .try_into()
            .unwrap(),
    );
    assert_eq!(length as usize, rebuilt.len());
    // A received message whose messageLength is wrong is refused and the
    // rebuilt one left as it is.
    let mut lying = received.clone();
    lying[RTPS_HEADER_LEN + 4] ^= 0xff;
    let before = rebuilt.clone();
    assert!(HeaderExtension::restate_for_rebuilt_message(&lying, &mut rebuilt).is_err());
    assert_eq!(rebuilt, before);
}

#[test]
fn messages_merge_only_under_one_header_and_within_the_size() {
    assert_eq!(RTPS_HEADER_LEN, 20);
    assert_eq!(DEFAULT_MTU, 1500 - 20 - 8, "IPv4 header 20, UDP header 8");
    let mut pending = header_bytes(prefix(11));
    pending.extend(heartbeat_bytes(1));
    let mut next = header_bytes(prefix(11));
    next.extend(heartbeat_bytes(2));
    assert!(try_merge_message(&mut pending, &next, DEFAULT_MTU));
    let parsed = decode_datagram(&pending).unwrap();
    let lasts: Vec<i64> = parsed
        .contents()
        .filter_map(|s| match s {
            ParsedSubmessage::Heartbeat(h) => Some(h.last_sn.0),
            _ => None,
        })
        .collect();
    assert_eq!(lasts, [1, 2]);
    let mut foreign = header_bytes(prefix(12));
    foreign.extend(heartbeat_bytes(3));
    let before = pending.clone();
    assert!(
        !try_merge_message(&mut pending, &foreign, DEFAULT_MTU),
        "another participant's header"
    );
    let limit = pending.len() + 10;
    assert!(!try_merge_message(&mut pending, &next, limit), "too large");
    assert_eq!(pending, before);
}

// ---------------------------------------------------------------------
// Inline QoS and lifecycle
// ---------------------------------------------------------------------

#[test]
fn the_inline_qos_pids_the_runtime_writes_and_reads() {
    assert_eq!((pid::KEY_HASH, pid::STATUS_INFO), (0x0070, 0x0071));
    // DDS-XTypes 1.3 §7.6.3.2.2: @id(0x0075) type_information.
    assert_eq!(pid::TYPE_INFORMATION, 0x0075);
    assert_eq!((status_info::DISPOSED, status_info::UNREGISTERED), (1, 2));
    let id = SampleIdentityBytes::new([0x11; 16], (2 << 32) | 5);
    // DDS-RPC 1.0 §7.8.2: PID_RELATED_SAMPLE_IDENTITY = 0x0083.
    let reply = reply_inline_qos(id, true);
    let p = reply.find(0x0083).unwrap();
    assert_eq!(p.value[..16], [0x11; 16]);
    assert_eq!(p.value[16..20], 2_i32.to_le_bytes());
    assert_eq!(p.value[20..24], 5_u32.to_le_bytes());
    // Fast DDS ParameterTypes.hpp: PID_RELATED_SAMPLE_IDENTITY = 0x800f.
    let eprosima = eprosima_related_sample_identity_param(id, true);
    assert_eq!(eprosima.id, 0x800f);
    assert_eq!(eprosima.value, p.value);
}

#[test]
fn rtps_9_6_4_7_original_writer_info_is_guid_then_sequence_number() {
    let p = original_writer_info_param([0x22; 16], (1 << 32) | 7, true);
    assert_eq!(p.id, 0x0061);
    assert_eq!(p.value.len(), 24);
    assert_eq!(p.value[..16], [0x22; 16]);
    assert_eq!(p.value[16..20], 1_i32.to_le_bytes(), "high");
    assert_eq!(p.value[20..24], 7_u32.to_le_bytes(), "low");
}

fn writer_guid() -> Guid {
    Guid::new(prefix(20), EntityId::user_writer_with_key([0, 0, 1]))
}

fn reader_guid() -> Guid {
    Guid::new(prefix(21), EntityId::user_reader_with_key([0, 0, 2]))
}

const LOOPBACK: [u8; 4] = [127, 0, 0, 1];

fn reader() -> ReliableReader {
    ReliableReader::new(ReliableReaderConfig {
        guid: reader_guid(),
        vendor_id: VendorId::ZERODDS,
        writer_proxies: vec![WriterProxy::new(
            writer_guid(),
            vec![Locator::udp_v4(LOOPBACK, 7410)],
            vec![],
            true,
        )],
        max_samples_per_proxy: 64,
        heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
        assembler_caps: AssemblerCaps::default(),
    })
}

fn lifecycle(sn: i64, bits: u32) -> DataSubmessage {
    let mut qos = ParameterList::new();
    qos.push(Parameter::new(pid::KEY_HASH, vec![0x5a; 16]));
    qos.push(Parameter::new(
        pid::STATUS_INFO,
        bits.to_be_bytes().to_vec(),
    ));
    DataSubmessage {
        extra_flags: 0,
        reader_id: reader_guid().entity_id,
        writer_id: writer_guid().entity_id,
        writer_sn: SequenceNumber(sn),
        inline_qos: Some(qos.into()),
        data_flag: false,
        key_flag: false,
        non_standard_flag: false,
        serialized_payload: Payload::from(Vec::new()),
    }
}

#[test]
fn rtps_9_6_4_9_status_info_flags_become_the_change_kinds() {
    let mut r = reader();
    let mut kinds = Vec::new();
    for (sn, bits) in [
        (1, status_info::DISPOSED),
        (2, status_info::UNREGISTERED),
        (3, status_info::DISPOSED | status_info::UNREGISTERED),
        // F = 1, D = U = 0: the writer filtered the sample out for this
        // reader (§9.6.4.9). No sample reaches the reader, and the number
        // counts as received: 5 is delivered behind it.
        (4, status_info::FILTERED),
        (5, status_info::DISPOSED),
    ] {
        for sample in r.handle_data(writer_guid().prefix, &lifecycle(sn, bits), None) {
            kinds.push((sample.kind, sample.key_hash));
        }
    }
    let key = Some([0x5a; 16]);
    assert_eq!(
        kinds,
        [
            (ChangeKind::NotAliveDisposed, key),
            (ChangeKind::NotAliveUnregistered, key),
            (ChangeKind::NotAliveDisposedUnregistered, key),
            (ChangeKind::NotAliveDisposed, key),
        ]
    );
    assert_ne!(ChangeKind::Alive, ChangeKind::AliveFiltered);
}

// ---------------------------------------------------------------------
// Builtin topic data and liveliness
// ---------------------------------------------------------------------

#[test]
fn rtps_9_3_2_participant_data_with_endpoint_sets_and_qos() {
    assert_eq!(
        builtin_endpoint_qos::BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER,
        1 << 0
    );
    // DDS-Security 1.2 Table 12: the secure TypeLookup endpoints, bits 0-3.
    assert_eq!(builtin_endpoint_ext::ALL_TYPE_LOOKUP_SECURE, 0b1111);
    assert_eq!(endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER, 1 << 10);
    assert_eq!(endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER, 1 << 11);
    let guid = Guid::new(prefix(30), EntityId::PARTICIPANT);
    let mut pl = ParameterList::new();
    pl.push(Parameter::new(
        pid::PARTICIPANT_GUID,
        guid.to_bytes().to_vec(),
    ));
    pl.push(Parameter::new(pid::PROTOCOL_VERSION, vec![2, 5, 0, 0]));
    pl.push(Parameter::new(pid::VENDOR_ID, vec![0x01, 0xF0, 0, 0]));
    pl.push(Parameter::new(
        pid::DEFAULT_UNICAST_LOCATOR,
        Locator::udp_v4([10, 1, 1, 1], 7411).to_bytes_le().to_vec(),
    ));
    let set = endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER
        | endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER;
    pl.push(Parameter::new(
        pid::BUILTIN_ENDPOINT_SET,
        set.to_le_bytes().to_vec(),
    ));
    // §9.6.2.2 Table 9.13: PID_BUILTIN_ENDPOINT_QOS = 0x0077.
    pl.push(Parameter::new(0x0077, 1_u32.to_le_bytes().to_vec()));
    pl.push(Parameter::new(
        pid::PARTICIPANT_LEASE_DURATION,
        vec![30, 0, 0, 0, 0, 0, 0, 0],
    ));
    let mut bytes = vec![0x00, 0x03, 0x00, 0x00];
    bytes.extend(pl.to_bytes(true));
    let participant = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(participant.guid, guid);
    assert_eq!(participant.builtin_endpoint_set, set);
    assert_eq!(participant.builtin_endpoint_qos, Some(1));
    assert_eq!(participant.lease_duration, Duration::from_secs(30));
    assert_eq!(
        participant.default_unicast_locators,
        [Locator::udp_v4([10, 1, 1, 1], 7411)]
    );
    let again = ParticipantBuiltinTopicData::from_pl_cdr_le(&participant.to_pl_cdr_le()).unwrap();
    assert_eq!(again, participant);
}

#[test]
fn rtps_9_6_3_1_participant_message_data_kinds_and_layout() {
    assert_eq!(
        PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
        0x0000_0001
    );
    assert_eq!(
        PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
        0x0000_0002
    );
    // kind.value[0] & 0x80: vendor-specific.
    assert_eq!(
        PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC & 0x8000_0000,
        0x8000_0000
    );
    let p = prefix(31);
    let bytes = ParticipantMessageData::automatic(p).to_cdr(true).unwrap();
    // §10.2: CDR_LE {0x00, 0x01}, options; then GuidPrefix, the kind as four
    // octets, and the empty data sequence.
    assert_eq!(bytes[..4], [0x00, 0x01, 0x00, 0x00]);
    assert_eq!(bytes[4..16], p.to_bytes());
    assert_eq!(bytes[16..20], [0, 0, 0, 1]);
    assert_eq!(bytes[20..24], 0_u32.to_le_bytes());
    let back = ParticipantMessageData::from_cdr(&bytes).unwrap();
    assert_eq!(back, ParticipantMessageData::automatic(p));
    let manual = ParticipantMessageData::manual_by_participant(p);
    assert_eq!(
        ParticipantMessageData::from_cdr(&manual.to_cdr(false).unwrap()).unwrap(),
        manual
    );
}

#[test]
fn property_lists_and_data_tags_survive_the_wire() {
    let list = WirePropertyList::new()
        .with(WireProperty::new(
            "dds.sec.auth.plugin_class",
            "DDS:Auth:PKI-DH:1.0",
        ))
        .with_binary(WireBinaryProperty::new("dds.sec.blob", vec![1, 2, 3]));
    assert_eq!(
        list.get("dds.sec.auth.plugin_class"),
        Some("DDS:Auth:PKI-DH:1.0")
    );
    let back = WirePropertyList::decode(&list.encode(true).unwrap(), true).unwrap();
    assert_eq!(
        back.get("dds.sec.auth.plugin_class"),
        Some("DDS:Auth:PKI-DH:1.0")
    );
    let mut plain = WirePropertyList::new();
    plain.push(WireProperty::new("a", "b"));
    assert_eq!(plain.len(), 1);
    let tags = WireDataTags {
        tags: vec![WireProperty::new("classification", "secret")],
    };
    let back = WireDataTags::decode(&tags.encode(true).unwrap(), true).unwrap();
    assert_eq!(back, tags);
}

/// The serialized payload of §10.6, octet for octet.
const SPEC_SUBSCRIPTION: [u8; 84] = [
    0x00, 0x03, 0x00, 0x00, 0x5a, 0x00, 0x10, 0x00, 0xc0, 0xa8, 0x02, 0x05, 0x00, 0x00, 0x3a, 0x20,
    0x00, 0x00, 0x00, 0x02, 0x80, 0x00, 0x00, 0x07, 0x05, 0x00, 0x0c, 0x00, 0x07, 0x00, 0x00, 0x00,
    b'S', b'q', b'u', b'a', b'r', b'e', 0x00, 0x00, 0x07, 0x00, 0x10, 0x00, 0x0a, 0x00, 0x00, 0x00,
    b'S', b'h', b'a', b'p', b'e', b'T', b'y', b'p', b'e', 0x00, 0x00, 0x00, 0x25, 0x00, 0x04, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x23, 0x00, 0x08, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00,
];

#[test]
fn rtps_10_6_subscription_with_a_content_filter_survives_re_encoding() {
    let mut sub = SubscriptionBuiltinTopicData::from_pl_cdr_le(&SPEC_SUBSCRIPTION).unwrap();
    assert_eq!(
        (sub.topic_name.as_str(), sub.type_name.as_str()),
        ("Square", "ShapeType")
    );
    sub.content_filter = Some(ContentFilterProperty {
        content_filtered_topic_name: "SquareFiltered".into(),
        related_topic_name: "Square".into(),
        filter_class_name: filter_class::DDSSQL.into(),
        filter_expression: "x > %0".into(),
        expression_parameters: vec!["10".into()],
    });
    let bytes = sub.to_pl_cdr_le().unwrap();
    let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
    assert!(
        pl.find(pid::CONTENT_FILTER_PROPERTY).is_some(),
        "PID 0x0035"
    );
    let again = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(again, sub);
}

#[test]
fn rtps_9_6_2_2_publication_and_topic_data_survive_re_encoding() {
    assert_eq!(
        STRING_256_MAX, 256,
        "string<256> of PID_TOPIC_NAME / PID_TYPE_NAME"
    );
    let mut pl = ParameterList::new();
    let key = Guid::new(prefix(40), EntityId::user_writer_with_key([0, 0, 3]));
    pl.push(Parameter::new(pid::ENDPOINT_GUID, key.to_bytes().to_vec()));
    pl.push(Parameter::new(
        pid::TOPIC_NAME,
        vec![7, 0, 0, 0, b'S', b'q', b'u', b'a', b'r', b'e', 0, 0],
    ));
    pl.push(Parameter::new(
        pid::TYPE_NAME,
        vec![2, 0, 0, 0, b'T', 0, 0, 0],
    ));
    // §9.3.2.1: RELIABLE = 2.
    pl.push(Parameter::new(
        pid::RELIABILITY,
        vec![2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ));
    let mut bytes = vec![0x00, 0x03, 0x00, 0x00];
    bytes.extend(pl.to_bytes(true));
    let publication = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(publication.key, key);
    assert_eq!(publication.reliability.kind, ReliabilityKind::Reliable);
    let again =
        PublicationBuiltinTopicData::from_pl_cdr_le(&publication.to_pl_cdr_le().unwrap()).unwrap();
    assert_eq!(again, publication);

    let topic = TopicBuiltinTopicData::new(key, "Square", "T");
    let back =
        TopicBuiltinTopicData::from_pl_cdr(&topic.to_pl_cdr_le().unwrap(), VendorId::ZERODDS)
            .unwrap();
    assert_eq!(
        (back.key, back.topic_name.as_str(), back.type_name.as_str()),
        (key, "Square", "T")
    );
}

#[test]
fn xtypes_7_6_3_1_2_data_representation_is_negotiated_on_the_writers_first_element() {
    assert_eq!((XCDR, XCDR2), (0, 2));
    // §7.6.3.1.1: the default is [XCDR]; an empty list means the same.
    assert_eq!(DEFAULT_OFFER, [XCDR]);
    assert_eq!(DataRepMatchMode::default(), DataRepMatchMode::Strict);
    use DataRepMatchMode::{Strict, Tolerant};
    assert_eq!(dr::negotiate(&[XCDR2], &[], Strict), None);
    assert_eq!(dr::negotiate(&[], &[], Strict), Some(XCDR));
    assert_eq!(
        dr::negotiate(&[XCDR, XCDR2], &[XCDR2, XCDR], Strict),
        Some(XCDR)
    );
    assert_eq!(dr::negotiate(&[XCDR2, XCDR], &[XCDR], Strict), None);
    assert_eq!(dr::negotiate(&[XCDR2, XCDR], &[XCDR], Tolerant), Some(XCDR));
}

// ---------------------------------------------------------------------
// Reliable endpoints
// ---------------------------------------------------------------------

#[test]
fn rtps_8_4_7_1_and_8_4_10_1_default_timing_values() {
    assert_eq!(DEFAULT_NACK_RESPONSE_DELAY, StdDuration::from_millis(200));
    assert_eq!(DEFAULT_NACK_SUPPRESSION_DURATION, StdDuration::ZERO);
    assert_eq!(
        DEFAULT_HEARTBEAT_RESPONSE_DELAY,
        StdDuration::from_millis(500)
    );
    assert!(DEFAULT_HEARTBEAT_PERIOD > StdDuration::ZERO);
    const { assert!(DEFAULT_UNACKED_WINDOW > 0) };
}

#[test]
fn rtps_9_4_5_5_fragment_sizes_fit_the_message_and_a_ushort() {
    let size = fragment_size_for_message_size(DEFAULT_MTU);
    assert!(size as usize <= DEFAULT_MTU);
    assert!(size <= u32::from(u16::MAX), "fragmentSize is a ushort");
    assert_eq!(
        fragment_size_for_message_size(usize::MAX),
        u32::from(u16::MAX)
    );
    assert_eq!(fragment_size_for_message_size(0), 1);
    assert!(fragment_size_for_message_size(LOOPBACK_MTU) as usize <= LOOPBACK_MTU);
    // One IPv4/UDP datagram.
    const { assert!(LOOPBACK_MTU <= 65_535 - 28) };
}

fn writer() -> ReliableWriter {
    ReliableWriter::new(ReliableWriterConfig {
        guid: writer_guid(),
        vendor_id: VendorId::ZERODDS,
        reader_proxies: vec![ReaderProxy::new(
            reader_guid(),
            vec![Locator::udp_v4(LOOPBACK, 7411)],
            vec![],
            true,
        )],
        max_samples: 64,
        history_kind: HistoryKind::KeepAll,
        heartbeat_period: DEFAULT_HEARTBEAT_PERIOD,
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    })
}

/// Hands every DATA of `out` to `r` through `take`.
fn deliver(
    out: &[OutboundDatagram],
    r: &mut ReliableReader,
    take: impl Fn(GuidPrefix, &DataSubmessage, &mut ReliableReader) -> Vec<DeliveredSample>,
) -> Vec<DeliveredSample> {
    let mut delivered = Vec::new();
    for dg in out {
        let parsed = decode_datagram(&dg.bytes).unwrap();
        for (state, sub) in
            parsed.receiver_walk_in(reader_guid().prefix, &ReceiveContext::default())
        {
            if let ParsedSubmessage::Data(d) = sub {
                delivered.extend(take(state.source_guid_prefix, d, r));
            }
        }
    }
    delivered
}

fn plain(source: GuidPrefix, d: &DataSubmessage, r: &mut ReliableReader) -> Vec<DeliveredSample> {
    r.handle_data(source, d, None)
}

fn opened_by<O: PayloadOpener>(
    opener: &O,
) -> impl Fn(GuidPrefix, &DataSubmessage, &mut ReliableReader) -> Vec<DeliveredSample> + '_ {
    move |source, d, r| {
        let mut out = Vec::new();
        r.handle_data_opening(source, d, None, &mut out, Some(opener));
        out
    }
}

#[test]
fn rtps_8_4_a_written_sample_reaches_the_matched_reader() {
    let mut w = writer();
    let mut r = reader();
    let out = w.write(b"hello!!!").unwrap();
    assert_eq!(
        out[0].targets.as_slice(),
        &[Locator::udp_v4(LOOPBACK, 7411)]
    );
    let delivered = deliver(&out, &mut r, plain);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].writer_guid, writer_guid());
    assert_eq!(delivered[0].sequence_number, SequenceNumber(1));
    assert_eq!(delivered[0].kind, ChangeKind::Alive);
    assert_eq!(&*delivered[0].payload, b"hello!!!");
}

/// Opens a payload by reversing it, refuses one that starts with 0xff.
struct Reverse;

impl PayloadOpener for Reverse {
    fn open(
        &self,
        _writer: Guid,
        _original: Option<Guid>,
        payload: &Payload,
        _marked: bool,
    ) -> PayloadOpening {
        if payload.first() == Some(&0xff) {
            return PayloadOpening::Refused;
        }
        let mut clear = payload.to_vec();
        clear.reverse();
        PayloadOpening::Opened(Payload::from(clear))
    }
}

#[test]
fn a_payload_opener_decides_what_the_reader_delivers() {
    let mut w = writer();
    let mut r = reader();
    let opened = deliver(
        &w.write(&[1, 2, 3, 4]).unwrap(),
        &mut r,
        opened_by(&Reverse),
    );
    assert_eq!(&*opened[0].payload, &[4, 3, 2, 1]);
    let refused = deliver(
        &w.write(&[0xff, 0, 0, 0]).unwrap(),
        &mut r,
        opened_by(&Reverse),
    );
    assert!(refused.is_empty());
    let mut r2 = reader();
    let mut w2 = writer();
    let as_is = deliver(&w2.write(&[5, 6, 7, 8]).unwrap(), &mut r2, opened_by(&AsIs));
    assert_eq!(&*as_is[0].payload, &[5, 6, 7, 8]);
}

// ---------------------------------------------------------------------
// GROUP access scope
// ---------------------------------------------------------------------

#[test]
fn rtps_9_6_4_3_group_numbers_and_the_writer_set_digest() {
    let writers = [
        EntityId::user_writer_with_key([0, 0, 1]),
        EntityId::user_writer_with_key([0, 1, 0]),
    ];
    // §9.3.2.5: MD5 of the CDR big-endian EntityIdSet_t, ids sorted as
    // little-endian int32 ({00 01 00 02, 00 00 01 02}); computed with
    // Python's hashlib, leading four octets.
    assert_eq!(
        GroupDigest::of_entities(&writers),
        GroupDigest([0xd4, 0xfa, 0xc0, 0x30])
    );
    assert_eq!(
        GroupDigest::of_entities(&[writers[1], writers[0], writers[0]]),
        GroupDigest::of_entities(&writers)
    );
    assert_eq!(
        GroupDigest::of_entities(&[]),
        GroupDigest([0; 4]),
        "the empty group is zero, not a hash"
    );
    let group = WriterGroup::new();
    group.set_writers(&writers);
    for _ in 0..10 {
        let _ = group.stamp_change();
    }
    group.begin_coherent_set();
    let changes = [
        group.stamp_change(),
        group.stamp_change(),
        group.stamp_change(),
    ];
    assert_eq!(changes.map(|c| c.gsn), [11, 12, 13]);
    assert!(changes.iter().all(|c| c.coherent_set_gsn == Some(11)));
    assert!(
        changes
            .iter()
            .all(|c| c.writer_set == GroupDigest::of_entities(&writers))
    );
    let end = group.end_coherent_set().unwrap();
    assert_eq!((end.first_gsn, end.ecs_gsn), (11, 14), "Table 9.23");
}

// ---------------------------------------------------------------------
// Dialect behaviour table
// ---------------------------------------------------------------------

#[test]
fn the_behaviour_table_answers_per_peer_and_falls_back_to_its_default() {
    let default = RtpsDialect::MaximallyInteroperable.behaviour();
    let mut table = BehaviourTable::new(default);
    let peer = Guid::new(prefix(50), EntityId::user_reader_with_key([0, 0, 9]));
    assert_eq!(table.for_peer(peer), default);
    let narrowed = RtpsBehaviour::new()
        .with_group_coherent_set(GroupCoherentSetPid::Omitted)
        .with_heartbeat_group_digests(HeartbeatGroupDigests::Omitted)
        .with_group_wire_form(GroupWireForm::CoherentSetsOnly)
        .with_spdp_response_targets(SpdpResponseTargets::AnnouncedLocatorsOnly);
    table.set_for_peer(peer, narrowed);
    assert_eq!(table.for_peer(peer), narrowed);
    assert_eq!(table.for_peer(writer_guid()), default);
    assert_eq!(RtpsDialect::parse("cyclone"), Some(RtpsDialect::Cyclone));
    assert_eq!(
        RtpsDialect::parse("cyclon"),
        None,
        "a typo is loud, not the default"
    );
}

// ---------------------------------------------------------------------
// Security info
// ---------------------------------------------------------------------

#[test]
fn security_9_4_2_8_endpoint_security_attribute_bits() {
    assert_eq!(attrs::IS_READ_PROTECTED, 1 << 0);
    assert_eq!(attrs::IS_WRITE_PROTECTED, 1 << 1);
    assert_eq!(attrs::IS_SUBMESSAGE_PROTECTED, 1 << 3);
    assert_eq!(attrs::IS_PAYLOAD_PROTECTED, 1 << 4);
    assert_eq!(attrs::IS_VALID, 1 << 31);
    let info = EndpointSecurityInfo {
        endpoint_security_attributes: attrs::IS_VALID | attrs::IS_PAYLOAD_PROTECTED,
        plugin_endpoint_security_attributes: 0x8000_0002,
    };
    let bytes = info.to_bytes(true);
    assert_eq!(
        bytes[..4],
        (attrs::IS_VALID | attrs::IS_PAYLOAD_PROTECTED).to_le_bytes()
    );
    assert_eq!(
        EndpointSecurityInfo::from_bytes(&bytes, true).unwrap(),
        info
    );
    assert!(
        info.is_compatible_with(None),
        "§7.3.24: absent is the default, compatible"
    );
}

#[test]
fn security_7_3_11_to_7_3_15_algorithm_info_defaults() {
    // §8.2: RSASSA_PSS 1<<0, RSASSA_PKCS1 1<<1, ECDSA_P256 1<<2.
    let sig = ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default();
    assert_eq!(
        sig.trust_chain,
        AlgorithmRequirements {
            supported: 0b111,
            required: 0b100
        }
    );
    assert_eq!(
        sig.message_auth,
        AlgorithmRequirements {
            supported: 0b101,
            required: 0b100
        }
    );
    // §8.3: DHE_MODP_2048_256 1<<0, ECDHE_CEUM_P256 1<<1.
    let kx = ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default();
    assert_eq!(
        kx.shared_secret,
        AlgorithmRequirements {
            supported: 0b11,
            required: 0b10
        }
    );
    // §8.1: AES128_GCM 1<<0, AES256_GCM 1<<1.
    let sym = ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default();
    assert_eq!(
        (
            sym.supported_mask,
            sym.builtin_endpoints_required_mask,
            sym.builtin_kx_endpoints_required_mask,
            sym.user_endpoints_default_required_mask
        ),
        (0b11, 0b10, 0b10, 0b10)
    );
    // §7.3.15.2: an endpoint without the info takes the participant's
    // user-endpoint default.
    let endpoint = EndpointSecuritySymmetricCipherAlgorithmInfo::spec_default(&sym);
    assert_eq!(
        endpoint.required_mask,
        sym.user_endpoints_default_required_mask
    );
    assert!(sig.compatible_with(&ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default()));
}

// ---------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------

#[test]
fn rtps_9_3_2_1_time_sentinels_and_the_dcps_conversion() {
    assert_eq!(
        (
            HeTimestamp::TIME_INVALID.seconds,
            HeTimestamp::TIME_INVALID.fraction
        ),
        (u32::MAX, u32::MAX)
    );
    assert_eq!(
        (
            HeTimestamp::TIME_INFINITE.seconds,
            HeTimestamp::TIME_INFINITE.fraction
        ),
        (u32::MAX, 0xffff_fffe)
    );
    use zerodds_dcps::time::{Time, he_timestamp_to_time, time_to_he_timestamp};
    let ts = time_to_he_timestamp(Time::new(7, 250_000_000));
    assert_eq!(
        (ts.seconds, ts.fraction),
        (7, 1 << 30),
        "a quarter second is 2^30 / 2^32"
    );
    assert_eq!(he_timestamp_to_time(ts), Time::new(7, 250_000_000));
    assert_eq!(
        time_to_he_timestamp(Time::INVALID),
        HeTimestamp::TIME_INVALID
    );
    assert_eq!(Duration::from_sec_nanosec(1, 500_000_000).fraction, 1 << 31);
}
