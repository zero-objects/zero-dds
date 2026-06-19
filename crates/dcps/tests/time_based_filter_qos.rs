//! WP QoS-Wiring T1 — TimeBasedFilter QoS tests.
//!
//! Spec DDS 1.4 §2.2.3.13 TIME_BASED_FILTER: reader-side; only one
//! sample per `minimum_separation` per instance is delivered to the
//! user API; further samples within that window are dropped.

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

#[cfg(target_os = "linux")]
mod linux {
    use std::thread;
    use std::time::Duration;

    use zerodds_dcps::interop::ShapeType;
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        SubscriberQos, TopicQos,
    };
    use zerodds_qos::Duration as QosDuration;
    use zerodds_qos::TimeBasedFilterQosPolicy;

    use super::common::unique_domain;

    fn pair_with_qos(
        domain: i32,
        topic_name: &str,
        rqos: DataReaderQos,
    ) -> (
        zerodds_dcps::DataWriter<ShapeType>,
        zerodds_dcps::DataReader<ShapeType>,
    ) {
        let factory = DomainParticipantFactory::instance();
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");
        let pub_topic = pub_p
            .create_topic::<ShapeType>(topic_name, TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<ShapeType>(topic_name, TopicQos::default())
            .expect("sub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, rqos)
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("writer match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("reader match");
        (writer, reader)
    }
    #[serial_test::serial(dcps)]
    #[test]
    fn min_separation_zero_passes_all_samples() {
        let (writer, reader) =
            pair_with_qos(unique_domain(80), "TbfZeroSquare", DataReaderQos::default());

        for i in 0..5 {
            writer
                .write(&ShapeType::new("RED", i, i, 30))
                .expect("write");
            thread::sleep(Duration::from_millis(20));
        }
        let _ = reader.wait_for_data(Duration::from_secs(2));
        thread::sleep(Duration::from_millis(200));

        let samples = reader.take().expect("take");
        assert_eq!(
            samples.len(),
            5,
            "min_separation=0 may let everything through"
        );
    }
    #[serial_test::serial(dcps)]
    #[test]
    fn min_separation_filters_close_samples_per_instance() {
        let rqos = DataReaderQos {
            time_based_filter: TimeBasedFilterQosPolicy {
                minimum_separation: QosDuration::from_millis(200),
            },
            ..Default::default()
        };
        let (writer, reader) = pair_with_qos(unique_domain(80), "TbfFilterSquare", rqos);

        // 5 samples of the same instance at a 50-ms cadence = 250 ms total.
        for i in 0..5 {
            writer
                .write(&ShapeType::new("RED", i, i, 30))
                .expect("write");
            thread::sleep(Duration::from_millis(50));
        }
        let _ = reader.wait_for_data(Duration::from_secs(2));
        thread::sleep(Duration::from_millis(200));

        let samples = reader.take().expect("take");
        assert!(
            (1..=3).contains(&samples.len()),
            "minimum_separation=200ms with 5 writes every 50ms → 1-3 samples; got {}",
            samples.len()
        );
    }
    #[serial_test::serial(dcps)]
    #[test]
    fn min_separation_is_per_instance() {
        let rqos = DataReaderQos {
            time_based_filter: TimeBasedFilterQosPolicy {
                minimum_separation: QosDuration::from_millis(500),
            },
            ..Default::default()
        };
        let (writer, reader) = pair_with_qos(unique_domain(80), "TbfPerInstance", rqos);

        // Different keys = different instances. The filter is per instance.
        writer
            .write(&ShapeType::new("RED", 0, 0, 30))
            .expect("write red");
        writer
            .write(&ShapeType::new("BLUE", 0, 0, 30))
            .expect("write blue");
        let _ = reader.wait_for_data(Duration::from_secs(2));
        thread::sleep(Duration::from_millis(200));

        let samples = reader.take().expect("take");
        assert_eq!(samples.len(), 2, "different instances → no filtering");
    }
}
