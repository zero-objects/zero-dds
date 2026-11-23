// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Administration of time — OMG Time Service 1.1 Appendix B.1.
//! Feature `std`.
//!
//! App. B: "Only administrators authorized by the system security policy
//! may set the time and specify the source of time". B.1: operations that
//! set or reset the current time, that designate a time source as
//! authoritative, and that modify the accuracy or the uncertainty
//! interval of generated timestamps "must be protected against
//! unauthorized invocation. They must also be mandatorily audited".
//!
//! [`crate::TimeService::administered`] turns a service into an
//! administered one and returns its [`TimeAdmin`]. Every operation of
//! [`TimeAdmin`] runs in this order, one at a time:
//!
//! 1. the [`AdminAuthorizer`] decides for the [`AdminSubject`] and the
//!    [`AdminOp`]; a refusal is audited ([`AuditOutcome::Denied`]) and
//!    returns [`AdminError::Unauthorized`];
//! 2. the parameters are checked; an invalid one is audited
//!    ([`AuditOutcome::Failed`]) and returns its error;
//! 3. the [`AuditSink`] writes the record ([`AuditOutcome::Granted`],
//!    the value before and after);
//! 4. only then the change takes effect.
//!
//! If the sink cannot write a record, the operation returns
//! [`AdminError::AuditFailed`] and changes nothing: no operation takes
//! effect without its record. The subject is the identity the embedding
//! application authenticated; this crate authenticates nobody. The
//! default authorizer [`DenyAll`] refuses every operation.
//!
//! The service has no other way to change these values: its fields are
//! private, and `with_source`, `with_tdf` and `with_inaccuracy` build a
//! new service that is not administered and never gives secure time.

use alloc::string::String;
use alloc::sync::Arc;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError, RwLock};

use crate::secure::SourceDesignation;
use crate::service::{ClockSource, TimeUnavailable, read_clock};
use crate::time_base::{InaccuracyT, MAX_INACCURACY, TimeT, current_time};

/// An operation B.1 protects and audits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdminOp {
    /// Sets the current time of the service ([`TimeAdmin::set_time`]).
    SetTime,
    /// Returns to the time of the source ([`TimeAdmin::reset_time`]).
    ResetTime,
    /// Designates the authoritative source or withdraws it
    /// ([`TimeAdmin::designate_source`], [`TimeAdmin::withdraw_source`]).
    DesignateSource,
    /// Sets the uncertainty interval of generated timestamps
    /// ([`TimeAdmin::set_inaccuracy`]).
    SetInaccuracy,
}

impl AdminOp {
    /// The name in audit records: `set-time`, `reset-time`,
    /// `designate-source`, `set-inaccuracy`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SetTime => "set-time",
            Self::ResetTime => "reset-time",
            Self::DesignateSource => "designate-source",
            Self::SetInaccuracy => "set-inaccuracy",
        }
    }
}

impl fmt::Display for AdminOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The identity that invokes an operation, as the embedding application
/// authenticated it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdminSubject(String);

impl AdminSubject {
    /// The subject `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AdminSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The system security policy for the operations of B.1: `true` lets
/// `subject` invoke `op`.
pub trait AdminAuthorizer: Send + Sync {
    /// Decides whether `subject` may invoke `op`.
    fn authorize(&self, subject: &AdminSubject, op: AdminOp) -> bool;
}

/// Refuses every operation; the authorizer of a service whose
/// administration nobody may change.
#[derive(Debug, Clone, Copy, Default)]
pub struct DenyAll;

impl AdminAuthorizer for DenyAll {
    fn authorize(&self, _subject: &AdminSubject, _op: AdminOp) -> bool {
        false
    }
}

impl<F> AdminAuthorizer for F
where
    F: Fn(&AdminSubject, AdminOp) -> bool + Send + Sync,
{
    fn authorize(&self, subject: &AdminSubject, op: AdminOp) -> bool {
        self(subject, op)
    }
}

/// What became of an invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditOutcome {
    /// Authorized; the change takes effect after the record.
    Granted,
    /// Refused by the authorizer; nothing changed.
    Denied,
    /// Authorized, but not carried out: an invalid parameter, or the
    /// clock gave no time; nothing changed.
    Failed,
}

impl AuditOutcome {
    /// `granted`, `denied` or `failed`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }
}

