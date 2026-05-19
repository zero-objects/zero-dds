//! Transient-Local Durability Tests.
//!
//! Kernsemantik (OMG DDS 1.4 §2.2.3.4):
//! * **Volatile**: Late-joiner-Reader bekommt KEINE Samples, die vor
//!   seinem Match geschrieben wurden.
//! * **TransientLocal**: Late-joiner-Reader bekommt alle Samples, die
//!   noch im Writer-History-Cache liegen (bis History-Depth).
//!
//! Diese Tests laufen nur auf Linux wegen Multicast-Loopback.

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
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        SubscriberQos, TopicQos,
    };
    use zerodds_qos::DurabilityKind;

    use super::common::unique_domain;

    fn fast_cfg() -> RuntimeConfig {
        RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        }
    }

    /// Helper: zwei Participants auf einer Domain aufsetzen.
    fn two_participants(
        domain: i32,
    ) -> (
        zerodds_dcps::DomainParticipant,
        zerodds_dcps::DomainParticipant,
    ) {
        let factory = DomainParticipantFactory::instance();
        let a = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("a");
        let b = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("b");
        (a, b)
    }

    #[test]
    fn transient_local_writer_delivers_history_to_late_joiner() {
        // Writer: TransientLocal. Schreibt 5 Samples BEVOR der Reader
        // joined. Reader joined spaet, muss trotzdem alle 5 Samples
        // empfangen.
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

        // Schreibe 5 Samples bevor irgendein Reader da ist.
        for i in 0i32..5 {
            writer
                .write(&ShapeType::new(format!("C{i}"), i, i * 2, 30))
                .expect("write");
        }

        // Jetzt joined der Reader — auch TransientLocal.
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

        // Discovery + Match — 5 s Budget.
        writer
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("writer sees reader");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("reader sees writer");

        // Alle 5 Samples einsammeln.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut received = Vec::new();
        while received.len() < 5 && std::time::Instant::now() < deadline {
            let _ = reader.wait_for_data(Duration::from_millis(200));
            received.extend(reader.take().expect("take"));
        }

        assert_eq!(
            received.len(),
            5,
            "TransientLocal sollte alle 5 vor-match-Samples liefern, nur {} bekommen: {received:?}",
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

    #[test]
    fn volatile_writer_does_not_deliver_history_to_late_joiner() {
        // Volatile-Kontrast: late-joiner sieht KEINE pre-match Samples.
        let (pub_p, sub_p) = two_participants(unique_domain(8));

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        // Default-QoS = Volatile.
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");

        // Schreibe 5 Samples bevor Reader existiert.
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

        // Nach Match schreiben wir ein "NEW"-Sample. Das sollte kommen,
        // die OLD-Samples nicht.
        writer
            .write(&ShapeType::new("NEW", 99, 99, 30))
            .expect("write post-match");

        let _ = reader.wait_for_data(Duration::from_secs(2));
        let received = reader.take().expect("take");

        for s in &received {
            assert!(
                !s.color.starts_with("OLD"),
                "Volatile sollte KEINE pre-match Samples liefern, aber bekam {s:?}"
            );
        }
        assert!(
            received.iter().any(|s| s.color == "NEW"),
            "post-match NEW sample sollte ankommen, got {received:?}"
        );
    }

    #[test]
    fn transient_local_reader_rejects_volatile_writer() {
        // QoS-Compat: Reader fordert TransientLocal, Writer bietet nur
        // Volatile → kein Match, wait_for_matched_publication muss
        // Timeout werfen.
        let (pub_p, sub_p) = two_participants(unique_domain(8));

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        // Writer bleibt Volatile (Default).
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

        // 2 s Budget sind genug — Mismatch muss SPDP+SEDP ueberleben,
        // aber kein Match triggern.
        let result = reader.wait_for_matched_publication(1, Duration::from_secs(2));
        assert!(
            result.is_err(),
            "Volatile-Writer sollte TransientLocal-Reader NICHT matchen, \
             got result={result:?}"
        );
    }
}
