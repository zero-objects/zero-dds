// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `get_*_status` operations of a DataWriter and a DataReader (DDS
//! 1.4 §2.2.2.4.2, §2.2.2.5.3) and what reading does to a status
//! (§2.2.4.1): `total_count` is cumulative, `current_count` is what holds
//! now, the change fields count since the status was last read or handed
//! to a listener, and the last handle names the endpoint that changed it.
//!
//! Before, the matched statuses had `total_count` equal to the current
//! match count, no last handle, and no read operation; the C API wrote 0
//! into every change field.
//!
//! One participant: the pairs match through its own routes, without
//! discovery traffic.

#![cfg(feature = "std")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{ParticipantGuard, isolated_cfg, unique_domain};
use zerodds_dcps::qos::ReliabilityKind;
use zerodds_dcps::status::PublicationMatchedStatus;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, InstanceHandle,
    PublisherQos, RawBytes, SubscriberQos, TopicQos,
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

/// Polls `cond` for up to five seconds.
fn eventually(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cond()
}

#[test]
fn publication_matched_counts_every_reader_that_ever_matched() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsMatched", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());

    let first = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let first_handle = writer.matched_subscriptions().unwrap()[0];
    let status = writer.get_publication_matched_status().unwrap();
    assert_eq!(
        status,
        PublicationMatchedStatus {
            total_count: 1,
            total_count_change: 1,
            current_count: 1,
            current_count_change: 1,
            last_subscription_handle: first_handle,
        }
    );
    // The read took the changes.
    let again = writer.get_publication_matched_status().unwrap();
    assert_eq!(
        (again.total_count_change, again.current_count_change),
        (0, 0)
    );
    assert_eq!((again.total_count, again.current_count), (1, 1));

    drop(first);
    let status = writer.get_publication_matched_status().unwrap();
    assert_eq!(
        status,
        PublicationMatchedStatus {
            total_count: 1,
            total_count_change: 0,
            current_count: 0,
            current_count_change: -1,
            last_subscription_handle: first_handle,
        },
        "an unmatch leaves the total and lowers the current count"
    );

    let _second = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let second_handle = writer.matched_subscriptions().unwrap()[0];
    assert_ne!(second_handle, first_handle);
    let status = writer.get_publication_matched_status().unwrap();
    assert_eq!(
        (
            status.total_count,
            status.total_count_change,
            status.current_count,
            status.current_count_change,
            status.last_subscription_handle
        ),
        (2, 1, 1, 1, second_handle),
        "the second reader counts into the total"
    );
}

#[test]
fn subscription_matched_names_the_writer() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsSubscription", TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert_eq!(
        reader
            .get_subscription_matched_status()
            .unwrap()
            .total_count,
        0
    );
    let _writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let writer_handle = reader.matched_publications().unwrap()[0];
    let status = reader.get_subscription_matched_status().unwrap();
    assert_eq!(
        (
            status.total_count,
            status.total_count_change,
            status.current_count,
            status.last_publication_handle
        ),
        (1, 1, 1, writer_handle)
    );
}

/// A listener that took the status read it: a `get_*_status` after it
/// finds no change left. Without a listener the change waits for the read.
#[test]
fn a_listener_takes_the_changes_it_was_handed() {
    #[derive(Default)]
    struct Seen(Mutex<Vec<PublicationMatchedStatus>>);
    impl zerodds_dcps::listener::DataWriterListener for Seen {
        fn on_publication_matched(&self, _w: InstanceHandle, status: PublicationMatchedStatus) {
            self.0.lock().unwrap().push(status);
        }
    }

    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsListener", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let heard = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let seen = Arc::new(Seen::default());
    heard.set_listener(
        Some(seen.clone()),
        zerodds_dcps::psm_constants::status::PUBLICATION_MATCHED,
    );
    let unheard = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();

    let _reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    heard.drive_listeners();
    assert!(
        eventually(|| !seen.0.lock().unwrap().is_empty()),
        "on_publication_matched was not called"
    );
    let handed = seen.0.lock().unwrap()[0];
    assert_eq!((handed.total_count, handed.total_count_change), (1, 1));
    let after = heard.get_publication_matched_status().unwrap();
    assert_eq!(
        (
            after.total_count,
            after.total_count_change,
            after.current_count_change
        ),
        (1, 0, 0),
        "the listener's call was the read"
    );

    unheard.drive_listeners();
    let waiting = unheard.get_publication_matched_status().unwrap();
    assert_eq!(
        (waiting.total_count, waiting.total_count_change),
        (1, 1),
        "without a listener the change waits for the read"
    );
}

