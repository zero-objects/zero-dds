# ZeroDDS-UDS-Transport 1.0 — Spec Coverage

A ZeroDDS-vendor-specific Unix-domain-socket transport for container IPC.
**Not OMG-normative** — Cyclone DDS and FastDDS have no official UDS
transport. Implemented in:

- `crates/transport-uds/` — path resolution + SOCK_DGRAM format + abstract namespace + cleanup (`lib.rs`, `abstract_dgram.rs`)

The vendor-reserved locator-kind value (§9.4) is a constant in
`crates/rtps/src/wire_types.rs`.

| Spec family | Status |
|---|---|
| **OMG-normative** | DDSI-RTPS 2.5 §9.4 LocatorKind (vendor-reserved) — locator value in `crates/rtps/src/wire_types.rs` |
| **ZeroDDS-own spec** | path resolution + SOCK_DGRAM format + abstract namespace — [`zerodds-uds-transport-1.0.md`](https://github.com/zero-objects/zero-dds/blob/main/docs/spec-coverage/zerodds-uds-transport-1.0.md) |

## §1 Scope and spec status

### §1.1 What OMG standardizes

DDSI-RTPS 2.5 §9.4 allows vendor-reserved locator kinds. ZeroDDS allocates
`LOCATOR_KIND_UDS = 0x81000001` (a vendor-specific value).

No normative wire format, no path schema, no cleanup protocol.

### §1.2 ZeroDDS choice

An own spec for:
- filesystem path resolution.
- the SOCK_DGRAM wire format.
- the Linux abstract-namespace variant.
- cleanup semantics.

## §2 Path resolution

### §2.1 Filesystem mode (default)

16-byte `Locator` address → path:

```text
<base_dir>/<lowercase-hex32>.sock
```

- Default `base_dir` = `/tmp/zerodds/uds`.
- `<lowercase-hex32>` is the 16-byte ID as 32 hex characters.
- The base directory is created lazily on bind with mode `0700`
  (user-private).

Repo anchors: `lib.rs::socket_path`, `lib.rs::DEFAULT_BASE_DIR`.

### §2.2 Abstract-namespace mode (Linux-only)

An alternative on Linux: sockets in the abstract namespace (no filesystem
inode). Path:

```text
\0zd-<lowercase-hex32>
```

(A leading null byte signals the abstract namespace on Linux.)

- Advantages: no file cleanup needed, no inode permissions.
- Trade-off: Linux only, no cross-mount volume.

Repo anchor: `abstract_dgram.rs::AbstractDgramSocket`.

## §3 Wire format

`SOCK_DGRAM` over UDS — one datagram message per RTPS frame. The kernel
preserves message boundaries (unlike a TCP stream).

- Default `max_datagram` = 65,536 bytes.
- Kernel limit: Linux `wmem_max` (default 212,992) is the upper bound;
  ZeroDDS stays below it.

Repo anchors: `lib.rs::DEFAULT_MAX_DATAGRAM`, `lib.rs::UdsTransport::recv`.

## §4 Cleanup semantics

### §4.1 Filesystem mode

- Bind creates the socket file on demand.
- Drop removes the socket file via `fs::remove_file` (best-effort).
- Crash: a zombie socket remains; the next bind detects it via
  `path.exists()` and fails with `AlreadyInUse` (TOCTOU-safe: probe first,
  then fail fast — no auto-cleanup, because auto-cleanup could be a
  cross-process race).

### §4.2 Abstract mode

No cleanup needed — Linux reclaims abstract sockets automatically when the
last FD is closed.

## §5 Container use case

### §5.1 Docker/Kubernetes pattern

A mounted volume `/tmp/zerodds/uds` between two containers; each container
binds its own 16-byte locator as a socket file. The kernel routes between
the FDs.

### §5.2 When to use UDS instead of SHM

- Multicast blocked (cluster network policy).
- POSIX SHM impractical cross-container (UID mapping, `/dev/shm`
  visibility, SELinux profiles).
- A volume mount is the realistic permission boundary.

## §6 Cross-vendor interop

**Not intended.** UDS is intra-container/intra-host IPC. Cross-vendor
interop with Cyclone/FastDDS stays in the UDP/TCP/SHM domain.

## §7 Platform support

| Platform | Status | Notes |
|---|---|---|
| Linux | ✅ primary | filesystem + abstract namespace |
| macOS | ✅ supported | filesystem mode only (no abstract namespace) |
| Windows | ❌ not supported | UDS is Unix-specific (Windows named pipes would be the analogue) |

## §8 Test coverage

| Spec section | Tests |
|---|---|
| §2.1 filesystem path | `lib.rs::tests::socket_path_*` |
| §2.2 abstract namespace | `abstract_dgram.rs::tests::abstract_*` |
| §3 SOCK_DGRAM | `lib.rs::tests::send_recv_*` |
| §4 cleanup | `lib.rs::tests::cleanup_*`, `bind_existing_path_*` |
| §5 cross-process | `tests/l1_cross_process.rs` |

Total: 16 lib + 1 cross-process = 17 tests green.

## §9 Status

**Fully covered.** The ZeroDDS UDS transport is a complete, internally
coherent spec; all § sections are implemented and tested.
