// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-security-main` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security`, all in `main.rs`
//! (`StdoutSecurityLogger`, the sink of `-logging`):
//! - `LogOptions`, `SecurityError` and `LoggerListener` in the signatures
//!   of `set_log_options`, `enable_logging` and `set_listener`, the
//!   operations of the `LoggingPlugin` trait it implements (the trait and
//!   `LogLevel` come through `zerodds_security_runtime`'s re-export of
//!   `zerodds_security::logging`).
//! - `SecurityError::from` for the errors of the logging crate.
//! - `log(level, participant, category, message)` and `plugin_class_id()`,
//!   which returns `DDS:Logging:DDS_LogTopic`; the plugin is handed to the
//!   runtime as `Box<dyn LoggingPlugin>`.
//!
//! What it relies on: the trait is implementable and object-safe with these
//! five operations, `LogLevel::Trace` is the most detailed level (the sink
//! is enabled at it to print everything), `LogOptions::new` distributes and
//! names no file, and a listener receives the parameters of `log`.
//!
//! Oracles: DDS-Security 1.2 §9.6.2.1 Table 45 (`LogOptions`: `log_level`,
//! `log_file`, `distribute`), §9.6.2.1.1 (the levels `FATAL_LEVEL` 0 to
//! `TRACE_LEVEL` 7, from the most severe to the most detailed), §9.6.2.2
//! Table 46 (the operations of the Logging plugin), §9.2.1 Table 28
//! (`SecurityException`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::logging::{
    DEBUG_LEVEL, ERROR_LEVEL, FATAL_LEVEL, INFO_LEVEL, LogLevel, LogOptions, LoggerListener,
    LoggingPlugin, NOTICE_LEVEL, SEVERE_LEVEL, TRACE_LEVEL, WARNING_LEVEL,
};

type Heard = (LogLevel, [u8; 16], String, String);

/// A sink with the five operations of the trait, as the client writes one:
/// it keeps the options, refuses them after `enable_logging`, and passes
/// every event to its listener.
#[derive(Default)]
struct Sink {
    options: Mutex<Option<LogOptions>>,
    enabled: Mutex<bool>,
    listener: Mutex<Option<Arc<dyn LoggerListener>>>,
}

impl LoggingPlugin for Sink {
    fn set_log_options(&self, options: LogOptions) -> Result<(), SecurityError> {
        if *self.enabled.lock().unwrap() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "logging has commenced",
            ));
        }
        *self.options.lock().unwrap() = Some(options);
        Ok(())
    }

    fn enable_logging(&self) -> Result<(), SecurityError> {
        *self.enabled.lock().unwrap() = true;
        Ok(())
    }

    fn set_listener(&self, listener: Option<Arc<dyn LoggerListener>>) -> Result<(), SecurityError> {
        *self.listener.lock().unwrap() = listener;
        Ok(())
    }

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        // §9.6.2.1.1: a message at or below `log_level`, once enabled.
        let admitted = *self.enabled.lock().unwrap()
            && self
                .options
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|options| level <= options.log_level);
        if !admitted {
            return;
        }
        if let Some(listener) = self.listener.lock().unwrap().as_ref() {
            listener.log_message(level, participant, category, message);
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:DDS_LogTopic"
    }
}

#[derive(Default)]
struct Recorder(Mutex<Vec<Heard>>);

impl LoggerListener for Recorder {
    fn log_message(
        &self,
        level: LogLevel,
        participant: [u8; 16],
        category: &str,
        message: &str,
    ) -> i32 {
        self.0.lock().unwrap().push((
            level,
            participant,
            category.to_string(),
            message.to_string(),
        ));
        0
    }
}

#[test]
fn security_9_6_2_2_a_sink_of_the_five_operations_works_as_a_boxed_plugin() {
    let plugin: Box<dyn LoggingPlugin> = Box::new(Sink::default());
    assert_eq!(plugin.plugin_class_id(), "DDS:Logging:DDS_LogTopic");
    plugin
        .set_log_options(LogOptions::new(LogLevel::Trace))
        .unwrap();
    plugin.enable_logging().unwrap();
    let refused = plugin
        .set_log_options(LogOptions::new(LogLevel::Error))
        .expect_err("after enable_logging");
    assert_eq!(refused.kind, SecurityErrorKind::BadArgument);
    assert_eq!(refused.code, SecurityErrorKind::BadArgument.code());

    let recorder = Arc::new(Recorder::default());
    plugin
        .set_listener(Some(recorder.clone() as Arc<dyn LoggerListener>))
        .unwrap();
    plugin.log(LogLevel::Notice, [7; 16], "auth.handshake", "completed");
    plugin.log(LogLevel::Trace, [7; 16], "auth.detail", "nonce");
    assert_eq!(recorder.0.lock().unwrap().len(), 2, "Trace is admitted");
    recorder.0.lock().unwrap().truncate(1);
    assert_eq!(
        *recorder.0.lock().unwrap(),
        [(
            LogLevel::Notice,
            [7; 16],
            "auth.handshake".to_string(),
            "completed".to_string(),
        )]
    );
}

#[test]
fn security_9_6_2_1_log_options_new_distributes_and_names_no_file() {
    let options = LogOptions::new(LogLevel::Warning);
    assert_eq!(
        options,
        LogOptions {
            log_level: LogLevel::Warning,
            log_file: None,
            distribute: true,
        }
    );
}

#[test]
fn security_9_6_2_1_1_trace_is_the_most_detailed_level() {
    // The sink is enabled at `Trace` so that it prints every event.
    let levels = [
        (LogLevel::Emergency, FATAL_LEVEL, "FATAL_LEVEL"),
        (LogLevel::Critical, SEVERE_LEVEL, "SEVERE_LEVEL"),
        (LogLevel::Error, ERROR_LEVEL, "ERROR_LEVEL"),
        (LogLevel::Warning, WARNING_LEVEL, "WARNING_LEVEL"),
        (LogLevel::Notice, NOTICE_LEVEL, "NOTICE_LEVEL"),
        (LogLevel::Informational, INFO_LEVEL, "INFO_LEVEL"),
        (LogLevel::Debug, DEBUG_LEVEL, "DEBUG_LEVEL"),
        (LogLevel::Trace, TRACE_LEVEL, "TRACE_LEVEL"),
    ];
    for (expected, (level, number, name)) in levels.iter().enumerate() {
        assert_eq!(*number, i32::try_from(expected).unwrap(), "{name}");
        assert_eq!(level.to_spec_level(), *number);
        assert_eq!(level.spec_name(), *name);
        assert_eq!(LogLevel::from_spec_level(*number), Some(*level));
        assert!(*level <= LogLevel::Trace);
    }
    for pair in levels.windows(2) {
        assert!(pair[0].0 < pair[1].0, "{:?} before {:?}", pair[0], pair[1]);
    }
    // `Alert` is a severe level too.
    assert_eq!(LogLevel::Alert.to_spec_level(), SEVERE_LEVEL);
    assert_eq!(LogLevel::from_spec_level(8), None);
    assert_eq!(LogLevel::from_spec_level(-1), None);
}
