// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The JSON log on stdout (spec §8.1): one line per event, `timestamp`
//! (RFC 3339, UTC, milliseconds), `level`, `event`, the `tenant` and
//! `container_id` of the participant whose thread writes it, then the
//! event's fields; filtered by `log_level`.

use std::string::{String, ToString};
use std::sync::atomic::Ordering;
use std::vec::Vec;

use zerodds_monitor::json::unix_millis;
pub(super) use zerodds_monitor::json::{escape, rfc3339_millis as rfc3339};

/// The `log_level` threshold: 0 trace, 1 debug, 2 info, 3 warn, 4 error.
pub(super) static LOG_LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);

pub(super) fn level_rank(level: &str) -> u8 {
    match level {
        "trace" => 0,
        "debug" => 1,
        "warn" => 3,
        "error" => 4,
        _ => 2,
    }
}

/// The level of a log event (§8.1): what went wrong is `warn` or `error`,
/// a line per transfer and per disposition is `debug`, the rest `info`.
pub(super) fn event_level(event: &str) -> &'static str {
    match event {
        "encode_error"
        | "dds_write_failed"
        | "connection_lost"
        | "shutdown_with_unsettled"
        | "shutdown_with_queued"
        | "tenant_stopped" => "error",
        "connect_error"
        | "deadline_missed"
        | "partition_mismatch"
        | "key_hash_unavailable"
        | "undeliverable_here"
        | "redelivery_failed"
        | "link_down"
        | "link_detached"
        | "attach_refused"
        | "rejected"
        | "rejected_by_broker"
        | "released_too_often"
        | "queue_full_dropped_oldest"
        | "dropped_hop_cap"
        | "acl_denied"
        | "settled_message_not_written"
        | "detach_of_unknown_link"
        | "end_of_unknown_session"
        | "signal_watcher_init_failed"
        | "admin_endpoint_bind_failed" => "warn",
        "dropped_own_message" | "transfer_in" | "transfer_out" | "disposition" => "debug",
        _ => "info",
    }
}

/// A field value: text, or a number written as a JSON number.
#[derive(Debug, Clone, Copy)]
pub(super) enum Field<'a> {
    /// Text.
    Text(&'a str),
    /// A number.
    Number(u64),
}

/// The participant a thread runs (§1.1, §8.1): its lines carry these.
#[derive(Default, Clone)]
struct Context {
    tenant: Option<String>,
    container_id: Option<String>,
}

std::thread_local! {
    static CONTEXT: std::cell::RefCell<Context> = std::cell::RefCell::new(Context::default());
}

