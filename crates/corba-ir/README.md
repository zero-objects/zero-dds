# `zerodds-corba-ir`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ir/badge.svg)](https://docs.rs/zerodds-corba-ir)

OMG CORBA 3.3 Part 1 §14 — Interface Repository (IR). TypeCode (all
32 TCKinds), Repository with containment hierarchy, DefinitionKind,
structured RepositoryId. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 1 | §10.7.3 (RepositoryId format), §14 (Interface Repository) |
| OMG CORBA 3.3 Part 1 | §3.13.1 (TypeCode operations) |

## What's included

- **`RepositoryId`** — `IDL:<scoped>:<major>.<minor>` parser/builder with
  roundtrip guarantee.
- **`TypeCode`** — all 32 OMG TCKinds (`tk_null` … `tk_local_interface`)
  with structured bodies for complex types (Struct/Union/Enum/Value/...).
- **`Repository`** + `Container` + `Definition` + `Module` — IR containment
  hierarchy per §14.
- **`DefinitionKind`** — `dk_*` constants per §14.

## What's not covered

- IIOP wire encoding of the IR operations: belongs in `corba-iiop` /
  `corba-giop`.
- TypeCode CDR encapsulation: lives in `zerodds-cdr` (OMG CDR §15.3.5.1
  TypeCode wire format).

## Example

```rust
use zerodds_corba_ir::{RepositoryId, TcKind, TypeCode};

let r = RepositoryId::parse("IDL:omg.org/CosNaming/NamingContext:1.0").unwrap();
assert_eq!(r.scoped_name, "omg.org/CosNaming/NamingContext");
assert_eq!(r.to_canonical(), "IDL:omg.org/CosNaming/NamingContext:1.0");
```

## Tests

```bash
cargo test -p zerodds-corba-ir
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [Components](../../documentation/02-architecture/components.md)
