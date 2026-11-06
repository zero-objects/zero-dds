// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-logging` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security`, from its non-test sources:
//! - `logging::LoggingPlugin`: implemented by every sink
//!   (`stderr_sink.rs`, `jsonl.rs`, `syslog.rs`, `fanout.rs`, `log_topic.rs`)
//!   with `set_log_options`, `enable_logging`, `set_listener`, `log` and
//!   `plugin_class_id`; `fanout.rs` holds sinks as `Box<dyn LoggingPlugin>`
//!   and `from_properties.rs` returns one.
//! - `logging::LogLevel`: the admission check of `control.rs` compares
//!   `level > options.log_level`; `syslog.rs` takes `level as u8` (clamped to
//!   7) as the syslog severity; `log_topic.rs` maps it onto the IDL
//!   `LoggingLevel`; `lib.rs` defines `DEFAULT_LOG_LEVEL` as
//!   `LogLevel::Informational`; `from_properties.rs` parses the names.
//! - `logging::LogOptions` (re-exported by `lib.rs`): built field by field
//!   (`log_level`, `log_file`, `distribute`) and cloned (`control.rs`,
//!   `from_properties.rs`, `log_topic.rs`).
//! - `logging::LoggerListener` (re-exported by `lib.rs`): held as
//!   `Arc<dyn LoggerListener>` and called with `log_message`.
//! - `logging::{TOPIC_LOG_TOPIC_V2, TOPIC_LOG_TOPIC}`: the topic names of
//!   the two writers of `log_topic.rs`.
//! - `error::{SecurityError, SecurityErrorKind}`: `control.rs` turns its
//!   `LogOptionsError` into `SecurityError::new(kind, message)` with
//!   `BadArgument`, `InvalidConfiguration` or `Internal`.
//!
//! Oracles: DDS-Security 1.2 §9.6.2.1.1 (the levels FATAL_LEVEL to
//! TRACE_LEVEL, "from low to high"; "messages at or below the log_level are
//! logged"), §9.6.2.1 Table 45 (`LogOptions`: `log_level`, `log_file`,
//! `distribute`), §9.6.2.2 Table 46 (the operations of `Logging`), Figure
//! 21 (`LoggerListener::log_message`), §10.6 (the topic names
//! "DDS:Security:LogTopicV2" and "DDS:Security:LogTopic", the IDL enum
//! `LoggingLevel` EMERGENCY_LEVEL to DEBUG_LEVEL), RFC 5424 §6.2.1 Table 2
//! (syslog severities 0 Emergency to 7 Debug), §9.2.1 Table 28
//! (`SecurityException`), and the rustdoc of `LogLevel` and `LogOptions::new`.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::logging::{
    DEBUG_LEVEL, ERROR_LEVEL, FATAL_LEVEL, INFO_LEVEL, LogLevel, LogOptions, LoggerListener,
    LoggingPlugin, NOTICE_LEVEL, SEVERE_LEVEL, TOPIC_LOG_TOPIC, TOPIC_LOG_TOPIC_V2, TRACE_LEVEL,
    WARNING_LEVEL,
};

/// Every level, in the order the rustdoc of `LogLevel` gives ("the order of
/// the variants is the order of the levels, low to high").
const LEVELS: [LogLevel; 9] = [
    LogLevel::Emergency,
    LogLevel::Alert,
    LogLevel::Critical,
    LogLevel::Error,
    LogLevel::Warning,
    LogLevel::Notice,
    LogLevel::Informational,
    LogLevel::Debug,
    LogLevel::Trace,
];

#[test]
fn security_9_6_2_1_1_levels_order_from_fatal_to_trace() {
    // §9.6.2.1.1 lists the levels from low to high.
    let spec = [
        FATAL_LEVEL,
        SEVERE_LEVEL,
        ERROR_LEVEL,
        WARNING_LEVEL,
        NOTICE_LEVEL,
        INFO_LEVEL,
        DEBUG_LEVEL,
        TRACE_LEVEL,
    ];
    assert!(spec.windows(2).all(|w| w[0] < w[1]));

    // `control.rs` drops an event when `level > options.log_level`, so the
    // order of `LogLevel` has to be the order of the clause: each variant is
    // above the one before it, and its spec level never goes down.
    for w in LEVELS.windows(2) {
        assert!(w[0] < w[1], "{:?} < {:?}", w[0], w[1]);
        assert!(w[0].to_spec_level() <= w[1].to_spec_level());
    }
    assert_eq!(LogLevel::Emergency.to_spec_level(), FATAL_LEVEL);
    assert_eq!(LogLevel::Trace.to_spec_level(), TRACE_LEVEL);

    // "Messages at or below the log_level are logged": at WARNING an ERROR
    // event passes and an INFO event does not.
    let threshold = LogLevel::Warning;
    assert!(LogLevel::Error <= threshold);
    assert!(LogLevel::Warning <= threshold);
    assert!(LogLevel::Informational > threshold);
}

#[test]
fn security_10_6_the_level_discriminants_are_the_logging_level_and_syslog_severities() {
    // §10.6 `enum LoggingLevel` numbers EMERGENCY_LEVEL .. DEBUG_LEVEL from
    // 0, and RFC 5424 §6.2.1 Table 2 gives the same eight severities the
    // codes 0 to 7. `syslog.rs` sends `level as u8` (clamped to 7).
    let expected: [(LogLevel, u8); 8] = [
        (LogLevel::Emergency, 0),
        (LogLevel::Alert, 1),
        (LogLevel::Critical, 2),
        (LogLevel::Error, 3),
        (LogLevel::Warning, 4),
        (LogLevel::Notice, 5),
        (LogLevel::Informational, 6),
        (LogLevel::Debug, 7),
    ];
    for (level, code) in expected {
        assert_eq!(level as u8, code, "{level:?}");
    }
    // TRACE_LEVEL has no syslog severity; it lies above Debug, so the clamp
    // to 7 sends it as Debug.
    let trace = LogLevel::Trace;
    assert!(trace as u8 > 7);
}

