// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! An endpoint created disabled (ENTITY_FACTORY.autoenable_created_entities
//! FALSE, DDS 1.4 §2.2.3.20) is not discovered before its `enable`: "the
//! status of a disabled entity never changes" (§2.2.2.1.1.7), so nothing
//! may match it. Its `enable` announces it with the QoS it has then. A
//! participant follows the factory's ENTITY_FACTORY as one value, also while
//! that changes, and keeps its own when a `set_qos` of it is refused.
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};
use zerodds_dcps::factory::DomainParticipantFactoryQos;
use zerodds_dcps::qos::PropertyQosPolicy;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DcpsParticipantBuiltinTopicData, DdsError,
    DomainParticipantFactory, DomainParticipantQos, Entity, PublisherQos, RawBytes, SubscriberQos,
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
fn dcps_2_2_2_4_1_5_a_writer_created_disabled_is_announced_by_its_enable_with_the_qos_it_has_then()
{
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
fn dcps_2_2_2_1_1_7_a_participant_created_disabled_is_discovered_after_its_enable() {
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

/// DDS 1.4 §2.2.3, ENTITY_FACTORY: RxO "No", across participants. The SEDP
/// publication and subscription data carry no ENTITY_FACTORY (DDSI-RTPS 2.5
/// has no parameter for it); the two participants of this process exchange
/// that data over the in-process path and match it against the SEDP cache
/// as for a remote participant (`run_matching_pass`).
/// Participant a has ENTITY_FACTORY FALSE, participant b the default. First
/// the writer on a in a Publisher with FALSE (its topic and publisher come
/// disabled and are enabled by hand; the publisher's enable leaves the
/// writer, which is enabled by hand) and the reader on b; then the writer
/// on b and the reader on a in a Subscriber with FALSE (the reader is
/// enabled by hand). Both pairs are announced and match on both sides, with
/// no incompatible-QoS status.
#[test]
fn dcps_2_2_3_20_entity_factory_of_participant_publisher_and_subscriber_is_not_compared_across_participants()
 {
    let _serial = FACTORY_QOS.lock();
    let cfg = isolated_cfg();
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let mut manual = DomainParticipantQos::default();
    manual.entity_factory.autoenable_created_entities = false;
    let a = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, manual, cfg.clone())
            .expect("participant a"),
    );
    let b = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("participant b"),
    );
    assert!(a.is_enabled() && b.is_enabled());

    // The writer on a.
    let topic_a = a
        .create_topic::<RawBytes>("EntityFactoryRxoWriterOnA", TopicQos::default())
        .unwrap();
    assert!(!topic_a.is_enabled());
    topic_a.enable().unwrap();
    let mut pq = PublisherQos::default();
    pq.entity_factory.autoenable_created_entities = false;
    let publisher = a.create_publisher(pq);
    assert!(!publisher.is_enabled());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic_a, DataWriterQos::default())
        .unwrap();
    assert!(!writer.is_enabled());
    publisher.enable().unwrap();
    assert!(
        !writer.is_enabled(),
        "the publisher's enable under FALSE enabled its writer"
    );
    writer.enable().unwrap();
    let topic_b = b
        .create_topic::<RawBytes>("EntityFactoryRxoWriterOnA", TopicQos::default())
        .unwrap();
    let reader = b
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();
    assert_pair_matches(&writer, &reader, "writer on a");

    // The reader on a, in a Subscriber with FALSE.
    let topic_b = b
        .create_topic::<RawBytes>("EntityFactoryRxoReaderOnA", TopicQos::default())
        .unwrap();
    let writer = b
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_b, DataWriterQos::default())
        .unwrap();
    let topic_a = a
        .create_topic::<RawBytes>("EntityFactoryRxoReaderOnA", TopicQos::default())
        .unwrap();
    topic_a.enable().unwrap();
    let mut sq = SubscriberQos::default();
    sq.entity_factory.autoenable_created_entities = false;
    let subscriber = a.create_subscriber(sq);
    assert!(!subscriber.is_enabled());
    subscriber.enable().unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic_a, DataReaderQos::default())
        .unwrap();
    assert!(!reader.is_enabled());
    reader.enable().unwrap();
    assert_pair_matches(&writer, &reader, "reader on a");
}

