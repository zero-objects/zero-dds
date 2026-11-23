// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DCPS time source adapter (C2(6)).
//!
//! [`crate::time::get_current_time`] is the wall clock behind
//! SOURCE_TIMESTAMP (a `write()` with no explicit `write_w_timestamp`
//! timestamp), the `register_instance`/`unregister_instance`/`dispose`
//! defaults, the reader's reception clock (LIFESPAN purge and
//! DESTINATION_ORDER=BY_RECEPTION_TIMESTAMP), and DESTINATION_ORDER=
//! BY_SOURCE_TIMESTAMP tie-breaking (which compares whatever
//! `SOURCE_TIMESTAMP` the writer stamped). This module makes that clock
//! pluggable, wired through [`crate::runtime::RuntimeConfig::time_source`]
//! (config key `time.source`, `man/man5/zerodds.yaml.5`).
//!
//! **DEADLINE is deliberately not routed through this module.** Its
//! monitor (`crate::runtime::check_deadlines`) already runs on
//! [`std::time::Instant`]-derived elapsed durations, not the wall clock
//! — the correct choice for a *relative* period, and one that stays
//! correct across a wall-clock jump. Rerouting it through a `TimeSource`
//! would reintroduce exactly the fragility the existing design avoids;
//! LIFESPAN's *expiration time* is a different, spec-mandated
//! wall-clock quantity (DDS 1.4 §2.2.3.16: "computed by adding the
//! duration ... to the source timestamp") and is the one this module
//! covers.
//!
//! [`TimeServiceClock`] is the adapter to `zerodds-time-service`'s OMG
//! Time Service 1.1 `TimeService`/`Uto` (§1.3.4): its `inaccuracy`/`tdf`
//! fields have no equivalent in DDS-DCPS `Time_t` (DDS-DCPS 1.4 §2.3.3
//! carries neither), so they are not silently dropped on the way into
//! [`crate::time::Time`] — [`TimeSource::now_uto`] keeps them available
//! to a caller that wants OMG Time Service semantics (comparison with an
//! inaccuracy envelope via `Uto::compare_time`), while [`TimeSource::now`]
//! is what the RTPS/QoS hot path actually consumes.

extern crate alloc;

use alloc::sync::Arc;
use core::fmt;

use zerodds_time_service::time_base::{TICKS_PER_SECOND, UTC_EPOCH_TO_UNIX_TICKS, local_tdf};
use zerodds_time_service::{TimeT, UtcT, Uto};

use crate::time::Time;

/// A pluggable wall-clock source for the DCPS layer. Default
/// ([`RuntimeConfig::time_source`](crate::runtime::RuntimeConfig)'s
/// default) is [`SystemClock`] — behaviourally identical to the
/// unconditional [`crate::time::get_current_time`] calls this replaced.
pub trait TimeSource: Send + Sync + fmt::Debug {
    /// Current time as DCPS `Time_t` (DDS-DCPS 1.4 §2.3.3).
    fn now(&self) -> Time;

    /// The same instant as an OMG Time Service 1.1 `Uto` (§1.3.4),
    /// carrying `inaccuracy`/`tdf` when the source knows them. Sources
    /// with no inaccuracy estimate (the system clock, `CLOCK_TAI`) report
    /// zero inaccuracy; the `tdf` is the displacement of the local time
    /// zone at that instant, which §1.3.2.4 requires of every absolute
    /// UTO. [`TimeServiceClock`] overrides this with the wrapped
    /// `TimeService`'s real `Uto`.
    fn now_uto(&self) -> Uto {
        let ticks = time_to_ticks(self.now());
        Uto::from_utc(UtcT {
            time: ticks,
            inacclo: 0,
            inacchi: 0,
            tdf: local_tdf(ticks),
        })
    }
}

/// `Time_t` (seconds + nanoseconds since 1970-01-01) -> `TimeBase::TimeT`
/// (100ns ticks since 1582-10-15, the OMG Time Service epoch). Sub-100ns
/// precision is dropped (`Time_t`'s own resolution is 1ns; `TimeT`'s is
/// 100ns) — the same rounding [`ticks_to_time`] reverses.
#[must_use]
pub fn time_to_ticks(t: Time) -> TimeT {
    let sec_ticks = i64::from(t.sec).saturating_mul(TICKS_PER_SECOND as i64);
    let sub_ticks = i64::from(t.nanosec / 100);
    let unix_ticks = sec_ticks.saturating_add(sub_ticks);
    if unix_ticks >= 0 {
        UTC_EPOCH_TO_UNIX_TICKS.saturating_add(unix_ticks as u64)
    } else {
        UTC_EPOCH_TO_UNIX_TICKS.saturating_sub(unix_ticks.unsigned_abs())
    }
}