#[test]
fn security_10_6_the_builtin_logging_topic_names() {
    // §10.6: "The BuiltinLoggingTopic shall have the Topic name
    // 'DDS:Security:LogTopicV2'"; the legacy topic is "DDS:Security:LogTopic".
    assert_eq!(TOPIC_LOG_TOPIC_V2, "DDS:Security:LogTopicV2");
    assert_eq!(TOPIC_LOG_TOPIC, "DDS:Security:LogTopic");
}

#[test]
fn security_9_6_2_1_log_options_carry_the_three_attributes_of_table_45() {
    // Built field by field, as `from_properties.rs` does.
    let options = LogOptions {
        log_level: LogLevel::Warning,
        log_file: Some("/var/log/dds-security.log".to_string()),
        distribute: false,
    };
    let copy = options.clone();
    assert_eq!(copy, options);
    assert_eq!(copy.log_level, LogLevel::Warning);
    assert_eq!(copy.log_file.as_deref(), Some("/var/log/dds-security.log"));
    assert!(!copy.distribute);

    // Rustdoc of `LogOptions::new`: no file (§9.6.2.1.2 NULL: "the logger
    // will not log messages to a file") and `distribute` on.
    let new = LogOptions::new(LogLevel::Error);
    assert_eq!(new.log_level, LogLevel::Error);
    assert_eq!(new.log_file, None);
    assert!(new.distribute);
}

/// One message a listener heard: level, participant, category, message.
type Heard = (LogLevel, [u8; 16], String, String);

/// A listener as `control.rs` holds one: behind `Arc<dyn LoggerListener>`.
#[derive(Default)]
struct Recorder {
    heard: Mutex<Vec<Heard>>,
}

impl LoggerListener for Recorder {
    fn log_message(
        &self,
        level: LogLevel,
        participant: [u8; 16],
        category: &str,
        message: &str,
    ) -> i32 {
        self.heard.lock().unwrap().push((
            level,
            participant,
            category.to_string(),
            message.to_string(),
        ));
        0
    }
}

/// A sink with the signatures of Table 46, as the sinks of this crate
/// implement them; it forwards each event at or below its level to its
/// listener.
#[derive(Default)]
struct Sink {
    options: Mutex<Option<LogOptions>>,
    listener: Mutex<Option<Arc<dyn LoggerListener>>>,
}

impl LoggingPlugin for Sink {
    fn set_log_options(&self, options: LogOptions) -> Result<(), SecurityError> {
        *self.options.lock().unwrap() = Some(options);
        Ok(())
    }

    fn enable_logging(&self) -> Result<(), SecurityError> {
        Ok(())
    }

    fn set_listener(&self, listener: Option<Arc<dyn LoggerListener>>) -> Result<(), SecurityError> {
        *self.listener.lock().unwrap() = listener;
        Ok(())
    }

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        let limit = self.options.lock().unwrap().as_ref().map(|o| o.log_level);
        if limit.is_some_and(|limit| level <= limit) {
            let listener = self.listener.lock().unwrap().clone();
            if let Some(listener) = listener {
                listener.log_message(level, participant, category, message);
            }
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:contract"
    }
}

#[test]
fn the_operations_of_table_46_work_through_a_trait_object() {
    // `fanout.rs` keeps its sinks as `Box<dyn LoggingPlugin>`, `control.rs`
    // its listener as `Arc<dyn LoggerListener>`; the runtime shares both
    // between threads.
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<dyn LoggingPlugin>();
    assert_send_sync::<dyn LoggerListener>();

    let sink: Box<dyn LoggingPlugin> = Box::<Sink>::default();
    let recorder = Arc::new(Recorder::default());
    let listener: Arc<dyn LoggerListener> = recorder.clone();
    // `None` is the NIL listener of §9.6.2.2.4.
    sink.set_listener(None).unwrap();
    sink.set_listener(Some(listener)).unwrap();
    sink.set_log_options(LogOptions::new(LogLevel::Warning))
        .unwrap();
    sink.enable_logging().unwrap();

    // `log` passes the level, the 16 octets of the participant GUID, the
    // category and the message (§9.6.2.2.2) on to `log_message` unchanged.
    let guid = [0xAB; 16];
    sink.log(LogLevel::Error, guid, "auth.handshake.failed", "bad cert");
    sink.log(
        LogLevel::Informational,
        guid,
        "auth.debug",
        "below the level",
    );
    let heard = recorder.heard.lock().unwrap();
    assert_eq!(
        *heard,
        vec![(
            LogLevel::Error,
            guid,
            "auth.handshake.failed".to_string(),
            "bad cert".to_string()
        )]
    );
    assert_eq!(sink.plugin_class_id(), "DDS:Logging:contract");
}

#[test]
fn security_9_2_1_the_refusals_of_this_crate_keep_their_kind_and_code() {
    // `control.rs` maps its `LogOptionsError` onto these three kinds; the
    // code follows the kind (rustdoc of `SecurityErrorKind::code`), the
    // minor code stays 0, and the message is kept.
    for (kind, code) in [
        (SecurityErrorKind::BadArgument, 5),
        (SecurityErrorKind::InvalidConfiguration, 4),
        (SecurityErrorKind::Internal, 7),
    ] {
        let e = SecurityError::new(kind, format!("refused: {code}"));
        assert_eq!(e.kind, kind);
        assert_eq!(e.code, code);
        assert_eq!(e.minor_code, 0);
        assert_eq!(e.detail, format!("refused: {code}"));
    }
}
