// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TimeService Interface — OMG Time Service 1.1 §2.1.
//!
//! Spec-IDL:
//! ```idl
//! interface TimeService {
//!     UTO universal_time() raises(TimeUnavailable);
//!     UTO secure_universal_time() raises(TimeUnavailable);
//!     UTO new_universal_time(in TimeT, in InaccuracyT, in TdfT);
//!     UTO uto_from_utc(in UtcT);
//!     TIO new_interval(in TimeT lower, in TimeT upper);
//! };
//! ```
//!
//! We model this without a CORBA interface — the operations are realized
//! as methods on the [`TimeService`] struct.

use core::fmt;

use alloc::sync::Arc;

#[cfg(feature = "std")]
use crate::admin::{AdminAuthorizer, Administration, AuditSink, TimeAdmin};
use crate::exception::{AbsoluteTimeError, BadParam};
use crate::time_base::{InaccuracyT, IntervalT, MAX_INACCURACY, TdfT, TimeT, UtcT};
#[cfg(feature = "std")]
use crate::time_base::{current_time, local_tdf};
use crate::tio::Tio;
use crate::uto::Uto;

/// Spec §1.3.3.1 — `TimeUnavailable`-Exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeUnavailable;

impl fmt::Display for TimeUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "underlying time service unavailable")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TimeUnavailable {}

/// A pluggable clock source (spec-external: the OMG spec has no `no_std`
/// embedding hook of its own — this is ZeroDDS's). Returns a [`TimeT`]
/// (100ns ticks since 1582-10-15, spec §1.3.2.1), the same domain
/// `std`'s [`current_time`] returns. `0` means "unavailable", the same
/// contract [`TimeService::universal_time`] already gives the `std`
/// stub.
pub type ClockSource = Arc<dyn Fn() -> TimeT + Send + Sync>;

/// The current tick of `source`, else `std`'s platform clock, else
/// (`no_std`, no source) `0`.
pub(crate) fn read_clock(source: Option<&ClockSource>) -> TimeT {
    if let Some(f) = source {
        return f();
    }
    #[cfg(feature = "std")]
    {
        current_time()
    }
    #[cfg(not(feature = "std"))]
    {
        0
    }
}

/// Where the time displacement factor of an absolute UTO comes from
/// (spec §1.3.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TdfSetting {
    /// The local time zone of the process at the time of the UTO
    /// ([`local_tdf`]); the `std` default.
    #[cfg(feature = "std")]
    Local,
    /// A displacement stated by the embedder ([`TimeService::with_tdf`]);
    /// the `no_std` default is `Fixed(0)`, since a `no_std` build has no
    /// zone database.
    Fixed(TdfT),
}

/// `TimeService` object — Spec §2.1. ZeroDDS implementation as a
/// plain Rust struct; feature `serve` makes it a CORBA object
/// (`crate::server`).
///
/// A service built with [`Self::default`], [`Self::with_source`],
/// [`Self::with_tdf`] and [`Self::with_inaccuracy`] is fixed once built
/// and not secure: [`Self::secure_universal_time`] raises
/// [`TimeUnavailable`]. [`Self::administered`] (feature `std`) gives it an
/// administration (App. B.1): an authorized and audited
/// [`TimeAdmin`] designates the authoritative source, sets the time and
/// the inaccuracy, and the service gives secure time when the criteria
/// of the crate documentation ("Secure time") hold. Clones share the
/// administration.
#[derive(Clone)]
pub struct TimeService {
    /// Time displacement factor of the absolute UTOs this service
    /// creates (§1.3.2.4).
    tdf: TdfSetting,
    /// Inaccuracy reported in the UTOs of `universal_time()`; at most
    /// [`MAX_INACCURACY`] ([`Self::with_inaccuracy`]).
    inaccuracy: InaccuracyT,
    /// Injected clock ([`Self::with_source`]). `None` (the `Default`
    /// case) reads `std`'s [`current_time`] on `std` builds, or is
    /// always-unavailable (`0`) under `no_std`.
    source: Option<ClockSource>,
    /// The administration (App. B.1) of an administered service.
    #[cfg(feature = "std")]
    administration: Option<Arc<Administration>>,
}

