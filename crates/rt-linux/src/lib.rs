// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-rt-linux`. Safety classification: **COMFORT** (FFI boundary
//! by convention — see `docs/architecture/04_safety_by_architecture.md`
//! §2.3 COMFORT class).
//!
//! Linux real-time scheduling adapter — `sched_setattr` (FIFO/RR/DEADLINE)
//! + `sched_setaffinity` (CPU pinning) + `current_scheduler()`.
//!
//! ## Layer position
//!
//! Layer 4 — core services. Linux-specific FFI layer; on
//! non-Linux targets all public APIs are `io::Error::Unsupported`.
//!
//! # Why COMFORT instead of SAFE
//!
//! This crate is the only place in the ZeroDDS workspace where
//! `unsafe` syscalls reach the runtime path. The rest of the
//! workspace stays `forbid(unsafe_code)` / SAFE. Rationale:
//!
//! * Linux RT profiles (SCHED_FIFO, SCHED_RR, SCHED_DEADLINE) and
//!   CPU pinning are reachable **only** via syscalls.
//! * `nix` as a safe wrapper pulls in 12 transitive crates and
//!   provides no `sched_setattr`/`SCHED_DEADLINE` support.
//! * `libc` is the transitive standard dep — no additional
//!   footprint, the FFI definitions have been stable for years.
//!
//! # Architecture
//!
//! Three modules, each with clear invariants:
//!
//! * [`scheduler`] — `SchedulerProfile` enum + `apply_to_current_thread`.
//! * [`affinity`] — `pin_current_thread_to_cpus` with `cpu_set_t`.
//! * `syscalls` (private module) — all `unsafe { libc::... }` calls live here; each
//!   function is a thin, documented wrapper layer over
//!   a single Linux syscall, with a `// SAFETY:` comment per
//!   block.
//!
//! On non-Linux targets all public APIs return
//! `io::Error::Unsupported` and compile without libc — the workspace
//! still builds on macOS/Windows.
//!
//! # Privileges
//!
//! `SCHED_FIFO`/`SCHED_RR` with priority > 0 + `SCHED_DEADLINE` need
//! `CAP_SYS_NICE` (effective). Default-user tests get `EPERM`
//! back — the test suite treats this as "skipped" and does not claim
//! the path was verified.
//!
//! # Threat model + invariants
//!
//! All FFI calls in this crate hold the following invariants:
//!
//! 1. **No pointer outliving:** every pointer passed to a syscall
//!    points to a stack-local structure whose lifetime
//!    overlaps the syscall. No heap, no 'static aliasing.
//! 2. **No FD leak:** none of the syscalls used create file
//!    descriptors.
//! 3. **No memory aliasing:** buffers are exclusive (`&mut T`)
//!    during the call.
//! 4. **Errno-to-Result:** every syscall return value is lifted via
//!    `io::Error::last_os_error()` into a Rust `io::Result`;
//!    `errno` is read exactly once, before any further libc
//!    operation.
//! 5. **No mut-aliasing race:** all calls refer to the
//!    **own** thread (`tid = 0` is the Linux convention for the
//!    "calling thread"), not to other threads — there
//!    race conditions between user code and syscall would be conceivable.
//!
//! # Test strategy
//!
//! * All tests use `#[cfg(target_os = "linux")]`.
//! * Tests that produce `EPERM` (FIFO with prio>0, DEADLINE)
//!   check the errno path instead of forcing the setting.
//! * `current_scheduler()` is privilege-free and is verified via a
//!   round-trip test after `apply` (where permitted).
//!
//! Spec: no OMG spec — Linux kernel API. References:
//! `sched(7)`, `sched_setattr(2)`, `sched_setaffinity(2)`.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "std")]
extern crate std;

pub mod affinity;
pub mod scheduler;

#[cfg(target_os = "linux")]
mod syscalls;

pub use affinity::pin_current_thread_to_cpus;
pub use scheduler::{RunningSchedulerInfo, SchedulerKind, SchedulerProfile};
