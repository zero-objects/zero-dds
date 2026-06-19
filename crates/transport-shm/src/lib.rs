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
//! - POSIX `shm_open` + `mmap` via the `shared_memory` crate.
//! - SpSc ring buffer per (writer, reader) pair — lock-free
//!   `head`/`tail` AcqRel atomics on shared memory.
//! - Owner crash recovery via a predictable `os_id` + `shm_unlink`
//!   before every owner `create()`. Idempotent.
//! - Advisory `flock` race protection on owner-create (Linux/macOS).
//! - Length-prefix frame format with a wraparound padding marker.
//! - Shutdown flag in the segment header for owner→consumer
//!   termination signaling.
//!
//! ## Platform support
//!
//! | Platform | Status |
//! |---|---|
//! | Linux | ✅ primary |
//! | macOS | ✅ supported |
//! | Windows | untested (the `shared_memory` crate provides the mapping primitives, but our `flock` race protection and `shm_unlink` cleanup are unix-only) |
//!
//! ## Public API
//!
//! - [`PosixShmTransport`] — `Transport` trait impl.
//! - [`ShmConfig`] — segment configuration.
//! - [`ShmRole`] — owner / consumer.
//! - [`PosixShmError`] — typed errors.
//!
//! ## Unsafe scope
//!
//! Limited to the `posix` module: two `ptr::read`/`ptr::write` on
//! mapped memory plus the `libc::flock` FFI. Everything else is
//! `AtomicU{32,64}` over shared memory; Rust guarantees well-defined
//! behavior of atomic operations across process boundaries when both
//! processes map the same address.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
pub mod posix;

#[cfg(feature = "std")]
pub use posix::{PosixShmError, PosixShmTransport, ShmConfig, ShmRole};
