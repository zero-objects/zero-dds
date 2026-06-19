// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! JSON-Lines-File-Backend (Audit-tauglich).

use alloc::string::String;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

use zerodds_security::logging::{LogLevel, LoggingPlugin};

/// Writes security events as JSON lines to a file.
///
/// Each line is a standalone JSON object:
/// ```json
/// {"ts":"2026-04-24T11:22:33Z","level":"CRITICAL","participant":"aabbcc...","category":"auth.handshake.failed","message":"bad cert"}
/// ```
///
/// Typical use: `auditd`/`filebeat`/`promtail` collects the
/// file. This plugin itself performs **no** log rotation —
/// that is the job of `logrotate(8)` or systemd-journald.
pub struct JsonLinesLoggingPlugin {
    min_level: LogLevel,
    writer: Mutex<BufWriter<File>>,
}

impl JsonLinesLoggingPlugin {
    /// Opens / creates the file and wraps it in a BufWriter.
    ///
    /// # Errors
    /// `io::Error` if the file cannot be opened
    /// (permissions, parent dir missing).
    pub fn open<P: AsRef<Path>>(path: P, min_level: LogLevel) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            min_level,
            writer: Mutex::new(BufWriter::new(file)),
        })
    }
}

fn level_name(l: LogLevel) -> &'static str {
    match l {
        LogLevel::Emergency => "EMERGENCY",
        LogLevel::Alert => "ALERT",
        LogLevel::Critical => "CRITICAL",
        LogLevel::Error => "ERROR",
        LogLevel::Warning => "WARNING",
        LogLevel::Notice => "NOTICE",
        LogLevel::Informational => "INFORMATIONAL",
        LogLevel::Debug => "DEBUG",
    }
}

/// Simple JSON escaping: only the control chars + `"` + `\`.
/// Enough for log messages; we don't need serde for
/// a 5-field emit.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&alloc::format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn hex16(bytes: [u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&alloc::format!("{b:02x}"));
    }
    s
}

impl LoggingPlugin for JsonLinesLoggingPlugin {
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        if level > self.min_level {
            return;
        }
        let line = alloc::format!(
            r#"{{"level":"{lvl}","participant":"{pid}","category":"{cat}","message":"{msg}"}}"#,
            lvl = level_name(level),
            pid = hex16(participant),
            cat = escape_json(category),
            msg = escape_json(message),
        );
        if let Ok(mut w) = self.writer.lock() {
            // `writeln!` + `flush` — otherwise a BufWriter would lose
            // the last events on a crash.
            let _ = writeln!(w, "{line}");
            let _ = w.flush();
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:jsonl"
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(alloc::format!(
            "zerodds_sec_log_{}_{name}.jsonl",
            std::process::id()
        ));
        // clean leftovers
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn writes_json_lines_for_events_above_threshold() {
        let path = tmp_path("threshold");
        let plugin = JsonLinesLoggingPlugin::open(&path, LogLevel::Warning).expect("open");
        plugin.log(LogLevel::Critical, [0xAA; 16], "auth.fail", "bad cert");
        plugin.log(LogLevel::Informational, [0xBB; 16], "debug", "ignored");
        // flush via Drop.
        drop(plugin);

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "only Critical should get through");
        assert!(lines[0].contains("\"level\":\"CRITICAL\""));
        assert!(lines[0].contains("\"category\":\"auth.fail\""));
        assert!(lines[0].contains("\"message\":\"bad cert\""));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_escapes_quotes_and_backslash() {
        let path = tmp_path("escape");
        let plugin = JsonLinesLoggingPlugin::open(&path, LogLevel::Debug).expect("open");
        plugin.log(LogLevel::Warning, [0u8; 16], "raw", "he said \"hi\\hello\"");
        drop(plugin);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains(r#"\"hi\\hello\""#));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_escapes_newline_as_backslash_n() {
        let path = tmp_path("nl");
        let plugin = JsonLinesLoggingPlugin::open(&path, LogLevel::Debug).expect("open");
        plugin.log(LogLevel::Warning, [0u8; 16], "multi\nline", "a\nb");
        drop(plugin);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains(r"multi\nline"));
        assert!(content.contains(r#""message":"a\nb""#));
        // Exactly ONE line — no real LF in the output.
        assert_eq!(content.lines().count(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn plugin_class_id_stable() {
        let path = tmp_path("class");
        let plugin = JsonLinesLoggingPlugin::open(&path, LogLevel::Warning).expect("open");
        assert_eq!(plugin.plugin_class_id(), "DDS:Logging:jsonl");
        drop(plugin);
        let _ = std::fs::remove_file(&path);
    }
}
