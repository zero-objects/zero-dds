// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-logging`. Safety classification: **SAFE** (reiner I/O-Wrapper; keine Secrets werden gepuffert ausserhalb der Log-Zeile selbst).
//!
//! Produktions-taugliche Logging-Backends fuer DDS-Security 1.1/1.2 §8.6
//! (`LoggingPlugin`-SPI aus `zerodds-security`).
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Konsumiert von end-user-Builds + DCPS-Runtime
//! (Feature `security`).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`StderrLoggingPlugin`] — structured log lines an `stderr`.
//! - [`JsonLinesLoggingPlugin`] — `application/x-ndjson` in eine Datei.
//! - [`SyslogLoggingPlugin`] — RFC-5424-UDP-Backend (Facility `LOCAL0`).
//! - [`FanOutLoggingPlugin`] — fan-out an mehrere Backends.
//!
//! # Was dieser Crate liefert
//!
//! 1. [`StderrLoggingPlugin`] — schreibt structured log lines an
//!    `stderr`. Default fuer Development + Container-Deployments mit
//!    stdout/stderr-Collector (Loki, Vector, Fluentd).
//! 2. [`JsonLinesLoggingPlugin`] — schreibt JSON-Lines
//!    (`application/x-ndjson`) in eine Datei. Jede Zeile = ein Event.
//!    Ein zweiter Prozess (z.B. auditd, filebeat) rotiert die Datei.
//! 3. [`FanOutLoggingPlugin`] — routet jedes Event an **mehrere**
//!    Backends (z.B. stderr + JSON-File gleichzeitig).
//!
//! Alle Backends filtern Events nach `LogLevel`; das Default-Level ist
//! `Warning` — niedriger (Informational, Debug) wird still verworfen.
//!
//! ## Nicht-Ziele
//!
//! - Syslog-TCP (RFC 5425) und Syslog-TLS — die meisten Syslog-
//!   Deployments laufen im vertrauten Segment; Re-Add bei Bedarf.
//! - Structured Telemetry (OpenTelemetry / OTLP) — abgedeckt von
//!   `zerodds-observability-otlp` (Layer 4.6).
//! - Log-Rotation im Plugin selbst — Aufgabe des Betriebssystems /
//!   `logrotate`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Drei Plugin-Impls fuer Box-Polymorphie — SPI-bedingt.
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
