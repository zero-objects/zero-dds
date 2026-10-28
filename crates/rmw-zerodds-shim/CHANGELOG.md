# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

- Dynamic data of a bitmask (DDS-XTypes 1.3 §7.5.2.11.1, §7.5.2.11.3): the
  flags are Boolean values reached by bit (`zerodds_dyn_data_get_bool`,
  `zerodds_dyn_data_set_bool`, `zerodds_dyn_data_clear_value`), the id of a
  flag by name is its bit, and a bit that no flag names is refused. Before,
  the accessors read the list of the names of the flags that are set as if it
  were a list of elements.

- `zerodds_dyn_data_equals` compares as §7.5.2.11.5 has it (members without
  regard to their order, a member left out equal to its default).
- Dynamic data (DDS-XTypes 1.3 §7.5.2.11): `zerodds_dyn_data_clear_nonkey_values`
  keeps the key members, where `clear_all_values` served for both and reset
  them (§7.5.2.11.3); `clear_value` removes the element of a sequence and
  removes an optional member, where it set them to their default;
  `zerodds_dyn_data_item_count` is one for an atomic value (it was 0) and
  counts the set flags of a bitmask (§7.5.2.11.6); `zerodds_dyn_data_loan`
  returns nil while a loan is out, and `zerodds_dyn_data_return_loan`
  refuses a value that is not the outstanding loan of that sample
  (§7.5.2.11.7).

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
