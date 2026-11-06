// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-discovery` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps`, from its non-test sources
//! (`spdp.rs`, `capabilities.rs`, `sedp/{cache,reader,writer,stack}.rs`,
//! `security/{stack,stateless,volatile_secure}.rs`, `type_lookup/`):
//! - `wire_types`: `Guid` (`new`, `from_bytes`, `to_bytes`), `GuidPrefix`
//!   (`UNKNOWN`, `from_bytes`), the builtin `EntityId`s of SPDP, SEDP,
//!   participant message, DDS-Security and TypeLookup, `EntityId::PARTICIPANT`
//!   and `UNKNOWN`, `VendorId` (`ZERODDS`, `OPENDDS`, `FASTDDS`, `CYCLONE`),
//!   `ProtocolVersion`, `SequenceNumber`, `Locator`.
//! - `header::RtpsHeader::new`, `datagram::{encode_data_datagram,
//!   decode_datagram, ReceiveContext, ParsedSubmessage}` and
//!   `ParsedDatagram::receiver_walk_in`, which attributes each submessage to
//!   the source and destination INFO_SRC / INFO_DST set.
//! - `submessages::{DataSubmessage, DataFragSubmessage, HeartbeatSubmessage,
//!   GapSubmessage, NackFragSubmessage, InfoSourceSubmessage,
//!   SequenceNumberSet}`.
//! - `parameter_list::{ParameterList, Parameter, pid, sedp_endpoint_key_pl_cdr_le,
//!   spdp_participant_key_pl_cdr_le}`; `inline_qos::{status_info,
//!   status_info_param, lifecycle_inline_qos, find_key_hash}`.
//! - `participant_data::{ParticipantBuiltinTopicData, Duration, endpoint_flag}`,
//!   `publication_data::{PublicationBuiltinTopicData, DurabilityKind,
//!   ReliabilityKind, ReliabilityQos, PresentationQosPolicy,
//!   inject_pid_shm_locator}`, `subscription_data::SubscriptionBuiltinTopicData`,
//!   `topic_data::{TopicBuiltinTopicData, topic_key, PID_CYCLONE_TOPIC_GUID}`.
//! - `reliable_writer::{ReliableWriter, ReliableWriterConfig,
//!   DEFAULT_FRAGMENT_SIZE}`, `reliable_reader::{ReliableReader,
//!   ReliableReaderConfig, DeliveredSample, DEFAULT_HEARTBEAT_RESPONSE_DELAY}`,
//!   `reader_proxy::ReaderProxy`, `writer_proxy::WriterProxy`,
//!   `history_cache::HistoryKind`, `message_builder::{OutboundDatagram,
//!   DEFAULT_MTU}`, `fragment_assembler::{FragmentAssembler, AssemblerCaps}`.
//! - `dialect::{RtpsBehaviour, RtpsDialect, DirectedAddressing}`: the secure
//!   writers force `PrefixEveryDatagram`.
//! - `error::WireError::ValueOutOfRange`.
//!
//! Oracles: DDSI-RTPS 2.5 (§8.3.8.8, §8.3.8.10, Table 8.17, §9.3.1.3
//! Table 9.2, §9.3.2.1, §9.3.2.12, §9.4.2.11, §9.4.4, §9.4.5.1, §9.4.5.4,
//! §9.4.5.7, §9.4.5.9, §9.4.5.11, §9.6.4.8, §9.6.4.9, §10.6 with its byte
//! example), DDS-Security 1.2 §7.4.7.1 Table 9 and §7.5.1.4, DDS-XTypes 1.3
//! §7.6.3.3.3 and §7.6.3.3.4, the OMG vendor id list, Cyclone DDS 0.10.5
//! `q_protocol.h` (`PID_CYCLONE_TOPIC_GUID = 0x8000 | 0x1b`), RFC 791/768 for
//! the UDP payload of a 1500-octet MTU. No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use core::time::Duration as StdDuration;

use zerodds_rtps::datagram::{
    ParsedSubmessage, ReceiveContext, decode_datagram, encode_data_datagram,
};
use zerodds_rtps::dialect::{DirectedAddressing, RtpsBehaviour, RtpsDialect};
use zerodds_rtps::error::WireError;
use zerodds_rtps::fragment_assembler::{AssemblerCaps, FragmentAssembler};
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::inline_qos::{
    find_key_hash, lifecycle_inline_qos, status_info, status_info_param,
};
use zerodds_rtps::message_builder::{DEFAULT_MTU, OutboundDatagram};
use zerodds_rtps::parameter_list::{
    Parameter, ParameterList, pid, sedp_endpoint_key_pl_cdr_le, spdp_participant_key_pl_cdr_le,
};
use zerodds_rtps::participant_data::{Duration, ParticipantBuiltinTopicData, endpoint_flag};
use zerodds_rtps::publication_data::{
    DurabilityKind, PresentationQosPolicy, PublicationBuiltinTopicData, ReliabilityKind,
    inject_pid_shm_locator,
};
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_reader::{
    DEFAULT_HEARTBEAT_RESPONSE_DELAY, DeliveredSample, ReliableReader, ReliableReaderConfig,
};
use zerodds_rtps::reliable_writer::{DEFAULT_FRAGMENT_SIZE, ReliableWriter, ReliableWriterConfig};
use zerodds_rtps::submessages::DataSubmessage;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::topic_data::{PID_CYCLONE_TOPIC_GUID, TopicBuiltinTopicData, topic_key};
use zerodds_rtps::wire_types::{
    EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, SequenceNumber, VendorId,
};
use zerodds_rtps::writer_proxy::WriterProxy;

fn prefix(tag: u8) -> GuidPrefix {
    GuidPrefix::from_bytes([0x01, 0xF0, tag, tag, tag, tag, 0, 0, 0, 0, 0, tag])
}

