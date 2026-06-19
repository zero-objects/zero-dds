# Changelog — `zerodds-idl-cpp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Codegen emits a `dds::topic::topic_type_support<T>`
  specialization per IDL `struct`, with
  `type_name()`, `encode()`, and `decode()`. The wire format is `cdr_lite`
  (LE, no padding, no DHEADER/EMHEADER) — compatible with the
  ZeroDDS↔ZeroDDS self-loop. Full XCDR2 for cross-vendor still
  runs via FFI (`zerodds_writer_write_xcdr2`).
  The header helpers live in `crates/cpp/include/dds/topic/TopicTraits.hpp`
  under `dds::topic::cdr_lite`. This means the mandatory-trait point from
  the `TopicTraits.hpp` doc comment is no longer aspirational —
  four C++ compile+run round-trip tests in `tests/compile_check.rs`
  demonstrate the symmetry for primitives, strings, `sequence<T>`, and
  module-nested types. `@optional`/`@shared` members are skipped with
  a doc comment (storage remains default-constructed), as are
  arrays and nested user types.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization as a layer-3 schema crate.

### Spec references
- OMG IDL4-CPP 1.0 (idl4-cpp-1.0) + DDS-PSM-Cxx 1.0 + DDS-RPC C++ PSM.

### RC1 audit
- License header (SPDX-Apache-2.0) on all src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X markers cleaned up (§1.13).

### Properties
- 283 tests passing; clippy clean.
- Pure Rust, std-only (build-time tool, no embedded use case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
