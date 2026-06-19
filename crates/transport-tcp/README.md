# zerodds-transport-tcp

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-tcp)](https://docs.rs/zerodds-transport-tcp)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-tcp)](https://crates.io/crates/zerodds-transport-tcp)

ZeroDDS TCP transport: RTPS-over-TCP implementation. Layer 2 (wire implementation).

`std`-only, `forbid(unsafe_code)`, safety class **STANDARD**.

## Spec status

This transport is **OMG-conformant at the wire-mapping level**:

- **DDSI-RTPS 2.5 §9.4** — locator kinds `TCPv4` (4) / `TCPv6` (8)
- **DDSI-RTPS 2.5 §9.5** — wire-bytes mapping (RTPS header + submessages,
  identical to the UDP PSM)

OMG standardizes **no** TCP connection bring-up handshake. Vendors
each have their own formats (Cyclone: no handshake; FastDDS:
0x71/0x72 submessages; RTI: TLS-oriented).

ZeroDDS defines its own handshake explicitly as its own spec:
**ZeroDDS TCP Transport 1.0**, documented in
[`docs/spec-coverage/zerodds-tcp-transport-1.0.md`](../../docs/spec-coverage/zerodds-tcp-transport-1.0.md).

## What this crate provides

- `TcpTransport` — `Transport` trait implementation with a connection pool
- `TcpTransport::without_handshake` — Cyclone `ddsi_tcp` compat mode
- `TcpTransportError` — typed errors
- `framing` — length-prefix frame encoder/decoder (§2.1)
- `handshake` — BindConnection request/response (§3.1+§3.2)

## Cross-vendor interop

| Peer | Status |
|---|---|
| ZeroDDS ↔ ZeroDDS | ✅ full (handshake + RTPS frames) |
| ZeroDDS ↔ Cyclone | ✅ via `without_handshake` (raw RTPS frames) |
| ZeroDDS ↔ FastDDS | optional extension point (vendor-specific handshake) |
| ZeroDDS ↔ RTI | optional extension point (TLS handshake) |

Cross-vendor extensions are documented as optional in the
ZeroDDS TCP Transport 1.0 spec §6 — no spec gap, since OMG
standardizes no TCP handshake.

## Tests

```bash
cargo test -p zerodds-transport-tcp
```

55 tests green (50 lib + 5 integration); for the covered spec sections see
[`zerodds-tcp-transport-1.0.md §7`](../../docs/spec-coverage/zerodds-tcp-transport-1.0.md).

## License

Apache-2.0 OR MIT — see the workspace root.
