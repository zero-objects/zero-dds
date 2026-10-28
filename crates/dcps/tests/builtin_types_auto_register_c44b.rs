//! Integration tests for C4.4-b: built-in types auto-register
//! in the participant.
//!
//! Spec: XTypes 1.3 §7.6.5 + Annex E. Four built-in types:
//! `DDS::String`, `DDS::KeyedString`, `DDS::Bytes`, `DDS::KeyedBytes`.
//!
//! Expected behavior:
//! - After `create_participant_offline()` all 4 are lookup-able.
//! - Idempotent: calling `register_builtin_types()` twice must not
//!   crash and must not double the count.
//! - `unregister_builtin_types()` removes them again; registering
//!   again restores them.

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

use zerodds_ast::{Bound, CharWidth, PrimitiveKind, TypeKind};
use zerodds_ast_core::Ref;
use zerodds_dcps::builtin_type_set::{
    NAME_DDS_BYTES, NAME_DDS_KEYED_BYTES, NAME_DDS_KEYED_STRING, NAME_DDS_STRING,
    is_builtin_type_name,
};
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};

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
    assert!(matches!(s.kind(), Some(TypeKind::Structure { .. })));
    assert_eq!(s.name(), NAME_DDS_STRING);
    let ks = p.find_builtin_type(NAME_DDS_KEYED_STRING).unwrap();
    assert!(matches!(ks.kind(), Some(TypeKind::Structure { .. })));
    let key = ks.member_by_name("key").unwrap();
    assert!(key.is_key());
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
    assert!(matches!(
        value.type_().and_then(|t| t.kind().cloned()),
        Some(TypeKind::String {
            width: CharWidth::Char,
            bound: Bound::Unbounded
        })
    ));
}

#[test]
fn dds_bytes_has_octet_sequence_value_member() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p.find_builtin_type(NAME_DDS_BYTES).unwrap();
    let value = t.member_by_name("value").unwrap();
    let value_type = value.type_().expect("the value member's type resolves");
    let Some(TypeKind::Sequence {
        element,
        bound: Bound::Unbounded,
    }) = value_type.kind().cloned()
    else {
        panic!("DDS::Bytes.value is {:?}", value_type.kind());
    };
    let Ref::Resolved(element) = element else {
        panic!("the element type does not resolve");
    };
    // §7.6.5 says `sequence<octet>`, and a sequence of anything else
    // would put a different number of octets on the wire per element.
    assert!(matches!(
        value_type.ast().type_(element).map(|t| t.kind.clone()),
        Ok(TypeKind::Primitive(PrimitiveKind::Octet))
    ));
}

#[test]
fn separate_participants_have_separate_registries() {
    let p1 = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let p2 = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    p1.unregister_builtin_types();
    assert_eq!(p1.registered_type_count(), 0);
    // P2 unaffected.
    assert_eq!(p2.registered_type_count(), 4);
}

#[test]
fn dds_keyed_bytes_has_key_member() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p.find_builtin_type(NAME_DDS_KEYED_BYTES).unwrap();
    assert_eq!(t.member_count(), 2);
    let key = t.member_by_name("key").unwrap();
    assert!(key.is_key());
    assert!(matches!(
        key.type_().and_then(|t| t.kind().cloned()),
        Some(TypeKind::String {
            width: CharWidth::Char,
            bound: Bound::Unbounded
        })
    ));
}

#[test]
fn builtin_types_are_not_nested() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    for name in [
        NAME_DDS_STRING,
        NAME_DDS_KEYED_STRING,
        NAME_DDS_BYTES,
        NAME_DDS_KEYED_BYTES,
    ] {
        let t = p.find_builtin_type(name).unwrap();
        assert!(!t.is_nested(), "{name} is non-nested (XTypes 1.3 §7.6.5)");
    }
}
