// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Public Scheduler-API. Plattform-Routing nach `target_os`.
//!
//! Auf Linux delegiert dieses Modul an das interne `syscalls`-Modul, wo die
//! `unsafe { libc::syscall(...) }`-Bloecke jeweils einzeln dokumentiert
//! sind. Auf anderen Targets gibt jede Funktion `Unsupported` zurueck,
//! aber die API kompiliert — der Workspace baut auf macOS/Windows.

use std::io;

/// Scheduler-Policy. Linux-spezifisch (siehe `sched(7)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerProfile {
    /// Linux `SCHED_OTHER` (CFS) — der Default fuer alle Threads.
    Default,
    /// Linux `SCHED_FIFO` — strikt prioritaets-basiert, kein Quantum.
    /// `priority` ist der Wert von `sched_priority` (1..=99, hoeher
    /// schlaegt niedriger; 0 ist nicht erlaubt fuer FIFO/RR mit
    /// nicht-leeren Queues, wird vom Kernel akzeptiert aber als
    /// SCHED_OTHER-Fallback behandelt).
    ///
    /// Privilegien: `CAP_SYS_NICE` ab Priority > 0; je nach
    /// `RLIMIT_RTPRIO` auch fuer Priority 0.
    RealtimeFifo {
        /// Wert von `sched_priority` (1..=99).
        priority: u8,
    },
    /// Linux `SCHED_RR` — wie FIFO, aber mit Round-Robin-Quantum
    /// pro Priority-Level.
    RealtimeRoundRobin {
        /// Wert von `sched_priority` (1..=99).
        priority: u8,
    },
    /// Linux `SCHED_DEADLINE` (CBS+EDF) — harte Garantien per
    /// (`runtime`, `deadline`, `period`)-Triple in Nanosekunden.
    /// Spec siehe `sched_setattr(2)`. Bedingungen:
    ///
    /// * `runtime <= deadline <= period`
    /// * Kernel berechnet eine Bandbreitenreservierung. EBUSY wenn
    ///   die globale Reservierung das Limit (default 95%) sprengt.
    ///
    /// Privilegien: immer `CAP_SYS_NICE`. Forks duerfen nicht
    /// vererben (sonst `EBUSY`).
    Deadline {
        /// Worst-Case-Execution-Time pro Periode (ns).
        runtime_ns: u64,
        /// Soft-Deadline ab Periodenstart (ns).
        deadline_ns: u64,
        /// Wiederholungsperiode (ns).
        period_ns: u64,
    },
}

impl SchedulerProfile {
    /// Wendet das Profil auf den **aufrufenden Thread** an.
    ///
    /// # Errors
    /// * `EPERM` (PermissionDenied) wenn die Privilegien fehlen.
    /// * `EINVAL` (InvalidInput) bei inkonsistenten Deadline-Werten.
    /// * `Unsupported` auf Nicht-Linux-Targets.
    pub fn apply_to_current_thread(&self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            crate::syscalls::apply_scheduler(self)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = self;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "SchedulerProfile::apply_to_current_thread requires Linux",
            ))
        }
    }
}

/// Beschreibung einer aktiven Scheduling-Konfiguration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunningSchedulerInfo {
    /// Ausgewaehlte Policy.
    pub kind: SchedulerKind,
    /// `sched_priority`. Nur bei FIFO/RR relevant.
    pub priority: u8,
    /// `sched_runtime` (ns). Nur bei Deadline relevant.
    pub runtime_ns: u64,
    /// `sched_deadline` (ns). Nur bei Deadline relevant.
    pub deadline_ns: u64,
    /// `sched_period` (ns). Nur bei Deadline relevant.
    pub period_ns: u64,
}

/// Klassifikation der Linux-Scheduler-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerKind {
    /// CFS (`SCHED_OTHER`/`SCHED_BATCH`/`SCHED_IDLE`).
    Other,
    /// `SCHED_FIFO`.
    Fifo,
    /// `SCHED_RR`.
    RoundRobin,
    /// `SCHED_DEADLINE`.
    Deadline,
}

/// Liest die aktuelle Scheduler-Konfiguration des aufrufenden Threads.
///
/// Privilegienfrei.
///
/// # Errors
/// Kernel-Fehler aus `sched_getattr(2)`.
/// `Unsupported` auf Nicht-Linux-Targets.
pub fn current_scheduler() -> io::Result<RunningSchedulerInfo> {
    #[cfg(target_os = "linux")]
    {
        crate::syscalls::read_scheduler()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "current_scheduler() requires Linux",
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_send_sync_clone_eq() {
        fn assert_traits<T: Send + Sync + Clone + Copy + PartialEq>() {}
        assert_traits::<SchedulerProfile>();
        assert_traits::<RunningSchedulerInfo>();
        assert_traits::<SchedulerKind>();
    }

    #[test]
    fn profile_default_is_distinct_from_fifo() {
        assert_ne!(
            SchedulerProfile::Default,
            SchedulerProfile::RealtimeFifo { priority: 1 }
        );
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn apply_returns_unsupported_off_linux() {
        let err = SchedulerProfile::Default
            .apply_to_current_thread()
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn current_scheduler_returns_unsupported_off_linux() {
        let err = current_scheduler().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_default_apply_round_trips_via_getattr() {
        SchedulerProfile::Default
            .apply_to_current_thread()
            .expect("apply default");
        let info = current_scheduler().expect("read");
        assert_eq!(info.kind, SchedulerKind::Other);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_eperm_for_deadline_without_caps() {
        // Ohne CAP_SYS_NICE muss DEADLINE EPERM oder EINVAL/EBUSY
        // liefern; auf gar keinen Fall ein Panic.
        let res = SchedulerProfile::Deadline {
            runtime_ns: 1_000_000,
            deadline_ns: 5_000_000,
            period_ns: 10_000_000,
        }
        .apply_to_current_thread();
        if let Err(e) = res {
            assert!(matches!(
                e.kind(),
                io::ErrorKind::PermissionDenied
                    | io::ErrorKind::InvalidInput
                    | io::ErrorKind::ResourceBusy
                    | io::ErrorKind::Other,
            ));
        }
    }
}
