// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `type_id` (`TypeIdV1`, `PID` 0x0069) and `type` (`TypeObjectV1`, `PID`
//! 0x0072) in the built-in topic data of DDS-XTypes 1.3 Annex D: the topic,
//! publication and subscription data carry them through their parameter
//! lists, and an endpoint that announces a `TypeObjectV1` and no
//! `TypeInformation` is identified by the Complete TypeObject it describes
//! (§7.6.3.2.2).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

extern crate alloc;

use zerodds_qos::{DurabilityKind, ReliabilityQosPolicy};
use zerodds_rtps::parameter_list::{ParameterList, pid};
use zerodds_rtps::publication_data::{PublicationBuiltinTopicData, ReliabilityQos};
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::topic_data::TopicBuiltinTopicData;
use zerodds_rtps::type_v1::identifier_of;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::hash::compute_complete_hash;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::{PrimitiveKind, TypeIdentifier};
use zerodds_types::type_object::CompleteTypeObject;
use zerodds_types::type_object_v1::convert::from_complete;
use zerodds_types::type_object_v1::{TypeId, TypeObjectV1, kind};

/// A structure `Demo::Sample { long a; string b; }` as a Complete object, its
/// identifier, and the `TypeObjectV1` that describes it.
fn sample_type() -> (TypeIdentifier, TypeObjectV1) {
    let object = CompleteTypeObject::Struct(
        TypeObjectBuilder::struct_type("Demo::Sample")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("b", TypeIdentifier::String8Small { bound: 0 }, |m| m)
            .build_complete(),
    );
    let hash = compute_complete_hash(&object).expect("hash");
    let mut registry = TypeRegistry::new();
    registry.insert_complete(hash, object);
    let identifier = TypeIdentifier::EquivalenceHashComplete(hash);
    let v1 = from_complete(&identifier, &registry).expect("converts");
    (identifier, v1)
}

fn publication(v1: Option<TypeObjectV1>) -> PublicationBuiltinTopicData {
    PublicationBuiltinTopicData {
        vendor_id: Some(VendorId::ZERODDS),
        key: Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0xAA; 3]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "Demo::Sample".into(),
        durability: DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        reliability: ReliabilityQos::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        lifespan: zerodds_qos::LifespanQosPolicy::default(),
        presentation: zerodds_qos::PresentationQosPolicy::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_information: None,
        type_id_v1: v1.as_ref().and_then(|o| o.the_type.first().copied()),
        type_object_v1: v1,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        data_representation: vec![],
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
        group_guid: None,
        data_max_size_serialized: None,
        entity_name: None,
    }
}