fn eid_bytes(id: EntityId) -> [u8; 4] {
    id.to_bytes()
}

// ---------------------------------------------------------------------
// GUIDs and entity ids
// ---------------------------------------------------------------------

#[test]
fn rtps_9_3_1_3_builtin_entity_ids_follow_table_9_2() {
    for (id, bytes) in [
        (EntityId::PARTICIPANT, [0x00, 0x00, 0x01, 0xc1]),
        (
            EntityId::SEDP_BUILTIN_TOPICS_WRITER,
            [0x00, 0x00, 0x02, 0xc2],
        ),
        (
            EntityId::SEDP_BUILTIN_TOPICS_READER,
            [0x00, 0x00, 0x02, 0xc7],
        ),
        (
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            [0x00, 0x00, 0x03, 0xc2],
        ),
        (
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            [0x00, 0x00, 0x03, 0xc7],
        ),
        (
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
            [0x00, 0x00, 0x04, 0xc2],
        ),
        (
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            [0x00, 0x00, 0x04, 0xc7],
        ),
        (
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
            [0x00, 0x01, 0x00, 0xc2],
        ),
        (
            EntityId::SPDP_BUILTIN_PARTICIPANT_READER,
            [0x00, 0x01, 0x00, 0xc7],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            [0x00, 0x02, 0x00, 0xc2],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER,
            [0x00, 0x02, 0x00, 0xc7],
        ),
    ] {
        assert_eq!(eid_bytes(id), bytes, "{id:?}");
        assert_eq!(EntityId::from_bytes(bytes), id);
    }
    // §9.3.1.2: ENTITYID_UNKNOWN = {{00,00,00},00}.
    assert_eq!(eid_bytes(EntityId::UNKNOWN), [0; 4]);
}

#[test]
fn security_7_4_7_1_secure_builtin_entity_ids_follow_table_9() {
    for (id, bytes) in [
        (
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
            [0xff, 0x00, 0x03, 0xc2],
        ),
        (
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            [0xff, 0x00, 0x03, 0xc7],
        ),
        (
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
            [0xff, 0x00, 0x04, 0xc2],
        ),
        (
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
            [0xff, 0x00, 0x04, 0xc7],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
            [0x00, 0x02, 0x01, 0xc3],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            [0x00, 0x02, 0x01, 0xc4],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            [0xff, 0x02, 0x02, 0xc3],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            [0xff, 0x02, 0x02, 0xc4],
        ),
        (
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            [0xff, 0x01, 0x01, 0xc2],
        ),
        (
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
            [0xff, 0x01, 0x01, 0xc7],
        ),
    ] {
        assert_eq!(eid_bytes(id), bytes, "{id:?}");
    }
}

#[test]
fn xtypes_7_6_3_3_3_type_lookup_entity_ids() {
    assert_eq!(
        eid_bytes(EntityId::TL_SVC_REQ_WRITER),
        [0x00, 0x03, 0x00, 0xc3]
    );
    assert_eq!(
        eid_bytes(EntityId::TL_SVC_REQ_READER),
        [0x00, 0x03, 0x00, 0xc4]
    );
    assert_eq!(
        eid_bytes(EntityId::TL_SVC_REPLY_WRITER),
        [0x00, 0x03, 0x01, 0xc3]
    );
    assert_eq!(
        eid_bytes(EntityId::TL_SVC_REPLY_READER),
        [0x00, 0x03, 0x01, 0xc4]
    );
}

#[test]
fn rtps_9_3_2_12_builtin_endpoint_set_bits() {
    use endpoint_flag as f;
    for (bit, position) in [
        (f::PARTICIPANT_ANNOUNCER, 0),
        (f::PARTICIPANT_DETECTOR, 1),
        (f::PUBLICATIONS_ANNOUNCER, 2),
        (f::PUBLICATIONS_DETECTOR, 3),
        (f::SUBSCRIPTIONS_ANNOUNCER, 4),
        (f::SUBSCRIPTIONS_DETECTOR, 5),
        (f::PARTICIPANT_MESSAGE_DATA_WRITER, 10),
        (f::PARTICIPANT_MESSAGE_DATA_READER, 11),
        // DDS-XTypes 1.3 §7.6.3.3.4.
        (f::TYPE_LOOKUP_REQUEST_DATA_WRITER, 12),
        (f::TYPE_LOOKUP_REQUEST_DATA_READER, 13),
        (f::TYPE_LOOKUP_REPLY_DATA_WRITER, 14),
        (f::TYPE_LOOKUP_REPLY_DATA_READER, 15),
        // DDS-Security 1.2 §7.5.1.4.
        (f::PUBLICATIONS_SECURE_WRITER, 16),
        (f::PUBLICATIONS_SECURE_READER, 17),
        (f::SUBSCRIPTIONS_SECURE_WRITER, 18),
        (f::SUBSCRIPTIONS_SECURE_READER, 19),
        (f::PARTICIPANT_MESSAGE_SECURE_WRITER, 20),
        (f::PARTICIPANT_MESSAGE_SECURE_READER, 21),
        (f::PARTICIPANT_STATELESS_MESSAGE_WRITER, 22),
        (f::PARTICIPANT_STATELESS_MESSAGE_READER, 23),
        (f::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER, 24),
        (f::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER, 25),
        (f::PARTICIPANT_SECURE_WRITER, 26),
        (f::PARTICIPANT_SECURE_READER, 27),
        (f::TOPICS_ANNOUNCER, 28),
        (f::TOPICS_DETECTOR, 29),
    ] {
        assert_eq!(bit, 1 << position);
    }
    assert_eq!(f::TYPE_LOOKUP_REQUEST, (1 << 12) | (1 << 13));
    assert_eq!(f::TYPE_LOOKUP_REPLY, (1 << 14) | (1 << 15));
    assert_eq!(f::ALL_SECURE, 0x0fff_0000, "positions 16-27");
    assert_eq!(f::ALL_STANDARD, 0x0000_fc3f, "positions 0-5 and 10-15");
}

