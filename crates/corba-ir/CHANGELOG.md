# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-ir` crate.

### Spec references

- **OMG CORBA 3.3 Part 1**: §10.7.3 (repository identification), §14 (Interface Repository).
- **OMG CORBA 3.3 Part 1**: §3.13.1 (TypeCode operations).

### Public API

**`RepositoryId` module:**
- `RepositoryId { scoped_name, major, minor }` — structured form.
- `RepositoryId::parse(s) -> IrResult<Self>` — `IDL:<scoped>:<m>.<n>` parser.
- `RepositoryId::to_canonical() -> String`.

**`TypeCode` module:**
- `TcKind` — all 32 OMG TCKinds (`tk_null` … `tk_local_interface`).
- `TypeCode { kind, body }` with `TypeCodeBody` variants.
- `UnionMember`, `StructMember`, `ValueMember`.

**`Repository` module:**
- `Repository`, `Container`, `Definition`, `Module` — IR containment hierarchy.

**`DefinitionKind` module:**
- `DefinitionKind::{None, Attribute, Constant, Exception, Interface, Module, Operation, Typedef, Alias, Struct, Union, Enum, Primitive, String, Sequence, Array, Repository, Wstring, Fixed, Value, ValueBox, ValueMember, Native, AbstractInterface, LocalInterface}` — `dk_*` constants.

**`error` module:**
- `IrError::{InvalidRepositoryId, NotFound, AlreadyDefined, ContainmentViolation, BadKind}`.
- `IrResult<T>`.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc` (feature `alloc`); `#![forbid(unsafe_code)]`.

The crate is substrate — no workspace deps. `RepositoryId::parse` validates the spec §10.7.3.1 format strictly (prefix `IDL:`, separator `:`, version format `<u16>.<u16>`).

`TypeCode` models all 32 OMG TCKinds; complex bodies (`Struct`, `Union`, `Enum`, `Sequence`, `Array`, `Value`, `Alias`) carry their member lists directly in the enum body, faithful to the §3.13.1 operations.

### Architecture

- **Layer:** 8 (CORBA stack, tier A).
- **Dependencies (in):** none.
- **Dependents (out):** `zerodds-corba-poa` (RepositoryId parse for servant `_is_a` validation).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- TypeCode bodies: fixed by the OMG spec; extension would be a major bump.
- RepositoryId format: spec §10.7.3.1, fixed.
