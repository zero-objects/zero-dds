// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The JSON log on stdout (spec §8.1): one object per line with
//! `timestamp` (RFC 3339, UTC, milliseconds), `level`, `event` and the
//! event's fields, filtered by `log_level`. A finished call is one `rpc`
//! line with `peer`, `service`, `method`, `dds_topic`, `bytes`,
//! `grpc_status`, `latency_us` and `stream_id`.

use std::io::Write as _;
use std::sync::atomic::{AtomicU8, Ordering};

pub(crate) use zerodds_monitor::json::{escape, rfc3339_millis as rfc3339, unix_millis};

use crate::status::Status;

/// The levels of `log_level` (Spec §2 `--log-level`, §3), lowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl Level {
    /// The level of a `log_level` value; the reader allows only the five.
    pub(crate) fn of(name: &str) -> Self {
        match name {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }

    /// The `level` field of a line.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// The threshold of the lines: `log_level`, default `info`.
pub(crate) static LOG_LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);

/// Whether a line of `level` goes out under the configured `log_level`.
pub(crate) fn log_enabled(level: Level) -> bool {
    level as u8 >= LOG_LEVEL.load(Ordering::Relaxed)
}

/// Sets the threshold from a `log_level` value.
pub(crate) fn set_log_level(name: &str) {
    LOG_LEVEL.store(Level::of(name) as u8, Ordering::Relaxed);
}

/// One line being built; below the threshold nothing is built.
#[must_use = "a line goes out with emit()"]
pub(crate) struct Line(Option<String>);

/// The line of `event` at `level`, at the time of now.
pub(crate) fn event(level: Level, name: &str) -> Line {
    if !log_enabled(level) {
        return Line(None);
    }
    Line::at(level, name, unix_millis())
}

impl Line {
    /// The line of `event` at `level` and the time `unix_millis`, whatever
    /// the threshold.
    pub(crate) fn at(level: Level, name: &str, unix_millis: u64) -> Self {
        Self(Some(format!(
            "{{\"timestamp\":\"{}\",\"level\":\"{}\",\"event\":\"{}\"",
            rfc3339(unix_millis),
            level.name(),
            escape(name)
        )))
    }

    /// A text field.
    pub(crate) fn text(mut self, key: &str, value: &str) -> Self {
        if let Some(out) = self.0.as_mut() {
            out.push_str(&format!(",\"{}\":\"{}\"", escape(key), escape(value)));
        }
        self
    }

    /// A number field, written as a JSON number.
    pub(crate) fn num(mut self, key: &str, value: u64) -> Self {
        if let Some(out) = self.0.as_mut() {
            out.push_str(&format!(",\"{}\":{value}", escape(key)));
        }
        self
    }

    /// A signed number field.
    pub(crate) fn int(mut self, key: &str, value: i64) -> Self {
        if let Some(out) = self.0.as_mut() {
            out.push_str(&format!(",\"{}\":{value}", escape(key)));
        }
        self
    }

    /// A boolean field.
    pub(crate) fn flag(mut self, key: &str, value: bool) -> Self {
        if let Some(out) = self.0.as_mut() {
            out.push_str(&format!(",\"{}\":{value}", escape(key)));
        }
        self
    }

    /// A text field when there is a value.
    pub(crate) fn text_opt(self, key: &str, value: Option<&str>) -> Self {
        match value {
            Some(v) => self.text(key, v),
            None => self,
        }
    }

    /// The finished object, `None` below the threshold.
    pub(crate) fn finish(self) -> Option<String> {
        self.0.map(|mut out| {
            out.push('}');
            out
        })
    }

    /// Writes the line to stdout. A closed stdout loses the line; it does
    /// not stop the daemon.
    pub(crate) fn emit(self) {
        if let Some(line) = self.finish() {
            write_line(&line);
        }
    }
}

/// A moment on both clocks: the monotonic one for `latency_us` (spec
/// §8.1), the Unix time for the spans (§8.3).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Clock {
    at: std::time::Instant,
    /// Nanoseconds since the Unix epoch.
    pub(crate) unix_ns: u64,
}

impl Clock {
    /// Now.
    pub(crate) fn now() -> Self {
        Self {
            at: std::time::Instant::now(),
            unix_ns: super::trace::unix_ns(),
        }
    }

