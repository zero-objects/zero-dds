# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-cpp`-Crate.

### Spec-Referenzen

- C++17 RAII-Wrapper-Header ueber [`zerodds-c-api`](../zerodds-c-api).
- Spec-konformer DDS-PSM-Cxx 1.0 Namespace wird vom Codegen in [`zerodds-idl-cpp`](../idl-cpp) erzeugt.

### Public-API

Keine Rust-Public-API. Die Surface dieser Crate liegt in:

- `include/zerodds/dds.hpp` — `zerodds::Runtime`, `zerodds::Writer`, `zerodds::Reader` (move-only RAII-Klassen).
- `examples/cpp_smoke.cpp` + `examples/build_cpp_smoke.sh` — kompilierbare End-to-End-Probe.

### Implementierung

`zerodds::Runtime::Runtime(domain_id)` ruft `zerodds_runtime_create`; Destructor ruft `zerodds_runtime_destroy`. `Writer::write(span)` ruft `zerodds_writer_write` mit Pointer + Length; Errors werden via `std::runtime_error("zerodds: ...")` propagiert. `Reader::take()` ruft `zerodds_reader_take`, kopiert das `bytes`-Array in einen `std::vector<uint8_t>`, und ruft `zerodds_reader_release` zur Cleanup-Phase.

Die Klassen sind move-only — Copy ist deleted, weil die zugrundeliegenden Handles nicht gemeinsam besessen werden duerfen.

`#![deny(unsafe_code)]` im Rust-Stub-`lib.rs`. Der eigentliche unsafe-FFI-Pfad lebt im C++-Header (per `extern "C"`-Decl), nicht in Rust.

### Architektur

- **Layer:** 6 (PSM/Bindings).
- **Dependencies (in):** keine ZeroDDS-Crate-Deps. C++-Build linkt zur Link-Zeit gegen `libzerodds.{so,dylib,a}` aus [`zerodds-c-api`](../zerodds-c-api).
- **Dependents (out):** Apex.AI-Plugin-Builds, ROS-2-RMW-Backends, embedded-C++-Apps.
- **Feature-Flags:** `std` (default), `alloc` (via std), `safety` (Reserve).
- `publish = false` — Distribution erfolgt als separates Header-Tarball, nicht via crates.io.

### Stabilitaet

C++-API + Header-ABI sind RC1-stabil. Major-Bumps erfolgen synchron mit `zerodds-c-api`.
