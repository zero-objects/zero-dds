//! LD_PRELOAD shim for clock-skew chaos (WP 5.F.2 Phase-B).
//!
//! Crate `chaos-clock-skew`. Safety classification: **STANDARD**
//! (FFI boundary, dlsym-based).
//!
//! Overrides `clock_gettime()` and `gettimeofday()` so that the
//! wall-clock time a process reads is shifted. The shift comes from
//! env variables set by the test harness and read on every call:
//!
//! * `CHAOS_CLOCK_SKEW_NS` — fixed offset in nanoseconds (signed;
//!   positive = clock runs ahead, negative = clock runs behind).
//! * `CHAOS_CLOCK_DRIFT_PPM` — rate error in parts per million
//!   (signed; `100` = the clock gains 100 µs per second, `-100` = it
//!   loses 100 µs per second). The drift accumulates from the first
//!   intercepted call in the process: after `t` elapsed seconds
//!   (measured on the real `CLOCK_MONOTONIC`) the drift share of the
//!   offset is `t * ppm` µs.
//!
//! Both terms apply to `CLOCK_REALTIME` in `clock_gettime()` and to
//! `gettimeofday()` (there truncated to microseconds).
//!
//! Activation:
//!
//! ```bash
//! cargo build -p chaos-clock-skew --release
//! LD_PRELOAD=$(pwd)/target/release/libchaos_clock_skew.so \
//!   CHAOS_CLOCK_SKEW_NS=100000000 \
//!   CHAOS_CLOCK_DRIFT_PPM=100 \
//!   ./my-process
//! ```
//!
//! The interceptors are built on Linux only (they rely on
//! `dlsym(RTLD_NEXT)` and the glibc symbol names). On other targets
//! the crate is a stub without exported symbols.
//!
//! # What is NOT intercepted?
//!
//! * Clocks other than `CLOCK_REALTIME` (`CLOCK_MONOTONIC`,
//!   `CLOCK_MONOTONIC_RAW`, `CLOCK_BOOTTIME`, `CLOCK_REALTIME_COARSE`,
//!   `CLOCK_TAI`, …) — passed through unchanged.
//! * Time reads that do not go through the libc symbols, e.g. a raw
//!   `syscall(SYS_clock_gettime, …)` or statically linked binaries.
//!   Rust `std` (`SystemTime::now()`) calls libc and is intercepted.

#![warn(missing_docs)]
#![allow(clippy::missing_safety_doc)]
// Accept init-time panics: without dlsym(RTLD_NEXT) the LD_PRELOAD
// lib is functionally dead, so process termination is the only
// sensible failure mode. This is convenience-tool code, not a
// runtime path.
#![allow(clippy::panic)]

#[cfg(target_os = "linux")]
mod linux {
    use core::ffi::{c_int, c_void};
    use std::sync::OnceLock;

    /// Linux `timespec`. glibc-compatible layout.
    #[repr(C)]
    pub struct Timespec {
        /// Seconds since epoch.
        pub tv_sec: i64,
        /// Nanoseconds, 0..1_000_000_000.
        pub tv_nsec: i64,
    }

    /// Linux `timeval`. glibc-compatible layout.
    #[repr(C)]
    pub struct Timeval {
        /// Seconds.
        pub tv_sec: i64,
        /// Microseconds.
        pub tv_usec: i64,
    }

    /// `clockid_t` enum, glibc-compatible.
    pub type ClockId = c_int;
    /// `CLOCK_REALTIME`.
    pub const CLOCK_REALTIME: ClockId = 0;
    /// `CLOCK_MONOTONIC`.
    pub const CLOCK_MONOTONIC: ClockId = 1;

    const NS_PER_SEC: i64 = 1_000_000_000;

    type ClockGettimeFn = unsafe extern "C" fn(ClockId, *mut Timespec) -> c_int;
    type GetTimeOfDayFn = unsafe extern "C" fn(*mut Timeval, *mut c_void) -> c_int;

    static REAL_CLOCK_GETTIME: OnceLock<ClockGettimeFn> = OnceLock::new();
    static REAL_GETTIMEOFDAY: OnceLock<GetTimeOfDayFn> = OnceLock::new();
    /// Real `CLOCK_MONOTONIC` reading (ns) at the first intercepted
    /// call; the drift accumulates from here.
    static DRIFT_START_NS: OnceLock<i64> = OnceLock::new();

