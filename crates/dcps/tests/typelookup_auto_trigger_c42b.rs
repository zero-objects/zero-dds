//! Integration-Tests fuer C4.2-b: TypeLookup Auto-Trigger via
//! SEDP-Discovery.
//!
//! Spec: XTypes 1.3 §7.6.3.3.4. Wenn eine eingehende
//! Pub/SubBuiltinTopicData einen TypeID-Hash mitbringt, der lokal
//! nicht aufloesbar ist, queued der Participant einen
//! TypeLookup-`getTypes`-Request.
//!
//! Backoff: pro unbekanntem Hash max alle 5s ein Request, max 3
//! Versuche.

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
use zerodds_types::EquivalenceHash;

fn participant() -> zerodds_dcps::DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

#[test]
fn enqueue_unknown_hash_queues_request() {
    let p = participant();
    let h = EquivalenceHash([0x11; 14]);
    assert!(p.enqueue_type_lookup(h));
    let drained = p.drain_type_lookup_requests();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].0, h);
}

#[test]
fn enqueue_same_hash_twice_within_backoff_only_first_queued() {
    let p = participant();
    let h = EquivalenceHash([0x22; 14]);
    assert!(p.enqueue_type_lookup(h));
    // Sofort wieder → Backoff (5s) suppressed.
    assert!(!p.enqueue_type_lookup(h));
    assert!(!p.enqueue_type_lookup(h));
    let drained = p.drain_type_lookup_requests();
    assert_eq!(drained.len(), 1);
}

#[test]
fn enqueue_distinct_hashes_both_queued() {
    let p = participant();
    let h1 = EquivalenceHash([1; 14]);
    let h2 = EquivalenceHash([2; 14]);
    assert!(p.enqueue_type_lookup(h1));
    assert!(p.enqueue_type_lookup(h2));
    let drained = p.drain_type_lookup_requests();
    assert_eq!(drained.len(), 2);
}

#[test]
fn drain_requests_consumes_queue() {
    let p = participant();
    let h = EquivalenceHash([3; 14]);
    p.enqueue_type_lookup(h);
    let _ = p.drain_type_lookup_requests();
    let again = p.drain_type_lookup_requests();
    assert!(again.is_empty());
}

#[test]
fn type_lookup_exhausted_returns_false_initially() {
    let p = participant();
    let h = EquivalenceHash([4; 14]);
    assert!(!p.type_lookup_exhausted(h));
    p.enqueue_type_lookup(h);
    // Nach 1 Versuch: nicht exhausted.
    assert!(!p.type_lookup_exhausted(h));
}

#[test]
fn ingest_reply_clears_attempts() {
    let p = participant();
    let h = EquivalenceHash([5; 14]);
    p.enqueue_type_lookup(h);
    use zerodds_types::builder::TypeObjectBuilder;
    use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier};
    let mto = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::Test")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
            .build_minimal(),
    );
    let count = p.ingest_type_lookup_reply(vec![(h, mto)]);
    assert_eq!(count, 1);
    // Re-enqueue sollte jetzt wieder gehen (attempts entfernt).
    assert!(p.enqueue_type_lookup(h));
}

#[test]
fn on_remote_publication_with_no_type_info_is_noop() {
    let p = participant();
    let queued = p.on_remote_publication_discovered(None);
    assert_eq!(queued, 0);
    assert!(p.drain_type_lookup_requests().is_empty());
}

#[test]
fn on_remote_publication_with_unknown_hash_queues_lookup() {
    use zerodds_types::builder::TypeObjectBuilder;
    use zerodds_types::type_information::TypeInformation;
    use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier};
    let p = participant();
    // Bauer Test-TypeInformation.
    let minimal = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::Remote")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
            .build_minimal(),
    );
    let complete = zerodds_types::CompleteTypeObject::Struct(
        TypeObjectBuilder::struct_type("::Remote")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
            .build_complete(),
    );
    let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
    let bytes = ti.to_bytes_le().unwrap();
    let queued = p.on_remote_publication_discovered(Some(&bytes));
    assert!(queued >= 1);
    assert!(!p.drain_type_lookup_requests().is_empty());
}

#[test]
fn on_remote_subscription_uses_same_path() {
    use zerodds_types::builder::TypeObjectBuilder;
    use zerodds_types::type_information::TypeInformation;
    use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier};
    let p = participant();
    let minimal = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::Sub")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal(),
    );
    let complete = zerodds_types::CompleteTypeObject::Struct(
        TypeObjectBuilder::struct_type("::Sub")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_complete(),
    );
    let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
    let bytes = ti.to_bytes_le().unwrap();
    let queued = p.on_remote_subscription_discovered(Some(&bytes));
    assert!(queued >= 1);
}

#[test]
fn malformed_type_information_blob_does_not_crash() {
    let p = participant();
    let bad_bytes = [0xFFu8; 16];
    let queued = p.on_remote_publication_discovered(Some(&bad_bytes));
    assert_eq!(queued, 0);
}

#[test]
fn empty_blob_is_noop() {
    let p = participant();
    let queued = p.on_remote_publication_discovered(Some(&[]));
    assert_eq!(queued, 0);
}

#[test]
fn backoff_burst_only_first_request_goes_through() {
    let p = participant();
    let h = EquivalenceHash([0xAA; 14]);
    assert!(p.enqueue_type_lookup(h));
    assert!(!p.enqueue_type_lookup(h));
    assert!(!p.enqueue_type_lookup(h));
    assert!(!p.enqueue_type_lookup(h));
    // 4 Aufrufe → 1 outgoing.
    assert_eq!(p.drain_type_lookup_requests().len(), 1);
}

#[test]
fn requests_have_monotone_sequence_numbers() {
    let p = participant();
    let h1 = EquivalenceHash([0x11; 14]);
    let h2 = EquivalenceHash([0x22; 14]);
    let h3 = EquivalenceHash([0x33; 14]);
    p.enqueue_type_lookup(h1);
    p.enqueue_type_lookup(h2);
    p.enqueue_type_lookup(h3);
    let drained = p.drain_type_lookup_requests();
    assert_eq!(drained.len(), 3);
    assert!(drained[0].1 < drained[1].1);
    assert!(drained[1].1 < drained[2].1);
}
