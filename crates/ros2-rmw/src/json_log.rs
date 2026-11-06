// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! rcutils-compatible JSON logging.
//!
//! Spec: `zerodds-ros2-bridge-1.0.md` §8.1 (= rcutils logging API).
//!
//! A minimal JSON log layer for bridge-daemon diagnostics. Format:
//!
//! ```json
//! {"ts":"2026-05-06T12:34:56.789Z","level":"INFO","logger":"ros2-bridge","msg":"…"}
//! ```
//!
//! We format without `serde_json` (no_std-friendly) and escape
//! the necessary JSON characters `"`, `\`, control chars.

use alloc::string::String;

/// rcutils-compatible severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// `RCUTILS_LOG_SEVERITY_DEBUG`.
    Debug,
    /// `RCUTILS_LOG_SEVERITY_INFO`.
    Info,
    /// `RCUTILS_LOG_SEVERITY_WARN`.
    Warn,
    /// `RCUTILS_LOG_SEVERITY_ERROR`.
    Error,
    /// `RCUTILS_LOG_SEVERITY_FATAL`.
    Fatal,
}

impl LogLevel {
    /// String as usual in `RCUTILS_CONSOLE_OUTPUT_FORMAT`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Fatal => "FATAL",
        }
    }
}

/// Render a log line as 1-line JSON.
#[must_use]
pub fn render_json(ts_iso: &str, level: LogLevel, logger: &str, msg: &str) -> String {
    let mut out = String::with_capacity(64 + msg.len());
    out.push('{');
    out.push_str("\"ts\":\"");
    push_escaped(&mut out, ts_iso);
    out.push_str("\",\"level\":\"");
    out.push_str(level.as_str());
    out.push_str("\",\"logger\":\"");
    push_escaped(&mut out, logger);
    out.push_str("\",\"msg\":\"");
    push_escaped(&mut out, msg);
    out.push_str("\"}");
    out
}

fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = core::fmt::Write::write_fmt(out, format_args!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// rcutils' severity names (`g_rcutils_log_severity_names` in
    /// rcutils/src/logging.c): DEBUG, INFO, WARN, ERROR, FATAL.
    #[test]
    fn rcutils_logging_the_five_severities_carry_rcutils_names() {
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Fatal.as_str(), "FATAL");
    }

    /// zerodds-ros2-bridge-1.0 §8.1 with `logging.format: "json"`: one
    /// JSON object per line with the members the module documents.
    #[test]
    fn ros2bridge_8_1_a_json_line_is_one_object_with_ts_level_logger_and_msg() {
        let s = render_json(
            "2026-05-06T12:34:56.789Z",
            LogLevel::Info,
            "ros2-bridge",
            "starting up",
        );
        assert_eq!(
            s,
            "{\"ts\":\"2026-05-06T12:34:56.789Z\",\"level\":\"INFO\",\"logger\":\"ros2-bridge\",\"msg\":\"starting up\"}"
        );
    }

    /// RFC 8259 §7: a quotation mark, a reverse solidus and the control
    /// characters U+0000 to U+001F are escaped in a string.
    #[test]
    fn rfc8259_7_quotes_backslashes_and_control_characters_are_escaped() {
        let s = render_json("t", LogLevel::Warn, "x", "she said \"hi\\there\"");
        assert!(
            s.ends_with("\"msg\":\"she said \\\"hi\\\\there\\\"\"}"),
            "{s}"
        );
        let s = render_json("t", LogLevel::Error, "x", "a\nb\tc\rd");
        assert!(s.ends_with("\"msg\":\"a\\nb\\tc\\rd\"}"), "{s}");
        let s = render_json("t", LogLevel::Debug, "x", "\u{1}\u{1f}");
        assert!(s.ends_with("\"msg\":\"\\u0001\\u001f\"}"), "{s}");
    }
}
