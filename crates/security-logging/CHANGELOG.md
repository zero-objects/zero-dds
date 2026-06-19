# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-logging` crate.

### Spec references

- **OMG DDS-Security 1.1** §8.6 (LoggingPlugin).
- **RFC 5424** Syslog (UDP, facility `LOCAL0`).

### Public API

- `StderrLoggingPlugin::new(min_level)`.
- `JsonLinesLoggingPlugin::open(path, min_level)`.
- `SyslogLoggingPlugin::connect(target, app_name, hostname, min_level)`.
- `FanOutLoggingPlugin::new(backends)`.

### Implementation

Each backend implements `zerodds_security::LoggingPlugin` and filters events by `LogLevel` (default `Warning`). RFC-5424 format of the syslog variant: `<PRI>1 - HOST APP - CAT - participant=<hex16> MSG`. Multi-byte fields are CR/LF-escaped so the collector does not tear the line apart.

`FanOutLoggingPlugin` enables composition: `Box<dyn LoggingPlugin>` are assembled as a Vec — each event goes to all.

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (LoggingPlugin trait + LogLevel + SecurityError).
- **Dependents (out):** end-user builds, `dcps` (feature `security`).
- **Feature flags:** `std` (default).

### Stability

Public API + JSON-lines format + RFC-5424 encoding RC1-stable.