    fn real_clock_gettime() -> ClockGettimeFn {
        *REAL_CLOCK_GETTIME.get_or_init(|| {
            // SAFETY: dlsym with RTLD_NEXT returns the real glibc
            // implementation. Casting the pointer to a function type
            // with the same signature is contractually correct.
            unsafe {
                let p = libc_stub::dlsym(libc_stub::RTLD_NEXT, c"clock_gettime".as_ptr());
                if p.is_null() {
                    panic!("dlsym(clock_gettime) failed");
                }
                core::mem::transmute::<*mut c_void, ClockGettimeFn>(p)
            }
        })
    }

    fn real_gettimeofday() -> GetTimeOfDayFn {
        *REAL_GETTIMEOFDAY.get_or_init(|| {
            // SAFETY: same reasoning as real_clock_gettime.
            unsafe {
                let p = libc_stub::dlsym(libc_stub::RTLD_NEXT, c"gettimeofday".as_ptr());
                if p.is_null() {
                    panic!("dlsym(gettimeofday) failed");
                }
                core::mem::transmute::<*mut c_void, GetTimeOfDayFn>(p)
            }
        })
    }

    /// Returns the fixed skew offset (ns). Read on every call so that
    /// tests can change the value at runtime.
    fn skew_ns() -> i64 {
        std::env::var("CHAOS_CLOCK_SKEW_NS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    }

    fn drift_ppm() -> i64 {
        std::env::var("CHAOS_CLOCK_DRIFT_PPM")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    }

    /// Real `CLOCK_MONOTONIC` in nanoseconds, 0 if the call fails.
    fn real_monotonic_ns() -> i64 {
        let mut ts = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `ts` is a valid, writable timespec; the real
        // clock_gettime only writes into it.
        let r = unsafe { real_clock_gettime()(CLOCK_MONOTONIC, &mut ts) };
        if r != 0 {
            return 0;
        }
        ts.tv_sec
            .saturating_mul(NS_PER_SEC)
            .saturating_add(ts.tv_nsec)
    }

    /// Drift share of the offset: `elapsed_ns * ppm / 1e6`, saturated
    /// to the `i64` range.
    pub(crate) fn drift_offset_ns(elapsed_ns: i64, ppm: i64) -> i64 {
        let v = i128::from(elapsed_ns) * i128::from(ppm) / 1_000_000;
        i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
    }

    /// Total offset (skew + drift) in nanoseconds for the current call.
    fn current_offset_ns() -> i64 {
        let start = *DRIFT_START_NS.get_or_init(real_monotonic_ns);
        let ppm = drift_ppm();
        let drift = if ppm == 0 {
            0
        } else {
            drift_offset_ns(real_monotonic_ns().saturating_sub(start), ppm)
        };
        skew_ns().saturating_add(drift)
    }

    /// Adds `offset_ns` to a (seconds, sub-second) pair whose
    /// sub-second unit is `ns_per_unit` nanoseconds (1 for `timespec`,
    /// 1000 for `timeval`). The sub-second field stays normalised to
    /// `0..units_per_second`.
    pub(crate) fn shift(sec: &mut i64, sub: &mut i64, offset_ns: i64, ns_per_unit: i64) {
        let units_per_sec = i128::from(NS_PER_SEC / ns_per_unit);
        let total = i128::from(*sub) + i128::from(offset_ns / ns_per_unit);
        let carry = total.div_euclid(units_per_sec);
        // |carry| <= (i64::MAX + 1e9) / 1e6 and 0 <= rem < 1e9: both fit.
        let carry = i64::try_from(carry).unwrap_or(if carry < 0 { i64::MIN } else { i64::MAX });
        let rem = i64::try_from(total.rem_euclid(units_per_sec)).unwrap_or(0);
        *sec = sec.saturating_add(carry);
        *sub = rem;
    }

    /// `clock_gettime` replacement.
    ///
    /// # Safety
    /// `tp` must be valid.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn clock_gettime(clk_id: ClockId, tp: *mut Timespec) -> c_int {
        // SAFETY: real_clock_gettime() returns the real glibc
        // implementation; the call semantics match the signature.
        let r = unsafe { real_clock_gettime()(clk_id, tp) };
        if r != 0 || tp.is_null() {
            return r;
        }
        // Skew only for CLOCK_REALTIME — MONOTONIC stays unchanged.
        if clk_id == CLOCK_REALTIME {
            let offset = current_offset_ns();
            if offset != 0 {
                // SAFETY: r==0, tp was filled by the real function.
                let ts = unsafe { &mut *tp };
                shift(&mut ts.tv_sec, &mut ts.tv_nsec, offset, 1);
            }
        }
        r
    }

    /// `gettimeofday` replacement.
    ///
    /// # Safety
    /// `tv` must be valid.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gettimeofday(tv: *mut Timeval, tz: *mut c_void) -> c_int {
        // SAFETY: real_gettimeofday() returns the real glibc
        // implementation; signature-compatible pass-through.
        let r = unsafe { real_gettimeofday()(tv, tz) };
        if r != 0 || tv.is_null() {
            return r;
        }
        let offset = current_offset_ns();
        if offset != 0 {
            // SAFETY: r==0 → tv was filled.
            let v = unsafe { &mut *tv };
            shift(&mut v.tv_sec, &mut v.tv_usec, offset, 1_000);
        }
        r
    }

