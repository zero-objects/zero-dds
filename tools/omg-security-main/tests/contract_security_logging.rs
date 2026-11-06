// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-security-main` with `zerodds-security-logging`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-logging`, all in `main.rs`
//! (`StdoutSecurityLogger`, the sink of `-logging`):
//! - `LogControl::enabled_at(LogLevel::Trace, false)`: the sink is enabled
//!   at the most detailed level when it is built, without distribution.
//! - `LogControl::set_log_options(options, false)`,
//!   `LogControl::enable_logging()` and `LogControl::set_listener(listener)`,
//!   the operations of Table 46 the sink forwards, with their error mapped
//!   by `SecurityError::from(LogOptionsError)`.
//! - `LogControl::admit(level, participant, category, message)`: `None`
//!   means the line is not printed.
//!
//! What it relies on: a control built enabled at `Trace` admits every
//! level and says the event is not distributed; options cannot change once
//! logging is enabled, and the refusal reaches the caller as a
//! `SecurityException`; a sink without a DataWriter refuses `distribute`;
//! the listener hears every admitted event with its parameters; an event
//! above the saved level is not admitted.
//!
//! Oracles: DDS-Security 1.2 §9.6.2.1 ("LogOptions ... may not be modified
//! once logging has commenced"), §9.6.2.1.1 (the levels; messages at or
//! below `log_level` are logged), §9.6.2.1.3 (`distribute`), §9.6.2.2
//! Table 46 (`set_log_options`, `log`, `enable_logging`, `set_listener`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::logging::{LogLevel, LogOptions, LoggerListener};
use zerodds_security_logging::{LogControl, LogOptionsError};

type Heard = (LogLevel, [u8; 16], String, String);

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

/// Every level, from the most severe to the most detailed.
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
fn security_9_6_2_1_1_a_control_enabled_at_trace_admits_every_level_undistributed() {
    let control = LogControl::enabled_at(LogLevel::Trace, false);
    assert!(control.is_enabled());
    assert_eq!(
        control.options(),
        Some(LogOptions {
            log_level: LogLevel::Trace,
            log_file: None,
            distribute: false,
        })
    );
    for level in LEVELS {
        assert_eq!(
            control.admit(level, [1; 16], "auth.handshake", "m"),
            Some(false),
            "{level:?} is printed, and not distributed"
        );
    }
}

#[test]
fn security_9_6_2_1_1_an_event_above_the_saved_level_is_not_admitted() {
    let control = LogControl::enabled_at(LogLevel::Warning, false);
    for level in LEVELS {
        let admitted = control.admit(level, [2; 16], "c", "m").is_some();
        assert_eq!(admitted, level <= LogLevel::Warning, "{level:?}");
    }
}

#[test]
fn security_9_6_2_1_options_cannot_change_once_logging_is_enabled() {
    let control = LogControl::enabled_at(LogLevel::Trace, false);
    let refused = control
        .set_log_options(LogOptions::new(LogLevel::Error), false)
        .expect_err("logging has commenced");
    assert!(matches!(refused, LogOptionsError::AlreadyEnabled));
    // The sink hands the refusal to its caller as a SecurityException.
    let exception = SecurityError::from(refused);
    assert_eq!(exception.kind, SecurityErrorKind::BadArgument);
    assert!(!exception.detail.is_empty());
    // The saved options are those of the constructor.
    assert_eq!(control.options().unwrap().log_level, LogLevel::Trace);
    // Enabling again is not an error.
    control.enable_logging().unwrap();
}

#[test]
fn security_9_6_2_1_3_a_sink_without_a_datawriter_refuses_distribute() {
    // A control with no options yet, as `set_log_options` sees it first.
    let control = LogControl::new();
    assert!(!control.is_enabled());
    assert_eq!(control.admit(LogLevel::Emergency, [3; 16], "c", "m"), None);
    let refused = control
        .set_log_options(LogOptions::new(LogLevel::Error), false)
        .expect_err("LogOptions::new distributes");
    assert!(matches!(refused, LogOptionsError::MissingPublisher));
    assert_eq!(
        SecurityError::from(refused).kind,
        SecurityErrorKind::InvalidConfiguration
    );
    // Without `distribute` the options are taken.
    control
        .set_log_options(
            LogOptions {
                distribute: false,
                ..LogOptions::new(LogLevel::Error)
            },
            false,
        )
        .unwrap();
    control.enable_logging().unwrap();
    assert_eq!(
        control.admit(LogLevel::Error, [3; 16], "c", "m"),
        Some(false)
    );
}

#[test]
fn security_9_6_2_2_enable_logging_needs_options() {
    let control = LogControl::new();
    let refused = control.enable_logging().expect_err("no options were set");
    assert!(matches!(refused, LogOptionsError::OptionsNotSet));
    assert_eq!(
        SecurityError::from(refused).kind,
        SecurityErrorKind::BadArgument
    );
}

#[test]
fn security_9_6_2_2_the_listener_hears_every_admitted_event() {
    let control = LogControl::enabled_at(LogLevel::Notice, false);
    let recorder = Arc::new(Recorder::default());
    control
        .set_listener(Some(recorder.clone() as Arc<dyn LoggerListener>))
        .unwrap();
    control.admit(LogLevel::Error, [4; 16], "access.denied", "topic Square");
    control.admit(LogLevel::Debug, [4; 16], "too.detailed", "not admitted");
    assert_eq!(
        *recorder.0.lock().unwrap(),
        [(
            LogLevel::Error,
            [4; 16],
            "access.denied".to_string(),
            "topic Square".to_string(),
        )]
    );
    // `None` removes the listener.
    control.set_listener(None).unwrap();
    control.admit(LogLevel::Error, [4; 16], "c", "m");
    assert_eq!(recorder.0.lock().unwrap().len(), 1);
}
