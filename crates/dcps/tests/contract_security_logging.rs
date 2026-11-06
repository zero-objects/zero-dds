// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-security-logging`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-logging` (only with its
//! feature `security`, see `Cargo.toml`), from its non-test sources:
//! - `participant.rs`: `log_topic_requested`, `log_topic_legacy_requested`
//!   and `log_topic_distribute_requested` on the `dds.sec.log.*` pairs of
//!   the `PropertyQosPolicy`; `LogTopicWriterSlot::empty()`, handed to the
//!   runtime as `Arc<dyn LogTopicPublisher>` before the participant exists
//!   and filled with the DataWriter publisher afterwards.
//! - `runtime.rs`: `logging_plugin_from_properties` and
//!   `logging_plugin_from_properties_with_publisher`, whose
//!   `Box<dyn LoggingPlugin>` becomes `RuntimeConfig::security_logger`;
//!   `LogConfigError`.
//! - `security_log_topic.rs`: the re-exports `BuiltinLoggingType`,
//!   `BuiltinLoggingTypeV2`, `LogTopicLoggingPlugin`,
//!   `LogTopicPublishError`, `LogTopicPublisher`, `LoggingLevel`; the
//!   `DdsType` codecs of the two sample types written field by field over
//!   `facility`, `severity` (as `u32`, read back with the eight IDL
//!   values), `timestamp` (`Time { sec: i64, nanosec: u32 }`,
//!   `LegacyTime { sec: i32, nanosec: u32 }`), the six strings and
//!   `structured_data` (`NameValuePair`, `NameValuePairSeq`); the
//!   `LogTopicPublisher` over the DataWriters, which adds `domain_id` to the
//!   `DDS` entry of `structured_data`.
//!
//! What it relies on: without `dds.sec.log.plugin` no logger is built; the
//! Log Topic sink is requested by its names, distributes unless told
//! otherwise, and refuses to be built without a publisher when it
//! distributes; the slot can be filled once; an event admitted by the sink
//! reaches the publisher as a V2 sample (and a legacy one when asked) with
//! the parameters of `log`, the facility of security events and the `DDS`
//! entry; the enumerators of `LoggingLevel` have the IDL numbers.
//!
//! Oracles: DDS-Security 1.2 §10.6 (the builtin `DDS:Logging:DDS_LogTopic`
//! plugin, the IDL of `BuiltinLoggingTypeV2` and `BuiltinLoggingType`:
//! facility 0x0A "sec/auth msgs", the `LoggingLevel` enumerators
//! `EMERGENCY_LEVEL` 0 to `DEBUG_LEVEL` 7, `long long`/`long` seconds),
//! §10.6.1 (a writer is needed only when the events are distributed),
//! §9.6.2.1.3 (`distribute`), §9.6.2.2.2 Table 47 (the `DDS` entry of
//! `structured_data` with `guid`, `plugin_class`, `plugin_method`), and the
//! rustdoc of `from_properties` (the `dds.sec.log.*` property table and its
//! defaults).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use zerodds_security::logging::{LogLevel, LoggingPlugin};
use zerodds_security_logging::{
    BuiltinLoggingType, BuiltinLoggingTypeV2, FACILITY_SEC_AUTH, LOG_TOPIC_SINKS, LegacyTime,
    LogConfigError, LogTopicPublishError, LogTopicPublisher, LogTopicWriterSlot, LoggingLevel,
    PROP_LOG_BUILTIN_DISTRIBUTE, PROP_LOG_BUILTIN_LEGACY_TOPIC, PROP_LOG_LEVEL, PROP_LOG_PLUGIN,
    Time, log_topic_distribute_requested, log_topic_legacy_requested, log_topic_requested,
    logging_plugin_from_properties, logging_plugin_from_properties_with_publisher,
};