    mod libc_stub {
        use core::ffi::{c_char, c_void};
        unsafe extern "C" {
            pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        }
        // RTLD_NEXT on Linux glibc = -1 as void*.
        pub const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::Mutex;
        use std::time::Duration;

        /// cargo test runs multi-threaded by default; CHAOS_CLOCK_SKEW_NS
        /// and CHAOS_CLOCK_DRIFT_PPM are process-global env vars, so all
        /// tests that set them must hold the same mutex.
        /// Pattern: `let _g = ENV_LOCK.lock()...` at the start of each test.
        static ENV_LOCK: Mutex<()> = Mutex::new(());

        fn set_env(skew: Option<&str>, ppm: Option<&str>) {
            // SAFETY: env mutation serialized under ENV_LOCK.
            unsafe {
                match skew {
                    Some(v) => std::env::set_var("CHAOS_CLOCK_SKEW_NS", v),
                    None => std::env::remove_var("CHAOS_CLOCK_SKEW_NS"),
                }
                match ppm {
                    Some(v) => std::env::set_var("CHAOS_CLOCK_DRIFT_PPM", v),
                    None => std::env::remove_var("CHAOS_CLOCK_DRIFT_PPM"),
                }
            }
        }

        fn ns(ts: &Timespec) -> i128 {
            i128::from(ts.tv_sec) * 1_000_000_000 + i128::from(ts.tv_nsec)
        }

        fn real_realtime() -> Timespec {
            let mut ts = Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // SAFETY: valid out-pointer.
            let r = unsafe { real_clock_gettime()(CLOCK_REALTIME, &mut ts) };
            assert_eq!(r, 0);
            ts
        }

        fn shim(clk: ClockId) -> Timespec {
            let mut ts = Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // SAFETY: valid out-pointer.
            let r = unsafe { clock_gettime(clk, &mut ts) };
            assert_eq!(r, 0);
            ts
        }

        #[test]
        fn shift_zero_is_noop() {
            let (mut s, mut n) = (100, 500);
            shift(&mut s, &mut n, 0, 1);
            assert_eq!((s, n), (100, 500));
        }

        #[test]
        fn shift_positive_offset_adds_seconds() {
            let (mut s, mut n) = (100, 0);
            shift(&mut s, &mut n, 5_000_000_000, 1);
            assert_eq!((s, n), (105, 0));
        }

        #[test]
        fn shift_negative_offset_subtracts() {
            let (mut s, mut n) = (100, 0);
            shift(&mut s, &mut n, -2_000_000_000, 1);
            assert_eq!((s, n), (98, 0));
        }

        #[test]
        fn shift_handles_nsec_carry_and_borrow() {
            // 100.6 s + 1.5 s = 102.1 s
            let (mut s, mut n) = (100, 600_000_000);
            shift(&mut s, &mut n, 1_500_000_000, 1);
            assert_eq!((s, n), (102, 100_000_000));
            // 100.2 s - 0.5 s = 99.7 s
            let (mut s, mut n) = (100, 200_000_000);
            shift(&mut s, &mut n, -500_000_000, 1);
            assert_eq!((s, n), (99, 700_000_000));
        }

        #[test]
        fn shift_microsecond_unit() {
            // 100.999999 s + 2 µs = 101.000001 s
            let (mut s, mut us) = (100, 999_999);
            shift(&mut s, &mut us, 2_000, 1_000);
            assert_eq!((s, us), (101, 1));
            // 100.000001 s - 3 µs = 99.999998 s
            let (mut s, mut us) = (100, 1);
            shift(&mut s, &mut us, -3_000, 1_000);
            assert_eq!((s, us), (99, 999_998));
        }

