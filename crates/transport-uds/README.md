# zerodds-transport-uds

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-uds)](https://docs.rs/zerodds-transport-uds)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-uds)](https://crates.io/crates/zerodds-transport-uds)

ZeroDDS UDS transport: container IPC via Unix domain sockets.
Layer 2 (wire implementation).

`std`-only, safety class **STANDARD** (on Unix: the receive hands an
initialised buffer to `socket2` as `[MaybeUninit<u8>]` to pass
`MSG_TRUNC`, `geteuid`, `fchmodat`; on Windows: the Win32 calls that read
the user SID and create a directory with an access control list).

## Spec status

OMG does not standardize a UDS transport for DDS. Cyclone DDS and FastDDS
have no official UDS transport (they use iceoryx/SHM for container IPC).
ZeroDDS defines its own variant explicitly as
**ZeroDDS UDS Transport 1.0**, documented in
[`docs/spec-coverage/zerodds-uds-transport-1.0.md`](../../docs/spec-coverage/zerodds-uds-transport-1.0.md).

DDSI-RTPS conformance: the locator kind `0x01F00002` lies in the range
DDSI-RTPS 2.5 §9.3.2.4 reserves for vendor-specific kinds (in
`crates/rtps/src/wire_types.rs`).

## What this crate provides

- `UdsTransport` — `Transport` trait impl via filesystem UDS, `SOCK_DGRAM`
  (Unix)
- `UdsStreamTransport` — the same over `SOCK_STREAM` with length-prefixed
  frames (§3.5): a connection begins with `ZUDS` and the sender's 16-byte
  id, each message is a little-endian `u32` length and its octets; the
  UDS transport on Windows (§6.2), an alternative on Unix
- `UdsConfig` — configuration (`base_dir`, `max_datagram`, `recv_timeout`,
  `send_timeout`, `socket_mode`, `base_dir_mode`)
- `socket_path` — path-resolution helper
- `abstract_dgram::UdsAbstractDgramTransport` with `AbstractDgramConfig` —
  Linux abstract-namespace variant, also with filesystem addressing

## Use case

Container IPC when:
- Multicast is blocked (cluster network policy)
- POSIX SHM is impractical cross-container (UID mapping, `/dev/shm` visibility, SELinux)

Docker/Kubernetes pattern: a mounted volume `/tmp/zerodds/uds` shared between
containers. Peers under different UIDs need a shared group, a directory
of that group with group write permission, the set-group-ID bit and the
sticky bit (`base_dir_mode` `0o3770`, or prepared so by the operator) and
socket files with group write permission (`socket_mode` `0o660`). The
set-group-ID bit gives each socket file the directory's group; the sticky
bit keeps one UID from removing another's socket file and putting its own
socket or a symlink at that path. The mode of a socket file is set only on
the socket the bind created, never through a symlink.

A send to a peer whose receive queue is full waits at most `send_timeout`
(default 100 ms) for room and then fails with "uds: kernel send buffer full
(EAGAIN)"; it does not hold the sending thread until the peer reads.

## Platform support

| Platform | Status |
|---|---|
| Linux | ✅ primary (datagram and stream over filesystem sockets, abstract namespace) |
| macOS | ✅ supported (datagram and stream over filesystem sockets, no abstract namespace) |
| Windows | stream over filesystem sockets (Windows has no `SOCK_DGRAM` for `AF_UNIX`); a created `base_dir` gets an access control list of the user and SYSTEM; `socket_mode` and other `base_dir_mode`s are refused. Compiled, not run on Windows yet |

## Tests

```bash
cargo nextest run -p zerodds-transport-uds --all-targets
```

The tests of `tests/l2_cross_uid.rs` start processes under other UIDs and
need root.

## License

Apache-2.0 OR MIT — see the workspace root.
