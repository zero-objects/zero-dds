# Changelog — `zerodds-idl-csharp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- XCDR2 TypeSupport codegen: per IDL `struct`, in addition to the
  data class a `*TypeSupport` class is emitted that
  implements `ZeroDDS.Cdr.IDdsTopicType<T>` (zerodds-xcdr2-csharp-1.0 §3).
- Encode/decode/keyHash emission covers §5 wire type mapping
  (primitives, `string`, `sequence<T>`, nested modules), §6 extensibility
  (Final/Appendable/Mutable incl. DHEADER + EMHEADER) and §7 key extraction
  (PlainCdr2BeKeyHolder + MD5 fallback over 16 byte).
- The spec type-name convention `Module1::Module2::Struct`
  (zerodds-xcdr2-bindings-conformance-1.0 §5) is set via the new
  module-path tracker in the emitter.
- `using ZeroDDS.Cdr;` is added automatically when a top-level
  (non-`@nested`) struct is emitted.
- 11 snapshot tests for V-1..V-11 (`tests/snapshot_xcdr2_vectors.rs`)
  document the codegen form of the conformance wire-vector IDLs.

### Changed
- `tests/compile_check.rs` now additionally holds an inline stub for
  `ZeroDDS.Cdr.*` and `Omg.Types.SequenceList<T>`, so the dotnet build
  of the generated code goes through without external assembly-reference wiring.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization as a Layer-3 schema crate.

### Spec references
- OMG IDL4-CSharp 1.0 (idl4-csharp-1.0).

### RC1 audit
- License header (SPDX-Apache-2.0) on all src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X markers cleaned up (§1.13).

### Properties
- 193 tests green; clippy clean.
- Pure-Rust, std-only (build-time tool, no embedded use case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