/// DDS 1.4 §2.2.2.1.1.1 on a DomainParticipant: "The existing set of
/// policies are only changed if the set_qos operation succeeds … In all
/// other cases, none of the policies is modified." A participant its
/// factory created disabled, with ENTITY_FACTORY TRUE, gets a `set_qos`
/// that changes ENTITY_FACTORY to FALSE together with a propagated PROPERTY
/// the SPDP beacon cannot carry (a value over `MAX_PROPERTY_STRING_LEN`,
/// 64 KiB). It is refused with BAD_PARAMETER, and ENTITY_FACTORY stays TRUE
/// for both readers of it: the participant's `enable` enables the topic,
/// publisher and subscriber it created before (§2.2.2.1.1.7), and a
/// publisher created afterwards is enabled (§2.2.3.20).
#[test]
fn dcps_2_2_2_1_1_1_a_refused_set_qos_leaves_the_entity_factory_of_the_participant() {
    let _serial = FACTORY_QOS.lock();
    let factory = DomainParticipantFactory::instance();
    factory
        .set_qos(DomainParticipantFactoryQos {
            autoenable_created_entities: false,
        })
        .unwrap();
    let created = factory.create_participant_with_config(
        unique_domain(),
        DomainParticipantQos::default(),
        isolated_cfg(),
    );
    factory
        .set_qos(DomainParticipantFactoryQos::default())
        .unwrap();
    let p = ParticipantGuard::new(created.expect("participant"));
    assert!(!p.is_enabled());
    assert!(p.runtime().is_some(), "the refusal needs a runtime");
    let topic = p
        .create_topic::<RawBytes>("RefusedParticipantSetQos", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    assert!(!topic.is_enabled() && !publisher.is_enabled() && !subscriber.is_enabled());

    let mut refused_qos = p.get_qos();
    refused_qos.entity_factory.autoenable_created_entities = false;
    refused_qos.property =
        PropertyQosPolicy::new().with_propagated("test.oversized", "x".repeat(64 * 1024 + 1));
    let refused = p.set_qos(refused_qos);
    assert!(
        matches!(refused, Err(DdsError::BadParameter { .. })),
        "{refused:?}"
    );
    assert!(p.get_qos().entity_factory.autoenable_created_entities);
    assert_eq!(p.get_qos().property, PropertyQosPolicy::default());

    p.enable().unwrap();
    assert!(topic.is_enabled(), "the enable left the topic");
    assert!(publisher.is_enabled(), "the enable left the publisher");
    assert!(subscriber.is_enabled(), "the enable left the subscriber");
    assert!(
        p.create_publisher(PublisherQos::default()).is_enabled(),
        "a publisher created after the refusal"
    );
}

/// Switches the factory's `autoenable_created_entities` between FALSE and
/// TRUE on its own thread until it drops; then puts the factory's QoS back.
struct FactorySwitcher {
    stop: Arc<AtomicBool>,
    switches: Arc<AtomicU32>,
    thread: Option<std::thread::JoinHandle<()>>,
    before: DomainParticipantFactoryQos,
}

impl FactorySwitcher {
    fn start() -> Self {
        let factory = DomainParticipantFactory::instance();
        let before = factory.get_qos();
        let stop = Arc::new(AtomicBool::new(false));
        let switches = Arc::new(AtomicU32::new(0));
        let thread = {
            let stop = Arc::clone(&stop);
            let switches = Arc::clone(&switches);
            std::thread::spawn(move || {
                let mut on = false;
                while !stop.load(Ordering::Relaxed) {
                    on = !on;
                    factory
                        .set_qos(DomainParticipantFactoryQos {
                            autoenable_created_entities: on,
                        })
                        .unwrap();
                    switches.fetch_add(1, Ordering::Relaxed);
                    std::thread::yield_now();
                }
            })
        };
        Self {
            stop,
            switches,
            thread: Some(thread),
            before,
        }
    }
}

impl Drop for FactorySwitcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = DomainParticipantFactory::instance().set_qos(self.before);
    }
}

/// DDS 1.4 §2.2.3.20 on the DomainParticipantFactory: the ENTITY_FACTORY a
/// creation reads decides both whether the participant comes enabled and
/// whether it announces itself (a disabled one sends no SPDP before its
/// `enable`, §2.2.2.1.1.7). While a second thread switches the factory's
/// value without pause, every participant created is enabled exactly when
/// its runtime is discoverable: it follows the old or the new value, not
/// one half of each.
#[test]
fn dcps_2_2_3_20_a_participant_created_while_the_factory_qos_changes_follows_one_value() {
    const CREATIONS: usize = 12;
    let _serial = FACTORY_QOS.lock();
    let factory = DomainParticipantFactory::instance();
    let domain = unique_domain();
    let switcher = FactorySwitcher::start();
    let mut split = Vec::new();
    let mut enabled_count = 0;
    for i in 0..CREATIONS {
        let p = ParticipantGuard::new(
            factory
                .create_participant_with_config(
                    domain,
                    DomainParticipantQos::default(),
                    isolated_cfg(),
                )
                .expect("participant"),
        );
        let enabled = p.is_enabled();
        let discoverable = p.runtime().expect("runtime").is_discoverable();
        if enabled != discoverable {
            split.push((i, enabled, discoverable));
        }
        enabled_count += usize::from(enabled);
    }
    let switches = switcher.switches.load(Ordering::Relaxed);
    drop(switcher);
    assert!(
        switches > 1,
        "the factory QoS did not change during the run"
    );
    assert!(
        split.is_empty(),
        "participants (index, enabled, discoverable) that follow two values: {split:?}; \
         {enabled_count} of {CREATIONS} enabled, {switches} switches"
    );
}

/// Both endpoints are announced over SEDP, match each other, and count no
/// incompatible QoS.
fn assert_pair_matches(
    writer: &zerodds_dcps::DataWriter<RawBytes>,
    reader: &zerodds_dcps::DataReader<RawBytes>,
    case: &str,
) {
    writer
        .wait_for_matched_subscription(1, match_timeout())
        .unwrap_or_else(|e| panic!("{case}: the writer did not match: {e:?}"));
    reader
        .wait_for_matched_publication(1, match_timeout())
        .unwrap_or_else(|e| panic!("{case}: the reader did not match: {e:?}"));
    let (writer_rt, writer_eid) = writer.runtime_handle().expect("live writer");
    assert!(
        writer_rt.latest_announced_publication(writer_eid).is_some(),
        "{case}: the writer was not announced"
    );
    let (reader_rt, reader_eid) = reader.runtime_handle().expect("live reader");
    assert!(
        reader_rt
            .latest_announced_subscription(reader_eid)
            .is_some(),
        "{case}: the reader was not announced"
    );
    assert_eq!(
        writer
            .get_offered_incompatible_qos_status()
            .unwrap()
            .total_count,
        0,
        "{case}"
    );
    assert_eq!(
        reader
            .get_requested_incompatible_qos_status()
            .unwrap()
            .total_count,
        0,
        "{case}"
    );
}
