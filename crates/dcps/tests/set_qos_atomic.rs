// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `set_qos` of a Publisher and of a Subscriber changes nothing when it fails
//! (DDS 1.4 §2.2.2.1.1.1: "The existing set of policies are only changed if
//! the set_qos operation succeeds … In all other cases, none of the policies
//! is modified").

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::qos::{EntityFactoryQosPolicy, PresentationAccessScope, PresentationQosPolicy};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

fn factory_off() -> EntityFactoryQosPolicy {
    EntityFactoryQosPolicy {
        autoenable_created_entities: false,
    }
}

/// A `set_qos` that changes ENTITY_FACTORY and the immutable PRESENTATION
/// at once is refused; ENTITY_FACTORY stays as it was, which a writer or
/// reader created afterwards shows: it is enabled.
#[test]
fn a_refused_set_qos_leaves_the_entity_factory_of_publisher_and_subscriber() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("SetQosAtomic", TopicQos::default())
        .unwrap();
    let group = PresentationQosPolicy {
        access_scope: PresentationAccessScope::Group,
        coherent_access: true,
        ordered_access: false,
    };

    let publisher = p.create_publisher(PublisherQos::default());
    let refused = publisher.set_qos(PublisherQos {
        entity_factory: factory_off(),
        presentation: group,
        ..PublisherQos::default()
    });
    assert!(
        matches!(refused, Err(DdsError::ImmutablePolicy { .. })),
        "{refused:?}"
    );
    assert!(
        publisher
            .get_qos()
            .entity_factory
            .autoenable_created_entities
    );
    assert_ne!(publisher.get_qos().presentation, group);
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert!(writer.is_enabled(), "ENTITY_FACTORY was not changed");
    // The same QoS without the refused part is accepted and takes effect.
    publisher
        .set_qos(PublisherQos {
            entity_factory: factory_off(),
            ..PublisherQos::default()
        })
        .unwrap();
    let later = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert!(!later.is_enabled());

    let subscriber = p.create_subscriber(SubscriberQos::default());
    let refused = subscriber.set_qos(SubscriberQos {
        entity_factory: factory_off(),
        presentation: group,
        ..SubscriberQos::default()
    });
    assert!(
        matches!(refused, Err(DdsError::ImmutablePolicy { .. })),
        "{refused:?}"
    );
    assert!(
        subscriber
            .get_qos()
            .entity_factory
            .autoenable_created_entities
    );
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert!(reader.is_enabled(), "ENTITY_FACTORY was not changed");
    subscriber
        .set_qos(SubscriberQos {
            entity_factory: factory_off(),
            ..SubscriberQos::default()
        })
        .unwrap();
    let later = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert!(!later.is_enabled());
}