/// One audit record (App. B.1: "mandatorily audited").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    /// When the record was made: the system clock, 100 ns ticks since
    /// 1582-10-15.
    pub time: TimeT,
    /// Who invoked the operation.
    pub subject: AdminSubject,
    /// The operation.
    pub op: AdminOp,
    /// The value before the operation.
    pub before: String,
    /// The value the operation sets; for a refusal `-`, for a failure
    /// the reason.
    pub after: String,
    /// What became of it.
    pub outcome: AuditOutcome,
}

impl fmt::Display for AuditRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "time={} subject={} op={} outcome={} before={} after={}",
            self.time,
            self.subject,
            self.op,
            self.outcome.name(),
            self.before,
            self.after
        )
    }
}

/// A record could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditError(pub String);

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit record not written: {}", self.0)
    }
}

impl std::error::Error for AuditError {}

/// Where the records go. `record` returns only once the record is kept;
/// an error refuses the operation. `zerodds-security-logging` (feature
/// `time-service`) writes them as JSON lines.
pub trait AuditSink: Send + Sync {
    /// Keeps `record`.
    ///
    /// # Errors
    /// [`AuditError`] if the record was not kept.
    fn record(&self, record: &AuditRecord) -> Result<(), AuditError>;
}

/// Why an operation did not take effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminError {
    /// The authorizer refused it; the refusal is audited.
    Unauthorized,
    /// The audit record could not be written; nothing changed.
    AuditFailed(AuditError),
    /// An inaccuracy above 48 bits, or a time whose distance to the
    /// source does not fit 64 signed bits of 100 ns.
    BadParam,
    /// `set_time` needs a reading of the clock, and the clock gave none.
    TimeUnavailable,
}

impl fmt::Display for AdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => f.write_str("not authorized"),
            Self::AuditFailed(e) => write!(f, "{e}"),
            Self::BadParam => f.write_str("parameter out of range"),
            Self::TimeUnavailable => f.write_str("the clock gives no time"),
        }
    }
}

impl std::error::Error for AdminError {}

/// The values an administrator changes.
#[derive(Clone)]
pub(crate) struct AdminState {
    /// `Some`: the time was set; the service reports the source plus
    /// this many 100 ns ticks.
    offset: Option<i64>,
    /// The uncertainty interval of generated timestamps; with a source
    /// the larger of it and the source's bound.
    inaccuracy: InaccuracyT,
    /// The authoritative source; `None`: the base clock.
    source: Option<SourceDesignation>,
}

impl AdminState {
    fn describe_time(&self) -> String {
        match self.offset {
            None => String::from("source time"),
            Some(o) => alloc::format!("source time {o:+} ticks"),
        }
    }

    fn describe_source(&self) -> String {
        self.source
            .as_ref()
            .map_or_else(|| String::from("base clock"), SourceDesignation::describe)
    }
}

/// The shared state of an administered service.
pub(crate) struct Administration {
    authorizer: Arc<dyn AdminAuthorizer>,
    audit: Arc<dyn AuditSink>,
    /// The clock of the service before any designation.
    base: Option<ClockSource>,
    /// One operation at a time, from the decision to the change.
    serial: Mutex<()>,
    state: RwLock<AdminState>,
    /// The last time `secure_universal_time` returned (§1.1.3: "from a
    /// monotonically increasing series").
    last_secure: AtomicU64,
}

impl Administration {
    pub(crate) fn new(
        authorizer: Arc<dyn AdminAuthorizer>,
        audit: Arc<dyn AuditSink>,
        base: Option<ClockSource>,
        inaccuracy: InaccuracyT,
    ) -> Self {
        Self {
            authorizer,
            audit,
            base,
            serial: Mutex::new(()),
            state: RwLock::new(AdminState {
                offset: None,
                inaccuracy,
                source: None,
            }),
            last_secure: AtomicU64::new(0),
        }
    }

