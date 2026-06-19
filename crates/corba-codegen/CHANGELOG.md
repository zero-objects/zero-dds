# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-codegen` crate.

### Spec references

- **OMG CORBA 3.3 Part 1**: Annex A (IDL type mappings), §10.7.3.1 (repository ID format).
- **OMG IDL-to-C++** (`formal/2008-01-09`).
- **OMG IDL-to-Java** (`formal/2008-01-04`).

### Public API

**Special types (`special_types` module):**
- `SpecialType::{Object, ValueBase, AbstractBase, NativeRef, TypeCode, Any, SequenceOfAny, String, WString, Time, Fixed, ULongLong, LongDouble}` (all 13 Annex-A.1 special types).
- `TargetLanguage::{Cpp, CSharp, Java}`.
- `language_mapping(special, lang) -> &'static str`.

**Repository ID (`repository_id` module):**
- `build_repository_id(modules: &[&str], type_name: &str, major: u16, minor: u16) -> String`.

**Stub template (`stub` module):**
- `StubOp { operation_name, return_type, parameters, raises }`.
- `render_stub_op(op) -> String` — code skeleton for the client stub.

**Skeleton template (`skeleton` module):**
- `SkeletonOp` (server-side dispatch operation).
- `render_skeleton_dispatch(ops) -> String` — operation-name switch.

### Implementation

`special_types::language_mapping` is a 13×3 lookup table that statically contains all 39 mappings (e.g. `(SpecialType::Object, TargetLanguage::Cpp) → "CORBA::Object_var"`, `(SpecialType::Time, TargetLanguage::Java) → "org.omg.TimeBase.UtcT"`).

`build_repository_id` assembles the Spec §10.7.3.1 form: `IDL:<module>/.../<type-name>:<major>.<minor>`.

`render_stub_op` and `render_skeleton_dispatch` are string-based code templates consumed by the three OMG PSM crates (idl-cpp / idl-csharp / idl-java).

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc` (feature `alloc`); `#![forbid(unsafe_code)]`.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-A).
- **Dependencies (in):** none (substrate crate).
- **Dependents (out):** `zerodds-idl-cpp`, `zerodds-idl-csharp`, `zerodds-idl-java`.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Annex-A.1 mapping tables: fixed by the OMG spec.
- Stub/skeleton templates: stable; new template methods are major-additive.
