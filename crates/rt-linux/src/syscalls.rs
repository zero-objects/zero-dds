// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Raw syscall wrappers. All `unsafe` blocks of the crate live here.
//!
//! # Scope
//!
//! This module is `cfg(target_os = "linux")`-only — it simply does
//! not exist on macOS/Windows. The public API (modules
//! `scheduler` + `affinity`) decides via `cfg` whether it delegates
//! here or returns an `Unsupported` error.
//!
//! # `unsafe` discipline
//!
//! Each `unsafe { libc::... }` block is:
//!
//! 1. the shortest possible sequence for **exactly one** syscall;
//! 2. documented with a `// SAFETY:` comment that concretely proves the five
//!    crate invariants (see `lib.rs`);
//! 3. immediately followed by an `errno` check `if rc < 0 { ... }`.
//!
//! There are **no** helper functions that pass `unsafe` pointers through
//! multiple `safe` layers.
//!
//! # Linux man pages
//!
//! * sched_setattr(2) — `struct sched_attr` + flags.
//! * sched_getattr(2) — symmetric read.
//! * sched_setaffinity(2) — `cpu_set_t` bit-mask.
//! * sched_getaffinity(2) — symmetric read.
//! * `sched(7)` — overarching documentation of the policies.

use core::mem::{MaybeUninit, size_of};
use std::io;

use crate::scheduler::{RunningSchedulerInfo, SchedulerKind, SchedulerProfile};

// ============================================================================
// `struct sched_attr` — defined in <linux/sched/types.h> since 3.14.
// libc 0.2 does not provide it; we replicate the layout exactly.
// ============================================================================

/// Linux `struct sched_attr` (kernel uapi).
///
/// Layout see `man 2 sched_setattr`. Fields must be in exactly this
/// order, because the kernel reads the bytes position-by-position
/// and compares `size` with `sizeof(struct sched_attr)`.
#[repr(C)]
#[derive(Default)]
struct SchedAttr {
    size: u32,
    sched_policy: u32,
    sched_flags: u64,
    /// Only for SCHED_NORMAL/SCHED_BATCH/SCHED_IDLE.
    sched_nice: i32,
    /// Only for SCHED_FIFO/SCHED_RR.
    sched_priority: u32,
    /// Only for SCHED_DEADLINE.
    sched_runtime: u64,
    /// Only for SCHED_DEADLINE.
    sched_deadline: u64,
    /// Only for SCHED_DEADLINE.
    sched_period: u64,
}

/// Syscall numbers for the two calls that `libc` 0.2 does not export
/// directly as functions. `SYS_sched_setattr`/`SYS_sched_getattr`
/// are declared in `libc::SYS_*`.
const SYS_SCHED_SETATTR: libc::c_long = libc::SYS_sched_setattr;
const SYS_SCHED_GETATTR: libc::c_long = libc::SYS_sched_getattr;

// SCHED_DEADLINE is 6, FIFO 1, RR 2, OTHER 0. libc has them as
// SCHED_FIFO etc., but `SCHED_DEADLINE` is partly missing — hardcode
// defensively, as uapi does.
const SCHED_OTHER: u32 = 0;
const SCHED_FIFO: u32 = 1;
const SCHED_RR: u32 = 2;
const SCHED_DEADLINE: u32 = 6;

// ============================================================================
// sched_setattr — apply
// ============================================================================

