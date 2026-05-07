# Changelog — `zerodds-idl-csharp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- XCDR2 TypeSupport-Codegen: pro IDL-`struct` wird zusaetzlich zur
  Datenklasse eine `*TypeSupport`-Klasse emittiert, die
  `ZeroDDS.Cdr.IDdsTopicType<T>` (zerodds-xcdr2-csharp-1.0 §3) implementiert.
- Encode-/Decode-/KeyHash-Emission deckt §5 Wire-Type-Mapping
  (Primitives, `string`, `sequence<T>`, nested modules), §6 Extensibility
  (Final/Appendable/Mutable inkl. DHEADER + EMHEADER) und §7 Key-Extraction
  (PlainCdr2BeKeyHolder + MD5-Fallback ueber 16 Byte) ab.
- Spec-Type-Name-Konvention `Module1::Module2::Struct`
  (zerodds-xcdr2-bindings-conformance-1.0 §5) wird ueber den neuen
  Module-Path-Tracker im Emitter gesetzt.
- `using ZeroDDS.Cdr;` wird automatisch hinzugefuegt, wenn ein Top-Level-
  (non-`@nested`)-Struct emittiert wird.
- 11 Snapshot-Tests fuer V-1..V-11 (`tests/snapshot_xcdr2_vectors.rs`)
  belegen die Codegen-Form der Conformance-Wire-Vector-IDLs.

### Changed
- `tests/compile_check.rs` haelt jetzt zusaetzlich einen Inline-Stub fuer
  `ZeroDDS.Cdr.*` und `Omg.Types.SequenceList<T>`, damit der dotnet-build
  des generierten Codes ohne externes Assembly-Reference-Wiring durchlaeuft.

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung als Layer-3-Schema-Crate.

### Spec-Referenzen
- OMG IDL4-CSharp 1.0 (idl4-csharp-1.0).

### RC1-Audit
- License-Header (SPDX-Apache-2.0) auf alle src-Files.
- Cargo.toml RC1-Metadaten (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Phase-X-Marker bereinigt (§1.13).

### Eigenschaften
- 193 Tests grün; clippy clean.
- Pure-Rust, std-only (Build-Zeit-Tool, kein embedded-Use-Case).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
