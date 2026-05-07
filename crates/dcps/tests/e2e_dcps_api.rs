//! Integrationtests ueber die DCPS-Public-API.
//!
//! Diese Tests nutzen **ausschliesslich** die nach aussen exportierten
//! DCPS-Typen. Kein Zugriff auf `runtime::*` (ausser RuntimeConfig
//! fuer Timing-Tuning), kein `__push_raw`.

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

use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, RawBytes, SubscriberQos,
    TopicQos,
};

#[test]
fn public_api_topic_reuse_same_type_returns_shared_handle() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(21, DomainParticipantQos::default());

    let t = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("first topic");
    let t2 = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("same-type reuse");

    assert_eq!(t.name(), t2.name());
    assert_eq!(p.topics_len(), 1);
}

#[test]
fn public_api_create_subscriber_and_reader_offline() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(22, DomainParticipantQos::default());

    let topic = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("topic");
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");

    assert_eq!(reader.topic().name(), "Chatter");
    assert_eq!(reader.matched_publication_count(), 0);
    let samples = reader.take().expect("take");
    assert!(samples.is_empty());
}

#[path = "common/mod.rs"]
mod common;

#[cfg(target_os = "linux")]
mod linux_e2e {
    //! Linux-only: echter SPDP-Multicast-E2E ueber die Public-API, jetzt
    //! mit `wait_for_matched_*` als Synchronisationspunkt. Damit entfaellt
    //! der bisherige "blind poll + hope"-Ansatz und der Test wird stabil.

    use std::time::Duration;

    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        RawBytes, SubscriberQos, TopicQos,
    };

    use super::common::unique_domain;

    #[test]
    fn two_participants_exchange_samples_via_public_api() {
        // Eigene Domain (20) + tight SPDP-Period (100 ms).
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };

        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(9);
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .expect("sub topic");

        let publisher = pub_p.create_publisher(PublisherQos::default());
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());

        let writer = publisher
            .create_datawriter::<RawBytes>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let reader = subscriber
            .create_datareader::<RawBytes>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        // Discovery-Sync ueber die Public-API. 5 s Budget deckt CI-Jitter.
        writer
            .wait_for_matched_subscription(1, Duration::from_secs(5))
            .expect("writer sees subscriber");
        reader
            .wait_for_matched_publication(1, Duration::from_secs(5))
            .expect("reader sees publisher");

        writer
            .write(&RawBytes::new(vec![0xDE, 0xAD, 0xBE, 0xEF]))
            .expect("write");

        // wait_for_data statt busy-poll.
        reader
            .wait_for_data(Duration::from_secs(3))
            .expect("sample arrives");
        let samples = reader.take().expect("take");
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].data, vec![0xDE, 0xAD, 0xBE, 0xEF]);

        // wait_for_acknowledgments: Reader hat bis hier via AckNack
        // alles bestaetigt — sollte praktisch instant zurueckkommen.
        writer
            .wait_for_acknowledgments(Duration::from_secs(3))
            .expect("writer acks complete");
    }
}
