// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The fictitious persistence-service entities of DDS 1.4 §2.2.3.4: for a
//! TRANSIENT or PERSISTENT topic the service behaves as if a built-in
//! DataReader and DataWriter with the Topic QoS existed. A reader that comes
//! after the last application DataWriter is gone still gets the stored
//! samples, and an endpoint whose QoS is incompatible with the Topic's
//! changes its incompatible-QoS status.

#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use common::{ParticipantGuard, isolated_cfg, unique_domain};
use zerodds_dcps::psm_constants::qos_policy_id;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};
use zerodds_qos::{
    DurabilityKind, DurabilityQosPolicy, DurabilityServiceQosPolicy, HistoryKind, HistoryQosPolicy,
    LifespanQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
};

fn participant() -> ParticipantGuard {
    ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                unique_domain(),
                DomainParticipantQos::default(),
                isolated_cfg(),
            )
            .expect("participant"),
    )
}

fn transient_topic() -> TopicQos {
    TopicQos {
        durability: DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        },
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(100),
        },
        ..TopicQos::default()
    }
}

fn writer_qos(kind: DurabilityKind, reliability: ReliabilityKind) -> DataWriterQos {
    DataWriterQos {
        durability: DurabilityQosPolicy { kind },
        reliability: ReliabilityQosPolicy {
            kind: reliability,
            max_blocking_time: zerodds_qos::Duration::from_millis(100),
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        durability_service: DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: 16,
            ..Default::default()
        },
        // Deleting the writer must not dispose the instance: a disposed
        // instance with no writer left goes at once with the default
        // `service_cleanup_delay` of zero (§2.2.3.4).
        writer_data_lifecycle: zerodds_qos::WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances: false,
        },
        ..DataWriterQos::default()
    }
}

fn reader_qos(kind: DurabilityKind, reliability: ReliabilityKind) -> DataReaderQos {
    DataReaderQos {
        durability: DurabilityQosPolicy { kind },
        reliability: ReliabilityQosPolicy {
            kind: reliability,
            max_blocking_time: zerodds_qos::Duration::from_millis(100),
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

fn eventually(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

/// §2.2.3.4: the data of a TRANSIENT topic survives its DataWriter and a
/// reader that comes later gets it, with no application DataWriter around.
#[test]
fn a_reader_after_the_last_writer_gets_the_stored_samples() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("ServiceWriterSurvives", transient_topic())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter(
            &topic,
            writer_qos(DurabilityKind::Transient, ReliabilityKind::Reliable),
        )
        .unwrap();
    for byte in 1..=3u8 {
        writer.write(&RawBytes { data: vec![byte] }).unwrap();
    }
    publisher.delete_datawriter(&writer).unwrap();
    drop(writer);

    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(
            &topic,
            reader_qos(DurabilityKind::Transient, ReliabilityKind::Reliable),
        )
        .unwrap();
    let mut got = Vec::new();
    assert!(
        eventually(|| {
            got.extend(reader.take().unwrap());
            got.len() >= 3
        }),
        "the reader got {} samples",
        got.len()
    );
    let bytes: Vec<u8> = got.iter().map(|s| s.data[0]).collect();
    assert_eq!(bytes, vec![1, 2, 3]);
}

/// The service ends when its store holds nothing any more: LIFESPAN took the
/// samples, so a reader that comes then has no writer to match.
#[test]
fn the_service_writer_ends_with_the_content_of_the_store() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("ServiceWriterEnds", transient_topic())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let mut qos = writer_qos(DurabilityKind::Transient, ReliabilityKind::Reliable);
    qos.lifespan = LifespanQosPolicy {
        duration: zerodds_qos::Duration::from_millis(300),
    };
    let writer = publisher.create_datawriter(&topic, qos).unwrap();
    writer.write(&RawBytes { data: vec![1] }).unwrap();
    publisher.delete_datawriter(&writer).unwrap();
    drop(writer);

    let subscriber = p.create_subscriber(SubscriberQos::default());
    let early = subscriber
        .create_datareader::<RawBytes>(
            &topic,
            reader_qos(DurabilityKind::Transient, ReliabilityKind::Reliable),
        )
        .unwrap();
    assert!(
        eventually(|| early.matched_publication_count() == 1),
        "the persistence service writer serves the early reader"
    );
    // The sample expires after 300 ms; the service checks about once a second.
    assert!(
        eventually(|| early.matched_publication_count() == 0),
        "the service writer stayed after its store emptied"
    );
}

/// §2.2.3.4: "Incompatibilities between local DataReader/DataWriter entities
/// and the corresponding fictitious built-in transient/persistent entities
/// cause the REQUESTED_INCOMPATIBLE_QOS/OFFERED_INCOMPATIBLE_QOS status to
/// change." A VOLATILE writer offers less durability than the Topic's
/// fictitious reader requests; a reader that asks for RELIABLE meets a Topic
/// that offers BEST_EFFORT.
#[test]
fn an_endpoint_incompatible_with_the_topic_changes_its_status() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("ServiceIncompatible", transient_topic())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());

    let fine = publisher
        .create_datawriter(
            &topic,
            writer_qos(DurabilityKind::Transient, ReliabilityKind::Reliable),
        )
        .unwrap();
    assert_eq!(
        fine.get_offered_incompatible_qos_status()
            .unwrap()
            .total_count,
        0
    );

    let volatile = publisher
        .create_datawriter(
            &topic,
            writer_qos(DurabilityKind::Volatile, ReliabilityKind::Reliable),
        )
        .unwrap();
    let status = volatile.get_offered_incompatible_qos_status().unwrap();
    assert_eq!(status.total_count, 1);
    assert_eq!(status.last_policy_id, qos_policy_id::DURABILITY);

    // A Topic that offers BEST_EFFORT: a RELIABLE reader requests more.
    let best_effort_topic = p
        .create_topic::<RawBytes>(
            "ServiceIncompatibleReader",
            TopicQos {
                durability: DurabilityQosPolicy {
                    kind: DurabilityKind::Transient,
                },
                reliability: ReliabilityQosPolicy {
                    kind: ReliabilityKind::BestEffort,
                    max_blocking_time: zerodds_qos::Duration::from_millis(100),
                },
                ..TopicQos::default()
            },
        )
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(
            &best_effort_topic,
            reader_qos(DurabilityKind::Transient, ReliabilityKind::Reliable),
        )
        .unwrap();
    let status = reader.get_requested_incompatible_qos_status().unwrap();
    assert_eq!(status.total_count, 1);
    assert_eq!(status.last_policy_id, qos_policy_id::RELIABILITY);
}