/// Inverse of [`time_to_ticks`]. A `ticks` before the 1970 epoch
/// saturates to `Time { sec: 0, nanosec: 0 }`; one past `i32::MAX`
/// seconds saturates to `i32::MAX` (the same overflow convention
/// [`crate::time::get_current_time`] already uses).
#[must_use]
pub fn ticks_to_time(ticks: TimeT) -> Time {
    let unix_ticks = ticks.saturating_sub(UTC_EPOCH_TO_UNIX_TICKS);
    let sec = i32::try_from(unix_ticks / TICKS_PER_SECOND).unwrap_or(i32::MAX);
    let nanosec = ((unix_ticks % TICKS_PER_SECOND) * 100) as u32;
    Time::new(sec, nanosec)
}

/// The default source: `std::time::SystemTime`, via
/// [`crate::time::get_current_time`]. Identical output to every
/// unconditional `get_current_time()`/`SystemTime::now()` call this
/// replaced.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl TimeSource for SystemClock {
    fn now(&self) -> Time {
        crate::time::get_current_time()
    }
}

/// A deterministic, manually-advanced clock — for tests that must drive
/// DESTINATION_ORDER=BY_SOURCE_TIMESTAMP (or LIFESPAN expiry) without a
/// wall-clock race. `now()` never advances on its own; `set`/`advance`
/// move it. Cloning shares the same underlying instant.
#[derive(Debug, Clone)]
pub struct TestClock {
    nanos_since_epoch: Arc<core::sync::atomic::AtomicI64>,
}

impl TestClock {
    /// A clock starting at `start`.
    #[must_use]
    pub fn new(start: Time) -> Self {
        Self {
            nanos_since_epoch: Arc::new(core::sync::atomic::AtomicI64::new(time_to_nanos(start))),
        }
    }

    /// Sets the clock to exactly `t`.
    pub fn set(&self, t: Time) {
        self.nanos_since_epoch
            .store(time_to_nanos(t), core::sync::atomic::Ordering::SeqCst);
    }

    /// Advances the clock by `d` (nanosecond precision).
    pub fn advance(&self, d: crate::time::Duration) {
        let delta = i64::from(d.sec).saturating_mul(1_000_000_000) + i64::from(d.nanosec);
        self.nanos_since_epoch
            .fetch_add(delta, core::sync::atomic::Ordering::SeqCst);
    }
}

impl TimeSource for TestClock {
    fn now(&self) -> Time {
        nanos_to_time(
            self.nanos_since_epoch
                .load(core::sync::atomic::Ordering::SeqCst),
        )
    }
}

fn time_to_nanos(t: Time) -> i64 {
    i64::from(t.sec)
        .saturating_mul(1_000_000_000)
        .saturating_add(i64::from(t.nanosec))
}

fn nanos_to_time(n: i64) -> Time {
    let sec = i32::try_from(n.div_euclid(1_000_000_000)).unwrap_or(if n < 0 {
        i32::MIN
    } else {
        i32::MAX
    });
    let nanosec = n.rem_euclid(1_000_000_000) as u32;
    Time::new(sec, nanosec)
}

/// The Linux `CLOCK_TAI` source (DDS-DCPS carries no epoch/scale marker
/// of its own, so a peer reading these timestamps must agree
/// out-of-band that they are TAI, not UTC). `now()` reports
/// [`Time::INVALID`] if the read fails (e.g. no leap-second table
/// loaded) — never a silently-wrong value.
#[cfg(all(feature = "std", target_os = "linux"))]
#[derive(Debug, Default, Clone, Copy)]
pub struct TaiClock;

#[cfg(all(feature = "std", target_os = "linux"))]
impl TimeSource for TaiClock {
    fn now(&self) -> Time {
        tai_now().unwrap_or(Time::INVALID)
    }
}

