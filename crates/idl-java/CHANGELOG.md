# Changelog — `zerodds-idl-java`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- XCDR2 TypeSupport codegen (`crates/idl-java/src/typesupport.rs`):
  emits a `<Name>TypeSupport.java` class per top-level `struct`
  that implements `org.zerodds.cdr.TopicTypeSupport<T>`
  (singleton `INSTANCE`, `getTypeName`, `isKeyed`, `getExtensibility`,
  `encode`/`decode` with an endian option, `keyHash` via MD5 over
  PlainCdr2BeKeyHolder).
- `JavaGenOptions::emit_typesupport` (default `true`) per
  `zerodds-xcdr2-java-1.0` §4. POJO snapshot tests set the flag to
  `false`; new TypeSupport snapshot tests
  (`snapshot_typesupport_{final,keyed,mutable}_struct`) cover Final/
  Mutable/Keyed.
- Wire mapping: Final → plain body, Appendable → DHEADER, Mutable →
  DHEADER + EMHEADER + LC code (LC=2 inline 4-byte / LC=3 inline 8-byte
  / LC=4 NEXTINT form for String/Sequence/Scoped).

### Changed
- Tests `edge_cases::three_top_level_structs_produce_three_files`,
  `nested_three_modules_become_three_packages`,
  `forward_struct_does_not_emit_file` and
  `fixtures::multi_file_output_one_class_per_top_level_type` now count
  POJO + TypeSupport (doubled file count).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization as a Layer-3 schema crate.

### Spec references
- OMG IDL4-Java 1.0 (idl4-java-1.0) + DDS-Java-PSM 1.0.

### RC1 audit
- License header (SPDX-Apache-2.0) on all src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X markers cleaned up (§1.13).

### Features
- 260 tests green; clippy clean.
- Pure-Rust, std-only (build-time tool, no embedded use case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