#[test]
fn rtps_9_3_1_5_guid_is_prefix_then_entity_id() {
    let p = prefix(7);
    let g = Guid::new(p, EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER);
    let bytes = g.to_bytes();
    assert_eq!(bytes[..12], p.to_bytes());
    assert_eq!(bytes[12..], [0x00, 0x00, 0x03, 0xc2]);
    assert_eq!(Guid::from_bytes(bytes), g);
    // §9.3.1.1: GUIDPREFIX_UNKNOWN is twelve zero octets.
    assert_eq!(GuidPrefix::UNKNOWN.to_bytes(), [0; 12]);
}

#[test]
fn the_vendor_ids_discovery_tells_apart_are_the_omg_assigned_ones() {
    assert_eq!(VendorId::OPENDDS.to_bytes(), [0x01, 0x03]);
    assert_eq!(VendorId::FASTDDS.to_bytes(), [0x01, 0x0F]);
    assert_eq!(VendorId::CYCLONE.to_bytes(), [0x01, 0x10]);
    assert_eq!(VendorId::ZERODDS.to_bytes(), [0x01, 0xF0]);
}

// ---------------------------------------------------------------------
// Header, datagrams and the receiver walk
// ---------------------------------------------------------------------

#[test]
fn rtps_9_4_4_header_is_rtps_version_vendor_prefix() {
    let p = prefix(3);
    let header = RtpsHeader::new(VendorId::ZERODDS, p);
    assert_eq!(header.protocol_version, ProtocolVersion::V2_5);
    let bytes = header.to_bytes();
    assert_eq!(bytes[..4], *b"RTPS");
    assert_eq!(bytes[4..6], [2, 5]);
    assert_eq!(bytes[6..8], [0x01, 0xF0]);
    assert_eq!(bytes[8..], p.to_bytes());
    assert_eq!(RtpsHeader::from_bytes(&bytes).unwrap(), header);
    let mut bad = bytes;
    bad[0] = b'X';
    assert!(RtpsHeader::from_bytes(&bad).is_err());
    assert!(decode_datagram(&bad).is_err());
}

fn data(
    writer_id: EntityId,
    sn: i64,
    payload: &[u8],
    inline_qos: Option<ParameterList>,
) -> DataSubmessage {
    DataSubmessage {
        extra_flags: 0,
        reader_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
        writer_id,
        writer_sn: SequenceNumber(sn),
        inline_qos: inline_qos.map(Into::into),
        data_flag: true,
        key_flag: false,
        non_standard_flag: false,
        serialized_payload: payload.to_vec().into(),
    }
}

#[test]
fn rtps_9_4_5_4_data_submessage_layout_and_decode() {
    let header = RtpsHeader::new(VendorId::ZERODDS, prefix(1));
    let d = data(
        EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        5,
        &[0, 3, 0, 0, 1, 0, 0, 0],
        None,
    );
    let bytes = encode_data_datagram(header, core::slice::from_ref(&d)).unwrap();
    let sub = &bytes[20..];
    assert_eq!(sub[0], 0x15, "DATA");
    assert_eq!(sub[1], 0x01 | 0x04, "E and D, no Q");
    assert_eq!(u16::from_le_bytes([sub[2], sub[3]]) as usize, sub.len() - 4);
    assert_eq!(sub[4..6], [0, 0], "extraFlags");
    assert_eq!(
        u16::from_le_bytes([sub[6], sub[7]]),
        16,
        "octetsToInlineQos"
    );
    assert_eq!(sub[8..12], [0x00, 0x00, 0x03, 0xc7]);
    assert_eq!(sub[12..16], [0x00, 0x00, 0x03, 0xc2]);
    // SequenceNumber_t {long high; unsigned long low}, little-endian.
    assert_eq!(sub[16..20], 0_i32.to_le_bytes());
    assert_eq!(sub[20..24], 5_u32.to_le_bytes());
    assert_eq!(sub[24..], [0, 3, 0, 0, 1, 0, 0, 0]);

    let parsed = decode_datagram(&bytes).unwrap();
    assert_eq!(parsed.header, header);
    let decoded: Vec<_> = parsed.contents().collect();
    assert_eq!(decoded.len(), 1);
    match decoded[0] {
        ParsedSubmessage::Data(got) => assert_eq!(got, &d),
        other => panic!("expected DATA, got {other:?}"),
    }
}

#[test]
fn rtps_9_4_5_4_inline_qos_sets_the_q_flag() {
    let header = RtpsHeader::new(VendorId::ZERODDS, prefix(1));
    let qos = lifecycle_inline_qos([7; 16], status_info::DISPOSED);
    let mut d = data(
        EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        1,
        &[],
        Some(qos),
    );
    d.data_flag = false;
    let bytes = encode_data_datagram(header, core::slice::from_ref(&d)).unwrap();
    assert_eq!(bytes[21] & 0x02, 0x02, "Q");
    assert_eq!(bytes[21] & 0x04, 0, "no D");
    let parsed = decode_datagram(&bytes).unwrap();
    let Some(ParsedSubmessage::Data(got)) = parsed.contents().next() else {
        panic!("expected DATA");
    };
    let qos = got.inline_qos.as_ref().unwrap().to_list();
    assert_eq!(find_key_hash(&qos), Some([7; 16]));
    assert_eq!(qos.find(pid::STATUS_INFO).unwrap().value, [0, 0, 0, 1]);
}