#[test]
fn the_log_topic_sink_is_requested_by_its_names() {
    for sink in LOG_TOPIC_SINKS {
        let list = format!("stderr, {sink}");
        assert!(
            log_topic_requested(&[(PROP_LOG_PLUGIN, list.as_str())]),
            "{sink}"
        );
    }
    assert_eq!(LOG_TOPIC_SINKS[0], "builtin.DDS_LogTopic");
    assert!(!log_topic_requested(&[(PROP_LOG_PLUGIN, "stderr,jsonl")]));
    assert!(!log_topic_requested(&[]));
    assert!(!log_topic_requested(&[(PROP_LOG_LEVEL, "ERROR_LEVEL")]));
}

#[test]
fn security_9_6_2_1_3_the_sink_distributes_unless_told_otherwise() {
    assert!(log_topic_distribute_requested(&[]).unwrap());
    assert!(!log_topic_legacy_requested(&[]).unwrap());
    assert!(!log_topic_distribute_requested(&[(PROP_LOG_BUILTIN_DISTRIBUTE, "false")]).unwrap());
    assert!(log_topic_legacy_requested(&[(PROP_LOG_BUILTIN_LEGACY_TOPIC, "true")]).unwrap());
    // A value that is no boolean is refused by name: the participant is not
    // created rather than logging somewhere else.
    match log_topic_distribute_requested(&[(PROP_LOG_BUILTIN_DISTRIBUTE, "sometimes")]) {
        Err(LogConfigError::BadBoolean { property, value }) => {
            assert_eq!(property, PROP_LOG_BUILTIN_DISTRIBUTE);
            assert_eq!(value, "sometimes");
        }
        other => panic!("expected BadBoolean, got {other:?}"),
    }
    assert!(log_topic_legacy_requested(&[(PROP_LOG_BUILTIN_LEGACY_TOPIC, "maybe")]).is_err());
}

#[test]
fn without_a_plugin_property_no_logger_is_built() {
    assert!(logging_plugin_from_properties(&[]).unwrap().is_none());
    assert!(
        logging_plugin_from_properties(&[(PROP_LOG_LEVEL, "WARNING_LEVEL")])
            .unwrap()
            .is_none()
    );
    let stderr = logging_plugin_from_properties(&[
        (PROP_LOG_PLUGIN, "stderr"),
        (PROP_LOG_LEVEL, "ERROR_LEVEL"),
    ])
    .unwrap();
    assert!(stderr.is_some());
    assert!(matches!(
        logging_plugin_from_properties(&[(PROP_LOG_PLUGIN, "carrier-pigeon")]),
        Err(LogConfigError::UnknownSink(_))
    ));
}

#[test]
fn security_10_6_1_a_distributing_log_topic_sink_needs_a_publisher() {
    let props = [(PROP_LOG_PLUGIN, "builtin.DDS_LogTopic")];
    match logging_plugin_from_properties(&props) {
        Err(LogConfigError::MissingPublisher) => {}
        Err(other) => panic!("expected MissingPublisher, got {other:?}"),
        Ok(_) => panic!("a distributing sink without a DataWriter was built"),
    }
    // File-only (`distribute` false): no publisher needed.
    let file_only = [
        (PROP_LOG_PLUGIN, "builtin.DDS_LogTopic"),
        (PROP_LOG_BUILTIN_DISTRIBUTE, "false"),
    ];
    assert!(
        logging_plugin_from_properties(&file_only)
            .unwrap()
            .is_some()
    );
}

/// The DataWriters of the participant, as the slot sees them.
#[derive(Default)]
struct Writers {
    v2: Mutex<Vec<BuiltinLoggingTypeV2>>,
    legacy: Mutex<Vec<BuiltinLoggingType>>,
}

