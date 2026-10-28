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
//! | `dds.sec.log.plugin` | comma-separated sinks: `stderr`, `jsonl`, `syslog`, `builtin.DDS_LogTopic` |
//! | `org.zerodds.sec.log.level` | minimum level (default `Informational`) |
//! | `org.zerodds.sec.log.jsonl.path` | output file for the `jsonl` sink |
//! | `org.zerodds.sec.log.syslog.addr` | `host:port` for the `syslog` sink |
//! | `org.zerodds.sec.log.syslog.app` | app name (default `zerodds`) |
//! | `org.zerodds.sec.log.syslog.host` | hostname field (default `localhost`) |
//! | `org.zerodds.sec.log.builtin.DDS_LogTopic.logging_level` | level for the builtin sink when `org.zerodds.sec.log.level` is absent |
//! | `dds.sec.log.builtin.DDS_LogTopic.log_file` | `LogOptions.log_file`: local file the builtin sink appends to (§10.6.1) |
//! | `dds.sec.log.builtin.DDS_LogTopic.distribute` | `LogOptions.distribute`: publish on DDS; default `true` (§9.6.2.1.3) |
//! | `org.zerodds.sec.log.builtin.DDS_LogTopic.legacy_topic` | also publish the legacy `DDS:Security:LogTopic` (§10.6) |
//!
//! The `builtin.DDS_LogTopic` sink (DDS-Security 1.2 §10.6) publishes through
//! a DDS DataWriter, which this crate does not have; the caller supplies a
//! [`LogTopicPublisher`] to
//! [`logging_plugin_from_properties_with_publisher`] when the sink
//! distributes (§9.6.2.1.3). Without one, selecting the sink while
//! `distribute` is set is [`LogConfigError::MissingPublisher`] and not a
//! silent drop; with `distribute` off the sink is file-only and needs none.

// Returns a `Box<dyn LoggingPlugin>` built at runtime from properties — the concrete sink type is unknown until the `dds.sec.log.*` values are parsed.
// zerodds-lint: allow no_dyn_in_safe

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use core::fmt;

use crate::control::LogOptionsError;
use crate::log_topic::{LogTopicLoggingPlugin, LogTopicPublisher};
use crate::{
    FanOutLoggingPlugin, JsonLinesLoggingPlugin, StderrLoggingPlugin, SyslogLoggingPlugin,
};
use zerodds_security::logging::LogOptions;
use zerodds_security::logging::{LogLevel, LoggingPlugin};

/// `dds.sec.log.plugin` — comma-separated sink list (`stderr,jsonl,syslog`).
pub const PROP_LOG_PLUGIN: &str = "dds.sec.log.plugin";
/// `org.zerodds.sec.log.level` — minimum level. Default: `Informational`.
pub const PROP_LOG_LEVEL: &str = "org.zerodds.sec.log.level";
/// `org.zerodds.sec.log.jsonl.path` — output file for the `jsonl` sink.
pub const PROP_LOG_JSONL_PATH: &str = "org.zerodds.sec.log.jsonl.path";
/// `org.zerodds.sec.log.syslog.addr` — `host:port` target for the `syslog` sink.
pub const PROP_LOG_SYSLOG_ADDR: &str = "org.zerodds.sec.log.syslog.addr";
/// `org.zerodds.sec.log.syslog.app` — RFC-5424 app name (default `zerodds`).
pub const PROP_LOG_SYSLOG_APP: &str = "org.zerodds.sec.log.syslog.app";
/// `org.zerodds.sec.log.syslog.host` — RFC-5424 hostname (default `localhost`).
pub const PROP_LOG_SYSLOG_HOST: &str = "org.zerodds.sec.log.syslog.host";
/// `org.zerodds.sec.log.builtin.DDS_LogTopic.logging_level` — the builtin Logging
/// plugin's `log_level` (DDS-Security 1.2 §9.6.2.1.1 / §10.3.1 Table 50).
/// `org.zerodds.sec.log.level` wins when both are present.
pub const PROP_LOG_BUILTIN_LOGGING_LEVEL: &str =
    "org.zerodds.sec.log.builtin.DDS_LogTopic.logging_level";