/// A HEARTBEAT laid out by hand from §9.4.5.7, little-endian.
fn heartbeat_bytes(writer: EntityId, last: u32) -> Vec<u8> {
    let mut out = vec![0x07, 0x01 | 0x02, 28, 0];
    out.extend_from_slice(&EntityId::UNKNOWN.to_bytes());
    out.extend_from_slice(&writer.to_bytes());
    out.extend_from_slice(&0_i32.to_le_bytes());
    out.extend_from_slice(&1_u32.to_le_bytes());
    out.extend_from_slice(&0_i32.to_le_bytes());
    out.extend_from_slice(&last.to_le_bytes());
    out.extend_from_slice(&1_i32.to_le_bytes());
    out
}

#[test]
fn rtps_8_3_8_8_and_8_3_8_10_info_dst_and_info_src_steer_the_following_submessages() {
    let local = prefix(0x10);
    let other = prefix(0x20);
    let relayed = prefix(0x30);
    let mut bytes = RtpsHeader::new(VendorId::ZERODDS, prefix(0x40))
        .to_bytes()
        .to_vec();
    // 1: no INFO_* yet.
    bytes.extend(heartbeat_bytes(
        EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        1,
    ));
    // INFO_DST to another participant (§9.4.5.9).
    bytes.extend([0x0e, 0x01, 12, 0]);
    bytes.extend(other.to_bytes());
    bytes.extend(heartbeat_bytes(
        EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        2,
    ));
    // INFO_DST with GUIDPREFIX_UNKNOWN: back to the receiver itself.
    bytes.extend([0x0e, 0x01, 12, 0]);
    bytes.extend(GuidPrefix::UNKNOWN.to_bytes());
    // INFO_SRC (§9.4.5.11): unused, version, vendor, prefix.
    bytes.extend([0x0c, 0x01, 20, 0, 0, 0, 0, 0, 2, 5, 0x01, 0x10]);
    bytes.extend(relayed.to_bytes());
    bytes.extend(heartbeat_bytes(
        EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        3,
    ));

    let parsed = decode_datagram(&bytes).unwrap();
    let mut seen = Vec::new();
    for (state, sub) in parsed.receiver_walk_in(local, &ReceiveContext::default()) {
        if let ParsedSubmessage::Heartbeat(hb) = sub {
            seen.push((
                hb.last_sn.0,
                state.dest_guid_prefix,
                state.source_guid_prefix,
                state.source_vendor_id,
            ));
        }
    }
    assert_eq!(
        seen,
        [
            // Table 8.17: destGuidPrefix starts as the receiver's own prefix,
            // the source as the header's.
            (1, local, prefix(0x40), VendorId::ZERODDS),
            (2, other, prefix(0x40), VendorId::ZERODDS),
            (3, local, relayed, VendorId::CYCLONE),
        ]
    );
}

// ---------------------------------------------------------------------
// Parameter lists, keys and inline QoS
// ---------------------------------------------------------------------

const SPEC_GUID: [u8; 16] = [
    0xc0, 0xa8, 0x02, 0x05, 0x00, 0x00, 0x3a, 0x20, 0x00, 0x00, 0x00, 0x02, 0x80, 0x00, 0x00, 0x07,
];

#[test]
fn rtps_9_4_2_11_parameter_list_encoding_with_sentinel() {
    assert_eq!(
        (pid::SENTINEL, pid::ENDPOINT_GUID, pid::PARTICIPANT_GUID),
        (0x0001, 0x005a, 0x0050)
    );
    assert_eq!(
        (
            pid::TOPIC_NAME,
            pid::TYPE_NAME,
            pid::KEY_HASH,
            pid::STATUS_INFO
        ),
        (0x0005, 0x0007, 0x0070, 0x0071)
    );
    let mut pl = ParameterList::new();
    pl.push(Parameter::new(pid::ENDPOINT_GUID, SPEC_GUID.to_vec()));
    let bytes = pl.to_bytes(true);
    let mut expected = vec![0x5a, 0x00, 0x10, 0x00];
    expected.extend(SPEC_GUID);
    expected.extend([0x01, 0x00, 0x00, 0x00]);
    assert_eq!(bytes, expected);
    let back = ParameterList::from_bytes(&bytes, true).unwrap();
    assert_eq!(back.find(pid::ENDPOINT_GUID).unwrap().value, SPEC_GUID);
    assert!(back.find(pid::TOPIC_NAME).is_none());
}

#[test]
fn rtps_10_6_sedp_and_spdp_keys_are_pl_cdr_le_guid_and_sentinel() {
    // §10.6: identifier PL_CDR_LE {0x00,0x03}, options 0, PID_ENDPOINT_GUID.
    let mut expected = vec![0x00, 0x03, 0x00, 0x00, 0x5a, 0x00, 0x10, 0x00];
    expected.extend(SPEC_GUID);
    expected.extend([0x01, 0x00, 0x00, 0x00]);
    assert_eq!(sedp_endpoint_key_pl_cdr_le(SPEC_GUID), expected);
    let participant = spdp_participant_key_pl_cdr_le(SPEC_GUID);
    assert_eq!(
        participant[..8],
        [0x00, 0x03, 0x00, 0x00, 0x50, 0x00, 0x10, 0x00]
    );
    assert_eq!(participant[8..24], SPEC_GUID);
    assert_eq!(participant[24..], [0x01, 0x00, 0x00, 0x00]);
}

#[test]
fn rtps_9_6_4_9_status_info_is_carried_big_endian() {
    let p = status_info_param(status_info::DISPOSED | status_info::UNREGISTERED);
    assert_eq!(p.id, 0x0071);
    // The flags D and U are the two lowest bits of the last octet.
    assert_eq!(p.value, [0x00, 0x00, 0x00, 0x03]);
    assert_eq!((status_info::DISPOSED, status_info::UNREGISTERED), (1, 2));
}

