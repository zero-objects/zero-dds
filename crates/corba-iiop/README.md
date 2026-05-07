# `zerodds-corba-iiop`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-iiop/badge.svg)](https://docs.rs/zerodds-corba-iiop)

OMG CORBA 3.3 Part 2 §14 + §15.7 + §15.9 — voller IIOP-TCP-Transport-
Stack: ProfileBody (alle 4 Versionen 1.0-1.3 inkl. TaggedComponents),
Connection / Connector / Acceptor mit thread-safer Connection-Reuse,
Bidirectional-GIOP. `no_std + alloc`, `forbid(unsafe_code)`.
Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CORBA 3.3 Part 2 | §14 IIOP Overview |
| OMG CORBA 3.3 Part 2 | §15.7 IIOP-Profile + ProfileBody |
| OMG CORBA 3.3 Part 2 | §15.9 Bidirectional-GIOP |

## Was ist drin

- **`IiopProfileBody`** + **`IiopVersion`** + **`TaggedComponent`**
  fuer alle 4 IIOP-Versionen.
- **`Connection`** — TCP-Stream-Wrapper, Frame-genau.
- **`Connector`** + **`ConnectorConfig`** — Client-Connect mit
  Connection-Reuse-Pool und Reconnect-Logik.
- **`Acceptor`** + **`AcceptorConfig`** — Server-Listener-Loop.
- **`framing::{read_giop_message, write_giop_message}`** — Codec
  ueber `corba-giop::Message`.
- **`BiDirIiopServiceContext`** + **`BiDirIiopListenPoint`** —
  Bidirectional-GIOP-Aushandlung.

## Was nicht abgedeckt ist

- TLS-Sicherung des TCP-Streams: separater Layer (`corba-csiv2` /
  `security-pki`).
- IOR-Aufbau/Stringification: gehoert in `corba-ior`.

## Beispiel

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

- [`zerodds-corba-giop`](../corba-giop/README.md) — GIOP Wire-Codec.
- [`zerodds-corba-ior`](../corba-ior/README.md) — IOR-Format mit
  IIOP-ProfileBody-Inhalt.
- [Architecture](../../docs/architecture/02_architecture.md)
