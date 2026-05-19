//! LD_PRELOAD-Shim fuer Clock-Skew-Chaos (WP 5.F.2 Phase-B).
//!
//! Crate `chaos-clock-skew`. Safety classification: **STANDARD**
//! (FFI-Boundary, dlsym-basiert).
//!
//! Setzt `clock_gettime()` und `gettimeofday()` so um, dass eine
//! konfigurierbare Drift addiert wird. Die Drift kommt aus Env-
//! Variablen die der Test-Harness setzt:
//!
//! * `CHAOS_CLOCK_SKEW_NS` — fixe Offset in Nanosekunden (signed,
//!   negative Werte = Clock laeuft hinten).
//! * `CHAOS_CLOCK_DRIFT_PPM` — kontinuierliche Drift in
//!   parts-per-million (z.B. `100` = 100 µs/s langsamer).
//!
//! Aktivierung:
//!
//! ```bash
//! cargo build -p chaos-clock-skew --release
//! LD_PRELOAD=$(pwd)/target/release/libchaos_clock_skew.so \
//!   CHAOS_CLOCK_SKEW_NS=100000000 \
//!   ./my-process
//! ```
//!
//! Auf macOS/Windows kompiliert die Crate nicht (kein dlsym-Pendant
//! fuer system clock). Fuer den CI laeuft sie nur auf Linux.
//!
//! # Was wird NICHT abgefangen?
//!
//! * `CLOCK_MONOTONIC_RAW` — bleibt unveraendert (nur fuer Bench-Tools
//!   wichtig).
//! * `CLOCK_BOOTTIME` — bleibt unveraendert.
//! * Direct-syscall `clock_gettime` ueber `vDSO`-Bypass —
//!   `LD_PRELOAD` greift bei vDSO-Calls nur wenn das Programm explizit
//!   ueber libc callt; das ist bei stdlib-Code der Fall.

#![warn(missing_docs)]
#![allow(clippy::missing_safety_doc)]
// Init-time panics akzeptieren: ohne dlsym(RTLD_NEXT) ist die
// LD_PRELOAD-Lib funktional tot, also ist Process-Termination das
// einzig sinnvolle Fail-Mode. Das ist Comfort-Tool-Code, kein
// Runtime-Pfad.
#![allow(clippy::panic)]

#[cfg(target_os = "linux")]
mod linux {
    use core::ffi::{c_int, c_void};
    use core::ptr;
    use std::sync::OnceLock;

    /// Linux-`timespec`. Layout glibc-kompatibel.
    #[repr(C)]
    pub struct Timespec {
        /// Sekunden seit Epoch.
        pub tv_sec: i64,
        /// Nanosekunden, 0..1_000_000_000.
        pub tv_nsec: i64,
    }

    /// Linux-`timeval`. Layout glibc-kompatibel.
    #[repr(C)]
    pub struct Timeval {
        /// Sekunden.
        pub tv_sec: i64,
        /// Microsekunden.
        pub tv_usec: i64,
    }