#[test]
fn rtps_9_6_4_8_lifecycle_inline_qos_names_the_instance_by_key_hash() {
    let qos = lifecycle_inline_qos([0xab; 16], status_info::UNREGISTERED);
    assert_eq!(qos.find(pid::KEY_HASH).unwrap().value, [0xab; 16]);
    assert_eq!(qos.find(pid::STATUS_INFO).unwrap().value, [0, 0, 0, 2]);
    assert_eq!(find_key_hash(&qos), Some([0xab; 16]));
    let mut short = ParameterList::new();
    short.push(Parameter::new(pid::KEY_HASH, vec![1; 8]));
    assert_eq!(find_key_hash(&short), None, "a key hash is 16 octets");
}

#[test]
fn the_shm_locator_goes_in_front_of_the_sentinel_as_a_vendor_pid() {
    let mut bytes = sedp_endpoint_key_pl_cdr_le(SPEC_GUID);
    inject_pid_shm_locator(&mut bytes, &[1, 2, 3, 4, 5]).unwrap();
    assert_eq!(
        bytes[bytes.len() - 4..],
        [0x01, 0x00, 0x00, 0x00],
        "still ends with the sentinel"
    );
    let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
    let shm = pl.find(pid::SHM_LOCATOR).unwrap();
    assert_eq!(
        pid::SHM_LOCATOR & 0x8000,
        0x8000,
        "vendor-specific bit (§9.6.2.2.1)"
    );
    assert_eq!(shm.value[..5], [1, 2, 3, 4, 5]);
    assert_eq!(shm.value.len() % 4, 0, "padded to four octets (§9.4.2.11)");
    let mut no_sentinel = vec![0x00, 0x03, 0x00, 0x00];
    assert!(matches!(
        inject_pid_shm_locator(&mut no_sentinel, &[1]),
        Err(WireError::ValueOutOfRange { .. })
    ));
}

// ---------------------------------------------------------------------
// Builtin topic data
// ---------------------------------------------------------------------

/// The serialized payload of §10.6, octet for octet.
const SPEC_SUBSCRIPTION: [u8; 84] = [
    0x00, 0x03, 0x00, 0x00, // PL_CDR_LE
    0x5a, 0x00, 0x10, 0x00, // PID_ENDPOINT_GUID
    0xc0, 0xa8, 0x02, 0x05, 0x00, 0x00, 0x3a, 0x20, 0x00, 0x00, 0x00, 0x02, 0x80, 0x00, 0x00, 0x07,
    0x05, 0x00, 0x0c, 0x00, // PID_TOPIC_NAME
    0x07, 0x00, 0x00, 0x00, b'S', b'q', b'u', b'a', b'r', b'e', 0x00, 0x00, //
    0x07, 0x00, 0x10, 0x00, // PID_TYPE_NAME
    0x0a, 0x00, 0x00, 0x00, b'S', b'h', b'a', b'p', b'e', b'T', b'y', b'p', b'e', 0x00, 0x00, 0x00,
    0x25, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, // PID_DESTINATION_ORDER
    0x23, 0x00, 0x08, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // PID_DEADLINE
    0x01, 0x00, 0x00, 0x00, // PID_SENTINEL
];

#[test]
fn rtps_10_6_the_spec_subscription_example_decodes() {
    let sub = SubscriptionBuiltinTopicData::from_pl_cdr_le(&SPEC_SUBSCRIPTION).unwrap();
    assert_eq!(sub.key, Guid::from_bytes(SPEC_GUID));
    assert_eq!(sub.topic_name, "Square");
    assert_eq!(sub.type_name, "ShapeType");
    assert_eq!(
        sub.destination_order.kind,
        zerodds_qos::DestinationOrderKind::BySourceTimestamp
    );
    assert_eq!(sub.deadline.period, Duration::from_secs(3));
    // Lossless through the writer side of SEDP.
    let again = SubscriptionBuiltinTopicData::from_pl_cdr_le(&sub.to_pl_cdr_le().unwrap()).unwrap();
    assert_eq!(again, sub);
}

