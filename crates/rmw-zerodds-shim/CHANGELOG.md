# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`rmw-zerodds-shim`** crate as a Layer-7 profile.

### Spec-Referenzen
Siehe `README.md` + `docs/spec-coverage/<spec>.md`.

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments.

### Implementation
ROS 2 rmw_zerodds shim — C-FFI wrapper around zerodds-ros2-rmw + zerodds-c-api

### Architektur
- Layer: 7 (Profiles)

### Stabilitaet
All `pub` items are RC1-stable; breaking changes require a major bump.

### Added — CLI subcommands

- `zerodds-ros2-shim` CLI with `catalog` (topic inventory), `metrics`
  (Prometheus snapshot) and `selftest` subcommands.
- `zerodds-monitor` + `zerodds-observability-otlp` integration for
  diagnostics output (JSON lines + OTLP spans) — the CLI is exempt from
  the SIGTERM watcher (§9.2 applies only to daemons).
