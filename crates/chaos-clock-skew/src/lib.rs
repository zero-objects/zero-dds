//! LD_PRELOAD shim for clock-skew chaos (WP 5.F.2 Phase-B).
//!
//! Crate `chaos-clock-skew`. Safety classification: **STANDARD**
//! (FFI boundary, dlsym-based).
//!
//! Overrides `clock_gettime()` and `gettimeofday()` so that a
//! configurable drift is added. The drift comes from env variables
//! set by the test harness:
//!
//! * `CHAOS_CLOCK_SKEW_NS` — fixed offset in nanoseconds (signed,
//!   negative values = clock runs behind).
//! * `CHAOS_CLOCK_DRIFT_PPM` — continuous drift in
//!   parts-per-million (e.g. `100` = 100 µs/s slower).
//!
//! Activation:
//!
//! ```bash
//! cargo build -p chaos-clock-skew --release
//! LD_PRELOAD=$(pwd)/target/release/libchaos_clock_skew.so \
//!   CHAOS_CLOCK_SKEW_NS=100000000 \
//!   ./my-process
//! ```
//!
//! The crate does not compile on macOS/Windows (no dlsym equivalent
//! for the system clock). In CI it runs only on Linux.
//!
//! # What is NOT intercepted?
//!
//! * `CLOCK_MONOTONIC_RAW` — left unchanged (only relevant for bench
//!   tools).
//! * `CLOCK_BOOTTIME` — left unchanged.
//! * Direct-syscall `clock_gettime` via `vDSO` bypass —
//!   `LD_PRELOAD` only takes effect on vDSO calls when the program
//!   explicitly calls through libc; that is the case for stdlib code.

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
    use core::ptr;
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

    type ClockGettimeFn = unsafe extern "C" fn(ClockId, *mut Timespec) -> c_int;
    type GetTimeOfDayFn = unsafe extern "C" fn(*mut Timeval, *mut c_void) -> c_int;

    static REAL_CLOCK_GETTIME: OnceLock<ClockGettimeFn> = OnceLock::new();
    static REAL_GETTIMEOFDAY: OnceLock<GetTimeOfDayFn> = OnceLock::new();

    fn real_clock_gettime() -> ClockGettimeFn {
        *REAL_CLOCK_GETTIME.get_or_init(|| {
            // SAFETY: dlsym with RTLD_NEXT returns the real glibc
            // implementation. Casting the pointer to a function type
            // with the same signature is contractually correct.
            unsafe {
                let p = libc_stub::dlsym(libc_stub::RTLD_NEXT, c"clock_gettime".as_ptr());
                if p.is_null() {
                    // Fallback: resolve nothing, let crashes happen
                    // in a controlled way at call time.
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

    /// Applies skew + drift to a Timespec.
    fn apply_skew(ts: &mut Timespec, monotonic_ns: i64) {
        let skew = skew_ns();
        let drift = drift_ppm();
        let drift_offset_ns = if drift != 0 {
            // PPM = ns/sec/1e6; absolute drift = elapsed_ns * ppm / 1e6.
            (monotonic_ns / 1_000_000) * drift / 1_000
        } else {
            0
        };
        let total = skew.saturating_add(drift_offset_ns);
        let total_sec = total / 1_000_000_000;
        let total_nsec = total % 1_000_000_000;
        ts.tv_sec = ts.tv_sec.saturating_add(total_sec);
        let mut new_nsec = ts.tv_nsec.saturating_add(total_nsec);
        if new_nsec >= 1_000_000_000 {
            ts.tv_sec = ts.tv_sec.saturating_add(1);
            new_nsec -= 1_000_000_000;
        }
        if new_nsec < 0 {
            ts.tv_sec = ts.tv_sec.saturating_sub(1);
            new_nsec += 1_000_000_000;
        }
        ts.tv_nsec = new_nsec;
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
            // SAFETY: r==0, tp was filled by the real function.
            let ts = unsafe { &mut *tp };
            // Drift: monotonic_ns is the wallclock since boot.
            // Approximation: ts.tv_sec * 1e9 + tv_nsec (this is
            // since-epoch rather than since-boot, but the PPM drift
            // is uniform).
            let monotonic_ns = ts.tv_sec.saturating_mul(1_000_000_000) + ts.tv_nsec;
            apply_skew(ts, monotonic_ns);
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
        // Apply skew.
        let skew = skew_ns();
        if skew == 0 {
            return r;
        }
        // SAFETY: r==0 → tv was filled.
        let v = unsafe { &mut *tv };
        let total_us = skew / 1_000;
        let total_sec = total_us / 1_000_000;
        let total_us_rem = total_us % 1_000_000;
        v.tv_sec = v.tv_sec.saturating_add(total_sec);
        let mut new_us = v.tv_usec.saturating_add(total_us_rem);
        if new_us >= 1_000_000 {
            v.tv_sec = v.tv_sec.saturating_add(1);
            new_us -= 1_000_000;
        }
        if new_us < 0 {
            v.tv_sec = v.tv_sec.saturating_sub(1);
            new_us += 1_000_000;
        }
        v.tv_usec = new_us;
        // Swallow unused tz.
        let _ = tz;
        let _ = ptr::eq::<()>(ptr::null(), ptr::null());
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

        /// cargo test runs multi-threaded by default; CHAOS_CLOCK_SKEW_NS
        /// and CHAOS_CLOCK_DRIFT_PPM are process-global env vars, so all
        /// tests that set them must hold the same mutex.
        /// Pattern: `let _g = ENV_LOCK.lock()...` at the start of each test.
        static ENV_LOCK: Mutex<()> = Mutex::new(());

        #[test]
        fn apply_skew_zero_is_noop() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // SAFETY: env mutation serialized under ENV_LOCK.
            unsafe {
                std::env::remove_var("CHAOS_CLOCK_SKEW_NS");
                std::env::remove_var("CHAOS_CLOCK_DRIFT_PPM");
            }
            let mut ts = Timespec {
                tv_sec: 100,
                tv_nsec: 500,
            };
            apply_skew(&mut ts, 100_000_000_000);
            assert_eq!(ts.tv_sec, 100);
            assert_eq!(ts.tv_nsec, 500);
        }

        #[test]
        fn apply_skew_positive_offset_adds_seconds() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // SAFETY: env mutation serialized under ENV_LOCK.
            unsafe {
                std::env::set_var("CHAOS_CLOCK_SKEW_NS", "5000000000"); // +5s
                std::env::remove_var("CHAOS_CLOCK_DRIFT_PPM");
            }
            let mut ts = Timespec {
                tv_sec: 100,
                tv_nsec: 0,
            };
            apply_skew(&mut ts, 0);
            assert_eq!(ts.tv_sec, 105);
            // SAFETY: env cleanup in single-threaded scope.
            unsafe { std::env::remove_var("CHAOS_CLOCK_SKEW_NS") };
        }

        #[test]
        fn apply_skew_negative_offset_subtracts() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // SAFETY: env mutation serialized under ENV_LOCK.
            unsafe {
                std::env::set_var("CHAOS_CLOCK_SKEW_NS", "-2000000000"); // -2s
                std::env::remove_var("CHAOS_CLOCK_DRIFT_PPM");
            }
            let mut ts = Timespec {
                tv_sec: 100,
                tv_nsec: 0,
            };
            apply_skew(&mut ts, 0);
            assert_eq!(ts.tv_sec, 98);
            // SAFETY: env cleanup in single-threaded scope.
            unsafe { std::env::remove_var("CHAOS_CLOCK_SKEW_NS") };
        }

        #[test]
        fn apply_skew_handles_nsec_carry() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // SAFETY: env mutation serialized under ENV_LOCK.
            unsafe {
                std::env::set_var("CHAOS_CLOCK_SKEW_NS", "1500000000"); // +1.5s
                std::env::remove_var("CHAOS_CLOCK_DRIFT_PPM");
            }
            let mut ts = Timespec {
                tv_sec: 100,
                tv_nsec: 600_000_000,
            };
            apply_skew(&mut ts, 0);
            // 100s + 0.6s + 1.5s = 102s + 0.1s
            assert_eq!(ts.tv_sec, 102);
            assert_eq!(ts.tv_nsec, 100_000_000);
            // SAFETY: env cleanup in single-threaded scope.
            unsafe { std::env::remove_var("CHAOS_CLOCK_SKEW_NS") };
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
