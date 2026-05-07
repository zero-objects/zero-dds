# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-codegen`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 1**: Annex A (IDL-Type-Mappings), §10.7.3.1 (Repository-ID-Format).
- **OMG IDL-to-C++** (`formal/2008-01-09`).
- **OMG IDL-to-Java** (`formal/2008-01-04`).

### Public-API

**Special-Types (`special_types`-Modul):**
- `SpecialType::{Object, ValueBase, AbstractBase, NativeRef, TypeCode, Any, SequenceOfAny, String, WString, Time, Fixed, ULongLong, LongDouble}` (alle 13 Annex-A.1 Special-Types).
- `TargetLanguage::{Cpp, CSharp, Java}`.
- `language_mapping(special, lang) -> &'static str`.

**Repository-ID (`repository_id`-Modul):**
- `build_repository_id(modules: &[&str], type_name: &str, major: u16, minor: u16) -> String`.

**Stub-Template (`stub`-Modul):**
- `StubOp { operation_name, return_type, parameters, raises }`.
- `render_stub_op(op) -> String` — Code-Skeleton fuer Client-Stub.

**Skeleton-Template (`skeleton`-Modul):**
- `SkeletonOp` (Server-Side-Dispatch-Operation).
- `render_skeleton_dispatch(ops) -> String` — Operation-Name-Switch.

### Implementierung

`special_types::language_mapping` ist eine 13×3-Lookup-Tabelle, die statisch alle 39 Mappings enthaelt (z.B. `(SpecialType::Object, TargetLanguage::Cpp) → "CORBA::Object_var"`, `(SpecialType::Time, TargetLanguage::Java) → "org.omg.TimeBase.UtcT"`).

`build_repository_id` setzt die Spec-§10.7.3.1-Form zusammen: `IDL:<module>/.../<type-name>:<major>.<minor>`.

`render_stub_op` und `render_skeleton_dispatch` sind String-basierte Code-Templates, die von den drei OMG-PSM-Crates (idl-cpp / idl-csharp / idl-java) konsumiert werden.

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc` (Feature `alloc`); `#![forbid(unsafe_code)]`.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** keine (Substrat-Crate).
- **Dependents (out):** `zerodds-idl-cpp`, `zerodds-idl-csharp`, `zerodds-idl-java`.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Annex-A.1-Mapping-Tabellen: durch OMG-Spec fixiert.
- Stub/Skeleton-Templates: stable; neue Template-Methoden sind Major-additive.
