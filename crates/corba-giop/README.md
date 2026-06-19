# `zerodds-corba-giop`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-giop/badge.svg)](https://docs.rs/zerodds-corba-giop)

OMG CORBA 3.3 Part 2 §15 — General Inter-ORB Protocol (GIOP) wire
codec. Full stack with all 8 message types for GIOP 1.0, 1.1
and 1.2 (including bidirectional GIOP). `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 2 | §15 General Inter-ORB Protocol |
| OMG CORBA 3.3 Part 2 | §15.4.1-§15.4.9 (all 8 message types) |
| OMG CORBA 3.3 Part 2 | §15.5 service context tags |

## What's included

- **Header + codec** (`encode_message` / `decode_message`,
  `MessageHeader`, `Version`, `Flags`, `MessageType`, `MAGIC`).
- **All 8 message types** — Request/Reply/CancelRequest/
  LocateRequest/LocateReply/CloseConnection/MessageError/Fragment.
- **Reply statuses** (all 6 — NO_EXCEPTION/USER_EXCEPTION/
  SYSTEM_EXCEPTION/LOCATION_FORWARD/LOCATION_FORWARD_PERM/
  NEEDS_ADDRESSING_MODE).
- **GIOP 1.2 TargetAddress union** + ObjectKey.
- **ServiceContext + list + tags**.

## What's not covered

- Transport (TCP/UDS delivery): lives in `corba-iiop`.
- IOR format: lives in `corba-ior`.
- POA servant dispatch: lives in `corba-poa`.

## Example

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
