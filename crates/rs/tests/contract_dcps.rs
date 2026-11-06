// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rs` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps` (`src/lib.rs`): the whole crate,
//! re-exported as `zerodds_rs::dcps` (`pub use zerodds_dcps as dcps`), and
//! at the top level `DataReader`, `DataReaderQos`, `DataWriter`,
//! `DataWriterQos`, `DdsError`, `DomainParticipant`,
//! `DomainParticipantFactory`, `DomainParticipantQos`, `Publisher`,
//! `PublisherQos`, `RawBytes`, `Result`, `Subscriber`, `SubscriberQos`,
//! `Topic`, `TopicQos`, `dds_type::DdsType` and
//! `instance_handle::InstanceHandle`. The crate documentation shows the
//! offline path: `DomainParticipantFactory::instance`,
//! `create_participant_offline`, `create_topic::<RawBytes>`,
//! `create_publisher`, `create_datawriter::<RawBytes>`.
//!
//! What it relies on: the re-exported items are the `zerodds-dcps` items
//! themselves, and the documented offline path runs without a network.
//!
//! Oracles: the identity of the re-exported items (`TypeId`), DDS 1.4
//! §2.2.2.2.2.3 (`get_instance` returns the singleton), the rustdoc of
//! `DataWriter::matched_subscription_count` ("Always 0 in offline mode") and
//! of `RawBytes` ("placed on the wire as-is, no CDR framing"). No socket is
//! opened.
#![allow(clippy::unwrap_used)]

use core::any::TypeId;

use zerodds_rs::{
    DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes,
    TopicQos,
};

#[test]
fn the_top_level_names_are_the_dcps_items() {
    assert_eq!(
        TypeId::of::<zerodds_rs::DomainParticipant>(),
        TypeId::of::<zerodds_dcps::DomainParticipant>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::DomainParticipantFactory>(),
        TypeId::of::<zerodds_dcps::DomainParticipantFactory>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::Publisher>(),
        TypeId::of::<zerodds_dcps::Publisher>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::Subscriber>(),
        TypeId::of::<zerodds_dcps::Subscriber>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::Topic<RawBytes>>(),
        TypeId::of::<zerodds_dcps::Topic<zerodds_dcps::RawBytes>>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::DataWriter<RawBytes>>(),
        TypeId::of::<zerodds_dcps::DataWriter<zerodds_dcps::RawBytes>>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::DataReader<RawBytes>>(),
        TypeId::of::<zerodds_dcps::DataReader<zerodds_dcps::RawBytes>>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::DdsError>(),
        TypeId::of::<zerodds_dcps::DdsError>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::Result<()>>(),
        TypeId::of::<core::result::Result<(), zerodds_dcps::DdsError>>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::InstanceHandle>(),
        TypeId::of::<zerodds_dcps::InstanceHandle>()
    );
    for (ours, theirs) in [
        (
            TypeId::of::<zerodds_rs::DomainParticipantQos>(),
            TypeId::of::<zerodds_dcps::DomainParticipantQos>(),
        ),
        (
            TypeId::of::<zerodds_rs::TopicQos>(),
            TypeId::of::<zerodds_dcps::TopicQos>(),
        ),
        (
            TypeId::of::<zerodds_rs::PublisherQos>(),
            TypeId::of::<zerodds_dcps::PublisherQos>(),
        ),
        (
            TypeId::of::<zerodds_rs::SubscriberQos>(),
            TypeId::of::<zerodds_dcps::SubscriberQos>(),
        ),
        (
            TypeId::of::<zerodds_rs::DataWriterQos>(),
            TypeId::of::<zerodds_dcps::DataWriterQos>(),
        ),
        (
            TypeId::of::<zerodds_rs::DataReaderQos>(),
            TypeId::of::<zerodds_dcps::DataReaderQos>(),
        ),
    ] {
        assert_eq!(ours, theirs);
    }
}

#[test]
fn the_dcps_module_is_zerodds_dcps_itself() {
    assert_eq!(
        TypeId::of::<zerodds_rs::dcps::sample_info::SampleInfo>(),
        TypeId::of::<zerodds_dcps::sample_info::SampleInfo>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::dcps::status::PublicationMatchedStatus>(),
        TypeId::of::<zerodds_dcps::status::PublicationMatchedStatus>()
    );
    // `DdsType` at the top level is the trait of `zerodds-dcps`: a type that
    // implements the one is accepted where the other is asked for.
    fn dcps_type<T: zerodds_dcps::DdsType>() -> &'static str {
        T::TYPE_NAME
    }
    fn facade_type<T: DdsType>() -> &'static str {
        dcps_type::<T>()
    }
    assert_eq!(facade_type::<RawBytes>(), zerodds_dcps::RawBytes::TYPE_NAME);
}

#[test]
fn dcps_2_2_2_2_2_3_the_factory_of_the_facade_is_the_dcps_singleton() {
    assert!(core::ptr::eq(
        DomainParticipantFactory::instance(),
        zerodds_dcps::DomainParticipantFactory::instance()
    ));
}

#[test]
fn the_documented_offline_path_creates_a_writer_without_a_network() {
    // The example of the crate documentation, step by step.
    let factory = DomainParticipantFactory::instance();
    let dp = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = dp
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .unwrap();
    assert_eq!(topic.name(), "Chatter");
    assert_eq!(topic.type_name(), RawBytes::TYPE_NAME);
    let publisher = dp.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert!(dp.runtime().is_none(), "offline: no runtime, no socket");
    assert_eq!(writer.matched_subscription_count(), 0);
    writer.write(&RawBytes::new(vec![1, 2, 3])).unwrap();
    // `RawBytes`: "Payload bytes (placed on the wire as-is, no CDR framing)".
    let mut body = Vec::new();
    RawBytes::new(vec![1, 2, 3]).encode(&mut body).unwrap();
    assert_eq!(body, [1, 2, 3]);
}
