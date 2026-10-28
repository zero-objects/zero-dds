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
            unique_domain(),
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
                unique_domain(),
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

    /// DDS 1.4 §2.2.4.1: REQUESTED_DEADLINE_MISSED is missed "for a
    /// specific instance". A reader that received RED once and nothing
    /// after misses RED's deadline once per period, and names RED; a
    /// reader that received nothing has no instance to miss. It counted
    /// misses from its creation, with or without an instance.
    #[test]
    fn requested_deadline_is_missed_per_received_instance() {
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(
                unique_domain(),
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
            .create_datareader::<ShapeType>(&topic, qos.clone())
            .expect("reader");
        let other_topic = p
            .create_topic::<ShapeType>("RequestedDeadlineQuiet", TopicQos::default())
            .expect("topic");
        let quiet = subscriber
            .create_datareader::<ShapeType>(&other_topic, qos)
            .expect("reader");
        // DEADLINE is requested-offered (§2.2.3.7): a writer offering the
        // default, infinite period does not match a reader asking for
        // 150 ms.
        let writer = p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(
                &topic,
                DataWriterQos {
                    deadline: DeadlineQosPolicy {
                        period: QosDuration::from_millis(150),
                    },
                    ..Default::default()
                },
            )
            .expect("writer");

        // The sample has to reach the reader: before the pair matches it
        // goes nowhere, and a reader without an instance misses nothing.
        writer
            .wait_for_matched_subscription(1, Duration::from_secs(5))
            .expect("the writer matches the reader");
        let red = ShapeType::new("RED", 0, 0, 30);
        writer.write(&red).expect("write");
        thread::sleep(Duration::from_millis(1000));

        let status = reader
            .get_requested_deadline_missed_status()
            .expect("enabled");
        assert!(
            (3..=10).contains(&status.total_count),
            "expected ~6-7 missed at 1000ms/150ms, got {}",
            status.total_count
        );
        assert_eq!(status.last_instance_handle, reader.lookup_instance(&red));
        assert_eq!(
            quiet.requested_deadline_missed_count(),
            0,
            "a reader without an instance missed a deadline"
        );
    }

    /// DDS 1.4 §2.2.3.7: each instance is to be updated once per period.
    /// A writer that keeps BLUE fresh and leaves RED misses RED's
    /// deadline, and names RED. It watched the writer as a whole: BLUE's
    /// writes hid RED.
    #[test]
    fn offered_deadline_is_missed_per_instance() {
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(
                unique_domain(),
                DomainParticipantQos::default(),
                fast_cfg(),
            )
            .expect("participant");
        let topic = p
            .create_topic::<ShapeType>("OfferedDeadlinePerInstance", TopicQos::default())
            .expect("topic");
        let qos = DataWriterQos {
            deadline: DeadlineQosPolicy {
                period: QosDuration::from_millis(200),
            },
            ..Default::default()
        };
        let writer = p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(&topic, qos)
            .expect("writer");
        let red = ShapeType::new("RED", 0, 0, 30);
        writer.write(&red).expect("write");
        for i in 0..10 {
            writer
                .write(&ShapeType::new("BLUE", i, i, 30))
                .expect("write");
            thread::sleep(Duration::from_millis(100));
        }
        let status = writer
            .get_offered_deadline_missed_status()
            .expect("enabled");
        assert!(
            (2..=6).contains(&status.total_count),
            "RED's deadline: expected ~4 missed at 1000ms/200ms, got {}",
            status.total_count
        );
        assert_eq!(status.last_instance_handle, writer.lookup_instance(&red));
    }

    #[test]
    fn writes_within_deadline_keep_counter_at_zero() {
        // Writer with deadline=500ms, writes every 100ms → counter stays 0.
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant_with_config(
                unique_domain(),
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
