// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-process serialization for the C# tests' `dotnet build`.
//!
//! Safety classification: **TOOLING**. Test-only helper, not linked into any
//! runtime artifact; a failed lock acquisition aborts the test run loudly.
//!
//! Several test binaries — `endpoint-e2e/csharp`, `idl-csharp/roundtrip_xcdr2`,
//! `idl-csharp/adversarial_corpus`, `conformance/cross_language_xcdr2` — each
//! spawn `dotnet build`/`dotnet test`/`dotnet run` against a project that
//! (directly or transitively) references the **one** `ZeroDDS.Cdr` project.
//! MSBuild writes that project's intermediate/output (`obj/…/ZeroDDS.Cdr.dll`)
//! into a location shared by every consumer, so two builds running at once —
//! and `cargo test` runs test binaries in parallel, across crates — collide on
//! it and fail with `CS2012` ("the file is being used by another process").
//!
//! A process-local `Mutex` cannot fix this: the colliding builds live in
//! different processes. This crate takes an OS advisory lock (`flock(2)`) on a
//! fixed lockfile, so the guard is mutually exclusive across threads *and*
//! processes. The lock is released when the returned guard drops or the process
//! exits — a test killed mid-build never leaves a stale lock behind.
//!
//! Hold the guard only around the build step; the built binaries then run
//! concurrently as before.

// Test-only tooling: a lockfile that cannot be created or locked is an
// unrecoverable setup failure, so panicking (not bubbling a Result up through
// every call site) is the right behaviour here.
#![allow(clippy::expect_used, clippy::panic)]

/// Guard returned by [`dotnet_build_guard`]. Holds the exclusive lock for its
/// lifetime; dropping it (or the process exiting) releases the lock.
#[must_use = "the lock is held only while the guard is alive"]
pub struct BuildGuard {
    #[cfg(unix)]
    _file: std::fs::File,
}

/// Acquires the cross-process `dotnet build` lock, blocking until it is free.
///
/// Call this immediately before spawning `dotnet build`/`test`/`run` and keep
/// the returned guard alive until the build completes.
#[cfg(unix)]
pub fn dotnet_build_guard() -> BuildGuard {
    use std::os::unix::io::AsRawFd;

    let path = std::env::temp_dir().join("zerodds-cdr-dotnet-build.lock");
    let file = std::fs::File::create(&path).expect("create dotnet build lockfile");
    loop {
        // SAFETY: `file` owns a valid, open fd for the whole call.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc == 0 {
            break;
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EINTR) {
            continue; // interrupted by a signal — retry
        }
        panic!("flock dotnet build lock: {err}");
    }
    BuildGuard { _file: file }
}

/// Non-Unix fallback: the C# `dotnet` end-to-end tests run on Unix CI, so there
/// is no cross-process lock to take here. Returns an inert guard so callers
/// compile unchanged on every workspace target.
#[cfg(not(unix))]
pub fn dotnet_build_guard() -> BuildGuard {
    BuildGuard {}
}
