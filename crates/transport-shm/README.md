# zerodds-transport-shm

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-shm)](https://docs.rs/zerodds-transport-shm)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-shm)](https://crates.io/crates/zerodds-transport-shm)

ZeroDDS SHM transport: cross-process shared-memory transport.
Layer 2 (wire implementation).

Safety class **STANDARD**. The ring (`ring`) is `core` alone; the
transport over a region an integrator gives (`region`) needs `alloc`; the
named segments of an operating system (`posix`, `bell`) need `std`.

## Spec status

OMG standardizes no SHM transport for DDS. Vendors each have their own
implementations (Cyclone+iceoryx, FastDDS SHM, RTI DDS SHM).
ZeroDDS defines its own variant explicitly as
**ZeroDDS SHM Transport 1.0**, documented in
[`docs/spec-coverage/zerodds-shm-transport-1.0.md`](../../docs/spec-coverage/zerodds-shm-transport-1.0.md).

DDSI-RTPS conformance: the locator kind is the DDSI-RTPS 2.5 §9.4
vendor-reserved value (in `crates/rtps/src/wire_types.rs`).

## What this crate provides

- `PosixShmTransport` (`std`) — `Transport` over a named segment: POSIX
  `shm_open` + `mmap` on Unix, an unnamed `CreateFileMapping` over a file
  on Windows
- `ShmConfig` — segment configuration (capacity, flink_dir, …)
- `ShmRole` — owner / consumer
- `PosixShmError` — typed errors
- `ring::Ring` (`core`) — the ring over a region, shared by both transports
- `RegionShmTransport`, `RegionConfig`, `RingWait`, `Parked`, `SpinWait` (`alloc`) —
  `Transport` over a region the integrator gives, without an operating
  system (two cores of one chip, two partitions of a hypervisor)

## Architecture overview

| Aspect | Choice | Rationale |
|---|---|---|
| Sync model | SpSc per (owner, consumer) pair | Lock-free, linear scaling with reader count |
| Atomics | `AcqRel` on `head`/`tail` | Cross-process well-defined |
| Crash recovery | predictable `os_id` + `shm_unlink` before owner-create | Idempotent, prevents zombie segments |
| Race protection | `flock(LOCK_EX)` on Unix, `LockFileEx` on Windows | Serializes parallel owner-creates |
| Without an OS | session word in the header, low-32-bit positions | A reset core's frames are not delivered; a core without 64-bit atomics takes part |
| Owner termination | `shutdown` flag in the header (release store in Drop) | Clear owner-gone signal to the consumer |

Full details: [spec §2-§5](../../docs/spec-coverage/zerodds-shm-transport-1.0.md).

## Platform support

| Platform | Status |
|---|---|
| Linux | ✅ primary (test coverage) |
| macOS | ✅ supported (PSHMNAMLEN limit) |
| other Unixes | the same protocol (pair directory `/tmp/zerodds-shm-<euid>`) |
| Windows | the same protocol (`LockFileEx`, a file per segment); weekly jobs `platform-windows-transport-shm` and `platform-windows-dcps-shm` |
| no_std | `RegionShmTransport` over a caller's region; run on two bare-metal cores under QEMU (`tools/shm-region-two-cores.sh`) and on a core without 64-bit atomics (`tools/shm-ring-armv5te.sh`) |

## Tests

```bash
cargo nextest run -p zerodds-transport-shm --all-targets
tools/shm-region-two-cores.sh   # two bare-metal cores under qemu-system-aarch64
tools/shm-ring-armv5te.sh       # ring and region tests without std, 32-bit atomics only, under qemu-arm
```

## License

Apache-2.0 OR MIT — see the workspace root.
