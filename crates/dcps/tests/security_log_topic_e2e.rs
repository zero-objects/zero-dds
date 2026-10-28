// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-end: the builtin `DDS:Logging:DDS_LogTopic` sink publishes a
//! security event on the DDS topic `DDS:Security:LogTopicV2` with type
//! `BuiltinLoggingTypeV2` (DDS-Security 1.2 §10.6), and — when asked — on the
//! legacy `DDS:Security:LogTopic` with type `BuiltinLoggingType`.
//!
//! The sink is exercised through the same public path a participant uses:
//! [`zerodds_dcps::security_log_topic::log_topic_publisher_for_participant`]
//! creates the DataWriter(s), and the sample is read back through a normal
//! `DataReader`.

#![cfg(all(feature = "security", target_os = "linux"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy};
use zerodds_dcps::security_log_topic::{
    BuiltinLoggingType, BuiltinLoggingTypeV2, log_topic_publisher_for_participant,
};
use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos, TopicQos,
};
use zerodds_security::logging::TOPIC_LOG_TOPIC;
use zerodds_security_logging::LogTopicLoggingPlugin;
use zerodds_security_runtime::{LogLevel, LoggingPlugin};

use common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};

fn reader_qos() -> DataReaderQos {
    DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

fn poll_one<T: zerodds_dcps::DdsType + Clone + Send + 'static>(
    reader: &zerodds_dcps::DataReader<T>,
) -> Option<T> {
    let deadline = Instant::now() + match_timeout();
    while Instant::now() < deadline {
        if let Ok(mut samples) = reader.take() {
            if let Some(sample) = samples.pop() {
                return Some(sample);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn a_security_event_reaches_the_v2_log_topic() {
    let factory = DomainParticipantFactory::instance();
    let domain = unique_domain();
    let participant = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
            .expect("participant"),
    );

    let topic = participant
        .create_topic::<BuiltinLoggingTypeV2>("DDS:Security:LogTopicV2", TopicQos::default())
        .expect("v2 topic");
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<BuiltinLoggingTypeV2>(&topic, reader_qos())
        .expect("v2 reader");

    let publisher =
        log_topic_publisher_for_participant(&participant, false).expect("log topic writer");
    let sink = LogTopicLoggingPlugin::new(publisher, LogLevel::Warning, false);
    let guid = [0x11; 16];
    sink.log(
        LogLevel::Critical,
        guid,
        "access.create_datawriter.denied",
        "topic Chatter: denied",
    );

    let sample = poll_one(&reader).expect("the sample must reach the reader");
    assert_eq!(sample.facility, zerodds_security_logging::FACILITY_SEC_AUTH);
    assert_eq!(
        sample.severity,
        zerodds_security_logging::LoggingLevel::Critical
    );
    assert_eq!(sample.msgid, "access.create_datawriter.denied");
    assert_eq!(sample.message, "topic Chatter: denied");
    let dds = sample
        .structured_data
        .get("DDS")
        .expect("the DDS structured-data entry (§9.6.2.2.2)");
    assert!(
        dds.iter()
            .any(|p| p.name == "guid" && p.value == "11111111111111111111111111111111")
    );
    assert!(
        dds.iter()
            .any(|p| p.name == "domain_id" && p.value == domain.to_string()),
        "the publisher adds the domain_id pair (§9.6.2.2.2)"
    );
}

#[test]
fn the_legacy_topic_is_published_when_configured() {
    let factory = DomainParticipantFactory::instance();
    let domain = unique_domain();
    let qos = DomainParticipantQos::default();
    let participant = ParticipantGuard::new(
        factory
            .create_participant_with_config(domain, qos, isolated_cfg())
            .expect("participant"),
    );

    let topic = participant
        .create_topic::<BuiltinLoggingType>("DDS:Security:LogTopic", TopicQos::default())
        .expect("legacy topic");
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<BuiltinLoggingType>(&topic, reader_qos())
        .expect("legacy reader");

    let publisher =
        log_topic_publisher_for_participant(&participant, true).expect("log topic writers");
    let sink = LogTopicLoggingPlugin::new(publisher, LogLevel::Warning, true);
    sink.log(
        LogLevel::Error,
        [0x22; 16],
        "auth.handshake.failed",
        "bad certificate",
    );

    let sample = poll_one(&reader).expect("the legacy sample must reach the reader");
    assert_eq!(sample.facility, zerodds_security_logging::FACILITY_SEC_AUTH);
    assert_eq!(sample.msgid, "auth.handshake.failed");
    assert_eq!(sample.message, "bad certificate");
}

#[test]
fn the_topic_names_are_the_clause_names() {
    // Guard against a rename: the two names are the spec's
    // (DDS-Security 1.2 §10.6).
    assert_eq!(TOPIC_LOG_TOPIC, "DDS:Security:LogTopic");
    assert_eq!(
        zerodds_security::logging::TOPIC_LOG_TOPIC_V2,
        "DDS:Security:LogTopicV2"
    );
}