#[cfg(test)]
std::thread_local! {
    /// The lines [`emit`] wrote on this thread while [`capture`] runs.
    static CAPTURED: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` and returns the lines [`emit`] wrote on this thread meanwhile,
/// instead of writing them on stdout: a test sees the line a call logs.
#[cfg(test)]
pub(super) fn capture(f: impl FnOnce()) -> Vec<String> {
    CAPTURED.with(|c| *c.borrow_mut() = Some(Vec::new()));
    f();
    CAPTURED.with(|c| c.borrow_mut().take().unwrap_or_default())
}

/// Marks the lines of this thread as those of the participant with
/// `container_id`, of tenant `tenant` (§1 L6) or the daemon's own.
pub(super) fn set_participant(tenant: Option<&str>, container_id: Option<&str>) {
    CONTEXT.with(|c| {
        *c.borrow_mut() = Context {
            tenant: tenant.map(str::to_string),
            container_id: container_id.map(str::to_string),
        };
    });
}

/// One JSON log line with text fields on stdout (§8.1), if its level
/// reaches `log_level`.
pub(super) fn log(event: &str, fields: &[(&str, &str)]) {
    let fields: Vec<(&str, Field<'_>)> = fields.iter().map(|(k, v)| (*k, Field::Text(v))).collect();
    emit(event, &fields);
}

/// One JSON log line on stdout (§8.1), if its level reaches `log_level`.
pub(super) fn emit(event: &str, fields: &[(&str, Field<'_>)]) {
    let Some(l) = line(
        event,
        fields,
        unix_millis(),
        LOG_LEVEL.load(Ordering::Relaxed),
    ) else {
        return;
    };
    #[cfg(test)]
    {
        let captured = CAPTURED.with(|c| match c.borrow_mut().as_mut() {
            Some(lines) => {
                lines.push(l.clone());
                true
            }
            None => false,
        });
        if captured {
            return;
        }
    }
    use std::io::Write as _;
    // One write per line, the lock held, so the lines of two threads do
    // not interleave. A closed stdout loses the line; the daemon goes on.
    let out = std::io::stdout();
    let mut out = out.lock();
    let _ = writeln!(out, "{l}");
    let _ = out.flush();
}

/// Whether a line of `event` would be written: callers that build costly
/// fields ask first.
pub(super) fn enabled(event: &str) -> bool {
    level_rank(event_level(event)) >= LOG_LEVEL.load(Ordering::Relaxed)
}

/// The line of `event`, or `None` below the `threshold` (a
/// [`level_rank`]): timestamp, level, event, the participant of this
/// thread, then `fields`.
pub(super) fn line(
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
    CONTEXT.with(|c| {
        let c = c.borrow();
        if let Some(t) = &c.tenant {
            out.push_str(&format!(",\"tenant\":\"{}\"", escape(t)));
        }
        if let Some(id) = &c.container_id {
            out.push_str(&format!(",\"container_id\":\"{}\"", escape(id)));
        }
    });
    for (k, v) in fields {
        match v {
            Field::Text(t) => out.push_str(&format!(",\"{}\":\"{}\"", escape(k), escape(t))),
            Field::Number(n) => out.push_str(&format!(",\"{}\":{n}", escape(k))),
        }
    }
    out.push('}');
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn amqpbridge_8_1_every_event_has_a_level_and_the_threshold_orders_them() {
        assert_eq!(event_level("connected"), "info");
        assert_eq!(event_level("link_down"), "warn");
        assert_eq!(event_level("connection_lost"), "error");
        assert_eq!(event_level("transfer_in"), "debug");
        assert_eq!(event_level("transfer_out"), "debug");
        assert_eq!(event_level("disposition"), "debug");
        assert_eq!(event_level("key_hash_unavailable"), "warn");
        assert!(level_rank("warn") > level_rank("info"));
        assert!(level_rank("error") > level_rank("warn"));
        assert_eq!(level_rank("nonsense"), level_rank("info"));
    }

    /// §8.1: one JSON object with `timestamp`, `level`, `event`, the
    /// participant's `container_id` and the fields of the event, numbers
    /// as numbers.
    #[test]
    fn amqpbridge_8_1_a_line_is_a_json_object_with_the_spec_fields() {
        set_participant(None, Some("zd-1"));
        let l = line(
            "transfer_in",
            &[
                ("link_name", Field::Text("zd-1/T/receiver")),
                ("dds_topic", Field::Text("T")),
                ("amqp_address", Field::Text("/queues/t")),
                ("delivery_id", Field::Number(7)),
                ("bytes", Field::Number(24)),
                ("latency_us", Field::Number(1500)),
            ],
            1_798_761_599_999,
            level_rank("debug"),
        )
        .unwrap();
        assert_eq!(
            l,
            "{\"timestamp\":\"2026-12-31T23:59:59.999Z\",\"level\":\"debug\",\"event\":\"transfer_in\",\"container_id\":\"zd-1\",\"link_name\":\"zd-1/T/receiver\",\"dds_topic\":\"T\",\"amqp_address\":\"/queues/t\",\"delivery_id\":7,\"bytes\":24,\"latency_us\":1500}"
        );
        set_participant(None, None);
        assert!(
            !line("connected", &[], 0, 0)
                .unwrap()
                .contains("container_id"),
            "a thread of no participant has none"
        );
    }

    /// §8.1: `log_level` is the threshold; a transfer line is `debug` and
    /// below the default `info`.
    #[test]
    fn amqpbridge_8_1_the_log_level_is_the_threshold() {
        assert!(line("transfer_in", &[], 0, level_rank("info")).is_none());
        assert!(line("transfer_in", &[], 0, level_rank("debug")).is_some());
        assert!(line("connected", &[], 0, level_rank("info")).is_some());
        assert!(line("connected", &[], 0, level_rank("warn")).is_none());
        assert!(line("link_down", &[], 0, level_rank("warn")).is_some());
        assert!(line("connection_lost", &[], 0, level_rank("error")).is_some());
        assert!(line("link_down", &[], 0, level_rank("error")).is_none());
    }

    /// RFC 8259 §7: a quote, a backslash and a control character in a
    /// value, a broker's error text for example, stay inside the string.
    #[test]
    fn amqpbridge_8_1_control_characters_are_escaped() {
        assert_eq!(escape("a\"b\\c\nd\u{1}\te"), "a\\\"b\\\\c\\nd\\u0001\\te");
        set_participant(None, None);
        let l = line("connect_error", &[("err", Field::Text("x\u{7}\"y"))], 0, 0).unwrap();
        assert!(l.ends_with(",\"err\":\"x\\u0007\\\"y\"}"), "{l}");
    }

    #[test]
    fn rfc3339_5_6_the_utc_date_and_time() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339(1_798_761_599_999), "2026-12-31T23:59:59.999Z");
    }

    /// §1.1: the lines of a thread marked as a tenant's carry `"tenant"`
    /// after level and event; without the mark, or after it is cleared,
    /// there is no such field.
    #[test]
    fn amqpbridge_1_l6_a_tenants_log_lines_carry_the_tenant() {
        set_participant(None, None);
        assert_eq!(
            line("connected", &[("broker", Field::Text("b"))], 0, 0).unwrap(),
            r#"{"timestamp":"1970-01-01T00:00:00.000Z","level":"info","event":"connected","broker":"b"}"#
        );
        set_participant(Some("plant-\"b\""), Some("cb"));
        assert_eq!(
            line("link_down", &[("dds_topic", Field::Text("T"))], 0, 0).unwrap(),
            r#"{"timestamp":"1970-01-01T00:00:00.000Z","level":"warn","event":"link_down","tenant":"plant-\"b\"","container_id":"cb","dds_topic":"T"}"#
        );
        // Another thread is not marked.
        let other = std::thread::spawn(|| line("connected", &[], 0, 0).unwrap())
            .join()
            .unwrap();
        assert!(!other.contains("tenant"), "{other}");
        set_participant(None, None);
        assert!(!line("connected", &[], 0, 0).unwrap().contains("tenant"));
    }
}
