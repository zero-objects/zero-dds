# zerodds-transport-uds

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-uds)](https://docs.rs/zerodds-transport-uds)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-uds)](https://crates.io/crates/zerodds-transport-uds)

ZeroDDS UDS transport: container IPC via Unix domain sockets.
Layer 2 (wire implementation).

`std`-only, safety class **STANDARD** (unsafe island in the
`abstract_dgram` module for libc FFI; the default DGRAM module is safe-only).

## Spec status

OMG does not standardize a UDS transport for DDS. Cyclone DDS and FastDDS
have no official UDS transport (they use iceoryx/SHM for container IPC).
ZeroDDS defines its own variant explicitly as
**ZeroDDS UDS Transport 1.0**, documented in
[`docs/spec-coverage/zerodds-uds-transport-1.0.md`](../../docs/spec-coverage/zerodds-uds-transport-1.0.md).

DDSI-RTPS conformance: the locator kind is the DDSI-RTPS 2.5 §9.4 vendor-
reserved value `0x81000001` (in `crates/rtps/src/wire_types.rs`).

## What this crate provides

- `UdsTransport` — `Transport` trait impl via filesystem UDS
- `UdsConfig` — configuration (base_dir, max_datagram, recv_timeout)
- `socket_path` — path-resolution helper
- `abstract_dgram::AbstractDgramSocket` — Linux abstract-namespace variant

## Use case

Container IPC when:
- Multicast is blocked (cluster network policy)
- POSIX SHM is impractical cross-container (UID mapping, `/dev/shm` visibility, SELinux)

Docker/Kubernetes pattern: a mounted volume `/tmp/zerodds/uds` shared between
containers.

## Platform support

| Platform | Status |
|---|---|
| Linux | ✅ primary (filesystem + abstract namespace) |
| macOS | ✅ supported (filesystem only, no abstract namespace) |
| Windows | ❌ not supported (Unix-specific) |

## Tests

```bash
cargo test -p zerodds-transport-uds
```

17 tests green (16 lib + 1 cross-process integration).

## License

Apache-2.0 OR MIT — see the workspace root.
