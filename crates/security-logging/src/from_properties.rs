// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Build a security-event [`LoggingPlugin`] from DDS-Security `dds.sec.log.*`
//! properties.
//!
//! This is the spec-style wireup: instead of constructing a logger object and
//! handing it to the runtime directly, the participant carries `dds.sec.log.*`
//! name/value properties on its QoS, and the runtime materializes the logger
//! from them. Multiple sinks fan out automatically.
//!
//! | Property | Meaning |
//! |---|---|
//! | `dds.sec.log.plugin` | comma-separated sinks: `stderr`, `jsonl`, `syslog` |
//! | `dds.sec.log.level` | minimum level (default `Informational`) |
//! | `dds.sec.log.jsonl.path` | output file for the `jsonl` sink |
//! | `dds.sec.log.syslog.addr` | `host:port` for the `syslog` sink |
//! | `dds.sec.log.syslog.app` | app name (default `zerodds`) |
//! | `dds.sec.log.syslog.host` | hostname field (default `localhost`) |

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use core::fmt;

use crate::{
    FanOutLoggingPlugin, JsonLinesLoggingPlugin, StderrLoggingPlugin, SyslogLoggingPlugin,
};
use zerodds_security::logging::{LogLevel, LoggingPlugin};

/// `dds.sec.log.plugin` — comma-separated sink list (`stderr,jsonl,syslog`).
pub const PROP_LOG_PLUGIN: &str = "dds.sec.log.plugin";
/// `dds.sec.log.level` — minimum level. Default: `Informational`.
pub const PROP_LOG_LEVEL: &str = "dds.sec.log.level";
/// `dds.sec.log.jsonl.path` — output file for the `jsonl` sink.
pub const PROP_LOG_JSONL_PATH: &str = "dds.sec.log.jsonl.path";
/// `dds.sec.log.syslog.addr` — `host:port` target for the `syslog` sink.
pub const PROP_LOG_SYSLOG_ADDR: &str = "dds.sec.log.syslog.addr";
/// `dds.sec.log.syslog.app` — RFC-5424 app name (default `zerodds`).
pub const PROP_LOG_SYSLOG_APP: &str = "dds.sec.log.syslog.app";
/// `dds.sec.log.syslog.host` — RFC-5424 hostname (default `localhost`).
pub const PROP_LOG_SYSLOG_HOST: &str = "dds.sec.log.syslog.host";

/// Error materializing a logger from `dds.sec.log.*` properties.
#[derive(Debug)]
pub enum LogConfigError {
    /// Unknown sink name in `dds.sec.log.plugin`.
    UnknownSink(String),
    /// Unparseable `dds.sec.log.level`.
    UnknownLevel(String),
    /// A required property for a selected sink is missing.
    MissingProperty(&'static str),
    /// Malformed `dds.sec.log.syslog.addr`.
    BadAddress(String),
    /// I/O error opening a sink (e.g. the jsonl file or syslog socket).
    Io(std::io::Error),
}

impl fmt::Display for LogConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSink(s) => write!(f, "unknown log sink '{s}'"),
            Self::UnknownLevel(s) => write!(f, "unknown log level '{s}'"),
            Self::MissingProperty(p) => write!(f, "missing required property '{p}'"),
            Self::BadAddress(s) => write!(f, "malformed syslog address '{s}'"),
            Self::Io(e) => write!(f, "log sink i/o error: {e}"),
        }
    }
}

impl std::error::Error for LogConfigError {}

/// Parse a DDS-Security log-level name (case-insensitive; `info` aliases
/// `informational`).
pub fn parse_log_level(s: &str) -> Result<LogLevel, LogConfigError> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "emergency" => LogLevel::Emergency,
        "alert" => LogLevel::Alert,
        "critical" => LogLevel::Critical,
        "error" => LogLevel::Error,
        "warning" => LogLevel::Warning,
        "notice" => LogLevel::Notice,
        "informational" | "info" => LogLevel::Informational,
        "debug" => LogLevel::Debug,
        _ => return Err(LogConfigError::UnknownLevel(s.to_string())),
    })
}

