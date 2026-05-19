//! Liveliness QoS Tests (Automatic-Kind, Reader-Side).
//!
//! Spec OMG DDS 1.4 §2.2.3.11 + §2.2.4.2.14:
//! * Writer mit `LivelinessKind::Automatic` gilt als "alive" solange
//!   er regelmaessig Samples innerhalb seiner `lease_duration` schickt.
//! * Reader-Side fuehrt Counter fuer alive_count (Wiederbelebungen)
//!   und not_alive_count (Lease-Abläufe).
//!
//! WP-3.4a (dieser Commit): Automatic-Kind + Lease-Monitoring am
//! Reader. WP-3.4b: SEDP-Kompatibilitaets-Check + Manual-Kind-
//! Explicit-Assert-Messages.

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

use std::thread;
use std::time::Duration;

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos, TopicQos,
};

#[serial_test::serial(dcps)]
#[test]
fn liveliness_default_infinite_keeps_counters_zero() {
    // Default-Lease = INFINITE. Counter darf nie steigen, selbst wenn
    // kein Sample je ankommt. Sonst bricht die API-Shape.
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(
            common::unique_domain(7),
            DomainParticipantQos::default(),
            common::isolated_cfg(),
        )
        .expect("participant");
    let topic = p
        .create_topic::<ShapeType>("Square", TopicQos::default())
        .expect("topic");
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<ShapeType>(&topic, DataReaderQos::default())
        .expect("reader");

    thread::sleep(Duration::from_millis(300));
    let (_alive, alive_c, not_alive_c) = reader.liveliness_changed_status();
    assert_eq!(
        alive_c, 0,
        "INFINITE lease darf NIEMALS alive_count aendern"
    );
    assert_eq!(
        not_alive_c, 0,
        "INFINITE lease darf NIEMALS not_alive_count aendern"
    );
}

#[path = "common/mod.rs"]
mod common;

#[cfg(target_os = "linux")]
mod linux {
    use super::common::unique_domain;
    use super::*;
    use zerodds_dcps::{DataWriterQos, PublisherQos};
    use zerodds_qos::Duration as QosDuration;
    use zerodds_qos::LivelinessQosPolicy;

    #[serial_test::serial(dcps)]
    #[test]
    fn reader_with_short_lease_marks_writer_not_alive_when_silent() {
        // Reader mit 150ms-Lease, kein Writer schickt Samples →
        // not_alive_count sollte innerhalb 1s auf >=1 steigen.
        // Wir brauchen einen matched Writer (sonst startet die
        // last_sample_received-Uhr nicht); Writer schickt **ein**
        // Sample, dann Ruhe.
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(7);
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");

        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let rqos = DataReaderQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::Automatic,
                lease_duration: QosDuration::from_millis(150),
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

        // Ein Sample, dann 1s Ruhe.
        writer
            .write(&ShapeType::new("RED", 1, 2, 30))
            .expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(1));
        let _ = reader.take().expect("take");

        thread::sleep(Duration::from_millis(1_000));

        let (alive, _alive_c, not_alive_c) = reader.liveliness_changed_status();
        assert!(
            not_alive_c >= 1,
            "Writer schweigt 1s bei 150ms-Lease → not_alive_count muss >=1, got {not_alive_c}"
        );
        assert!(
            !alive,
            "Aktueller Zustand sollte not-alive sein, got alive={alive}"
        );
    }

    #[serial_test::serial(dcps)]
    #[test]
    fn reader_sees_writer_alive_again_after_resumed_publishing() {
        // Writer → silence → Writer wieder aktiv. alive_count sollte
        // den Transition not-alive → alive einmal zaehlen.
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(7);
        let cfg = super::common::isolated_cfg();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("pub topic");
        let publisher = pub_p.create_publisher(PublisherQos::default());
        let writer = publisher
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");

        let sub_topic = sub_p
            .create_topic::<ShapeType>("Square", TopicQos::default())
            .expect("sub topic");
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());
        let rqos = DataReaderQos {
            liveliness: LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::Automatic,
                lease_duration: QosDuration::from_millis(150),
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

        // Initialer Sample + Warte (→ not_alive).
        writer
            .write(&ShapeType::new("RED", 1, 2, 30))
            .expect("write");
        let _ = reader.wait_for_data(Duration::from_secs(1));
        let _ = reader.take().expect("take");
        thread::sleep(Duration::from_millis(500));

        let (alive_mid, _, not_alive_mid) = reader.liveliness_changed_status();
        assert!(
            !alive_mid && not_alive_mid >= 1,
            "Writer should be marked not-alive after silence"
        );

        // Writer wird wieder aktiv.
        writer
            .write(&ShapeType::new("RED", 3, 4, 30))
            .expect("write 2");
        let _ = reader.wait_for_data(Duration::from_secs(1));
        let _ = reader.take().expect("take");

        let (alive_end, alive_c_end, _not_alive_c_end) = reader.liveliness_changed_status();
        assert!(
            alive_end,
            "Writer sollte wieder alive sein, got {alive_end}"
        );
        assert!(
            alive_c_end >= 1,
            "Transition not_alive -> alive sollte alive_count >=1 geben, got {alive_c_end}"
        );
    }
}
