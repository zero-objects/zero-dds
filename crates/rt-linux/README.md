# `zerodds-rt-linux`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-rt-linux/badge.svg)](https://docs.rs/zerodds-rt-linux)

Linux Real-Time-Scheduling Adapter fuer den
[ZeroDDS](https://zerodds.org)-Stack: `SCHED_FIFO`/`SCHED_RR`/
`SCHED_DEADLINE`-Profile + CPU-Pinning. Safety classification:
**COMFORT** — die einzige Stelle im Workspace, wo `unsafe`-Syscalls
in den Runtime-Pfad gelangen.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| `sched(7)` Linux-Manpage | SCHED_OTHER / SCHED_FIFO / SCHED_RR / SCHED_DEADLINE |
| `sched_setattr(2)` | `sched_attr`-Struktur + Syscall-Number |
| `sched_setaffinity(2)` | `cpu_set_t` |

Keine OMG-Spec — Linux-Kernel-API.

## Was ist drin

- **`SchedulerProfile`** — Builder fuer `SCHED_FIFO`/`SCHED_RR`/
  `SCHED_DEADLINE`/`SCHED_OTHER`-Profile mit Validation.
- **`SchedulerKind`** — Enum-Diskriminanten.
- **`apply_to_current_thread()`** — `sched_setattr` auf `tid=0`.
- **`current_scheduler()`** — `sched_getattr` auf `tid=0`, liefert
  `RunningSchedulerInfo`.
- **`pin_current_thread_to_cpus(&[u32])`** — `sched_setaffinity` mit
  CPU-Set.

Auf Nicht-Linux-Targets liefern alle Public-APIs
`io::ErrorKind::Unsupported` zurueck — der Workspace baut weiter auf
macOS und Windows.

## Schichten-Position

Layer 4 — Core Services. **Keine** ZeroDDS-Crate-Deps; `libc` ist die
einzige externe Dep, target-gegated `cfg(target_os = "linux")`.

## Quickstart

```rust,no_run
use zerodds_rt_linux::{SchedulerProfile, pin_current_thread_to_cpus};

// Real-Time-FIFO mit Priority 80, CPU-Pinning auf Core 2+3:
let profile = SchedulerProfile::fifo(80).expect("priority valid");
profile.apply_to_current_thread().expect("CAP_SYS_NICE noetig");
pin_current_thread_to_cpus(&[2, 3]).expect("affinity set");
```

## Privilegien

`SCHED_FIFO`/`SCHED_RR` mit Priority > 0 + `SCHED_DEADLINE` brauchen
`CAP_SYS_NICE` (effective). Default-User-Tests bekommen `EPERM`
zurueck — die Test-Suite handelt das als "skipped" und behauptet
nicht, der Pfad sei verifiziert.

## Threat-Model + Invarianten

Alle FFI-Calls in diesem Crate halten folgende Invarianten ein:

1. **Kein Pointer-Outliving** — Stack-lokale Strukturen, kein Heap.
2. **Kein FD-Leak** — keiner der genutzten Syscalls erzeugt FDs.
3. **Kein Memory-Aliasing** — Buffer sind exklusiv waehrend des Aufrufs.
4. **Errno-zu-Result** — `io::Error::last_os_error()` wird genau einmal vor weiteren libc-Operationen gelesen.
5. **Keine Mut-Aliasing-Race** — alle Calls beziehen sich auf `tid = 0`
   (calling thread), nicht auf Fremd-Threads.

Jeder `unsafe { libc::... }`-Block in `syscalls.rs` traegt einen
`// SAFETY:`-Kommentar mit Block-genauer Begruendung.

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::io::Error` + Stack-Strukturen. |

Ohne `std` baut die Crate als no-op-Stub auf Linux (alle Public-APIs
liefern `Unsupported`).

## Stabilitaet

`1.0.0-rc.1`. Public-API + Errno-Mapping RC1-stabil. Linux-Kernel-API
(`sched_setattr`/`sched_setaffinity`) ist seit Kernel 3.14 stabil.

## Tests

```bash
cargo test -p zerodds-rt-linux
```

7 Tests — privilegienfreie Pfade + EPERM-Errno-Pfade + Round-Trip
zwischen `apply` und `current_scheduler`.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- `sched(7)`, `sched_setattr(2)`, `sched_setaffinity(2)` — Linux-Manpages.
- `docs/architecture/04_safety_by_architecture.md` §2.3 COMFORT-Klasse.
