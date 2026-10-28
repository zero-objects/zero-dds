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
    use zerodds_dcps::runtime::TimeBasedFilterClock;
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        SubscriberQos, TopicQos,
    };
    use zerodds_qos::Duration as QosDuration;
    use zerodds_qos::{HistoryKind, HistoryQosPolicy, TimeBasedFilterQosPolicy};

    use super::common::unique_domain;

    /// KEEP_ALL: `min_separation_zero_passes_all_samples` and
    /// `min_separation_filters_close_samples_per_instance` write several
    /// samples to the same instance ("RED") and assert on how many
    /// TIME_BASED_FILTER admits — the default KEEP_LAST(1) would
    /// (correctly) evict everything TIME_BASED_FILTER did not already
    /// drop, since `take()` routes through the same HISTORY-enforcing
    /// cache as `take_with_info()`.
    fn unbounded_history() -> HistoryQosPolicy {
        HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        }
    }

    fn pair_with_qos(
        domain: i32,
        topic_name: &str,
        rqos: DataReaderQos,
    ) -> (
        zerodds_dcps::DataWriter<ShapeType>,
        zerodds_dcps::DataReader<ShapeType>,
    ) {
        pair_with_clock(domain, topic_name, rqos, TimeBasedFilterClock::default())
    }

    /// As [`pair_with_qos`], the subscriber's participant measuring its
    /// TIME_BASED_FILTER on `clock`.
    fn pair_with_clock(
        domain: i32,
        topic_name: &str,
        rqos: DataReaderQos,
        clock: TimeBasedFilterClock,
    ) -> (
        zerodds_dcps::DataWriter<ShapeType>,
        zerodds_dcps::DataReader<ShapeType>,
    ) {
        let factory = DomainParticipantFactory::instance();
        let cfg = super::common::isolated_cfg();
        let mut sub_cfg = cfg.clone();
        sub_cfg.reader_time_based_filter_clock = clock;
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), sub_cfg)
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
    #[serial_test::file_serial(dcps)]
    #[test]
    fn min_separation_zero_passes_all_samples() {
        let (writer, reader) = pair_with_qos(
            unique_domain(),
            "TbfZeroSquare",
            DataReaderQos {
                history: unbounded_history(),
                ..Default::default()
            },
        );

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
    #[serial_test::file_serial(dcps)]
    #[test]
    fn min_separation_filters_close_samples_per_instance() {
        let rqos = DataReaderQos {
            time_based_filter: TimeBasedFilterQosPolicy {
                minimum_separation: QosDuration::from_millis(200),
            },
            history: unbounded_history(),
            ..Default::default()
        };
        let (writer, reader) = pair_with_qos(unique_domain(), "TbfFilterSquare", rqos);

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
    #[serial_test::file_serial(dcps)]
    #[test]
    fn min_separation_is_per_instance() {
        let rqos = DataReaderQos {
            time_based_filter: TimeBasedFilterQosPolicy {
                minimum_separation: QosDuration::from_millis(500),
            },
            ..Default::default()
        };
        let (writer, reader) = pair_with_qos(unique_domain(), "TbfPerInstance", rqos);

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

    /// Five samples of one instance, written back to back with source
    /// timestamps a second apart, then taken.
    fn five_seconds_written_at_once(clock: TimeBasedFilterClock, topic: &str) -> usize {
        let rqos = DataReaderQos {
            time_based_filter: TimeBasedFilterQosPolicy {
                minimum_separation: QosDuration::from_millis(500),
            },
            history: unbounded_history(),
            ..Default::default()
        };
        let (writer, reader) = pair_with_clock(unique_domain(), topic, rqos, clock);
        let start = zerodds_dcps::get_current_time();
        for i in 0..5 {
            writer
                .write_w_timestamp(
                    &ShapeType::new("RED", i, i, 30),
                    zerodds_dcps::Time::new(start.sec + i, start.nanosec),
                )
                .expect("write");
        }
        let _ = reader.wait_for_data(Duration::from_secs(2));
        thread::sleep(Duration::from_millis(200));
        reader.take().expect("take").len()
    }

    #[serial_test::file_serial(dcps)]
    #[test]
    fn the_sender_clock_filters_on_the_source_timestamps() {
        // The default: the changes are a second apart where they were
        // made, so each is more than minimum_separation after the one
        // before, however close they arrive.
        assert_eq!(
            five_seconds_written_at_once(TimeBasedFilterClock::Sender, "TbfSenderClock"),
            5
        );
    }

    #[serial_test::file_serial(dcps)]
    #[test]
    fn the_receiver_clock_filters_on_the_arrival() {
        // They arrive within milliseconds: one per minimum_separation.
        assert_eq!(
            five_seconds_written_at_once(TimeBasedFilterClock::Receiver, "TbfReceiverClock"),
            1
        );
    }

    /// A reader with a TIME_BASED_FILTER of 300 ms, KEEP_ALL, `reliable`
    /// or BEST_EFFORT.
    fn filtered_reader(reliable: bool) -> DataReaderQos {
        DataReaderQos {
            time_based_filter: TimeBasedFilterQosPolicy {
                minimum_separation: QosDuration::from_millis(300),
            },
            history: unbounded_history(),
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: if reliable {
                    zerodds_qos::ReliabilityKind::Reliable
                } else {
                    zerodds_qos::ReliabilityKind::BestEffort
                },
                max_blocking_time: QosDuration::from_millis(100),
            },
            ..Default::default()
        }
    }

    /// The `x` of every sample takes of `reader` return until `want`
    /// arrived or `within` passed.
    fn take_xs(
        reader: &zerodds_dcps::DataReader<ShapeType>,
        want: usize,
        within: Duration,
    ) -> Vec<i32> {
        let deadline = std::time::Instant::now() + within;
        let mut xs = Vec::new();
        while xs.len() < want && std::time::Instant::now() < deadline {
            xs.extend(reader.take().expect("take").into_iter().map(|s| s.x));
            thread::sleep(Duration::from_millis(5));
        }
        xs
    }

    #[serial_test::file_serial(dcps)]
    #[test]
    fn a_reliable_reader_gets_the_last_filtered_sample_once_the_writer_is_quiet() {
        // DDS 1.4 §2.2.3.12: "In the case where the reliability QoS kind is
        // RELIABLE then in steady-state ... the system should guarantee
        // delivery the last sample to the DataReader."
        let (writer, reader) =
            pair_with_qos(unique_domain(), "TbfLastSample", filtered_reader(true));
        for i in 0..3 {
            writer
                .write(&ShapeType::new("RED", i, i, 30))
                .expect("write");
        }
        assert_eq!(take_xs(&reader, 1, Duration::from_secs(2)), vec![0]);
        assert_eq!(
            take_xs(&reader, 1, Duration::from_secs(2)),
            vec![2],
            "the last sample, not the one between"
        );
        assert!(take_xs(&reader, 1, Duration::from_millis(500)).is_empty());
    }

    #[serial_test::file_serial(dcps)]
    #[test]
    fn a_best_effort_reader_gets_no_filtered_sample() {
        let (writer, reader) =
            pair_with_qos(unique_domain(), "TbfLastSampleBe", filtered_reader(false));
        for i in 0..3 {
            writer
                .write(&ShapeType::new("RED", i, i, 30))
                .expect("write");
        }
        assert_eq!(take_xs(&reader, 1, Duration::from_secs(2)), vec![0]);
        assert!(take_xs(&reader, 1, Duration::from_millis(800)).is_empty());
    }
}
