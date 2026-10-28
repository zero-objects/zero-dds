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
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
        key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "TypeA".into(),
        durability: qos::DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
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
        type_id_v1: None,
        type_object_v1: None,
        data_representation: alloc::vec::Vec::new(),
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: type_id,
        unicast_locators: alloc::vec::Vec::new(),
        multicast_locators: alloc::vec::Vec::new(),
        group_guid: None,
        data_max_size_serialized: None,
        entity_name: None,
    }
}

fn mk_subd(type_id: TypeIdentifier) -> SubscriptionBuiltinTopicData {
    SubscriptionBuiltinTopicData {
        content_filter_unreadable: false,
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
        key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
        participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
        topic_name: "T".into(),
        type_name: "TypeA".into(),
        durability: qos::DurabilityKind::Volatile,
        reliability: qos::ReliabilityQosPolicy::default(),
        ownership: qos::OwnershipKind::Shared,
        liveliness: qos::LivelinessQosPolicy::default(),
        deadline: qos::DeadlineQosPolicy::default(),
        time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        presentation: qos::PresentationQosPolicy::default(),
        partition: alloc::vec::Vec::new(),
        user_data: alloc::vec::Vec::new(),
        topic_data: alloc::vec::Vec::new(),
        group_data: alloc::vec::Vec::new(),
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        type_information: None,
        type_id_v1: None,
        type_object_v1: None,
        data_representation: alloc::vec::Vec::new(),
        content_filter: None,
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: type_id,
        unicast_locators: alloc::vec::Vec::new(),
        multicast_locators: alloc::vec::Vec::new(),
        expects_inline_qos: false,
        group_guid: None,
        type_consistency_announced: true,
        entity_name: None,
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

/// XTypes 1.3 §7.6.3.4, PID_TYPE_CONSISTENCY_ENFORCEMENT (0x0074).
///
/// The reader's policy is announced, so a remote writer can reach the
/// same §7.6.3.4.2 verdict its reader will. It is not a writer-side
/// policy and has no offered counterpart, which is exactly why the
/// writer needs the reader's own on the wire — without it the writer
/// decides under `TypeConsistencyEnforcement::default()` and publishes
/// to readers that reject the type, or refuses readers whose policy
/// would have taken it.
#[test]
fn a_readers_type_consistency_policy_survives_its_announcement() {
    let mut subd = mk_subd(TypeIdentifier::Primitive(PrimitiveKind::Int32));
    subd.type_consistency = TypeConsistencyEnforcement {
        kind: zerodds_types::qos::TypeConsistencyKind::DisallowTypeCoercion,
        ignore_sequence_bounds: false,
        ignore_string_bounds: false,
        ignore_member_names: true,
        prevent_type_widening: true,
        force_type_validation: true,
    };
    let bytes = subd.to_pl_cdr_le().unwrap();
    let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(
        decoded.type_consistency, subd.type_consistency,
        "every field of the policy has to survive the announcement, not only the ones \
         that differ from the default"
    );

    // And the defaults are announced too, rather than omitted: a writer
    // that has to decide for both Topics needs the policy on every
    // announcement.
    let plain = mk_subd(TypeIdentifier::Primitive(PrimitiveKind::Int32));
    let decoded =
        SubscriptionBuiltinTopicData::from_pl_cdr_le(&plain.to_pl_cdr_le().unwrap()).unwrap();
    assert_eq!(
        decoded.type_consistency,
        TypeConsistencyEnforcement::default()
    );
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

// ========================================================================
// Issue #30: standard PID_TYPE_INFORMATION → TypeIdentifier for cross-vendor
// (non-ZeroDDS) peers, so the assignability guard is exercised instead of a
// name-only fallback. The blobs below are built from REAL TypeObjects (not
// mocked), so the decode path is genuinely exercised.
// ========================================================================

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::hash::compute_minimal_hash;
use zerodds_types::type_information::TypeInformation;
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject};

/// `::chat::Chatter {@key id: i64, text: string<255>}` at a chosen
/// extensibility.
fn chatter(ext: Extensibility) -> (MinimalTypeObject, CompleteTypeObject) {
    let b = TypeObjectBuilder::struct_type("::chat::Chatter")
        .extensibility(ext)
        .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
            m.key()
        })
        .member("text", TypeIdentifier::String8Small { bound: 255 }, |m| m);
    (
        MinimalTypeObject::Struct(b.build_minimal()),
        CompleteTypeObject::Struct(b.build_complete()),
    )
}

