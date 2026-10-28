// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `set_qos` once the entities are enabled (DDS 1.4 §2.2.3, the
//! "Changeable" policies), between two participants of one process. The
//! changed side announces its endpoint again, and both sides decide the
//! pair again under the new values.
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DcpsPublicationBuiltinTopicData, DomainParticipantFactory,
    DomainParticipantQos, Entity, PublisherQos, RawBytes, SubscriberQos,
    TOPIC_NAME_DCPS_PUBLICATION, TopicQos,
};
use zerodds_qos::PartitionQosPolicy;

fn pair() -> (ParticipantGuard, ParticipantGuard) {
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let a = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
        .expect("participant a");
    let b = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
        .expect("participant b");
    (ParticipantGuard::new(a), ParticipantGuard::new(b))
}

fn partition(name: &str) -> PartitionQosPolicy {
    PartitionQosPolicy {
        names: vec![name.into()],
    }
}

/// Polls `cond` until it holds or the match timeout passes.
fn eventually(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + match_timeout();
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

/// The publisher moves to a partition the subscriber is not in: both sides
/// drop the pair. The subscriber follows: both sides match it again.
#[test]
fn a_group_partition_change_unmatches_and_matches_again() {
    let (a, b) = pair();
    let topic_a = a
        .create_topic::<RawBytes>("SetQosPartition", TopicQos::default())
        .unwrap();
    let topic_b = b
        .create_topic::<RawBytes>("SetQosPartition", TopicQos::default())
        .unwrap();
    let publisher = a.create_publisher(PublisherQos {
        partition: partition("A"),
        ..PublisherQos::default()
    });
    let subscriber = b.create_subscriber(SubscriberQos {
        partition: partition("A"),
        ..SubscriberQos::default()
    });
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic_a, DataWriterQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();
    writer
        .wait_for_matched_subscription(1, match_timeout())
        .expect("first match");

    publisher
        .set_qos(PublisherQos {
            partition: partition("B"),
            ..PublisherQos::default()
        })
        .unwrap();
    assert!(
        eventually(
            || writer.matched_subscription_count() == 0 && reader.matched_publication_count() == 0
        ),
        "the pair is still matched after the publisher left the partition: \
         writer {}, reader {}",
        writer.matched_subscription_count(),
        reader.matched_publication_count()
    );
    assert_eq!(writer.offered_incompatible_qos_status().total_count, 0);

    subscriber
        .set_qos(SubscriberQos {
            partition: partition("B"),
            ..SubscriberQos::default()
        })
        .unwrap();
    writer
        .wait_for_matched_subscription(1, match_timeout())
        .expect("match in the new partition");
    reader
        .wait_for_matched_publication(1, match_timeout())
        .expect("match in the new partition, reader side");
}

/// The reader asks for a shorter DEADLINE than the writer offers: the pair
/// is RxO-incompatible now, and each side counts the other once
/// (DDS 1.4 §2.2.3.7, §2.2.4.1).
#[test]
fn a_reader_deadline_change_makes_the_pair_incompatible() {
    let (a, b) = pair();
    let topic_a = a
        .create_topic::<RawBytes>("SetQosDeadline", TopicQos::default())
        .unwrap();
    let topic_b = b
        .create_topic::<RawBytes>("SetQosDeadline", TopicQos::default())
        .unwrap();
    let mut wq = DataWriterQos::default();
    wq.deadline.period = zerodds_qos::Duration::from_millis(100);
    let writer = a
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_a, wq)
        .unwrap();
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();
    writer
        .wait_for_matched_subscription(1, match_timeout())
        .expect("first match");

    let mut rq = reader.get_qos();
    rq.deadline.period = zerodds_qos::Duration::from_millis(50);
    reader.set_qos(rq).unwrap();
    assert!(
        eventually(
            || writer.matched_subscription_count() == 0 && reader.matched_publication_count() == 0
        ),
        "the pair is still matched under an incompatible DEADLINE"
    );
    assert!(
        eventually(|| writer.offered_incompatible_qos_status().total_count == 1),
        "the writer did not count the reader once: {:?}, the reader: {:?}",
        writer.offered_incompatible_qos_status(),
        reader.requested_incompatible_qos_status()
    );
    assert_eq!(reader.requested_incompatible_qos_status().total_count, 1);
}

