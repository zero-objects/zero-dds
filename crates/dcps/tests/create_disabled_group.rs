// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `create_disabled_publisher` and `create_disabled_subscriber`: a
//! Publisher and a Subscriber created not enabled on an enabled
//! participant whose ENTITY_FACTORY enables what it creates, so that a
//! policy that is immutable once enabled (PRESENTATION, DDS 1.4 §2.2.3.6)
//! can still be set before `enable` (the Publisher and Subscriber of a
//! DLRL Cache, DDS 1.2 §8.1.5.3).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::qos::PresentationAccessScope;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

#[test]
fn dcps_2_2_3_20_a_publisher_and_subscriber_created_disabled_take_presentation_before_enable() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    assert!(p.is_enabled());
    assert!(p.get_qos().entity_factory.autoenable_created_entities);
    let topic = p
        .create_topic::<RawBytes>("DisabledGroup", TopicQos::default())
        .unwrap();

    // The plain operations enable what they create.
    let plain = p.create_publisher(PublisherQos::default());
    assert!(plain.is_enabled());
    let mut group = PublisherQos::default();
    group.presentation.access_scope = PresentationAccessScope::Group;
    group.presentation.coherent_access = true;
    assert!(matches!(
        plain.set_qos(group.clone()),
        Err(DdsError::ImmutablePolicy { .. })
    ));

    // Not enabled: PRESENTATION changes, and the participant's QoS is
    // what it was.
    let publisher = p.create_disabled_publisher(PublisherQos::default());
    let subscriber = p.create_disabled_subscriber(SubscriberQos::default());
    assert!(!publisher.is_enabled());
    assert!(!subscriber.is_enabled());
    assert!(p.get_qos().entity_factory.autoenable_created_entities);
    publisher.set_qos(group.clone()).unwrap();
    let mut sgroup = SubscriberQos::default();
    sgroup.presentation.access_scope = PresentationAccessScope::Group;
    sgroup.presentation.coherent_access = true;
    subscriber.set_qos(sgroup.clone()).unwrap();

    // Their endpoints are created not enabled and cannot be enabled before
    // their factory (DDS 1.4 §2.2.2.1.1.7).
    let mut no_auto = publisher.get_qos();
    no_auto.entity_factory.autoenable_created_entities = false;
    publisher.set_qos(no_auto).unwrap();
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert!(!writer.is_enabled());
    assert!(!reader.is_enabled());
    assert!(matches!(
        writer.enable(),
        Err(DdsError::PreconditionNotMet { .. })
    ));

    // enable: the publisher with the QoS set before; the writer after it.
    publisher.enable().unwrap();
    assert!(publisher.is_enabled());
    assert!(!writer.is_enabled(), "its ENTITY_FACTORY enables nothing");
    assert_eq!(
        publisher.get_qos().presentation.access_scope,
        PresentationAccessScope::Group
    );
    assert!(matches!(
        publisher.set_qos(PublisherQos::default()),
        Err(DdsError::ImmutablePolicy { .. })
    ));
    writer.enable().unwrap();
    assert!(writer.is_enabled());
    // The subscriber's ENTITY_FACTORY enables its reader with it.
    subscriber.enable().unwrap();
    assert!(subscriber.is_enabled());
    assert!(reader.is_enabled());
    assert!(subscriber.get_qos().presentation.coherent_access);
    publisher.delete_contained_entities().unwrap();
    subscriber.delete_contained_entities().unwrap();
    p.delete_publisher(&publisher).unwrap();
    p.delete_subscriber(&subscriber).unwrap();
    p.delete_publisher(&plain).unwrap();
}
