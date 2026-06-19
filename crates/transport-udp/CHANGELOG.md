# Changelog — `zerodds-transport-udp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-05

### RC1 audit
- License header (SPDX-Apache-2.0).
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate header rewrite: honest listing (multicast had been live since WP
  0.7-A, no longer "out-of-scope") + spec anchor DDSI-RTPS 2.5 §9.6.1.
- README + CHANGELOG.

### Features
- `std`-only, `forbid(unsafe_code)`.
- Safety class **SAFE**.
- 11 tests green; clippy clean.
- UDPv4 unicast + multicast (group join + TTL + SO_REUSE).
- Bind retry loop for the CI EADDRINUSE race.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
