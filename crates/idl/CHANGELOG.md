# Changelog — `zerodds-idl`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung als Layer-3-Schema-Crate.

### Spec-Referenzen

- **OMG IDL 4.2** (formal/2018-01-05) / ISO/IEC 19516:2020 — Lexical
  Conventions §7.2, Preprocessing §7.3, Building Blocks §7.4
  (Constants, Types, Interfaces, Templates, Components, Annotations).
- **OMG XTypes 1.3** §7.3.1.2 (NameHash für TypeObject-Construction).
- **Vendor-Erweiterungen**: RTI Connext (`@rti::*`, `keylist`,
  `#pragma DCPS_*`), OpenSplice/TAO (`#pragma DCPS_DATA_*`),
  FastDDS (XTypes-Alias-Quirks), Cyclone DDS (Standard-OMG-IDL).

### Public-API

- `parse(src, &ParserConfig) -> Result<IdlAst, Error>` — Top-Level-Entry.
- `parse_with_deltas(src, cfg, &[&GrammarDelta]) -> Result<IdlAst>` —
  Vendor-Variante.
- `ParserConfig` — Version, CompatMode, VendorExt-Flags.
- `IdlAst` (Pretty-Printable + Roundtrip-fähig).
- `Builder` + `Validator` für Programmatic AST-Konstruktion.
- `Preprocessor` + `MemoryResolver`/`FileResolver` (#include/#define/#ifdef).
- `IDL_42` + `GrammarDelta` — Grammar-Engine.
- `lexer`, `cst`, `engine`, `ast`, `semantics`, `preprocessor`,
  `grammar` Submodule.
- 1047 Tests insgesamt.

### Implementierung

Earley-Recognizer auf zentraler OMG-IDL-4.2-Grammar (108 Productions
mit `spec_ref`-Annotationen pro Rule). Memoization-Pass macht das
polynomial. CST→AST-Builder mit Source-Spans. Vendor-Deltas als
additive Patches ohne Base-Grammar-Modifikation.

`forbid(unsafe_code)`, std-only (Build-Zeit-Tool, kein embedded-
Use-Case). Pure-Rust ohne externe Parser-Crates.

### Architektur

- Layer 3 (Schema). Konsumiert von `zerodds-idl-{cpp,csharp,java,rust,ts}`.
- Dependencies: `zerodds-types`, `num-bigint`, `num-traits`.
- Feature-Flags: `default = ["std"]`, `std = []`.

### Stabilität

Alle `pub`-Items RC1-stabil. Breaking-Changes erfordern Major-Bump.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
