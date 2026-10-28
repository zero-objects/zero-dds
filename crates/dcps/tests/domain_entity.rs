// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DomainEntity (DDS 1.4 §2.2.2.1.2): "DomainEntity is the abstract base
//! class for all DCPS entities except for the DomainParticipant … a
//! DomainParticipant cannot contain another DomainParticipant". Everything a
//! participant creates is a DomainEntity of it, and no other participant is
//! among its entities.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, Entity,
    PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

#[test]
fn a_domain_participant_contains_no_domain_participant() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let q = factory.create_participant_offline(0, DomainParticipantQos::default());
    assert!(
        !p.contains_entity(q.instance_handle()),
        "another participant is not an entity of this one"
    );
    assert!(!q.contains_entity(p.instance_handle()));

    // What the participant creates is its entity and nobody else's.
    let topic = p
        .create_topic::<RawBytes>("DomainEntity", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    for handle in [
        topic.get_instance_handle(),
        publisher.get_instance_handle(),
        subscriber.get_instance_handle(),
        writer.get_instance_handle(),
        reader.get_instance_handle(),
    ] {
        assert!(p.contains_entity(handle), "{handle:?} belongs to p");
        assert!(
            !q.contains_entity(handle),
            "{handle:?} does not belong to q"
        );
    }
}
