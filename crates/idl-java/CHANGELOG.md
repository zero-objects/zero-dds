# Changelog — `zerodds-idl-java`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- XCDR2-TypeSupport-Codegen (`crates/idl-java/src/typesupport.rs`):
  emittiert pro Top-Level-`struct` eine `<Name>TypeSupport.java`-Klasse,
  die `org.zerodds.cdr.TopicTypeSupport<T>` implementiert
  (Singleton-`INSTANCE`, `getTypeName`, `isKeyed`, `getExtensibility`,
  `encode`/`decode` mit Endian-Option, `keyHash` per MD5 ueber
  PlainCdr2BeKeyHolder).
- `JavaGenOptions::emit_typesupport` (Default `true`) gemaess
  `zerodds-xcdr2-java-1.0` §4. POJO-Snapshot-Tests setzen den Flag auf
  `false`; neue TypeSupport-Snapshot-Tests
  (`snapshot_typesupport_{final,keyed,mutable}_struct`) decken Final/
  Mutable/Keyed ab.
- Wire-Mapping: Final → Plain-Body, Appendable → DHEADER, Mutable →
  DHEADER + EMHEADER + LC-Code (LC=2 inline 4-byte / LC=3 inline 8-byte
  / LC=4 NEXTINT-Form fuer String/Sequence/Scoped).

### Changed
- Tests `edge_cases::three_top_level_structs_produce_three_files`,
  `nested_three_modules_become_three_packages`,
  `forward_struct_does_not_emit_file` und
  `fixtures::multi_file_output_one_class_per_top_level_type` zaehlen
  jetzt POJO + TypeSupport (verdoppelte File-Anzahl).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung als Layer-3-Schema-Crate.

### Spec-Referenzen
- OMG IDL4-Java 1.0 (idl4-java-1.0) + DDS-Java-PSM 1.0.

### RC1-Audit
- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X-Marker bereinigt (§1.13).

### Eigenschaften
- 260 Tests grün; clippy clean.
- Pure-Rust, std-only (Build-Zeit-Tool, kein embedded-Use-Case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
