// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TRANSIENT and PERSISTENT store (DDS 1.4 §2.2.3.4, §2.2.3.5,
//! §2.2.3.16): it outlives the writer that filled it, it keeps an instance
//! until the instance is disposed, no live writer writes it and
//! `service_cleanup_delay` has run out, and it drops a sample whose LIFESPAN
//! has expired.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::{
    DataWriterQos, DomainParticipant, DomainParticipantFactory, DomainParticipantQos, HANDLE_NIL,
    PublisherQos, RawBytes, TopicQos,
};
use zerodds_qos::{
    DurabilityKind, DurabilityQosPolicy, DurabilityServiceQosPolicy, LifespanQosPolicy,
    WriterDataLifecycleQosPolicy,
};

fn participant() -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

fn transient(service: DurabilityServiceQosPolicy, autodispose: bool) -> DataWriterQos {
    DataWriterQos {
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        },
        durability_service: service,
        writer_data_lifecycle: WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances: autodispose,
        },
        ..DataWriterQos::default()
    }
}

fn sample(byte: u8) -> RawBytes {
    RawBytes { data: vec![byte] }
}

/// §2.2.3.4: TRANSIENT data outlives its DataWriter. The second writer of
/// the topic gets the store the first one filled, not an empty one, and its
/// samples take the places after the first writer's.
#[test]
fn the_store_outlives_the_writer_that_filled_it() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("TransientAcrossWriters", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let qos = transient(DurabilityServiceQosPolicy::default(), false);

    let first = publisher.create_datawriter(&topic, qos.clone()).unwrap();
    let store = first.durability_backend().unwrap();
    first.write(&sample(1)).unwrap();
    publisher.delete_datawriter(&first).unwrap();
    drop(first);

    let second = publisher.create_datawriter(&topic, qos).unwrap();
    assert!(
        Arc::ptr_eq(&store, &second.durability_backend().unwrap()),
        "one store per topic of the participant"
    );
    assert_eq!(
        store
            .replay_for_topic("TransientAcrossWriters")
            .unwrap()
            .len(),
        1,
        "the sample of the deleted writer is still there"
    );
    second.write(&sample(2)).unwrap();
    let kept = store.replay_for_topic("TransientAcrossWriters").unwrap();
    assert_eq!(kept.len(), 1, "KEEP_LAST(1) of the one instance");
    assert_eq!(kept[0].payload, vec![2]);
    assert!(
        kept[0].sequence > 1,
        "the second writer's sample does not take the first one's sequence number"
    );
}

/// §2.2.3.4: an instance that is disposed and that no writer writes any
/// more leaves the store once `service_cleanup_delay` has run out; before
/// that it stays. With a delay of an hour it stays.
#[test]
fn a_disposed_unregistered_instance_stays_for_the_service_cleanup_delay() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("CleanupDelay", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());

    let held = DurabilityServiceQosPolicy {
        service_cleanup_delay: zerodds_qos::Duration::from_secs(3600),
        ..DurabilityServiceQosPolicy::default()
    };
    let writer = publisher
        .create_datawriter(&topic, transient(held, true))
        .unwrap();
    writer.write(&sample(1)).unwrap();
    writer.unregister_instance(&sample(1), HANDLE_NIL).unwrap();
    let store = writer.durability_backend().unwrap();
    assert_eq!(
        store.replay_for_topic("CleanupDelay").unwrap().len(),
        1,
        "the delay of an hour has not run out"
    );

    let gone = p
        .create_topic::<RawBytes>("CleanupNoDelay", TopicQos::default())
        .unwrap();
    let writer = publisher
        .create_datawriter(
            &gone,
            transient(DurabilityServiceQosPolicy::default(), true),
        )
        .unwrap();
    writer.write(&sample(1)).unwrap();
    assert_eq!(
        writer
            .durability_backend()
            .unwrap()
            .replay_for_topic("CleanupNoDelay")
            .unwrap()
            .len(),
        1,
        "a live instance is kept"
    );
    writer.unregister_instance(&sample(1), HANDLE_NIL).unwrap();
    assert!(
        writer
            .durability_backend()
            .unwrap()
            .replay_for_topic("CleanupNoDelay")
            .unwrap()
            .is_empty(),
        "disposed, unregistered, and the delay (zero) has run out"
    );
}

/// §2.2.3.4 without a dispose: an instance a writer only unregisters
/// (`autodispose_unregistered_instances` false) is not disposed, so the
/// store keeps it.
#[test]
fn an_instance_that_is_not_disposed_stays() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("NotDisposed", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter(
            &topic,
            transient(DurabilityServiceQosPolicy::default(), false),
        )
        .unwrap();
    writer.write(&sample(1)).unwrap();
    writer.unregister_instance(&sample(1), HANDLE_NIL).unwrap();
    assert_eq!(
        writer
            .durability_backend()
            .unwrap()
            .replay_for_topic("NotDisposed")
            .unwrap()
            .len(),
        1
    );
}

