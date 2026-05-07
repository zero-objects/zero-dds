# Changelog — `zerodds-discovery`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

### RC1-Audit

- License-Header (SPDX-Apache-2.0) auf alle 18 src-Files (root + sedp/
  + security/ + type_lookup/).
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate-Header rewrite mit vollständiger Spec-Verankerung
  (DDSI-RTPS §8.5.3+§8.5.4, XTypes 1.3 §7.6.3.3.4, DDS-Security 1.2
  §7.4.4+§7.4.5).
- Phase-X-Marker bereinigt:
  - `lib.rs`: "Phase-0-Scope (WP 0.7-A)" → vollständige Public-API-
    Beschreibung mit Layer-Boundary-Statement an DCPS.
  - `spdp.rs`: "Phase-0-Scope" entfernt; Lease-Tracking als Caller-
    Layer-Responsibility dokumentiert.
  - `sedp/mod.rs`: "Phase-1-Scope (WP 1.4)" → Modul-Inhalt-Liste.
  - `sedp/stack.rs`: "Phase-2-Kosmetik" → bewusste Architektur-
    Entscheidung erklärt; `silence unused in Phase 1` → kommentiert
    als "derzeit nicht ausgewertet".
  - `endpoint_match.rs`: "Phase-1-Fallback" → "Fallback".
  - `security/mod.rs`: "Out-of-scope (C3.4-c)" → Layer-Boundary-
    Statement mit Spec-Sektion.
  - `type_lookup/mod.rs` + `endpoints.rs`: "TODO: wire" + "Phase-4-
    Followup" → Layer-Boundary-Statement an DCPS-Builtin-Endpoint-
    Spawn-Pfad.
  - `capabilities.rs`: Test "phase0_peer_only" → "legacy_peer_only".
- README + CHANGELOG.

### Eigenschaften

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety-Klasse **SAFE**.
- 144+ Tests grün; clippy/doc clean.

### Bekannte Cross-Layer-Findings

- **F-DISC-1 / F-DCPS-typelookup-wiring** (RC2-Target): TypeLookup-
  Service-Endpoints sind in `discovery` voll implementiert, aber nicht
  in `dcps::runtime` als Reliable-Writer/Reader-Pairs gespawnt. Wird
  während des DCPS-Layer-3-Review behoben (XTypes 1.3 §7.6.3.3.4
  Spec-Compliance).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
