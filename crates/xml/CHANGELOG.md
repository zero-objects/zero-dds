# Changelog — `zerodds-xml`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization as a Layer-3 schema crate.

### Spec-Referenzen
- OMG DDS-XML 1.0 — Parser + QoS-Profile-Loader + Building-Block-Foundation.

### RC1-Audit
- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X markers cleaned up (§1.13).

### Properties
- 302 tests green; clippy clean.
- Pure-Rust, std-only (build-time tool, no embedded use case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
