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
    LivelinessQosPolicy, OwnershipKind, PresentationAccessScope, PresentationQosPolicy,
    ReliabilityKind, ReliabilityQosPolicy,
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
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
        key: writer_guid(),
        participant_key: Guid::new(GuidPrefix::from_bytes([0xAA; 12]), EntityId::PARTICIPANT),
        topic_name: "SensorData".into(),
        type_name: "sensor_msgs::msg::Temperature".into(),
        durability: DurabilityKind::TransientLocal,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy {
            service_cleanup_delay: Duration::from_secs(30),
            history_kind: zerodds_qos::HistoryKind::KeepAll,
            history_depth: 1,
            max_samples: 500,
            max_instances: 20,
            max_samples_per_instance: 50,
        },
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
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy {
            duration: Duration::from_millis(15),
        },
        destination_order: zerodds_qos::DestinationOrderQosPolicy {
            kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
        },
        lifespan: LifespanQosPolicy {
            duration: Duration::from_millis(10_000),
        },
        presentation: PresentationQosPolicy {
            access_scope: PresentationAccessScope::Group,
            coherent_access: true,
            ordered_access: true,
        },
        partition: vec!["alpha".into(), "beta".into(), "".into()],
        user_data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        topic_data: vec![0xCA, 0xFE],
        group_data: vec![0xBA, 0xBE],
        type_information: None,
        type_id_v1: None,
        type_object_v1: None,
        data_representation: vec![2],
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
        group_guid: None,
        data_max_size_serialized: None,
        entity_name: None,
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
    assert_eq!(decoded.latency_budget.duration, Duration::from_millis(15));
    assert_eq!(
        decoded.destination_order.kind,
        zerodds_qos::DestinationOrderKind::BySourceTimestamp
    );
    assert_eq!(decoded.lifespan.duration, Duration::from_millis(10_000));
    assert_eq!(
        decoded.presentation.access_scope,
        PresentationAccessScope::Group
    );
    assert!(decoded.presentation.coherent_access);
    assert!(decoded.presentation.ordered_access);
    assert_eq!(decoded.partition.len(), 3);
    assert_eq!(decoded.partition[0], "alpha");
    assert_eq!(decoded.partition[1], "beta");
    assert_eq!(decoded.partition[2], "");
    // Sanity: the existing PIDs survive as well.
    assert_eq!(decoded.durability, DurabilityKind::TransientLocal);
    assert_eq!(decoded.reliability.kind, ReliabilityKind::Reliable);
    assert_eq!(decoded.topic_name, "SensorData");
    assert_eq!(
        decoded.durability_service,
        zerodds_qos::DurabilityServiceQosPolicy {
            service_cleanup_delay: Duration::from_secs(30),
            history_kind: zerodds_qos::HistoryKind::KeepAll,
            history_depth: 1,
            max_samples: 500,
            max_instances: 20,
            max_samples_per_instance: 50,
        }
    );
}

fn minimal_publication() -> PublicationBuiltinTopicData {
    PublicationBuiltinTopicData {
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
        key: writer_guid(),
        participant_key: Guid::new(GuidPrefix::from_bytes([0xAA; 12]), EntityId::PARTICIPANT),
        topic_name: "Plain".into(),
        type_name: "bool".into(),
        durability: DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        reliability: ReliabilityQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: LivelinessQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        presentation: PresentationQosPolicy::default(),
        partition: Vec::new(),
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_information: None,
        type_id_v1: None,
        type_object_v1: None,
        data_representation: Vec::new(),
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
        group_guid: None,
        data_max_size_serialized: None,
        entity_name: None,
    }
}

fn minimal_subscription() -> SubscriptionBuiltinTopicData {
    SubscriptionBuiltinTopicData {
        content_filter_unreadable: false,
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
        key: reader_guid(),
        participant_key: Guid::new(GuidPrefix::from_bytes([0xBB; 12]), EntityId::PARTICIPANT),
        topic_name: "Plain".into(),
        type_name: "bool".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        liveliness: LivelinessQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        presentation: PresentationQosPolicy::default(),
        partition: Vec::new(),
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        type_information: None,
        type_id_v1: None,
        type_object_v1: None,
        data_representation: Vec::new(),
        content_filter: None,
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
        expects_inline_qos: false,
        group_guid: None,
        type_consistency_announced: true,
        entity_name: None,
    }
}