impl Default for TimeService {
    fn default() -> Self {
        Self {
            #[cfg(feature = "std")]
            tdf: TdfSetting::Local,
            #[cfg(not(feature = "std"))]
            tdf: TdfSetting::Fixed(0),
            inaccuracy: 0,
            source: None,
            #[cfg(feature = "std")]
            administration: None,
        }
    }
}

impl fmt::Debug for TimeService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimeService")
            .field("tdf", &self.tdf)
            .field("inaccuracy", &self.inaccuracy)
            .field("source", &self.source.as_ref().map(|_| "<fn>"))
            .field("administered", &self.is_administered())
            .finish()
    }
}

impl TimeService {
    /// Injects a pluggable clock source. This is the `no_std` embedding
    /// hook: without it, `no_std`'s [`crate::time_base::current_time`]
    /// stub always returns `0` and every `universal_time()` call fails
    /// with [`TimeUnavailable`] — there was previously no way for an
    /// embedder to supply a real clock (a disciplined RTC, `CLOCK_TAI`,
    /// a deterministic test clock). `std` embedders may use it too, to
    /// override the platform `SystemTime` clock.
    ///
    /// `source` returning `0` is read as "unavailable", the same
    /// contract the `std` stub already has.
    #[must_use]
    pub fn with_source<F>(source: F) -> Self
    where
        F: Fn() -> TimeT + Send + Sync + 'static,
    {
        Self {
            source: Some(Arc::new(source)),
            ..Self::default()
        }
    }

    /// States the time displacement factor of the local time zone as a
    /// fixed number of minutes east of Greenwich (§1.3.2.3). Every
    /// absolute UTO the service creates carries it (§1.3.2.4). Without
    /// it, a `std` build takes the zone of the process at the time of
    /// each UTO ([`local_tdf`], daylight saving time included) and a
    /// `no_std` build uses `0`.
    ///
    /// On an administered service the result is a new service without
    /// the administration: not secure, and not changed by its
    /// [`TimeAdmin`].
    #[must_use]
    pub fn with_tdf(mut self, tdf: TdfT) -> Self {
        self.tdf = TdfSetting::Fixed(tdf);
        self.leave_administration();
        self
    }

    /// Sets the inaccuracy (100 ns units) the UTOs of
    /// [`Self::universal_time`] report: the error estimate of the clock
    /// (§1.1.1, §2.1.1).
    ///
    /// On an administered service the result is a new service without
    /// the administration (App. B.1 protects the change of the
    /// inaccuracy of an administered service: [`TimeAdmin::set_inaccuracy`]).
    ///
    /// # Errors
    /// [`BadParam`] if `inaccuracy` does not fit 48 bits (§1.3.2.2).
    pub fn with_inaccuracy(mut self, inaccuracy: InaccuracyT) -> Result<Self, BadParam> {
        if inaccuracy > MAX_INACCURACY {
            return Err(BadParam);
        }
        self.inaccuracy = inaccuracy;
        self.leave_administration();
        Ok(self)
    }

    /// Drops the administration: the service becomes a separate one
    /// that is not administered.
    fn leave_administration(&mut self) {
        #[cfg(feature = "std")]
        {
            self.administration = None;
        }
    }

