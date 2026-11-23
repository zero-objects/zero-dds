// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-shm`. Safety classification: **STANDARD**.
//!
//! Cross-process shared-memory transport for ZeroDDS — POSIX
//! `shm_open`/`mmap` with a lock-free single-producer-single-consumer
//! ring buffer.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §9.4** — locator kind `LOCATOR_KIND_SHM` (vendor-
//!   reserved range; the ZeroDDS value in `crates/rtps/src/wire_types.rs`).
//! - **ZeroDDS SHM Transport 1.0** — vendor-specific transport spec
//!   (segment layout, SpSc ring buffer, cleanup semantics),
//!   `docs/spec-coverage/zerodds-shm-transport-1.0.md`.
//!
//! ## Note on OMG normativity
//!
//! OMG standardizes no SHM transport for DDS. Vendors each have their
//! own implementations: Cyclone DDS integrates iceoryx (third-party),
//! FastDDS has its own SHM transport, RTI has RTI Connext DDS SHM.
//! ZeroDDS defines its own variant explicitly as the
//! ZeroDDS SHM Transport 1.0 spec.
//!
//! ## Implemented (RC1)
//!
//! - POSIX `shm_open` + `mmap` on Unix, `CreateFileMapping` without a
//!   name over a file per segment on Windows.
//! - SpSc ring buffer per (writer, reader) pair — lock-free
//!   `head`/`tail` AcqRel atomics on shared memory.
//! - Owner crash recovery via a predictable `os_id` + `shm_unlink`
//!   before every owner `create()` (on Windows the removal of the backing
//!   file). Idempotent.
//! - Pair lock and liveness lock: `flock` on Unix, `LockFileEx` on
//!   Windows; sweeps of what ended processes left on every platform.
//! - Length-prefix frame format with a wraparound padding marker.
//! - Shutdown flag in the segment header for owner→consumer
//!   termination signaling.
//! - Without `std` (§6.4): the ring ([`ring`]) over a region the integrator
//!   gives, and [`RegionShmTransport`] on it with a wait the integrator
//!   chooses ([`RingWait`]); a session rule takes the place of names and
//!   locks.
//!
//! ## Platform support
//!
//! | Platform | Status |
//! |---|---|
//! | Linux | ✅ primary |
//! | macOS | ✅ supported |
//! | other Unixes | the same protocol, the pair directory `/tmp/zerodds-shm-<euid>`, the wait over a FIFO |
//! | Windows | the same protocol over `LockFileEx` and a file per segment; CI job `platform-windows-transport-shm` |
//! | no operating system (`no_std` + `alloc`) | [`RegionShmTransport`] over a region the integrator gives; a target with 32-bit atomics with compare-and-swap |
//!
//! ## Public API
//!
//! - `PosixShmTransport` (with `std`) — `Transport` over a named segment.
//! - `ShmConfig` — segment configuration.
//! - [`ShmRole`] — owner / consumer.
//! - `PosixShmError` — typed errors.
//! - [`ring`] — the ring over a region, in `core`.
//! - [`RegionShmTransport`], [`RegionConfig`], [`RingWait`], [`Parked`],
//!   [`SpinWait`] (with `alloc`) — `Transport` over a caller's region.
//!
//! ## Unsafe scope
//!
//! Six modules hold `unsafe` code:
//! - `ring`: the header words and positions as atomics in the region, and
//!   the bounded copies in and out of its data region;
//! - `region`: the construction of a ring over the caller's region (the
//!   caller vouches for the region);
//! - `posix`: the probe of a segment's header in a sweep, `flock`,
//!   `geteuid`, `confstr` and `shm_unlink` on Unix, `LockFileEx`,
//!   `GetFileInformationByHandle` and `GetFileInformationByHandleEx` on
//!   Windows;
//! - `mapping`: `shm_open`, `ftruncate`, `mmap`, `munmap` and
//!   `shm_unlink` of the segments on Unix, `CreateFileMappingW`,
//!   `MapViewOfFile`, `UnmapViewOfFile` and `CloseHandle` on Windows;
//! - `wait`: the waits and wakes of §4.4, the `futex` syscall on Linux,
//!   `os_sync_wait_on_address_with_timeout`, `os_sync_wake_by_address_all`,
//!   `__ulock_wait` and `__ulock_wake` on macOS, looked up with `dlsym` and
//!   called through a `transmute`d pointer, `mkfifo`,
//!   `poll` and `uname` for the FIFO, `CreateEventW`, `SetEvent` and
//!   `WaitForSingleObject` on Windows;
//! - `bell`: the doorbell's words in its mapping and `Send`/`Sync` for the
//!   type that holds the mapping.
//!
//! Everything else is `AtomicU{32,64}` over shared memory; Rust guarantees
//! well-defined behavior of atomic operations across process boundaries
//! when both processes map the same address.

// Without `std` the ring and the transport over a caller's region remain
// (§6.4); the named segments need an operating system.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod ring;
pub use ring::ShmRole;

#[cfg(all(feature = "alloc", target_has_atomic = "32"))]
pub mod region;
#[cfg(all(feature = "alloc", target_has_atomic = "32"))]
pub use region::{Parked, RegionConfig, RegionShmError, RegionShmTransport, RingWait, SpinWait};

#[cfg(all(feature = "std", any(unix, windows)))]
pub mod posix;

#[cfg(all(feature = "std", any(unix, windows)))]
pub use posix::{PosixShmError, PosixShmTransport, ShmConfig};

#[cfg(all(feature = "std", any(unix, windows)))]
pub mod bell;
#[cfg(all(feature = "std", any(unix, windows)))]
mod mapping;
#[cfg(all(feature = "std", any(unix, windows)))]
mod wait;

#[cfg(all(feature = "std", any(unix, windows)))]
pub use bell::ShmDoorbell;
