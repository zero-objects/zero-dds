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

use zerodds_time_service::time_base::{TICKS_PER_SECOND, UTC_EPOCH_TO_UNIX_TICKS};
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
    /// with no such data (the system clock, `CLOCK_TAI`) report zero
    /// inaccuracy and `tdf=0` — a true statement about what they know,
    /// not a fabricated value. [`TimeServiceClock`] overrides this with
    /// the wrapped `TimeService`'s real `Uto`.
    fn now_uto(&self) -> Uto {
        Uto::from_utc(UtcT::new(time_to_ticks(self.now()), 0, 0))
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
        self.service.universal_time().unwrap_or_else(|_| {
            Uto::new(0, self.service.default_inaccuracy, self.service.default_tdf)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ticks_roundtrip_seconds_only() {
        let t = Time::new(1_700_000_000, 0);
        assert_eq!(ticks_to_time(time_to_ticks(t)), t);
    }

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
    fn system_clock_matches_get_current_time() {
        let a = SystemClock.now();
        let b = crate::time::get_current_time();
        // Both read `SystemTime::now()` a few nanoseconds apart; compare
        // seconds only to avoid a flaky sub-second race.
        assert_eq!(a.sec, b.sec);
    }

    #[test]
    fn test_clock_does_not_advance_on_its_own() {
        let clock = TestClock::new(Time::new(100, 0));
        assert_eq!(clock.now(), Time::new(100, 0));
        assert_eq!(clock.now(), Time::new(100, 0));
    }

    #[test]
    fn test_clock_set_and_advance() {
        let clock = TestClock::new(Time::new(0, 0));
        clock.set(Time::new(50, 0));
        assert_eq!(clock.now(), Time::new(50, 0));
        clock.advance(crate::time::Duration::new(1, 500_000_000));
        assert_eq!(clock.now(), Time::new(51, 500_000_000));
    }

    #[test]
    fn test_clock_clones_share_state() {
        let a = TestClock::new(Time::new(0, 0));
        let b = a.clone();
        a.set(Time::new(7, 0));
        assert_eq!(b.now(), Time::new(7, 0));
    }

    #[test]
    fn now_uto_default_carries_zero_inaccuracy_and_tdf() {
        let clock = TestClock::new(Time::new(1_700_000_000, 0));
        let uto = clock.now_uto();
        assert_eq!(uto.inaccuracy(), 0);
        assert_eq!(uto.tdf(), 0);
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
        let mut ts = zerodds_time_service::TimeService::with_source(move || ticks);
        ts.default_inaccuracy = 5_000;
        ts.default_tdf = 60;
        let clock = TimeServiceClock::new(ts);
        let uto = clock.now_uto();
        assert_eq!(uto.inaccuracy(), 5_000);
        assert_eq!(uto.tdf(), 60);
    }

    #[test]
    fn time_service_clock_unavailable_source_reports_invalid_time() {
        let ts = zerodds_time_service::TimeService::with_source(|| 0);
        let clock = TimeServiceClock::new(ts);
        assert_eq!(clock.now(), Time::INVALID);
    }
}
