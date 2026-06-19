# Changelog — `zerodds-idl`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization as a layer-3 schema crate.

### Spec references

- **OMG IDL 4.2** (formal/2018-01-05) / ISO/IEC 19516:2020 — Lexical
  Conventions §7.2, Preprocessing §7.3, Building Blocks §7.4
  (Constants, Types, Interfaces, Templates, Components, Annotations).
- **OMG XTypes 1.3** §7.3.1.2 (NameHash for TypeObject construction).
- **Vendor extensions**: RTI Connext (`@rti::*`, `keylist`,
  `#pragma DCPS_*`), OpenSplice/TAO (`#pragma DCPS_DATA_*`),
  FastDDS (XTypes alias quirks), Cyclone DDS (standard OMG IDL).

### Public API

- `parse(src, &ParserConfig) -> Result<IdlAst, Error>` — top-level entry.
- `parse_with_deltas(src, cfg, &[&GrammarDelta]) -> Result<IdlAst>` —
  vendor variant.
- `ParserConfig` — version, CompatMode, VendorExt flags.
- `IdlAst` (pretty-printable + roundtrip-capable).
- `Builder` + `Validator` for programmatic AST construction.
- `Preprocessor` + `MemoryResolver`/`FileResolver` (#include/#define/#ifdef).
- `IDL_42` + `GrammarDelta` — grammar engine.
- `lexer`, `cst`, `engine`, `ast`, `semantics`, `preprocessor`,
  `grammar` submodules.
- 1047 tests in total.

### Implementation

Earley recognizer on a central OMG-IDL-4.2 grammar (108 productions
with `spec_ref` annotations per rule). A memoization pass makes it
polynomial. CST→AST builder with source spans. Vendor deltas as
additive patches without base-grammar modification.

`forbid(unsafe_code)`, std-only (build-time tool, no embedded
use case). Pure Rust without external parser crates.

### Architecture

- Layer 3 (schema). Consumed by `zerodds-idl-{cpp,csharp,java,rust,ts}`.
- Dependencies: `zerodds-types`, `num-bigint`, `num-traits`.
- Feature flags: `default = ["std"]`, `std = []`.

### Stability

All `pub` items RC1-stable. Breaking changes require a major bump.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
