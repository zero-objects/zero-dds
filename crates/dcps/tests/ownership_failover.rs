//! T9 — Liveliness-driven OWNERSHIP-Failover (Spec §2.2.3.23).
//!
//! Verifiziert die DataReader-Hooks `notify_writer_liveliness_lost`
//! und `notify_participant_liveliness_lost`, die im WLP-Pfad gerufen
//! werden, sobald ein Writer-/Participant-Lease abgelaufen ist und
//! die Failover-Selection neu greifen muss.

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

#[test]
fn notify_writer_liveliness_lost_clears_owner() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(220, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("OwTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();

    let it = reader.instance_tracker();
    let key = [0u8; 16];
    let _h = it.register(key, alloc::vec::Vec::new(), None);

    let strong = [9u8; 16];
    let weak = [1u8; 16];
    // Strong writer becomes owner.
    assert!(it.should_accept_sample_under_exclusive_ownership(&key, strong, 100));
    // Weak writer rejected while strong holds ownership.
    assert!(!it.should_accept_sample_under_exclusive_ownership(&key, weak, 10));
    // Liveliness-Lost hook on strong writer → owner cleared.
    assert_eq!(reader.notify_writer_liveliness_lost(strong), 1);
    // Now weak can win.
    assert!(it.should_accept_sample_under_exclusive_ownership(&key, weak, 10));
}

#[test]
fn notify_participant_liveliness_lost_clears_all_writers_with_prefix() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(221, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("OwTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();

    let it = reader.instance_tracker();
    let k1 = [1u8; 16];
    let k2 = [2u8; 16];
    let _ = it.register(k1, alloc::vec::Vec::new(), None);
    let _ = it.register(k2, alloc::vec::Vec::new(), None);

    // Two GUIDs sharing the same Participant prefix [7;12].
    let mut g1 = [0u8; 16];
    g1[..12].fill(7);
    g1[12..].copy_from_slice(&[0xA1, 0xA2, 0xA3, 0xA4]);
    let mut g2 = [0u8; 16];
    g2[..12].fill(7);
    g2[12..].copy_from_slice(&[0xB1, 0xB2, 0xB3, 0xB4]);

    assert!(it.should_accept_sample_under_exclusive_ownership(&k1, g1, 50));
    assert!(it.should_accept_sample_under_exclusive_ownership(&k2, g2, 50));

    // SPDP-Lease-Expiry → clear-by-prefix loescht beide Owner.
    let cleared = reader.notify_participant_liveliness_lost([7u8; 12]);
    assert_eq!(cleared, 2);

    // Schwaecherer Writer kann jetzt gewinnen.
    let weak = [1u8; 16];
    assert!(it.should_accept_sample_under_exclusive_ownership(&k1, weak, 1));
    assert!(it.should_accept_sample_under_exclusive_ownership(&k2, weak, 1));
}

#[test]
fn notify_writer_liveliness_lost_for_unknown_writer_is_noop() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(222, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("OwTopic", TopicQos::default())
        .unwrap();
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();

    let it = reader.instance_tracker();
    let key = [3u8; 16];
    let _ = it.register(key, alloc::vec::Vec::new(), None);
    let strong = [9u8; 16];
    assert!(it.should_accept_sample_under_exclusive_ownership(&key, strong, 100));

    // Unknown writer → kein Owner clear.
    let unknown = [42u8; 16];
    assert_eq!(reader.notify_writer_liveliness_lost(unknown), 0);
    // Original Owner immer noch aktiv.
    assert!(it.should_accept_sample_under_exclusive_ownership(&key, strong, 100));
    let weak = [1u8; 16];
    assert!(!it.should_accept_sample_under_exclusive_ownership(&key, weak, 10));
}

extern crate alloc;
