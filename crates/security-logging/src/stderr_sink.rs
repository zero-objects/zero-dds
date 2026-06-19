// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stderr-Backend.

use alloc::string::String;
use std::io::{self, Write};
use std::sync::Mutex;

use zerodds_security::logging::{LogLevel, LoggingPlugin};

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
    min_level: LogLevel,
    serializer: Mutex<()>,
}

impl Default for StderrLoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl StderrLoggingPlugin {
    /// Constructor with default level `Warning`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_level(LogLevel::Warning)
    }

    /// Mit explizitem Min-Level.
    #[must_use]
    pub fn with_level(min_level: LogLevel) -> Self {
        Self {
            min_level,
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
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        // `level <= min_level` because LogLevel 0=Emergency and 7=Debug.
        if level > self.min_level {
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