    fn read_state(&self) -> AdminState {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The inaccuracy an administrator set.
    pub(crate) fn inaccuracy(&self) -> InaccuracyT {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .inaccuracy
    }

    /// The reading of the authoritative source, or of the base clock,
    /// without the offset.
    fn source_now(&self, state: &AdminState) -> Result<(TimeT, InaccuracyT), TimeUnavailable> {
        let (time, inaccuracy) = match &state.source {
            Some(d) => {
                let r = d.read()?;
                (r.time, r.inaccuracy.max(state.inaccuracy))
            }
            None => (read_clock(self.base.as_ref()), state.inaccuracy),
        };
        if time == 0 || inaccuracy > MAX_INACCURACY {
            return Err(TimeUnavailable);
        }
        Ok((time, inaccuracy))
    }

    /// The current time of the service: the source, shifted by the time
    /// an administrator set (`universal_time`, `absolute_time`).
    pub(crate) fn now(&self) -> Result<(TimeT, InaccuracyT), TimeUnavailable> {
        let state = self.read_state();
        let (time, inaccuracy) = self.source_now(&state)?;
        let time = shift(time, state.offset.unwrap_or(0)).ok_or(TimeUnavailable)?;
        if time == 0 {
            return Err(TimeUnavailable);
        }
        Ok((time, inaccuracy))
    }

    /// The time for `secure_universal_time`: every criterion of the
    /// crate documentation ("Secure time") holds, else
    /// [`TimeUnavailable`].
    pub(crate) fn secure_now(&self) -> Result<(TimeT, InaccuracyT), TimeUnavailable> {
        let state = self.read_state();
        // An authoritative source, designated by an authorized and
        // audited operation (App. B.1).
        let designation = state.source.as_ref().ok_or(TimeUnavailable)?;
        // Its synchronization protected in transit (App. B.1.1).
        if !designation.protection().is_protected() {
            return Err(TimeUnavailable);
        }
        // A time set by hand has no known error bound.
        if state.offset.is_some() {
            return Err(TimeUnavailable);
        }
        // A reading with an error bound, synchronized (§1.1.3).
        let reading = designation.read()?;
        if !reading.synchronized || reading.time == 0 {
            return Err(TimeUnavailable);
        }
        let inaccuracy = reading.inaccuracy.max(state.inaccuracy);
        if inaccuracy > MAX_INACCURACY {
            return Err(TimeUnavailable);
        }
        // Monotonically increasing (§1.1.3): never before a time this
        // operation returned.
        let mut last = self.last_secure.load(Ordering::Acquire);
        loop {
            if reading.time < last {
                return Err(TimeUnavailable);
            }
            match self.last_secure.compare_exchange_weak(
                last,
                reading.time,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok((reading.time, inaccuracy)),
                Err(now) => last = now,
            }
        }
    }

    fn write_record(
        &self,
        subject: &AdminSubject,
        op: AdminOp,
        before: String,
        after: String,
        outcome: AuditOutcome,
    ) -> Result<(), AdminError> {
        let record = AuditRecord {
            time: current_time(),
            subject: subject.clone(),
            op,
            before,
            after,
            outcome,
        };
        self.audit.record(&record).map_err(AdminError::AuditFailed)
    }

    /// Runs one operation in the order of the module documentation.
    /// `plan` gets the state before and returns the record's `after`
    /// and the new state, or the reason and the error of a failure.
    fn run<F>(
        &self,
        subject: &AdminSubject,
        op: AdminOp,
        describe: fn(&AdminState) -> String,
        plan: F,
    ) -> Result<(), AdminError>
    where
        F: FnOnce(&AdminState) -> Result<(String, AdminState), (String, AdminError)>,
    {
        let _serial = self.serial.lock().unwrap_or_else(PoisonError::into_inner);
        let before = self.read_state();
        let before_text = describe(&before);
        if !self.authorizer.authorize(subject, op) {
            self.write_record(
                subject,
                op,
                before_text,
                String::from("-"),
                AuditOutcome::Denied,
            )?;
            return Err(AdminError::Unauthorized);
        }
        match plan(&before) {
            Err((reason, error)) => {
                self.write_record(subject, op, before_text, reason, AuditOutcome::Failed)?;
                Err(error)
            }
            Ok((after_text, after)) => {
                self.write_record(subject, op, before_text, after_text, AuditOutcome::Granted)?;
                *self.state.write().unwrap_or_else(PoisonError::into_inner) = after;
                Ok(())
            }
        }
    }
}

/// `time + offset`, `None` outside `TimeT`.
fn shift(time: TimeT, offset: i64) -> Option<TimeT> {
    if offset >= 0 {
        time.checked_add(offset.unsigned_abs())
    } else {
        time.checked_sub(offset.unsigned_abs())
    }
}

/// The administration of one [`crate::TimeService`] and its clones
/// (App. B.1).
#[derive(Clone)]
pub struct TimeAdmin {
    inner: Arc<Administration>,
}

impl fmt::Debug for TimeAdmin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.inner.read_state();
        f.debug_struct("TimeAdmin")
            .field("time", &state.describe_time())
            .field("source", &state.describe_source())
            .field("inaccuracy", &state.inaccuracy)
            .finish()
    }
}

