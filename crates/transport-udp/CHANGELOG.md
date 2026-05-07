# Changelog — `zerodds-transport-udp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-05

### RC1-Audit
- License-Header (SPDX-Apache-2.0).
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate-Header rewrite: ehrliche Auflistung (Multicast war seit WP 0.7-A
  live, nicht mehr "out-of-scope") + Spec-Anker DDSI-RTPS 2.5 §9.6.1.
- README + CHANGELOG.

### Eigenschaften
- `std`-only, `forbid(unsafe_code)`.
- Safety-Klasse **SAFE**.
- 11 Tests grün; clippy clean.
- UDPv4 Unicast + Multicast (Group-Join + TTL + SO_REUSE).
- Bind-Retry-Loop für CI-EADDRINUSE-Race.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