/// §2.2.3.16: a sample whose LIFESPAN has expired is removed from the
/// transient information cache.
#[test]
fn an_expired_sample_leaves_the_store() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StoreLifespan", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let mut qos = transient(DurabilityServiceQosPolicy::default(), false);
    qos.lifespan = LifespanQosPolicy {
        duration: zerodds_qos::Duration::from_millis(30),
    };
    let writer = publisher.create_datawriter(&topic, qos).unwrap();
    writer.write(&sample(1)).unwrap();
    let store = writer.durability_backend().unwrap();
    assert_eq!(store.replay_for_topic("StoreLifespan").unwrap().len(), 1);
    std::thread::sleep(Duration::from_millis(120));
    assert!(
        store.replay_for_topic("StoreLifespan").unwrap().is_empty(),
        "30 ms have passed"
    );
}

/// §2.2.3.4 condition 2: the information on an instance is kept while a
/// "live" DataWriter writes it. With two writers of the instance, the first
/// to dispose and unregister it does not start `service_cleanup_delay`; the
/// last one does.
#[test]
fn the_cleanup_delay_waits_for_the_last_writer_of_the_instance() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("CleanupTwoWriters", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let qos = transient(DurabilityServiceQosPolicy::default(), true);

    let first = publisher.create_datawriter(&topic, qos.clone()).unwrap();
    let second = publisher.create_datawriter(&topic, qos).unwrap();
    first.write(&sample(1)).unwrap();
    second.write(&sample(2)).unwrap();
    let store = first.durability_backend().unwrap();

    first.unregister_instance(&sample(1), HANDLE_NIL).unwrap();
    assert_eq!(
        store.replay_for_topic("CleanupTwoWriters").unwrap().len(),
        1,
        "the second writer still writes the instance"
    );
    second.unregister_instance(&sample(2), HANDLE_NIL).unwrap();
    assert!(
        store
            .replay_for_topic("CleanupTwoWriters")
            .unwrap()
            .is_empty(),
        "no live writer is left and the delay is zero"
    );
}

/// The same with a writer that is deleted instead of unregistering.
#[test]
fn a_deleted_writer_is_no_live_writer() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("CleanupDeletedWriter", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let qos = transient(DurabilityServiceQosPolicy::default(), true);

    let first = publisher.create_datawriter(&topic, qos.clone()).unwrap();
    let second = publisher.create_datawriter(&topic, qos).unwrap();
    first.write(&sample(1)).unwrap();
    second.write(&sample(2)).unwrap();
    let store = first.durability_backend().unwrap();

    publisher.delete_datawriter(&first).unwrap();
    assert_eq!(
        store
            .replay_for_topic("CleanupDeletedWriter")
            .unwrap()
            .len(),
        1,
        "the second writer still writes the instance"
    );
    publisher.delete_datawriter(&second).unwrap();
    assert!(
        store
            .replay_for_topic("CleanupDeletedWriter")
            .unwrap()
            .is_empty()
    );
}

fn keep_last(depth: i32) -> DurabilityServiceQosPolicy {
    DurabilityServiceQosPolicy {
        history_kind: zerodds_qos::HistoryKind::KeepLast,
        history_depth: depth,
        ..DurabilityServiceQosPolicy::default()
    }
}

/// How many of five writes the store of a topic keeps.
fn kept_of_five(
    topic_name: &str,
    topic_service: DurabilityServiceQosPolicy,
    writer_service: DurabilityServiceQosPolicy,
) -> usize {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>(
            topic_name,
            TopicQos {
                durability_service: topic_service,
                ..TopicQos::default()
            },
        )
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, transient(writer_service, false))
        .unwrap();
    for byte in 0..5u8 {
        writer.write(&sample(byte)).unwrap();
    }
    writer
        .durability_backend()
        .unwrap()
        .replay_for_topic(topic_name)
        .unwrap()
        .len()
}

/// §2.2.3.5 and §2.2.3.4: the fictitious entities of a topic are configured
/// from the Topic QoS; the store of the topic follows the Topic's
/// DURABILITY_SERVICE.
#[test]
fn the_store_follows_the_topics_durability_service() {
    // The Topic says KEEP_LAST(3); the writer leaves the default KEEP_LAST(1).
    assert_eq!(
        kept_of_five(
            "StoreTopicDepth",
            keep_last(3),
            DurabilityServiceQosPolicy::default()
        ),
        3
    );
}

/// A Topic that leaves DURABILITY_SERVICE at the default leaves the setting
/// to the writer: its value is the topic's in effect.
#[test]
fn a_default_topic_leaves_the_setting_to_the_writer() {
    assert_eq!(
        kept_of_five(
            "StoreWriterDepth",
            DurabilityServiceQosPolicy::default(),
            keep_last(2)
        ),
        2
    );
}

/// A writer whose DURABILITY_SERVICE differs from a Topic that sets one is
/// overridden: the Topic's applies (§2.2.3.4: "has its QoS configured from
/// the Topic QoS of the corresponding Topic").
#[test]
fn a_writer_that_differs_from_the_topic_is_overridden() {
    assert_eq!(
        kept_of_five("StoreOverridden", keep_last(4), keep_last(2)),
        4
    );
}
