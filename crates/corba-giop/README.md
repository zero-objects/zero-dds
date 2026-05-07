# `zerodds-corba-giop`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-giop/badge.svg)](https://docs.rs/zerodds-corba-giop)

OMG CORBA 3.3 Part 2 §15 — General Inter-ORB Protocol (GIOP) Wire-
Codec. Voller Stack mit allen 8 Message-Types fuer GIOP 1.0, 1.1
und 1.2 (inkl. Bidirectional-GIOP). `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CORBA 3.3 Part 2 | §15 General Inter-ORB Protocol |
| OMG CORBA 3.3 Part 2 | §15.4.1-§15.4.9 (alle 8 Message-Types) |
| OMG CORBA 3.3 Part 2 | §15.5 Service-Context-Tags |

## Was ist drin

- **Header + Codec** (`encode_message` / `decode_message`,
  `MessageHeader`, `Version`, `Flags`, `MessageType`, `MAGIC`).
- **Alle 8 Message-Types** — Request/Reply/CancelRequest/
  LocateRequest/LocateReply/CloseConnection/MessageError/Fragment.
- **Reply-Statuses** (alle 6 — NO_EXCEPTION/USER_EXCEPTION/
  SYSTEM_EXCEPTION/LOCATION_FORWARD/LOCATION_FORWARD_PERM/
  NEEDS_ADDRESSING_MODE).
- **GIOP 1.2 TargetAddress-Union** + ObjectKey.
- **ServiceContext + List + Tags**.

## Was nicht abgedeckt ist

- Transport (TCP/UDS-Lieferung): liegt in `corba-iiop`.
- IOR-Format: liegt in `corba-ior`.
- POA Servant-Dispatch: liegt in `corba-poa`.

## Beispiel

```rust
use zerodds_corba_giop::{MAGIC_BYTES, Version};
assert_eq!(MAGIC_BYTES, *b"GIOP");
let v = Version { major: 1, minor: 2 };
```

## Tests

```bash
cargo test -p zerodds-corba-giop
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [`zerodds-cdr`](../cdr/README.md) — CDR-1-Marshalling.
