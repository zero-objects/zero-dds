# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-rt-linux` crate.

### Spec references

- **Linux kernel API** (no OMG spec):
  - `sched(7)` — SCHED_OTHER/SCHED_FIFO/SCHED_RR/SCHED_DEADLINE.
  - `sched_setattr(2)` + `sched_getattr(2)` — `sched_attr` struct.
  - `sched_setaffinity(2)` + `sched_getaffinity(2)` — `cpu_set_t`.

### Public API

- `SchedulerProfile::{other, fifo, rr, deadline}` with validation.
- `SchedulerProfile::apply_to_current_thread`.
- `SchedulerKind::{Other, Fifo, Rr, Deadline, Batch, Idle}`.
- `RunningSchedulerInfo` + `current_scheduler()`.
- `pin_current_thread_to_cpus(&[u32]) -> io::Result<()>`.

### Implementation

Three modules:
- `affinity.rs` — `pin_current_thread_to_cpus` with a `cpu_set_t` builder.
- `scheduler.rs` — `SchedulerProfile` builder with priority/deadline validation.
- `syscalls.rs` — all `unsafe { libc::syscall(...) }` calls live here; each function is a thin wrapper layer with a `// SAFETY:` comment per block.

`SCHED_DEADLINE` is set via `sched_setattr(SYS_sched_setattr, ...)` — `nix` does not provide it, hence the direct libc syscall layer. `SCHED_FIFO`/`SCHED_RR` could run via `pthread_setschedparam`, but we keep all paths on the `sched_attr` code path for symmetry.

On non-Linux targets all public APIs return `io::ErrorKind::Unsupported`. The workspace still builds on macOS and Windows. `forbid(unsafe_code)` is not set — this crate is the explicit exception of the COMFORT classification (`docs/architecture/04_safety_by_architecture.md` §2.3).

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** `libc` (target-gated `cfg(target_os = "linux")`). No ZeroDDS crate deps.
- **Dependents (out):** end-user builds + DCPS hot-path threads that need RT profiles.
- **Feature flags:** `std` (default).

### Stability

The public API + errno mapping are RC1-stable. `sched_setattr(2)` is a stable Linux kernel API since 3.14 (March 2014).
