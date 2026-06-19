# Changelog — `zerodds-transport-tcp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-05

### RC1 audit
- License header (SPDX-Apache-2.0) on all 4 src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate header rewrite: honest spec story (DDSI-RTPS §9.4+§9.5
  OMG-normative, handshake ZeroDDS-own) instead of the non-existent
  "DDS-TCP-PSM §5.2.1" reference.
- `handshake.rs` header rewrite: phase-2b TODO markers removed;
  cross-vendor interop status clearly documented.
- ZeroDDS TCP Transport 1.0 spec materialized in
  `docs/spec-coverage/zerodds-tcp-transport-1.0.md` (§1-§8 with
  wire format, reject codes, Cyclone compat, test mapping).
- README + CHANGELOG.

### Features
- `std`-only, `forbid(unsafe_code)`.
- Safety class **STANDARD**.
- 55 tests green (50 lib + 5 integration); clippy clean.
- Length-prefix frame (DDSI-RTPS §9.5 conformant) + ZeroDDS handshake
  + Cyclone compat mode + connection pool.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