#[cfg(all(feature = "std", target_os = "linux"))]
#[allow(unsafe_code)]
fn tai_now() -> Option<Time> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable `timespec`; `CLOCK_TAI` is a
    // compile-time clock id; POSIX `clock_gettime` writes only
    // `ts.tv_sec`/`ts.tv_nsec` and returns a status code.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_TAI, &mut ts) };
    if rc != 0 {
        return None;
    }
    Some(Time::new(
        i32::try_from(ts.tv_sec).unwrap_or(i32::MAX),
        u32::try_from(ts.tv_nsec).unwrap_or(0),
    ))
}

/// The kernel clock of Linux with the state of its synchronization, as
/// a secure source of the OMG Time Service 1.1 (§1.1.3, §2.1.2, App. A):
/// `zerodds_time_service::admin::TimeAdmin::designate_source` makes it the
/// authoritative source of an administered `TimeService`.
///
/// One read-only `adjtimex(2)` call (`modes` 0, no privilege needed)
/// gives the time, the kernel's maximum error (`maxerror`) and the state
/// of synchronization from the same instant. The kernel clock counts as
/// synchronized while the process that disciplines it (chrony, ntpd,
/// ptp4l with phc2sys) keeps `STA_UNSYNC` and `STA_CLOCKERR` clear, the
/// call does not return `TIME_ERROR` and no leap second is in progress
/// (`TIME_OOP`). How that process is protected in transit is not visible
/// here: the administrator states it with the designation
/// (`SyncProtection`).
#[cfg(all(feature = "std", target_os = "linux"))]
#[derive(Debug, Default, Clone, Copy)]
pub struct KernelClock;

/// The fields of one read-only `adjtimex(2)` call that [`KernelClock`]
/// uses.
#[cfg(all(feature = "std", target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelTimex {
    /// The return value: the clock state (`TIME_OK` 0 to `TIME_ERROR` 5).
    pub state: i32,
    /// `status`: the `STA_*` bits.
    pub status: i32,
    /// `maxerror` in microseconds.
    pub maxerror_us: i64,
    /// `time.tv_sec`, seconds since 1970.
    pub sec: i64,
    /// `time.tv_usec`: microseconds, or nanoseconds with `STA_NANO`.
    pub subsec: i64,
}

#[cfg(all(feature = "std", target_os = "linux"))]
mod kernel_timex {
    //! The values of `<linux/timex.h>` (kernel ABI).
    pub const STA_UNSYNC: i32 = 0x0040;
    pub const STA_CLOCKERR: i32 = 0x1000;
    pub const STA_NANO: i32 = 0x2000;
    pub const TIME_OOP: i32 = 3;
    pub const TIME_ERROR: i32 = 5;
}

/// The secure-source reading of one `adjtimex(2)` result: time in
/// 100 ns ticks since 1582, inaccuracy `maxerror` in 100 ns, and whether
/// the clock is synchronized ([`KernelClock`]).
///
/// # Errors
/// `TimeUnavailable` for a time before 1970 or a negative `maxerror`.
#[cfg(all(feature = "std", target_os = "linux"))]
pub fn kernel_reading(
    k: KernelTimex,
) -> Result<zerodds_time_service::secure::SourceReading, zerodds_time_service::TimeUnavailable> {
    use kernel_timex::{STA_CLOCKERR, STA_NANO, STA_UNSYNC, TIME_ERROR, TIME_OOP};
    use zerodds_time_service::TimeUnavailable;
    let nanos = if k.status & STA_NANO != 0 {
        k.subsec
    } else {
        k.subsec.saturating_mul(1_000)
    };
    let sec = u64::try_from(k.sec).map_err(|_| TimeUnavailable)?;
    let nanos = u64::try_from(nanos).map_err(|_| TimeUnavailable)?;
    let time = sec
        .checked_mul(TICKS_PER_SECOND)
        .and_then(|t| t.checked_add(nanos / 100))
        .and_then(|t| t.checked_add(UTC_EPOCH_TO_UNIX_TICKS))
        .ok_or(TimeUnavailable)?;
    let inaccuracy = u64::try_from(k.maxerror_us)
        .map_err(|_| TimeUnavailable)?
        .saturating_mul(10);
    let synchronized =
        k.state != TIME_ERROR && k.state != TIME_OOP && k.status & (STA_UNSYNC | STA_CLOCKERR) == 0;
    Ok(zerodds_time_service::secure::SourceReading {
        time,
        inaccuracy,
        synchronized,
    })
}

