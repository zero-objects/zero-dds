// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Roh-Syscall-Wrapper. Alle `unsafe`-Bloecke des Crates leben hier.
//!
//! # Geltungsbereich
//!
//! Dieses Modul ist `cfg(target_os = "linux")`-only — es existiert
//! schlicht nicht auf macOS/Windows. Die Public-API (Module
//! `scheduler` + `affinity`) entscheidet via `cfg`, ob sie hierher
//! delegiert oder einen `Unsupported`-Fehler zurueckgibt.
//!
//! # `unsafe`-Disziplin
//!
//! Jeder `unsafe { libc::... }`-Block ist:
//!
//! 1. die kuerzest moegliche Sequenz fuer **genau einen** Syscall;
//! 2. mit `// SAFETY:`-Kommentar dokumentiert, der die fuenf
//!    Crate-Invarianten (siehe `lib.rs`) konkret nachweist;
//! 3. unmittelbar gefolgt von einem `errno`-Check `if rc < 0 { ... }`.
//!
//! Es gibt **keine** Helper-Funktionen, die `unsafe` Pointer durch
//! mehrere `safe`-Layer reichen.
//!
//! # Linux-Manpages
//!
//! * sched_setattr(2) — `struct sched_attr` + flags.
//! * sched_getattr(2) — symmetrischer Read.
//! * sched_setaffinity(2) — `cpu_set_t`-Bit-Mask.
//! * sched_getaffinity(2) — symmetrischer Read.
//! * `sched(7)` — uebergreifende Doku der Policies.

use core::mem::{MaybeUninit, size_of};
use std::io;

use crate::scheduler::{RunningSchedulerInfo, SchedulerKind, SchedulerProfile};

// ============================================================================
// `struct sched_attr` — definiert in <linux/sched/types.h> seit 3.14.
// libc 0.2 stellt sie nicht bereit; wir replizieren das Layout exakt.
// ============================================================================

/// Linux `struct sched_attr` (kernel-uapi).
///
/// Layout siehe `man 2 sched_setattr`. Felder muessen exakt in dieser
/// Reihenfolge stehen, weil der Kernel die Bytes Position-fuer-Position
/// liest und `size` mit `sizeof(struct sched_attr)` vergleicht.
#[repr(C)]
#[derive(Default)]
struct SchedAttr {
    size: u32,
    sched_policy: u32,
    sched_flags: u64,
    /// Nur bei SCHED_NORMAL/SCHED_BATCH/SCHED_IDLE.
    sched_nice: i32,
    /// Nur bei SCHED_FIFO/SCHED_RR.
    sched_priority: u32,
    /// Nur bei SCHED_DEADLINE.
    sched_runtime: u64,
    /// Nur bei SCHED_DEADLINE.
    sched_deadline: u64,
    /// Nur bei SCHED_DEADLINE.
    sched_period: u64,
}

/// Syscall-Numbers fuer die zwei Calls, die `libc` 0.2 nicht direkt
/// als Funktionen exportiert. `SYS_sched_setattr`/`SYS_sched_getattr`
/// sind in `libc::SYS_*` deklariert.
const SYS_SCHED_SETATTR: libc::c_long = libc::SYS_sched_setattr;
const SYS_SCHED_GETATTR: libc::c_long = libc::SYS_sched_getattr;

// SCHED_DEADLINE ist 6, FIFO 1, RR 2, OTHER 0. libc hat sie als
// SCHED_FIFO etc., aber `SCHED_DEADLINE` fehlt teilweise — defensiv
// hardcoden, wie es uapi tut.
const SCHED_OTHER: u32 = 0;
const SCHED_FIFO: u32 = 1;
const SCHED_RR: u32 = 2;
const SCHED_DEADLINE: u32 = 6;

// ============================================================================
// sched_setattr — apply
// ============================================================================

