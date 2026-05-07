# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung des Crates **`zerodds-ts-wasm`** als Layer-6-PSM-/-Binding.

### Spec-Referenzen
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM-API-Surface
- ZeroDDS-Vendor-Spec `zerodds-c-api-1.0` (C-FFI Foundation)

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments + zugehoerige Coverage-Doc.

### Implementierung
ZeroDDS WASM bindings — XCDR1/XCDR2 codec für Browser/JS

### Architektur
- Layer: 6 (PSMs / Bindings)
- Dependencies (in): `zerodds-c-api` (Foundation) + Sprach-spezifische Helper-Crates
- Dependents (out): Anwender-Code

### Stabilitaet
Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump.

