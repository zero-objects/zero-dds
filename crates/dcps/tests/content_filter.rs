//! Content-Filter Topic (Closure-basiert).
//!
//! Spec OMG DDS 1.4 §2.2.2.5.4 `ContentFilteredTopic`: Reader evaluiert
//! einen Filter pro Sample und leitet nur gematchte Samples weiter.
//! Statt der SQL-artigen Expression aus der Spec nutzen wir eine
//! Rust-Closure (idiomatisch, typsicher, keine Parser-Runtime).
//!
//! SQL-Parser + SEDP-Propagation fuer Cross-Vendor-Kompatibilitaet
//! folgen in der nachgelagerten SQL-Filter-Erweiterung.

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
        // Nur Samples mit gerader erster Byte durchlassen.
        .with_filter(|s| s.data.first().is_some_and(|b| b % 2 == 0));

    // Pushe 4 Samples: 0x02 (pass), 0x03 (drop), 0x04 (pass), 0x07 (drop).
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
    // `read()` peekt (entfernt nicht), muss aber auch filtern.
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
    // Peek ist non-destructive — zweites read liefert wieder 2.
    let peeked2 = reader.read().expect("read2");
    assert_eq!(peeked2.len(), 2);
    // take konsumiert dann endgueltig.
    let taken = reader.take().expect("take");
    assert_eq!(taken.len(), 2);
    let after = reader.take().expect("take3");
    assert!(after.is_empty());
}
