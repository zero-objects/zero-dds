// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! rcutils-kompatibles JSON-Logging.
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

    #[test]
    fn level_strings_match_rcutils() {
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Fatal.as_str(), "FATAL");
    }

    #[test]
    fn render_json_simple() {
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

    #[test]
    fn render_json_escapes_quotes_and_backslash() {
        let s = render_json("t", LogLevel::Warn, "x", "she said \"hi\\there\"");
        // `"` → `\"`, `\` → `\\`.
        assert!(s.contains("\\\"hi\\\\there\\\""));
    }

    #[test]
    fn render_json_escapes_newline_and_tab() {
        let s = render_json("t", LogLevel::Error, "x", "a\nb\tc");
        assert!(s.contains("a\\nb\\tc"));
    }

    #[test]
    fn render_json_escapes_control_char() {
        let s = render_json("t", LogLevel::Debug, "x", "\x01");
        assert!(s.contains("\\u0001"));
    }
}