#[cfg(all(feature = "std", target_os = "linux"))]
#[allow(unsafe_code)]
// `c_long` and `time_t` are `i64` on 64-bit targets and `i32` on 32-bit
// ones: `i64::from` is a no-op only on the former.
#[allow(clippy::useless_conversion)]
fn adjtimex_read() -> Option<KernelTimex> {
    // SAFETY: `timex` is a plain C struct of integers; all zeroes is a
    // valid value of it (`modes` 0: read only).
    let mut tx: libc::timex = unsafe { core::mem::zeroed() };
    // SAFETY: `tx` is a valid, writable `timex`; with `modes` 0
    // `adjtimex` only writes the current values into it and returns the
    // clock state or -1.
    let state = unsafe { libc::adjtimex(&mut tx) };
    if state < 0 {
        return None;
    }
    Some(KernelTimex {
        state,
        status: tx.status,
        maxerror_us: i64::from(tx.maxerror),
        sec: i64::from(tx.time.tv_sec),
        subsec: i64::from(tx.time.tv_usec),
    })
}

#[cfg(all(feature = "std", target_os = "linux"))]
impl zerodds_time_service::secure::SecureTimeSource for KernelClock {
    fn read(
        &self,
    ) -> Result<zerodds_time_service::secure::SourceReading, zerodds_time_service::TimeUnavailable>
    {
        kernel_reading(adjtimex_read().ok_or(zerodds_time_service::TimeUnavailable)?)
    }
}

/// Adapter from `zerodds-time-service`'s `TimeService` (OMG Time
/// Service 1.1 §2.1) to the DCPS [`TimeSource`] — the "DCPS time
/// source adapter" C2(6) asks for. Build the wrapped `TimeService` with
/// `TimeService::with_source(...)` for a disciplined clock under
/// `no_std`, or leave it at `TimeService::default()` for the `std`
/// system clock; either way [`Self::now_uto`] returns the service's
/// real `Uto`, inaccuracy/tdf included.
#[derive(Clone)]
pub struct TimeServiceClock {
    service: Arc<zerodds_time_service::TimeService>,
}

impl TimeServiceClock {
    /// Wraps `service`.
    #[must_use]
    pub fn new(service: zerodds_time_service::TimeService) -> Self {
        Self {
            service: Arc::new(service),
        }
    }
}

impl fmt::Debug for TimeServiceClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimeServiceClock").finish_non_exhaustive()
    }
}

impl TimeSource for TimeServiceClock {
    fn now(&self) -> Time {
        match self.service.universal_time() {
            Ok(uto) => ticks_to_time(uto.time()),
            Err(_) => Time::INVALID,
        }
    }

