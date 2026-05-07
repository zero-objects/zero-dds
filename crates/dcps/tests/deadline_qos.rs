//! Deadline QoS Tests.
//!
//! Testet Counter-Semantik fuer `OFFERED_DEADLINE_MISSED_STATUS` und
//! `REQUESTED_DEADLINE_MISSED_STATUS` (OMG DDS 1.4 §2.2.4.2.9 + .11).
//!
//! Der Tick-Loop im DcpsRuntime-Event-Loop checkt alle ~20 ms
//! ob ein Writer/Reader in seinem Deadline-Fenster ein Sample gehabt hat.
//! Wenn nicht, inkrementiert er den entsprechenden Missed-Counter.
//!
//! **WP-3.2a (dieser Commit):** lokales Deadline-Monitoring + Counter-
//! Public-API. **WP-3.2b (Folge-Commit):** QoS-Compat-Check zwischen
//! Peers via SEDP (braucht Deadline-PID in Publication/Subscription-
//! BuiltinTopicData).

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
    // Default-Deadline = INFINITE. Auch wenn Writer nie schreibt darf
    // kein Missed-Counter hochgehen. Sonst waere die API-Shape kaputt.
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
        "INFINITE-Deadline darf NIEMALS inkrementieren"
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
        // Writer mit Deadline=150ms, keine Writes → Counter muss
        // innerhalb 1s mehrere Male hochgehen (jedes 150-ms-Fenster
        // inkrementiert genau einmal).
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

        // Ersten Write sofort machen, dann 1 s warten ohne weitere Writes.
        writer
            .write(&ShapeType::new("RED", 0, 0, 30))
            .expect("write");
        thread::sleep(Duration::from_millis(1000));

        let missed = writer.offered_deadline_missed_count();
        assert!(
            (3..=10).contains(&missed),
            "erwartet ~6-7 Missed bei 1000ms/150ms=6.67, got {missed}"
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
            "erwartet ~6-7 Missed bei 1000ms/150ms, got {missed}"
        );
    }

    #[test]
    fn writes_within_deadline_keep_counter_at_zero() {
        // Writer mit Deadline=500ms, Writes alle 100ms → Counter bleibt 0.
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

        // 10 Writes à 100 ms = 1 Sekunde, alle innerhalb Deadline.
        for i in 0..10 {
            writer
                .write(&ShapeType::new("RED", i, i, 30))
                .expect("write");
            thread::sleep(Duration::from_millis(100));
        }

        assert_eq!(
            writer.offered_deadline_missed_count(),
            0,
            "write-Rate (100ms) < Deadline (500ms) → KEINE Misses"
        );
    }
}
