// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon's log (Spec §8.1): one JSON object per line on stdout, with
//! `timestamp` (RFC 3339, UTC, milliseconds), `level` and `event`, and the
//! fields of the event: `mqtt_topic`, `dds_topic`, `bytes`, `mqtt_qos`,
//! `latency_us`, `connect_state` and the others the table of §8.1 names. A
//! line is written when its level reaches `log_level`. The Windows
//! service, which has no stdout, writes the lines to a file instead
//! ([`to_file`], [`service_log_path`]; Spec §11).

use std::ffi::OsStr;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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

/// The file the lines go to in place of stdout ([`to_file`]).
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Spec §8.1, §11: from now on the lines are appended to the file `path`
/// instead of stdout; its directory is made when it is missing.
///
/// # Errors
/// The directory cannot be made or the file cannot be opened for
/// appending; the lines then stay on stdout.
pub fn to_file(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    match FILE.lock() {
        Ok(mut f) => {
            *f = Some(file);
            Ok(())
        }
        Err(_) => Err(std::io::Error::other("the log's file lock is poisoned")),
    }
}

/// The log file of the Windows service (Spec §11, zerodds-deployment-1.0
/// §4.1): `ZeroDDS\logs\mqtt-bridged.log` under `programdata`, the value
/// of `%PROGRAMDATA%`; `C:\ProgramData` when it is not set or empty.
#[must_use]
pub fn service_log_path(programdata: Option<&OsStr>) -> PathBuf {
    let base = programdata
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from);
    base.join("ZeroDDS").join("logs").join("mqtt-bridged.log")
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

    /// A number field (`bytes`, `mqtt_qos`, `latency_us`, a count).
    pub fn number(mut self, key: &'static str, value: u64) -> Self {
        self.fields.push((key, Value::Number(value)));
        self
    }

    /// A number field that may be negative (`domain`).
    pub fn integer(mut self, key: &'static str, value: i64) -> Self {
        self.fields.push((key, Value::Integer(value)));
        self
    }

    /// A field `true` or `false`.
    pub fn flag(mut self, key: &'static str, value: bool) -> Self {
        self.fields.push((key, Value::Bool(value)));
        self
    }

    /// `tenant` (Spec §1 L6): the tenant whose connection the line is
    /// about; left out for the top level (`name` empty).
    pub fn tenant(self, name: &str) -> Self {
        if name.is_empty() {
            self
        } else {
            self.field("tenant", name)
        }
    }

    /// `reason_code`: a Reason Code as `0x8e`, as `reason_code` of §8.2.
    pub fn reason_code(self, code: u8) -> Self {
        self.field("reason_code", &format!("0x{code:02x}"))
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

    /// Writes the line when its level reaches `log_level`: to the file of
    /// [`to_file`] when one is set, else to stdout. A line that cannot be
    /// written is lost: the log is where the daemon would say so.
    pub fn emit(self) {
        if !enabled(self.level) {
            return;
        }
        let millis = unix_millis();
        let mut line = self.render(millis);
        line.push('\n');
        if let Ok(mut file) = FILE.lock() {
            if let Some(f) = file.as_mut() {
                let _ = f.write_all(line.as_bytes());
                return;
            }
        }
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
    /// `event` and the fields of the spec, numbers as numbers.
    #[test]
    fn mqttbridge_8_1_a_line_is_one_json_object_with_timestamp_level_and_event() {
        let line = event(Level::Debug, "publish_out")
            .field("mqtt_topic", "plant/temp")
            .field("dds_topic", "Plant::Temp")
            .number("bytes", 42)
            .number("mqtt_qos", 1)
            .number("latency_us", 380)
            .field("connect_state", "connected")
            .integer("domain", -1)
            .flag("session_present", true)
            .reason_code(0x8e)
            .render(1_731_576_600_123);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["timestamp"], "2024-11-14T09:30:00.123Z");
        assert_eq!(v["level"], "debug");
        assert_eq!(v["event"], "publish_out");
        assert_eq!(v["mqtt_topic"], "plant/temp");
        assert_eq!(v["dds_topic"], "Plant::Temp");
        assert_eq!(v["bytes"], 42);
        assert_eq!(v["mqtt_qos"], 1);
        assert_eq!(v["latency_us"], 380);
        assert_eq!(v["connect_state"], "connected");
        assert_eq!(v["domain"], -1);
        assert_eq!(v["session_present"], true);
        assert_eq!(v["reason_code"], "0x8e");
        assert!(!line.contains('\n'), "one line");
    }

    /// Spec §8.1: text with quotes, a backslash, line breaks and control
    /// characters stays one line and reads back as it was.
    #[test]
    fn mqttbridge_8_1_text_is_escaped() {
        let text = "a \"quoted\"\nline\twith \\ and \u{1}";
        let line = event(Level::Warn, "connect_failed")
            .field("reason", text)
            .render(0);
        assert!(!line.contains('\n'), "{line}");
        assert!(line.contains("\\u0001"), "{line}");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["reason"], text);
    }

    /// RFC 3339 §5.6: the date of the epoch, of a leap day and of the end
    /// of a year, in UTC with milliseconds.
    #[test]
    fn mqttbridge_8_1_the_timestamp_is_rfc_3339_utc() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339(1_798_761_599_999), "2026-12-31T23:59:59.999Z");
    }

    /// Spec §8.1: a field the event does not have is not in the line; the
    /// top level's lines carry no `tenant`, a tenant's carry its name.
    #[test]
    fn mqttbridge_8_1_an_unset_field_is_left_out() {
        let top = event(Level::Info, "connect").tenant("").render(0);
        let v: serde_json::Value = serde_json::from_str(&top).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys.len(), 3, "{keys:?}");
        assert!(v.get("tenant").is_none() && v.get("latency_us").is_none());
        let tenant = event(Level::Info, "connect").tenant("a").render(0);
        let v: serde_json::Value = serde_json::from_str(&tenant).unwrap();
        assert_eq!(v["tenant"], "a");
    }

    /// Spec §8.1, §11: after `to_file` the lines are appended to the file
    /// (its directory made), one JSON object each; a second `to_file` on
    /// the same file keeps what is there.
    #[test]
    fn mqttbridge_11_the_lines_go_to_the_log_file_of_the_service() {
        let dir = std::env::temp_dir()
            .join(format!("zerodds-mqtt-log-{}", std::process::id()))
            .join("logs");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("mqtt-bridged.log");
        to_file(&path).unwrap();
        event(Level::Error, "start_failed")
            .field("kind", "test-file-1")
            .emit();
        to_file(&path).unwrap();
        event(Level::Error, "start_failed")
            .field("kind", "test-file-2")
            .emit();
        *FILE.lock().unwrap() = None;
        let text = std::fs::read_to_string(&path).unwrap();
        let kinds: Vec<String> = text
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .filter(|v| v["event"] == "start_failed")
            .filter_map(|v| v["kind"].as_str().map(str::to_string))
            .filter(|k| k.starts_with("test-file-"))
            .collect();
        assert_eq!(kinds, ["test-file-1", "test-file-2"], "{text}");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// Spec §11, zerodds-deployment-1.0 §4.1: the service's log file is
    /// `ZeroDDS\logs\mqtt-bridged.log` under `%PROGRAMDATA%`, under
    /// `C:\ProgramData` when the variable is not set or empty.
    #[test]
    fn mqttbridge_11_the_log_file_of_the_service_is_under_programdata() {
        let tail = Path::new("ZeroDDS").join("logs").join("mqtt-bridged.log");
        let p = service_log_path(Some(OsStr::new("D:\\Data")));
        assert!(
            p.starts_with("D:\\Data") && p.ends_with(&tail),
            "{}",
            p.display()
        );
        for unset in [None, Some(OsStr::new(""))] {
            let p = service_log_path(unset);
            assert!(
                p.starts_with(r"C:\ProgramData") && p.ends_with(&tail),
                "{}",
                p.display()
            );
        }
    }

    /// Spec §8.1: `log_level` is the threshold; a line below it is not
    /// written, a line at or above it is.
    #[test]
    fn mqttbridge_8_1_a_line_below_log_level_is_not_written() {
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
