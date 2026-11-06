// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-pki` with `zerodds-security-logging`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security-logging`, from its non-test
//! sources: `StderrLoggingPlugin::new().log(level, participant, category,
//! message)` alone (`plugin.rs`: `log_ocsp_outcome` and
//! `log_cert_acceptance` with `LogLevel::Warning`, `log_listener_failure`
//! with `LogLevel::Error`), the sink of last resort when the deployment
//! installed no logger.
//!
//! What it relies on: a sink made with `new()` is enabled at once and takes
//! a Warning and an Error event without `set_log_options` and
//! `enable_logging`; the comment in `plugin.rs` reads "An accepted deviation
//! that produced no record would be the one outcome worse than refusing the
//! certificate".
//!
//! Oracles: the rustdoc of `StderrLoggingPlugin::new` ("Constructor with
//! `DEFAULT_LOG_LEVEL`") and of `DEFAULT_LOG_LEVEL` (Informational),
//! DDS-Security 1.2 §9.6.2.1.1 ("Messages at or below the log_level are
//! logged", FATAL_LEVEL to TRACE_LEVEL from low to high) and §9.6.2.2.4
//! (the listener is notified of each log message the logger produces).
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use zerodds_security::logging::{LogLevel, LoggerListener, LoggingPlugin};
use zerodds_security_logging::{DEFAULT_LOG_LEVEL, StderrLoggingPlugin};
use zerodds_security_pki::LOG_CATEGORY_X509V1_DEVIATION;

/// Records each message the sink produces.
#[derive(Default)]
struct Heard(Mutex<Vec<(LogLevel, [u8; 16], String)>>);

impl LoggerListener for Heard {
    fn log_message(
        &self,
        level: LogLevel,
        participant: [u8; 16],
        category: &str,
        _message: &str,
    ) -> i32 {
        self.0
            .lock()
            .unwrap()
            .push((level, participant, category.to_string()));
        0
    }
}

#[test]
fn security_9_6_2_1_1_the_default_level_takes_warnings_and_errors() {
    assert_eq!(DEFAULT_LOG_LEVEL, LogLevel::Informational);
    assert!(LogLevel::Warning <= DEFAULT_LOG_LEVEL);
    assert!(LogLevel::Error <= DEFAULT_LOG_LEVEL);
}

#[test]
fn security_9_6_2_2_4_a_new_stderr_sink_records_the_events_of_this_crate() {
    let sink = StderrLoggingPlugin::new();
    let heard = Arc::new(Heard::default());
    let listener: Arc<dyn LoggerListener> = heard.clone();
    sink.set_listener(Some(listener)).unwrap();

    let guid = [0x5a; 16];
    // No `set_log_options`, no `enable_logging`: `plugin.rs` calls `log`
    // on a fresh sink.
    sink.log(
        LogLevel::Warning,
        guid,
        LOG_CATEGORY_X509V1_DEVIATION,
        "accepted X.509 v1 local identity certificate",
    );
    sink.log(
        LogLevel::Warning,
        guid,
        "ocsp.unreachable",
        "no revocation status",
    );
    sink.log(
        LogLevel::Error,
        [0; 16],
        "authentication.listener.failed",
        "the listener refused on_revoke_identity",
    );
    // Above the default level: not produced.
    sink.log(LogLevel::Debug, guid, "pki.debug", "below the level");

    assert_eq!(
        *heard.0.lock().unwrap(),
        vec![
            (
                LogLevel::Warning,
                guid,
                LOG_CATEGORY_X509V1_DEVIATION.to_string()
            ),
            (LogLevel::Warning, guid, "ocsp.unreachable".to_string()),
            (
                LogLevel::Error,
                [0; 16],
                "authentication.listener.failed".to_string()
            ),
        ]
    );
}