/// A writer that is incompatible with the Topic "will not send its data to
/// the transient/persistent service": nothing of it reaches the store.
#[test]
fn an_incompatible_writer_does_not_feed_the_store() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("ServiceRefusesWriter", transient_topic())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    // TRANSIENT but BEST_EFFORT against a RELIABLE Topic.
    let writer = publisher
        .create_datawriter(
            &topic,
            writer_qos(DurabilityKind::Transient, ReliabilityKind::BestEffort),
        )
        .unwrap();
    assert_eq!(
        writer
            .get_offered_incompatible_qos_status()
            .unwrap()
            .total_count,
        1
    );
    writer.write(&RawBytes { data: vec![1] }).unwrap();
    let store = writer.durability_backend().unwrap();
    assert!(
        store
            .replay_for_topic("ServiceRefusesWriter")
            .unwrap()
            .is_empty(),
        "the refused writer stored nothing"
    );
}

/// §2.2.3.4 condition 2 counts "live" DataWriters: a writer that lost its
/// liveliness (§2.2.3.11) is not one. Two writers write the instance; the
/// automatic one disposes and unregisters it, the manual one falls silent:
/// when its lease runs out, no live writer is left and the instance goes
/// with the zero `service_cleanup_delay`.
#[test]
fn a_writer_that_lost_its_liveliness_is_no_live_writer() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("ServiceLostLiveliness", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let mut manual_qos = writer_qos(DurabilityKind::Transient, ReliabilityKind::Reliable);
    manual_qos.liveliness = zerodds_qos::LivelinessQosPolicy {
        kind: zerodds_qos::LivelinessKind::ManualByTopic,
        lease_duration: zerodds_qos::Duration::from_millis(300),
    };
    let mut automatic_qos = writer_qos(DurabilityKind::Transient, ReliabilityKind::Reliable);
    automatic_qos
        .writer_data_lifecycle
        .autodispose_unregistered_instances = true;
    let manual = publisher.create_datawriter(&topic, manual_qos).unwrap();
    let automatic = publisher.create_datawriter(&topic, automatic_qos).unwrap();
    manual.write(&RawBytes { data: vec![1] }).unwrap();
    automatic.write(&RawBytes { data: vec![2] }).unwrap();
    let store = manual.durability_backend().unwrap();

    automatic
        .unregister_instance(&RawBytes { data: vec![2] }, zerodds_dcps::HANDLE_NIL)
        .unwrap();
    assert_eq!(
        store
            .replay_for_topic("ServiceLostLiveliness")
            .unwrap()
            .len(),
        2,
        "the manual writer is still live"
    );
    assert!(
        eventually(|| store
            .replay_for_topic("ServiceLostLiveliness")
            .unwrap()
            .is_empty()),
        "the instance stayed after the only writer left lost its liveliness"
    );
}
