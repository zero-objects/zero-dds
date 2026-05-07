# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung der `zerodds-corba-cosnaming`-Crate.

### Spec-Referenzen

- **OMG CosNaming 1.3** (`formal/2004-10-03`): NamingContext (§2.2),
  NamingContextExt (§2.3), Stringified-Name (§2.4),
  corbaname-URL-Scheme (§2.5).
- **CORBA 3.3 Part 2 §13.6.10** — corbaname-URL als IOR-Resolver-Ersatz.

### Public-API

- `name::{Name, NameComponent}` — Name-Sequenz mit `id`/`kind`-Paaren.
- `context::{Binding, BindingType, NamingContext, ObjectRef}` —
  NamingContext-In-Memory-Impl mit Bind/Rebind/Resolve/Unbind/
  BindContext/NewContext/Destroy + ListBindings.
- `error::{NamingError, NotFoundReason}` — alle 5 Exception-Klassen
  (`NotFound`, `CannotProceed`, `InvalidName`, `AlreadyBound`,
  `NotEmpty`).
- `stringified::{name_to_string, string_to_name}` — Spec §2.4
  Stringified-Name-Format.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`NamingContext` ist ein In-Memory-`BTreeMap`-basiertes Objekt mit
parent-Tracking fuer iterative Resolves.

`name_to_string` escaped `/`, `.` und `\` gemaess §2.4.

`ObjectRef`-Variante traegt einen `Ior`-Inhalt aus
`zerodds-corba-ior` (Cross-Crate-Wire-up).

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-C).
- **Dependencies (in):** `zerodds-cdr`, `zerodds-corba-ior`.
- **Dependents (out):** Hosting-Anwendungen + Naming-Service-Server.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format und Stringified-Name-Format durch OMG fixiert.