impl TimeAdmin {
    pub(crate) fn new(inner: Arc<Administration>) -> Self {
        Self { inner }
    }

    /// Sets the current time of the service to `time` (B.1: "Operations
    /// that set or reset the current time"). From here on
    /// `universal_time` reports the source plus the distance between
    /// `time` and the source at this call; `secure_universal_time` gives
    /// no time until [`Self::reset_time`], since a time set by hand has
    /// no known error bound.
    ///
    /// # Errors
    /// [`AdminError::Unauthorized`], [`AdminError::AuditFailed`];
    /// [`AdminError::TimeUnavailable`] if the source gives no time;
    /// [`AdminError::BadParam`] if `time` is 0 or its distance to the
    /// source does not fit `i64`.
    pub fn set_time(&self, subject: &AdminSubject, time: TimeT) -> Result<(), AdminError> {
        let admin = &self.inner;
        admin.run(
            subject,
            AdminOp::SetTime,
            AdminState::describe_time,
            |before| {
                if time == 0 {
                    return Err((String::from("time 0"), AdminError::BadParam));
                }
                let (source, _) = admin.source_now(before).map_err(|_| {
                    (
                        String::from("the source gives no time"),
                        AdminError::TimeUnavailable,
                    )
                })?;
                let distance = i128::from(time) - i128::from(source);
                let offset = i64::try_from(distance).map_err(|_| {
                    (
                        alloc::format!("time {time}: distance {distance} out of range"),
                        AdminError::BadParam,
                    )
                })?;
                let after = AdminState {
                    offset: Some(offset),
                    ..before.clone()
                };
                Ok((
                    alloc::format!("time {time} ({})", after.describe_time()),
                    after,
                ))
            },
        )
    }

    /// Returns to the time of the source (B.1: "reset the current
    /// time").
    ///
    /// # Errors
    /// [`AdminError::Unauthorized`], [`AdminError::AuditFailed`].
    pub fn reset_time(&self, subject: &AdminSubject) -> Result<(), AdminError> {
        self.inner.run(
            subject,
            AdminOp::ResetTime,
            AdminState::describe_time,
            |before| {
                let after = AdminState {
                    offset: None,
                    ..before.clone()
                };
                Ok((after.describe_time(), after))
            },
        )
    }

    /// Designates the authoritative source of the service (B.1:
    /// "Operations that designate a time source as authoritative").
    /// `universal_time` and `secure_universal_time` read it from here on.
    ///
    /// # Errors
    /// [`AdminError::Unauthorized`], [`AdminError::AuditFailed`].
    pub fn designate_source(
        &self,
        subject: &AdminSubject,
        designation: SourceDesignation,
    ) -> Result<(), AdminError> {
        self.inner.run(
            subject,
            AdminOp::DesignateSource,
            AdminState::describe_source,
            |before| {
                let after = AdminState {
                    source: Some(designation),
                    ..before.clone()
                };
                Ok((after.describe_source(), after))
            },
        )
    }

    /// Withdraws the designation: the base clock again, and no secure
    /// time.
    ///
    /// # Errors
    /// [`AdminError::Unauthorized`], [`AdminError::AuditFailed`].
    pub fn withdraw_source(&self, subject: &AdminSubject) -> Result<(), AdminError> {
        self.inner.run(
            subject,
            AdminOp::DesignateSource,
            AdminState::describe_source,
            |before| {
                let after = AdminState {
                    source: None,
                    ..before.clone()
                };
                Ok((after.describe_source(), after))
            },
        )
    }

