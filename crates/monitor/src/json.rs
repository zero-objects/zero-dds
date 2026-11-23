// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The text of a JSON log line and of a JSON catalog (spec §6.5): string
//! escaping (RFC 8259 §7), the UTC timestamp with milliseconds (RFC 3339
//! §5.6) the daemons' log lines start with, the five levels of
//! `log_level` and the line itself.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Appends `s` to `out` as the content of a JSON string (RFC 8259 §7):
/// the quotation mark, the reverse solidus and every control character
/// escaped, `\n`, `\r` and `\t` in their short form, the others as
/// `\u00XX`.
pub fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
}

/// `s` as the content of a JSON string ([`push_escaped`]).
#[must_use]
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    push_escaped(&mut out, s);
    out
}

/// The milliseconds since the Unix epoch; 0 for a clock set before it.
#[must_use]
pub fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// RFC 3339 §5.6 in UTC with milliseconds, `1970-01-01T00:00:00.000Z`.
#[must_use]
pub fn rfc3339_millis(unix_millis: u64) -> String {
    let secs = unix_millis / 1000;
    let millis = unix_millis % 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX);
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// The proleptic Gregorian date of a day count since 1970-01-01 (H.
/// Hinnant, "chrono-Compatible Low-Level Date Algorithms").
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The level of a log line, in the order of `log_level`'s values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// `trace`: every frame in and out.
    Trace,
    /// `debug`: the messages of the connections.
    Debug,
    /// `info`: what the daemon did (start, connections, reload, shutdown).
    Info,
    /// `warn`: something was refused or failed; the daemon goes on.
    Warn,
    /// `error`: a part of the daemon failed.
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
    pub const fn name(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    /// Whether a line of this level is written under `threshold`.
    #[must_use]
    pub fn reaches(self, threshold: Self) -> bool {
        self >= threshold
    }
}

/// The threshold `log_level` sets; `info` until it is set.
#[derive(Debug)]
pub struct Threshold(AtomicU8);

impl Threshold {
    /// The threshold `info`.
    #[must_use]
    pub const fn new() -> Self {
        Self(AtomicU8::new(Level::Info as u8))
    }

    /// Sets the threshold to the level `name` names ([`Level::of`]).
    pub fn set(&self, name: &str) {
        self.0.store(Level::of(name) as u8, Ordering::Relaxed);
    }

    /// The threshold in force.
    #[must_use]
    pub fn get(&self) -> Level {
        match self.0.load(Ordering::Relaxed) {
            0 => Level::Trace,
            1 => Level::Debug,
            3 => Level::Warn,
            4 => Level::Error,
            _ => Level::Info,
        }
    }

    /// Whether a line of `level` is written.
    #[must_use]
    pub fn enabled(&self, level: Level) -> bool {
        level.reaches(self.get())
    }
}

impl Default for Threshold {
    fn default() -> Self {
        Self::new()
    }
}

/// The value of a field of a log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A JSON string.
    Text(String),
    /// A JSON number.
    Number(u64),
    /// A JSON number that may be negative.
    Integer(i64),
    /// `true` or `false`.
    Bool(bool),
}

/// The JSON object of one log line (spec §6.5): `timestamp` (RFC 3339 in
/// UTC with milliseconds), `level`, `event`, then `fields` in their order,
/// text escaped, numbers and flags as JSON literals. No line end.
#[must_use]
pub fn render_line(
    unix_millis: u64,
    level: Level,
    event: &str,
    fields: &[(&str, Value)],
) -> String {
    let mut out = String::with_capacity(96 + fields.len() * 24);
    out.push_str("{\"timestamp\":\"");
    out.push_str(&rfc3339_millis(unix_millis));
    out.push_str("\",\"level\":\"");
    out.push_str(level.name());
    out.push_str("\",\"event\":\"");
    push_escaped(&mut out, event);
    out.push('"');
    for (key, value) in fields {
        out.push_str(",\"");
        push_escaped(&mut out, key);
        out.push_str("\":");
        match value {
            Value::Text(t) => {
                out.push('"');
                push_escaped(&mut out, t);
                out.push('"');
            }
            Value::Number(n) => {
                let _ = write!(out, "{n}");
            }
            Value::Integer(n) => {
                let _ = write!(out, "{n}");
            }
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        }
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec §6.5: `log_level` names the threshold; an unknown name is
    /// `info`; a line is written at or above it.
    #[test]
    fn monitor_6_5_the_log_level_is_the_threshold() {
        for name in ["trace", "debug", "info", "warn", "error"] {
            assert_eq!(Level::of(name).name(), name);
        }
        assert_eq!(Level::of("nonsense"), Level::Info);
        let t = Threshold::new();
        assert_eq!(t.get(), Level::Info);
        assert!(t.enabled(Level::Info) && t.enabled(Level::Error));
        assert!(!t.enabled(Level::Debug));
        t.set("error");
        assert_eq!(t.get(), Level::Error);
        assert!(!t.enabled(Level::Warn));
        t.set("trace");
        assert!(t.enabled(Level::Trace));
    }

    /// Spec §6.5: a line is one JSON object, the fields in their order,
    /// text escaped, numbers and flags as literals.
    #[test]
    fn monitor_6_5_a_line_is_one_json_object() {
        let line = render_line(
            1_731_576_600_123,
            Level::Warn,
            "frame_\"in\"",
            &[
                ("peer", Value::Text("a \"b\"\nc".into())),
                ("bytes", Value::Number(42)),
                ("domain", Value::Integer(-1)),
                ("retain", Value::Bool(true)),
            ],
        );
        assert_eq!(
            line,
            "{\"timestamp\":\"2024-11-14T09:30:00.123Z\",\"level\":\"warn\",\
             \"event\":\"frame_\\\"in\\\"\",\"peer\":\"a \\\"b\\\"\\nc\",\
             \"bytes\":42,\"domain\":-1,\"retain\":true}"
        );
    }

    /// Spec §6.5, RFC 8259 §7: the quotation mark, the reverse solidus and
    /// the control characters are escaped, everything else is kept.
    #[test]
    fn monitor_6_5_escape_quotes_backslash_and_controls() {
        assert_eq!(
            escape("a\"b\\c\nd\re\tf\u{1}g\u{1f}h\u{7f}ä"),
            "a\\\"b\\\\c\\nd\\re\\tf\\u0001g\\u001fh\u{7f}ä"
        );
        let mut out = String::from("x=");
        push_escaped(&mut out, "\"");
        assert_eq!(out, "x=\\\"");
    }

    /// Spec §6.5, RFC 3339 §5.6: the date of the epoch, of a leap day, of
    /// the end of a year and of a day after 2100 (not a leap year).
    #[test]
    fn monitor_6_5_rfc3339_utc_with_milliseconds() {
        assert_eq!(rfc3339_millis(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339_millis(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(
            rfc3339_millis(1_798_761_599_999),
            "2026-12-31T23:59:59.999Z"
        );
        assert_eq!(
            rfc3339_millis(4_107_542_400_001),
            "2100-03-01T00:00:00.001Z"
        );
    }

    /// Spec §6.5: the clock is read in milliseconds since the epoch.
    #[test]
    fn monitor_6_5_unix_millis_is_after_the_epoch() {
        assert!(unix_millis() > 1_700_000_000_000);
    }
}
