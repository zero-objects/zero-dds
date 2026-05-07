//! Lifespan QoS Tests (Writer-side sample expiration).
//!
//! Spec OMG DDS 1.4 §2.2.3.16 LifespanQosPolicy: "if the duration
//! elapses and the sample is still in the writer's cache, the sample
//! is no longer available to any future DataReaders."
//!
//! Implementiert als Writer-seitiger Cache-Scan alle ~20 ms. Reader-
//! seitiger Lifespan-Filter (auf empfangene Samples) ist Reader-Lifespan-Track.
//!
//! Test-Strategie: Writer schreibt N Samples bei t=0 mit Lifespan
//! kuerzer als der Wartezeit, joined **spaet** der Reader. Bei korrekter
//! Expiration: Reader empfaengt nichts (oder nur Samples die nach dem
//! Match entstanden sind). Das funktioniert gut mit TransientLocal
//! Durability — sonst wuerde Volatile den Replay eh verhindern und
//! der Test wuerde nichts ueber Lifespan aussagen.

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
    use std::thread;
    use std::time::Duration;

    use zerodds_dcps::interop::ShapeType;
    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        SubscriberQos, TopicQos,
    };
    use zerodds_qos::Duration as QosDuration;
    use zerodds_qos::{DurabilityKind, LifespanQosPolicy};

    use super::common::unique_domain;

    fn fast_cfg() -> RuntimeConfig {
        RuntimeConfig {
            tick_period: Duration::from_millis(10),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        }
    }

    /// Wartet bis beide Participants sich auf SPDP-Ebene gegenseitig
    /// gesehen haben. Ohne diesen Warm-up startet der spaete Reader
    /// im Late-Joiner-Test mit kalter SPDP-Cache-Tabelle, dann muss
    /// SEDP-Match Discovery- + SEDP-Roundtrip in 5 s erledigen — auf
    /// stark ausgelasteten CI-Runnern reicht das nicht. Mit Warm-up
    /// ist nur noch SEDP zu uebertragen.
    fn wait_spdp_bidirectional(
        a: &zerodds_dcps::DomainParticipant,
        b: &zerodds_dcps::DomainParticipant,
        timeout: Duration,
    ) {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if a.discovered_participants_count() >= 1 && b.discovered_participants_count() >= 1 {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "SPDP-Warm-up Timeout: a={}, b={}",
            a.discovered_participants_count(),
            b.discovered_participants_count()
        );
    }

    #[test]
    fn lifespan_expires_samples_before_late_joiner_arrives() {
        // Setup: Writer mit Durability=TransientLocal + Lifespan=150ms.
        // Schreibt 3 Samples, wartet 1 s (alle expiren) → neuer
        // Reader joined → bekommt nichts.
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(5);
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());

        let wqos = DataWriterQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            lifespan: LifespanQosPolicy {
                duration: QosDuration::from_millis(150),
            },
            ..Default::default()
        };
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, wqos)
            .expect("writer");

        // SPDP bidirektional bootstrappen, bevor das Lifespan-Fenster
        // startet. Damit ist der spaete Reader-Match ein reiner SEDP-
        // Roundtrip und passt sicher in 5 s auf CI-Runnern unter Last.
        wait_spdp_bidirectional(&pub_p, &sub_p, Duration::from_secs(5));

        // 3 Samples im selben Moment.
        for i in 0i32..3 {
            writer
                .write(&ShapeType::new(format!("EXP{i}"), i, 0, 30))
                .expect("write");
        }

        // 1 s warten — alle Samples muessen expiren (150 ms Lifespan).
        thread::sleep(Duration::from_millis(1_000));

        // Jetzt kommt der Reader. Auch TransientLocal damit er
        // Replay erwarten wuerde, falls Lifespan nicht gezogen haette.
        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let rqos = DataReaderQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            ..Default::default()
        };
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, rqos)
            .expect("reader");

        // SEDP-DATA(r) fuer den spaeten Reader laeuft ueber Reliable-
        // RTPS mit `SEDP_HEARTBEAT_PERIOD = 500ms`. Bei Multicast-
        // Loss unter CI-Last braucht die Recovery mehrere HB-Zyklen
        // — 5 s reichen nicht reproduzierbar. 15 s deckt den
        // Worst-Case (3 dropped DATA + 3 HB-Recovery-Roundtrips).
        writer
            .wait_for_matched_subscription(1, Duration::from_secs(15))
            .expect("match");
        reader
            .wait_for_matched_publication(1, Duration::from_secs(15))
            .expect("match");

        // 500 ms sollten mehr als genug sein fuer Heartbeat/AckNack
        // wenn da was nachgeliefert wuerde.
        thread::sleep(Duration::from_millis(500));
        let received = reader.take().expect("take");

        // KEINE "EXP*"-Samples duerfen ankommen — alle expired.
        for s in &received {
            assert!(
                !s.color.starts_with("EXP"),
                "Lifespan=150ms sollte alle EXP-Samples nach 1s entfernt haben, \
                 bekam aber {s:?}"
            );
        }
    }

    #[test]
    fn lifespan_keeps_fresh_samples_available_to_late_joiner() {
        // Gegenbeispiel: Writer schreibt, wartet **weniger** als Lifespan,
        // Reader joined → muss Samples sehen.
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(5);
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), fast_cfg())
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());

        let wqos = DataWriterQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            lifespan: LifespanQosPolicy {
                duration: QosDuration::from_millis(10_000),
            },
            ..Default::default()
        };
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, wqos)
            .expect("writer");

        writer
            .write(&ShapeType::new("FRESH", 1, 2, 30))
            .expect("write");

        // 100ms warten — Sample muss noch frisch sein (Lifespan 10s).
        thread::sleep(Duration::from_millis(100));

        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let rqos = DataReaderQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            ..Default::default()
        };
        let reader = subscriber
            .create_datareader::<ShapeType>(&sub_topic, rqos)
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, Duration::from_secs(5))
            .expect("match");
        reader
            .wait_for_matched_publication(1, Duration::from_secs(5))
            .expect("match");

        let _ = reader.wait_for_data(Duration::from_secs(2));
        let received = reader.take().expect("take");

        assert!(
            received.iter().any(|s| s.color == "FRESH"),
            "Lifespan=10s + TL sollte FRESH liefern, got {received:?}"
        );
    }
}
