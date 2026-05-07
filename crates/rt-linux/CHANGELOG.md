# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-rt-linux`-Crate.

### Spec-Referenzen

- **Linux-Kernel-API** (keine OMG-Spec):
  - `sched(7)` — SCHED_OTHER/SCHED_FIFO/SCHED_RR/SCHED_DEADLINE.
  - `sched_setattr(2)` + `sched_getattr(2)` — `sched_attr`-Struktur.
  - `sched_setaffinity(2)` + `sched_getaffinity(2)` — `cpu_set_t`.

### Public-API

- `SchedulerProfile::{other, fifo, rr, deadline}` mit Validation.
- `SchedulerProfile::apply_to_current_thread`.
- `SchedulerKind::{Other, Fifo, Rr, Deadline, Batch, Idle}`.
- `RunningSchedulerInfo` + `current_scheduler()`.
- `pin_current_thread_to_cpus(&[u32]) -> io::Result<()>`.

### Implementierung

Drei Module:
- `affinity.rs` — `pin_current_thread_to_cpus` mit `cpu_set_t`-Builder.
- `scheduler.rs` — `SchedulerProfile`-Builder mit Priority-/Deadline-Validation.
- `syscalls.rs` — alle `unsafe { libc::syscall(...) }`-Calls liegen hier; jede Funktion ist eine duenne Wrapper-Schicht mit `// SAFETY:`-Kommentar pro Block.

`SCHED_DEADLINE` wird via `sched_setattr(SYS_sched_setattr, ...)` gesetzt — `nix` liefert das nicht, daher die direkte libc-syscall-Schicht. `SCHED_FIFO`/`SCHED_RR` koennten ueber `pthread_setschedparam` laufen, aber wir halten alle Pfade auf dem `sched_attr`-Codepath fuer Symmetrie.

Auf Nicht-Linux-Targets liefern alle Public-APIs `io::ErrorKind::Unsupported`. Der Workspace baut weiter auf macOS und Windows. `forbid(unsafe_code)` ist nicht gesetzt — diese Crate ist die explizite Ausnahme der COMFORT-Klassifikation (`docs/architecture/04_safety_by_architecture.md` §2.3).

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `libc` (target-gegated `cfg(target_os = "linux")`). Keine ZeroDDS-Crate-Deps.
- **Dependents (out):** End-User-Builds + DCPS-Hot-Path-Threads die RT-Profile brauchen.
- **Feature-Flags:** `std` (default).

### Stabilitaet

Public-API + Errno-Mapping sind RC1-stabil. `sched_setattr(2)` ist Linux-Kernel-stabile-API seit 3.14 (Maerz 2014).
