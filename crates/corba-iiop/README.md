# `zerodds-corba-iiop`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-iiop/badge.svg)](https://docs.rs/zerodds-corba-iiop)

OMG CORBA 3.3 Part 2 §14 + §15.7 + §15.9 — full IIOP-over-TCP transport
stack: ProfileBody (all 4 versions 1.0-1.3 incl. TaggedComponents),
Connection / Connector / Acceptor with thread-safe connection reuse,
bidirectional GIOP. `no_std + alloc`, `forbid(unsafe_code)`.
Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 2 | §14 IIOP Overview |
| OMG CORBA 3.3 Part 2 | §15.7 IIOP profile + ProfileBody |
| OMG CORBA 3.3 Part 2 | §15.9 Bidirectional GIOP |

## What's included

- **`IiopProfileBody`** + **`IiopVersion`** + **`TaggedComponent`**
  for all 4 IIOP versions.
- **`Connection`** — TCP stream wrapper, frame-exact.
- **`Connector`** + **`ConnectorConfig`** — client connect with a
  connection-reuse pool and reconnect logic.
- **`Acceptor`** + **`AcceptorConfig`** — server listener loop.
- **`framing::{read_giop_message, write_giop_message}`** — codec
  over `corba-giop::Message`.
- **`BiDirIiopServiceContext`** + **`BiDirIiopListenPoint`** —
  bidirectional-GIOP negotiation.

## What's not covered

- TLS protection of the TCP stream: separate layer (`corba-csiv2` /
  `security-pki`).
- IOR construction/stringification: belongs in `corba-ior`.

## Example

```rust
use zerodds_corba_iiop::IiopVersion;
assert_eq!(IiopVersion::V1_2.major, 1);
assert_eq!(IiopVersion::V1_2.minor, 2);
```

## Tests

```bash
cargo test -p zerodds-corba-iiop
```

## See also

- [`zerodds-corba-giop`](../corba-giop/README.md) — GIOP wire codec.
- [`zerodds-corba-ior`](../corba-ior/README.md) — IOR format with
  IIOP-ProfileBody content.
- [Architecture](../../docs/architecture/02_architecture.md)
