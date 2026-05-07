# `zerodds-security-logging`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-logging/badge.svg)](https://docs.rs/zerodds-security-logging)

Security-Logging-Backends fuer den
[ZeroDDS](https://zerodds.org)-Stack: `LoggingPlugin`-Implementationen
fuer stderr, JSON-Lines, Syslog und FanOut. Safety classification:
**SAFE**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-Security 1.1 | §8.6 (LoggingPlugin) |
| RFC 5424 | Syslog (UDP) |

## Was ist drin

- **`StderrLoggingPlugin`** — Default fuer Container-Deployments mit stdout/stderr-Collector (Loki, Vector, Fluentd).
- **`JsonLinesLoggingPlugin`** — `application/x-ndjson` in eine Datei. Rotation via `logrotate`.
- **`SyslogLoggingPlugin`** — UDP an Syslog-Collector (Facility `LOCAL0`).
- **`FanOutLoggingPlugin`** — fan-out an mehrere Backends parallel.

Alle Backends filtern Events nach `LogLevel`; Default-Level `Warning`.

## Schichten-Position

Layer 4. Konsumiert `zerodds-security` (LoggingPlugin-Trait + LogLevel + SecurityError).

## Quickstart

```rust,no_run
use zerodds_security_logging::{StderrLoggingPlugin, FanOutLoggingPlugin, JsonLinesLoggingPlugin};
use zerodds_security::{LoggingPlugin, LogLevel};

let stderr = StderrLoggingPlugin::new(LogLevel::Warning);
let json = JsonLinesLoggingPlugin::open("/var/log/zerodds/security.jsonl", LogLevel::Informational)
    .expect("open");
let fanout: Box<dyn LoggingPlugin> = Box::new(FanOutLoggingPlugin::new(vec![
    Box::new(stderr),
    Box::new(json),
]));
```

## Nicht-Ziele

- Syslog-TCP (RFC 5425) und Syslog-TLS — vertrautes Segment vorausgesetzt.
- OpenTelemetry/OTLP — abgedeckt von [`zerodds-observability-otlp`](../observability-otlp).
- Log-Rotation im Plugin — Aufgabe von `logrotate`/`journald`.

## Stabilitaet

`1.0.0-rc.1`. Public-API + JSON-Lines-Format + RFC-5424-Encoding RC1-stabil.

## Tests

```bash
cargo test -p zerodds-security-logging
```

16 Tests grün.

## Lizenz

Apache-2.0.
