# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-ir`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 1**: §10.7.3 (Repository-Identification), §14 (Interface Repository).
- **OMG CORBA 3.3 Part 1**: §3.13.1 (TypeCode-Operations).

### Public-API

**`RepositoryId`-Modul:**
- `RepositoryId { scoped_name, major, minor }` — strukturierte Form.
- `RepositoryId::parse(s) -> IrResult<Self>` — `IDL:<scoped>:<m>.<n>`-Parser.
- `RepositoryId::to_canonical() -> String`.

**`TypeCode`-Modul:**
- `TcKind` — alle 32 OMG-TCKinds (`tk_null` … `tk_local_interface`).
- `TypeCode { kind, body }` mit `TypeCodeBody`-Varianten.
- `UnionMember`, `StructMember`, `ValueMember`.

**`Repository`-Modul:**
- `Repository`, `Container`, `Definition`, `Module` — IR-Containment-Hierarchie.

**`DefinitionKind`-Modul:**
- `DefinitionKind::{None, Attribute, Constant, Exception, Interface, Module, Operation, Typedef, Alias, Struct, Union, Enum, Primitive, String, Sequence, Array, Repository, Wstring, Fixed, Value, ValueBox, ValueMember, Native, AbstractInterface, LocalInterface}` — `dk_*`-Konstanten.

**`error`-Modul:**
- `IrError::{InvalidRepositoryId, NotFound, AlreadyDefined, ContainmentViolation, BadKind}`.
- `IrResult<T>`.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc` (Feature `alloc`); `#![forbid(unsafe_code)]`.

Die Crate ist substrat — keine Workspace-Deps. `RepositoryId::parse` validiert das Spec-§10.7.3.1-Format streng (Prefix `IDL:`, Trenner `:`, Versions-Format `<u16>.<u16>`).

`TypeCode` modelliert alle 32 OMG-TCKinds; komplexe Bodies (`Struct`, `Union`, `Enum`, `Sequence`, `Array`, `Value`, `Alias`) carrying ihre Member-Listen direkt im Enum-Body, Spec-treu zu §3.13.1-Operations.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** keine.
- **Dependents (out):** `zerodds-corba-poa` (RepositoryId-Parse fuer Servant `_is_a`-Validation).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- TypeCode-Bodies: durch OMG-Spec fixiert; Erweiterung waere Major-Bump.
- RepositoryId-Format: Spec-§10.7.3.1, fixiert.
