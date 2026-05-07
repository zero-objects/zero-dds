# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security-logging`-Crate.

### Spec-Referenzen

- **OMG DDS-Security 1.1** §8.6 (LoggingPlugin).
- **RFC 5424** Syslog (UDP, Facility `LOCAL0`).

### Public-API

- `StderrLoggingPlugin::new(min_level)`.
- `JsonLinesLoggingPlugin::open(path, min_level)`.
- `SyslogLoggingPlugin::connect(target, app_name, hostname, min_level)`.
- `FanOutLoggingPlugin::new(backends)`.

### Implementierung

Jedes Backend implementiert `zerodds_security::LoggingPlugin` und filtert Events nach `LogLevel` (Default `Warning`). RFC-5424-Format der Syslog-Variante: `<PRI>1 - HOST APP - CAT - participant=<hex16> MSG`. Multi-Byte-Felder werden CR/LF-escaped, damit der Collector die Zeile nicht zerreisst.

`FanOutLoggingPlugin` erlaubt Composition: `Box<dyn LoggingPlugin>` wird als Vec zusammengesteckt — jedes Event geht an alle.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (LoggingPlugin-Trait + LogLevel + SecurityError).
- **Dependents (out):** end-user-Builds, `dcps` (Feature `security`).
- **Feature-Flags:** `std` (default).

### Stabilitaet

Public-API + JSON-Lines-Format + RFC-5424-Encoding RC1-stabil.