/// A publication laid out by hand: GUID, topic, type, RELIABLE with a
/// max_blocking_time of 1 s, TRANSIENT_LOCAL.
fn hand_publication() -> Vec<u8> {
    let mut pl = ParameterList::new();
    pl.push(Parameter::new(pid::ENDPOINT_GUID, SPEC_GUID.to_vec()));
    pl.push(Parameter::new(
        pid::TOPIC_NAME,
        vec![7, 0, 0, 0, b'S', b'q', b'u', b'a', b'r', b'e', 0, 0],
    ));
    pl.push(Parameter::new(
        pid::TYPE_NAME,
        vec![
            10, 0, 0, 0, b'S', b'h', b'a', b'p', b'e', b'T', b'y', b'p', b'e', 0, 0, 0,
        ],
    ));
    // §9.3.2.1: RELIABLE = 2; Duration_t {1 s, 0}.
    pl.push(Parameter::new(
        pid::RELIABILITY,
        vec![2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    ));
    // DDS: TRANSIENT_LOCAL_DURABILITY_QOS = 1.
    pl.push(Parameter::new(pid::DURABILITY, vec![1, 0, 0, 0]));
    let mut out = vec![0x00, 0x03, 0x00, 0x00];
    out.extend(pl.to_bytes(true));
    out
}

#[test]
fn rtps_9_6_2_2_a_hand_laid_publication_decodes_and_survives_re_encoding() {
    let publication = PublicationBuiltinTopicData::from_pl_cdr_le(&hand_publication()).unwrap();
    assert_eq!(publication.key, Guid::from_bytes(SPEC_GUID));
    assert_eq!(
        (
            publication.topic_name.as_str(),
            publication.type_name.as_str()
        ),
        ("Square", "ShapeType")
    );
    assert_eq!(publication.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(
        publication.reliability.max_blocking_time,
        Duration::from_secs(1)
    );
    assert_eq!(publication.durability, DurabilityKind::TransientLocal);
    assert_eq!(publication.presentation, PresentationQosPolicy::default());
    let again =
        PublicationBuiltinTopicData::from_pl_cdr_le(&publication.to_pl_cdr_le().unwrap()).unwrap();
    assert_eq!(again, publication);
    // A publication without its key is refused.
    let mut keyless = vec![0x00, 0x03, 0x00, 0x00];
    keyless.extend(ParameterList::new().to_bytes(true));
    assert!(PublicationBuiltinTopicData::from_pl_cdr_le(&keyless).is_err());
}

#[test]
fn rtps_8_5_3_a_hand_laid_participant_decodes_and_survives_re_encoding() {
    let guid = Guid::new(prefix(9), EntityId::PARTICIPANT);
    let mut pl = ParameterList::new();
    pl.push(Parameter::new(
        pid::PARTICIPANT_GUID,
        guid.to_bytes().to_vec(),
    ));
    pl.push(Parameter::new(pid::PROTOCOL_VERSION, vec![2, 5, 0, 0]));
    pl.push(Parameter::new(pid::VENDOR_ID, vec![0x01, 0x10, 0, 0]));
    let locator = Locator::udp_v4([10, 0, 0, 9], 7410);
    pl.push(Parameter::new(
        pid::METATRAFFIC_UNICAST_LOCATOR,
        locator.to_bytes_le().to_vec(),
    ));
    let set = endpoint_flag::PARTICIPANT_ANNOUNCER | endpoint_flag::PUBLICATIONS_DETECTOR;
    pl.push(Parameter::new(
        pid::BUILTIN_ENDPOINT_SET,
        set.to_le_bytes().to_vec(),
    ));
    pl.push(Parameter::new(
        pid::PARTICIPANT_LEASE_DURATION,
        vec![20, 0, 0, 0, 0, 0, 0, 0],
    ));
    let mut bytes = vec![0x00, 0x03, 0x00, 0x00];
    bytes.extend(pl.to_bytes(true));

    let participant = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(participant.guid, guid);
    assert_eq!(participant.protocol_version, ProtocolVersion::V2_5);
    assert_eq!(participant.vendor_id, VendorId::CYCLONE);
    assert_eq!(participant.metatraffic_unicast_locators, [locator]);
    assert_eq!(participant.builtin_endpoint_set, set);
    assert_eq!(participant.lease_duration, Duration::from_secs(20));
    let again = ParticipantBuiltinTopicData::from_pl_cdr_le(&participant.to_pl_cdr_le()).unwrap();
    assert_eq!(again, participant);
}

#[test]
fn rtps_9_3_2_1_duration_infinite_is_0x7fffffff_0xffffffff() {
    assert_eq!(
        (Duration::INFINITE.seconds, Duration::INFINITE.fraction),
        (0x7fff_ffff, 0xffff_ffff)
    );
    assert!(Duration::INFINITE.is_infinite());
    assert_eq!(Duration::from_secs(5).seconds, 5);
}

#[test]
fn topic_announcements_round_trip_and_take_the_cyclone_key_only_from_cyclone() {
    let key = Guid::new(prefix(5), EntityId::from_bytes([0, 0, 9, 0x0a]));
    let topic = TopicBuiltinTopicData::new(key, "Square", "ShapeType");
    let bytes = topic.to_pl_cdr_le().unwrap();
    let back = TopicBuiltinTopicData::from_pl_cdr(&bytes, VendorId::ZERODDS).unwrap();
    assert_eq!(
        (back.key, back.topic_name.as_str(), back.type_name.as_str()),
        (key, "Square", "ShapeType")
    );

    let mut standard = ParameterList::new();
    standard.push(Parameter::new(pid::ENDPOINT_GUID, key.to_bytes().to_vec()));
    assert_eq!(topic_key(&standard, VendorId::FASTDDS), Some(key));
    // Cyclone 0.10.5 q_protocol.h: PID_CYCLONE_TOPIC_GUID = 0x8000 | 0x1b.
    assert_eq!(PID_CYCLONE_TOPIC_GUID, 0x801b);
    let mut cyclone = ParameterList::new();
    cyclone.push(Parameter::new(
        PID_CYCLONE_TOPIC_GUID,
        key.to_bytes().to_vec(),
    ));
    assert_eq!(topic_key(&cyclone, VendorId::CYCLONE), Some(key));
    // §9.6.2.2.1: a vendor-specific PID means something only from its vendor.
    assert_eq!(topic_key(&cyclone, VendorId::FASTDDS), None);
}

// ---------------------------------------------------------------------
// The reliable endpoints SEDP and the secure channels are built from
// ---------------------------------------------------------------------

const READER_LOC: Locator = Locator {
    kind: zerodds_rtps::wire_types::LocatorKind::UdpV4,
    port: 7411,
    address: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 127, 0, 0, 1],
};

const WRITER_LOC: Locator = Locator {
    kind: zerodds_rtps::wire_types::LocatorKind::UdpV4,
    port: 7410,
    address: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 127, 0, 0, 1],
};

fn writer_guid() -> Guid {
    Guid::new(prefix(1), EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER)
}

fn reader_guid() -> Guid {
    Guid::new(prefix(2), EntityId::SEDP_BUILTIN_PUBLICATIONS_READER)
}

