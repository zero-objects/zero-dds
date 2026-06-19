// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Public scheduler API. Platform routing by `target_os`.
//!
//! On Linux this module delegates to the internal `syscalls` module, where the
//! `unsafe { libc::syscall(...) }` blocks are each documented individually.
//! On other targets every function returns `Unsupported`,
//! but the API compiles — the workspace builds on macOS/Windows.

use std::io;

/// Scheduler policy. Linux-specific (see `sched(7)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerProfile {
    /// Linux `SCHED_OTHER` (CFS) — the default for all threads.
    Default,
    /// Linux `SCHED_FIFO` — strictly priority-based, no quantum.
    /// `priority` is the value of `sched_priority` (1..=99, higher
    /// beats lower; 0 is not allowed for FIFO/RR with
    /// non-empty queues, is accepted by the kernel but treated as
    /// a SCHED_OTHER fallback).
    ///
    /// Privileges: `CAP_SYS_NICE` from priority > 0 on; depending on
    /// `RLIMIT_RTPRIO` also for priority 0.
    RealtimeFifo {
        /// Value of `sched_priority` (1..=99).
        priority: u8,
    },
    /// Linux `SCHED_RR` — like FIFO, but with a round-robin quantum
    /// per priority level.
    RealtimeRoundRobin {
        /// Value of `sched_priority` (1..=99).
        priority: u8,
    },
    /// Linux `SCHED_DEADLINE` (CBS+EDF) — hard guarantees via a
    /// (`runtime`, `deadline`, `period`) triple in nanoseconds.
    /// See `sched_setattr(2)` for the spec. Conditions:
    ///
    /// * `runtime <= deadline <= period`
    /// * The kernel computes a bandwidth reservation. EBUSY if
    ///   the global reservation blows the limit (default 95%).
    ///
    /// Privileges: always `CAP_SYS_NICE`. Forks must not
    /// inherit (otherwise `EBUSY`).
    Deadline {
        /// Worst-case execution time per period (ns).
        runtime_ns: u64,
        /// Soft deadline from the start of the period (ns).
        deadline_ns: u64,
        /// Repetition period (ns).
        period_ns: u64,
    },
}

impl SchedulerProfile {
    /// Applies the profile to the **calling thread**.
    ///
    /// # Errors
    /// * `EPERM` (PermissionDenied) if the privileges are missing.
    /// * `EINVAL` (InvalidInput) on inconsistent deadline values.
    /// * `Unsupported` on non-Linux targets.
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

/// Description of an active scheduling configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunningSchedulerInfo {
    /// Selected policy.
    pub kind: SchedulerKind,
    /// `sched_priority`. Only relevant for FIFO/RR.
    pub priority: u8,
    /// `sched_runtime` (ns). Only relevant for Deadline.
    pub runtime_ns: u64,
    /// `sched_deadline` (ns). Only relevant for Deadline.
    pub deadline_ns: u64,
    /// `sched_period` (ns). Only relevant for Deadline.
    pub period_ns: u64,
}

/// Classification of the Linux scheduler policy.
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

/// Reads the current scheduler configuration of the calling thread.
///
/// Privilege-free.
///
/// # Errors
/// Kernel error from `sched_getattr(2)`.
/// `Unsupported` on non-Linux targets.
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
        // Without CAP_SYS_NICE, DEADLINE must return EPERM or EINVAL/EBUSY;
        // under no circumstances a panic.
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