    /// Sets the uncertainty interval of generated timestamps, 100 ns
    /// units (B.1: "Operations that modify the accuracy of the time
    /// service or the uncertainty interval of generated timestamps").
    /// With an authoritative source the UTOs carry the larger of this
    /// value and the source's bound.
    ///
    /// # Errors
    /// [`AdminError::Unauthorized`], [`AdminError::AuditFailed`];
    /// [`AdminError::BadParam`] above 48 bits (§1.3.2.2).
    pub fn set_inaccuracy(
        &self,
        subject: &AdminSubject,
        inaccuracy: InaccuracyT,
    ) -> Result<(), AdminError> {
        self.inner.run(
            subject,
            AdminOp::SetInaccuracy,
            |s| alloc::format!("{}", s.inaccuracy),
            |before| {
                if inaccuracy > MAX_INACCURACY {
                    return Err((
                        alloc::format!("{inaccuracy} above 48 bits"),
                        AdminError::BadParam,
                    ));
                }
                let after = AdminState {
                    inaccuracy,
                    ..before.clone()
                };
                Ok((alloc::format!("{inaccuracy}"), after))
            },
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::TimeService;
    use crate::secure::{SecureTimeSource, SourceReading, SyncProtection};
    use alloc::vec::Vec;
    use core::sync::atomic::AtomicBool;

    /// Keeps every record; fails while `fail` is set.
    #[derive(Default)]
    struct Records {
        kept: Mutex<Vec<AuditRecord>>,
        fail: AtomicBool,
    }

    impl Records {
        fn all(&self) -> Vec<AuditRecord> {
            self.kept.lock().unwrap().clone()
        }
    }

    impl AuditSink for Records {
        fn record(&self, record: &AuditRecord) -> Result<(), AuditError> {
            if self.fail.load(Ordering::SeqCst) {
                return Err(AuditError(String::from("disk full")));
            }
            self.kept.lock().unwrap().push(record.clone());
            Ok(())
        }
    }

    /// A source whose reading a test sets.
    struct Manual(Mutex<Result<SourceReading, TimeUnavailable>>);

    impl Manual {
        fn new(time: TimeT, inaccuracy: InaccuracyT, synchronized: bool) -> Arc<Self> {
            Arc::new(Self(Mutex::new(Ok(SourceReading {
                time,
                inaccuracy,
                synchronized,
            }))))
        }
        fn set(&self, time: TimeT, inaccuracy: InaccuracyT, synchronized: bool) {
            *self.0.lock().unwrap() = Ok(SourceReading {
                time,
                inaccuracy,
                synchronized,
            });
        }
        fn fail(&self) {
            *self.0.lock().unwrap() = Err(TimeUnavailable);
        }
    }

    impl SecureTimeSource for Manual {
        fn read(&self) -> Result<SourceReading, TimeUnavailable> {
            *self.0.lock().unwrap()
        }
    }

    const BASE: TimeT = 138_000_000_000_000_000;

    fn admin_subject() -> AdminSubject {
        AdminSubject::new("timeadmin")
    }

    /// An administered service on a base clock at `BASE`, where only
    /// `timeadmin` may do anything.
    fn administered() -> (TimeService, TimeAdmin, Arc<Records>) {
        let records = Arc::new(Records::default());
        let only_admin = |s: &AdminSubject, _op: AdminOp| s.as_str() == "timeadmin";
        let (service, admin) = TimeService::with_source(|| BASE)
            .with_tdf(0)
            .with_inaccuracy(20)
            .unwrap()
            .administered(Arc::new(only_admin), records.clone());
        (service, admin, records)
    }

    fn designation(source: &Arc<Manual>, protection: SyncProtection) -> SourceDesignation {
        let source: Arc<dyn SecureTimeSource> = source.clone();
        SourceDesignation::new("kernel", protection, source)
    }

    #[test]
    fn time_b_1_default_authorizer_denies_every_operation() {
        let records = Arc::new(Records::default());
        let (service, admin) =
            TimeService::with_source(|| BASE).administered(Arc::new(DenyAll), records.clone());
        let anyone = AdminSubject::new("root");
        let src = Manual::new(BASE, 5, true);
        assert_eq!(
            admin.set_time(&anyone, BASE + 1),
            Err(AdminError::Unauthorized)
        );
        assert_eq!(admin.reset_time(&anyone), Err(AdminError::Unauthorized));
        assert_eq!(
            admin.designate_source(&anyone, designation(&src, SyncProtection::Nts)),
            Err(AdminError::Unauthorized)
        );
        assert_eq!(
            admin.withdraw_source(&anyone),
            Err(AdminError::Unauthorized)
        );
        assert_eq!(
            admin.set_inaccuracy(&anyone, 9),
            Err(AdminError::Unauthorized)
        );
        // Nothing changed, each refusal is a record.
        assert_eq!(service.universal_time().unwrap().time(), BASE);
        assert_eq!(service.inaccuracy(), 0);
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        let kept = records.all();
        assert_eq!(
            kept.iter().map(|r| r.op).collect::<Vec<_>>(),
            [
                AdminOp::SetTime,
                AdminOp::ResetTime,
                AdminOp::DesignateSource,
                AdminOp::DesignateSource,
                AdminOp::SetInaccuracy
            ]
        );
        assert!(kept.iter().all(|r| r.outcome == AuditOutcome::Denied));
        assert!(kept.iter().all(|r| r.subject == anyone && r.after == "-"));
    }

    #[test]
    fn time_b_1_set_time_unauthorized_is_refused_and_audited() {
        let (service, admin, records) = administered();
        let intruder = AdminSubject::new("intruder");
        assert_eq!(
            admin.set_time(&intruder, BASE + 1_000),
            Err(AdminError::Unauthorized)
        );
        assert_eq!(service.universal_time().unwrap().time(), BASE);
        let kept = records.all();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].subject, intruder);
        assert_eq!(kept[0].op, AdminOp::SetTime);
        assert_eq!(kept[0].outcome, AuditOutcome::Denied);
        assert_eq!(kept[0].before, "source time");
        // The record's time is the system clock.
        assert!(kept[0].time > BASE);
    }

