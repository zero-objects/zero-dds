// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-logging`. Safety classification: **SAFE** (a pure I/O wrapper; no secrets are buffered outside the log line itself).
//!
//! Production-grade logging backends for DDS-Security 1.2 §9.6 and §10.6
//! (the `LoggingPlugin` SPI from `zerodds-security`).
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumed by end-user builds + the DCPS runtime
//! (feature `security`).
//!
//! ## Public API
//!
//! - [`StderrLoggingPlugin`] — structured log lines to `stderr`.
//! - [`JsonLinesLoggingPlugin`] — `application/x-ndjson` to a file.
//! - [`SyslogLoggingPlugin`] — RFC-5424 UDP backend (facility `LOCAL0`).
//! - [`LogTopicLoggingPlugin`] — the builtin `DDS:Logging:DDS_LogTopic`
//!   (DDS-Security 1.2 §10.6): publishes `BuiltinLoggingTypeV2` on
//!   `DDS:Security:LogTopicV2` through a [`LogTopicPublisher`].
//! - [`FanOutLoggingPlugin`] — fan-out to multiple backends.
//!
//! # What this crate provides
//!
//! 1. [`StderrLoggingPlugin`] — writes structured log lines to
//!    `stderr`. Default for development + container deployments with a
//!    stdout/stderr collector (Loki, Vector, Fluentd).
//! 2. [`JsonLinesLoggingPlugin`] — writes JSON lines
//!    (`application/x-ndjson`) to a file. Each line = one event.
//!    A second process (e.g. auditd, filebeat) rotates the file.
//! 3. [`FanOutLoggingPlugin`] — routes each event to **multiple**
//!    backends (e.g. stderr + JSON file simultaneously).
//!
//! All backends filter events by `LogLevel`; the default level is
//! `Warning` — lower (Informational, Debug) is silently discarded.
//!
//! ## Non-goals
//!
//! - Syslog TCP (RFC 5425) and syslog TLS — most syslog
//!   deployments run in a trusted segment; re-add on demand.
//! - Structured telemetry (OpenTelemetry / OTLP) — covered by
//!   `zerodds-observability-otlp` (layer 4.6).
//! - Log rotation in the plugin itself — the job of the operating system /
//!   `logrotate`.

// Needs `std`: every backend writes to a file, to stderr or to a socket.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Three plugin impls for box polymorphism — SPI-driven.
// zerodds-lint: allow no_dyn_in_safe

extern crate alloc;

#[macro_use]
mod control;
mod fanout;
#[cfg(feature = "std")]
mod from_properties;
mod jsonl;
#[cfg(feature = "std")]
mod log_topic;
mod stderr_sink;
mod syslog;

pub use control::{LogControl, LogOptionsError};
pub use fanout::FanOutLoggingPlugin;
pub use zerodds_security::logging::{LogOptions, LoggerListener};

/// The minimum level a sink logs when none is given: by
/// [`StderrLoggingPlugin::new`] and, without `org.zerodds.sec.log.level`, by the
/// `dds.sec.log.*` properties. DDS-Security 1.2 §9.6.2.1.1 names the levels
/// and leaves the default to the implementation.
pub const DEFAULT_LOG_LEVEL: zerodds_security::logging::LogLevel =
    zerodds_security::logging::LogLevel::Informational;
#[cfg(feature = "std")]
pub use from_properties::{
    LOG_TOPIC_SINKS, LogConfigError, PROP_LOG_BUILTIN_DISTRIBUTE, PROP_LOG_BUILTIN_LEGACY_TOPIC,
    PROP_LOG_BUILTIN_LOG_FILE, PROP_LOG_BUILTIN_LOGGING_LEVEL, PROP_LOG_JSONL_PATH, PROP_LOG_LEVEL,
    PROP_LOG_PLUGIN, PROP_LOG_SYSLOG_ADDR, PROP_LOG_SYSLOG_APP, PROP_LOG_SYSLOG_HOST,
    log_topic_distribute_requested, log_topic_legacy_requested, log_topic_requested,
    logging_plugin_from_properties, logging_plugin_from_properties_with_publisher, parse_log_level,
};
pub use jsonl::JsonLinesLoggingPlugin;
#[cfg(feature = "std")]
pub use log_topic::{
    BuiltinLoggingType, BuiltinLoggingTypeV2, FACILITY_SEC_AUTH, LegacyTime, LogTopicLoggingPlugin,
    LogTopicPublishError, LogTopicPublisher, LogTopicWriterSlot, LoggingLevel, NameValuePair,
    NameValuePairSeq, Time,
};
pub use stderr_sink::StderrLoggingPlugin;
pub use syslog::SyslogLoggingPlugin;