/// Applies a `SchedulerProfile` to the **calling thread**.
///
/// The `tid = 0` convention of the Linux syscalls means "this
/// task", which eliminates the race question between user code and syscall
/// (no other thread is the target of the mutation).
///
/// # Errors
/// `EPERM` if the privileges are missing (typical for FIFO/RR with
/// `priority > 0` and all DEADLINE calls). Other errors see
/// `man 2 sched_setattr`.
pub(crate) fn apply_scheduler(profile: &SchedulerProfile) -> io::Result<()> {
    let mut attr = build_attr(profile);
    // SAFETY: sched_setattr(tid=0, &mut attr, flags=0) — the kernel
    // reads `attr` (size matches, layout = kernel uapi) and mutates
    // the **calling** thread. Detailed justification of the five
    // crate invariants:
    // 1. `&mut attr` points to a stack-local `SchedAttr` struct
    //    that lives for the duration of this block → no outliving.
    // 2. `SchedAttr` is `#[repr(C)]` with a layout exactly like the
    //    kernel `struct sched_attr` (see `man 2 sched_setattr`),
    //    `attr.size = sizeof(SchedAttr)` is set → no
    //    buffer-overrun danger in the kernel.
    // 3. `tid = 0` → the kernel addresses the calling thread,
    //    not a foreign thread → no mut-aliasing.
    // 4. `flags = 0` → no undeclared behavior.
    // 5. The syscall only reads the bytes, writes nothing back into `attr`
    //    → no aliasing risk after the call.
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

/// Queries the current scheduling profile of the **calling thread**.
///
/// # Errors
/// Kernel error from `sched_getattr(2)`. Privilege-free.
pub(crate) fn read_scheduler() -> io::Result<RunningSchedulerInfo> {
    let mut attr = MaybeUninit::<SchedAttr>::zeroed();
    let p = attr.as_mut_ptr();
    // SAFETY: `MaybeUninit::zeroed()` yields a validly initialized
    // all-zeros `SchedAttr` (all fields are primitive POD), so
    // `(*p).size = ...` overwrites initialized memory — no UB.
    unsafe {
        (*p).size = size_of::<SchedAttr>() as u32;
    }
    // SAFETY: sched_getattr(tid=0, attr_ptr, sizeof, flags=0) — the
    // kernel writes into our stack-local storage, no aliasing,
    // tid=0 = calling thread, flags=0 = no extensions.
    // `attr` is fully initialized on success (see assume_init below).
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
    // SAFETY: rc >= 0 → the kernel has written all fields; the
    // storage was already all-zeroed beforehand; `assume_init` is therefore
    // unconditionally safe in this path.
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

/// Returns a zero-initialized `cpu_set_t`.
///
/// Encapsulated `unsafe`, so that callers see a pure safe API.
fn zeroed_cpu_set() -> libc::cpu_set_t {
    // SAFETY: `cpu_set_t` is POD (array of u-long bitmasks); all-zeros
    // bytes represent a valid empty mask. `assume_init` from
    // `MaybeUninit::zeroed()` is therefore unconditionally safe.
    unsafe { MaybeUninit::<libc::cpu_set_t>::zeroed().assume_init() }
}

/// Pins the calling thread to the given CPUs.
///
/// # Errors
/// `EINVAL` if none of the CPUs exists. `EPERM` is not to be expected on
/// modern kernels (pinning to one's own CPUs is
/// privilege-free).
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
    // SAFETY: CPU_ZERO is redundant after zeroed_cpu_set(), but
    // canonical per uapi. `&mut set` is exclusive during the call;
    // cpu_set_t is POD.
    unsafe {
        libc::CPU_ZERO(&mut set);
    }
    for cpu in cpus {
        // SAFETY: CPU_SET writes the bit `*cpu` into the mask.
        // `*cpu < CPU_SETSIZE` is guaranteed by the range check above;
        // `&mut set` is exclusive in the loop body.
        unsafe {
            libc::CPU_SET(*cpu, &mut set);
        }
    }
    // SAFETY: sched_setaffinity(tid=0, sizeof, &set) — the kernel
    // reads the bit-mask, writes nothing into our storage. tid=0 =
    // calling thread; size = sizeof(cpu_set_t); set is
    // stack-local + fully initialized.
    let rc = unsafe { libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set as *const _) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Returns the CPU-set bitmask of the calling thread as a vec
/// of allowed CPU indices.
///
/// # Errors
/// Kernel error from `sched_getaffinity(2)`.
pub(crate) fn get_cpus() -> io::Result<std::vec::Vec<usize>> {
    let mut set = zeroed_cpu_set();
    // SAFETY: canonical uapi initialization; `&mut set` exclusive.
    unsafe {
        libc::CPU_ZERO(&mut set);
    }
    let sz = size_of::<libc::cpu_set_t>();
    let p = &mut set as *mut _;
    // SAFETY: sched_getaffinity(tid=0, sizeof, &mut set) — the kernel
    // writes the current CPU mask into the storage. tid=0 = calling
    // thread; `&mut set` is exclusive during the call;
    // allocator-free.
    let rc = unsafe { libc::sched_getaffinity(0, sz, p) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut out = std::vec::Vec::new();
    for cpu in 0..(libc::CPU_SETSIZE as usize) {
        // SAFETY: CPU_ISSET reads the bit `cpu` in `set`. `cpu <
        // CPU_SETSIZE` per the loop bound; `set` is fully
        // initialized + a read-only borrow.
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
        // Privilege-free — must also run in CI without CAP_SYS_NICE.
        let info = read_scheduler().expect("getattr");
        // Default threads run as SCHED_OTHER; the value depends
        // on the CI container. We accept any kind, only check
        // that the FFI layer returns cleanly.
        let _ = info.kind;
    }

    #[test]
    fn get_cpus_returns_at_least_one() {
        let cpus = get_cpus().expect("getaffinity");
        assert!(!cpus.is_empty());
    }

    #[test]
    fn pin_to_cpus_round_trip_on_first_cpu() {
        // Pinning to a CPU that is already allowed is
        // privilege-free.
        let allowed = get_cpus().expect("getaffinity");
        let target = allowed[0];
        pin_to_cpus(&[target]).expect("setaffinity");
        let after = get_cpus().expect("getaffinity post");
        assert_eq!(after, std::vec![target]);

        // Restore: pin to the whole original set.
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
        // SCHED_OTHER is privilege-free — must pass everywhere.
        apply_scheduler(&SchedulerProfile::Default).expect("setattr SCHED_OTHER");
    }

    #[test]
    fn apply_fifo_with_priority_zero_does_not_panic() {
        // Priority 0 is privilege-free. On some kernels (or
        // with sysctl settings) the kernel still rejects the request
        // — we only check that the FFI layer returns an errno
        // cleanly, no panic.
        let _ = apply_scheduler(&SchedulerProfile::RealtimeFifo { priority: 0 });
    }

    #[test]
    fn apply_deadline_without_priv_returns_eperm_or_einval() {
        // Privileged path: must return EPERM (or EINVAL for
        // unsuitable period parameters). No panic, no crash.
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