fn lookup<'a>(props: &'a [(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    props.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Build a [`LoggingPlugin`] from `dds.sec.log.*` properties, fanning out to
/// every named sink. Returns `Ok(None)` if `dds.sec.log.plugin` is absent or
/// names no sinks.
pub fn logging_plugin_from_properties(
    props: &[(&str, &str)],
) -> Result<Option<Box<dyn LoggingPlugin>>, LogConfigError> {
    let Some(sinks) = lookup(props, PROP_LOG_PLUGIN) else {
        return Ok(None);
    };
    let level = match lookup(props, PROP_LOG_LEVEL) {
        Some(s) => parse_log_level(s)?,
        None => LogLevel::Informational,
    };

    let mut fanout = FanOutLoggingPlugin::new();
    let mut configured = 0usize;
    for sink in sinks.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match sink {
            "stderr" => {
                fanout = fanout.with(StderrLoggingPlugin::with_level(level));
            }
            "jsonl" => {
                let path = lookup(props, PROP_LOG_JSONL_PATH)
                    .ok_or(LogConfigError::MissingProperty(PROP_LOG_JSONL_PATH))?;
                let jsonl =
                    JsonLinesLoggingPlugin::open(path, level).map_err(LogConfigError::Io)?;
                fanout = fanout.with(jsonl);
            }
            "syslog" => {
                let addr = lookup(props, PROP_LOG_SYSLOG_ADDR)
                    .ok_or(LogConfigError::MissingProperty(PROP_LOG_SYSLOG_ADDR))?;
                let target = addr
                    .parse()
                    .map_err(|_| LogConfigError::BadAddress(addr.to_string()))?;
                let app = lookup(props, PROP_LOG_SYSLOG_APP).unwrap_or("zerodds");
                let host = lookup(props, PROP_LOG_SYSLOG_HOST).unwrap_or("localhost");
                let syslog = SyslogLoggingPlugin::connect(target, app, host, level)
                    .map_err(LogConfigError::Io)?;
                fanout = fanout.with(syslog);
            }
            other => return Err(LogConfigError::UnknownSink(other.to_string())),
        }
        configured += 1;
    }

    if configured == 0 {
        return Ok(None);
    }
    Ok(Some(Box::new(fanout)))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn absent_plugin_property_yields_none() {
        assert!(logging_plugin_from_properties(&[]).unwrap().is_none());
        assert!(
            logging_plugin_from_properties(&[("dds.sec.log.level", "Warning")])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn stderr_sink_builds() {
        let p = logging_plugin_from_properties(&[
            (PROP_LOG_PLUGIN, "stderr"),
            (PROP_LOG_LEVEL, "Warning"),
        ])
        .unwrap();
        assert!(p.is_some());
        assert_eq!(p.unwrap().plugin_class_id(), "DDS:Logging:fanout");
    }

    #[test]
    fn jsonl_without_path_errors() {
        // `Box<dyn LoggingPlugin>` isn't Debug, so match instead of unwrap_err.
        match logging_plugin_from_properties(&[(PROP_LOG_PLUGIN, "jsonl")]) {
            Err(LogConfigError::MissingProperty(p)) => assert_eq!(p, PROP_LOG_JSONL_PATH),
            _ => panic!("expected MissingProperty error"),
        }
    }

    #[test]
    fn jsonl_sink_writes_to_file() {
        let dir = std::env::temp_dir().join(format!("zerodds-log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("audit.ndjson");
        let path_s = path.to_str().unwrap();
        let plugin = logging_plugin_from_properties(&[
            (PROP_LOG_PLUGIN, "stderr,jsonl"),
            (PROP_LOG_LEVEL, "Notice"),
            (PROP_LOG_JSONL_PATH, path_s),
        ])
        .unwrap()
        .expect("fanout built");
        plugin.log(LogLevel::Error, [0u8; 16], "access_control", "denied");
        // Flush is implicit per-line; the file must now contain our event.
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("access_control"), "got: {contents}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_sink_errors() {
        match logging_plugin_from_properties(&[(PROP_LOG_PLUGIN, "carrier-pigeon")]) {
            Err(LogConfigError::UnknownSink(s)) => assert_eq!(s, "carrier-pigeon"),
            _ => panic!("expected UnknownSink error"),
        }
    }

    #[test]
    fn level_parsing() {
        assert!(matches!(parse_log_level("Warning"), Ok(LogLevel::Warning)));
        assert!(matches!(
            parse_log_level("info"),
            Ok(LogLevel::Informational)
        ));
        assert!(parse_log_level("nonsense").is_err());
    }
}
