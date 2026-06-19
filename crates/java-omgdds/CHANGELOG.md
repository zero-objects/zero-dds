# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Helper package `org.zerodds.cdr` (pure Java, JVM 17+):
  - `Xcdr2Writer` / `Xcdr2Reader` mit Padding/Alignment per OMG XTypes
    1.3 §7.4.1.5, DHEADER (Appendable/Mutable), EMHEADER (PL_CDR2 LC0–7),
    NEXTINT, Optional-Presence-Flag, String (UTF-8 length+1+NUL),
    WString (UTF-16-LE), endian switch (LE default, BE for the key holder).
  - `TopicTypeSupport<T>` (extends `org.omg.dds.topic.TopicTypeSupport<T>`)
    mit `getTypeName`/`isKeyed`/`getExtensibility`/`encode`/`decode`/
    `keyHash` per `zerodds-xcdr2-java-1.0` §2/§3.
  - `Md5` (java.security-Wrapper), `EndianMode`, `ExtensibilityKind`,
    `XcdrException`.
- Wire-vector tests `Xcdr2WireVectorsTest` (16 tests) against
  `zerodds-xcdr2-bindings-conformance-1.0` §6 V-1..V-12 (corrected
  spec 2026-05-07): V-3 byte-exact (48 bytes natural alignment),
  V-8 Key-Hash byte-exact (`A5 15 85 57 99 DD BD A0 8B C9 9F C2 CE 87
  FA 79`), V-10 DHEADER=23 / 27 Bytes, V-11A DHEADER=8 / 12 Bytes.

### Notes
- The EMHEADER wire layout follows OMG XTypes 1.3 §7.4.3.4.5 (LC bits 30-28,
  member-id bits 27-0, in body endianness LE) and is consistent with
  the Rust encoder (`crates/cdr`). The conceptual MSB hex notation
  in the spec (e.g. `20 00 00 01` for {LC=2, id=1}) corresponds to the
  in-memory u32 view; the wire output is `01 00 00 20`. The V-10/
  V-11A test therefore verifies DHEADER bytes + byte count + roundtrip
  instead of an EMHEADER pseudo-MSB form.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-java-omgdds`** crate as a Layer-6 PSM/binding.

### Spec references
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM API surface
- ZeroDDS vendor spec `zerodds-c-api-1.0` (C-FFI foundation)

### Public API
See `README.md` + `src/lib.rs` doc comments + the associated coverage doc.

### Implementation
Native Java DDS PSM scaffolding (org.omg.dds.* package)

### Architecture
- Layer: 6 (PSMs / bindings)
- Dependencies (in): `zerodds-c-api` (foundation) + language-specific helper crates
- Dependents (out): user code

### Stability
All `pub` items are RC1-stable; breaking changes require a major bump.