/// OFFERED_ and REQUESTED_INCOMPATIBLE_QOS of a refused same-participant
/// pair: counted once, the change taken by the read. The change field
/// stayed 0.
#[test]
fn incompatible_qos_is_counted_once_and_read_once() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsIncompatible", TopicQos::default())
        .unwrap();
    let mut wq = DataWriterQos::default();
    wq.reliability.kind = ReliabilityKind::BestEffort;
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, wq)
        .unwrap();
    let mut rq = DataReaderQos::default();
    rq.reliability.kind = ReliabilityKind::Reliable;
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, rq)
        .unwrap();

    assert!(writer.matched_subscriptions().unwrap().is_empty());
    let offered = writer.get_offered_incompatible_qos_status().unwrap();
    assert_eq!((offered.total_count, offered.total_count_change), (1, 1));
    let requested = reader.get_requested_incompatible_qos_status().unwrap();
    assert_eq!(
        (requested.total_count, requested.total_count_change),
        (1, 1)
    );
    assert_eq!(requested.last_policy_id, offered.last_policy_id);

    let offered = writer.get_offered_incompatible_qos_status().unwrap();
    assert_eq!((offered.total_count, offered.total_count_change), (1, 0));
}

/// §2.2.2.1.1.7 allows "get_status_changes and other get status operations
/// (although the status of a disabled entity never changes)" on a disabled
/// entity: the status is untriggered. Operations it does not list, such as
/// `matched_subscriptions`, return NOT_ENABLED.
#[test]
fn a_disabled_endpoint_has_an_untriggered_status() {
    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsDisabled", TopicQos::default())
        .unwrap();
    let mut pq = PublisherQos::default();
    pq.entity_factory.autoenable_created_entities = false;
    let writer = p
        .create_publisher(pq)
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert_eq!(
        writer.get_publication_matched_status().unwrap(),
        PublicationMatchedStatus::default()
    );
    assert_eq!(
        writer.get_liveliness_lost_status().unwrap(),
        zerodds_dcps::status::LivelinessLostStatus::default()
    );
    assert!(matches!(
        writer.matched_subscriptions(),
        Err(zerodds_dcps::DdsError::NotEnabled)
    ));
}

/// DDS 1.4 §2.2.4.1: the StatusChangedFlag of a status is set when the
/// status changes and reset when the application reads it, and a
/// StatusCondition triggers on it (§2.2.2.1.6). No status ever set the
/// flag: `get_status_changes` answered 0 and a StatusCondition never
/// triggered on a communication status.
#[test]
fn a_status_change_raises_the_flag_and_the_read_lowers_it() {
    use zerodds_dcps::Entity;
    use zerodds_dcps::psm_constants::status::PUBLICATION_MATCHED;

    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsFlag", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let condition = writer.get_status_condition();
    condition.set_enabled_statuses(PUBLICATION_MATCHED);
    assert!(!condition.trigger_value());

    let _reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert!(
        eventually(|| condition.trigger_value()),
        "the match did not raise PUBLICATION_MATCHED"
    );
    assert_ne!(writer.get_status_changes() & PUBLICATION_MATCHED, 0);

    writer.get_publication_matched_status().unwrap();
    assert!(
        !condition.trigger_value(),
        "the read did not lower the flag"
    );
    assert_eq!(writer.get_status_changes() & PUBLICATION_MATCHED, 0);
}