    #[test]
    fn time_b_1_set_time_authorized_shifts_universal_time_and_is_audited() {
        let (service, admin, records) = administered();
        let clone = service.clone();
        admin.set_time(&admin_subject(), BASE + 5_000).unwrap();
        // The clones share the administration.
        assert_eq!(service.universal_time().unwrap().time(), BASE + 5_000);
        assert_eq!(clone.universal_time().unwrap().time(), BASE + 5_000);
        let relative = crate::Uto::new(7, 0, 0).unwrap();
        assert_eq!(
            service.absolute_time(relative).unwrap().time(),
            BASE + 5_007
        );
        admin.set_time(&admin_subject(), BASE - 3).unwrap();
        assert_eq!(service.universal_time().unwrap().time(), BASE - 3);
        let kept = records.all();
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|r| r.outcome == AuditOutcome::Granted));
        assert_eq!(kept[0].before, "source time");
        assert_eq!(
            kept[0].after,
            alloc::format!("time {} (source time +5000 ticks)", BASE + 5_000)
        );
        assert_eq!(kept[1].before, "source time +5000 ticks");
        assert_eq!(
            kept[1].after,
            alloc::format!("time {} (source time -3 ticks)", BASE - 3)
        );
        // Time 0 is no time: refused, and the attempt is a record.
        assert_eq!(
            admin.set_time(&admin_subject(), 0),
            Err(AdminError::BadParam)
        );
        assert_eq!(records.all()[2].outcome, AuditOutcome::Failed);
        assert_eq!(service.universal_time().unwrap().time(), BASE - 3);
    }

    #[test]
    fn time_b_1_reset_time_restores_the_source_reading() {
        let (service, admin, records) = administered();
        admin.set_time(&admin_subject(), BASE + 99).unwrap();
        admin.reset_time(&admin_subject()).unwrap();
        assert_eq!(service.universal_time().unwrap().time(), BASE);
        let last = records.all().pop().unwrap();
        assert_eq!(last.op, AdminOp::ResetTime);
        assert_eq!(last.before, "source time +99 ticks");
        assert_eq!(last.after, "source time");
    }

    #[test]
    fn time_b_1_designate_source_is_audited_before_it_takes_effect() {
        let (service, admin, records) = administered();
        let src = Manual::new(BASE + 777, 3, true);
        // The sink sees the record while the old source is still in
        // effect: it reads the service from inside `record`.
        struct Probe {
            service: Mutex<Option<TimeService>>,
            seen: Mutex<Vec<TimeT>>,
        }
        impl AuditSink for Probe {
            fn record(&self, _record: &AuditRecord) -> Result<(), AuditError> {
                if let Some(s) = self.service.lock().unwrap().as_ref() {
                    self.seen
                        .lock()
                        .unwrap()
                        .push(s.universal_time().unwrap().time());
                }
                Ok(())
            }
        }
        let probe = Arc::new(Probe {
            service: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
        });
        let (probed, probed_admin) = TimeService::with_source(|| BASE)
            .with_tdf(0)
            .administered(Arc::new(|_: &AdminSubject, _: AdminOp| true), probe.clone());
        *probe.service.lock().unwrap() = Some(probed.clone());
        probed_admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        assert_eq!(*probe.seen.lock().unwrap(), [BASE]);
        assert_eq!(probed.universal_time().unwrap().time(), BASE + 777);

        admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        assert_eq!(service.universal_time().unwrap().time(), BASE + 777);
        admin.withdraw_source(&admin_subject()).unwrap();
        assert_eq!(service.universal_time().unwrap().time(), BASE);
        let kept = records.all();
        assert_eq!(kept[0].op, AdminOp::DesignateSource);
        assert_eq!(kept[0].before, "base clock");
        assert_eq!(kept[0].after, "kernel (nts)");
        assert_eq!(kept[1].before, "kernel (nts)");
        assert_eq!(kept[1].after, "base clock");
    }

    #[test]
    fn time_b_1_set_inaccuracy_changes_generated_timestamps_and_is_audited() {
        let (service, admin, records) = administered();
        assert_eq!(service.universal_time().unwrap().inaccuracy(), 20);
        admin.set_inaccuracy(&admin_subject(), 4_000).unwrap();
        assert_eq!(service.inaccuracy(), 4_000);
        assert_eq!(service.universal_time().unwrap().inaccuracy(), 4_000);
        // With a source: the larger of the two.
        let src = Manual::new(BASE, 9_000, true);
        admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        assert_eq!(service.universal_time().unwrap().inaccuracy(), 9_000);
        src.set(BASE + 1, 10, true);
        assert_eq!(service.universal_time().unwrap().inaccuracy(), 4_000);
        assert_eq!(service.secure_universal_time().unwrap().inaccuracy(), 4_000);
        // Above 48 bits: refused and recorded as failed.
        assert_eq!(
            admin.set_inaccuracy(&admin_subject(), MAX_INACCURACY + 1),
            Err(AdminError::BadParam)
        );
        let kept = records.all();
        assert_eq!(kept[0].before, "20");
        assert_eq!(kept[0].after, "4000");
        let last = kept.last().unwrap();
        assert_eq!(last.op, AdminOp::SetInaccuracy);
        assert_eq!(last.outcome, AuditOutcome::Failed);
        assert_eq!(service.inaccuracy(), 4_000);
    }

    #[test]
    fn time_b_1_audit_failure_refuses_the_operation() {
        let (service, admin, records) = administered();
        records.fail.store(true, Ordering::SeqCst);
        let src = Manual::new(BASE + 1, 1, true);
        let failed = |r: Result<(), AdminError>| matches!(r, Err(AdminError::AuditFailed(_)));
        assert!(failed(admin.set_time(&admin_subject(), BASE + 50)));
        assert!(failed(admin.reset_time(&admin_subject())));
        assert!(failed(admin.designate_source(
            &admin_subject(),
            designation(&src, SyncProtection::Nts)
        )));
        assert!(failed(admin.set_inaccuracy(&admin_subject(), 1)));
        // A refusal whose record fails is refused as well.
        assert!(failed(admin.set_time(&AdminSubject::new("intruder"), 1)));
        assert_eq!(service.universal_time().unwrap().time(), BASE);
        assert_eq!(service.inaccuracy(), 20);
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        assert!(records.all().is_empty());
        records.fail.store(false, Ordering::SeqCst);
        admin.set_time(&admin_subject(), BASE + 50).unwrap();
        assert_eq!(service.universal_time().unwrap().time(), BASE + 50);
    }

    #[test]
    fn time_b_1_with_methods_on_an_administered_service_leave_the_administration() {
        let (service, admin, _records) = administered();
        let copy = service.clone().with_tdf(60);
        assert!(service.is_administered());
        assert!(!copy.is_administered());
        let src = Manual::new(BASE + 10, 1, true);
        admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        assert!(service.secure_universal_time().is_ok());
        // The copy is not changed by the admin and gives no secure time.
        assert_eq!(copy.universal_time().unwrap().time(), BASE);
        assert_eq!(copy.secure_universal_time(), Err(TimeUnavailable));
        let copy = service.clone().with_inaccuracy(1).unwrap();
        assert!(!copy.is_administered());
        assert_eq!(copy.secure_universal_time(), Err(TimeUnavailable));
    }

    #[test]
    fn time_a_1_secure_time_unavailable_without_designated_source() {
        let (service, _admin, _records) = administered();
        assert!(service.universal_time().is_ok());
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
    }

    #[test]
    fn time_a_1_secure_time_returns_source_time_and_inaccuracy() {
        let (service, admin, _records) = administered();
        let src = Manual::new(BASE + 400, 1_234, true);
        admin
            .designate_source(
                &admin_subject(),
                designation(&src, SyncProtection::LocalReference),
            )
            .unwrap();
        let uto = service.secure_universal_time().unwrap();
        assert_eq!(uto.time(), BASE + 400);
        assert_eq!(uto.inaccuracy(), 1_234);
        assert_eq!(uto.tdf(), 0);
    }

    #[test]
    fn time_a_1_secure_time_unavailable_when_source_reports_unsynchronized() {
        let (service, admin, _records) = administered();
        let src = Manual::new(BASE, 100, false);
        admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        // universal_time is not trusted: the time is returned.
        assert_eq!(service.universal_time().unwrap().time(), BASE);
        src.set(BASE + 1, 100, true);
        assert!(service.secure_universal_time().is_ok());
        src.fail();
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        assert_eq!(service.universal_time(), Err(TimeUnavailable));
    }

    #[test]
    fn time_a_1_secure_time_unavailable_for_unprotected_synchronization() {
        let (service, admin, _records) = administered();
        let src = Manual::new(BASE, 100, true);
        admin
            .designate_source(
                &admin_subject(),
                designation(&src, SyncProtection::Unprotected),
            )
            .unwrap();
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        admin
            .designate_source(
                &admin_subject(),
                designation(&src, SyncProtection::PtpAuthenticated),
            )
            .unwrap();
        assert!(service.secure_universal_time().is_ok());
    }

    #[test]
    fn time_a_1_secure_time_unavailable_when_reading_goes_backwards() {
        let (service, admin, _records) = administered();
        let src = Manual::new(BASE + 100, 1, true);
        admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        assert_eq!(service.secure_universal_time().unwrap().time(), BASE + 100);
        // The same tick again is not backwards.
        assert_eq!(service.secure_universal_time().unwrap().time(), BASE + 100);
        src.set(BASE + 99, 1, true);
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        // Another source does not start a new series.
        let other = Manual::new(BASE + 50, 1, true);
        admin
            .designate_source(&admin_subject(), designation(&other, SyncProtection::Nts))
            .unwrap();
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        other.set(BASE + 101, 1, true);
        assert_eq!(service.secure_universal_time().unwrap().time(), BASE + 101);
    }

    #[test]
    fn time_a_1_secure_time_unavailable_while_a_time_set_by_hand_is_in_effect() {
        let (service, admin, _records) = administered();
        let src = Manual::new(BASE, 1, true);
        admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        admin.set_time(&admin_subject(), BASE + 10).unwrap();
        assert_eq!(service.universal_time().unwrap().time(), BASE + 10);
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        admin.reset_time(&admin_subject()).unwrap();
        assert_eq!(service.secure_universal_time().unwrap().time(), BASE);
    }

    #[test]
    fn time_a_1_secure_time_unavailable_when_the_bound_exceeds_48_bits() {
        let (service, admin, _records) = administered();
        let src = Manual::new(BASE, MAX_INACCURACY + 1, true);
        admin
            .designate_source(&admin_subject(), designation(&src, SyncProtection::Nts))
            .unwrap();
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
        assert_eq!(service.universal_time(), Err(TimeUnavailable));
    }

    #[test]
    fn time_b_1_audit_record_display_names_every_field() {
        let record = AuditRecord {
            time: 1,
            subject: AdminSubject::new("ops"),
            op: AdminOp::SetInaccuracy,
            before: String::from("0"),
            after: String::from("10"),
            outcome: AuditOutcome::Granted,
        };
        assert_eq!(
            record.to_string(),
            "time=1 subject=ops op=set-inaccuracy outcome=granted before=0 after=10"
        );
    }
}
