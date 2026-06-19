# Changelog — `zerodds-idl-ts`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- XCDR2 TypeSupport codegen per `idl-struct` (zerodds-xcdr2-ts-1.0
  §3 + §4): emits `<Name>TypeSupport: DdsTopicType<<Name>>` const
  with `encode`/`decode`/`keyHash`/`typeName`/`isKeyed`/
  `extensibility` members.
- Auto-import header `import { Xcdr2Writer, Xcdr2Reader, md5 } from
  "@zerodds/cdr"` plus type-side `DdsTopicType, EndianMode`.
- Final/appendable/mutable encode+decode paths via
  `beginAppendable`/`beginMutable`/`writeEmHeader` helpers.
- `@key` fields are mapped to a 16-byte hash via `PlainCdr2BeKeyHolder`
  (BE) plus MD5 (XTypes §7.6.8).
- `@optional` member: present-byte for final/appendable, "omit EMHEADER
  if None" for mutable.

### Fixed
- Mutable member encode for non-primitive members (string, sequence,
  nested struct) now uses LC=3 + NEXTINT form per zerodds-
  xcdr2-bindings-conformance-1.0 §6 V-10 (previously LC=4).
- Mutable member decode consumes NEXTINT explicitly for LC=3 +
  non-primitive, instead of the standard XTypes 1.3 LC=3 path (8 bytes
  inline).

### Tests
- Snapshots `simple_struct`, `module_nesting`, `enum`, `union`,
  `struct_with_string_and_sequence`, `amqp_helpers_struct`,
  `amqp_helpers_union` updated (TypeSupport const + import block).
- `compile_check.rs` stub runtime extended with the `@zerodds/cdr`
  surface (Xcdr2Writer/Reader/md5/DdsTopicType/EndianMode) so that
  emitted TypeSupport constants are validatable via tsc.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization as a layer-3 schema crate.

### Spec references
- DDS-TS 1.0 (ZeroDDS-own spec) — TypeScript codegen for browser + Node.js.

### RC1 audit
- License header (SPDX-Apache-2.0) on all src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X markers cleaned up (§1.13).

### Features
- 149 tests green; clippy clean.
- Pure-Rust, std-only (build-time tool, no embedded use case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
