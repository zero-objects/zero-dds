// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! F-TYPES-3 E2E test: XTypes 1.3 §7.6.3 PID_ZERODDS_TYPE_ID wire roundtrip.
//!
//! Verifies that the vendor PID 0x8002 roundtrips cleanly through
//! `PublicationBuiltinTopicData::to_pl_cdr_le` + `from_pl_cdr_le`, and
//! that reader matching via `TypeMatcher` checks compatibility.

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

use zerodds_qos as qos;
use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};
use zerodds_types::qos::TypeConsistencyEnforcement;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_matcher::TypeMatcher;
use zerodds_types::{PrimitiveKind, TypeIdentifier};

fn mk_pubd(type_id: TypeIdentifier) -> PublicationBuiltinTopicData {
    PublicationBuiltinTopicData {
        key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "TypeA".into(),
        durability: qos::DurabilityKind::Volatile,
        reliability: qos::ReliabilityQosPolicy::default(),
        ownership: qos::OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: qos::LivelinessQosPolicy::default(),
        deadline: qos::DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        lifespan: qos::LifespanQosPolicy::default(),
        presentation: qos::PresentationQosPolicy::default(),
        partition: alloc::vec::Vec::new(),
        user_data: alloc::vec::Vec::new(),
        topic_data: alloc::vec::Vec::new(),
        group_data: alloc::vec::Vec::new(),
        type_information: None,
        data_representation: alloc::vec::Vec::new(),
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: type_id,
        unicast_locators: alloc::vec::Vec::new(),
        multicast_locators: alloc::vec::Vec::new(),
    }
}

fn mk_subd(type_id: TypeIdentifier) -> SubscriptionBuiltinTopicData {
    SubscriptionBuiltinTopicData {
        key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
        participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "TypeA".into(),
        durability: qos::DurabilityKind::Volatile,
        reliability: qos::ReliabilityQosPolicy::default(),
        ownership: qos::OwnershipKind::Shared,
        liveliness: qos::LivelinessQosPolicy::default(),
        deadline: qos::DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        presentation: qos::PresentationQosPolicy::default(),
        partition: alloc::vec::Vec::new(),
        user_data: alloc::vec::Vec::new(),
        topic_data: alloc::vec::Vec::new(),
        group_data: alloc::vec::Vec::new(),
        type_information: None,
        data_representation: alloc::vec::Vec::new(),
        content_filter: None,
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: type_id,
        unicast_locators: alloc::vec::Vec::new(),
        multicast_locators: alloc::vec::Vec::new(),
    }
}

#[test]
fn pid_zerodds_type_id_publication_roundtrip_primitive() {
    // Wire roundtrip with a primitive TypeIdentifier.
    let original_id = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    let pubd = mk_pubd(original_id.clone());
    let bytes = pubd.to_pl_cdr_le().unwrap();
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.type_identifier, original_id);
}

#[test]
fn pid_zerodds_type_id_subscription_roundtrip_string8() {
    let original_id = TypeIdentifier::String8Small { bound: 64 };
    let subd = mk_subd(original_id.clone());
    let bytes = subd.to_pl_cdr_le().unwrap();
    let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.type_identifier, original_id);
}

#[test]
fn pid_zerodds_type_id_default_none_omitted_from_wire() {
    // type_identifier = None: PID not emitted. Decoded stays None.
    let pubd = mk_pubd(TypeIdentifier::None);
    let bytes = pubd.to_pl_cdr_le().unwrap();
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.type_identifier, TypeIdentifier::None);
}

#[test]
fn type_matcher_accepts_identical_primitive_ids() {
    let tce = TypeConsistencyEnforcement::default();
    let m = TypeMatcher::new(&tce);
    let registry = TypeRegistry::new();
    let id = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    assert!(m.match_types(&id, &id, &registry).is_match());
}

#[test]
fn type_matcher_rejects_incompatible_primitives_under_force_validation() {
    let tce = TypeConsistencyEnforcement {
        force_type_validation: true,
        ..Default::default()
    };
    let m = TypeMatcher::new(&tce);
    let registry = TypeRegistry::new();
    let writer = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    let reader = TypeIdentifier::Primitive(PrimitiveKind::Float64);
    assert!(
        !m.match_types(&writer, &reader, &registry).is_match(),
        "Int32 vs Float64 under force_type_validation must not match"
    );
}
