# zerodds-transport

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport)](https://docs.rs/zerodds-transport)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport)](https://crates.io/crates/zerodds-transport)

Transport-Trait + Locator-Re-Export für ZeroDDS. Layer 2 (Wire-Trait-Crate).

Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`, Safety-Klasse **SAFE**.

## Was liefert dieses Crate

- `Transport` — Trait für send/receive-Operationen mit Locator-Adressierung
- `SendError` / `RecvError` — typisierte Fehler
- `ReceivedDatagram` — Empfangenes Datagramm + Source-Locator
- `Locator` — re-exportiert aus `zerodds-rtps::wire_types` (DDSI-RTPS 2.5 §8.3.2)

## Spec

- DDSI-RTPS 2.5 §8.3.2 — Locator-Definition
- Transport-Trait ist ZeroDDS-eigene Abstraktion über RTPS-Wire-Protokollen

## Konkrete Implementations

- `zerodds-transport-udp` — UDPv4/UDPv6 Datagram-Sockets
- `zerodds-transport-tcp` — TCP-Stream + Length-Prefix-Framing
- `zerodds-transport-shm` — POSIX Shared-Memory-Ringbuffer
- `zerodds-transport-uds` — Unix Domain Sockets
- `zerodds-transport-tsn` — TSN/IEEE 802.1Qbv Time-Aware Shaper

## Architektur-Hinweis: `transport → rtps` Crate-Dep

`Locator` lebt **bewusst** in `zerodds-rtps`, nicht in `zerodds-transport`:

- DDSI-RTPS-Spec definiert das Wire-Format des Locators in §8.3.2 — das ist
  RTPS-Domäne, nicht Transport-Domäne.
- `zerodds-transport` re-exportiert ihn nur, damit Konsumenten eine
  Transport-zentrische Import-Pfad-Option haben.
- Es gibt keinen Cycle: `zerodds-rtps` hängt **nicht** von
  `zerodds-transport` ab.

## Tests

```bash
cargo test -p zerodds-transport
cargo build -p zerodds-transport --no-default-features
cargo build -p zerodds-transport --no-default-features --features alloc
```

## Lizenz

Apache-2.0 OR MIT — siehe Workspace-Root.
