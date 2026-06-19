# zerodds-transport

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport)](https://docs.rs/zerodds-transport)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport)](https://crates.io/crates/zerodds-transport)

Transport trait + Locator re-export for ZeroDDS. Layer 2 (wire trait crate).

Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`, safety class **SAFE**.

## What this crate provides

- `Transport` — trait for send/receive operations with Locator addressing
- `SendError` / `RecvError` — typed errors
- `ReceivedDatagram` — received datagram + source Locator
- `Locator` — re-exported from `zerodds-rtps::wire_types` (DDSI-RTPS 2.5 §8.3.2)

## Spec

- DDSI-RTPS 2.5 §8.3.2 — Locator definition
- The transport trait is ZeroDDS's own abstraction over RTPS wire protocols

## Concrete implementations

- `zerodds-transport-udp` — UDPv4/UDPv6 datagram sockets
- `zerodds-transport-tcp` — TCP stream + length-prefix framing
- `zerodds-transport-shm` — POSIX shared-memory ring buffer
- `zerodds-transport-uds` — Unix domain sockets
- `zerodds-transport-tsn` — TSN/IEEE 802.1Qbv time-aware shaper

## Architecture note: the `transport → rtps` crate dependency

`Locator` lives **deliberately** in `zerodds-rtps`, not in `zerodds-transport`:

- The DDSI-RTPS spec defines the locator's wire format in §8.3.2 — that is
  the RTPS domain, not the transport domain.
- `zerodds-transport` only re-exports it so that consumers have a
  transport-centric import-path option.
- There is no cycle: `zerodds-rtps` does **not** depend on
  `zerodds-transport`.

## Tests

```bash
cargo test -p zerodds-transport
cargo build -p zerodds-transport --no-default-features
cargo build -p zerodds-transport --no-default-features --features alloc
```

## License

Apache-2.0 OR MIT — see the workspace root.