fn writer(history_kind: HistoryKind) -> ReliableWriter {
    ReliableWriter::new(ReliableWriterConfig {
        guid: writer_guid(),
        vendor_id: VendorId::ZERODDS,
        reader_proxies: vec![ReaderProxy::new(
            reader_guid(),
            vec![READER_LOC],
            vec![],
            true,
        )],
        max_samples: 64,
        history_kind,
        heartbeat_period: StdDuration::from_millis(100),
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    })
}

fn reader() -> ReliableReader {
    ReliableReader::new(ReliableReaderConfig {
        guid: reader_guid(),
        vendor_id: VendorId::ZERODDS,
        writer_proxies: vec![WriterProxy::new(
            writer_guid(),
            vec![WRITER_LOC],
            vec![],
            true,
        )],
        max_samples_per_proxy: 64,
        heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
        assembler_caps: AssemblerCaps::default(),
    })
}

/// Hands every submessage of writer datagrams to the reader.
fn to_reader(
    datagrams: &[OutboundDatagram],
    r: &mut ReliableReader,
    now: StdDuration,
) -> Vec<DeliveredSample> {
    let mut out = Vec::new();
    for dg in datagrams {
        let parsed = decode_datagram(&dg.bytes).unwrap();
        for (state, sub) in
            parsed.receiver_walk_in(reader_guid().prefix, &ReceiveContext::default())
        {
            let src = state.source_guid_prefix;
            match sub {
                ParsedSubmessage::Data(d) => out.extend(r.handle_data(src, d, None)),
                ParsedSubmessage::DataFrag(df) => {
                    out.extend(r.handle_data_frag(src, df, now, None))
                }
                ParsedSubmessage::Heartbeat(h) => out.extend(r.handle_heartbeat(src, h, now)),
                ParsedSubmessage::Gap(g) => out.extend(r.handle_gap(src, g)),
                _ => {}
            }
        }
    }
    out
}

/// Hands every ACKNACK / NACK_FRAG of reader datagrams to the writer.
fn to_writer(datagrams: &[OutboundDatagram], w: &mut ReliableWriter) -> usize {
    let mut acknacks = 0;
    for dg in datagrams {
        assert_eq!(
            dg.targets.as_slice(),
            &[WRITER_LOC],
            "replies go to the writer proxy's locator"
        );
        let parsed = decode_datagram(&dg.bytes).unwrap();
        for sub in parsed.contents() {
            match sub {
                ParsedSubmessage::AckNack(a) => {
                    acknacks += 1;
                    w.handle_acknack(
                        reader_guid(),
                        a.reader_sn_state.bitmap_base,
                        a.reader_sn_state.iter_set(),
                    );
                }
                ParsedSubmessage::NackFrag(nf) => w.handle_nackfrag(reader_guid(), nf),
                _ => {}
            }
        }
    }
    acknacks
}

#[test]
fn rtps_8_4_reliable_exchange_delivers_in_order_and_is_acknowledged() {
    let mut w = writer(HistoryKind::KeepAll);
    let mut r = reader();
    let mut delivered = Vec::new();
    for i in 1..=3_u8 {
        let out = w.write(&[i; 4]).unwrap();
        assert!(!out.is_empty());
        for dg in &out {
            assert_eq!(
                dg.targets.as_slice(),
                &[READER_LOC],
                "sent to the proxy's unicast locator"
            );
        }
        delivered.extend(to_reader(&out, &mut r, StdDuration::ZERO));
    }
    let sns: Vec<i64> = delivered.iter().map(|s| s.sequence_number.0).collect();
    assert_eq!(sns, [1, 2, 3]);
    assert!(delivered.iter().all(|s| s.writer_guid == writer_guid()));
    assert_eq!(delivered[2].payload.as_ref(), &[3; 4]);

    // HEARTBEAT → ACKNACK → the writer may drop what every reader has.
    let mut now = StdDuration::ZERO;
    let mut acked = false;
    for _ in 0..20 {
        now += StdDuration::from_millis(250);
        let hb = w.tick(now).unwrap();
        to_reader(&hb, &mut r, now);
        let replies = r.tick_outbound(now).unwrap();
        to_writer(&replies, &mut w);
        if w.remove_acknowledged() > 0 || w.cache().is_empty() {
            acked = true;
            break;
        }
    }
    assert!(acked, "the reader's ACKNACK releases the writer's samples");
    assert_eq!(w.reader_proxies().len(), 1);
    assert!(w.remove_reader_proxy(reader_guid()).is_some());
    assert!(r.remove_writer_proxy(writer_guid()).is_some());
    assert_eq!((w.reader_proxy_count(), r.writer_proxy_count()), (0, 0));
}

#[test]
fn rtps_8_4_a_lost_sample_is_repaired_from_the_acknack() {
    let mut w = writer(HistoryKind::KeepAll);
    let mut r = reader();
    let first = w.write(b"one!").unwrap();
    let _lost = w.write(b"two!").unwrap();
    let third = w.write(b"thr!").unwrap();
    let mut got: Vec<i64> = to_reader(&first, &mut r, StdDuration::ZERO)
        .iter()
        .map(|s| s.sequence_number.0)
        .collect();
    got.extend(
        to_reader(&third, &mut r, StdDuration::ZERO)
            .iter()
            .map(|s| s.sequence_number.0),
    );
    assert_eq!(got, [1], "3 waits behind the missing 2");
    let mut now = StdDuration::ZERO;
    for _ in 0..20 {
        now += StdDuration::from_millis(250);
        let hb = w.tick(now).unwrap();
        got.extend(
            to_reader(&hb, &mut r, now)
                .iter()
                .map(|s| s.sequence_number.0),
        );
        let replies = r.tick_outbound(now).unwrap();
        to_writer(&replies, &mut w);
        let resent = w.tick(now + StdDuration::from_millis(1)).unwrap();
        got.extend(
            to_reader(&resent, &mut r, now)
                .iter()
                .map(|s| s.sequence_number.0),
        );
        if got.len() == 3 {
            break;
        }
    }
    assert_eq!(got, [1, 2, 3]);
}

