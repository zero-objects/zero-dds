//! Stage-1 E2E test: two ZeroDDS participants exchange `ShapeType`
//! samples via the public API. Validates the full chain
//! Factory → Participant → Pub/Sub → Writer/Reader **with a real
//! XCDR2-encoded application type** (not RawBytes).
//!
//! Linux-only because of multicast-loopback limitations on macOS.

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

#[cfg(target_os = "linux")]
#[path = "common/mod.rs"]
mod common;

#[cfg(target_os = "linux")]
mod linux {
    use std::time::Duration;

    use zerodds_dcps::interop::ShapeType;
    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DdsError, DomainParticipantFactory, DomainParticipantQos,
        PublisherQos, SubscriberQos, TopicQos,
    };

    use super::common::unique_domain;

    #[test]
    fn shape_type_roundtrip_through_full_dcps_stack() {
        // Dedicated domain (30) — collision-free with all other tests.
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(10);
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");

        let publisher = pub_p.create_publisher(PublisherQos::default());
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());

        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        // Discovery sync — 5 s covers CI jitter.
        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("writer sees sub");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("reader sees pub");

        // Reference sample — also validated in the wire tests.
        let sent = ShapeType::new("RED", 42, 77, 30);
        writer.write(&sent).expect("write");

        // wait_for_data with 3 s for Heartbeat/AckNack/Resend.
        match reader.wait_for_data(Duration::from_secs(3)) {
            Ok(()) => {}
            Err(DdsError::Timeout) => panic!("no sample arrived in 3 s"),
            Err(e) => panic!("wait_for_data failed: {e:?}"),
        }

        let samples = reader.take().expect("take");
        assert_eq!(samples.len(), 1, "expected exactly 1 sample");
        assert_eq!(samples[0], sent, "sample roundtrip broken");
    }

    #[test]
    fn multiple_colors_on_same_topic_all_delivered() {
        // Tests that different "instances" (different colors → different
        // keys in vendor ShapesDemo) all arrive. The instance map in the
        // reader only lands in v1.3; here it is enough that "all N samples
        // are delivered, order irrelevant".
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(10);
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Circle", TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<ShapeType>("Circle", TopicQos::default())
            .expect("sub topic");

        let publisher = pub_p.create_publisher(PublisherQos::default());
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());

        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");

        let sent_samples = [
            ShapeType::new("RED", 10, 20, 30),
            ShapeType::new("BLUE", 40, 50, 30),
            ShapeType::new("GREEN", 70, 80, 30),
            ShapeType::new("YELLOW", 100, 110, 30),
        ];
        for s in &sent_samples {
            writer.write(s).expect("write");
        }

        // Collect all 4 samples within 5 s.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut received = Vec::new();
        while received.len() < sent_samples.len() && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            received.extend(reader.take().expect("take"));
        }

        assert_eq!(
            received.len(),
            sent_samples.len(),
            "not all samples delivered: got {received:?}"
        );
        // Order is not guaranteed with parallel writes, so do a set check.
        for sent in &sent_samples {
            assert!(
                received.contains(sent),
                "missing sample {sent:?} in {received:?}"
            );
        }
    }
}
