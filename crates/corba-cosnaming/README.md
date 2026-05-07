# `zerodds-corba-cosnaming`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-cosnaming/badge.svg)](https://docs.rs/zerodds-corba-cosnaming)

OMG CosNaming 1.3 (`formal/2004-10-03`) — voller Naming-Service-Stack:
NamingContext + NamingContextExt In-Memory-Implementation, alle 5
Exception-Klassen, Stringified-Name (§2.4), corbaname-URL-Scheme (§2.5).
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CosNaming 1.3 | §2.2 NamingContext, §2.3 NamingContextExt |
| OMG CosNaming 1.3 | §2.4 Stringified-Name |
| OMG CosNaming 1.3 | §2.5 corbaname-URL-Scheme |

## Was ist drin

- **`Name`** + **`NameComponent`** mit `id`/`kind`-Paaren.
- **`NamingContext`** — In-Memory mit Bind/Rebind/Resolve/Unbind/
  BindContext/NewContext/Destroy + ListBindings.
- **`Binding`** + **`BindingType`** (Object/Context).
- **`ObjectRef`** mit IOR-Inhalt aus `corba-ior`.
- **5 Exception-Klassen**: NotFound (mit `NotFoundReason`),
  CannotProceed, InvalidName, AlreadyBound, NotEmpty.
- **`name_to_string`** + **`string_to_name`** Stringified-Codec.

## Was nicht abgedeckt ist

- Persistente Naming-Storage: Caller-Layer.
- Federation zwischen mehreren Naming-Services: Caller-Layer.

## Beispiel

```rust
use zerodds_corba_cosnaming::NameComponent;
let nc = NameComponent { id: "obj".into(), kind: "Object".into() };
assert_eq!(nc.id, "obj");
assert_eq!(nc.kind, "Object");
```

## Tests

```bash
cargo test -p zerodds-corba-cosnaming
```

## See also

- [`zerodds-corba-ior`](../corba-ior/README.md) — Object-Refs als
  IOR-Inhalt.
- [Architecture](../../docs/architecture/02_architecture.md)
