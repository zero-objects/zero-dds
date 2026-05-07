# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Added
- Helper-Package `org.zerodds.cdr` (Pure-Java, JVM 17+):
  - `Xcdr2Writer` / `Xcdr2Reader` mit Padding/Alignment per OMG XTypes
    1.3 §7.4.1.5, DHEADER (Appendable/Mutable), EMHEADER (PL_CDR2 LC0–7),
    NEXTINT, Optional-Presence-Flag, String (UTF-8 length+1+NUL),
    WString (UTF-16-LE), Endian-Switch (LE Default, BE fuer Key-Holder).
  - `TopicTypeSupport<T>` (extends `org.omg.dds.topic.TopicTypeSupport<T>`)
    mit `getTypeName`/`isKeyed`/`getExtensibility`/`encode`/`decode`/
    `keyHash` per `zerodds-xcdr2-java-1.0` §2/§3.
  - `Md5` (java.security-Wrapper), `EndianMode`, `ExtensibilityKind`,
    `XcdrException`.
- Wire-Vector-Tests `Xcdr2WireVectorsTest` (16 Tests) gegen
  `zerodds-xcdr2-bindings-conformance-1.0` §6 V-1..V-12 (corrected
  spec 2026-05-07): V-3 byte-exact (48 Bytes natuerliche Alignment),
  V-8 Key-Hash byte-exact (`A5 15 85 57 99 DD BD A0 8B C9 9F C2 CE 87
  FA 79`), V-10 DHEADER=23 / 27 Bytes, V-11A DHEADER=8 / 12 Bytes.

### Notes
- EMHEADER-Wire-Layout folgt OMG XTypes 1.3 §7.4.3.4.5 (LC bits 30-28,
  member-id bits 27-0, in Body-Endianness LE) und ist konsistent mit
  dem Rust-Encoder (`crates/cdr`). Die konzeptuelle MSB-Hex-Notation
  in der Spec (z.B. `20 00 00 01` fuer {LC=2, id=1}) entspricht der
  in-memory u32-Sicht; der Wire-Output ist `01 00 00 20`. Der V-10/
  V-11A-Test verifiziert daher DHEADER-Bytes + Byte-Anzahl + Roundtrip
  statt einer EMHEADER-Pseudo-MSB-Form.

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung des Crates **`zerodds-java-omgdds`** als Layer-6-PSM-/-Binding.

### Spec-Referenzen
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM-API-Surface
- ZeroDDS-Vendor-Spec `zerodds-c-api-1.0` (C-FFI Foundation)

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments + zugehoerige Coverage-Doc.

### Implementierung
Native Java DDS-PSM scaffolding (org.omg.dds.* package)

### Architektur
- Layer: 6 (PSMs / Bindings)
- Dependencies (in): `zerodds-c-api` (Foundation) + Sprach-spezifische Helper-Crates
- Dependents (out): Anwender-Code

### Stabilitaet
Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump.

