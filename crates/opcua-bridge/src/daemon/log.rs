// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon's log (spec §8.1): lines on stderr, each with its level,
//! written when the level reaches `log_level`.

use std::string::String;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::vec::Vec;

/// The level of a log line, in the order of `log_level`'s values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// `trace`.
    Trace,
    /// `debug`.
    Debug,
    /// `info`: what the daemon did.
    Info,
    /// `warn`: something was skipped or lost, the daemon goes on.
    Warn,
    /// `error`: a sample was not bridged, or a part of the daemon failed.
    Error,
}

impl Level {
    /// The level a `log_level` value names; `info` for any other text (the
    /// configuration accepts only the five names).
    #[must_use]
    pub fn of(name: &str) -> Self {
        match name {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }

    /// The name as `log_level` spells it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn rank(self) -> u8 {
        self as u8
    }
}

/// The threshold `log_level` set; `info` until the daemon starts.
static THRESHOLD: AtomicU8 = AtomicU8::new(Level::Info as u8);

/// The lines written since [`capture`], while a capture runs.
static CAPTURED: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Sets the threshold from the configuration's `log_level`.
pub fn set_level(name: &str) {
    THRESHOLD.store(Level::of(name).rank(), Ordering::Relaxed);
}

/// The line for `message` at `level`, or `None` when `level` is below
/// `threshold`.
#[must_use]
pub fn line(level: Level, threshold: Level, message: &str) -> Option<String> {
    (level >= threshold)
        .then(|| std::format!("[zerodds-opcua-bridged] {}: {message}", level.name()))
}

/// Writes `message` to stderr when `level` reaches the threshold.
pub fn emit(level: Level, message: core::fmt::Arguments<'_>) {
    if level.rank() < THRESHOLD.load(Ordering::Relaxed) {
        return;
    }
    let line = std::format!("[zerodds-opcua-bridged] {}: {message}", level.name());
    if let Ok(mut captured) = CAPTURED.lock()
        && let Some(lines) = captured.as_mut()
    {
        lines.push(line.clone());
    }
    #[allow(clippy::print_stderr)]
    {
        std::eprintln!("{line}");
    }
}

/// Keeps the lines written from now on as well, for [`captured`]: what a
/// test of the daemon in its own process reads the log by.
pub fn capture() {
    if let Ok(mut captured) = CAPTURED.lock() {
        *captured = Some(Vec::new());
    }
}

/// The lines written since [`capture`].
#[must_use]
pub fn captured() -> Vec<String> {
    CAPTURED
        .lock()
        .ok()
        .and_then(|captured| captured.clone())
        .unwrap_or_default()
}

/// Writes a line of the daemon's log: `daemon_log!(Level::Warn, "…", …)`.
macro_rules! daemon_log {
    ($level:expr, $($arg:tt)*) => {
        $crate::daemon::log::emit($level, core::format_args!($($arg)*))
    };
}
pub(crate) use daemon_log;

#[cfg(test)]
mod tests {
    use super::*;

    /// §8.1: `log_level` is the lowest level written.
    #[test]
    fn opcuabridge_8_1_a_line_below_the_log_level_is_not_written() {
        assert_eq!(
            line(Level::Warn, Level::Info, "topic `T`: skipped").as_deref(),
            Some("[zerodds-opcua-bridged] warn: topic `T`: skipped")
        );
        assert_eq!(line(Level::Info, Level::Warn, "started"), None);
        assert!(line(Level::Error, Level::Error, "x").is_some());
        assert!(line(Level::Warn, Level::Error, "x").is_none());
        assert!(line(Level::Trace, Level::Trace, "x").is_some());
        let order = ["trace", "debug", "info", "warn", "error"].map(Level::of);
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            order.map(Level::name),
            ["trace", "debug", "info", "warn", "error"]
        );
    }
}