/// Serialized standard TypeInformation blob, exactly as a non-ZeroDDS peer
/// carries it in PID_TYPE_INFORMATION (0x0075).
fn type_info_blob(min: &MinimalTypeObject, com: &CompleteTypeObject) -> alloc::vec::Vec<u8> {
    TypeInformation::from_minimal_and_complete(min, com)
        .unwrap()
        .to_bytes_le()
        .unwrap()
}

#[test]
fn publication_standard_type_information_yields_type_identifier() {
    // Regression repro (issue #30): a peer with only the standard
    // PID_TYPE_INFORMATION (NO vendor PID_ZERODDS_TYPE_ID) must now decode to
    // `type_identifier != None`, so the assignability guard runs.
    let (min, com) = chatter(Extensibility::Appendable);
    let mut pubd = mk_pubd(TypeIdentifier::None);
    pubd.type_information = Some(type_info_blob(&min, &com));
    let bytes = pubd.to_pl_cdr_le().unwrap();
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();

    assert_ne!(
        decoded.type_identifier,
        TypeIdentifier::None,
        "standard PID_TYPE_INFORMATION must yield a comparable TypeIdentifier"
    );
    let expected = TypeIdentifier::EquivalenceHashMinimal(compute_minimal_hash(&min).unwrap());
    assert_eq!(
        decoded.type_identifier, expected,
        "minimal equivalence hash is preferred for match-time assignability"
    );
    assert!(
        decoded.type_information.is_some(),
        "the raw blob is retained for the TypeLookup getTypes fetch"
    );
}

#[test]
fn subscription_standard_type_information_yields_type_identifier() {
    // Symmetric to the publication direction (breite Probe: both directions).
    let (min, com) = chatter(Extensibility::Appendable);
    let mut subd = mk_subd(TypeIdentifier::None);
    subd.type_information = Some(type_info_blob(&min, &com));
    let bytes = subd.to_pl_cdr_le().unwrap();
    let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();

    let expected = TypeIdentifier::EquivalenceHashMinimal(compute_minimal_hash(&min).unwrap());
    assert_eq!(decoded.type_identifier, expected);
}

#[test]
fn vendor_pid_takes_precedence_over_standard_blob() {
    // If BOTH the vendor PID and the standard blob are present (ZeroDDS↔ZeroDDS
    // that also advertises TypeInformation), the vendor PID wins and the blob
    // derivation is NOT applied.
    let vendor_id = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    let (min, com) = chatter(Extensibility::Appendable);
    let mut pubd = mk_pubd(vendor_id.clone());
    pubd.type_information = Some(type_info_blob(&min, &com));
    let bytes = pubd.to_pl_cdr_le().unwrap();
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.type_identifier, vendor_id);
}

#[test]
fn unparseable_standard_blob_leaves_none_and_retains_blob() {
    // Silent-drop guard: a corrupt TypeInformation blob must NOT be coerced
    // into a bogus identifier. It stays None, and the raw blob is retained so
    // the match layer can emit a loud signal (no silent name-fallback).
    let mut pubd = mk_pubd(TypeIdentifier::None);
    pubd.type_information = Some(alloc::vec![0xAB, 0xCD, 0x01]); // not valid CDR TypeInformation
    let bytes = pubd.to_pl_cdr_le().unwrap();
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.type_identifier, TypeIdentifier::None);
    assert!(decoded.type_information.is_some());
}

#[test]
fn final_vs_appendable_mismatch_rejected_at_match_time() {
    // The core of issue #30: an extensibility mismatch (@final writer vs
    // @appendable reader) is a MATCH-TIME rejection, not a match followed by a
    // decode failure. Both identifiers are derived exactly as a decoded
    // cross-vendor peer would derive them (minimal hash from the standard
    // blob), resolved structurally through a populated registry.
    let (w_min, w_com) = chatter(Extensibility::Final);
    let (r_min, r_com) = chatter(Extensibility::Appendable);

    let w_id = TypeInformation::from_minimal_and_complete(&w_min, &w_com)
        .unwrap()
        .preferred_type_identifier()
        .unwrap();
    let r_id = TypeInformation::from_minimal_and_complete(&r_min, &r_com)
        .unwrap()
        .preferred_type_identifier()
        .unwrap();
    assert_ne!(
        w_id, r_id,
        "differing extensibility must yield distinct hashes"
    );

    let mut reg = TypeRegistry::new();
    reg.insert_minimal(compute_minimal_hash(&w_min).unwrap(), w_min.clone());
    reg.insert_minimal(compute_minimal_hash(&r_min).unwrap(), r_min.clone());

    let tce = TypeConsistencyEnforcement::default();
    let m = TypeMatcher::new(&tce);
    assert!(
        !m.match_types(&w_id, &r_id, &reg).is_match(),
        "@final writer vs @appendable reader must be rejected at match time"
    );
}

