# Changelog — `zerodds-transport-uds`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1-Audit

- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate-Header rewrite: ehrliche Spec-Story (DDSI-RTPS §9.4 LocatorKind
  OMG-normativ; Path-Resolution + SOCK_DGRAM ZeroDDS-eigen).
- Internal-only Review-Cycle-Marker (`phase2-0-*`) gestrippt.
- ZeroDDS-UDS-Transport-1.0-Spec materialisiert in
  `docs/spec-coverage/zerodds-uds-transport-1.0.md` (§1-§9 mit
  Path-Resolution, Wire-Format, Cleanup-Semantik, Container-Use-Case,
  Plattform-Support, Test-Mapping).
- README + CHANGELOG.

### Eigenschaften

- `std`-only, Safety-Klasse **STANDARD**.
- 17 Tests grün (16 lib + 1 cross-process integration); clippy clean.
- SOCK_DGRAM-Filesystem-Sockets (default).
- Linux Abstract-Namespace-Support via `abstract_dgram`-Modul.
- TOCTOU-sichere Bind-Path-Validation.
- Lazy Base-Directory-Erstellung mit Mode 0700.

### Plattform-Support

- Linux primary (Filesystem + Abstract Namespace).
- macOS supported (Filesystem only).
- Windows nicht supported (Unix-spezifisch).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