        #[test]
        fn drift_is_elapsed_times_ppm() {
            // 1 s at 100 ppm = 100 µs.
            assert_eq!(drift_offset_ns(1_000_000_000, 100), 100_000);
            assert_eq!(drift_offset_ns(1_000_000_000, -100), -100_000);
            // 1 h at 100 ppm = 360 ms.
            assert_eq!(drift_offset_ns(3_600_000_000_000, 100), 360_000_000);
            // Sub-microsecond elapsed times are not truncated away.
            assert_eq!(drift_offset_ns(10_000, 500_000), 5_000);
            assert_eq!(drift_offset_ns(0, 100), 0);
            // Saturates instead of overflowing.
            assert_eq!(drift_offset_ns(i64::MAX, 2_000_000), i64::MAX);
            assert_eq!(drift_offset_ns(i64::MAX, -2_000_000), i64::MIN);
        }

        #[test]
        fn realtime_skew_is_applied_monotonic_is_not() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            set_env(Some("3000000000"), None); // +3 s
            let real = real_realtime();
            let shifted = shim(CLOCK_REALTIME);
            let diff = ns(&shifted) - ns(&real);
            assert!(
                (2_990_000_000..3_010_000_000).contains(&diff),
                "realtime diff {diff} ns"
            );
            let m1 = shim(CLOCK_MONOTONIC);
            let m_real = real_monotonic_ns();
            let mdiff = i128::from(m_real) - ns(&m1);
            assert!(
                (0..10_000_000).contains(&mdiff),
                "monotonic diff {mdiff} ns"
            );
            set_env(None, None);
        }

        #[test]
        fn realtime_drift_grows_with_elapsed_time() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // 500_000 ppm: the shifted clock runs 1.5x as fast.
            set_env(None, Some("500000"));
            let (r1, s1) = (real_realtime(), shim(CLOCK_REALTIME));
            std::thread::sleep(Duration::from_millis(200));
            let (r2, s2) = (real_realtime(), shim(CLOCK_REALTIME));
            let real_delta = ns(&r2) - ns(&r1);
            let shim_delta = ns(&s2) - ns(&s1);
            let extra = shim_delta - real_delta;
            let expected = real_delta / 2;
            assert!(
                (extra - expected).abs() < 5_000_000,
                "real {real_delta} ns, shim {shim_delta} ns, extra {extra} ns, expected {expected} ns"
            );
            set_env(None, None);
        }

        #[test]
        fn small_drift_does_not_jump() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // 100 ppm: the offset is bounded by elapsed time since the
            // drift start times 1e-4 (the old formula based on the time
            // since the epoch gave a fixed ~176 s here).
            set_env(None, Some("100"));
            let real = real_realtime();
            let shifted = shim(CLOCK_REALTIME);
            let start = *DRIFT_START_NS.get_or_init(real_monotonic_ns);
            let bound = drift_offset_ns(real_monotonic_ns() - start, 100);
            let diff = ns(&shifted) - ns(&real);
            // 100 µs margin for the gap between the two reads.
            assert!(
                (0..=i128::from(bound) + 100_000).contains(&diff),
                "diff {diff} ns, bound {bound} ns"
            );
            set_env(None, None);
        }

        #[test]
        fn gettimeofday_applies_skew_and_drift() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            let tv_us = |v: &Timeval| i128::from(v.tv_sec) * 1_000_000 + i128::from(v.tv_usec);
            let call = |real: bool| {
                let mut v = Timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                };
                // SAFETY: valid out-pointer, tz may be null.
                let r = unsafe {
                    if real {
                        real_gettimeofday()(&mut v, core::ptr::null_mut())
                    } else {
                        gettimeofday(&mut v, core::ptr::null_mut())
                    }
                };
                assert_eq!(r, 0);
                v
            };

            set_env(Some("-2000000000"), None); // -2 s
            let diff = tv_us(&call(false)) - tv_us(&call(true));
            assert!(
                (-2_010_000..-1_990_000).contains(&diff),
                "skew diff {diff} µs"
            );

            set_env(None, Some("500000"));
            let (r1, s1) = (call(true), call(false));
            std::thread::sleep(Duration::from_millis(200));
            let (r2, s2) = (call(true), call(false));
            let extra = (tv_us(&s2) - tv_us(&s1)) - (tv_us(&r2) - tv_us(&r1));
            let expected = (tv_us(&r2) - tv_us(&r1)) / 2;
            assert!(
                (extra - expected).abs() < 5_000,
                "drift extra {extra} µs, expected {expected} µs"
            );
            set_env(None, None);
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

// Stub for non-Linux: the lib compiles but has no no_mangle
// symbols.
#[cfg(not(target_os = "linux"))]
mod stub {
    /// On non-Linux the crate is a no-op stub.
    pub fn unsupported() -> &'static str {
        "chaos-clock-skew is Linux-only"
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn stub_advertises_unsupported() {
            assert!(unsupported().contains("Linux-only"));
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub use stub::*;
