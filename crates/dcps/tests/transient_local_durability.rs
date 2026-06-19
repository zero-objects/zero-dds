//! Transient-Local Durability tests.
//!
//! Core semantics (OMG DDS 1.4 §2.2.3.4):
//! * **Volatile**: a late-joiner reader receives NO samples written
//!   before its match.
//! * **TransientLocal**: a late-joiner reader receives all samples
//!   still in the writer history cache (up to history depth).
//!
//! These tests run only on Linux because of multicast loopback.

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
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        SubscriberQos, TopicQos,
    };
    use zerodds_qos::DurabilityKind;

    use super::common::unique_domain;

    /// Helper: two participants in the same isolated multicast group.
    /// Returns RAII guards that are cleanly removed from the singleton
    /// via `factory.delete_participant` at test end — otherwise threads
    /// and sockets accumulate across test boundaries.
    fn two_participants(
        domain: i32,
    ) -> (
        super::common::ParticipantGuard,
        super::common::ParticipantGuard,
    ) {
        let factory = DomainParticipantFactory::instance();
        let cfg = super::common::isolated_cfg();
        let a = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("a");
        let b = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("b");
        (
            super::common::ParticipantGuard::new(a),
            super::common::ParticipantGuard::new(b),
        )
    }

    #[serial_test::serial(dcps)]
    #[test]
    fn transient_local_writer_delivers_history_to_late_joiner() {
        // Writer: TransientLocal. Writes 5 samples BEFORE the reader
        // joins. The reader joins late but must still receive all 5
        // samples.
        let (pub_p, sub_p) = two_participants(unique_domain(8));

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());

        let writer_qos = DataWriterQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            ..Default::default()
        };
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, writer_qos)
            .expect("writer");

        // Write 5 samples before any reader exists.
        for i in 0i32..5 {
            writer
                .write(&ShapeType::new(format!("C{i}"), i, i * 2, 30))
                .expect("write");
        }

        // Now the reader joins — also TransientLocal.
        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let reader_qos = DataReaderQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            ..Default::default()
        };
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, reader_qos)
            .expect("reader");

        // Discovery + match — 5 s budget.
        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("writer sees reader");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("reader sees writer");

        // Collect all 5 samples.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut received = Vec::new();
        while received.len() < 5 && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            received.extend(reader.take().expect("take"));
        }

        assert_eq!(
            received.len(),
            5,
            "TransientLocal should deliver all 5 pre-match samples, only got {}: {received:?}",
            received.len()
        );
        for i in 0..5 {
            let expected_color = format!("C{i}");
            assert!(
                received.iter().any(|s| s.color == expected_color),
                "missing historic sample {expected_color}, got {received:?}"
            );
        }
    }

    #[serial_test::serial(dcps)]
    #[test]
    fn volatile_writer_does_not_deliver_history_to_late_joiner() {
        // Volatile contrast: the late-joiner sees NO pre-match samples.
        let (pub_p, sub_p) = two_participants(unique_domain(8));

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        // Default QoS = Volatile.
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");

        // Write 5 samples before the reader exists.
        for i in 0i32..5 {
            writer
                .write(&ShapeType::new(format!("OLD{i}"), i, 0, 30))
                .expect("write");
        }

        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");

        // After the match we write a "NEW" sample. That should arrive,
        // the OLD samples should not.
        writer
            .write(&ShapeType::new("NEW", 99, 99, 30))
            .expect("write post-match");

        let _ = reader.wait_for_data(Duration::from_secs(2));
        let received = reader.take().expect("take");

        for s in &received {
            assert!(
                !s.color.starts_with("OLD"),
                "Volatile should deliver NO pre-match samples, but got {s:?}"
            );
        }
        assert!(
            received.iter().any(|s| s.color == "NEW"),
            "post-match NEW sample should arrive, got {received:?}"
        );
    }

    #[serial_test::serial(dcps)]
    #[test]
    fn transient_local_reader_rejects_volatile_writer() {
        // QoS compat: the reader requires TransientLocal, the writer
        // offers only Volatile → no match, wait_for_matched_publication
        // must time out.
        let (pub_p, sub_p) = two_participants(unique_domain(8));

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        // The writer stays Volatile (default).
        let _writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");

        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let reader_qos = DataReaderQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            ..Default::default()
        };
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, reader_qos)
            .expect("reader");

        // 2 s budget is enough — the mismatch must survive SPDP+SEDP but
        // not trigger a match.
        let result = reader.wait_for_matched_publication(1, Duration::from_secs(2));
        assert!(
            result.is_err(),
            "Volatile writer should NOT match a TransientLocal reader, \
             got result={result:?}"
        );
    }
}