/// `dds.sec.log.builtin.DDS_LogTopic.log_file` — the builtin Logging plugin's
/// `LogOptions.log_file` (DDS-Security 1.2 §10.6.1, §9.6.2.1.2): the local file
/// each logged message is appended to. Absent means no file.
pub const PROP_LOG_BUILTIN_LOG_FILE: &str = "dds.sec.log.builtin.DDS_LogTopic.log_file";
/// `dds.sec.log.builtin.DDS_LogTopic.distribute` — the builtin Logging plugin's
/// `LogOptions.distribute` (DDS-Security 1.2 §10.6.1, §9.6.2.1.3): whether the
/// sink publishes on DDS. Default `true`; the sink is only selected on purpose,
/// and a default of `false` would make it silently inert.
pub const PROP_LOG_BUILTIN_DISTRIBUTE: &str = "dds.sec.log.builtin.DDS_LogTopic.distribute";
/// `org.zerodds.sec.log.builtin.DDS_LogTopic.legacy_topic` — also publish the legacy
/// `DDS:Security:LogTopic` (type `BuiltinLoggingType`) on a second
/// DataWriter (DDS-Security 1.2 §10.6). Default: only the V2 topic.
pub const PROP_LOG_BUILTIN_LEGACY_TOPIC: &str =
    "org.zerodds.sec.log.builtin.DDS_LogTopic.legacy_topic";

/// The `dds.sec.log.plugin` entries that select the builtin
/// `DDS:Logging:DDS_LogTopic` sink. `builtin.DDS_LogTopic` is the spec-style
/// name; the other two are accepted spellings.
pub const LOG_TOPIC_SINKS: [&str; 3] = ["builtin.DDS_LogTopic", "dds_log_topic", "topic"];

/// Error materializing a logger from `dds.sec.log.*` properties.
#[derive(Debug)]
pub enum LogConfigError {
    /// Unknown sink name in `dds.sec.log.plugin`.
    UnknownSink(String),
    /// Unparseable `org.zerodds.sec.log.level`.
    UnknownLevel(String),
    /// A required property for a selected sink is missing.
    MissingProperty(&'static str),
    /// Malformed `org.zerodds.sec.log.syslog.addr`.
    BadAddress(String),
    /// A boolean-valued property that is neither `true` nor `false`.
    BadBoolean {
        /// The property whose value does not parse.
        property: &'static str,
        /// The value, as written.
        value: String,
    },
    /// The `builtin.DDS_LogTopic` sink was selected, but no
    /// [`LogTopicPublisher`] was supplied to publish through. A DDS Log Topic
    /// sink without a DataWriter cannot publish, and silently not publishing
    /// would lose every security event, so it is refused by name.
    MissingPublisher,
    /// I/O error opening a sink (e.g. the jsonl file or syslog socket).
    Io(std::io::Error),
    /// The builtin sink's [`LogOptions`] could not be applied (e.g. its
    /// `log_file` could not be opened).
    LogOptions(LogOptionsError),
}

impl fmt::Display for LogConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSink(s) => write!(f, "unknown log sink '{s}'"),
            Self::UnknownLevel(s) => write!(f, "unknown log level '{s}'"),
            Self::MissingProperty(p) => write!(f, "missing required property '{p}'"),
            Self::BadAddress(s) => write!(f, "malformed syslog address '{s}'"),
            Self::BadBoolean { property, value } => {
                write!(f, "'{property}' = '{value}' is not a boolean")
            }
            Self::MissingPublisher => write!(
                f,
                "the '{}' sink needs a DDS DataWriter, and none was supplied \
                 (DDS-Security 1.2 §10.6)",
                LOG_TOPIC_SINKS[0]
            ),
            Self::Io(e) => write!(f, "log sink i/o error: {e}"),
            Self::LogOptions(e) => write!(f, "builtin log options: {e}"),
        }
    }
}

impl From<LogOptionsError> for LogConfigError {
    fn from(e: LogOptionsError) -> Self {
        match e {
            LogOptionsError::Io(e) => Self::Io(e),
            LogOptionsError::MissingPublisher => Self::MissingPublisher,
            other => Self::LogOptions(other),
        }
    }
}

impl std::error::Error for LogConfigError {}

