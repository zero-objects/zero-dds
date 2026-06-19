# Changelog — `zerodds-transport-uds`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

- License header (SPDX-Apache-2.0) on all src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate header rewrite: honest spec story (DDSI-RTPS §9.4 LocatorKind
  OMG-normative; path resolution + SOCK_DGRAM ZeroDDS-own).
- Internal-only review-cycle markers (`phase2-0-*`) stripped.
- ZeroDDS UDS Transport 1.0 spec materialized in
  `docs/spec-coverage/zerodds-uds-transport-1.0.md` (§1-§9 with
  path resolution, wire format, cleanup semantics, container use case,
  platform support, test mapping).
- README + CHANGELOG.

### Features

- `std`-only, safety class **STANDARD**.
- 17 tests green (16 lib + 1 cross-process integration); clippy clean.
- SOCK_DGRAM filesystem sockets (default).
- Linux abstract-namespace support via the `abstract_dgram` module.
- TOCTOU-safe bind-path validation.
- Lazy base-directory creation with mode 0700.

### Platform support

- Linux primary (filesystem + abstract namespace).
- macOS supported (filesystem only).
- Windows not supported (Unix-specific).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
