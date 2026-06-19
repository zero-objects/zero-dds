//! Lifespan QoS Tests (Writer-side sample expiration).
//!
//! Spec OMG DDS 1.4 §2.2.3.16 LifespanQosPolicy: "if the duration
//! elapses and the sample is still in the writer's cache, the sample
//! is no longer available to any future DataReaders."
//!
//! Implemented as a writer-side cache scan every ~20 ms. The reader-
//! side lifespan filter (on received samples) is the reader-lifespan track.
//!
//! Test strategy: the writer writes N samples at t=0 with a lifespan
//! shorter than the wait time, then the reader joins **late**. On correct
//! expiration: the reader receives nothing (or only samples produced after
//! the match). This works well with TransientLocal
//! durability — otherwise Volatile would prevent replay anyway and
//! the test would say nothing about lifespan.

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

    /// Waits until both participants have seen each other at the SPDP
    /// level. Without this warm-up, the late reader in the late-joiner
    /// test starts with a cold SPDP cache table, then the SEDP match must
    /// complete the discovery + SEDP roundtrip within 5 s — on
    /// heavily loaded CI runners that is not enough. With warm-up,
    /// only SEDP remains to be transferred.
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
        // Setup: writer with Durability=TransientLocal + Lifespan=150ms.
        // Writes 3 samples, waits 1 s (all expire) → a new
        // reader joins → receives nothing.
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

        // Bootstrap SPDP bidirectionally before the lifespan window
        // starts. That makes the late reader match a pure SEDP
        // roundtrip and fits safely within 5 s on CI runners under load.
        wait_spdp_bidirectional(&pub_p, &sub_p, Duration::from_secs(5));

        // 3 samples at the same moment.
        for i in 0i32..3 {
            writer
                .write(&ShapeType::new(format!("EXP{i}"), i, 0, 30))
                .expect("write");
        }

        // Wait 1 s — all samples must expire (150 ms lifespan).
        thread::sleep(Duration::from_millis(1_000));

        // Now the reader arrives. Also TransientLocal so that it
        // would expect replay if lifespan had not taken effect.
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

        // SEDP-DATA(r) for the late reader runs over reliable
        // RTPS with `SEDP_HEARTBEAT_PERIOD = 500ms`. Under multicast
        // loss with CI load, recovery needs several HB cycles
        // — 5 s is not reproducibly enough. 15 s covers the
        // worst case (3 dropped DATA + 3 HB recovery roundtrips).
        writer
            .wait_for_matched_subscription(1, Duration::from_secs(15))
            .expect("match");
        reader
            .wait_for_matched_publication(1, Duration::from_secs(15))
            .expect("match");

        // 500 ms should be more than enough for heartbeat/AckNack
        // if anything were to be delivered late.
        thread::sleep(Duration::from_millis(500));
        let received = reader.take().expect("take");

        // NO "EXP*" samples may arrive — all expired.
        for s in &received {
            assert!(
                !s.color.starts_with("EXP"),
                "Lifespan=150ms should have removed all EXP samples after 1s, \
                 but got {s:?}"
            );
        }
    }

    #[test]
    fn lifespan_keeps_fresh_samples_available_to_late_joiner() {
        // Counter-example: writer writes, waits **less** than lifespan,
        // reader joins → must see samples.
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

        // Wait 100ms — sample must still be fresh (lifespan 10s).
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
            .wait_for_matched_subscription(1, super::common::match_timeout())
            .expect("match");
        reader
            .wait_for_matched_publication(1, super::common::match_timeout())
            .expect("match");

        let _ = reader.wait_for_data(Duration::from_secs(2));
        let received = reader.take().expect("take");

        assert!(
            received.iter().any(|s| s.color == "FRESH"),
            "Lifespan=10s + TL should deliver FRESH, got {received:?}"
        );
    }
}
