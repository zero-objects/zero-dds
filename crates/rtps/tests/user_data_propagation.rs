//! WP QoS-Wiring T5 — UserData/TopicData/GroupData Roundtrip in
//! Pub/Sub/Participant BuiltinTopicData.
//!
//! Spec DDS 1.4 §2.2.3.1-3: opaque sequence<octet> per
//! User/Topic/Group data; discovery propagates it via PIDs
//! 0x002c/0x002e/0x002d.

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

extern crate alloc;

use zerodds_qos::DurabilityKind;
use zerodds_qos::ReliabilityKind;
use zerodds_qos::ReliabilityQosPolicy;
use zerodds_rtps::participant_data::{
    Duration as RtpsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::publication_data::{PublicationBuiltinTopicData, ReliabilityQos};
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, ProtocolVersion, VendorId};

#[test]
fn publication_user_data_roundtrip() {
    let orig = PublicationBuiltinTopicData {
        key: Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0xAA; 3]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "X".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQos {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: RtpsDuration::from_secs(1),
        },
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        lifespan: zerodds_qos::LifespanQosPolicy::default(),
        partition: vec![],
        user_data: vec![0x01, 0x02, 0x03, 0xFF],
        topic_data: vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE],
        group_data: vec![0x10],
        type_information: None,
        data_representation: vec![],
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
    };

    let bytes = orig.to_pl_cdr_le().expect("encode");
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decode");
    assert_eq!(decoded.user_data, orig.user_data);
    assert_eq!(decoded.topic_data, orig.topic_data);
    assert_eq!(decoded.group_data, orig.group_data);
}

#[test]
fn publication_empty_user_data_omits_pid() {
    // The default (empty Vec) is NOT written as a parameter — if
    // we leave everything empty, the decoder should see the same defaults.
    let orig = PublicationBuiltinTopicData {
        key: Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_writer_with_key([0xBB; 3]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "X".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQos::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        lifespan: zerodds_qos::LifespanQosPolicy::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_information: None,
        data_representation: vec![],
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
    };

    let bytes = orig.to_pl_cdr_le().expect("encode");
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decode");
    assert!(decoded.user_data.is_empty());
    assert!(decoded.topic_data.is_empty());
    assert!(decoded.group_data.is_empty());
}

#[test]
fn subscription_user_data_roundtrip() {
    let orig = SubscriptionBuiltinTopicData {
        key: Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0xCC; 3]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes([3; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "X".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQosPolicy::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        partition: vec![],
        user_data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        topic_data: vec![0xCA, 0xFE],
        group_data: vec![0xBA, 0xBE],
        type_information: None,
        data_representation: vec![],
        content_filter: None,
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
    };

    let bytes = orig.to_pl_cdr_le().expect("encode");
    let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decode");
    assert_eq!(decoded.user_data, orig.user_data);
    assert_eq!(decoded.topic_data, orig.topic_data);
    assert_eq!(decoded.group_data, orig.group_data);
}

#[test]
fn participant_user_data_roundtrip() {
    let orig = ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes([4; 12]), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        participant_security_info: None,
        default_unicast_locator: None,
        default_multicast_locator: None,
        metatraffic_unicast_locator: None,
        metatraffic_multicast_locator: None,
        domain_id: Some(7),
        builtin_endpoint_set: endpoint_flag::PARTICIPANT_ANNOUNCER,
        lease_duration: RtpsDuration::from_secs(100),
        user_data: vec![0x42, 0x43, 0x44],
        properties: Default::default(),
        identity_token: None,
        permissions_token: None,
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
    };

    let bytes = orig.to_pl_cdr_le();
    let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decode");
    assert_eq!(decoded.user_data, orig.user_data);
}

#[test]
fn user_data_large_payload_32kib() {
    // 32 KiB payload — stays under the u16 PID-length limit (wire
    // Parameter-Length ist u16 = max 64 KiB inkl. Header).
    let payload: Vec<u8> = (0..32 * 1024).map(|i| (i & 0xFF) as u8).collect();
    let orig = ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes([5; 12]), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        participant_security_info: None,
        default_unicast_locator: None,
        default_multicast_locator: None,
        metatraffic_unicast_locator: None,
        metatraffic_multicast_locator: None,
        domain_id: Some(7),
        builtin_endpoint_set: 0,
        lease_duration: RtpsDuration::from_secs(100),
        user_data: payload.clone(),
        properties: Default::default(),
        identity_token: None,
        permissions_token: None,
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
    };

    let bytes = orig.to_pl_cdr_le();
    let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decode");
    assert_eq!(decoded.user_data.len(), 32 * 1024);
    assert_eq!(decoded.user_data, payload);
}