    /// `clockid_t`-Enum, glibc-kompatibel.
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
            // SAFETY: dlsym mit RTLD_NEXT liefert die echte glibc-
            // Implementation. Pointer-Cast auf Funktionstyp gleicher
            // Signatur ist kontraktgemaess.
            unsafe {
                let p = libc_stub::dlsym(libc_stub::RTLD_NEXT, c"clock_gettime".as_ptr());
                if p.is_null() {
                    // Fallback: gar nichts auflösen, Crashes lassen
                    // wir kontrolliert bei Aufruf.
                    panic!("dlsym(clock_gettime) failed");
                }
                core::mem::transmute::<*mut c_void, ClockGettimeFn>(p)
            }
        })
    }

    fn real_gettimeofday() -> GetTimeOfDayFn {
        *REAL_GETTIMEOFDAY.get_or_init(|| {
            // SAFETY: gleiche Begruendung wie real_clock_gettime.
            unsafe {
                let p = libc_stub::dlsym(libc_stub::RTLD_NEXT, c"gettimeofday".as_ptr());
                if p.is_null() {
                    panic!("dlsym(gettimeofday) failed");
                }
                core::mem::transmute::<*mut c_void, GetTimeOfDayFn>(p)
            }
        })
    }

    /// Gibt den fixen Skew-Offset (ns) zurueck. Wird bei jedem Call
    /// gelesen, damit Tests den Wert zur Laufzeit aendern koennen.
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

    /// Wendet Skew + Drift auf einen Timespec an.
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

    /// `clock_gettime`-Replacement.
    ///
    /// # Safety
    /// `tp` muss valid sein.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn clock_gettime(clk_id: ClockId, tp: *mut Timespec) -> c_int {
        // SAFETY: real_clock_gettime() liefert die echte glibc-
        // Implementation; die Aufruf-Semantik ist signaturgleich.
        let r = unsafe { real_clock_gettime()(clk_id, tp) };
        if r != 0 || tp.is_null() {
            return r;
        }
        // Skew nur fuer CLOCK_REALTIME — MONOTONIC bleibt unveraendert.
        if clk_id == CLOCK_REALTIME {
            // SAFETY: r==0, tp wurde von der echten Funktion befuellt.
            let ts = unsafe { &mut *tp };
            // Drift: monotonic_ns ist die Wallclock seit Boot.
            // Naeherung: ts.tv_sec * 1e9 + tv_nsec (ist seit-Epoch
            // statt seit-Boot, aber die PPM-Drift ist gleichmaessig).
            let monotonic_ns = ts.tv_sec.saturating_mul(1_000_000_000) + ts.tv_nsec;
            apply_skew(ts, monotonic_ns);
        }
        r
    }

    /// `gettimeofday`-Replacement.
    ///
    /// # Safety
    /// `tv` muss valid sein.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gettimeofday(tv: *mut Timeval, tz: *mut c_void) -> c_int {
        // SAFETY: real_gettimeofday() liefert die echte glibc-
        // Implementation; signaturkompatibler Pass-Through.
        let r = unsafe { real_gettimeofday()(tv, tz) };
        if r != 0 || tv.is_null() {
            return r;
        }
        // Skew anwenden.
        let skew = skew_ns();
        if skew == 0 {
            return r;
        }
        // SAFETY: r==0 → tv wurde befuellt.
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
        // Schlucke unused tz.
        let _ = tz;
        let _ = ptr::eq::<()>(ptr::null(), ptr::null());
        r
    }

    mod libc_stub {
        use core::ffi::{c_char, c_void};
        unsafe extern "C" {
            pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        }
        // RTLD_NEXT auf Linux glibc = -1 als void*.
        pub const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::Mutex;

        /// cargo test laeuft multi-threaded by default; CHAOS_CLOCK_SKEW_NS
        /// und CHAOS_CLOCK_DRIFT_PPM sind process-global env-vars, daher
        /// muessen alle Tests die diese setzen den selben Mutex halten.
        /// Pattern: `let _g = ENV_LOCK.lock()...` am Anfang jedes Tests.
        static ENV_LOCK: Mutex<()> = Mutex::new(());

        #[test]
        fn apply_skew_zero_is_noop() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // SAFETY: env-mutation unter ENV_LOCK serialisiert.
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
            // SAFETY: env-mutation unter ENV_LOCK serialisiert.
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
            // SAFETY: env-cleanup im single-threaded scope.
            unsafe { std::env::remove_var("CHAOS_CLOCK_SKEW_NS") };
        }

        #[test]
        fn apply_skew_negative_offset_subtracts() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // SAFETY: env-mutation unter ENV_LOCK serialisiert.
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
            // SAFETY: env-cleanup im single-threaded scope.
            unsafe { std::env::remove_var("CHAOS_CLOCK_SKEW_NS") };
        }

        #[test]
        fn apply_skew_handles_nsec_carry() {
            let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            // SAFETY: env-mutation unter ENV_LOCK serialisiert.
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
            // SAFETY: env-cleanup im single-threaded scope.
            unsafe { std::env::remove_var("CHAOS_CLOCK_SKEW_NS") };
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

// Stub fuer Nicht-Linux: lib kompiliert, hat aber keine no_mangle-
// Symbole.
#[cfg(not(target_os = "linux"))]
mod stub {
    /// Auf Nicht-Linux ist die Crate ein No-Op-Stub.
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