/// Parse a DDS-Security log-level name (case-insensitive; `info` aliases
/// `informational`).
///
/// Two spellings occur in the spec. The Logging-plugin SPI
/// (§9.6.2.1.1) names `FATAL_LEVEL`, `SEVERE_LEVEL`, `ERROR_LEVEL`,
/// `WARNING_LEVEL`, `NOTICE_LEVEL`, `INFO_LEVEL`, `DEBUG_LEVEL` and
/// `TRACE_LEVEL`; the `LogOptions.log_level` property
/// (`org.zerodds.sec.log.builtin.DDS_LogTopic.logging_level`) is written in that
/// upper-case form. The table here also takes the lower-case names.
/// `FATAL`, `SEVERE` and `TRACE` are [`LogLevel::Emergency`],
/// [`LogLevel::Critical`] and [`LogLevel::Trace`], the levels of
/// [`LogLevel::from_spec_level`].
pub fn parse_log_level(s: &str) -> Result<LogLevel, LogConfigError> {
    let lower = s.trim().to_ascii_lowercase();
    let name = lower.strip_suffix("_level").unwrap_or(&lower);
    Ok(match name {
        "emergency" | "fatal" => LogLevel::Emergency,
        "alert" => LogLevel::Alert,
        "critical" | "severe" => LogLevel::Critical,
        "error" => LogLevel::Error,
        "warning" => LogLevel::Warning,
        "notice" => LogLevel::Notice,
        "informational" | "info" => LogLevel::Informational,
        "debug" => LogLevel::Debug,
        "trace" => LogLevel::Trace,
        _ => return Err(LogConfigError::UnknownLevel(s.to_string())),
    })
}

fn lookup<'a>(props: &'a [(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    props.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn parse_bool(property: &'static str, value: &str) -> Result<bool, LogConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(LogConfigError::BadBoolean {
            property,
            value: value.to_string(),
        }),
    }
}

/// `true` when `dds.sec.log.plugin` names the builtin
/// `DDS:Logging:DDS_LogTopic` sink (any of [`LOG_TOPIC_SINKS`]).
///
/// The participant uses this to decide whether it has to build the
/// BuiltinLoggingTopic DataWriter(s) before materializing the logger.
#[must_use]
pub fn log_topic_requested(props: &[(&str, &str)]) -> bool {
    lookup(props, PROP_LOG_PLUGIN).is_some_and(|sinks| {
        sinks
            .split(',')
            .map(str::trim)
            .any(|s| LOG_TOPIC_SINKS.contains(&s))
    })
}

/// The `org.zerodds.sec.log.builtin.DDS_LogTopic.legacy_topic` flag — whether the
/// BuiltinLoggingTopic sink also publishes the legacy
/// `DDS:Security:LogTopic` (DDS-Security 1.2 §10.6). Absent is `false`.
///
/// # Errors
/// [`LogConfigError::BadBoolean`] for a value that is not a boolean.
pub fn log_topic_legacy_requested(props: &[(&str, &str)]) -> Result<bool, LogConfigError> {
    match lookup(props, PROP_LOG_BUILTIN_LEGACY_TOPIC) {
        Some(v) => parse_bool(PROP_LOG_BUILTIN_LEGACY_TOPIC, v),
        None => Ok(false),
    }
}

/// The `dds.sec.log.builtin.DDS_LogTopic.distribute` flag — whether the
/// BuiltinLoggingTopic sink publishes on DDS (DDS-Security 1.2 §9.6.2.1.3).
/// Absent is `true`: the sink is only selected on purpose, and a `false`
/// default would make it publish nothing while appearing configured.
///
/// # Errors
/// [`LogConfigError::BadBoolean`] for a value that is not a boolean.
pub fn log_topic_distribute_requested(props: &[(&str, &str)]) -> Result<bool, LogConfigError> {
    match lookup(props, PROP_LOG_BUILTIN_DISTRIBUTE) {
        Some(v) => parse_bool(PROP_LOG_BUILTIN_DISTRIBUTE, v),
        None => Ok(true),
    }
}

/// Build a [`LoggingPlugin`] from `dds.sec.log.*` properties, fanning out to
/// every named sink. Returns `Ok(None)` if `dds.sec.log.plugin` is absent or
/// names no sinks.
///
/// The builtin `DDS:Logging:DDS_LogTopic` sink needs a DDS DataWriter when it
/// distributes, which [`logging_plugin_from_properties_with_publisher`]
/// supplies. Selecting it with `distribute` set (the default) here returns
/// [`LogConfigError::MissingPublisher`] rather than silently leaving the
/// events unpublishable; with `distribute` off the sink writes only a
/// `log_file` and needs no writer.
///
/// # Errors
/// A misconfigured sink ([`LogConfigError`]); see the individual variants.
pub fn logging_plugin_from_properties(
    props: &[(&str, &str)],
) -> Result<Option<Box<dyn LoggingPlugin>>, LogConfigError> {
    logging_plugin_from_properties_with_publisher(props, None)
}

