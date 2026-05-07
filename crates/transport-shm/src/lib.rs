// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-shm`. Safety classification: **STANDARD**.
//!
//! Cross-Process Shared-Memory-Transport für ZeroDDS — POSIX
//! `shm_open`/`mmap` mit Lock-free Single-Producer-Single-Consumer-
//! Ringbuffer.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5 §9.4** — Locator-Kind `LOCATOR_KIND_SHM` (vendor-
//!   reserved range; ZeroDDS-Wert in `crates/rtps/src/wire_types.rs`).
//! - **ZeroDDS-SHM-Transport 1.0** — vendor-spezifischer Transport-
//!   Spec (Segment-Layout, SpSc-Ringbuffer, Cleanup-Semantik),
//!   `docs/spec-coverage/zerodds-shm-transport-1.0.md`.
//!
//! ## Hinweis zur OMG-Normativität
//!
//! OMG normiert keinen SHM-Transport für DDS. Vendoren haben jeweils
//! eigene Implementationen: Cyclone DDS integriert iceoryx (third-
//! party), FastDDS hat einen eigenen SHM-Transport, RTI hat
//! RTI-Connext-DDS-SHM. ZeroDDS definiert seine eigene Variante
//! explizit als ZeroDDS-SHM-Transport-1.0-Spec.
//!
//! ## Implementiert (RC1)
//!
//! - POSIX `shm_open` + `mmap` via `shared_memory`-Crate.
//! - SpSc-Ringbuffer pro (Writer, Reader)-Paar — lock-free
//!   `head`/`tail`-AcqRel-Atomics auf shared memory.
//! - Owner-Crash-Recovery via predictable `os_id` + `shm_unlink` vor
//!   jedem Owner-`create()`. Idempotent.
//! - Advisory `flock`-Race-Protection beim Owner-Create (Linux/macOS).
//! - Length-Prefix-Frame-Format mit Wraparound-Padding-Marker.
//! - Shutdown-Flag im Segment-Header für Owner→Consumer-Termination-
//!   Signaling.
//!
//! ## Plattform-Support
//!
//! | Plattform | Status |
//! |---|---|
//! | Linux | ✅ primary |
//! | macOS | ✅ supported |
//! | Windows | nicht-getestet (`shared_memory`-Crate liefert die Mapping-Primitives, aber unsere `flock`-Race-Protection und `shm_unlink`-Cleanup sind unix-only) |
//!
//! ## Public API
//!
//! - [`PosixShmTransport`] — `Transport`-Trait-Impl.
//! - [`ShmConfig`] — Segment-Konfiguration.
//! - [`ShmRole`] — Owner / Consumer.
//! - [`PosixShmError`] — typisierte Fehler.
//!
//! ## Unsafe-Scope
//!
//! Begrenzt auf das `posix`-Modul: zwei `ptr::read`/`ptr::write` auf
//! mapped memory plus die `libc::flock`-FFI. Alles andere ist
//! `AtomicU{32,64}` über shared memory; Rust garantiert wohldefiniertes
//! Verhalten von Atomic-Operationen über Prozess-Grenzen, wenn beide
//! Prozesse dieselbe Adresse mappen.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
pub mod posix;

#[cfg(feature = "std")]
pub use posix::{PosixShmError, PosixShmTransport, ShmConfig, ShmRole};
