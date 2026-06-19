# `zerodds-security-logging`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-logging/badge.svg)](https://docs.rs/zerodds-security-logging)

Security logging backends for the
[ZeroDDS](https://zerodds.org) stack: `LoggingPlugin` implementations
for stderr, JSON lines, syslog and FanOut. Safety classification:
**SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.1 | §8.6 (LoggingPlugin) |
| RFC 5424 | Syslog (UDP) |

## What's inside

- **`StderrLoggingPlugin`** — default for container deployments with a stdout/stderr collector (Loki, Vector, Fluentd).
- **`JsonLinesLoggingPlugin`** — `application/x-ndjson` to a file. Rotation via `logrotate`.
- **`SyslogLoggingPlugin`** — UDP to a syslog collector (facility `LOCAL0`).
- **`FanOutLoggingPlugin`** — fan-out to multiple backends in parallel.

All backends filter events by `LogLevel`; default level `Warning`.

## Layer position

Layer 4. Consumes `zerodds-security` (LoggingPlugin trait + LogLevel + SecurityError).

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

## Non-goals

- Syslog TCP (RFC 5425) and syslog TLS — a trusted segment is assumed.
- OpenTelemetry/OTLP — covered by [`zerodds-observability-otlp`](../observability-otlp).
- Log rotation in the plugin — the job of `logrotate`/`journald`.

## Stability

`1.0.0-rc.1`. Public API + JSON-lines format + RFC-5424 encoding RC1-stable.

## Tests

```bash
cargo test -p zerodds-security-logging
```

16 tests green.

## License

Apache-2.0.
