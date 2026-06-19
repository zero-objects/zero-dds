// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E test for the exclusive-ownership filter in the DataReader.take()
//! path (DDS 1.4 §2.2.3.23 / §2.2.2.5.5).
//!
//! Verifies that:
//! - With `OwnershipKind::Shared` no filtering happens (all samples pass
//!   take()).
//! - With `OwnershipKind::Exclusive` the first received sample sets the
//!   writer as owner; samples from weaker writers are dropped; samples
//!   from stronger writers take over ownership; tie-break via GUID.

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
    assert_eq!(out.len(), 2, "shared ownership must not filter");
}

#[test]
fn exclusive_ownership_filters_weaker_writer() {
    let reader = mk_reader(311, OwnershipKind::Exclusive);
    let strong = [9u8; 16];
    let weak = [1u8; 16];
    // Strong writer arrives first and becomes owner.
    reader
        .__push_raw_with_writer(b"strong".to_vec(), strong, 100)
        .unwrap();
    // Weak writer is rejected.
    reader
        .__push_raw_with_writer(b"weak".to_vec(), weak, 1)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 1, "weaker writer must be dropped");
    assert_eq!(out[0].data.as_slice(), b"strong");
}

#[test]
fn exclusive_ownership_stronger_writer_takes_over() {
    let reader = mk_reader(312, OwnershipKind::Exclusive);
    let weak = [1u8; 16];
    let strong = [9u8; 16];
    // Weak writer arrives first, becomes owner.
    reader
        .__push_raw_with_writer(b"weak".to_vec(), weak, 10)
        .unwrap();
    // Strong writer arrives later and takes over.
    reader
        .__push_raw_with_writer(b"strong".to_vec(), strong, 100)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 2);
    // Both samples pass through — weak was the owner at receive time,
    // strong then takes over.
    assert_eq!(out[0].data.as_slice(), b"weak");
    assert_eq!(out[1].data.as_slice(), b"strong");
}

#[test]
fn exclusive_ownership_tie_break_by_higher_guid() {
    let reader = mk_reader(313, OwnershipKind::Exclusive);
    let lower = [1u8; 16];
    let higher = [9u8; 16];
    // Lower-GUID writer arrives first.
    reader
        .__push_raw_with_writer(b"lower".to_vec(), lower, 50)
        .unwrap();
    // Higher-GUID writer with equal strength wins (tie-break).
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
    // Higher arrives first, becomes owner.
    reader
        .__push_raw_with_writer(b"higher".to_vec(), higher, 50)
        .unwrap();
    // Lower GUID with equal strength → reject.
    reader
        .__push_raw_with_writer(b"lower".to_vec(), lower, 50)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 1, "lower-GUID tie must be rejected");
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
    // Liveliness loss → owner clear.
    let cleared = reader.notify_writer_liveliness_lost(strong);
    assert!(
        cleared >= 1,
        "at least one instance should have been cleared"
    );
    // Now weak is allowed to win.
    reader
        .__push_raw_with_writer(b"weak".to_vec(), weak, 10)
        .unwrap();
    let out = reader.take().unwrap();
    assert_eq!(out.len(), 1, "weak after owner clear must pass through");
    assert_eq!(out[0].data.as_slice(), b"weak");
}