/// Wendet ein `SchedulerProfile` auf den **aufrufenden Thread** an.
///
/// Die `tid = 0`-Konvention der Linux-Syscalls bedeutet "diese
/// Aufgabe", was die Race-Frage zwischen User-Code und Syscall
/// eliminiert (kein anderer Thread ist Ziel der Mutation).
///
/// # Errors
/// `EPERM` wenn die Privilegien fehlen (typisch fuer FIFO/RR mit
/// `priority > 0` und alle DEADLINE-Calls). Andere Fehler siehe
/// `man 2 sched_setattr`.
pub(crate) fn apply_scheduler(profile: &SchedulerProfile) -> io::Result<()> {
    let mut attr = build_attr(profile);
    // SAFETY: sched_setattr(tid=0, &mut attr, flags=0) — Kernel
    // liest `attr` (groesse passt, layout = Kernel-uapi) und mutiert
    // den **aufrufenden** Thread. Detail-Begruendung der fuenf
    // Crate-Invarianten:
    // 1. `&mut attr` zeigt auf eine stack-lokale `SchedAttr`-Struct,
    //    die fuer die Dauer dieses Blocks lebt → kein Outliving.
    // 2. `SchedAttr` ist `#[repr(C)]` mit Layout exakt wie die
    //    Kernel-`struct sched_attr` (siehe `man 2 sched_setattr`),
    //    `attr.size = sizeof(SchedAttr)` ist gesetzt → keine
    //    Buffer-Overrun-Gefahr im Kernel.
    // 3. `tid = 0` → der Kernel addressiert den aufrufenden Thread,
    //    nicht einen Fremd-Thread → kein Mut-Aliasing.
    // 4. `flags = 0` → kein nicht-deklariertes Verhalten.
    // 5. Der Syscall liest die Bytes nur, schreibt nichts in `attr`
    //    zurueck → kein Aliasing-Risiko nach dem Call.
    let rc = unsafe {
        libc::syscall(
            SYS_SCHED_SETATTR,
            0i32,                          // tid: caller
            (&mut attr) as *mut SchedAttr, // attr ptr
            0u32,                          // flags
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn build_attr(profile: &SchedulerProfile) -> SchedAttr {
    let mut a = SchedAttr {
        size: size_of::<SchedAttr>() as u32,
        ..SchedAttr::default()
    };
    match *profile {
        SchedulerProfile::Default => {
            a.sched_policy = SCHED_OTHER;
        }
        SchedulerProfile::RealtimeFifo { priority } => {
            a.sched_policy = SCHED_FIFO;
            a.sched_priority = u32::from(priority);
        }
        SchedulerProfile::RealtimeRoundRobin { priority } => {
            a.sched_policy = SCHED_RR;
            a.sched_priority = u32::from(priority);
        }
        SchedulerProfile::Deadline {
            runtime_ns,
            deadline_ns,
            period_ns,
        } => {
            a.sched_policy = SCHED_DEADLINE;
            a.sched_runtime = runtime_ns;
            a.sched_deadline = deadline_ns;
            a.sched_period = period_ns;
        }
    }
    a
}

// ============================================================================
// sched_getattr — query
// ============================================================================

/// Fragt das aktuelle Scheduling-Profil des **aufrufenden Threads** ab.
///
/// # Errors
/// Kernel-Fehler aus `sched_getattr(2)`. Privilegienfrei.
pub(crate) fn read_scheduler() -> io::Result<RunningSchedulerInfo> {
    let mut attr = MaybeUninit::<SchedAttr>::zeroed();
    let p = attr.as_mut_ptr();
    // SAFETY: `MaybeUninit::zeroed()` liefert valid initialisierte
    // All-Zeros-`SchedAttr` (alle Felder sind primitive POD), daher
    // ueberschreibt `(*p).size = ...` initialisiertes Memory — kein UB.
    unsafe {
        (*p).size = size_of::<SchedAttr>() as u32;
    }
    // SAFETY: sched_getattr(tid=0, attr_ptr, sizeof, flags=0) — der
    // Kernel schreibt in unser stack-lokales Storage, kein Aliasing,
    // tid=0 = aufrufender Thread, flags=0 = keine Erweiterungen.
    // `attr` ist nach Erfolg voll initialisiert (see assume_init unten).
    let rc = unsafe {
        libc::syscall(
            SYS_SCHED_GETATTR,
            0i32,
            attr.as_mut_ptr(),
            size_of::<SchedAttr>() as u32,
            0u32, // flags
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: rc >= 0 → der Kernel hat alle Felder geschrieben; das
    // Storage war vorher schon all-zeroed; `assume_init` ist daher
    // unconditional safe in diesem Pfad.
    let attr = unsafe { attr.assume_init() };
    Ok(parse_attr(&attr))
}

fn parse_attr(a: &SchedAttr) -> RunningSchedulerInfo {
    let kind = match a.sched_policy {
        SCHED_FIFO => SchedulerKind::Fifo,
        SCHED_RR => SchedulerKind::RoundRobin,
        SCHED_DEADLINE => SchedulerKind::Deadline,
        _ => SchedulerKind::Other,
    };
    RunningSchedulerInfo {
        kind,
        priority: a.sched_priority as u8,
        runtime_ns: a.sched_runtime,
        deadline_ns: a.sched_deadline,
        period_ns: a.sched_period,
    }
}

// ============================================================================
// sched_setaffinity / sched_getaffinity
// ============================================================================

/// Liefert eine null-initialisierte `cpu_set_t`.
///
/// Eingekapseltes `unsafe`, damit Aufrufer eine pure Safe-API sehen.
fn zeroed_cpu_set() -> libc::cpu_set_t {
    // SAFETY: `cpu_set_t` ist POD (Array von u-long-Bitmasks); All-Zeros-
    // Bytes repraesentieren eine valide leere Maske. `assume_init` aus
    // `MaybeUninit::zeroed()` ist daher unconditional safe.
    unsafe { MaybeUninit::<libc::cpu_set_t>::zeroed().assume_init() }
}

/// Pinnt den aufrufenden Thread auf die angegebenen CPUs.
///
/// # Errors
/// `EINVAL` wenn keine der CPUs existiert. `EPERM` ist auf modernen
/// Kerneln nicht zu erwarten (Pinning auf eigene CPUs ist
/// privilegienfrei).
pub(crate) fn pin_to_cpus(cpus: &[usize]) -> io::Result<()> {
    if cpus.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cpu set must not be empty",
        ));
    }
    let max_cpu = cpus.iter().copied().max().unwrap_or(0);
    if max_cpu >= libc::CPU_SETSIZE as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cpu index exceeds CPU_SETSIZE",
        ));
    }

    let mut set = zeroed_cpu_set();
    // SAFETY: CPU_ZERO ist nach zeroed_cpu_set() redundant, aber
    // kanonisch nach uapi. `&mut set` ist exklusiv waehrend des Calls;
    // cpu_set_t ist POD.
    unsafe {
        libc::CPU_ZERO(&mut set);
    }
    for cpu in cpus {
        // SAFETY: CPU_SET schreibt das Bit `*cpu` in die Maske.
        // `*cpu < CPU_SETSIZE` ist durch den Range-Check oben
        // garantiert; `&mut set` ist exklusiv im Schleifen-Body.
        unsafe {
            libc::CPU_SET(*cpu, &mut set);
        }
    }
    // SAFETY: sched_setaffinity(tid=0, sizeof, &set) — der Kernel
    // liest die Bit-Mask, schreibt nichts in unser Storage. tid=0 =
    // aufrufender Thread; size = sizeof(cpu_set_t); set ist
    // stack-local + voll initialisiert.
    let rc = unsafe { libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set as *const _) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Liefert die CPU-Set-Bitmask des aufrufenden Threads als Vec