/// TOPIC_DATA set on the Topic reaches the announcement of a writer that
/// sets none of its own; a writer's own value stays (DDS 1.4 §2.2.3.2).
#[test]
fn a_topic_data_change_reaches_the_writers_announcement() {
    let (a, _b) = pair();
    let topic = a
        .create_topic::<RawBytes>("SetQosTopicData", TopicQos::default())
        .unwrap();
    let publisher = a.create_publisher(PublisherQos::default());
    let inheriting = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let mut own = DataWriterQos::default();
    own.topic_data.value = vec![1];
    let overriding = publisher
        .create_datawriter::<RawBytes>(&topic, own)
        .unwrap();

    let mut tq = topic.get_qos();
    tq.topic_data.value = vec![4, 2];
    topic.set_qos(tq).unwrap();

    let announced = |w: &zerodds_dcps::DataWriter<RawBytes>| {
        let (rt, eid) = w.runtime_handle().expect("live writer");
        rt.latest_announced_publication(eid)
            .expect("announced")
            .topic_data
    };
    assert_eq!(announced(&inheriting), vec![4, 2]);
    assert_eq!(announced(&overriding), vec![1]);
}

/// A writer's USER_DATA reaches the DCPSPublication reader of the other
/// participant (DDS 1.4 §2.2.5), and a `set_qos` of it arrives there as a
/// new sample.
#[test]
fn a_user_data_change_reaches_the_peers_builtin_reader() {
    let (a, b) = pair();
    let topic = a
        .create_topic::<RawBytes>("SetQosUserData", TopicQos::default())
        .unwrap();
    let mut wq = DataWriterQos::default();
    wq.user_data.value = vec![1, 2];
    let writer = a
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, wq)
        .unwrap();
    let builtin = b
        .get_builtin_subscriber()
        .lookup_datareader::<DcpsPublicationBuiltinTopicData>(TOPIC_NAME_DCPS_PUBLICATION)
        .unwrap();
    let seen = |value: &[u8]| {
        eventually(|| {
            builtin
                .read()
                .unwrap_or_default()
                .iter()
                .any(|s| s.topic_name == "SetQosUserData" && s.user_data.value == value)
        })
    };
    assert!(seen(&[1, 2]), "the first USER_DATA never reached the peer");

    let mut wq = writer.get_qos();
    wq.user_data.value = vec![3];
    writer.set_qos(wq).unwrap();
    assert!(seen(&[3]), "the changed USER_DATA never reached the peer");
}

/// DDS 1.4 §2.2.2.4.2 `get_matched_subscription_data` and §2.2.2.5.3
/// `get_matched_publication_data` across two participants: each side
/// reads the other's announced data, and a reader's `set_qos` reaches
/// the writer's copy.
#[test]
fn matched_data_across_participants_follows_a_set_qos() {
    let (a, b) = pair();
    let topic_a = a
        .create_topic::<RawBytes>("SetQosMatchedData", TopicQos::default())
        .unwrap();
    let topic_b = b
        .create_topic::<RawBytes>("SetQosMatchedData", TopicQos::default())
        .unwrap();
    let mut wq = DataWriterQos::default();
    wq.user_data.value = vec![4, 5];
    let writer = a
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_a, wq)
        .unwrap();
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();
    writer
        .wait_for_matched_subscription(1, match_timeout())
        .expect("match");
    reader
        .wait_for_matched_publication(1, match_timeout())
        .expect("match");

    let publications = reader.matched_publications().unwrap();
    assert_eq!(publications.len(), 1);
    let publication = reader.matched_publication_data(publications[0]).unwrap();
    assert_eq!(publication.topic_name, "SetQosMatchedData");
    assert_eq!(publication.user_data.value, vec![4, 5]);

    let subscriptions = writer.matched_subscriptions().unwrap();
    assert_eq!(subscriptions.len(), 1);
    let handle = subscriptions[0];
    assert_eq!(
        writer
            .matched_subscription_data(handle)
            .unwrap()
            .time_based_filter
            .minimum_separation,
        zerodds_qos::Duration::ZERO
    );

    let mut rq = reader.get_qos();
    rq.time_based_filter.minimum_separation = zerodds_qos::Duration::from_millis(50);
    reader.set_qos(rq).unwrap();
    assert!(
        eventually(|| writer
            .matched_subscription_data(handle)
            .is_ok_and(|s| s.time_based_filter.minimum_separation
                == zerodds_qos::Duration::from_millis(50))),
        "the writer's copy of the reader's data kept the old TIME_BASED_FILTER"
    );
}
