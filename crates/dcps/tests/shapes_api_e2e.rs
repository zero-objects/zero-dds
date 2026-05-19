//! Stufe-1 E2E-Test: zwei ZeroDDS-Participants tauschen `ShapeType`-
//! Samples über die Public-API. Validiert die komplette Kette
//! Factory → Participant → Pub/Sub → Writer/Reader **mit einem echten
//! XCDR2-kodierten Application-Typ** (nicht RawBytes).
//!
//! Linux-only wegen Multicast-Loopback-Limitierungen auf macOS.

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
        // Eigene Domain (30) — kollisionsfrei zu allen anderen Tests.
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

        // Discovery-Sync — 5 s deckt CI-Jitter.
        writer
            .wait_for_matched_subscription(1, common::match_timeout())
            .expect("writer sees sub");
        reader
            .wait_for_matched_publication(1, common::match_timeout())
            .expect("reader sees pub");

        // Referenz-Sample — wurde auch in den Wire-Tests validiert.
        let sent = ShapeType::new("RED", 42, 77, 30);
        writer.write(&sent).expect("write");

        // wait_for_data mit 3 s fuer Heartbeat/AckNack/Resend.
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
        // Testet, dass verschiedene "Instanzen" (unterschiedliche Farben
        // → unterschiedliche Keys in Vendor-ShapesDemo) alle ankommen.
        // Instance-Map im Reader kommt erst in v1.3; hier reicht uns
        // "alle N Samples werden zugestellt, Reihenfolge egal".
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
            .wait_for_matched_subscription(1, common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, common::match_timeout())
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

        // Sammle innerhalb 5 s alle 4 Samples.
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
        // Reihenfolge ist bei parallelen Writes nicht garantiert, also Set-Check.
        for sent in &sent_samples {
            assert!(
                received.contains(sent),
                "missing sample {sent:?} in {received:?}"
            );
        }
    }
}
