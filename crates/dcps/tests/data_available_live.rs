// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `on_data_available` on live readers (DDS 1.4 §2.2.4.1, §2.2.4.2.6.1):
//! the listener is called when samples arrive, whether or not the
//! application waits for them. None of these tests calls `wait_for_data`
//! before the listener has been heard; the listener tests in
//! `listener_integration.rs` run offline and push samples by hand.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use zerodds_dcps::listener::{DataReaderListener, SubscriberListener};
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, Entity,
    InstanceHandle, PublisherQos, RawBytes, SubscriberQos, TopicQos,
};

/// RELIABLE KEEP_ALL on both sides, so every sample written arrives.
fn reliable_keep_all() -> (DataWriterQos, DataReaderQos) {
    let history = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    };
    let reliability = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..Default::default()
    };
    (
        DataWriterQos {
            reliability,
            history,
            ..Default::default()
        },
        DataReaderQos {
            reliability,
            history,
            ..Default::default()
        },
    )
}

#[derive(Default)]
struct Heard(AtomicU32);

impl DataReaderListener for Heard {
    fn on_data_available(&self, _reader: InstanceHandle) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl SubscriberListener for Heard {
    fn on_data_on_readers(&self, _subscriber: InstanceHandle) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Polls `heard` until it is non-zero or `window` is over.
fn heard_within(heard: &Heard, window: Duration) -> u32 {
    let until = Instant::now() + window;
    while Instant::now() < until {
        let n = heard.0.load(Ordering::SeqCst);
        if n > 0 {
            return n;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    heard.0.load(Ordering::SeqCst)
}

/// What the reader has once the samples are in, taken without waiting.
fn take_until(
    reader: &zerodds_dcps::subscriber::DataReader<RawBytes>,
    count: usize,
    window: Duration,
) -> Vec<u8> {
    let mut got = Vec::new();
    let until = Instant::now() + window;
    while got.len() < count && Instant::now() < until {
        if let Ok(samples) = reader.take() {
            got.extend(samples.into_iter().map(|s| s.data[0]));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    got
}

/// A reader in another participant of this process, a listener and no
/// wait: the listener is called for what arrives.
#[test]
fn a_live_readers_listener_hears_samples_nobody_waited_for() {
    let lease = common::lease_domain();
    let cfg = common::isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let (wq, rq) = reliable_keep_all();

    let pa = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("participant A"),
    );
    let pb = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
            .expect("participant B"),
    );
    let topic_a = pa
        .create_topic::<RawBytes>("DataAvailableLive", TopicQos::default())
        .expect("topic A");
    let topic_b = pb
        .create_topic::<RawBytes>("DataAvailableLive", TopicQos::default())
        .expect("topic B");
    let writer = pa
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_a, wq)
        .expect("writer");
    let reader = pb
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, rq)
        .expect("reader");
    let heard = Arc::new(Heard::default());
    reader.set_listener(Some(heard.clone()), status::DATA_AVAILABLE);
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");

    for n in 1..=5u8 {
        writer.write(&RawBytes::new(vec![n])).expect("write");
    }
    assert!(
        heard_within(&heard, Duration::from_secs(5)) > 0,
        "on_data_available was not called for five samples"
    );
    let mut got = take_until(&reader, 5, Duration::from_secs(5));
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 3, 4, 5]);
}

/// The writer and the reader in one participant: the samples reach the
/// reader without the wire, and the listener is called all the same.
#[test]
fn a_listener_hears_a_writer_of_its_own_participant() {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();
    let (wq, rq) = reliable_keep_all();

    let p = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                common::isolated_cfg(),
            )
            .expect("participant"),
    );
    let topic = p
        .create_topic::<RawBytes>("DataAvailableSameParticipant", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, rq)
        .expect("reader");
    let heard = Arc::new(Heard::default());
    reader.set_listener(Some(heard.clone()), status::DATA_AVAILABLE);
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, wq)
        .expect("writer");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");

    writer.write(&RawBytes::new(vec![7])).expect("write");
    assert!(
        heard_within(&heard, Duration::from_secs(5)) > 0,
        "on_data_available was not called for a same-participant sample"
    );
    assert_eq!(take_until(&reader, 1, Duration::from_secs(5)), vec![7]);
}

/// A listener set after the samples arrived hears of them: nothing was
/// consumed while no data listener was there.
#[test]
fn a_listener_set_later_hears_of_samples_that_came_before_it() {
    let lease = common::lease_domain();
    let cfg = common::isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let (wq, rq) = reliable_keep_all();

    let pa = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("participant A"),
    );
    let pb = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
            .expect("participant B"),
    );
    let topic_a = pa
        .create_topic::<RawBytes>("DataAvailableLater", TopicQos::default())
        .expect("topic A");
    let topic_b = pb
        .create_topic::<RawBytes>("DataAvailableLater", TopicQos::default())
        .expect("topic B");
    let writer = pa
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_a, wq)
        .expect("writer");
    let reader = pb
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic_b, rq)
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");
    writer.write(&RawBytes::new(vec![3])).expect("write");
    // The DATA_AVAILABLE status reads the reader's channel: set, the sample
    // is in, and no listener has been told yet.
    let until = Instant::now() + Duration::from_secs(5);
    while reader.get_status_changes() & status::DATA_AVAILABLE == 0 {
        assert!(Instant::now() < until, "the sample did not arrive");
        std::thread::sleep(Duration::from_millis(5));
    }

    let heard = Arc::new(Heard::default());
    reader.set_listener(Some(heard.clone()), status::DATA_AVAILABLE);
    assert!(
        heard_within(&heard, Duration::from_secs(5)) > 0,
        "a listener set after the sample arrived was not told of it"
    );
    assert_eq!(take_until(&reader, 1, Duration::from_secs(5)), vec![3]);
}

