# Changelog — `zerodds-transport`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-05

### RC1-Audit
- License header (SPDX-Apache-2.0).
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- README with an architecture note on the `transport → rtps` crate dependency.
- Crate header with spec anchor DDSI-RTPS 2.5 §8.3.2 for the Locator re-export.

### Properties
- `no_std + alloc`, `forbid(unsafe_code)`.
- Safety class **SAFE**.
- 6 unit tests green; clippy clean.
- 15 cross-crate import sites (dcps, discovery, transport-udp/tcp/shm/uds, tools).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
