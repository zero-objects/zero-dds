# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-corba-rust`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Annex-A** — IDL-Mapping-Tabellen.
- **OMG IDL4** §7.4.6 (interface), §7.4.5.4 (valuetype), §7.4.16 (oneway), §7.4.3.1 (attribute).
- **`zerodds-corba-rust-1.0`** Vendor-Spec (`docs/specs/zerodds-corba-rust-1.0.md`).

### Public-API

**Codegen-Entry:**
- `generate_corba_rust_module(spec, opts)` — AST → Rust-String.
- `CorbaRustGenOptions { header_comment }`.
- `error::CorbaRustError` — Fehler-Familie inkl. `From<zerodds_idl_rust::RustGenError>`.

**Runtime-Types** (vom emittierten Code referenziert):
- `ObjectReference` — IOR-Wrapper mit `type_id` + `iiop_profile`.
- `CorbaException` — `SystemException { minor, message }` + `UserException { repository_id, payload }`.
- `SkeletonResult` — `Reply(Vec<u8>)` / `Exception(...)` / `BadOperation` / `NotYetWired`.
- `ValueBase` — Trait für alle Valuetype-Implementierungen.
- `Servant` — POA-Servant-Marker mit `target_repository_id`.

### Implementierung

- **Interface-Trait:** pro IDL-Operation eine trait-method mit `Result<T, CorbaException>`-Return; `&self` für state-frei, `&mut self` wenn out/inout-Params; `attribute` → getter + setter.
- **Client-Stub:** `pub struct IStub { object_ref: ObjectReference }` mit `impl I for IStub` — Phase-1 returnt `SystemException("not yet wired")`; Phase-2 wired GIOP via `corba-iiop`.
- **Server-Skeleton:** `pub fn dispatch_<i>(servant: &dyn I, operation: &str, _payload: &[u8]) -> SkeletonResult` mit Operation-Name-Switch — Phase-1 returnt `NotYetWired`; Phase-2 decodet payload + ruft servant.
- **Valuetype-Trait:** `pub trait V: ValueBase` mit state-member-getter (public-state als `fn x(&self) -> T`, private als `fn _priv_x(&self) -> T`).

### Architektur

- **Layer:** 8 (CORBA-Stack).
- **Dependencies (in):** `zerodds-idl` (AST), `zerodds-idl-rust` (Type-Mapping), `zerodds-corba-codegen` (Special-Types + Stub/Skeleton-Templates).
- **Dependents (out):** End-User-Build-Scripts oder CLI-Tool das CORBA-IDL-Files für Rust-Service-Implementierungen generiert.

### Tests

- 6 Snapshot-Tests in `tests/snapshot_codegen.rs` (simple interface, attribute, oneway, inout, valuetype, module-nested).

### Out-of-Scope (Phase 2+)

- **Component / Home / D&C-Deployment** — CCM-Servant-Bindings, konsumiert dann `corba-ccm-lib`.
- **POA-Configuration-Builder** — `pub struct PoaBuilder`-API für Activation, Lifespan, RequestProcessing-Policies.
- **GIOP-Wire-Wiring im Stub/Skeleton** — Phase-1 emittiert Stubs/Skeletons mit `NotYetWired`-Return; Phase-2 ersetzt das durch echte GIOP-Marshalling über `corba-iiop`/`corba-giop`.
- **User-Exception-Codegen** — IDL `exception E { ... };` mit eigenem Error-Type pro Interface.
- **Repository-ID-Helper im generierten Code** — aktuell nicht emittiert; Phase-2 fügt `const REPOSITORY_ID: &str = "IDL:..."` zum trait hinzu.

### Stabilität

`1.0.0-rc.1` — RC-Phase. `pub`-Items der Codegen-Entry-API sind stabil; Runtime-Types (`ObjectReference` / `CorbaException` / etc.) können Phase-2-Erweiterungen erhalten (z.B. neue `CorbaException`-Varianten via `#[non_exhaustive]`).