/// [`logging_plugin_from_properties`], with the [`LogTopicPublisher`] that
/// backs the builtin `DDS:Logging:DDS_LogTopic` sink.
///
/// `topic_publisher` is `None` unless the caller selected that sink; it is
/// required then and every other sink ignores it.
///
/// # Errors
/// [`LogConfigError::MissingPublisher`] when the builtin sink is selected and
/// no publisher was supplied; otherwise as [`logging_plugin_from_properties`].
pub fn logging_plugin_from_properties_with_publisher(
    props: &[(&str, &str)],
    topic_publisher: Option<Arc<dyn LogTopicPublisher>>,
) -> Result<Option<Box<dyn LoggingPlugin>>, LogConfigError> {
    let Some(sinks) = lookup(props, PROP_LOG_PLUGIN) else {
        return Ok(None);
    };
    let level = match lookup(props, PROP_LOG_LEVEL) {
        Some(s) => parse_log_level(s)?,
        None => match lookup(props, PROP_LOG_BUILTIN_LOGGING_LEVEL) {
            Some(s) => parse_log_level(s)?,
            None => crate::DEFAULT_LOG_LEVEL,
        },
    };
    let legacy = log_topic_legacy_requested(props)?;

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
            other if LOG_TOPIC_SINKS.contains(&other) => {
                let distribute = log_topic_distribute_requested(props)?;
                // §10.6.1: a writer is needed only when the events are to be
                // distributed; with `distribute` off the sink is file-only.
                let publisher = if distribute {
                    Some(
                        topic_publisher
                            .clone()
                            .ok_or(LogConfigError::MissingPublisher)?,
                    )
                } else {
                    None
                };
                let log_file = lookup(props, PROP_LOG_BUILTIN_LOG_FILE).map(str::to_string);
                let options = LogOptions {
                    log_level: level,
                    log_file,
                    distribute,
                };
                let sink = LogTopicLoggingPlugin::with_options(publisher, options, legacy)?;
                sink.enable_logging()?;
                fanout = fanout.with(sink);
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
            logging_plugin_from_properties(&[("org.zerodds.sec.log.level", "Warning")])
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

    #[test]
    fn spec_level_spellings_map_to_levels() {
        // §9.6.2.1.1 writes the LogOptions levels with a `_LEVEL` suffix.
        assert!(matches!(
            parse_log_level("DEBUG_LEVEL"),
            Ok(LogLevel::Debug)
        ));
        assert!(matches!(
            parse_log_level("INFO_LEVEL"),
            Ok(LogLevel::Informational)
        ));
        assert!(matches!(
            parse_log_level("FATAL_LEVEL"),
            Ok(LogLevel::Emergency)
        ));
        assert!(matches!(
            parse_log_level("TRACE_LEVEL"),
            Ok(LogLevel::Trace)
        ));
    }

    #[test]
    fn log_topic_requested_follows_the_plugin_list() {
        assert!(!log_topic_requested(&[]));
        assert!(!log_topic_requested(&[(PROP_LOG_PLUGIN, "stderr,jsonl")]));
        assert!(log_topic_requested(&[(PROP_LOG_PLUGIN, "stderr,topic")]));
        assert!(log_topic_requested(&[(
            PROP_LOG_PLUGIN,
            "builtin.DDS_LogTopic"
        )]));
    }

    #[test]
    fn builtin_topic_sink_without_a_publisher_is_refused() {
        match logging_plugin_from_properties(&[(PROP_LOG_PLUGIN, "builtin.DDS_LogTopic")]) {
            Err(LogConfigError::MissingPublisher) => {}
            _ => panic!("expected MissingPublisher"),
        }
    }

    #[derive(Default)]
    struct RecordingPublisher {
        v2: std::sync::atomic::AtomicUsize,
        legacy: std::sync::atomic::AtomicUsize,
    }

    impl LogTopicPublisher for RecordingPublisher {
        fn publish_v2(
            &self,
            _event: &crate::BuiltinLoggingTypeV2,
        ) -> Result<(), crate::LogTopicPublishError> {
            self.v2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        fn publish_legacy(
            &self,
            _event: &crate::BuiltinLoggingType,
        ) -> Result<(), crate::LogTopicPublishError> {
            self.legacy
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn builtin_topic_sink_publishes_when_a_publisher_is_supplied() {
        let recorder = Arc::new(RecordingPublisher::default());
        let plugin = logging_plugin_from_properties_with_publisher(
            &[
                (PROP_LOG_PLUGIN, "builtin.DDS_LogTopic"),
                (PROP_LOG_BUILTIN_LOGGING_LEVEL, "WARNING_LEVEL"),
                (PROP_LOG_BUILTIN_LEGACY_TOPIC, "true"),
            ],
            Some(recorder.clone()),
        )
        .unwrap()
        .expect("fanout built");
        plugin.log(LogLevel::Error, [0; 16], "auth", "logged");
        plugin.log(LogLevel::Informational, [0; 16], "auth", "below level");
        assert_eq!(recorder.v2.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(recorder.legacy.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_bad_legacy_boolean_is_refused_by_name() {
        let recorder = Arc::new(RecordingPublisher::default());
        match logging_plugin_from_properties_with_publisher(
            &[
                (PROP_LOG_PLUGIN, "builtin.DDS_LogTopic"),
                (PROP_LOG_BUILTIN_LEGACY_TOPIC, "sometimes"),
            ],
            Some(recorder),
        ) {
            Err(LogConfigError::BadBoolean { property, value }) => {
                assert_eq!(property, PROP_LOG_BUILTIN_LEGACY_TOPIC);
                assert_eq!(value, "sometimes");
            }
            _ => panic!("expected BadBoolean"),
        }
    }

    /// The property names the builtin sink reads are the ones the module
    /// documents; renaming a constant must not quietly rename the key.
    #[test]
    fn builtin_option_property_names_are_stable() {
        assert_eq!(
            PROP_LOG_BUILTIN_LOGGING_LEVEL,
            "org.zerodds.sec.log.builtin.DDS_LogTopic.logging_level"
        );
        assert_eq!(
            PROP_LOG_BUILTIN_LOG_FILE,
            "dds.sec.log.builtin.DDS_LogTopic.log_file"
        );
        assert_eq!(
            PROP_LOG_BUILTIN_DISTRIBUTE,
            "dds.sec.log.builtin.DDS_LogTopic.distribute"
        );
    }

    /// §10.6.1 — the builtin sink appends to the file named by
    /// `dds.sec.log.builtin.DDS_LogTopic.log_file` and does not publish when
    /// `distribute` is false.
    #[test]
    fn builtin_file_and_distribute_come_from_properties() {
        let dir = std::env::temp_dir().join(format!("zerodds-log-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("builtin.log");
        let path_s = path.to_str().unwrap();
        let recorder = Arc::new(RecordingPublisher::default());
        let plugin = logging_plugin_from_properties_with_publisher(
            &[
                (PROP_LOG_PLUGIN, "builtin.DDS_LogTopic"),
                (PROP_LOG_BUILTIN_LOGGING_LEVEL, "WARNING_LEVEL"),
                (PROP_LOG_BUILTIN_LOG_FILE, path_s),
                (PROP_LOG_BUILTIN_DISTRIBUTE, "false"),
            ],
            Some(recorder.clone()),
        )
        .unwrap()
        .expect("builtin sink built");
        plugin.log(LogLevel::Error, [0x11; 16], "auth.handshake.failed", "bad");
        plugin.log(LogLevel::Informational, [0x22; 16], "auth.debug", "below");
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 1, "only Warning-and-above: {contents}");
        assert!(
            lines[0].contains("auth.handshake.failed"),
            "got: {contents}"
        );
        assert_eq!(recorder.v2.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(recorder.legacy.load(std::sync::atomic::Ordering::SeqCst), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §10.6.1 — with `distribute` off the sink creates no writer and so needs
    /// no publisher; a file-only logger is built.
    #[test]
    fn distribute_off_needs_no_publisher() {
        let dir = std::env::temp_dir().join(format!("zerodds-log-file-off-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("builtin-off.log");
        let path_s = path.to_str().unwrap();
        let plugin = logging_plugin_from_properties(&[
            (PROP_LOG_PLUGIN, "builtin.DDS_LogTopic"),
            (PROP_LOG_BUILTIN_LOG_FILE, path_s),
            (PROP_LOG_BUILTIN_DISTRIBUTE, "false"),
        ])
        .unwrap()
        .expect("file-only builtin sink built");
        plugin.log(LogLevel::Emergency, [0; 16], "auth", "x");
        assert!(std::fs::read_to_string(&path).unwrap().contains("auth"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A malformed `distribute` value is refused by name, not read as `false`.
    #[test]
    fn a_bad_distribute_boolean_is_refused_by_name() {
        match logging_plugin_from_properties(&[
            (PROP_LOG_PLUGIN, "builtin.DDS_LogTopic"),
            (PROP_LOG_BUILTIN_DISTRIBUTE, "sometimes"),
        ]) {
            Err(LogConfigError::BadBoolean { property, value }) => {
                assert_eq!(property, PROP_LOG_BUILTIN_DISTRIBUTE);
                assert_eq!(value, "sometimes");
            }
            _ => panic!("expected BadBoolean"),
        }
    }
}
