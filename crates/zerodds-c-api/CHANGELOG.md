# Changelog — `zerodds-c-api`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer.

## [Unreleased]

### Added

- **`xcdr2`-Modul** — implementiert die `zerodds-xcdr2-c-1.0`
  Vendor-Spec.
  - `ZeroDdsTypeSupport` (`zerodds_typesupport_t` C-side) als
    function-table-basiertes TypeSupport-Struct.
  - FFI-Funktionen `zerodds_topic_create_typed`,
    `zerodds_topic_destroy_typed`, `zerodds_writer_write_typed`,
    `zerodds_reader_take_typed`, `zerodds_xcdr2_encode`,
    `zerodds_xcdr2_decode`.
  - `ZeroDdsTopic` opaque-Handle fuer typisierte Topics.
  - Helper-Funktionen `copy_to_out_buf`, `input_slice`, `write_out_len`
    fuer Codegen-Encoder/-Decoder.
- **`include/zerodds_xcdr2.h`** — hand-maintained C99-Header mit
  TypeSupport-Struct + FFI-Forward-Declarations + inline-Helpers fuer
  Codegen-Output (`zerodds_xcdr2_c_write_uN/iN/fN/string`,
  `zerodds_xcdr2_c_read_*`, `zerodds_xcdr2_c_kh_write_*`,
  `zerodds_xcdr2_c_compute_key_hash`, eingebauter MD5).
- **L1 Wire-Conformance-Tests** in `tests/xcdr2_wire_vectors.rs` —
  pruefen V-1..V-12 byte-genau ueber das FFI-Pattern.
- **L2 Codegen-Conformance-Tests** in `tests/xcdr2_c_codegen.rs` —
  verifizieren `idl-cpp::generate_c_header`-Output fuer jeden
  V-1..V-12 IDL-Snippet.
- **L2 C-Compile-Tests** in `tests/xcdr2_c_compile.rs` — kompilieren
  jeden generierten Header mit `cc -std=c99 -Wall -Werror`, um C99-
  Validitaet zu garantieren (skipt wenn kein C-Compiler im PATH).

### Changed

- `Cargo.toml`: add `zerodds-cdr` als runtime-Abhaengigkeit (mit
  `alloc` + `std`-Features), `zerodds-idl` + `zerodds-idl-cpp` als
  dev-dependencies fuer Codegen-/Wire-Tests.
- `src/lib.rs`: registriert `pub mod xcdr2`.

### Notes

- **Codegen-Pfad-Wahl** (Vendor-Spec §4): **Option A** umgesetzt — neues
  Modul `crates/idl-cpp/src/c_mode.rs` mit `generate_c_header` /
  `CGenOptions`, integriert in die existierende `zerodds-idl-cpp`-
  Crate. Begrenzt auf den V-1..V-12-Korpus + extensible Form (final/
  appendable/mutable Strukturen, primitive Typen, `string`, `sequence<T>`,
  geschachtelte Module, `@key`, `@id`).
- **Wire-Vector-Errata** vs. `zerodds-xcdr2-bindings-conformance-1.0`
  §6:
  - **V-3** Spec-Doku zeigt 40-Byte-Wire mit drei numerischen Fehlern
    (`l = -1234567`, `ul = 2345678`, `ll = -987654321`); XCDR2-spec-
    konforme LE-Bytes ergeben 48 Byte. Tests pruefen die korrigierte
    Form, die mit `zerodds-cdr` und Cyclone DDS interoperiert.
  - **V-10**/V-11/V-12 EMHEADER ist Spec-Doku visuell BE-gruppiert
    dargestellt; Wire-Bytes sind LE-serialisiert (XCDR2 stream-
    endianness gilt fuer alle Felder, inkl. EMHEADER). Tests assertieren
    die LE-Form, konsistent mit `zerodds-cdr::struct_enc` und Cyclone DDS.

### Added — ABI-Compat-Test

- `tests/abi_compat.rs` + `abi.snapshot.json` als 185-Symbol-Baseline
  fuer Drift-Detection: jeder neue oder umbenannte FFI-Symbol-Eintrag
  laesst den Test fail'en, bis `abi.snapshot.json` mit Begruendung
  aktualisiert wurde. `serde` + `serde_json` als dev-deps.
