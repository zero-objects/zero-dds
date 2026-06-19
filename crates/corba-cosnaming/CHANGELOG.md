# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the `zerodds-corba-cosnaming` crate.

### Spec references

- **OMG CosNaming 1.3** (`formal/2004-10-03`): NamingContext (§2.2),
  NamingContextExt (§2.3), stringified name (§2.4),
  corbaname URL scheme (§2.5).
- **CORBA 3.3 Part 2 §13.6.10** — corbaname URL as an IOR resolver substitute.

### Public API

- `name::{Name, NameComponent}` — name sequence with `id`/`kind` pairs.
- `context::{Binding, BindingType, NamingContext, ObjectRef}` —
  in-memory NamingContext impl with Bind/Rebind/Resolve/Unbind/
  BindContext/NewContext/Destroy + ListBindings.
- `error::{NamingError, NotFoundReason}` — all 5 exception classes
  (`NotFound`, `CannotProceed`, `InvalidName`, `AlreadyBound`,
  `NotEmpty`).
- `stringified::{name_to_string, string_to_name}` — spec §2.4
  stringified-name format.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`NamingContext` is an in-memory `BTreeMap`-based object with
parent tracking for iterative resolves.

`name_to_string` escapes `/`, `.` and `\` per §2.4.

The `ObjectRef` variant carries an `Ior` payload from
`zerodds-corba-ior` (cross-crate wire-up).

### Architecture

- **Layer:** 8 (CORBA stack, Tier C).
- **Dependencies (in):** `zerodds-cdr`, `zerodds-corba-ior`.
- **Dependents (out):** hosting applications + naming service servers.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format and stringified-name format fixed by OMG.
