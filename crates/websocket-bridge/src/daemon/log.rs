// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon's log (spec §8.1): one JSON object per line on stdout, with
//! `timestamp` (RFC 3339, UTC, milliseconds), `level` and `event`, and the
//! fields of the event: `connection_id`, `topic`, `bytes`, `peer`,
//! `latency_us` and others. A line is written when its level reaches
//! `log_level`.

use std::io::Write as _;

pub use zerodds_monitor::json::Level;
pub use zerodds_monitor::json::rfc3339_millis as rfc3339;
use zerodds_monitor::json::{Threshold, Value, render_line, unix_millis};

/// The threshold `log_level` set; `info` until the daemon starts.
static THRESHOLD: Threshold = Threshold::new();

/// Sets the threshold from the configuration's `log_level`.
pub fn set_level(name: &str) {
    THRESHOLD.set(name);
}

/// The threshold in force.
#[must_use]
pub fn threshold() -> Level {
    THRESHOLD.get()
}

/// Whether a line of `level` is written.
#[must_use]
pub fn enabled(level: Level) -> bool {
    THRESHOLD.enabled(level)
}

/// A line being built.
#[derive(Debug)]
#[must_use = "a log event is written by emit()"]
pub struct Event {
    level: Level,
    event: &'static str,
    fields: Vec<(&'static str, Value)>,
}

/// A line of `level` about `event`.
pub fn event(level: Level, event: &'static str) -> Event {
    Event {
        level,
        event,
        fields: Vec::new(),
    }
}

impl Event {
    /// A text field.
    pub fn field(mut self, key: &'static str, value: &str) -> Self {
        self.fields.push((key, Value::Text(value.to_string())));
        self
    }

    /// A number field (`connection_id`, `bytes`, `latency_us`, `code`).
    pub fn number(mut self, key: &'static str, value: u64) -> Self {
        self.fields.push((key, Value::Number(value)));
        self
    }

    /// A number field that may be negative (`domain`).
    pub fn integer(mut self, key: &'static str, value: i64) -> Self {
        self.fields.push((key, Value::Integer(value)));
        self
    }

    /// `latency_us`: the microseconds since `since`.
    pub fn latency(self, since: std::time::Instant) -> Self {
        let us = u64::try_from(since.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.number("latency_us", us)
    }

    /// The line, at `unix_millis`.
    #[must_use]
    pub fn render(&self, unix_millis: u64) -> String {
        render_line(unix_millis, self.level, self.event, &self.fields)
    }

    /// Writes the line to stdout when its level reaches `log_level`.
    pub fn emit(self) {
        if !enabled(self.level) {
            return;
        }
        let millis = unix_millis();
        let mut line = self.render(millis);
        line.push('\n');
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(line.as_bytes());
        let _ = out.flush();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Spec §8.1: a line is one JSON object with `timestamp`, `level`,
    /// `event` and the fields of the spec, numbers as numbers, text escaped.
    #[test]
    fn wsbridge_8_1_a_line_is_a_json_object_with_the_spec_fields() {
        let line = event(Level::Debug, "frame_in")
            .number("connection_id", 7)
            .field("topic", "Chat::Message")
            .number("bytes", 42)
            .field("peer", "127.0.0.1:50000")
            .number("latency_us", 380)
            .integer("domain", -1)
            .field("reason", "a \"quoted\"\nline")
            .render(1_731_576_600_123);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["timestamp"], "2024-11-14T09:30:00.123Z");
        assert_eq!(v["level"], "debug");
        assert_eq!(v["event"], "frame_in");
        assert_eq!(v["connection_id"], 7);
        assert_eq!(v["topic"], "Chat::Message");
        assert_eq!(v["bytes"], 42);
        assert_eq!(v["peer"], "127.0.0.1:50000");
        assert_eq!(v["latency_us"], 380);
        assert_eq!(v["domain"], -1);
        assert_eq!(v["reason"], "a \"quoted\"\nline");
        assert!(!line.contains('\n'), "one line");
    }

    /// RFC 3339 §5.6: the date of the epoch, of a leap day and of the end
    /// of a year.
    #[test]
    fn rfc3339_5_6_the_utc_date_and_time() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339(1_798_761_599_999), "2026-12-31T23:59:59.999Z");
    }

    /// Spec §8.1: `log_level` is the threshold; a line below it is not
    /// written, a line at or above it is.
    #[test]
    fn wsbridge_8_1_the_log_level_is_the_threshold() {
        assert!(Level::of("warn") > Level::of("info"));
        assert_eq!(Level::of("nonsense"), Level::Info);
        for name in ["trace", "debug", "info", "warn", "error"] {
            assert_eq!(Level::of(name).name(), name);
        }
        assert!(Level::Info.reaches(Level::Info));
        assert!(Level::Warn.reaches(Level::Info));
        assert!(!Level::Debug.reaches(Level::Info));
        assert!(!Level::Info.reaches(Level::Error));
        assert!(Level::Error.reaches(Level::Error));
        assert!(Level::Trace.reaches(Level::Trace));
    }
}
