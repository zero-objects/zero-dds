//! Integration-Tests fuer C4.4-b: Built-in Types Auto-Register
//! im Participant.
//!
//! Spec: XTypes 1.3 §7.6.5 + Annex E. Vier Built-in-Types:
//! `DDS::String`, `DDS::KeyedString`, `DDS::Bytes`, `DDS::KeyedBytes`.
//!
//! Erwartetes Verhalten:
//! - Nach `create_participant_offline()` sind alle 4 lookupbar.
//! - Idempotent: doppeltes `register_builtin_types()` darf nicht
//!   crashen + nicht die Anzahl verdoppeln.
//! - `unregister_builtin_types()` entfernt sie wieder; erneutes
//!   Register stellt sie wieder her.

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

use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_types::dynamic::{
    NAME_DDS_BYTES, NAME_DDS_KEYED_BYTES, NAME_DDS_KEYED_STRING, NAME_DDS_STRING, TypeKind,
    is_builtin_type_name,
};

#[test]
fn participant_after_create_has_four_builtin_types_registered() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    assert_eq!(p.registered_type_count(), 4);
    for name in [
        NAME_DDS_STRING,
        NAME_DDS_KEYED_STRING,
        NAME_DDS_BYTES,
        NAME_DDS_KEYED_BYTES,
    ] {
        assert!(p.find_builtin_type(name).is_some(), "{name} not registered");
    }
}

#[test]
fn find_builtin_type_returns_correct_kind() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let s = p.find_builtin_type(NAME_DDS_STRING).unwrap();
    assert_eq!(s.kind(), TypeKind::Structure);
    assert_eq!(s.name(), NAME_DDS_STRING);
    let ks = p.find_builtin_type(NAME_DDS_KEYED_STRING).unwrap();
    assert_eq!(ks.kind(), TypeKind::Structure);
    let key = ks.member_by_name("key").unwrap();
    assert!(key.descriptor().is_key);
}

#[test]
fn register_builtin_types_is_idempotent() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    assert_eq!(p.registered_type_count(), 4);
    p.register_builtin_types();
    p.register_builtin_types();
    assert_eq!(p.registered_type_count(), 4);
}

#[test]
fn unregister_builtin_types_removes_all_four() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    p.unregister_builtin_types();
    assert_eq!(p.registered_type_count(), 0);
    assert!(p.find_builtin_type(NAME_DDS_STRING).is_none());
}

#[test]
fn unregister_then_register_restores_all_four() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    p.unregister_builtin_types();
    assert_eq!(p.registered_type_count(), 0);
    p.register_builtin_types();
    assert_eq!(p.registered_type_count(), 4);
}

#[test]
fn find_builtin_type_unknown_name_returns_none() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    assert!(p.find_builtin_type("My::Custom").is_none());
    assert!(p.find_builtin_type("dds::string").is_none());
}

#[test]
fn is_builtin_type_name_helper_matches_registry() {
    assert!(is_builtin_type_name("DDS::String"));
    assert!(is_builtin_type_name("DDS::KeyedString"));
    assert!(is_builtin_type_name("DDS::Bytes"));
    assert!(is_builtin_type_name("DDS::KeyedBytes"));
    assert!(!is_builtin_type_name("DDS::String_"));
}

#[test]
fn dds_string_has_unbounded_string_value_member() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p.find_builtin_type(NAME_DDS_STRING).unwrap();
    let value = t.member_by_name("value").unwrap();
    assert_eq!(value.dynamic_type().kind(), TypeKind::String8);
}

#[test]
fn dds_bytes_has_octet_sequence_value_member() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p.find_builtin_type(NAME_DDS_BYTES).unwrap();
    let value = t.member_by_name("value").unwrap();
    assert_eq!(value.dynamic_type().kind(), TypeKind::Sequence);
}

#[test]
fn separate_participants_have_separate_registries() {
    let p1 = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let p2 = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    p1.unregister_builtin_types();
    assert_eq!(p1.registered_type_count(), 0);
    // P2 unbeeinflusst.
    assert_eq!(p2.registered_type_count(), 4);
}

#[test]
fn dds_keyed_bytes_has_key_member() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p.find_builtin_type(NAME_DDS_KEYED_BYTES).unwrap();
    assert_eq!(t.member_count(), 2);
    let key = t.member_by_name("key").unwrap();
    assert!(key.descriptor().is_key);
    assert_eq!(key.dynamic_type().kind(), TypeKind::String8);
}

#[test]
fn builtin_types_have_nested_annotation() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    for name in [
        NAME_DDS_STRING,
        NAME_DDS_KEYED_STRING,
        NAME_DDS_BYTES,
        NAME_DDS_KEYED_BYTES,
    ] {
        let t = p.find_builtin_type(name).unwrap();
        assert!(t.descriptor().is_nested, "{name} should be is_nested=true");
    }
}
