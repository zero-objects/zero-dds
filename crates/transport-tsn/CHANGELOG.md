# Changelog — `zerodds-transport-tsn`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1-Audit

- License-Header (SPDX-Apache-2.0) auf alle 14 src-Files (10 root +
  4 pim-Submodule).
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- README rewrite mit Spec-§-Mapping + Scope-Boundary-Tabelle.

### Eigenschaften

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety-Klasse **STANDARD**.
- 69 Tests grün; clippy clean.
- OMG DDS-TSN 1.0 (formal/2024-05-16) Configuration-Modell PIM (§7.2)
  + DDSI-RTPS-Ethernet-PSM (Annex A) + XML/JSON-Configuration-PSM (§7.3).

### Spec-Coverage

Volle Coverage-Doc in
[`docs/spec-coverage/dds-tsn-1.0.md`](../../docs/spec-coverage/dds-tsn-1.0.md).
Alle Spec-Sektionen `done` oder explizit als Caller-Layer markiert
(TSN-UNI-Wire, YANG-PSM, Hardware-Timestamping, gPTP-Daemon).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