/// DDS 1.4 §2.2.2.4.2 `get_publisher`, §2.2.2.5.3 `get_subscriber`: the
/// entity that created the endpoint.
#[test]
fn an_endpoint_names_the_entity_that_created_it() {
    use zerodds_dcps::Entity;

    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsParent", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    assert_eq!(
        writer.get_publisher().get_instance_handle(),
        publisher.get_instance_handle()
    );
    assert_eq!(
        reader.get_subscriber().get_instance_handle(),
        subscriber.get_instance_handle()
    );
}

/// DATA_AVAILABLE on the reader and DATA_ON_READERS on its subscriber
/// (DDS 1.4 §2.2.4.1): set while the reader holds data not read yet, and
/// a StatusCondition triggers on them; a take brings both back. Neither
/// flag was ever set.
#[test]
fn data_available_raises_the_readers_and_its_subscribers_flags() {
    use zerodds_dcps::Entity;
    use zerodds_dcps::psm_constants::status::{DATA_AVAILABLE, DATA_ON_READERS};

    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsData", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let on_reader = reader.get_status_condition();
    on_reader.set_enabled_statuses(DATA_AVAILABLE);
    let on_subscriber = subscriber.get_status_condition();
    on_subscriber.set_enabled_statuses(DATA_ON_READERS);
    assert!(!on_reader.trigger_value());
    assert!(!on_subscriber.trigger_value());

    writer.write(&RawBytes::new(vec![1, 2, 3])).unwrap();
    assert!(
        eventually(|| on_reader.trigger_value()),
        "the sample did not raise DATA_AVAILABLE"
    );
    assert!(on_subscriber.trigger_value(), "DATA_ON_READERS stayed down");

    assert_eq!(reader.take().unwrap().len(), 1);
    assert!(
        !on_reader.trigger_value(),
        "the take left DATA_AVAILABLE up"
    );
    assert!(!on_subscriber.trigger_value());
}

