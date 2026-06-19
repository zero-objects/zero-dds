//! WP 3.X SEDP PID extension: roundtrip tests for the newly introduced
//! Wire-Felder in `PublicationBuiltinTopicData` +
//! `SubscriptionBuiltinTopicData`.
//!
//! Deckt ab:
//! * Deadline (PID 0x0023) — 8 Byte Duration.
//! * Liveliness (PID 0x001B) — 12 Byte (u32 kind + Duration).
//! * Lifespan (PID 0x002B) — 8 Byte Duration (publication-only).
//! * Ownership (PID 0x001F) — 4 Byte u32.
//! * Ownership-Strength (PID 0x0006) — 4 Byte i32 (publication-only).
//! * Partition (PID 0x0029) — sequence<string>.

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

use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, Duration, LifespanQosPolicy, LivelinessKind,
    LivelinessQosPolicy, OwnershipKind, ReliabilityKind, ReliabilityQosPolicy,
};
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

fn writer_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes([0xAA; 12]),
        EntityId::user_writer_with_key([0x01, 0x02, 0x03]),
    )
}

fn reader_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes([0xBB; 12]),
        EntityId::user_reader_with_key([0x10, 0x20, 0x30]),
    )
}

#[test]
fn publication_data_roundtrip_preserves_all_new_qos_policies() {
    let orig = PublicationBuiltinTopicData {
        key: writer_guid(),
        participant_key: Guid::new(GuidPrefix::from_bytes([0xAA; 12]), EntityId::PARTICIPANT),
        topic_name: "SensorData".into(),
        type_name: "sensor_msgs::msg::Temperature".into(),
        durability: DurabilityKind::TransientLocal,
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::from_millis(500),
        },
        ownership: OwnershipKind::Exclusive,
        ownership_strength: 42,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::ManualByTopic,
            lease_duration: Duration::from_millis(1200),
        },
        deadline: DeadlineQosPolicy {
            period: Duration::from_millis(200),
        },
        lifespan: LifespanQosPolicy {
            duration: Duration::from_millis(10_000),
        },
        partition: vec!["alpha".into(), "beta".into(), "".into()],
        user_data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        topic_data: vec![0xCA, 0xFE],
        group_data: vec![0xBA, 0xBE],
        type_information: None,
        data_representation: vec![2],
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

    assert_eq!(decoded.ownership, OwnershipKind::Exclusive);
    assert_eq!(decoded.ownership_strength, 42);
    assert_eq!(decoded.liveliness.kind, LivelinessKind::ManualByTopic);
    assert_eq!(
        decoded.liveliness.lease_duration,
        Duration::from_millis(1200)
    );
    assert_eq!(decoded.deadline.period, Duration::from_millis(200));
    assert_eq!(decoded.lifespan.duration, Duration::from_millis(10_000));
    assert_eq!(decoded.partition.len(), 3);
    assert_eq!(decoded.partition[0], "alpha");
    assert_eq!(decoded.partition[1], "beta");
    assert_eq!(decoded.partition[2], "");
    // Sanity: the existing PIDs survive as well.
    assert_eq!(decoded.durability, DurabilityKind::TransientLocal);
    assert_eq!(decoded.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(decoded.topic_name, "SensorData");
}

#[test]
fn subscription_data_roundtrip_preserves_reader_qos_policies() {
    let orig = SubscriptionBuiltinTopicData {
        key: reader_guid(),
        participant_key: Guid::new(GuidPrefix::from_bytes([0xBB; 12]), EntityId::PARTICIPANT),
        topic_name: "SensorData".into(),
        type_name: "sensor_msgs::msg::Temperature".into(),
        durability: DurabilityKind::TransientLocal,
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::from_millis(100),
        },
        ownership: OwnershipKind::Exclusive,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::ManualByParticipant,
            lease_duration: Duration::from_millis(2500),
        },
        deadline: DeadlineQosPolicy {
            period: Duration::from_millis(500),
        },
        partition: vec!["ControlRoom".into()],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_information: None,
        data_representation: vec![2],
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

    assert_eq!(decoded.ownership, OwnershipKind::Exclusive);
    assert_eq!(decoded.liveliness.kind, LivelinessKind::ManualByParticipant);
    assert_eq!(
        decoded.liveliness.lease_duration,
        Duration::from_millis(2500)
    );
    assert_eq!(decoded.deadline.period, Duration::from_millis(500));
    assert_eq!(decoded.partition, vec!["ControlRoom".to_string()]);
}

#[test]
fn defaults_roundtrip_stays_default() {
    // If the writer leaves all new fields at default, the decoder must
    // dieselben Defaults rekonstruieren.
    let orig = PublicationBuiltinTopicData {
        key: writer_guid(),
        participant_key: Guid::new(GuidPrefix::from_bytes([0xAA; 12]), EntityId::PARTICIPANT),
        topic_name: "Plain".into(),
        type_name: "bool".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: LivelinessQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        partition: Vec::new(),
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_information: None,
        data_representation: Vec::new(),
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
    };
    let bytes = orig.to_pl_cdr_le().unwrap();
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.ownership, OwnershipKind::Shared);
    assert_eq!(decoded.ownership_strength, 0);
    assert!(decoded.liveliness.lease_duration.is_infinite());
    assert!(decoded.deadline.period.is_infinite());
    assert!(decoded.lifespan.duration.is_infinite());
    assert!(decoded.partition.is_empty());
}
