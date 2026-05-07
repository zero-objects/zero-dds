# Changelog — `zerodds-idl-ts`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- XCDR2 TypeSupport-Codegen pro `idl-struct` (zerodds-xcdr2-ts-1.0
  §3 + §4): emits `<Name>TypeSupport: DdsTopicType<<Name>>` const
  mit `encode`/`decode`/`keyHash`/`typeName`/`isKeyed`/
  `extensibility`-Members.
- Auto-Import-Header `import { Xcdr2Writer, Xcdr2Reader, md5 } from
  "@zerodds/cdr"` plus type-side `DdsTopicType, EndianMode`.
- Final/Appendable/Mutable encode+decode-Pfade via
  `beginAppendable`/`beginMutable`/`writeEmHeader`-Helper.
- `@key`-Felder werden ueber `PlainCdr2BeKeyHolder` (BE) plus MD5
  zum 16-Byte-Hash gemappt (XTypes §7.6.8).
- `@optional`-Member: present-byte fuer Final/Appendable, "EMHEADER
  weglassen wenn None" fuer Mutable.

### Fixed
- Mutable-Member-Encode fuer non-primitive Members (string, sequence,
  nested struct) verwendet jetzt LC=3 + NEXTINT-form per zerodds-
  xcdr2-bindings-conformance-1.0 §6 V-10 (vorher LC=4).
- Mutable-Member-Decode konsumiert NEXTINT explizit fuer LC=3 +
  non-primitive, statt dem standard XTypes-1.3-LC=3-Pfad (8 Bytes
  inline).

### Tests
- Snapshots `simple_struct`, `module_nesting`, `enum`, `union`,
  `struct_with_string_and_sequence`, `amqp_helpers_struct`,
  `amqp_helpers_union` aktualisiert (TypeSupport-Const + Import-Block).
- `compile_check.rs` Stub-Runtime erweitert um `@zerodds/cdr`-Surface
  (Xcdr2Writer/Reader/md5/DdsTopicType/EndianMode) damit emittierte
  TypeSupport-Konstanten via tsc validierbar sind.

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung als Layer-3-Schema-Crate.

### Spec-Referenzen
- DDS-TS 1.0 (ZeroDDS-eigene Spec) — TypeScript-Codegen für Browser + Node.js.

### RC1-Audit
- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X-Marker bereinigt (§1.13).

### Eigenschaften
- 149 Tests grün; clippy clean.
- Pure-Rust, std-only (Build-Zeit-Tool, kein embedded-Use-Case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