/// DDS 1.4 §2.2.4.2.2: "The DATA_AVAILABLE StatusChangedFlag becomes FALSE
/// when either the corresponding listener operation (on_data_available) is
/// called or the read or take operation (or their variants) is called on
/// the associated DataReader." A `read_instance` of one instance lowers it
/// though the other instance's sample is unread; a later sample raises it
/// again. It stayed up while any sample was NOT_READ.
#[test]
fn a_read_of_one_instance_lowers_data_available() {
    use zerodds_dcps::Entity;
    use zerodds_dcps::interop::ShapeType;
    use zerodds_dcps::psm_constants::status::{DATA_AVAILABLE, DATA_ON_READERS};

    let p = participant();
    let topic = p
        .create_topic::<ShapeType>("StatusReadsInstances", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<ShapeType>(&topic, DataWriterQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<ShapeType>(&topic, DataReaderQos::default())
        .unwrap();
    let on_reader = reader.get_status_condition();
    on_reader.set_enabled_statuses(DATA_AVAILABLE);
    let on_subscriber = subscriber.get_status_condition();
    on_subscriber.set_enabled_statuses(DATA_ON_READERS);

    // RED becomes an instance of the reader.
    let red = ShapeType::new("RED", 1, 1, 10);
    writer.write(&red).unwrap();
    assert!(eventually(|| reader.read().unwrap().len() == 1));
    let handle = reader.lookup_instance(&red);
    assert!(!on_reader.trigger_value());

    writer.write(&ShapeType::new("RED", 2, 2, 10)).unwrap();
    writer.write(&ShapeType::new("BLUE", 2, 2, 10)).unwrap();
    assert!(eventually(|| on_reader.trigger_value()));
    // The read takes both samples into the cache and returns RED's; BLUE's
    // stays NOT_READ.
    assert_eq!(reader.read_instance(handle).unwrap().len(), 1);
    assert!(
        !on_reader.trigger_value(),
        "the read of one instance left DATA_AVAILABLE up"
    );
    assert!(!on_subscriber.trigger_value());

    writer.write(&ShapeType::new("RED", 3, 3, 10)).unwrap();
    assert!(
        eventually(|| on_reader.trigger_value()),
        "a new sample did not raise DATA_AVAILABLE"
    );
    assert!(on_subscriber.trigger_value());
}

/// DDS 1.4 §2.2.4.2.2: an `on_data_available` call lowers DATA_AVAILABLE
/// and the Subscriber's DATA_ON_READERS, whether the listener reads or not.
/// They stayed up until the application read.
#[test]
fn an_on_data_available_call_lowers_data_available() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use zerodds_dcps::Entity;
    use zerodds_dcps::listener::DataReaderListener;
    use zerodds_dcps::psm_constants::status::{DATA_AVAILABLE, DATA_ON_READERS};

    #[derive(Default)]
    struct Calls(AtomicUsize);
    impl DataReaderListener for Calls {
        fn on_data_available(&self, _reader: InstanceHandle) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let p = participant();
    let topic = p
        .create_topic::<RawBytes>("StatusReadsListener", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let calls = Arc::new(Calls::default());
    reader.set_listener(Some(calls.clone()), DATA_AVAILABLE);
    let on_reader = reader.get_status_condition();
    on_reader.set_enabled_statuses(DATA_AVAILABLE);
    let on_subscriber = subscriber.get_status_condition();
    on_subscriber.set_enabled_statuses(DATA_ON_READERS);

    writer.write(&RawBytes::new(vec![4])).unwrap();
    assert!(eventually(|| calls.0.load(Ordering::SeqCst) == 1));
    // The flags go down once the call returned.
    assert!(
        eventually(|| !on_reader.trigger_value()),
        "the listener call left DATA_AVAILABLE up"
    );
    assert!(eventually(|| !on_subscriber.trigger_value()));
    assert_eq!(calls.0.load(Ordering::SeqCst), 1);
    // The sample is still there to take.
    reader.set_listener(None, 0);
    assert_eq!(reader.take().unwrap().len(), 1);
}

/// DDS 1.4 §2.2.4.2.2: DATA_ON_READERS "becomes FALSE when any of the
/// following events occurs: … The read or take operation (or their
/// variants) is called on any DataReader belonging to the Subscriber." A
/// take on one reader lowers it while another reader's DATA_AVAILABLE stays
/// up; a sample on that other reader raises it again. It stayed up while any
/// reader had DATA_AVAILABLE.
#[test]
fn a_take_on_one_reader_lowers_data_on_readers() {
    use zerodds_dcps::Entity;
    use zerodds_dcps::psm_constants::status::{DATA_AVAILABLE, DATA_ON_READERS};

    let p = participant();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let mut pairs = Vec::new();
    for name in ["StatusReadsFirst", "StatusReadsSecond"] {
        let topic = p
            .create_topic::<RawBytes>(name, TopicQos::default())
            .unwrap();
        let writer = publisher
            .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
            .unwrap();
        let reader = subscriber
            .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
            .unwrap();
        pairs.push((writer, reader));
    }
    let on_subscriber = subscriber.get_status_condition();
    on_subscriber.set_enabled_statuses(DATA_ON_READERS);
    let second = pairs[1].1.get_status_condition();
    second.set_enabled_statuses(DATA_AVAILABLE);

    pairs[0].0.write(&RawBytes::new(vec![1])).unwrap();
    pairs[1].0.write(&RawBytes::new(vec![2])).unwrap();
    assert!(eventually(|| second.trigger_value()));
    assert!(on_subscriber.trigger_value());

    assert!(eventually(|| pairs[0].1.take().unwrap().len() == 1));
    assert!(
        !on_subscriber.trigger_value(),
        "the take on the first reader left DATA_ON_READERS up"
    );
    assert!(second.trigger_value(), "the second reader's flag went down");

    pairs[1].0.write(&RawBytes::new(vec![3])).unwrap();
    assert!(
        eventually(|| on_subscriber.trigger_value()),
        "a new sample on the second reader did not raise DATA_ON_READERS"
    );
}
