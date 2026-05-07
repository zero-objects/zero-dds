# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Added
- Neue Helper-Library `ZeroDDS.Cdr` (`csharp/ZeroDDS.Cdr/`) als pure-C#-
  Implementation der Vendor-Spec `zerodds-xcdr2-csharp-1.0`:
  - `IDdsTopicType<T>`-Interface (§2/§3).
  - `Xcdr2Writer` mit Padding/Alignment laut XTypes 1.3 §7.4.1.5,
    DHEADER (`BeginAppendable`/`BeginMutable`) und EMHEADER
    (`WriteEmHeader`/`WriteEmHeaderNextInt`).
  - `Xcdr2Reader` als `ref struct` mit Bounds-Checks und passendem
    `BeginDHeader`/`EndDHeader`/`DHeaderDone`-Flow.
  - `Md5` als RFC-1321-Vendor-Implementation (kein
    `System.Security.Cryptography.MD5`-Bedarf -> FIPS-mode-portabel).
  - `EndianMode` + `ExtensibilityKind` Enums, `XcdrException` fuer
    Wire-Format-Fehler.
- Neue Test-Suite `csharp/ZeroDDS.Cdr.Tests/` (xUnit) mit
  `Xcdr2WireVectorsTests` (V-1..V-12 aus
  zerodds-xcdr2-bindings-conformance-1.0 §6), `Md5Tests` (RFC-1321 +
  V-8 PlainCdr2BeKeyHolder), `AlignmentTests` (Padding, DHEADER-Origin-
  Reset, Endianness-Roundtrip, EMHEADER-Roundtrip).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung des Crates **`zerodds-cs`** als Layer-6-PSM-/-Binding.

### Spec-Referenzen
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM-API-Surface
- ZeroDDS-Vendor-Spec `zerodds-c-api-1.0` (C-FFI Foundation)

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments + zugehoerige Coverage-Doc.

### Implementierung
C# P/Invoke, NativeAOT-compatible, IDL4-C# runtime

### Architektur
- Layer: 6 (PSMs / Bindings)
- Dependencies (in): `zerodds-c-api` (Foundation) + Sprach-spezifische Helper-Crates
- Dependents (out): Anwender-Code

### Stabilitaet
Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump.

