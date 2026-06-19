# Changelog — `zerodds-discovery`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

- License header (SPDX-Apache-2.0) on all 18 src files (root + sedp/
  + security/ + type_lookup/).
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate-header rewrite with full spec anchoring
  (DDSI-RTPS §8.5.3+§8.5.4, XTypes 1.3 §7.6.3.3.4, DDS-Security 1.2
  §7.4.4+§7.4.5).
- Phase-X markers cleaned up:
  - `lib.rs`: "Phase-0 scope (WP 0.7-A)" → full public-API
    description with a layer-boundary statement toward DCPS.
  - `spdp.rs`: "Phase-0 scope" removed; lease tracking documented as a
    caller-layer responsibility.
  - `sedp/mod.rs`: "Phase-1 scope (WP 1.4)" → module-contents list.
  - `sedp/stack.rs`: "Phase-2 cosmetics" → deliberate architecture
    decision explained; `silence unused in Phase 1` → commented
    as "currently not evaluated".
  - `endpoint_match.rs`: "Phase-1 fallback" → "fallback".
  - `security/mod.rs`: "Out-of-scope (C3.4-c)" → layer-boundary
    statement with spec section.
  - `type_lookup/mod.rs` + `endpoints.rs`: "TODO: wire" + "Phase-4
    followup" → layer-boundary statement toward the DCPS builtin-endpoint
    spawn path.
  - `capabilities.rs`: test "phase0_peer_only" → "legacy_peer_only".
- README + CHANGELOG.

### Properties

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety class **SAFE**.
- 144+ tests green; clippy/doc clean.

### Known cross-layer findings

- **F-DISC-1 / F-DCPS-typelookup-wiring** (RC2 target): TypeLookup
  service endpoints are fully implemented in `discovery`, but not
  spawned as reliable writer/reader pairs in `dcps::runtime`. To be
  fixed during the DCPS layer-3 review (XTypes 1.3 §7.6.3.3.4
  spec compliance).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
