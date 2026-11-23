// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The JSON log on stderr (spec §8.1): one line per event, `timestamp`
//! (RFC 3339, UTC, milliseconds), `level`, `event` and the event's fields,
//! filtered by `log_level`.

#[allow(unused_imports)]
use super::prelude::*;

pub(crate) use zerodds_monitor::json::{escape, rfc3339_millis as rfc3339, unix_millis};

pub(crate) static LOG_LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);

pub(crate) fn level_rank(level: &str) -> u8 {
    match level {
        "trace" => 0,
        "debug" => 1,
        "warn" => 3,
        "error" => 4,
        _ => 2,
    }
}

pub(crate) fn event_level(event: &str) -> &'static str {
    match event {
        "dds_write_failed" | "encode_error" | "reply_write_failed" => "error",
        "timeout"
        | "unknown_object"
        | "bad_operation"
        | "acl_denied"
        | "csiv2_rejected"
        | "giop_version_refused"
        | "reply_without_request"
        | "reply_malformed"
        | "notify_failed"
        | "conn_error"
        | "message_error"
        | "message_too_large"
        | "deadline_missed"
        | "notify_sample_refused" => "warn",
        "request" | "reply" | "notify" | "forward" | "replayed" => "debug",
        _ => "info",
    }
}

/// A field value: text, or a number written as a JSON number.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Field<'a> {
    /// Text.
    Text(&'a str),
    /// A number.
    Number(u64),
}

/// The line of `event`, or `None` below the `threshold` (a
/// [`level_rank`]).
pub(crate) fn line(
    event: &str,
    fields: &[(&str, Field<'_>)],
    unix_millis: u64,
    threshold: u8,
) -> Option<String> {
    let level = event_level(event);
    if level_rank(level) < threshold {
        return None;
    }
    let mut out = format!(
        "{{\"timestamp\":\"{}\",\"level\":\"{level}\",\"event\":\"{}\"",
        rfc3339(unix_millis),
        escape(event)
    );
    for (k, v) in fields {
        match v {
            Field::Text(t) => out.push_str(&format!(",\"{}\":\"{}\"", escape(k), escape(t))),
            Field::Number(n) => out.push_str(&format!(",\"{}\":{n}", escape(k))),
        }
    }
    out.push('}');
    Some(out)
}

/// Writes the line of `event` with text fields.
pub(crate) fn log(event: &str, fields: &[(&str, &str)]) {
    let fields: Vec<(&str, Field<'_>)> = fields.iter().map(|(k, v)| (*k, Field::Text(v))).collect();
    emit(event, &fields);
}

/// Writes the line of `event`.
pub(crate) fn emit(event: &str, fields: &[(&str, Field<'_>)]) {
    if let Some(l) = line(
        event,
        fields,
        unix_millis(),
        LOG_LEVEL.load(Ordering::Relaxed),
    ) {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("{l}");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Spec §8.1: one JSON object with `timestamp`, `level`, `event` and the
    /// fields of the event, numbers as numbers.
    #[test]
    fn corbabridge_8_1_a_line_is_a_json_object_with_the_spec_fields() {
        let l = line(
            "exchange",
            &[
                ("peer", Field::Text("127.0.0.1:40000")),
                ("giop_version", Field::Text("1.2")),
                ("request_id", Field::Number(7)),
                ("operation", Field::Text("add")),
                ("repo_id", Field::Text("IDL:x/Y:1.0")),
                ("bytes", Field::Number(24)),
                ("reply_status", Field::Text("NO_EXCEPTION")),
                ("latency_us", Field::Number(1500)),
            ],
            1_798_761_599_999,
            level_rank("warn"),
        );
        // Below the threshold of `warn` an exchange is not written.
        assert!(l.is_none());
        let l = line(
            "exchange",
            &[
                ("request_id", Field::Number(7)),
                ("operation", Field::Text("add")),
            ],
            1_798_761_599_999,
            level_rank("info"),
        )
        .unwrap();
        assert_eq!(
            l,
            "{\"timestamp\":\"2026-12-31T23:59:59.999Z\",\"level\":\"info\",\"event\":\"exchange\",\"request_id\":7,\"operation\":\"add\"}"
        );
    }

    #[test]
    fn rfc3339_5_6_the_utc_date_and_time() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339(1_798_761_599_999), "2026-12-31T23:59:59.999Z");
    }

    /// RFC 8259 §7: a quote, a backslash and a control character in a
    /// value stay inside the string.
    #[test]
    fn rfc8259_7_text_is_escaped() {
        assert_eq!(escape("a\"b\\c\nd\u{1}"), "a\\\"b\\\\c\\nd\\u0001");
    }

    /// Spec §8.1: `log_level` is the threshold.
    #[test]
    fn corbabridge_8_1_the_log_level_is_the_threshold() {
        assert!(level_rank("warn") > level_rank("info"));
        assert_eq!(level_rank("nonsense"), level_rank("info"));
        assert_eq!(event_level("timeout"), "warn");
        assert_eq!(event_level("dds_write_failed"), "error");
        assert_eq!(event_level("listening"), "info");
        assert!(line("listening", &[], 0, level_rank("info")).is_some());
        assert!(
            line("request", &[], 0, level_rank("info")).is_none(),
            "debug below info"
        );
        assert!(line("listening", &[], 0, level_rank("warn")).is_none());
    }
}