#[test]
fn appendable_compatible_matches_at_match_time() {
    // Positive: appendable-compatible evolution (writer = reader + trailing
    // @optional member) matches under the default coercing TCE — both ids
    // derived from standard blobs, resolved through the registry.
    let reader_b = TypeObjectBuilder::struct_type("::evo::S")
        .extensibility(Extensibility::Appendable)
        .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
        .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m);
    let writer_b = TypeObjectBuilder::struct_type("::evo::S")
        .extensibility(Extensibility::Appendable)
        .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
        .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
        .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
            m.optional()
        });
    let r_min = MinimalTypeObject::Struct(reader_b.build_minimal());
    let r_com = CompleteTypeObject::Struct(reader_b.build_complete());
    let w_min = MinimalTypeObject::Struct(writer_b.build_minimal());
    let w_com = CompleteTypeObject::Struct(writer_b.build_complete());

    let w_id = TypeInformation::from_minimal_and_complete(&w_min, &w_com)
        .unwrap()
        .preferred_type_identifier()
        .unwrap();
    let r_id = TypeInformation::from_minimal_and_complete(&r_min, &r_com)
        .unwrap()
        .preferred_type_identifier()
        .unwrap();

    let mut reg = TypeRegistry::new();
    reg.insert_minimal(compute_minimal_hash(&w_min).unwrap(), w_min.clone());
    reg.insert_minimal(compute_minimal_hash(&r_min).unwrap(), r_min.clone());

    let tce = TypeConsistencyEnforcement::default();
    let m = TypeMatcher::new(&tce);
    assert!(
        m.match_types(&w_id, &r_id, &reg).is_match(),
        "appendable writer-superset must match through the populated registry"
    );
}

#[test]
fn datareader_qos_exposes_type_consistency_enforcement() {
    // Issue #30 item 3: TypeConsistencyEnforcement is reachable on
    // DataReaderQos with the spec default (ALLOW_TYPE_COERCION), and
    // force_type_validation is user-settable and demonstrably changes the
    // matcher verdict.
    use zerodds_dcps::DataReaderQos;
    use zerodds_types::qos::TypeConsistencyKind;

    let default_qos = DataReaderQos::default();
    assert_eq!(
        default_qos.type_consistency.kind,
        TypeConsistencyKind::AllowTypeCoercion
    );
    assert!(!default_qos.type_consistency.force_type_validation);

    // The QoS reaches the matcher: the identical type matches, the
    // widening i16 → i32 does not. XTypes 1.3 §7.2.4.4.4, Table 15, gives
    // a primitive type one assignable source, the same primitive type, so
    // the widening is rejected under the coercing default as well.
    let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    let reg = TypeRegistry::new();
    assert!(
        TypeMatcher::new(&default_qos.type_consistency)
            .match_types(&w, &w, &reg)
            .is_match()
    );
    assert!(
        !TypeMatcher::new(&default_qos.type_consistency)
            .match_types(&w, &r, &reg)
            .is_match(),
        "Table 15 has no widening row, so the coercing default rejects i16 → i32"
    );

    // force_type_validation on the DataReaderQos reaches the same verdict
    // here. Its own effect — failing Step 2 of §7.6.3.4.2 when a side
    // announces no TypeObject — lives in `decide_type_consistency`.
    let strict_qos = DataReaderQos {
        type_consistency: TypeConsistencyEnforcement {
            force_type_validation: true,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        !TypeMatcher::new(&strict_qos.type_consistency)
            .match_types(&w, &r, &reg)
            .is_match(),
        "force_type_validation set via DataReaderQos must reject the widening"
    );
}