#[test]
fn subscription_data_roundtrip_preserves_reader_qos_policies() {
    let orig = SubscriptionBuiltinTopicData {
        content_filter_unreadable: false,
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
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
        time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy {
            minimum_separation: Duration::from_millis(250),
        },
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy {
            duration: Duration::from_millis(80),
        },
        destination_order: zerodds_qos::DestinationOrderQosPolicy {
            kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
        },
        presentation: PresentationQosPolicy {
            access_scope: PresentationAccessScope::Topic,
            coherent_access: false,
            ordered_access: true,
        },
        partition: vec!["ControlRoom".into()],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        type_information: None,
        type_id_v1: None,
        type_object_v1: None,
        data_representation: vec![2],
        content_filter: None,
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
        expects_inline_qos: false,
        group_guid: None,
        type_consistency_announced: true,
        entity_name: None,
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
    assert_eq!(
        decoded.time_based_filter.minimum_separation,
        Duration::from_millis(250)
    );
    assert_eq!(decoded.latency_budget.duration, Duration::from_millis(80));
    assert_eq!(
        decoded.destination_order.kind,
        zerodds_qos::DestinationOrderKind::BySourceTimestamp
    );
    assert_eq!(
        decoded.presentation.access_scope,
        PresentationAccessScope::Topic
    );
    assert!(!decoded.presentation.coherent_access);
    assert!(decoded.presentation.ordered_access);
    assert_eq!(decoded.partition, vec!["ControlRoom".to_string()]);
}

#[test]
fn defaults_roundtrip_stays_default() {
    // If the writer leaves all new fields at default, the decoder must
    // dieselben Defaults rekonstruieren.
    let orig = PublicationBuiltinTopicData {
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
        key: writer_guid(),
        participant_key: Guid::new(GuidPrefix::from_bytes([0xAA; 12]), EntityId::PARTICIPANT),
        topic_name: "Plain".into(),
        type_name: "bool".into(),
        durability: DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        reliability: ReliabilityQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: LivelinessQosPolicy::default(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        presentation: PresentationQosPolicy::default(),
        partition: Vec::new(),
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_information: None,
        type_id_v1: None,
        type_object_v1: None,
        data_representation: Vec::new(),
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
        group_guid: None,
        data_max_size_serialized: None,
        entity_name: None,
    };
    let bytes = orig.to_pl_cdr_le().unwrap();
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.ownership, OwnershipKind::Shared);
    assert_eq!(decoded.ownership_strength, 0);
    assert!(decoded.liveliness.lease_duration.is_infinite());
    assert!(decoded.deadline.period.is_infinite());
    assert!(decoded.lifespan.duration.is_infinite());
    assert_eq!(decoded.presentation, PresentationQosPolicy::default());
    assert!(decoded.partition.is_empty());
    assert_eq!(
        decoded.durability_service,
        zerodds_qos::DurabilityServiceQosPolicy::default()
    );
}

/// DDS-DCPS 1.4 §2.2.3.2 TOPIC_DATA and §2.2.3.3 GROUP_DATA: "the value
/// … is available … via the built-in topic". Both are opaque octet
/// sequences the middleware must carry through untouched, in the
/// publication and the subscription announcement alike. Binary bytes,
/// including an embedded NUL, so "opaque" is measured rather than
/// assumed; and an empty value must stay empty rather than come back as
/// a one-octet artefact.
#[test]
fn topic_data_and_group_data_survive_the_builtin_topic_untouched() {
    let payload = vec![0x00, 0xFF, 0x7F, 0x80, 0x00, 0x01];

    let mut pubd = minimal_publication();
    pubd.topic_data.clone_from(&payload);
    pubd.group_data = vec![0xDE, 0xAD];
    let decoded =
        PublicationBuiltinTopicData::from_pl_cdr_le(&pubd.to_pl_cdr_le().expect("encode"))
            .expect("decode");
    assert_eq!(decoded.topic_data, payload);
    assert_eq!(decoded.group_data, vec![0xDE, 0xAD]);

    let mut subd = minimal_subscription();
    subd.topic_data.clone_from(&payload);
    subd.group_data = vec![0xBE, 0xEF, 0x00];
    let decoded =
        SubscriptionBuiltinTopicData::from_pl_cdr_le(&subd.to_pl_cdr_le().expect("encode"))
            .expect("decode");
    assert_eq!(decoded.topic_data, payload);
    assert_eq!(decoded.group_data, vec![0xBE, 0xEF, 0x00]);

    // The default is an empty sequence and stays one.
    let empty = PublicationBuiltinTopicData::from_pl_cdr_le(
        &minimal_publication().to_pl_cdr_le().expect("encode"),
    )
    .expect("decode");
    assert!(empty.topic_data.is_empty());
    assert!(empty.group_data.is_empty());
}
