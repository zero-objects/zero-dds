# Changelog — `zerodds-transport`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-05

### RC1-Audit
- License-Header (SPDX-Apache-2.0).
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- README mit Architektur-Note zur `transport → rtps` Crate-Dep.
- Crate-Header mit Spec-Anker DDSI-RTPS 2.5 §8.3.2 für Locator-Re-Export.

### Eigenschaften
- `no_std + alloc`, `forbid(unsafe_code)`.
- Safety-Klasse **SAFE**.
- 6 Unit-Tests grün; clippy clean.
- 15 Cross-Crate-Import-Sites (dcps, discovery, transport-udp/tcp/shm/uds, tools).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
