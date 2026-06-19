# ZeroDDS-SHM-Transport 1.0 — Spec Coverage

A ZeroDDS-vendor-specific shared-memory transport. Analogous to Cyclone's
iceoryx integration, FastDDS SHM and RTI DDS SHM. **Not OMG-normative.**
Implemented in:

- `crates/transport-shm/` — segment layout + SPSC ring buffer + crash recovery (`posix.rs`)

The vendor-reserved locator-kind value (§9.4) is a constant in
`crates/rtps/src/wire_types.rs`.

| Spec family | Status |
|---|---|
| **OMG-normative** | DDSI-RTPS 2.5 §9.4 LocatorKind (vendor-reserved range) — locator value in `crates/rtps/src/wire_types.rs` |
| **ZeroDDS-own spec** | segment layout + SPSC ring buffer + crash recovery — [`zerodds-shm-transport-1.0.md`](https://github.com/zero-objects/zero-dds/blob/main/docs/spec-coverage/zerodds-shm-transport-1.0.md) |

## §1 Scope and spec status

### §1.1 What OMG standardizes

For SHM, DDSI-RTPS 2.5 standardizes only:

- **§9.4 LocatorKind**: vendor-reserved values for non-IP transports. The
  ZeroDDS value is in `crates/rtps/src/wire_types.rs`.

No normative wire format, no normative segment layout, no normative cleanup
protocol.

### §1.2 What OMG does not standardize

- **Segment layout**: vendor-specific.
- **Synchronization model** (SPSC / SPMC / mutex): vendor-specific.
- **Crash recovery**: vendor-specific.
- **Cleanup mechanics**: vendor-specific.
- **Multi-reader distribution**: vendor-specific.

### §1.3 ZeroDDS choice

The ZeroDDS SHM transport defines its **own spec** (this file) for segment
layout + synchronization. The spec is:

- **OMG-conformant** at the locator level (§9.4 vendor-reserved value).
- **Vendor-specific** at the wire/synchronization level (analogous to
  iceoryx/FastDDS-SHM/RTI).

## §2 Segment layout

```text
  offset 0:   magic: u32 BE   "ZSHM"  (0x5A53484D)
  offset 4:   version: u32 LE
  offset 8:   capacity: u64 LE         (data region, excluding header)
  offset 16:  head: AtomicU64          (next write offset, writer-owned)
  offset 24:  tail: AtomicU64          (next read offset, reader-owned)
  offset 32:  shutdown: AtomicU32      (0=active, 1=owner-gone)
  offset 36:  reserved (padding to a 64-byte cache line)
  offset 64:  data region [capacity bytes]
```

- **Magic** `"ZSHM"` — a version discriminator; rejects foreign segments.
- **Version**: currently `1`. Bumped on a layout change; openers reject
  unknown versions.
- **Head/tail**: `AcqRel` atomics for lock-free single-producer
  single-consumer synchronization.
- **Shutdown**: an owner→consumer termination signal (the owner sets it to
  `1` in `Drop`).

Repo anchors: `crates/transport-shm/src/posix.rs::HEADER_BYTES`,
`SHM_MAGIC`, `SHM_VERSION`.

## §3 Frame format

A length-prefix format inside the data region:

```text
+---------+---------+---------+---------+----- ...
| len: u32 LE                            | bytes [len]
+---------+---------+---------+---------+----- ...
```

- `len = 0xFFFF_FFFE` marks a **padding frame** (a ring-end marker). The
  writer inserts it when there is not enough contiguous space at the ring
  end; it then jumps to the start.
- `len < capacity` is a data frame.

Repo anchors: `posix.rs::PADDING_FRAME_LEN`,
`posix.rs::SegmentLayout::push_frame`,
`posix.rs::SegmentLayout::pop_frame`.

## §4 Synchronization model

### §4.1 Single-producer single-consumer

One segment per `(owner, consumer)` pair — not one segment per owner with a
multi-reader fan-out.

**Rationale**:
- Lock-free SPSC scales linearly with the reader count, with no global
  contention.
- A `pthread_mutex` with `PTHREAD_PROCESS_SHARED` would be the alternative,
  but it is crash-recovery-fragile.
- SPMC (like iceoryx) blocks the writer on the slowest reader — bad with
  heterogeneous readers.

**Cost**: N segments for N readers. With 100 readers × 1 MiB default
= 100 MiB. Acceptable; the per-pair segment size is configurable.

### §4.2 Memory ordering

- Writer: `Release` store on `head` after the frame write.
- Reader: `Acquire` load of `head` before the frame read.
- Guaranteed: the writer-side frame bytes are visible as soon as the reader
  sees the new `head` value.

Repo anchors: `posix.rs::SegmentLayout::head` / `SegmentLayout::tail`.

## §5 Cleanup semantics

### §5.1 Predictable os_id

`segment_os_id(owner, consumer)` returns a deterministic segment name
(`/zd-<owner>-<consumer>` or `/zd-<owner-tail15>-<consumer-tail15>` on the
macOS PSHMNAMLEN).

Repo anchor: `posix.rs::segment_os_id`.

### §5.2 Crash recovery

Before every owner `create()`, `shm_unlink(os_id)` is called. This:
- reclaims zombie segments of a crashed owner.
- is idempotent (`ENOENT` is ignored).
- prevents a system-wide `/dev/shm` leak.

Repo anchor: `posix.rs::shm_unlink_by_os_id`.

### §5.3 Shutdown flag

The owner sets `shutdown = 1` in `Drop` (Release store). The consumer checks
the flag in `wait_for_frame` after every empty poll and returns with a
targeted error (`Io{message:"shm owner terminated"}`) instead of falling
blindly into `recv_timeout`.

Repo anchors: `posix.rs::SegmentLayout::set_shutdown`,
`posix.rs::SegmentLayout::is_shutdown`.

### §5.4 Race protection on owner create

An exclusive whole-file lock on a sentinel file serializes parallel owner
creates on the same `os_id` — across threads AND processes. Linux/macOS via
`flock(LOCK_EX)`, Windows via `LockFileEx` (`LOCKFILE_EXCLUSIVE_LOCK`,
blocking). Both auto-release on handle close / process death (identical
crash-resilience).

Repo anchors: `posix.rs::acquire_flock_excl`, `posix.rs::FlockGuard`.

## §6 Platform support

| Platform | Status | Notes |
|---|---|---|
| Linux | ✅ primary | full test coverage |
| macOS | ✅ supported | PSHMNAMLEN limit observed |
| Windows | ✅ supported | zero-copy SHM via `shared_memory` (`CreateFileMapping`); owner-create race via `LockFileEx`; cleanup via OS handle reference counting. Test suite green on Windows (19/19, incl. `open_concurrent_two_threads_both_bound`) |
| no_std | not supported | std-only (mmap needs OS calls) |

## §7 Test coverage

| Spec section | Tests |
|---|---|
| §2 segment layout | `posix.rs::tests::magic_and_layout_*` |
| §3 frame format | `posix.rs::tests::push_pop_*`, `padding_*` |
| §4 SPSC synchronization | `posix.rs::tests::concurrent_*` |
| §5.2 crash recovery | `posix.rs::tests::recovers_zombie_segment` |
| §5.3 shutdown flag | `posix.rs::tests::owner_drop_signals_consumer` |
| §5.4 race protection | `posix.rs::tests::flock_*` |
| §6 cross-process | `tests/l1_cross_process.rs` |

Total: 19 lib + 1 integration = 20 tests. All green on Linux, macOS and
Windows (`cargo test -p zerodds-transport-shm`).

## §8 Status

**Fully covered.** The ZeroDDS SHM transport is a complete, internally
coherent spec; all § sections are implemented and tested. Platform support:
Linux (primary), macOS and Windows — all three with a green test suite.
