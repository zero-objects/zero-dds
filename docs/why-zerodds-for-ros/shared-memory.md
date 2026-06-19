# Shared Memory

← [Back to overview](index.md)

## The pain

Same-host transport should be the easy, fast case — two processes on one machine
exchanging data through shared memory. In DDS practice it is one of the largest
failure surfaces (**52 reports**): segfaults, `/dev/shm` exhaustion, cross-user
permission failures, mutex-timeout races on init, and the fixed-size-pool model
(Iceoryx) that does not fit variable-size robotics data.

- **Variable-size types deliver zero samples and pin a CPU core at 100 %** over
  the Iceoryx PSMX path — the fixed-pool model fights point clouds and images.
- Data-sharing readers can **loop forever** during shared-segment init.
- SHM files get the wrong permissions (`umask`), blocking cross-user access.
- Init races produce mutex-timeout failures and spurious "segment may be
  insufficient" warnings.

### Most recent example

**[rmw_cyclonedds#585 — "Variable-size types deliver zero samples over PSMX
(iceoryx) and pin a core at 100 % CPU"](https://github.com/ros2/rmw_cyclonedds/issues/585)**
(2026-06-02). The shared-memory fast path delivers *nothing* for variable-size
types while burning a full core — the exact mismatch between a fixed-size SHM
pool and variable-size robotics payloads.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-06-02 | [rmw_cyclonedds#585](https://github.com/ros2/rmw_cyclonedds/issues/585) | Variable-size types → 0 samples + 100 % CPU over iceoryx |
| 2026-03-21 | [Fast-DDS#6338](https://github.com/eProsima/Fast-DDS/issues/6338) | Data-sharing reader loops forever in segment init |
| 2025-12-02 | [Fast-DDS#6206](https://github.com/eProsima/Fast-DDS/issues/6206) | Spurious "segment_size may be insufficient" warning |
| 2025-11-10 | [Fast-DDS#6162](https://github.com/eProsima/Fast-DDS/issues/6162) | `umask` wrong on SHM files → cross-user access blocked |
| 2025-10-22 | [Fast-DDS#6117](https://github.com/eProsima/Fast-DDS/issues/6117) | SHM `init_port` mutex-timeout race |

## How ZeroDDS solves it

**A variable-size, length-prefixed SHM ring — and a safe core that cannot
segfault.**

- **Variable-size by design.** ZeroDDS's shared-memory transport is a
  length-prefixed ring, not a fixed-size pool. Variable-size payloads (point
  clouds, images) flow without a hand-dimensioned pool, so the
  [#585](https://github.com/ros2/rmw_cyclonedds/issues/585) "fixed pool delivers
  zero variable-size samples" failure does not arise. The single size knob,
  `ZERODDS_SHM_MAX_DATAGRAM`, sizes the ring; capacity tracks it automatically.
- **No segfault class.** The SHM path is built in Rust; the safe core is
  `forbid(unsafe_code)` and the small `unsafe` mmap/flock surface is isolated
  and audited. The buffer-overrun and use-after-free segfaults reported against
  C++ SHM transports are not expressible in the safe data path.
- **No busy-wait, no init-race livelock.** Wait paths are event-driven
  (condvar/notify), not spin loops, so the "loops forever pinning a core" and
  "mutex-timeout init race" failure modes are designed out.
- **Cross-process correctness.** Atomics over shared memory with well-defined
  cross-process semantics; a crash-recovery cleanup path handles a dead owner.

## Why it no longer has to be a pain

The SHM cluster is *fixed-size pools vs variable robotics data* plus *unsafe C++
plumbing that segfaults and live-locks*. ZeroDDS uses a variable-size ring and a
memory-safe implementation with event-driven waits — so same-host zero-copy is
the fast path it was supposed to be, for the payloads robotics actually sends.

## Reproduce it yourself

```bash
# Same-host SHM path with a large variable-size sample:
cargo test -p zerodds-transport-shm
# (and the largedata examples with the same-host-shm feature)
```

→ [Back to overview](index.md) · Next: [Scaling](scaling.md)
