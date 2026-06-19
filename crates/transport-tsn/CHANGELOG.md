# Changelog — `zerodds-transport-tsn`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **§7.3.3 YANG PSM** (`pim::yang`): transformation of the configuration
  model into `group-talker`/`group-listener` (UNI CUC↔CNC) — incl.
  `stream-id-type` (node MAC + 16-bit DataWriter ID),
  `interface-capabilities` (vlan-tag-capable + 802.1CB lists),
  `traffic-specification` (interval as numerator/denominator) +
  RFC-7951 YANG-JSON renderer. Closes the former §7.3 `partial`;
  the old assumption "the JSON PSM covers YANG" was wrong (separate mapping).
- **`NetworkRequirements`** (Tab 7.18/7.25): `num_seamless_trees` +
  `max_latency`. Previously missing entirely.
- **`TsnConfiguration`** (Figure 7.3): aggregate root with
  `tsn_talker`/`tsn_listener`.
- **Live AF_PACKET transport testable**: frame logic in `live_frame`
  (platform-neutral, 12 unit tests) + `tests/veth_loopback.rs` (real
  RTPS roundtrip over veth, root/CI) + CI job `tsn-live`.

### Changed

- `TsnTalker`: `stream_name` + `datawriter_ref` + `network_requirements`
  (0..1) added; `data_frame` is now `Option` (spec mult 0..1).
- `TsnListener`: `stream_name` + `datareader_ref` + `network_requirements`.
- `IPv4Tuple`/`IPv6Tuple`: `dscp` field (Tab 7.22/7.23) added.
- Legacy `config::TsnConfiguration` no longer re-exported at the crate root
  (collision with spec `stream::TsnConfiguration`); still reachable as
  `config::TsnConfiguration`.
- 93 tests green (default), 105 with `--features live`.

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

- License header (SPDX-Apache-2.0) on all 14 src files (10 root +
  4 pim submodules).
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- README rewrite with spec-§ mapping + scope-boundary table.

### Properties

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety class **STANDARD**.
- 69 tests green; clippy clean.
- OMG DDS-TSN 1.0 (formal/2024-05-16) configuration model PIM (§7.2)
  + DDSI-RTPS Ethernet PSM (Annex A) + XML/JSON configuration PSM (§7.3).

### Spec coverage

Full coverage doc in
[`docs/spec-coverage/dds-tsn-1.0.md`](../../docs/spec-coverage/dds-tsn-1.0.md).
All spec sections `done` or explicitly marked as caller layer
(TSN UNI wire, YANG PSM, hardware timestamping, gPTP daemon).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
