# `zerodds-rt-linux`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-rt-linux/badge.svg)](https://docs.rs/zerodds-rt-linux)

Linux real-time scheduling adapter for the
[ZeroDDS](https://zerodds.org) stack: `SCHED_FIFO`/`SCHED_RR`/
`SCHED_DEADLINE` profiles + CPU pinning. Safety classification:
**COMFORT** — the only place in the workspace where `unsafe` syscalls
reach the runtime path.

## Spec mapping

| Spec | Section |
|------|-----------|
| `sched(7)` Linux man page | SCHED_OTHER / SCHED_FIFO / SCHED_RR / SCHED_DEADLINE |
| `sched_setattr(2)` | `sched_attr` struct + syscall number |
| `sched_setaffinity(2)` | `cpu_set_t` |

No OMG spec — Linux kernel API.

## What's inside

- **`SchedulerProfile`** — builder for `SCHED_FIFO`/`SCHED_RR`/
  `SCHED_DEADLINE`/`SCHED_OTHER` profiles with validation.
- **`SchedulerKind`** — enum discriminants.
- **`apply_to_current_thread()`** — `sched_setattr` on `tid=0`.
- **`current_scheduler()`** — `sched_getattr` on `tid=0`, returns
  `RunningSchedulerInfo`.
- **`pin_current_thread_to_cpus(&[u32])`** — `sched_setaffinity` with a
  CPU set.

On non-Linux targets all public APIs return
`io::ErrorKind::Unsupported` — the workspace still builds on
macOS and Windows.

## Layer position

Layer 4 — core services. **No** ZeroDDS crate deps; `libc` is the
only external dep, target-gated `cfg(target_os = "linux")`.

## Quickstart

```rust,no_run
use zerodds_rt_linux::{SchedulerProfile, pin_current_thread_to_cpus};

// Real-time FIFO with priority 80, CPU pinning to cores 2+3:
let profile = SchedulerProfile::fifo(80).expect("priority valid");
profile.apply_to_current_thread().expect("CAP_SYS_NICE needed");
pin_current_thread_to_cpus(&[2, 3]).expect("affinity set");
```

## Privileges

`SCHED_FIFO`/`SCHED_RR` with priority > 0 + `SCHED_DEADLINE` need
`CAP_SYS_NICE` (effective). Default-user tests get `EPERM`
back — the test suite treats this as "skipped" and does not
claim the path is verified.

## Threat model + invariants

All FFI calls in this crate hold the following invariants:

1. **No pointer outliving** — stack-local structs, no heap.
2. **No FD leak** — none of the syscalls used create FDs.
3. **No memory aliasing** — buffers are exclusive during the call.
4. **Errno-to-Result** — `io::Error::last_os_error()` is read exactly once before any further libc operations.
5. **No mut-aliasing race** — all calls refer to `tid = 0`
   (the calling thread), not foreign threads.

Every `unsafe { libc::... }` block in `syscalls.rs` carries a
`// SAFETY:` comment with a block-specific rationale.

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::io::Error` + stack structs. |

Without `std` the crate builds as a no-op stub on Linux (all public APIs
return `Unsupported`).

## Stability

`1.0.0-rc.1`. Public API + errno mapping RC1-stable. The Linux kernel API
(`sched_setattr`/`sched_setaffinity`) has been stable since kernel 3.14.

## Tests

```bash
cargo test -p zerodds-rt-linux
```

7 tests — privilege-free paths + EPERM errno paths + round-trip
between `apply` and `current_scheduler`.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- `sched(7)`, `sched_setattr(2)`, `sched_setaffinity(2)` — Linux man pages.
- `docs/architecture/04_safety_by_architecture.md` §2.3 COMFORT class.
