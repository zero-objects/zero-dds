//! Deadline QoS tests.
//!
//! Tests the counter semantics for `OFFERED_DEADLINE_MISSED_STATUS` and
//! `REQUESTED_DEADLINE_MISSED_STATUS` (OMG DDS 1.4 §2.2.4.2.9 + .11).
//!
//! The tick loop in the DcpsRuntime event loop checks every ~20 ms
//! whether a writer/reader has had a sample within its deadline window.
//! If not, it increments the corresponding missed counter.
//!
//! **WP-3.2a (this commit):** local deadline monitoring + counter
//! public API. **WP-3.2b (follow-up commit):** QoS compatibility check
//! between peers via SEDP (needs the deadline PID in the
//! Publication/Subscription BuiltinTopicData).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

#[path = "common/mod.rs"]
mod common;

use std::thread;
use std::time::Duration;

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
};

use common::unique_domain;

fn fast_cfg() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(10),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    }
}

#[test]
fn deadline_default_is_infinite_so_counter_stays_zero() {
    // Default deadline = INFINITE. Even if the writer never writes, no
    // missed counter may go up. Otherwise the API shape would be broken.
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(
            unique_domain(63),
            DomainParticipantQos::default(),
            fast_cfg(),
        )
        .expect("participant");
    let topic = p
        .create_topic::<ShapeType>("InfiniteDeadlineSquare", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<ShapeType>(&topic, DataWriterQos::default())
        .expect("writer");

    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        writer.offered_deadline_missed_count(),
        0,
        "INFINITE deadline must NEVER increment"
    );
}

#[cfg(target_os = "linux")]
mod linux {
    use super::common::unique_domain;
    use super::*;
    use zerodds_dcps::{DataReaderQos, SubscriberQos};
    use zerodds_qos::DeadlineQosPolicy;
    use zerodds_qos::Duration as QosDuration;

    #[test]
    fn offered_deadline_increments_when_writer_misses_period() {
        // Writer with deadline=150ms, no writes → the counter must
        // go up several times within 1s (each 150-ms window
        // increments exactly once).
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(
                unique_domain(60),
                DomainParticipantQos::default(),
                fast_cfg(),
            )
            .expect("participant");
        let topic = p
            .create_topic::<ShapeType>("OfferedDeadlineSquare", TopicQos::default())
            .expect("topic");
        let publisher = p.create_publisher(PublisherQos::default());

        let qos = DataWriterQos {
            deadline: DeadlineQosPolicy {
                period: QosDuration::from_millis(150),
            },
            ..Default::default()
        };
        let writer = publisher
            .create_datawriter::<ShapeType>(&topic, qos)
            .expect("writer");

        // Do the first write immediately, then wait 1 s without further writes.
        writer
            .write(&ShapeType::new("RED", 0, 0, 30))
            .expect("write");
        thread::sleep(Duration::from_millis(1000));

        let missed = writer.offered_deadline_missed_count();
        assert!(
            (3..=10).contains(&missed),
            "expected ~6-7 missed at 1000ms/150ms=6.67, got {missed}"
        );
    }

    #[test]
    fn requested_deadline_increments_when_reader_misses_period() {
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(
                unique_domain(61),
                DomainParticipantQos::default(),
                fast_cfg(),
            )
            .expect("participant");
        let topic = p
            .create_topic::<ShapeType>("RequestedDeadlineSquare", TopicQos::default())
            .expect("topic");
        let subscriber = p.create_subscriber(SubscriberQos::default());

        let qos = DataReaderQos {
            deadline: DeadlineQosPolicy {
                period: QosDuration::from_millis(150),
            },
            ..Default::default()
        };
        let reader = subscriber
            .create_datareader::<ShapeType>(&topic, qos)
            .expect("reader");

        thread::sleep(Duration::from_millis(1000));

        let missed = reader.requested_deadline_missed_count();
        assert!(
            (3..=10).contains(&missed),
            "expected ~6-7 missed at 1000ms/150ms, got {missed}"
        );
    }

    #[test]
    fn writes_within_deadline_keep_counter_at_zero() {
        // Writer with deadline=500ms, writes every 100ms → counter stays 0.
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(
                unique_domain(62),
                DomainParticipantQos::default(),
                fast_cfg(),
            )
            .expect("participant");
        let topic = p
            .create_topic::<ShapeType>("WithinDeadlineSquare", TopicQos::default())
            .expect("topic");
        let publisher = p.create_publisher(PublisherQos::default());

        let qos = DataWriterQos {
            deadline: DeadlineQosPolicy {
                period: QosDuration::from_millis(500),
            },
            ..Default::default()
        };
        let writer = publisher
            .create_datawriter::<ShapeType>(&topic, qos)
            .expect("writer");

        // 10 writes at 100 ms = 1 second, all within the deadline.
        for i in 0..10 {
            writer
                .write(&ShapeType::new("RED", i, i, 30))
                .expect("write");
            thread::sleep(Duration::from_millis(100));
        }

        assert_eq!(
            writer.offered_deadline_missed_count(),
            0,
            "write rate (100ms) < deadline (500ms) → NO misses"
        );
    }
}
