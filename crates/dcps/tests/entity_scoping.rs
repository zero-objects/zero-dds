// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `get_status_changes` of one entity (DDS 1.4 §2.2.2.1.1.6), the errors of
//! `get_discovered_*_data` (§2.2.2.2.1.28, §2.2.2.2.1.30) and
//! `delete_contained_entities` of the participant, the publisher and the
//! subscriber (§2.2.2.2.1.18, §2.2.2.4.1.14, §2.2.2.5.2.14).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    PresentationAccessScope, PresentationQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos, Entity,
    HANDLE_NIL, InstanceHandle, PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

fn live_participant() -> (common::DomainLease, common::ParticipantGuard) {
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
    (lease, p)
}

/// §2.2.2.1.1.6: "The list of statuses returned by the get_status_changes
/// operation refers to the status that are triggered on the Entity itself and
/// does not include statuses that apply to contained entities."
#[test]
fn get_status_changes_reports_the_entity_itself() {
    let (_lease, p) = live_participant();
    let topic = p
        .create_topic::<RawBytes>("OwnStatuses", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reliable = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..Default::default()
    };
    let writer = publisher
        .create_datawriter::<RawBytes>(
            &topic,
            DataWriterQos {
                reliability: reliable,
                ..Default::default()
            },
        )
        .unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(
            &topic,
            DataReaderQos {
                reliability: reliable,
                ..Default::default()
            },
        )
        .unwrap();
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .unwrap();
    reader
        .wait_for_matched_publication(1, common::match_timeout())
        .unwrap();

    assert_ne!(
        writer.get_status_changes() & status::PUBLICATION_MATCHED,
        0,
        "the writer's own status"
    );
    assert_ne!(
        reader.get_status_changes() & status::SUBSCRIPTION_MATCHED,
        0,
        "the reader's own status"
    );
    // The contained entities' statuses are not those of the containers.
    for contained in [
        publisher.get_status_changes(),
        subscriber.get_status_changes(),
        p.get_status_changes(),
    ] {
        assert_eq!(
            contained & (status::PUBLICATION_MATCHED | status::SUBSCRIPTION_MATCHED),
            0
        );
    }
}

/// §2.2.2.2.1.28 and §2.2.2.2.1.30: a handle that is not that of a discovered
/// (and not ignored) participant or topic is PRECONDITION_NOT_MET.
#[test]
fn a_handle_that_is_no_discovered_entity_is_precondition_not_met() {
    let (_lease, p) = live_participant();
    let nobody = InstanceHandle::from_raw(0xFEED_FACE);
    assert!(matches!(
        p.get_discovered_participant_data(nobody),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        p.get_discovered_topic_data(nobody),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    // An ignored handle too.
    p.ignore_participant(nobody).unwrap();
    p.ignore_topic(nobody).unwrap();
    assert!(matches!(
        p.get_discovered_participant_data(nobody),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        p.get_discovered_topic_data(nobody),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    let _ = HANDLE_NIL;
}

/// §2.2.2.2.1.18, §2.2.2.4.1.14, §2.2.2.5.2.14: PRECONDITION_NOT_MET if a
/// contained entity is in a state where it cannot be deleted: a GROUP-scope
/// publisher inside a coherent set, a subscriber inside a group access.
/// Nothing is deleted then.
#[test]
fn delete_contained_entities_refuses_an_entity_that_cannot_be_deleted() {
    let (_lease, p) = live_participant();
    let topic = p
        .create_topic::<RawBytes>("CannotDelete", TopicQos::default())
        .unwrap();
    let group = PresentationQosPolicy {
        access_scope: PresentationAccessScope::Group,
        coherent_access: true,
        ordered_access: false,
    };
    let publisher = p.create_publisher(PublisherQos {
        presentation: group,
        ..Default::default()
    });
    let subscriber = p.create_subscriber(SubscriberQos {
        presentation: group,
        ..Default::default()
    });
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();

    publisher.begin_coherent_changes().unwrap();
    assert!(matches!(
        publisher.delete_contained_entities(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        p.delete_contained_entities(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    publisher.end_coherent_changes().unwrap();

    subscriber.begin_access().unwrap();
    assert!(matches!(
        subscriber.delete_contained_entities(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        p.delete_contained_entities(),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    // Nothing was deleted by the refused calls.
    assert!(writer.write(&RawBytes { data: vec![1] }).is_ok());
    assert!(reader.get_status_changes() < u32::MAX);
    subscriber.end_access().unwrap();

    p.delete_contained_entities().expect("now it can");
}

/// §2.2.2.2.1.18: the participant also deletes its ContentFilteredTopics.
#[test]
fn delete_contained_entities_deletes_a_content_filtered_topic() {
    let (_lease, p) = live_participant();
    let topic = p
        .create_topic::<RawBytes>("CftContained", TopicQos::default())
        .unwrap();
    let cft = p
        .create_contentfilteredtopic::<RawBytes>("CftContainedFiltered", &topic, "id > 1", vec![])
        .unwrap();
    assert!(p.lookup_topicdescription("CftContainedFiltered").is_some());
    p.delete_contained_entities().unwrap();
    assert!(p.lookup_topicdescription("CftContainedFiltered").is_none());
    assert!(matches!(
        p.delete_contentfilteredtopic(&cft),
        Err(DdsError::AlreadyDeleted)
    ));
}

/// §2.2.2.2.1.19 `assert_liveliness` on the DomainParticipant: it asserts the
/// liveliness of every writer with MANUAL_BY_PARTICIPANT. A writer of that
/// kind with a lease of 400 ms stays alive while the participant asserts
/// every 100 ms, and loses its liveliness when it stops.
#[test]
fn assert_liveliness_keeps_a_manual_by_participant_writer_alive() {
    use std::time::Duration;
    use zerodds_dcps::qos::{LivelinessKind, LivelinessQosPolicy};
    let (_lease, p) = live_participant();
    let topic = p
        .create_topic::<RawBytes>("AssertLiveliness", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(
            &topic,
            DataWriterQos {
                liveliness: LivelinessQosPolicy {
                    kind: LivelinessKind::ManualByParticipant,
                    lease_duration: zerodds_qos::Duration::from_millis(400),
                },
                ..Default::default()
            },
        )
        .unwrap();
    writer.write(&RawBytes { data: vec![1] }).unwrap();
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(100));
        p.assert_liveliness().unwrap();
    }
    assert_eq!(
        writer.get_liveliness_lost_status().unwrap().total_count,
        0,
        "the participant's assertions kept the writer alive"
    );
    // Without assertions the lease runs out.
    std::thread::sleep(Duration::from_millis(1200));
    assert!(writer.get_liveliness_lost_status().unwrap().total_count >= 1);
}
