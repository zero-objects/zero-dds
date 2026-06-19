//! Content-filter topic (closure-based).
//!
//! Spec OMG DDS 1.4 §2.2.2.5.4 `ContentFilteredTopic`: the reader
//! evaluates a filter per sample and only forwards matched samples.
//! Instead of the SQL-like expression from the spec, we use a
//! Rust closure (idiomatic, type-safe, no parser runtime).
//!
//! An SQL parser + SEDP propagation for cross-vendor compatibility
//! follow in the downstream SQL-filter extension.

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
fn filter_drops_samples_that_return_false() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(80, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("topic");
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader")
        // Only let through samples with an even first byte.
        .with_filter(|s| s.data.first().is_some_and(|b| b % 2 == 0));

    // Push 4 samples: 0x02 (pass), 0x03 (drop), 0x04 (pass), 0x07 (drop).
    reader.__push_raw(vec![0x02, 0xFF]).unwrap();
    reader.__push_raw(vec![0x03, 0xFF]).unwrap();
    reader.__push_raw(vec![0x04, 0xFF]).unwrap();
    reader.__push_raw(vec![0x07, 0xFF]).unwrap();

    let samples = reader.take().expect("take");
    assert_eq!(samples.len(), 2, "got {samples:?}");
    assert_eq!(samples[0].data[0], 0x02);
    assert_eq!(samples[1].data[0], 0x04);
}

#[test]
fn without_filter_all_samples_pass_through() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(81, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("topic");
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");

    reader.__push_raw(vec![1]).unwrap();
    reader.__push_raw(vec![2]).unwrap();
    reader.__push_raw(vec![3]).unwrap();

    let samples = reader.take().expect("take");
    assert_eq!(samples.len(), 3);
}

#[test]
fn filter_applies_also_to_read_peek() {
    // `read()` peeks (does not remove), but must also filter.
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(82, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("topic");
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader")
        .with_filter(|s| s.data.len() >= 2);

    reader.__push_raw(vec![0x01]).unwrap(); // dropped (len=1)
    reader.__push_raw(vec![0x01, 0x02]).unwrap(); // pass (len=2)
    reader.__push_raw(vec![0x01, 0x02, 0x03]).unwrap(); // pass (len=3)

    let peeked = reader.read().expect("read");
    assert_eq!(peeked.len(), 2);
    // Peek is non-destructive — a second read returns 2 again.
    let peeked2 = reader.read().expect("read2");
    assert_eq!(peeked2.len(), 2);
    // take then consumes for good.
    let taken = reader.take().expect("take");
    assert_eq!(taken.len(), 2);
    let after = reader.take().expect("take3");
    assert!(after.is_empty());
}