impl LogTopicPublisher for Writers {
    fn publish_v2(&self, event: &BuiltinLoggingTypeV2) -> Result<(), LogTopicPublishError> {
        self.v2.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn publish_legacy(&self, event: &BuiltinLoggingType) -> Result<(), LogTopicPublishError> {
        self.legacy.lock().unwrap().push(event.clone());
        Ok(())
    }
}

#[test]
fn security_10_6_an_admitted_event_reaches_the_writers_through_the_slot() {
    let slot = Arc::new(LogTopicWriterSlot::empty());
    assert!(!slot.is_set());
    let publisher: Arc<dyn LogTopicPublisher> = slot.clone();
    let props = [
        (PROP_LOG_PLUGIN, "builtin.DDS_LogTopic"),
        (PROP_LOG_LEVEL, "WARNING_LEVEL"),
        (PROP_LOG_BUILTIN_LEGACY_TOPIC, "true"),
    ];
    let logger: Box<dyn LoggingPlugin> =
        logging_plugin_from_properties_with_publisher(&props, Some(publisher))
            .unwrap()
            .expect("the sink is configured");

    // The participant fills the slot once it has its DataWriters.
    let writers = Arc::new(Writers::default());
    assert!(slot.set(writers.clone()).is_ok());
    assert!(slot.is_set());
    assert!(
        slot.set(writers.clone()).is_err(),
        "a second writer is refused"
    );

    let participant = [0x5a; 16];
    logger.log(
        LogLevel::Error,
        participant,
        "access.create_datawriter.denied",
        "topic Square",
    );
    logger.log(LogLevel::Debug, participant, "auth.detail", "not admitted");

    let v2 = writers.v2.lock().unwrap();
    assert_eq!(v2.len(), 1, "only the admitted event is published");
    let event = &v2[0];
    assert_eq!(event.facility, FACILITY_SEC_AUTH);
    assert_eq!(FACILITY_SEC_AUTH, 0x0a);
    assert_eq!(event.severity, LoggingLevel::Error);
    assert_eq!(event.msgid, "access.create_datawriter.denied");
    assert_eq!(event.message, "topic Square");
    let dds = event.structured_data.get("DDS").expect("the DDS entry");
    let value = |name: &str| {
        dds.iter()
            .find(|pair| pair.name == name)
            .map(|pair| pair.value.as_str())
    };
    assert_eq!(value("guid").map(str::len), Some(32), "{dds:?}");
    assert!(value("plugin_class").is_some(), "{dds:?}");
    assert!(value("plugin_method").is_some(), "{dds:?}");

    let legacy = writers.legacy.lock().unwrap();
    assert_eq!(legacy.len(), 1, "the legacy topic was asked for");
    assert_eq!(legacy[0].severity, LoggingLevel::Error);
    assert_eq!(legacy[0].msgid, event.msgid);
}

#[test]
fn security_10_6_the_logging_level_enumerators_have_the_idl_numbers() {
    // `severity_from_u32` reads these numbers back.
    let idl = [
        (LoggingLevel::Emergency, 0u32),
        (LoggingLevel::Alert, 1),
        (LoggingLevel::Critical, 2),
        (LoggingLevel::Error, 3),
        (LoggingLevel::Warning, 4),
        (LoggingLevel::Notice, 5),
        (LoggingLevel::Informational, 6),
        (LoggingLevel::Debug, 7),
    ];
    for (level, number) in idl {
        assert_eq!(level as u32, number, "{level:?}");
    }
    // The IDL has no TRACE; the most detailed level stands for it.
    assert_eq!(LoggingLevel::from(LogLevel::Trace), LoggingLevel::Debug);
    assert_eq!(LoggingLevel::from(LogLevel::Error), LoggingLevel::Error);
}

#[test]
fn security_10_6_the_timestamps_have_the_widths_of_the_idl() {
    // `long long sec` in the V2 type, `long sec` in the legacy type, both
    // with `unsigned long nanosec`.
    let v2 = Time {
        sec: i64::MIN,
        nanosec: u32::MAX,
    };
    let legacy = LegacyTime {
        sec: i32::MIN,
        nanosec: u32::MAX,
    };
    assert_eq!(v2.sec, i64::MIN);
    assert_eq!(legacy.sec, i32::MIN);
    let error = LogTopicPublishError {
        topic: "DDS:Security:LogTopicV2",
        reason: "writer gone".to_string(),
    };
    assert!(error.to_string().contains("DDS:Security:LogTopicV2"));
    assert!(error.to_string().contains("writer gone"));
}
