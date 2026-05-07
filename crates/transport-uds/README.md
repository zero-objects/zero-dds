# zerodds-transport-uds

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-uds)](https://docs.rs/zerodds-transport-uds)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-uds)](https://crates.io/crates/zerodds-transport-uds)

ZeroDDS-UDS-Transport: Container-IPC via Unix Domain Sockets.
Layer 2 (Wire-Implementation).

`std`-only, Safety-Klasse **STANDARD** (Unsafe-Island im
`abstract_dgram`-Modul für libc-FFI; Default-DGRAM-Modul ist safe-only).

## Spec-Status

OMG normiert keinen UDS-Transport für DDS. Cyclone DDS und FastDDS
haben keinen offiziellen UDS-Transport (nutzen iceoryx/SHM für
Container-IPC). ZeroDDS definiert seine eigene Variante explizit als
**ZeroDDS-UDS-Transport 1.0**, dokumentiert in
[`docs/spec-coverage/zerodds-uds-transport-1.0.md`](../../docs/spec-coverage/zerodds-uds-transport-1.0.md).

DDSI-RTPS-Konformität: Locator-Kind ist DDSI-RTPS 2.5 §9.4-vendor-
reservierter Wert `0x81000001` (in `crates/rtps/src/wire_types.rs`).

## Was liefert dieses Crate

- `UdsTransport` — `Transport`-Trait-Impl via Filesystem-UDS
- `UdsConfig` — Konfiguration (base_dir, max_datagram, recv_timeout)
- `socket_path` — Path-Resolution-Helper
- `abstract_dgram::AbstractDgramSocket` — Linux Abstract-Namespace-Variante

## Use Case

Container-IPC, wenn:
- Multicast geblockt (Cluster-Network-Policy)
- POSIX-SHM cross-Container unpraktisch (UID-Mapping, `/dev/shm`-Sichtbarkeit, SELinux)

Docker/Kubernetes-Pattern: gemountetes Volume `/tmp/zerodds/uds` zwischen
Containern.

## Plattform-Support

| Plattform | Status |
|---|---|
| Linux | ✅ primary (Filesystem + Abstract Namespace) |
| macOS | ✅ supported (Filesystem only, kein Abstract Namespace) |
| Windows | ❌ nicht supported (Unix-spezifisch) |

## Tests

```bash
cargo test -p zerodds-transport-uds
```

17 Tests grün (16 lib + 1 cross-process integration).

## Lizenz

Apache-2.0 OR MIT — siehe Workspace-Root.
