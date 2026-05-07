# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung des Crates **`rmw-zerodds-shim`** als Layer-7-Profile.

### Spec-Referenzen
Siehe `README.md` + `docs/spec-coverage/<spec>.md`.

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments.

### Implementierung
ROS 2 rmw_zerodds shim — C-FFI wrapper around zerodds-ros2-rmw + zerodds-c-api

### Architektur
- Layer: 7 (Profiles)

### Stabilitaet
Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump.

### Added — CLI-Subcommands

- `zerodds-ros2-shim` CLI mit `catalog` (Topic-Inventur), `metrics`
  (Prometheus-Snapshot) und `selftest`-Subcommands.
- `zerodds-monitor` + `zerodds-observability-otlp`-Integration fuer
  Diagnose-Output (JSON-Lines + OTLP-Spans) — CLI ist exempt vom
  SIGTERM-Watcher (§9.2 gilt nur fuer Daemons).
