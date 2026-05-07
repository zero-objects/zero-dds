// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CPU-Affinity-API. Plattform-Routing nach `target_os`.
//!
//! Auf Linux delegiert dieses Modul an die internen `syscalls::pin_to_cpus`
//! / `syscalls::get_cpus`. Auf anderen Targets gibt jede
//! Funktion `Unsupported` zurueck.

use std::io;

/// Pinnt den **aufrufenden Thread** auf die angegebenen CPU-Indizes.
///
/// Reihenfolge im Slice ist irrelevant — der Kernel verwendet eine
/// Bit-Mask. Doppelte Indizes werden silently dedupliziert.
///
/// # Errors
/// * `InvalidInput` wenn `cpus.is_empty()` oder ein Index `>=
///   CPU_SETSIZE` ist (Linux: 1024).
/// * Kernel-Errno aus `sched_setaffinity(2)` — typisch `EINVAL`,
///   wenn keine der CPUs online ist.
/// * `Unsupported` auf Nicht-Linux-Targets.
///
/// # Beispiel
/// ```no_run
/// use zerodds_rt_linux::pin_current_thread_to_cpus;
/// pin_current_thread_to_cpus(&[2, 3]).unwrap();
/// ```
pub fn pin_current_thread_to_cpus(cpus: &[usize]) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        crate::syscalls::pin_to_cpus(cpus)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cpus;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "pin_current_thread_to_cpus requires Linux",
        ))
    }
}

/// Liefert die CPUs, auf denen der aufrufende Thread laufen darf.
///
/// # Errors
/// Kernel-Errno aus `sched_getaffinity(2)`.
/// `Unsupported` auf Nicht-Linux-Targets.
pub fn current_thread_cpus() -> io::Result<std::vec::Vec<usize>> {
    #[cfg(target_os = "linux")]
    {
        crate::syscalls::get_cpus()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "current_thread_cpus() requires Linux",
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn pin_returns_unsupported_off_linux() {
        let err = pin_current_thread_to_cpus(&[0]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn cpus_returns_unsupported_off_linux() {
        let err = current_thread_cpus().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_round_trip_pin_then_read() {
        let allowed = current_thread_cpus().expect("getaffinity");
        let target = allowed[0];
        pin_current_thread_to_cpus(&[target]).expect("setaffinity");
        let after = current_thread_cpus().expect("getaffinity post");
        assert_eq!(after, std::vec![target]);
        // Restore.
        pin_current_thread_to_cpus(&allowed).expect("restore");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_empty_input_invalid() {
        let err = pin_current_thread_to_cpus(&[]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
