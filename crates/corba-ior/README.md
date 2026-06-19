# `zerodds-corba-ior`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ior/badge.svg)](https://docs.rs/zerodds-corba-ior)

OMG CORBA 3.3 Part 2 §13.6 — full IOR stack: IOR struct, all
standard profile tags including IIOP ProfileBody, all 32 standard
TaggedComponents including structured decoders (ORB_TYPE / CODE_SETS /
ALTERNATE_IIOP_ADDRESS / SSL_SEC_TRANS / TLS_SEC_TRANS /
RMI_CUSTOM_MAX_STREAM_FORMAT / JAVA_CODEBASE / CSI_SEC_MECH_LIST),
stringified IOR (`IOR:hex`), `corbaloc:` and `corbaname:` URL parsers.
`no_std + alloc`, `forbid(unsafe_code)`.
Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 2 | §13.6 Object References |
| OMG CORBA 3.3 Part 2 | §13.6.7.1 profile tags, §13.6.7.3 components |
| OMG CORBA 3.3 Part 2 | §13.6.10 stringified IOR + corbaloc/corbaname |
| OMG CORBA 3.3 Part 2 | §15.7.2 IIOP ProfileBody (content of TAG_INTERNET_IOP) |
| OMG CORBA 3.3 Part 2 | §10 CSIv2 (TAG_CSI_SEC_MECH_LIST) |

## What's included

- **`Ior`** + **`TaggedProfile`** + **`ProfileId`**.
- **32 standard component tags** in `ComponentId`.
- **Structured decoders** in `StructuredComponent`:
  `OrbType` / `CodeSetComponentInfo` / `AlternateIiopAddress` /
  `Ssl` / `TlsSecTrans` / `StreamFormatVersion` / `CsiSecMechList`
  (cross-crate wire-up to `corba-csiv2`).
- **Stringified IOR** (`IOR:hex`) bidirectional.
- **`corbaloc:`** + **`corbaname:`** URL parsers with multi-address.

## What's not covered

- IOR resolver (network lookup): caller layer.
- ORB-vendor-specific TaggedComponents beyond the 32 standards:
  passed through as opaque `TaggedComponent::Other`.

## Example

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

- [`zerodds-corba-iiop`](../corba-iiop/README.md) — IIOP ProfileBody.
- [`zerodds-corba-csiv2`](../corba-csiv2/README.md) —
  CompoundSecMechList content.
- [`zerodds-corba-cosnaming`](../corba-cosnaming/README.md) —
  IOR-based object refs.
- [Architecture](../../docs/architecture/02_architecture.md)