#[test]
fn keep_last_keeps_the_newest_samples_only() {
    let mut w = writer(HistoryKind::KeepLast { depth: 2 });
    for i in 0..5_u8 {
        let _ = w.write(&[i]).unwrap();
    }
    assert_eq!(w.cache().len(), 2);
    assert_eq!(w.last_written_sn(), SequenceNumber(5));
}

#[test]
fn rtps_8_3_8_3_a_sample_above_the_fragment_size_goes_out_as_data_frag_and_reassembles() {
    assert_eq!(DEFAULT_MTU, 1500 - 20 - 8, "IPv4 header 20, UDP header 8");
    let mut w = writer(HistoryKind::KeepAll);
    let payload: Vec<u8> = (0..(DEFAULT_FRAGMENT_SIZE as usize * 2 + 17))
        .map(|i| i as u8)
        .collect();
    let out = w.write(&payload).unwrap();
    let mut assembler = FragmentAssembler::new(AssemblerCaps::default());
    let mut frags = 0;
    let mut complete = None;
    for dg in &out {
        for sub in decode_datagram(&dg.bytes).unwrap().contents() {
            match sub {
                ParsedSubmessage::DataFrag(df) => {
                    frags += 1;
                    assert!(u32::from(df.fragment_size) <= DEFAULT_FRAGMENT_SIZE);
                    assert_eq!(df.sample_size as usize, payload.len());
                    if let Some(done) = assembler.insert(df) {
                        complete = Some(done);
                    }
                }
                ParsedSubmessage::Data(_) => {
                    panic!("a sample above the fragment size is not one DATA")
                }
                _ => {}
            }
        }
    }
    assert_eq!(frags, 3);
    let done = complete.expect("all fragments arrived");
    assert_eq!(done.sequence_number, SequenceNumber(1));
    assert_eq!(done.payload, payload);
    // The reader reassembles the same way.
    let mut r = reader();
    let delivered = to_reader(&out, &mut r, StdDuration::ZERO);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].payload.as_ref(), payload.as_slice());
}

#[test]
fn rtps_8_3_8_8_prefix_every_datagram_opens_each_datagram_with_info_dst() {
    let mut w = writer(HistoryKind::KeepAll);
    let secure = RtpsBehaviour {
        directed_addressing: DirectedAddressing::PrefixEveryDatagram,
        ..RtpsDialect::Cyclone.behaviour()
    };
    w.set_behaviour(secure);
    assert!(DirectedAddressing::PrefixEveryDatagram.emits_info_dst());
    assert!(!DirectedAddressing::ByEntityIdOnly.emits_info_dst());
    let out = w.write(b"sec!").unwrap();
    assert!(!out.is_empty());
    for dg in &out {
        let parsed = decode_datagram(&dg.bytes).unwrap();
        match parsed.contents().next() {
            Some(ParsedSubmessage::InfoDestination(dst)) => {
                assert_eq!(*dst, reader_guid().prefix.to_bytes())
            }
            other => panic!("expected INFO_DST first, got {other:?}"),
        }
    }
    // The default addressing does not.
    let mut plain = writer(HistoryKind::KeepAll);
    plain.set_behaviour(RtpsBehaviour::new());
    let out = plain.write(b"pln!").unwrap();
    assert!(!matches!(
        decode_datagram(&out[0].bytes).unwrap().contents().next(),
        Some(ParsedSubmessage::InfoDestination(_))
    ));
}

#[test]
fn rtps_8_3_8_4_a_gap_releases_what_waits_behind_it() {
    let mut w = writer(HistoryKind::KeepAll);
    let mut r = reader();
    let _skipped = w.write(b"skip").unwrap();
    let second = w.write(b"keep").unwrap();
    assert!(
        to_reader(&second, &mut r, StdDuration::ZERO).is_empty(),
        "2 waits for 1"
    );
    // GAP {gapStart = 1, gapList = {base 2, empty}} (§8.3.8.4): 1 is irrelevant.
    let gap = zerodds_rtps::submessages::GapSubmessage {
        reader_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
        writer_id: EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
        gap_start: SequenceNumber(1),
        gap_list: zerodds_rtps::submessages::SequenceNumberSet::from_missing(
            SequenceNumber(2),
            &[],
        ),
        group_info: None,
        filtered_count: None,
    };
    let released = r.handle_gap(writer_guid().prefix, &gap);
    let sns: Vec<i64> = released.iter().map(|s| s.sequence_number.0).collect();
    assert_eq!(sns, [2]);
    assert_eq!(released[0].payload.as_ref(), b"keep");
}

#[test]
fn the_proxies_keep_the_locators_and_reliability_they_were_given() {
    let multicast = Locator::udp_v4([239, 255, 0, 1], 7400);
    let rp = ReaderProxy::new(reader_guid(), vec![READER_LOC], vec![multicast], false);
    assert_eq!(rp.remote_reader_guid, reader_guid());
    assert_eq!(
        (
            rp.unicast_locators.as_slice(),
            rp.multicast_locators.as_slice()
        ),
        (&[READER_LOC][..], &[multicast][..])
    );
    assert!(!rp.is_reliable);
    let wp = WriterProxy::new(writer_guid(), vec![READER_LOC], vec![], true);
    assert_eq!(wp.remote_writer_guid, writer_guid());
    assert!(wp.is_reliable);
}

#[test]
fn wire_errors_name_what_was_out_of_range() {
    let error = WireError::ValueOutOfRange {
        message: "SEDP payload too large",
    };
    assert!(
        format!("{error}").contains("SEDP payload too large")
            || format!("{error:?}").contains("SEDP payload too large")
    );
}