    /// Makes this service an administered one (App. B.1) and returns it
    /// with its [`TimeAdmin`]. `authorizer` is the system security
    /// policy for the operations of B.1, `audit` keeps their records. The
    /// clock, inaccuracy and tdf built so far are its starting state; the
    /// clock stays its base clock until an authoritative source is
    /// designated.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn administered(
        mut self,
        authorizer: Arc<dyn AdminAuthorizer>,
        audit: Arc<dyn AuditSink>,
    ) -> (Self, TimeAdmin) {
        let administration = Arc::new(Administration::new(
            authorizer,
            audit,
            self.source.clone(),
            self.inaccuracy,
        ));
        self.administration = Some(Arc::clone(&administration));
        (self, TimeAdmin::new(administration))
    }

    /// `true` if the service has an administration ([`Self::administered`]).
    #[must_use]
    pub fn is_administered(&self) -> bool {
        #[cfg(feature = "std")]
        {
            self.administration.is_some()
        }
        #[cfg(not(feature = "std"))]
        {
            false
        }
    }

    /// The inaccuracy the UTOs of [`Self::universal_time`] report: the
    /// one built with [`Self::with_inaccuracy`], or the one an
    /// administrator set. With an authoritative source the UTOs carry the
    /// larger of it and the source's bound.
    #[must_use]
    pub fn inaccuracy(&self) -> InaccuracyT {
        #[cfg(feature = "std")]
        if let Some(a) = &self.administration {
            return a.inaccuracy();
        }
        self.inaccuracy
    }

    /// The time displacement factor an absolute UTO at `time` gets from
    /// this service (§1.3.2.4).
    #[must_use]
    pub fn tdf_at(&self, time: TimeT) -> TdfT {
        match self.tdf {
            #[cfg(feature = "std")]
            TdfSetting::Local => local_tdf(time),
            TdfSetting::Fixed(tdf) => {
                let _ = time;
                tdf
            }
        }
    }

    /// The current time and its inaccuracy: the administration's if the
    /// service has one, else the clock of [`Self::with_source`] (or the
    /// platform clock) with the built inaccuracy.
    fn now(&self) -> Result<(TimeT, InaccuracyT), TimeUnavailable> {
        #[cfg(feature = "std")]
        if let Some(a) = &self.administration {
            return a.now();
        }
        match read_clock(self.source.as_ref()) {
            0 => Err(TimeUnavailable),
            now => Ok((now, self.inaccuracy)),
        }
    }

    /// Spec §2.1.1 — `universal_time()`. Returns the current time with
    /// its inaccuracy and the time displacement factor of the local time
    /// zone (§1.3.2.4). Not guaranteed to be secure or trusted.
    ///
    /// # Errors
    /// `TimeUnavailable` if the clock (injected, platform or the
    /// authoritative source) gives no time (e.g. `no_std` without
    /// [`Self::with_source`]).
    pub fn universal_time(&self) -> Result<Uto, TimeUnavailable> {
        let (now, inaccuracy) = self.now()?;
        UtcT::new(now, inaccuracy, self.tdf_at(now))
            .map(Uto::from_utc)
            .map_err(|_| TimeUnavailable)
    }

    /// Spec §2.1.2 — `secure_universal_time()`. Returns the time only if
    /// every criterion for secure time (crate documentation, "Secure
    /// time"; App. A, App. B) holds at this call: an administered
    /// service, an authoritative source designated through
    /// [`TimeAdmin::designate_source`], its synchronization stated as
    /// protected in transit, no time set by hand, a synchronized reading
    /// with an error bound of at most 48 bits, and a time not before the
    /// last one this operation returned. The UTO carries the source's
    /// error bound (or the larger administered inaccuracy).
    ///
    /// # Errors
    /// `TimeUnavailable` if any criterion does not hold; always on a
    /// service that is not administered and under `no_std`.
    pub fn secure_universal_time(&self) -> Result<Uto, TimeUnavailable> {
        #[cfg(feature = "std")]
        {
            let administration = self.administration.as_ref().ok_or(TimeUnavailable)?;
            let (now, inaccuracy) = administration.secure_now()?;
            UtcT::new(now, inaccuracy, self.tdf_at(now))
                .map(Uto::from_utc)
                .map_err(|_| TimeUnavailable)
        }
        #[cfg(not(feature = "std"))]
        {
            Err(TimeUnavailable)
        }
    }

    /// Spec §1.3.4.5 — `absolute_time()` of the UTO `relative`, on the
    /// clock of this service: `current time + relative.time`. The result
    /// keeps the inaccuracy of `relative` and carries the time
    /// displacement factor of the local time zone at the absolute time
    /// (§1.3.2.4).
    ///
    /// # Errors
    /// [`AbsoluteTimeError::DataConversion`] (`CORBA::DATA_CONVERSION`)
    /// if the sum overflows `TimeT`; [`AbsoluteTimeError::TimeUnavailable`]
    /// if the clock gives no time.
    pub fn absolute_time(&self, relative: Uto) -> Result<Uto, AbsoluteTimeError> {
        let (now, _) = self.now().map_err(|_| AbsoluteTimeError::TimeUnavailable)?;
        let absolute = now
            .checked_add(relative.time())
            .ok_or(AbsoluteTimeError::DataConversion)?;
        UtcT::new(absolute, relative.inaccuracy(), self.tdf_at(absolute))
            .map(Uto::from_utc)
            .map_err(|_| AbsoluteTimeError::DataConversion)
    }

    /// Spec §2.1.2.1 — `new_universal_time(time, inaccuracy, tdf)`.
    ///
    /// # Errors
    /// [`BadParam`] (`CORBA::BAD_PARAM`) if `inaccuracy` is out of range,
    /// i.e. does not fit the 48 bits of `UtcT` (§1.3.2.2).
    pub fn new_universal_time(
        time: TimeT,
        inaccuracy: InaccuracyT,
        tdf: TdfT,
    ) -> Result<Uto, BadParam> {
        Uto::new(time, inaccuracy, tdf)
    }

    /// Spec §2.1.2.2 — `uto_from_utc(utc)`.
    #[must_use]
    pub fn uto_from_utc(utc: UtcT) -> Uto {
        Uto::from_utc(utc)
    }

    /// Spec §2.1.2.3 — `new_interval(lower, upper)`.
    ///
    /// # Errors
    /// [`BadParam`] (`CORBA::BAD_PARAM`) if `lower > upper`.
    pub fn new_interval(lower: TimeT, upper: TimeT) -> Result<Tio, BadParam> {
        IntervalT::new(lower, upper)
            .map(Tio::from_interval)
            .ok_or(BadParam)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    #[cfg(feature = "std")]
    use crate::time_base::{TICKS_PER_SECOND, UTC_EPOCH_TO_UNIX_TICKS};

    #[test]
    fn new_universal_time_creates_uto_from_components() {
        // Spec §2.1.2.1.
        let uto = TimeService::new_universal_time(1_000, 50, 60).expect("ok");
        assert_eq!(uto.time(), 1_000);
        assert_eq!(uto.inaccuracy(), 50);
        assert_eq!(uto.tdf(), 60);
    }

    #[test]
    fn time_2_1_2_1_inaccuracy_above_48_bits_is_bad_param() {
        // Spec §2.1.2.1: "CORBA::BAD_PARAM is raised in the case of an
        // out-of-range parameter value for inaccuracy."
        assert_eq!(
            TimeService::new_universal_time(1_000, MAX_INACCURACY + 1, 0),
            Err(BadParam)
        );
        assert_eq!(
            TimeService::new_universal_time(1_000, u64::MAX, 0),
            Err(BadParam)
        );
    }

    #[test]
    fn time_2_1_2_1_inaccuracy_at_48_bit_limit_is_accepted() {
        let uto = TimeService::new_universal_time(1_000, MAX_INACCURACY, -60).expect("in range");
        assert_eq!(uto.inaccuracy(), MAX_INACCURACY);
        assert_eq!(uto.tdf(), -60);
    }

    #[test]
    fn uto_from_utc_wraps_passed_struct() {
        // Spec §2.1.2.2.
        let utc = UtcT::new(100, 0, 0).expect("ok");
        let uto = TimeService::uto_from_utc(utc);
        assert_eq!(uto.utc_time(), utc);
    }

    #[test]
    fn new_interval_rejects_lower_greater_than_upper() {
        // Spec §2.1.2.3 — BAD_PARAM when lower > upper.
        assert_eq!(TimeService::new_interval(200, 100), Err(BadParam));
    }

    #[test]
    fn new_interval_creates_tio_for_valid_bounds() {
        let tio = TimeService::new_interval(100, 200).expect("ok");
        assert_eq!(tio.time_interval().lower_bound, 100);
        assert_eq!(tio.time_interval().upper_bound, 200);
    }

    #[cfg(feature = "std")]
    #[test]
    fn universal_time_returns_recent_value() {
        let service = TimeService::default();
        let uto = service.universal_time().expect("ok");
        // Spec §2.1.1 — time must be > 0 and in the plausible range
        // (post-2020, pre-2200).
        assert!(uto.time() > 130_000_000_000_000_000);
    }

    #[test]
    fn time_a_1_secure_time_unavailable_on_a_service_that_is_not_administered() {
        // Spec §2.1.2, App. A.1: a service nobody administers has no
        // authoritative source, so no secure time, whatever its clock.
        assert!(!TimeService::default().is_administered());
        assert_eq!(
            TimeService::default().secure_universal_time(),
            Err(TimeUnavailable)
        );
        let service = TimeService::with_source(|| 1_000).with_tdf(0);
        assert!(service.universal_time().is_ok());
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
    }

    #[test]
    fn time_unavailable_display_describes_failure_mode() {
        let s = alloc::format!("{TimeUnavailable}");
        assert!(s.contains("time service unavailable"));
    }

    #[test]
    fn with_source_overrides_the_platform_clock() {
        // The doc-drift this closes: website/docs/time-service.html and
        // time_base.rs's doc comment promised `TimeService::with_source(...)`
        // as the no_std embedding hook; it did not exist. It now reads the
        // injected source instead of the platform clock, on std too.
        let service = TimeService::with_source(|| 42);
        let uto = service.universal_time().expect("ok");
        assert_eq!(uto.time(), 42);
    }

    #[test]
    fn with_source_zero_is_time_unavailable() {
        // Same contract as the no_std stub: 0 means unavailable.
        let service = TimeService::with_source(|| 0);
        assert_eq!(service.universal_time(), Err(TimeUnavailable));
    }

    #[test]
    fn with_source_keeps_the_other_defaults() {
        let service = TimeService::with_source(|| 1_000);
        assert_eq!(service.inaccuracy(), 0);
        assert!(!service.is_administered());
        assert_eq!(service.tdf, TimeService::default().tdf);
    }

    #[test]
    fn with_source_reads_live_state_on_every_call() {
        // A closure capturing shared, mutable state (e.g. a test clock) is
        // read fresh each call -- the source is not a one-shot snapshot.
        use core::sync::atomic::{AtomicU64, Ordering};
        let tick = alloc::sync::Arc::new(AtomicU64::new(10));
        let for_closure = tick.clone();
        let service = TimeService::with_source(move || for_closure.load(Ordering::SeqCst));
        assert_eq!(service.universal_time().expect("ok").time(), 10);
        tick.store(20, Ordering::SeqCst);
        assert_eq!(service.universal_time().expect("ok").time(), 20);
    }

    #[test]
    fn with_source_debug_does_not_panic() {
        let service = TimeService::with_source(|| 1);
        let s = alloc::format!("{service:?}");
        assert!(s.contains("TimeService"));
    }

    #[test]
    fn time_2_1_1_configured_inaccuracy_is_reported_and_bounded_to_48_bits() {
        let service = TimeService::with_source(|| 1_000)
            .with_inaccuracy(5_000)
            .expect("in range");
        assert_eq!(service.universal_time().expect("ok").inaccuracy(), 5_000);
        assert_eq!(
            TimeService::with_source(|| 1_000)
                .with_inaccuracy(MAX_INACCURACY + 1)
                .map(|_| ()),
            Err(BadParam)
        );
    }

    #[test]
    fn time_1_3_2_4_stated_tdf_is_carried_by_universal_and_absolute_time() {
        let service = TimeService::with_source(|| 1_000_000).with_tdf(-300);
        assert_eq!(service.universal_time().expect("ok").tdf(), -300);
        let relative = Uto::new(500, 3, 0).expect("ok");
        let absolute = service.absolute_time(relative).expect("ok");
        assert_eq!(absolute.time(), 1_000_500);
        assert_eq!(absolute.inaccuracy(), 3);
        assert_eq!(absolute.tdf(), -300);
    }

    #[test]
    fn time_1_3_4_5_absolute_time_on_the_service_clock() {
        let service = TimeService::with_source(|| u64::MAX - 10).with_tdf(0);
        let relative = Uto::new(11, 0, 0).expect("ok");
        assert_eq!(
            service.absolute_time(relative),
            Err(AbsoluteTimeError::DataConversion)
        );
        let relative = Uto::new(10, 0, 0).expect("ok");
        assert_eq!(
            service.absolute_time(relative).expect("fits").time(),
            u64::MAX
        );
        let unavailable = TimeService::with_source(|| 0);
        assert_eq!(
            unavailable.absolute_time(relative),
            Err(AbsoluteTimeError::TimeUnavailable)
        );
    }

    /// The environment variable that turns [`tdf_probe_child`] on and
    /// carries its clock tick and relative time.
    #[cfg(feature = "std")]
    const TDF_PROBE_ENV: &str = "ZERODDS_TIME_SERVICE_TDF_PROBE";

    /// 2026-01-15T12:00:00Z (winter on the northern hemisphere).
    #[cfg(feature = "std")]
    const JANUARY: TimeT = UTC_EPOCH_TO_UNIX_TICKS + 1_768_478_400 * TICKS_PER_SECOND;

    /// 181 days: from `JANUARY` to 2026-07-15T12:00:00Z (summer).
    #[cfg(feature = "std")]
    const HALF_YEAR: TimeT = 181 * 86_400 * TICKS_PER_SECOND;

    /// Runs only as the child process the two `time_1_3_2_4_*_local_tdf`
    /// tests start with `TZ` set; in a normal run the variable is absent
    /// and it returns at once. Prints the tdf of `universal_time` at the
    /// tick and of `absolute_time` of the relative time.
    #[cfg(feature = "std")]
    #[test]
    #[allow(clippy::print_stdout)]
    fn tdf_probe_child() {
        let Ok(spec) = std::env::var(TDF_PROBE_ENV) else {
            return;
        };
        let mut parts = spec.split(' ');
        let tick: TimeT = parts.next().expect("tick").parse().expect("tick");
        let relative: TimeT = parts.next().expect("relative").parse().expect("relative");
        let service = TimeService::with_source(move || tick);
        let universal = service.universal_time().expect("ok");
        let absolute = service
            .absolute_time(Uto::new(relative, 0, 0).expect("ok"))
            .expect("ok");
        println!("ZERODDS-TDF {} {}", universal.tdf(), absolute.tdf());
    }

    /// Starts this test binary as a child with `TZ=tz` and returns the
    /// tdf of `universal_time` at `tick` and of `absolute_time` of
    /// `relative` that the child measured.
    #[cfg(feature = "std")]
    fn probe_tdf(tz: &str, tick: TimeT, relative: TimeT) -> (TdfT, TdfT) {
        let exe = std::env::current_exe().expect("test binary");
        let out = std::process::Command::new(exe)
            .args([
                "--exact",
                "service::tests::tdf_probe_child",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env("TZ", tz)
            .env(TDF_PROBE_ENV, alloc::format!("{tick} {relative}"))
            .output()
            .expect("start the probe");
        let stdout = alloc::string::String::from_utf8_lossy(&out.stdout);
        let line = stdout
            .lines()
            .find_map(|l| l.split_once("ZERODDS-TDF ").map(|(_, rest)| rest.trim()))
            .unwrap_or_else(|| panic!("TZ={tz}: no probe line in {stdout:?}"));
        let mut values = line.split(' ').map(|v| v.parse::<TdfT>().expect("tdf"));
        (
            values.next().expect("universal"),
            values.next().expect("absolute"),
        )
    }

    #[cfg(feature = "std")]
    #[test]
    fn time_1_3_2_4_universal_time_carries_the_local_tdf() {
        // Spec §1.3.2.4: "Implementations must place the time displacement
        // factor for the local time zone in this field whenever they create
        // a UTO that expresses absolute time." POSIX TZ rules need no zone
        // database on the host.
        assert_eq!(probe_tdf("CET-1CEST,M3.5.0,M10.5.0/3", JANUARY, 0).0, 60);
        assert_eq!(
            probe_tdf("CET-1CEST,M3.5.0,M10.5.0/3", JANUARY + HALF_YEAR, 0).0,
            120
        );
        assert_eq!(probe_tdf("EST5EDT,M3.2.0,M11.1.0", JANUARY, 0).0, -300);
        assert_eq!(probe_tdf("IST-5:30", JANUARY, 0).0, 330);
        assert_eq!(probe_tdf("UTC0", JANUARY, 0).0, 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn time_1_3_2_4_absolute_time_carries_the_local_tdf() {
        // The relative UTO has tdf 0 (a duration); the absolute UTO gets
        // the displacement of the local zone at the absolute time, here
        // half a year after the clock: summer time while the clock is in
        // winter.
        assert_eq!(
            probe_tdf("CET-1CEST,M3.5.0,M10.5.0/3", JANUARY, HALF_YEAR),
            (60, 120)
        );
        assert_eq!(
            probe_tdf("EST5EDT,M3.2.0,M11.1.0", JANUARY, HALF_YEAR),
            (-300, -240)
        );
    }
}
