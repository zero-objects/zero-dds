# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initial release materialization of the `zerodds-corba-rust` crate.

### Spec references

- **OMG CORBA 3.3 Annex-A** — IDL mapping tables.
- **OMG IDL4** §7.4.6 (interface), §7.4.5.4 (valuetype), §7.4.16 (oneway), §7.4.3.1 (attribute).
- **`zerodds-corba-rust-1.0`** vendor spec (`docs/specs/zerodds-corba-rust-1.0.md`).

### Public API

**Codegen entry:**
- `generate_corba_rust_module(spec, opts)` — AST → Rust string.
- `CorbaRustGenOptions { header_comment }`.
- `error::CorbaRustError` — error family incl. `From<zerodds_idl_rust::RustGenError>`.

**Runtime types** (referenced by the emitted code):
- `ObjectReference` — IOR wrapper with `type_id` + `iiop_profile`.
- `CorbaException` — `SystemException { minor, message }` + `UserException { repository_id, payload }`.
- `SkeletonResult` — `Reply(Vec<u8>)` / `Exception(...)` / `BadOperation` / `NotYetWired`.
- `ValueBase` — trait for all valuetype implementations.
- `Servant` — POA servant marker with `target_repository_id`.

### Implementation

- **Interface trait:** one trait method per IDL operation with `Result<T, CorbaException>` return; `&self` for state-free, `&mut self` when out/inout params; `attribute` → getter + setter.
- **Client stub:** `pub struct IStub { object_ref: ObjectReference }` with `impl I for IStub` — phase 1 returns `SystemException("not yet wired")`; phase 2 wires GIOP via `corba-iiop`.
- **Server skeleton:** `pub fn dispatch_<i>(servant: &dyn I, operation: &str, _payload: &[u8]) -> SkeletonResult` with operation-name switch — phase 1 returns `NotYetWired`; phase 2 decodes the payload + calls the servant.
- **Valuetype trait:** `pub trait V: ValueBase` with state-member getters (public state as `fn x(&self) -> T`, private as `fn _priv_x(&self) -> T`).

### Architecture

- **Layer:** 8 (CORBA stack).
- **Dependencies (in):** `zerodds-idl` (AST), `zerodds-idl-rust` (type mapping), `zerodds-corba-codegen` (special types + stub/skeleton templates).
- **Dependents (out):** end-user build scripts or a CLI tool that generates CORBA IDL files for Rust service implementations.

### Tests

- 6 snapshot tests in `tests/snapshot_codegen.rs` (simple interface, attribute, oneway, inout, valuetype, module-nested).

### Out of scope (phase 2+)

- **Component / Home / D&C deployment** — CCM servant bindings, then consuming `corba-ccm-lib`.
- **POA configuration builder** — `pub struct PoaBuilder` API for Activation, Lifespan, RequestProcessing policies.
- **GIOP wire wiring in the stub/skeleton** — phase 1 emits stubs/skeletons with a `NotYetWired` return; phase 2 replaces it with real GIOP marshalling over `corba-iiop`/`corba-giop`.
- **User-exception codegen** — IDL `exception E { ... };` with a dedicated error type per interface.
- **Repository-ID helper in the generated code** — currently not emitted; phase 2 adds `const REPOSITORY_ID: &str = "IDL:..."` to the trait.

### Stability

`1.0.0-rc.1` — RC phase. The `pub` items of the codegen entry API are stable; runtime types (`ObjectReference` / `CorbaException` / etc.) may receive phase-2 extensions (e.g. new `CorbaException` variants via `#[non_exhaustive]`).
