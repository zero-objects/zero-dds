# Changelog — `zerodds-transport-tcp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-05

### RC1-Audit
- License-Header (SPDX-Apache-2.0) auf alle 4 src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate-Header rewrite: ehrliche Spec-Story (DDSI-RTPS §9.4+§9.5
  OMG-normativ, Handshake ZeroDDS-eigen) statt nicht-existenter
  "DDS-TCP-PSM §5.2.1"-Referenz.
- `handshake.rs`-Header rewrite: Phase-2b-TODO-Marker entfernt;
  Cross-Vendor-Interop-Status klar dokumentiert.
- ZeroDDS-TCP-Transport-1.0-Spec materialisiert in
  `docs/spec-coverage/zerodds-tcp-transport-1.0.md` (§1-§8 mit
  Wire-Format, Reject-Codes, Cyclone-Compat, Test-Mapping).
- README + CHANGELOG.

### Eigenschaften
- `std`-only, `forbid(unsafe_code)`.
- Safety-Klasse **STANDARD**.
- 55 Tests grün (50 lib + 5 integration); clippy clean.
- Length-Prefix-Frame (DDSI-RTPS §9.5-konform) + ZeroDDS-Handshake
  + Cyclone-Compat-Mode + Connection-Pool.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
