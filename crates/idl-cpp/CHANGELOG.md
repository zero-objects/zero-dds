# Changelog — `zerodds-idl-cpp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Codegen emittiert pro IDL-`struct` eine
  `dds::topic::topic_type_support<T>`-Spezialisierung mit
  `type_name()`, `encode()` und `decode()`. Wire-Format ist `cdr_lite`
  (LE, kein Padding, kein DHEADER/EMHEADER) — kompatibel zum
  ZeroDDS↔ZeroDDS-Self-Loop. Voll-XCDR2 fuer Cross-Vendor laeuft
  weiterhin via FFI (`zerodds_writer_write_xcdr2`).
  Header-Helpers leben in `crates/cpp/include/dds/topic/TopicTraits.hpp`
  unter `dds::topic::cdr_lite`. Damit ist der Trait-Pflicht-Punkt aus
  dem `TopicTraits.hpp`-Doc-Comment kein Aspirational-Status mehr —
  vier C++-Compile+Run-Roundtrip-Tests in `tests/compile_check.rs`
  belegen die Symmetrie fuer Primitive, String, `sequence<T>` und
  Modul-nested Types. `@optional`/`@shared`-Member werden mit
  Doc-Kommentar uebersprungen (Storage bleibt default-konstruiert),
  Arrays + nested user-Types ebenso.

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung als Layer-3-Schema-Crate.

### Spec-Referenzen
- OMG IDL4-CPP 1.0 (idl4-cpp-1.0) + DDS-PSM-Cxx 1.0 + DDS-RPC C++ PSM.

### RC1-Audit
- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X-Marker bereinigt (§1.13).

### Eigenschaften
- 283 Tests grün; clippy clean.
- Pure-Rust, std-only (Build-Zeit-Tool, kein embedded-Use-Case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
