// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-rt-linux`. Safety classification: **COMFORT** (FFI-Boundary
//! per Konvention — siehe `docs/architecture/04_safety_by_architecture.md`
//! §2.3 COMFORT-Klasse).
//!
//! Linux Real-Time-Scheduling Adapter — `sched_setattr` (FIFO/RR/DEADLINE)
//! + `sched_setaffinity` (CPU-Pinning) + `current_scheduler()`.
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Linux-spezifische FFI-Schicht; auf
//! Nicht-Linux-Targets sind alle Public-APIs `io::Error::Unsupported`.
//!
//! # Warum COMFORT statt SAFE
//!
//! Diese Crate ist die einzige Stelle im ZeroDDS-Workspace, an der
//! `unsafe`-Syscalls in den Runtime-Pfad gelangen. Der Rest des
//! Workspaces bleibt `forbid(unsafe_code)` bzw. SAFE. Begruendung:
//!
//! * Linux-RT-Profile (SCHED_FIFO, SCHED_RR, SCHED_DEADLINE) und
//!   CPU-Pinning sind **nur** ueber Syscalls erreichbar.
//! * `nix` als sicherer Wrapper traegt 12 transitive Crates und
//!   liefert keinen `sched_setattr`/`SCHED_DEADLINE`-Support.
//! * `libc` ist die transitive Standard-Dep — kein zusaetzlicher
//!   Footprint, FFI-Definitionen sind seit Jahren stabil.
//!
//! # Architektur
//!
//! Drei Module, jeweils mit klaren Invarianten:
//!
//! * [`scheduler`] — `SchedulerProfile`-Enum + `apply_to_current_thread`.
//! * [`affinity`] — `pin_current_thread_to_cpus` mit `cpu_set_t`.
//! * `syscalls` (privates Modul) — alle `unsafe { libc::... }`-Calls liegen hier; jede
//!   Funktion ist eine duenne, dokumentierte Wrapper-Schicht ueber
//!   einem einzelnen Linux-Syscall, mit `// SAFETY:`-Kommentar pro
//!   Block.
//!
//! Auf Nicht-Linux-Targets liefern alle Public-APIs
//! `io::Error::Unsupported` und kompilieren ohne libc — der Workspace
//! baut weiter auf macOS/Windows.
//!
//! # Privilegien
//!
//! `SCHED_FIFO`/`SCHED_RR` mit Priority > 0 + `SCHED_DEADLINE` brauchen
//! `CAP_SYS_NICE` (effective). Default-User-Tests bekommen `EPERM`
//! zurueck — die Test-Suite handelt das als "skipped" und behauptet
//! nicht, der Pfad sei verifiziert.
//!
//! # Threat-Model + Invarianten
//!
//! Alle FFI-Calls in diesem Crate halten folgende Invarianten ein:
//!
//! 1. **Kein Pointer-Outliving:** jeder an einen Syscall uebergebene
//!    Pointer zeigt auf eine Stack-lokale Struktur, deren Lebensdauer
//!    den Syscall ueberlappt. Kein Heap, kein 'static-Aliasing.
//! 2. **Kein FD-Leak:** keiner der genutzten Syscalls erzeugt File-
//!    Descriptors.
//! 3. **Kein Memory-Aliasing:** Buffer sind exklusiv (`&mut T`)
//!    waehrend des Aufrufs.
//! 4. **Errno-zu-Result:** jeder Syscall-Returnwert wird per
//!    `io::Error::last_os_error()` in ein Rust-`io::Result` gehoben;
//!    `errno` wird genau einmal gelesen, vor jeder weiteren libc-
//!    Operation.
//! 5. **Keine Mut-Aliasing-Race:** alle Calls beziehen sich auf den
//!    **eigenen** Thread (`tid = 0` ist Linux-Konvention fuer
//!    "calling thread"), nicht auf andere Threads — dort waeren
//!    Race-Conditions zwischen User-Code und Syscall denkbar.
//!
//! # Test-Strategie
//!
//! * Alle Tests nutzen `#[cfg(target_os = "linux")]`.
//! * Tests, die `EPERM` produzieren (FIFO mit prio>0, DEADLINE),
//!   pruefen den Errno-Pfad statt das Setting zu erzwingen.
//! * `current_scheduler()` ist privilegienfrei und wird per
//!   Round-Trip-Test nach `apply` verifiziert (sofern erlaubt).
//!
//! Spec: keine OMG-Spec — Linux-Kernel-API. References:
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
