# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- New helper library `ZeroDDS.Cdr` (`csharp/ZeroDDS.Cdr/`) as a pure-C#
  implementation of the vendor spec `zerodds-xcdr2-csharp-1.0`:
  - `IDdsTopicType<T>` interface (§2/§3).
  - `Xcdr2Writer` with padding/alignment per XTypes 1.3 §7.4.1.5,
    DHEADER (`BeginAppendable`/`BeginMutable`) and EMHEADER
    (`WriteEmHeader`/`WriteEmHeaderNextInt`).
  - `Xcdr2Reader` as a `ref struct` with bounds checks and a matching
    `BeginDHeader`/`EndDHeader`/`DHeaderDone` flow.
  - `Md5` as an RFC 1321 vendor implementation (no
    `System.Security.Cryptography.MD5` dependency -> portable to FIPS mode).
  - `EndianMode` + `ExtensibilityKind` enums, `XcdrException` for
    wire-format errors.
- New test suite `csharp/ZeroDDS.Cdr.Tests/` (xUnit) with
  `Xcdr2WireVectorsTests` (V-1..V-12 from
  zerodds-xcdr2-bindings-conformance-1.0 §6), `Md5Tests` (RFC 1321 +
  V-8 PlainCdr2BeKeyHolder), `AlignmentTests` (padding, DHEADER origin
  reset, endianness roundtrip, EMHEADER roundtrip).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the crate **`zerodds-cs`** as a Layer-6 PSM/binding.

### Spec references
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM API surface
- ZeroDDS vendor spec `zerodds-c-api-1.0` (C-FFI foundation)

### Public API
See `README.md` + `src/lib.rs` doc comments + the associated coverage doc.

### Implementation
C# P/Invoke, NativeAOT-compatible, IDL4-C# runtime

### Architecture
- Layer: 6 (PSMs / Bindings)
- Dependencies (in): `zerodds-c-api` (Foundation) + language-specific helper crates
- Dependents (out): user code

### Stability
All `pub` items are RC1-stable; breaking changes require a major bump.