    /// Microseconds since this moment.
    pub(crate) fn elapsed_us(&self) -> u64 {
        u64::try_from(self.at.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    /// Now in Unix nanoseconds, measured from this moment on the monotonic
    /// clock: a span does not end before it starts when the wall clock is
    /// set back.
    pub(crate) fn unix_ns_now(&self) -> u64 {
        let elapsed = u64::try_from(self.at.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.unix_ns.saturating_add(elapsed)
    }
}

/// A finished call (spec §8.1): the fields of its `rpc` line.
#[derive(Debug, Clone, Default)]
pub(crate) struct CallLine<'a> {
    pub(crate) peer: &'a str,
    pub(crate) service: &'a str,
    pub(crate) method: &'a str,
    /// The DDS topic of the call (`<tenant>/<name>` for a tenant's), empty
    /// for a call without one (reflection, a call refused before its
    /// route).
    pub(crate) dds_topic: &'a str,
    /// The octets of the call's Length-Prefixed-Messages, both ways,
    /// each with its 5-octet prefix (as §8.2 counts them).
    pub(crate) bytes: u64,
    pub(crate) status: Option<Status>,
    pub(crate) message: Option<&'a str>,
    pub(crate) latency_us: u64,
    pub(crate) stream_id: u32,
}

/// The level of the `rpc` line: `info` for OK, `warn` for every other
/// status.
pub(crate) fn call_level(status: Status) -> Level {
    if status == Status::Ok {
        Level::Info
    } else {
        Level::Warn
    }
}

/// The `rpc` line of `call`, at the time `unix_millis` (spec §8.1).
pub(crate) fn call_line(call: &CallLine<'_>, unix_millis: u64) -> String {
    let status = call.status.unwrap_or(Status::Unknown);
    Line::at(call_level(status), "rpc", unix_millis)
        .text("peer", call.peer)
        .text("service", call.service)
        .text("method", call.method)
        .text("dds_topic", call.dds_topic)
        .num("bytes", call.bytes)
        .num("grpc_status", u64::from(status.code()))
        .num("latency_us", call.latency_us)
        .num("stream_id", u64::from(call.stream_id))
        .text_opt("grpc_message", call.message)
        .finish()
        .unwrap_or_default()
}

/// Writes the `rpc` line of `call` when its level reaches `log_level`.
pub(crate) fn log_call(call: &CallLine<'_>) {
    if log_enabled(call_level(call.status.unwrap_or(Status::Unknown))) {
        write_line(&call_line(call, unix_millis()));
    }
}

/// Writes one line to stdout. A closed stdout loses the line; it does not
/// stop the daemon.
fn write_line(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn call() -> CallLine<'static> {
        CallLine {
            peer: "127.0.0.1:40000",
            service: "zerodds.chat.v1.ChatMessageStream",
            method: "Publish",
            dds_topic: "Chat::Message",
            bytes: 42,
            status: Some(Status::Ok),
            message: None,
            latency_us: 1500,
            stream_id: 3,
        }
    }

    /// Spec §8.1: a finished call is one JSON object with `timestamp`,
    /// `level`, `event` and the fields of the spec, numbers as numbers.
    #[test]
    fn grpcbridge_8_1_a_call_line_carries_the_fields_of_the_spec() {
        assert_eq!(
            call_line(&call(), 1_798_761_599_999),
            "{\"timestamp\":\"2026-12-31T23:59:59.999Z\",\"level\":\"info\",\"event\":\"rpc\",\
             \"peer\":\"127.0.0.1:40000\",\"service\":\"zerodds.chat.v1.ChatMessageStream\",\
             \"method\":\"Publish\",\"dds_topic\":\"Chat::Message\",\"bytes\":42,\
             \"grpc_status\":0,\"latency_us\":1500,\"stream_id\":3}"
        );
    }

    /// Spec §8.1: a call that did not end OK is a `warn` line with its
    /// `grpc_message`.
    #[test]
    fn grpcbridge_8_1_a_failed_call_is_warn_with_its_message() {
        let failed = CallLine {
            status: Some(Status::InvalidArgument),
            message: Some("Sample: truncated"),
            ..call()
        };
        let line = call_line(&failed, 0);
        assert!(line.contains("\"level\":\"warn\""), "{line}");
        assert!(line.contains("\"grpc_status\":3"), "{line}");
        assert!(
            line.ends_with(",\"grpc_message\":\"Sample: truncated\"}"),
            "{line}"
        );
    }

    /// RFC 8259 §7: a quote, a backslash and a control character in a
    /// value stay inside the string; the line stays one line.
    #[test]
    fn grpcbridge_8_1_values_are_json_escaped() {
        assert_eq!(escape("a\"b\\c\nd\u{1}"), "a\\\"b\\\\c\\nd\\u0001");
        let hostile = CallLine {
            method: "M\"\n{\"event\":\"forged\"}",
            ..call()
        };
        let line = call_line(&hostile, 0);
        assert!(!line.contains('\n'), "{line}");
        assert!(
            line.contains("\"method\":\"M\\\"\\n{\\\"event\\\":\\\"forged\\\"}\""),
            "{line}"
        );
    }

    /// RFC 3339 §5.6: UTC with milliseconds.
    #[test]
    fn grpcbridge_8_1_rfc3339_is_utc_with_milliseconds() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339(1_798_761_599_999), "2026-12-31T23:59:59.999Z");
    }

    /// Spec §2 `--log-level`, §3 `log_level`: the threshold; a line below
    /// it is not built.
    #[test]
    fn grpcbridge_8_1_the_threshold_drops_lines_below_log_level() {
        assert!(Level::Warn > Level::Info);
        assert_eq!(Level::of("nonsense"), Level::Info);
        assert_eq!(call_level(Status::Ok), Level::Info);
        assert_eq!(call_level(Status::Unavailable), Level::Warn);
        let before = LOG_LEVEL.load(Ordering::Relaxed);
        set_log_level("warn");
        assert!(event(Level::Info, "listening").finish().is_none());
        assert!(event(Level::Warn, "drain_limit").finish().is_some());
        set_log_level("debug");
        assert!(event(Level::Debug, "accept").finish().is_some());
        LOG_LEVEL.store(before, Ordering::Relaxed);
    }

    /// Spec §8.1: numbers and flags are JSON numbers and booleans, text is
    /// a string.
    #[test]
    fn grpcbridge_8_1_numbers_are_json_numbers() {
        let line = Line::at(Level::Info, "dds_started", 0)
            .int("domain", -1)
            .num("topics", 2)
            .flag("reflection", true)
            .text("addr", "127.0.0.1:50051")
            .finish()
            .unwrap();
        assert!(
            line.ends_with(
                ",\"event\":\"dds_started\",\"domain\":-1,\"topics\":2,\"reflection\":true,\"addr\":\"127.0.0.1:50051\"}"
            ),
            "{line}"
        );
    }
}
