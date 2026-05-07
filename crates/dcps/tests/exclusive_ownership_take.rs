// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E-Test fuer den Exclusive-Ownership-Filter im DataReader.take()-Pfad
//! (DDS 1.4 §2.2.3.23 / §2.2.2.5.5).
//!
//! Verifiziert, dass:
//! - Bei `OwnershipKind::Shared` keine Filterung stattfindet (alle Samples
//!   passieren take()).
//! - Bei `OwnershipKind::Exclusive` der erste Sample-Empfang den Writer
//!   als Owner setzt; Samples schwaecherer Writer werden gedropt; Samples
//!   staerkerer Writer uebernehmen die Ownership; Tie-Break ueber GUID.

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

use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, RawBytes, SubscriberQos,
    TopicQos,
};
use zerodds_qos::{OwnershipKind, OwnershipQosPolicy};

fn mk_reader(domain: i32, ownership: OwnershipKind) -> zerodds_dcps::DataReader<RawBytes> {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(domain, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("OwTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let qos = DataReaderQos {
        ownership: OwnershipQosPolicy { kind: ownership },
        ..DataReaderQos::default()
    };
    sub.create_datareader::<RawBytes>(&topic, qos).unwrap()
}

#[test]
fn shared_ownership_passes_all_samples() {
    let reader = mk_reader(310, OwnershipKind::Shared);
    let strong = [9u8; 16];
    let weak = [1u8; 16];
    reader
        .__push_raw_with_writer(b"strong".to_vec(), strong, 100)
        .unwrap();
    reader
        .__push_raw_with_writer(b"weak".to_vec(), weak, 1)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 2, "Shared-Ownership darf nicht filtern");
}

#[test]
fn exclusive_ownership_filters_weaker_writer() {
    let reader = mk_reader(311, OwnershipKind::Exclusive);
    let strong = [9u8; 16];
    let weak = [1u8; 16];
    // Strong writer kommt zuerst und wird Owner.
    reader
        .__push_raw_with_writer(b"strong".to_vec(), strong, 100)
        .unwrap();
    // Weak writer wird verworfen.
    reader
        .__push_raw_with_writer(b"weak".to_vec(), weak, 1)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 1, "Schwacher Writer muss gedropt werden");
    assert_eq!(out[0].data.as_slice(), b"strong");
}

#[test]
fn exclusive_ownership_stronger_writer_takes_over() {
    let reader = mk_reader(312, OwnershipKind::Exclusive);
    let weak = [1u8; 16];
    let strong = [9u8; 16];
    // Weak writer kommt zuerst, wird Owner.
    reader
        .__push_raw_with_writer(b"weak".to_vec(), weak, 10)
        .unwrap();
    // Strong writer kommt spaeter und uebernimmt.
    reader
        .__push_raw_with_writer(b"strong".to_vec(), strong, 100)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 2);
    // Beide Samples kommen durch — weak war der Owner zum Empfangs-
    // zeitpunkt, strong nimmt dann ueber.
    assert_eq!(out[0].data.as_slice(), b"weak");
    assert_eq!(out[1].data.as_slice(), b"strong");
}

#[test]
fn exclusive_ownership_tie_break_by_higher_guid() {
    let reader = mk_reader(313, OwnershipKind::Exclusive);
    let lower = [1u8; 16];
    let higher = [9u8; 16];
    // Lower-GUID writer kommt zuerst.
    reader
        .__push_raw_with_writer(b"lower".to_vec(), lower, 50)
        .unwrap();
    // Higher-GUID-Writer mit gleicher Strength gewinnt (Tie-Break).
    reader
        .__push_raw_with_writer(b"higher".to_vec(), higher, 50)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].data.as_slice(), b"lower");
    assert_eq!(out[1].data.as_slice(), b"higher");
}

#[test]
fn exclusive_ownership_lower_guid_at_tie_rejected() {
    let reader = mk_reader(314, OwnershipKind::Exclusive);
    let higher = [9u8; 16];
    let lower = [1u8; 16];
    // Higher kommt zuerst, wird Owner.
    reader
        .__push_raw_with_writer(b"higher".to_vec(), higher, 50)
        .unwrap();
    // Lower-GUID mit gleicher Strength → reject.
    reader
        .__push_raw_with_writer(b"lower".to_vec(), lower, 50)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 1, "Lower-GUID-Tie muss verworfen werden");
    assert_eq!(out[0].data.as_slice(), b"higher");
}

#[test]
fn exclusive_ownership_after_owner_lost_weaker_wins() {
    let reader = mk_reader(315, OwnershipKind::Exclusive);
    let strong = [9u8; 16];
    let weak = [1u8; 16];
    reader
        .__push_raw_with_writer(b"strong".to_vec(), strong, 100)
        .unwrap();
    let _ = reader.take().unwrap();
    // Liveliness-Loss → Owner-Clear.
    let cleared = reader.notify_writer_liveliness_lost(strong);
    assert!(
        cleared >= 1,
        "Mindestens eine Instance haette geclearetzt sein muessen"
    );
    // Jetzt darf Weak gewinnen.
    reader
        .__push_raw_with_writer(b"weak".to_vec(), weak, 10)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 1, "Weak nach Owner-Clear muss durchkommen");
    assert_eq!(out[0].data.as_slice(), b"weak");
}
