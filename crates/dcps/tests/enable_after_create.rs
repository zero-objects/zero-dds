// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! An endpoint created disabled (ENTITY_FACTORY.autoenable_created_entities
//! FALSE, DDS 1.4 §2.2.3.20) is not discovered before its `enable`: "the
//! status of a disabled entity never changes" (§2.2.2.1.1.7), so nothing
//! may match it. Its `enable` announces it with the QoS it has then.
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};
use zerodds_dcps::factory::DomainParticipantFactoryQos;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DcpsParticipantBuiltinTopicData, DomainParticipantFactory,
    DomainParticipantQos, Entity, PublisherQos, RawBytes, SubscriberQos,
    TOPIC_NAME_DCPS_PARTICIPANT, TopicQos,
};

/// The factory's QoS is the process's: the test that changes it holds
/// this lock, and every test in this binary that creates a participant
/// holds it too.
static FACTORY_QOS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Polls `cond` until it holds or the match timeout passes.
fn eventually(cond: impl Fn() -> bool) -> bool {
    let deadline = std::time::Instant::now() + match_timeout();
    while std::time::Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

#[test]
fn a_writer_created_disabled_is_announced_by_its_enable_with_the_qos_it_has_then() {
    let _serial = FACTORY_QOS.lock();
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let a = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("participant a"),
    );
    let b = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("participant b"),
    );
    let topic_a = a
        .create_topic::<RawBytes>("EnableAfterCreate", TopicQos::default())
        .unwrap();
    let topic_b = b
        .create_topic::<RawBytes>("EnableAfterCreate", TopicQos::default())
        .unwrap();
    let mut pq = PublisherQos::default();
    pq.entity_factory.autoenable_created_entities = false;
    let publisher = a.create_publisher(pq);
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic_a, DataWriterQos::default())
        .unwrap();
    assert!(!writer.is_enabled());
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();

    // The reader is announced and the participants know each other; the
    // writer is not announced, so nothing matches it.
    let (rt, eid) = writer.runtime_handle().expect("live writer");
    assert!(rt.latest_announced_publication(eid).is_none());
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(reader.matched_publication_count(), 0);
    assert_eq!(
        b.runtime()
            .expect("live participant")
            .discovered_publications_count(),
        0,
        "participant b discovered a disabled writer"
    );

    // A QoS set before the enable is the QoS the writer is announced with.
    let mut wq = writer.get_qos();
    wq.deadline.period = zerodds_qos::Duration::from_millis(400);
    writer.set_qos(wq).unwrap();
    writer.enable().unwrap();
    reader
        .wait_for_matched_publication(1, match_timeout())
        .expect("the enabled writer matches");
    let announced = rt.latest_announced_publication(eid).expect("announced");
    assert_eq!(
        announced.deadline.period,
        zerodds_qos::Duration::from_millis(400)
    );
}

/// A participant its factory creates disabled sends no SPDP before its
/// `enable`; after it, a peer discovers it, and a `set_qos` of its
/// USER_DATA reaches the peer's DCPSParticipant reader.
#[test]
fn a_participant_created_disabled_is_discovered_after_its_enable() {
    let _serial = FACTORY_QOS.lock();
    let factory = DomainParticipantFactory::instance();
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let observer = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("observer"),
    );
    factory
        .set_qos(DomainParticipantFactoryQos {
            autoenable_created_entities: false,
        })
        .unwrap();
    let quiet =
        factory.create_participant_with_config(domain, DomainParticipantQos::default(), cfg);
    factory
        .set_qos(DomainParticipantFactoryQos::default())
        .unwrap();
    let quiet = ParticipantGuard::new(quiet.expect("quiet"));
    assert!(!quiet.is_enabled());

    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        observer.discovered_participants_count(),
        0,
        "a disabled participant was discovered"
    );

    quiet.enable().unwrap();
    assert!(
        eventually(|| observer.discovered_participants_count() == 1),
        "the enabled participant was not discovered"
    );

    let mut qos = quiet.get_qos();
    qos.user_data.value = vec![8, 9];
    quiet.set_qos(qos).unwrap();
    let builtin = observer
        .get_builtin_subscriber()
        .lookup_datareader::<DcpsParticipantBuiltinTopicData>(TOPIC_NAME_DCPS_PARTICIPANT)
        .unwrap();
    assert!(
        eventually(|| builtin
            .read()
            .unwrap_or_default()
            .iter()
            .any(|p| p.user_data == [8, 9])),
        "the changed USER_DATA never reached the peer"
    );
}
