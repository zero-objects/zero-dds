// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stderr backend.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The listener of `set_listener` (DDS-Security 1.2 §9.6.2.2.4) is a `dyn LoggerListener`.)

use alloc::string::String;
use std::io::{self, Write};
use std::sync::Mutex;

use alloc::sync::Arc;
use zerodds_security::error::SecurityError;
use zerodds_security::logging::{LogLevel, LogOptions, LoggerListener, LoggingPlugin};

use crate::control::LogControl;

/// Logs security events to `stderr` as human-readable text.
///
/// Format:
/// ```text
/// [SEC][<LEVEL>] participant=<hex16> category=<...> msg=<...>
/// ```
///
/// A mutex around `io::Stderr` serializes writes — otherwise parallel
/// writers could interleave the lines (stderr is line-buffered
/// **only** when it goes to the terminal; in a pipeline it is
/// fully buffered, or atomic per write(2) only up to PIPE_BUF bytes).
pub struct StderrLoggingPlugin {
    control: LogControl,
    serializer: Mutex<()>,
}

impl Default for StderrLoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl StderrLoggingPlugin {
    /// Constructor with [`crate::DEFAULT_LOG_LEVEL`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_level(crate::DEFAULT_LOG_LEVEL)
    }

    /// With explicit min level.
    #[must_use]
    pub fn with_level(min_level: LogLevel) -> Self {
        Self {
            control: LogControl::enabled_at(min_level, false),
            serializer: Mutex::new(()),
        }
    }
}

fn level_label(l: LogLevel) -> &'static str {
    match l {
        LogLevel::Emergency => "EMERG",
        LogLevel::Alert => "ALERT",
        LogLevel::Critical => "CRIT",
        LogLevel::Error => "ERROR",
        LogLevel::Warning => "WARN",
        LogLevel::Notice => "NOTICE",
        LogLevel::Informational => "INFO",
        LogLevel::Debug => "DEBUG",
        _ => "TRACE",
    }
}

fn hex16(bytes: [u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&alloc::format!("{b:02x}"));
    }
    s
}

impl LoggingPlugin for StderrLoggingPlugin {
    // The sink writes to stderr only; it has no DataWriter to distribute.
    sink_control_operations!();

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        // Not enabled, or above the saved level (LogLevel 0=Emergency is the
        // lowest): nothing to do (DDS-Security 1.2 §10.6.1).
        if self
            .control
            .admit(level, participant, category, message)
            .is_none()
        {
            return;
        }
        let _guard = self.serializer.lock().ok();
        let mut out = io::stderr().lock();
        let _ = writeln!(
            out,
            "[SEC][{level}] participant={pid} category={cat} msg={msg}",
            level = level_label(level),
            pid = hex16(participant),
            cat = category,
            msg = message,
        );
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:stderr"
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn plugin_class_id_stable() {
        assert_eq!(
            StderrLoggingPlugin::new().plugin_class_id(),
            "DDS:Logging:stderr"
        );
    }

    /// The constructor and the property path without `org.zerodds.sec.log.level`
    /// start at the same level.
    #[test]
    fn new_logs_from_the_default_level() {
        assert_eq!(
            StderrLoggingPlugin::new()
                .control
                .options()
                .map(|o| o.log_level),
            Some(crate::DEFAULT_LOG_LEVEL)
        );
    }

    #[test]
    fn level_label_covers_all_variants() {
        for &lvl in &[
            LogLevel::Emergency,
            LogLevel::Alert,
            LogLevel::Critical,
            LogLevel::Error,
            LogLevel::Warning,
            LogLevel::Notice,
            LogLevel::Informational,
            LogLevel::Debug,
        ] {
            assert!(!level_label(lvl).is_empty());
        }
    }

    #[test]
    fn hex16_pads_single_digits() {
        let bytes = [0x01, 0x0a, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(hex16(bytes), "010aff00000000000000000000000000");
    }
}