    fn now_uto(&self) -> Uto {
        // Unavailable: time 0 (no absolute time, so no tdf), with the
        // service's error estimate, which is at most 48 bits.
        self.service.universal_time().unwrap_or_else(|_| {
            Uto::from_utc(UtcT::new(0, self.service.inaccuracy(), 0).unwrap_or_default())
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ticks_roundtrip_rounds_down_to_100ns() {
        // 123 ns rounds down to the nearest 100ns tick (23ns dropped),
        // the same lossy direction UtcT::to_wire already accepts.
        let t = Time::new(10, 123);
        assert_eq!(ticks_to_time(time_to_ticks(t)), Time::new(10, 100));
    }

    #[test]
    fn ticks_before_1970_saturate_to_zero() {
        // TimeT is unsigned; a tick count before the Unix epoch clamps.
        assert_eq!(ticks_to_time(0), Time::new(0, 0));
    }

    #[test]
    fn test_clock_set_and_advance() {
        let clock = TestClock::new(Time::new(0, 0));
        clock.set(Time::new(50, 0));
        assert_eq!(clock.now(), Time::new(50, 0));
        clock.advance(crate::time::Duration::new(1, 500_000_000));
        assert_eq!(clock.now(), Time::new(51, 500_000_000));
    }

    /// The environment variable that turns [`now_uto_probe_child`] on and
    /// carries the second of its clock.
    const NOW_UTO_PROBE_ENV: &str = "ZERODDS_DCPS_NOW_UTO_PROBE";

    /// 2026-01-15T12:00:00Z (winter on the northern hemisphere).
    const JANUARY: i32 = 1_768_478_400;

    /// 2026-07-15T12:00:00Z (summer), 181 days after [`JANUARY`].
    const JULY: i32 = JANUARY + 181 * 86_400;

    /// Runs only as the child process [`probe_now_uto`] starts; in a
    /// normal run the variable is absent and it returns at once. Prints
    /// the inaccuracy and the tdf of the default `now_uto` of a clock at
    /// the given second.
    #[test]
    #[allow(clippy::print_stdout)]
    fn now_uto_probe_child() {
        let Ok(second) = std::env::var(NOW_UTO_PROBE_ENV) else {
            return;
        };
        let second: i32 = second.parse().expect("second");
        let uto = TestClock::new(Time::new(second, 0)).now_uto();
        assert_eq!(uto.time(), time_to_ticks(Time::new(second, 0)));
        println!("ZERODDS-NOW-UTO {} {}", uto.inaccuracy(), uto.tdf());
    }

    /// Starts this test binary as a child with `TZ=tz` (`None`: without
    /// `TZ`, so the zone of the system applies) and returns the
    /// inaccuracy and the tdf the child's `now_uto` carried at `second`.
    fn probe_now_uto(tz: Option<&str>, second: i32) -> (u64, zerodds_time_service::TdfT) {
        let exe = std::env::current_exe().expect("test binary");
        let mut child = std::process::Command::new(exe);
        child
            .args([
                "--exact",
                "time_source::tests::now_uto_probe_child",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(NOW_UTO_PROBE_ENV, alloc::format!("{second}"));
        match tz {
            Some(tz) => child.env("TZ", tz),
            None => child.env_remove("TZ"),
        };
        let out = child.output().expect("start the probe");
        let stdout = alloc::string::String::from_utf8_lossy(&out.stdout);
        let line = stdout
            .lines()
            .find_map(|l| {
                l.split_once("ZERODDS-NOW-UTO ")
                    .map(|(_, rest)| rest.trim())
            })
            .unwrap_or_else(|| panic!("TZ={tz:?}: no probe line in {stdout:?}"));
        let (inaccuracy, tdf) = line.split_once(' ').expect("two values");
        (
            inaccuracy.parse().expect("inaccuracy"),
            tdf.parse().expect("tdf"),
        )
    }

    #[test]
    fn time_1_3_2_4_now_uto_carries_zero_inaccuracy_and_the_local_tdf() {
        // OMG Time Service 1.1 §1.3.2.4: "Implementations must place the
        // time displacement factor for the local time zone in this field
        // whenever they create a UTO that expresses absolute time." A child
        // process per zone; POSIX TZ rules need no zone database. The
        // daylight saving time is the one at the clock's time.
        let cet = "CET-1CEST,M3.5.0,M10.5.0/3";
        assert_eq!(probe_now_uto(Some(cet), JANUARY), (0, 60));
        assert_eq!(probe_now_uto(Some(cet), JULY), (0, 120));
        let eastern = "EST5EDT,M3.2.0,M11.1.0";
        assert_eq!(probe_now_uto(Some(eastern), JANUARY), (0, -300));
        assert_eq!(probe_now_uto(Some(eastern), JULY), (0, -240));
        assert_eq!(probe_now_uto(Some("UTC0"), JULY), (0, 0));
    }

    /// Without `TZ`, Windows' own zone setting applies: its offset at the
    /// two instants as Windows states it (`TimeZoneInfo.Local`, read with
    /// PowerShell) is the tdf. The test can tell a missing zone (tdf 0)
    /// only when the zone of the machine has an offset at one of them.
    #[cfg(windows)]
    #[test]
    fn time_1_3_2_4_now_uto_carries_the_tdf_of_the_windows_zone() {
        for second in [JANUARY, JULY] {
            let script = alloc::format!(
                "$t = (New-Object DateTime 1970,1,1,0,0,0,([DateTimeKind]::Utc)).AddSeconds({second}); \
                 [int][TimeZoneInfo]::Local.GetUtcOffset($t).TotalMinutes"
            );
            let out = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .expect("start powershell");
            let stated = alloc::string::String::from_utf8_lossy(&out.stdout);
            let stated: zerodds_time_service::TdfT = stated
                .trim()
                .parse()
                .unwrap_or_else(|_| panic!("offset from powershell: {stated:?}"));
            assert_eq!(probe_now_uto(None, second), (0, stated), "at {second}");
        }
    }

    #[test]
    fn time_service_clock_reads_the_wrapped_source() {
        // A realistic recent timestamp (2023-11-14-ish), not an arbitrary
        // large tick count: `Time_t.sec` is `i32` (DDS-DCPS carries the
        // spec's own Y2038-style bound), so a tick count corresponding to
        // a date past ~2038 cannot round-trip through it — that is a
        // property of `Time_t`, not of this adapter.
        let ticks = time_to_ticks(Time::new(1_700_000_000, 0));
        let ts = zerodds_time_service::TimeService::with_source(move || ticks);
        let clock = TimeServiceClock::new(ts);
        let uto = clock.now_uto();
        assert_eq!(uto.time(), ticks);
        assert_eq!(clock.now(), Time::new(1_700_000_000, 0));
    }

    #[test]
    fn time_service_clock_carries_inaccuracy_and_tdf() {
        let ticks = time_to_ticks(Time::new(1_700_000_000, 0));
        let ts = zerodds_time_service::TimeService::with_source(move || ticks)
            .with_inaccuracy(5_000)
            .expect("in range")
            .with_tdf(60);
        let clock = TimeServiceClock::new(ts);
        let uto = clock.now_uto();
        assert_eq!(uto.inaccuracy(), 5_000);
        assert_eq!(uto.tdf(), 60);
    }

    #[cfg(target_os = "linux")]
    fn timex(state: i32, status: i32, maxerror_us: i64, subsec: i64) -> KernelTimex {
        KernelTimex {
            state,
            status,
            maxerror_us,
            sec: i64::from(JULY),
            subsec,
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn time_a_1_kernel_reading_follows_the_adjtimex_state() {
        // OMG Time Service 1.1 §1.1.3: time with its error bound, and
        // whether it can be trusted within it. STA_PLL 0x0001, STA_NANO
        // 0x2000, STA_UNSYNC 0x0040, STA_CLOCKERR 0x1000.
        let july = time_to_ticks(Time::new(JULY, 0));
        let r = kernel_reading(timex(0, 0x2001, 1_500, 123_456_789)).unwrap();
        assert_eq!(r.time, july + 1_234_567);
        assert_eq!(r.inaccuracy, 15_000);
        assert!(r.synchronized);
        // Microseconds without STA_NANO.
        let r = kernel_reading(timex(0, 0x0001, 7, 123_456)).unwrap();
        assert_eq!(r.time, july + 1_234_560);
        assert_eq!(r.inaccuracy, 70);
        assert!(r.synchronized);
        // A leap second announced (TIME_INS 1) is still synchronized.
        assert!(kernel_reading(timex(1, 0x2001, 1, 0)).unwrap().synchronized);
        for (state, status, why) in [
            (0, 0x2041, "STA_UNSYNC"),
            (0, 0x3001, "STA_CLOCKERR"),
            (5, 0x2001, "TIME_ERROR"),
            (3, 0x2001, "TIME_OOP"),
        ] {
            assert!(
                !kernel_reading(timex(state, status, 1, 0))
                    .unwrap()
                    .synchronized,
                "{why}"
            );
        }
        assert!(kernel_reading(timex(0, 0x2001, -1, 0)).is_err());
        assert!(
            kernel_reading(KernelTimex {
                sec: -1,
                ..timex(0, 0x2001, 1, 0)
            })
            .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn time_a_1_kernel_clock_reads_the_host_clock() {
        use zerodds_time_service::secure::SecureTimeSource;
        let before = time_to_ticks(crate::time::get_current_time());
        let r = KernelClock.read().expect("adjtimex(2) read-only");
        let after = time_to_ticks(crate::time::get_current_time());
        // The same clock as the system time, read in between; without
        // STA_NANO the kernel gives microseconds (10 ticks).
        assert!(
            before <= r.time + 10 && r.time <= after,
            "{before} {} {after}",
            r.time
        );
        // The kernel caps maxerror at 16 s (NTP_PHASE_LIMIT).
        assert!(r.inaccuracy <= 16 * 10_000_000, "{}", r.inaccuracy);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service() {
        // The path from one adjtimex(2) result to secure_universal_time
        // without the host clock: a source that returns the kernel_reading
        // of a given KernelTimex, designated in an administered service.
        use alloc::sync::Arc;
        use zerodds_time_service::admin::{
            AdminOp, AdminSubject, AuditError, AuditRecord, AuditSink,
        };
        use zerodds_time_service::secure::{
            SecureTimeSource, SourceDesignation, SourceReading, SyncProtection,
        };
        struct Keep;
        impl AuditSink for Keep {
            fn record(&self, _r: &AuditRecord) -> Result<(), AuditError> {
                Ok(())
            }
        }
        struct Adjtimex(KernelTimex);
        impl SecureTimeSource for Adjtimex {
            fn read(&self) -> Result<SourceReading, zerodds_time_service::TimeUnavailable> {
                kernel_reading(self.0)
            }
        }
        let secure = |k: KernelTimex| {
            let (service, admin) = zerodds_time_service::TimeService::default().administered(
                Arc::new(|_: &AdminSubject, _: AdminOp| true),
                Arc::new(Keep),
            );
            admin
                .designate_source(
                    &AdminSubject::new("ops"),
                    SourceDesignation::new("kernel", SyncProtection::Nts, Arc::new(Adjtimex(k))),
                )
                .unwrap();
            service.secure_universal_time()
        };
        // Synchronized (TIME_OK, STA_PLL | STA_NANO): the kernel time,
        // with maxerror as the inaccuracy.
        let july = time_to_ticks(Time::new(JULY, 0));
        let uto = secure(timex(0, 0x2001, 1_500, 123_456_789)).expect("synchronized");
        assert_eq!((uto.time(), uto.inaccuracy()), (july + 1_234_567, 15_000));
        // A leap second announced (TIME_INS) is still synchronized.
        assert!(secure(timex(1, 0x2001, 1, 0)).is_ok());
        // Not synchronized: no secure time.
        for (state, status, why) in [
            (0, 0x2041, "STA_UNSYNC"),
            (0, 0x3001, "STA_CLOCKERR"),
            (5, 0x2001, "TIME_ERROR"),
            (3, 0x2001, "TIME_OOP"),
        ] {
            assert!(secure(timex(state, status, 1, 0)).is_err(), "{why}");
        }
        // A maxerror beyond 48 bits of 100 ns: no secure time.
        assert!(secure(timex(0, 0x2001, 30_000_000_000_000, 0)).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[allow(clippy::print_stdout)]
    fn time_a_1_kernel_clock_as_authoritative_source() {
        use alloc::sync::Arc;
        use zerodds_time_service::admin::{
            AdminOp, AdminSubject, AuditError, AuditRecord, AuditSink,
        };
        use zerodds_time_service::secure::{SecureTimeSource, SourceDesignation, SyncProtection};
        struct Keep;
        impl AuditSink for Keep {
            fn record(&self, _r: &AuditRecord) -> Result<(), AuditError> {
                Ok(())
            }
        }
        let (service, admin) = zerodds_time_service::TimeService::default().administered(
            Arc::new(|s: &AdminSubject, _: AdminOp| s.as_str() == "ops"),
            Arc::new(Keep),
        );
        let ops = AdminSubject::new("ops");
        // Stated as unprotected: no secure time, whatever the kernel says.
        admin
            .designate_source(
                &ops,
                SourceDesignation::new(
                    "kernel",
                    SyncProtection::Unprotected,
                    Arc::new(KernelClock),
                ),
            )
            .unwrap();
        assert!(service.secure_universal_time().is_err());
        admin
            .designate_source(
                &ops,
                SourceDesignation::new("kernel", SyncProtection::Nts, Arc::new(KernelClock)),
            )
            .unwrap();
        // Secure time exactly when the kernel reports its clock
        // synchronized; the UTO carries the kernel's maximum error. Which
        // case the host gave is printed (`--no-capture`); both cases are
        // checked without the host clock in
        // `time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service`.
        let reading = KernelClock.read().unwrap();
        let secure = service.secure_universal_time();
        println!(
            "time_a_1 kernel clock: synchronized {}, maxerror {} x 100 ns, secure time {}",
            reading.synchronized,
            reading.inaccuracy,
            if secure.is_ok() {
                "given"
            } else {
                "unavailable"
            }
        );
        assert_eq!(secure.is_ok(), reading.synchronized, "{reading:?}");
        if let Ok(uto) = secure {
            assert!(uto.time() >= reading.time);
            assert!(uto.inaccuracy() > 0 && uto.inaccuracy() <= 16 * 10_000_000);
        }
    }

    #[test]
    fn time_service_clock_unavailable_source_reports_invalid_time() {
        let ts = zerodds_time_service::TimeService::with_source(|| 0);
        let clock = TimeServiceClock::new(ts);
        assert_eq!(clock.now(), Time::INVALID);
    }
}
