# zerodds-transport-shm

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-shm)](https://docs.rs/zerodds-transport-shm)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-shm)](https://crates.io/crates/zerodds-transport-shm)

ZeroDDS SHM transport: cross-process shared-memory transport.
Layer 2 (wire implementation).

`std`-only, safety class **STANDARD** (unsafe island in the `posix`
module for mmap access + libc flock FFI; the rest of the crate is
atomics-only).

## Spec status

OMG standardizes no SHM transport for DDS. Vendors each have their own
implementations (Cyclone+iceoryx, FastDDS SHM, RTI DDS SHM).
ZeroDDS defines its own variant explicitly as
**ZeroDDS SHM Transport 1.0**, documented in
[`docs/spec-coverage/zerodds-shm-transport-1.0.md`](../../docs/spec-coverage/zerodds-shm-transport-1.0.md).

DDSI-RTPS conformance: the locator kind is the DDSI-RTPS 2.5 §9.4
vendor-reserved value (in `crates/rtps/src/wire_types.rs`).

## What this crate provides

- `PosixShmTransport` — `Transport` trait impl via POSIX `shm_open` + `mmap`
- `ShmConfig` — segment configuration (capacity, flink_dir, …)
- `ShmRole` — owner / consumer
- `PosixShmError` — typed errors

## Architecture overview

| Aspect | Choice | Rationale |
|---|---|---|
| Sync model | SpSc per (owner, consumer) pair | Lock-free, linear scaling with reader count |
| Atomics | `AcqRel` on `head`/`tail` | Cross-process well-defined |
| Crash recovery | predictable `os_id` + `shm_unlink` before owner-create | Idempotent, prevents zombie segments |
| Race protection | advisory `flock(LOCK_EX)` (Linux/macOS) | Serializes parallel owner-creates |
| Owner termination | `shutdown` flag in the header (release store in Drop) | Clear owner-gone signal to the consumer |

Full details: [spec §2-§5](../../docs/spec-coverage/zerodds-shm-transport-1.0.md).

## Platform support

| Platform | Status |
|---|---|
| Linux | ✅ primary (test coverage) |
| macOS | ✅ supported (PSHMNAMLEN limit) |
| Windows | ⚠️ best-effort (compiles via the `shared_memory` crate; `flock`/`shm_unlink` no-op on non-Unix) |
| no_std | not supported (mmap needs an OS) |

## Tests

```bash
cargo test -p zerodds-transport-shm
```

18 tests green (17 lib + 1 cross-process integration).

## License

Apache-2.0 OR MIT — see the workspace root.
