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

## Interop measurements: foreign clients against `zerodds-naming-svc`

`zerodds-naming-svc` (feature `daemon`) answers the CORBA 3.4 Part 2
§13.6.10 `corbaloc::host:port/NameService` bootstrap URL like any other
ORB's naming service — the check is whether a foreign vendor's own
naming client can bind/list/resolve through it. As in the sibling
crate's README, ZeroDDS-side commands here are this crate's own
(`cargo build -p zerodds-corba-cosnaming --bin zerodds-naming-svc
--features daemon`, exercised by `tests/naming_svc_e2e.rs`); the
foreign-tool commands are sourced from each vendor's own docs (linked
below), not run against a real install from this repo, and anything
not confirmed this round says so.

```bash
zerodds-naming-svc --bind 0.0.0.0:2809
```

**omniORB — `nameclt`**

```bash
nameclt -ior corbaloc::<host>:2809/NameService list
nameclt -ior corbaloc::<host>:2809/NameService bind Echo::call <stringified-IOR>
nameclt -ior corbaloc::<host>:2809/NameService resolve Echo::call
```

`-ior` accepts a `corbaloc:`/`corbaname:` URL directly, not only a
stringified object reference. Source: [omniORB `nameclt(1)`](https://manpages.debian.org/experimental/omniorb/nameclt.1.en.html).

**TAO — `tao_nslist` / `tao_nsadd`**

```bash
tao_nslist -ORBInitRef NameService=corbaloc:iiop:<host>:2809/NameService
```

`-ORBInitRef NAME=URL` is the confirmed, spec-standard bootstrap flag
(also used by `tao_cosnaming` itself and any TAO client). `tao_nsadd`'s
own flags for adding one binding were not independently confirmed this
round — run `tao_nsadd -?`/`--help` on your TAO install before relying
on a specific flag shape. Source: [Wikibooks: ACE+TAO — Find a service on a naming server](https://en.wikibooks.org/wiki/ACE%2BTAO_Opensource_Programming_Notes/Find_a_service_on_a_naming_server).

**JacORB (`/opt/jacorb`) — `lsns` / `nmg`**

```bash
/opt/jacorb/bin/lsns -DORBInitRef.NameService=corbaloc::<host>:2809/NameService
/opt/jacorb/bin/nmg  -DORBInitRef.NameService=corbaloc::<host>:2809/NameService
```

`lsns` lists a naming context; `nmg` is JacORB's own naming-manager
GUI. `ORBInitRef.NameService` (set as a `-D` system property, JacORB
being Java-based) is confirmed from JacORB's own Naming-Service doc;
`lsns`'s own flag syntax was not found in what this round could fetch
— run `lsns --help`/`-h` first. Source: [JacORB `Nameservice.tex`](https://github.com/JacORB/JacORB/blob/master/doc/ProgrammingGuide/Naming/Nameservice.tex).

All three exercise the same wire format: NamingContext
bind/rebind/resolve/list over IIOP 1.2, the `corbaloc:`/`corbaname:`
bootstrap forms every vendor's own docs above name directly.

## See also

- [`zerodds-corba-ior`](../corba-ior/README.md) — object refs as
  IOR payload.
- [Architecture](../../docs/architecture/02_architecture.md)
