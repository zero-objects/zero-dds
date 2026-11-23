// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon's log (spec §8.1): one JSON object per line on stdout, with
//! `timestamp` (RFC 3339, UTC, milliseconds), `level` and `event`, and the
//! fields of the event: `peer`, `coap_uri`, `dds_topic`, `mid`,
//! `token_hex`, `block_num`, `bytes`, `latency_us` and others. A line is
//! written when its level reaches `log_level`.

use std::io::Write as _;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_monitor::json::Level;
pub use zerodds_monitor::json::rfc3339_millis as rfc3339;
use zerodds_monitor::json::{Threshold, Value, render_line, unix_millis};

/// The threshold `log_level` set; `info` until the daemon starts.
static THRESHOLD: Threshold = Threshold::new();

/// Sets the threshold from the configuration's `log_level`.
pub fn set_level(name: &str) {
    THRESHOLD.set(name);
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

    /// A number field (`mid`, `block_num`, `bytes`, `latency_us`).
    pub fn number(mut self, key: &'static str, value: u64) -> Self {
        self.fields.push((key, Value::Number(value)));
        self
    }

    /// `token_hex`: the token in lowercase hexadecimal.
    pub fn token(self, token: &[u8]) -> Self {
        use core::fmt::Write as _;
        let mut hex = String::with_capacity(token.len() * 2);
        for b in token {
            let _ = write!(hex, "{b:02x}");
        }
        self.field("token_hex", &hex)
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
    /// `event` and the event's fields, numbers as numbers, text escaped.
    #[test]
    fn coapbridge_8_1_a_line_is_a_json_object_with_the_spec_fields() {
        let line = event(Level::Info, "request")
            .field("peer", "127.0.0.1:5683")
            .field("coap_uri", "chat/message")
            .field("dds_topic", "Chat::Message")
            .number("mid", 4660)
            .token(&[0xde, 0xad])
            .number("block_num", 2)
            .number("bytes", 17)
            .number("latency_us", 380)
            .field("note", "a \"quoted\"\nline")
            .render(1_731_576_600_123);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["timestamp"], "2024-11-14T09:30:00.123Z");
        assert_eq!(v["level"], "info");
        assert_eq!(v["event"], "request");
        assert_eq!(v["peer"], "127.0.0.1:5683");
        assert_eq!(v["coap_uri"], "chat/message");
        assert_eq!(v["dds_topic"], "Chat::Message");
        assert_eq!(v["mid"], 4660);
        assert_eq!(v["token_hex"], "dead");
        assert_eq!(v["block_num"], 2);
        assert_eq!(v["bytes"], 17);
        assert_eq!(v["latency_us"], 380);
        assert_eq!(v["note"], "a \"quoted\"\nline");
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
    /// written.
    #[test]
    fn coapbridge_8_1_the_log_level_is_the_threshold() {
        assert!(Level::of("warn") > Level::of("info"));
        assert_eq!(Level::of("nonsense"), Level::Info);
        assert!(Level::Error > Level::Warn && Level::Debug < Level::Info);
        assert_eq!(Level::of("trace").name(), "trace");
    }
}
