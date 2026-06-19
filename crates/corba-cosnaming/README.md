# `zerodds-corba-cosnaming`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-cosnaming/badge.svg)](https://docs.rs/zerodds-corba-cosnaming)

OMG CosNaming 1.3 (`formal/2004-10-03`) — full naming-service stack:
NamingContext + NamingContextExt in-memory implementation, all 5
exception classes, stringified name (§2.4), corbaname URL scheme (§2.5).
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec mapping

| Spec | Section |
|------|---------|
| OMG CosNaming 1.3 | §2.2 NamingContext, §2.3 NamingContextExt |
| OMG CosNaming 1.3 | §2.4 stringified name |
| OMG CosNaming 1.3 | §2.5 corbaname URL scheme |

## What's included

- **`Name`** + **`NameComponent`** with `id`/`kind` pairs.
- **`NamingContext`** — in-memory with Bind/Rebind/Resolve/Unbind/
  BindContext/NewContext/Destroy + ListBindings.
- **`Binding`** + **`BindingType`** (Object/Context).
- **`ObjectRef`** with an IOR payload from `corba-ior`.
- **5 exception classes**: NotFound (with `NotFoundReason`),
  CannotProceed, InvalidName, AlreadyBound, NotEmpty.
- **`name_to_string`** + **`string_to_name`** stringified codec.

## What's not covered

- Persistent naming storage: caller layer.
- Federation across multiple naming services: caller layer.

## Example

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

- [`zerodds-corba-ior`](../corba-ior/README.md) — object refs as
  IOR payload.
- [Architecture](../../docs/architecture/02_architecture.md)