fn subscription(v1: Option<TypeObjectV1>) -> SubscriptionBuiltinTopicData {
    SubscriptionBuiltinTopicData {
        content_filter_unreadable: false,
        vendor_id: Some(VendorId::ZERODDS),
        key: Guid::new(
            GuidPrefix::from_bytes([3; 12]),
            EntityId::user_reader_with_key([0xCC; 3]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes([3; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "Demo::Sample".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQosPolicy::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        presentation: zerodds_qos::PresentationQosPolicy::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        type_information: None,
        type_id_v1: v1.as_ref().and_then(|o| o.the_type.first().copied()),
        type_object_v1: v1,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        data_representation: vec![],
        content_filter: None,
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: TypeIdentifier::None,
        unicast_locators: vec![],
        multicast_locators: vec![],
        expects_inline_qos: false,
        group_guid: None,
        type_consistency_announced: true,
        entity_name: None,
    }
}

#[test]
fn a_publication_carries_its_type_id_and_type() {
    let (identifier, v1) = sample_type();
    let orig = publication(Some(v1.clone()));
    let bytes = orig.to_pl_cdr_le().expect("encodes");
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decodes");
    assert_eq!(decoded.type_object_v1.as_ref(), Some(&v1));
    assert_eq!(decoded.type_id_v1, v1.the_type.first().copied());
    assert!(
        matches!(decoded.type_id_v1, Some(TypeId::Constructed { kind: k, .. }) if k == kind::STRUCTURE_TYPE)
    );
    // No PID_TYPE_INFORMATION was announced: the TypeObjectV1 is the
    // identity of the endpoint, the Complete hash of the type it describes.
    assert!(decoded.type_information.is_none());
    assert_eq!(decoded.type_identifier, identifier);
}

#[test]
fn a_subscription_carries_its_type_id_and_type() {
    let (identifier, v1) = sample_type();
    let orig = subscription(Some(v1.clone()));
    let bytes = orig.to_pl_cdr_le().expect("encodes");
    let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decodes");
    assert_eq!(decoded.type_object_v1.as_ref(), Some(&v1));
    assert_eq!(decoded.type_id_v1, v1.the_type.first().copied());
    assert_eq!(decoded.type_identifier, identifier);
}

#[test]
fn a_topic_carries_its_type_id_and_type() {
    let (_, v1) = sample_type();
    let mut orig = TopicBuiltinTopicData::new(
        Guid::new(GuidPrefix::from_bytes([4; 12]), EntityId::PARTICIPANT),
        "T",
        "Demo::Sample",
    );
    orig.type_id_v1 = v1.the_type.first().copied();
    orig.type_object_v1 = Some(v1.clone());
    let bytes = orig.to_pl_cdr_le().expect("encodes");
    let decoded = TopicBuiltinTopicData::from_pl_cdr(&bytes, VendorId::ZERODDS).expect("decodes");
    assert_eq!(decoded.type_object_v1, Some(v1));
    assert_eq!(decoded.type_id_v1, orig.type_id_v1);
}

#[test]
fn the_members_are_optional_and_left_out_when_absent() {
    let orig = publication(None);
    let bytes = orig.to_pl_cdr_le().expect("encodes");
    // Neither parameter id is in the list.
    let pl = ParameterList::from_bytes(&bytes[4..], true).expect("a parameter list");
    assert!(pl.find(pid::TYPE_ID_V1).is_none());
    assert!(pl.find(pid::TYPE_OBJECT_V1).is_none());
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decodes");
    assert!(decoded.type_id_v1.is_none() && decoded.type_object_v1.is_none());
    assert_eq!(decoded.type_identifier, TypeIdentifier::None);
}

#[test]
fn the_parameters_have_the_ids_of_annex_d() {
    let (_, v1) = sample_type();
    let bytes = publication(Some(v1)).to_pl_cdr_le().expect("encodes");
    let pl = ParameterList::from_bytes(&bytes[4..], true).expect("a parameter list");
    assert_eq!(pid::TYPE_ID_V1, 0x0069);
    assert_eq!(pid::TYPE_OBJECT_V1, 0x0072);
    assert!(pl.find(0x0069).is_some(), "type_id is @id(0x0069)");
    assert!(pl.find(0x0072).is_some(), "type is @id(0x0072)");
}

#[test]
fn a_parameter_that_does_not_decode_is_left_out_not_a_refusal() {
    // The members are optional (§7.6.3.2.2, non-normative note): an endpoint
    // whose 0x0072 is not Annex B of XTypes 1.1 is still discovered, by its
    // type name.
    let (_, v1) = sample_type();
    let bytes = publication(Some(v1)).to_pl_cdr_le().expect("encodes");
    let mut pl = ParameterList::from_bytes(&bytes[4..], true).expect("a parameter list");
    for p in &mut pl.parameters {
        if p.id == pid::TYPE_OBJECT_V1 {
            p.value = vec![0xFF; 8];
        }
    }
    let mut rebuilt = bytes[..4].to_vec();
    rebuilt.extend(pl.to_bytes(true));
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&rebuilt).expect("still decodes");
    assert!(decoded.type_object_v1.is_none());
    assert_eq!(decoded.type_identifier, TypeIdentifier::None);
}

#[test]
fn the_identity_of_a_type_object_is_the_complete_hash_of_its_type() {
    let (identifier, v1) = sample_type();
    assert_eq!(identifier_of(&v1), Some(identifier));
    // An object with no type says nothing.
    assert_eq!(identifier_of(&TypeObjectV1::default()), None);
}