/// A subscriber listener for DATA_ON_READERS and no reader listener: the
/// subscriber is told (DDS 1.4 §2.2.4.4).
#[test]
fn a_subscriber_listener_hears_data_on_its_readers() {
    let lease = common::lease_domain();
    let cfg = common::isolated_cfg();
    let factory = DomainParticipantFactory::instance();
    let (wq, rq) = reliable_keep_all();

    let pa = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("participant A"),
    );
    let pb = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
            .expect("participant B"),
    );
    let topic_a = pa
        .create_topic::<RawBytes>("DataOnReadersLive", TopicQos::default())
        .expect("topic A");
    let topic_b = pb
        .create_topic::<RawBytes>("DataOnReadersLive", TopicQos::default())
        .expect("topic B");
    let writer = pa
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic_a, wq)
        .expect("writer");
    let subscriber = pb.create_subscriber(SubscriberQos::default());
    let heard = Arc::new(Heard::default());
    subscriber.set_listener(Some(heard.clone()), status::DATA_ON_READERS);
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic_b, rq)
        .expect("reader");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");

    writer.write(&RawBytes::new(vec![9])).expect("write");
    assert!(
        heard_within(&heard, Duration::from_secs(5)) > 0,
        "on_data_on_readers was not called"
    );
    assert_eq!(take_until(&reader, 1, Duration::from_secs(5)), vec![9]);
}

/// Echoes what its reader receives on its writer, from inside
/// `on_data_available`, and notes whether it ran on the listener
/// dispatcher's thread.
struct Echo {
    reader: std::sync::OnceLock<zerodds_dcps::subscriber::DataReader<RawBytes>>,
    writer: std::sync::OnceLock<zerodds_dcps::publisher::DataWriter<RawBytes>>,
    echoed: AtomicU32,
    failed: AtomicU32,
    on_dispatcher: AtomicU32,
}

impl DataReaderListener for Echo {
    fn on_data_available(&self, _reader: InstanceHandle) {
        if std::thread::current().name() == Some("zerodds-listeners") {
            self.on_dispatcher.fetch_add(1, Ordering::SeqCst);
        }
        let (Some(reader), Some(writer)) = (self.reader.get(), self.writer.get()) else {
            return;
        };
        for sample in reader.take().unwrap_or_default() {
            match writer.write(&sample) {
                Ok(_) => self.echoed.fetch_add(1, Ordering::SeqCst),
                Err(_) => self.failed.fetch_add(1, Ordering::SeqCst),
            };
        }
    }
}

/// A data listener runs on the thread that handed the reader its samples,
/// not on the listener dispatcher's, and a RELIABLE write from it does not
/// wait for the flow-control window: the thread may be the receive thread
/// that would process the acknowledgements. With a window of one byte
/// every echo would otherwise wait out `max_blocking_time` and fail.
#[test]
fn a_listener_writes_from_the_receive_thread_without_waiting_for_the_window() {
    let lease = common::lease_domain();
    let mut cfg = common::isolated_cfg();
    cfg.writer_unacked_window = 1;
    let factory = DomainParticipantFactory::instance();
    let (wq, rq) = reliable_keep_all();

    let pa = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                cfg.clone(),
            )
            .expect("participant A"),
    );
    let pb = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
            .expect("participant B"),
    );
    let ping_a = pa
        .create_topic::<RawBytes>("EchoPing", TopicQos::default())
        .expect("ping A");
    let pong_a = pa
        .create_topic::<RawBytes>("EchoPong", TopicQos::default())
        .expect("pong A");
    let ping_b = pb
        .create_topic::<RawBytes>("EchoPing", TopicQos::default())
        .expect("ping B");
    let pong_b = pb
        .create_topic::<RawBytes>("EchoPong", TopicQos::default())
        .expect("pong B");
    let pinger = pa
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&ping_a, wq.clone())
        .expect("pinger");
    let pongs = pa
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&pong_a, rq.clone())
        .expect("pong reader");
    let echo = Arc::new(Echo {
        reader: std::sync::OnceLock::new(),
        writer: std::sync::OnceLock::new(),
        echoed: AtomicU32::new(0),
        failed: AtomicU32::new(0),
        on_dispatcher: AtomicU32::new(0),
    });
    let _ = echo.writer.set(
        pb.create_publisher(PublisherQos::default())
            .create_datawriter::<RawBytes>(&pong_b, wq)
            .expect("echo writer"),
    );
    let _ = echo.reader.set(
        pb.create_subscriber(SubscriberQos::default())
            .create_datareader::<RawBytes>(&ping_b, rq)
            .expect("echo reader"),
    );
    echo.reader
        .get()
        .expect("echo reader")
        .set_listener(Some(echo.clone()), status::DATA_AVAILABLE);
    pinger
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("ping match");
    echo.writer
        .get()
        .expect("echo writer")
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("pong match");

    for n in 1..=20u8 {
        pinger.write(&RawBytes::new(vec![n])).expect("ping");
    }
    let mut got = take_until(&pongs, 20, Duration::from_secs(10));
    got.sort_unstable();
    assert_eq!(got, (1..=20u8).collect::<Vec<_>>(), "every ping came back");
    assert_eq!(
        echo.failed.load(Ordering::SeqCst),
        0,
        "no echo write failed"
    );
    assert_eq!(
        echo.on_dispatcher.load(Ordering::SeqCst),
        0,
        "the listener ran on the dispatcher's thread"
    );
}
