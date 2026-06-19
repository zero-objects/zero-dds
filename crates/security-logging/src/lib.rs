// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-logging`. Safety classification: **SAFE** (a pure I/O wrapper; no secrets are buffered outside the log line itself).
//!
//! Production-grade logging backends for DDS-Security 1.1/1.2 §8.6
//! (the `LoggingPlugin` SPI from `zerodds-security`).
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumed by end-user builds + the DCPS runtime
//! (feature `security`).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`StderrLoggingPlugin`] — structured log lines to `stderr`.
//! - [`JsonLinesLoggingPlugin`] — `application/x-ndjson` to a file.
//! - [`SyslogLoggingPlugin`] — RFC-5424 UDP backend (facility `LOCAL0`).
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

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Three plugin impls for box polymorphism — SPI-driven.
// zerodds-lint: allow no_dyn_in_safe

extern crate alloc;

mod fanout;
mod jsonl;
mod stderr_sink;
mod syslog;

pub use fanout::FanOutLoggingPlugin;
pub use jsonl::JsonLinesLoggingPlugin;
pub use stderr_sink::StderrLoggingPlugin;
pub use syslog::SyslogLoggingPlugin;
