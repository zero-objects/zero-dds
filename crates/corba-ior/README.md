# `zerodds-corba-ior`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ior/badge.svg)](https://docs.rs/zerodds-corba-ior)

OMG CORBA 3.3 Part 2 §13.6 — voller IOR-Stack: IOR-Struct, alle
Standard-Profile-Tags inkl. IIOP-ProfileBody, alle 32 Standard-
TaggedComponents inkl. strukturierter Decoder (ORB_TYPE / CODE_SETS /
ALTERNATE_IIOP_ADDRESS / SSL_SEC_TRANS / TLS_SEC_TRANS /
RMI_CUSTOM_MAX_STREAM_FORMAT / JAVA_CODEBASE / CSI_SEC_MECH_LIST),
stringified-IOR (`IOR:hex`), `corbaloc:` und `corbaname:`-URL-Parser.
`no_std + alloc`, `forbid(unsafe_code)`.
Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CORBA 3.3 Part 2 | §13.6 Object References |
| OMG CORBA 3.3 Part 2 | §13.6.7.1 Profile-Tags, §13.6.7.3 Components |
| OMG CORBA 3.3 Part 2 | §13.6.10 Stringified-IOR + corbaloc/corbaname |
| OMG CORBA 3.3 Part 2 | §15.7.2 IIOP-ProfileBody (Inhalt von TAG_INTERNET_IOP) |
| OMG CORBA 3.3 Part 2 | §10 CSIv2 (TAG_CSI_SEC_MECH_LIST) |

## Was ist drin

- **`Ior`** + **`TaggedProfile`** + **`ProfileId`**.
- **32 Standard-Component-Tags** in `ComponentId`.
- **Strukturierte Decoder** in `StructuredComponent`:
  `OrbType` / `CodeSetComponentInfo` / `AlternateIiopAddress` /
  `Ssl` / `TlsSecTrans` / `StreamFormatVersion` / `CsiSecMechList`
  (Cross-Crate-Wire-up zu `corba-csiv2`).
- **Stringified-IOR** (`IOR:hex`) bidirektional.
- **`corbaloc:`** + **`corbaname:`**-URL-Parser mit Multi-Address.

## Was nicht abgedeckt ist

- IOR-Resolver (Network-Lookup): Caller-Layer.
- ORB-Vendor-spezifische TaggedComponents jenseits der 32 Standards:
  als opaque `TaggedComponent::Other` durchgereicht.

## Beispiel

```rust
use zerodds_corba_ior::{Ior, ProfileId};
let ior = Ior::default();
assert!(ior.profiles.is_empty());
assert_eq!(ProfileId::InternetIop.as_u32(), 0);
```

## Tests

```bash
cargo test -p zerodds-corba-ior
```

## See also

- [`zerodds-corba-iiop`](../corba-iiop/README.md) — IIOP-ProfileBody.
- [`zerodds-corba-csiv2`](../corba-csiv2/README.md) —
  CompoundSecMechList-Inhalt.
- [`zerodds-corba-cosnaming`](../corba-cosnaming/README.md) —
  IOR-basierte Object-Refs.
- [Architecture](../../docs/architecture/02_architecture.md)