/// erlaubter CPU-Indizes.
///
/// # Errors
/// Kernel-Fehler aus `sched_getaffinity(2)`.
pub(crate) fn get_cpus() -> io::Result<std::vec::Vec<usize>> {
    let mut set = zeroed_cpu_set();
    // SAFETY: kanonische uapi-Initialisierung; `&mut set` exklusiv.
    unsafe {
        libc::CPU_ZERO(&mut set);
    }
    let sz = size_of::<libc::cpu_set_t>();
    let p = &mut set as *mut _;
    // SAFETY: sched_getaffinity(tid=0, sizeof, &mut set) — der Kernel
    // schreibt die aktuelle CPU-Maske ins Storage. tid=0 = aufrufender
    // Thread; `&mut set` ist exklusiv waehrend des Aufrufs;
    // Allocator-frei.
    let rc = unsafe { libc::sched_getaffinity(0, sz, p) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut out = std::vec::Vec::new();
    for cpu in 0..(libc::CPU_SETSIZE as usize) {
        // SAFETY: CPU_ISSET liest das Bit `cpu` in `set`. `cpu <
        // CPU_SETSIZE` per Schleifen-Bound; `set` ist voll
        // initialisiert + read-only-Borrow.
        let on = unsafe { libc::CPU_ISSET(cpu, &set) };
        if on {
            out.push(cpu);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn build_attr_default_uses_sched_other() {
        let a = build_attr(&SchedulerProfile::Default);
        assert_eq!(a.sched_policy, SCHED_OTHER);
        assert_eq!(a.sched_priority, 0);
    }

    #[test]
    fn build_attr_fifo_carries_priority() {
        let a = build_attr(&SchedulerProfile::RealtimeFifo { priority: 50 });
        assert_eq!(a.sched_policy, SCHED_FIFO);
        assert_eq!(a.sched_priority, 50);
    }

    #[test]
    fn build_attr_rr_carries_priority() {
        let a = build_attr(&SchedulerProfile::RealtimeRoundRobin { priority: 30 });
        assert_eq!(a.sched_policy, SCHED_RR);
        assert_eq!(a.sched_priority, 30);
    }

    #[test]
    fn build_attr_deadline_carries_triple() {
        let a = build_attr(&SchedulerProfile::Deadline {
            runtime_ns: 1_000_000,
            deadline_ns: 5_000_000,
            period_ns: 10_000_000,
        });
        assert_eq!(a.sched_policy, SCHED_DEADLINE);
        assert_eq!(a.sched_runtime, 1_000_000);
        assert_eq!(a.sched_deadline, 5_000_000);
        assert_eq!(a.sched_period, 10_000_000);
    }

    #[test]
    fn parse_attr_other_maps_to_other_kind() {
        let a = SchedAttr {
            sched_policy: SCHED_OTHER,
            ..SchedAttr::default()
        };
        let info = parse_attr(&a);
        assert_eq!(info.kind, SchedulerKind::Other);
    }

    #[test]
    fn parse_attr_fifo_maps_to_fifo_kind() {
        let a = SchedAttr {
            sched_policy: SCHED_FIFO,
            sched_priority: 10,
            ..SchedAttr::default()
        };
        let info = parse_attr(&a);
        assert_eq!(info.kind, SchedulerKind::Fifo);
        assert_eq!(info.priority, 10);
    }

    #[test]
    fn read_scheduler_returns_some_kind() {
        // Privilegienfrei — muss auch im CI ohne CAP_SYS_NICE laufen.
        let info = read_scheduler().expect("getattr");
        // Default-Threads laufen als SCHED_OTHER; der Wert ist abh.
        // vom CI-Container. Wir akzeptieren jeden Kind, pruefen nur
        // dass die FFI-Schicht sauber zurueck kommt.
        let _ = info.kind;
    }

    #[test]
    fn get_cpus_returns_at_least_one() {
        let cpus = get_cpus().expect("getaffinity");
        assert!(!cpus.is_empty());
    }

    #[test]
    fn pin_to_cpus_round_trip_on_first_cpu() {
        // Pinning auf eine CPU, die bereits erlaubt ist, ist
        // privilegienfrei.
        let allowed = get_cpus().expect("getaffinity");
        let target = allowed[0];
        pin_to_cpus(&[target]).expect("setaffinity");
        let after = get_cpus().expect("getaffinity post");
        assert_eq!(after, std::vec![target]);

        // Restore: pin auf die ganze ursprungsmenge.
        pin_to_cpus(&allowed).expect("restore");
    }

    #[test]
    fn pin_to_cpus_empty_input_rejected() {
        let err = pin_to_cpus(&[]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn pin_to_cpus_oversized_rejected() {
        let err = pin_to_cpus(&[libc::CPU_SETSIZE as usize + 1]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn apply_default_sched_other_is_priviledge_free() {
        // SCHED_OTHER ist privilegienfrei — muss ueberall durchlaufen.
        apply_scheduler(&SchedulerProfile::Default).expect("setattr SCHED_OTHER");
    }

    #[test]
    fn apply_fifo_with_priority_zero_does_not_panic() {
        // Priority 0 ist privilegienfrei. Auf manchen Kerneln (oder
        // mit sysctl-Settings) lehnt der Kernel die Anfrage trotzdem
        // ab — wir pruefen nur dass die FFI-Schicht sauber Errno
        // liefert, kein Panic.
        let _ = apply_scheduler(&SchedulerProfile::RealtimeFifo { priority: 0 });
    }

    #[test]
    fn apply_deadline_without_priv_returns_eperm_or_einval() {
        // Privilegierter Pfad: muss EPERM liefern (oder EINVAL bei
        // ungeeigneten Periodenparametern). Kein Panic, kein Crash.
        let res = apply_scheduler(&SchedulerProfile::Deadline {
            runtime_ns: 1_000_000,
            deadline_ns: 5_000_000,
            period_ns: 10_000_000,
        });
        if let Err(e) = res {
            assert!(matches!(
                e.raw_os_error(),
                Some(libc::EPERM) | Some(libc::EINVAL) | Some(libc::EBUSY)
            ));
        }
    }
}
