// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `get_sample_lost_status` of the Subscriber (DDS 1.4 §2.2.2.5.2.12): the
//! SAMPLE_LOST status summed over the DataReaders it created.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use zerodds_dcps::{
    DataReaderQos, DdsError, DomainParticipantFactory, DomainParticipantQos, RawBytes,
    SubscriberQos, TopicQos,
};

#[test]
fn the_subscriber_sums_the_sample_lost_status_of_its_readers() {
    let lease = common::lease_domain();
    let p = common::ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                common::isolated_cfg(),
            )
            .unwrap(),
    );
    let topic_a = p
        .create_topic::<RawBytes>("LostA", TopicQos::default())
        .unwrap();
    let topic_b = p
        .create_topic::<RawBytes>("LostB", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let a = subscriber
        .create_datareader::<RawBytes>(&topic_a, DataReaderQos::default())
        .unwrap();
    let b = subscriber
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();
    // A reader of another Subscriber does not count.
    let other = p.create_subscriber(SubscriberQos::default());
    let c = other
        .create_datareader::<RawBytes>(&topic_a, DataReaderQos::default())
        .unwrap();

    let none = subscriber.get_sample_lost_status().unwrap();
    assert_eq!((none.total_count, none.total_count_change), (0, 0));

    let (rt, eid_a) = a.runtime_handle().expect("live reader");
    let (_, eid_b) = b.runtime_handle().expect("live reader");
    let (_, eid_c) = c.runtime_handle().expect("live reader");
    rt.record_sample_lost(eid_a, 3);
    rt.record_sample_lost(eid_b, 2);
    rt.record_sample_lost(eid_c, 7);

    let first = subscriber.get_sample_lost_status().unwrap();
    assert_eq!((first.total_count, first.total_count_change), (5, 5));
    // The look reset the change, not the total.
    let second = subscriber.get_sample_lost_status().unwrap();
    assert_eq!((second.total_count, second.total_count_change), (5, 0));
    // It reset the readers' own: nothing is new at the reader either.
    let reader_a = a.get_sample_lost_status().unwrap();
    assert_eq!(
        (reader_a.total_count, reader_a.total_count_change),
        (3, 0),
        "the total of the reader stays"
    );
    // The reader of the other Subscriber still has its change.
    let reader_c = c.get_sample_lost_status().unwrap();
    assert_eq!((reader_c.total_count, reader_c.total_count_change), (7, 7));

    rt.record_sample_lost(eid_b, 1);
    let third = subscriber.get_sample_lost_status().unwrap();
    assert_eq!((third.total_count, third.total_count_change), (6, 1));

    // A deleted Subscriber has no status.
    subscriber.delete_contained_entities().unwrap();
    p.delete_subscriber(&subscriber).unwrap();
    assert!(matches!(
        subscriber.get_sample_lost_status(),
        Err(DdsError::AlreadyDeleted)
    ));
}
