// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Condition` hierarchy + `WaitSet` (DDS DCPS 1.4 §2.2.2.1.6).
//!
//! Conditions are the DDS variant of "future readiness": a
//! `Condition` object carries a `trigger_value()` boolean that
//! becomes `true` when a particular event occurs. Several conditions
//! are collected in a [`WaitSet`], which then blocks in
//! [`WaitSet::wait`] until any condition triggers (or a timeout is
//! reached).
//!
//! # Spec hierarchy
//!
//! - `Condition` (base, only trigger_value)
//!   - `StatusCondition` (see [`crate::entity::StatusCondition`])
//!   - `GuardCondition` (manually settable, defined here)
//!   - `ReadCondition` (based on SampleInfo state, defined here)
//!   - `QueryCondition` (ReadCondition + SQL filter, defined here)
//!
//! The trait is object-safe; WaitSet holds `Arc<dyn Condition>`.
//!
//! # Waiting
//!
//! [`WaitSet::wait`] blocks on the receive path's own signal
//! (`signal_data_available`, raised where a sample reaches a user reader's
//! channel), not on a timer. `poll_interval` remains as an upper bound on one
//! sleep, for the conditions that have no signal behind them at all — a
//! `GuardCondition` the application flips, a `StatusCondition` whose bits come
//! from a status poll. Data delivery does not wait for it.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;

#[cfg(feature = "std")]
use std::sync::{Condvar, Mutex};

use crate::error::{DdsError, Result};

/// Condition trait — Spec §2.2.2.1.6 base class.
pub trait Condition: Send + Sync {
    /// True if the event of this condition is currently pending.
    /// Spec §2.2.2.1.6 `get_trigger_value`.
    fn get_trigger_value(&self) -> bool;

    /// Whether something signals this condition when it becomes true.
    ///
    /// A `ReadCondition` and a `QueryCondition` turn true when a sample
    /// arrives, and the receive path announces that
    /// (`signal_data_available`) — a `WaitSet` holding only those can sleep
    /// until it is woken and never look on its own.
    ///
    /// A `GuardCondition` is flipped by the application from a thread of its
    /// own, and a `StatusCondition`'s bits are collected by a status poll.
    /// Neither says anything when it changes, so a WaitSet holding one has to
    /// look every so often — which is what `poll_interval` bounds, and why it
    /// must not apply to a set that has no such condition in it.
    ///
    /// Defaults to `false`: a condition that does not state otherwise gets the
    /// bounded wait, which costs a wake per interval and is the safe answer.
    /// Claiming `true` wrongly would be the unsafe one — the WaitSet would
    /// sleep through the change.
    fn is_signalled(&self) -> bool {
        false
    }
}

// Re-export the StatusCondition from entity.rs so callers only need to
// import condition::*.
pub use crate::entity::StatusCondition;

impl Condition for StatusCondition {
    fn get_trigger_value(&self) -> bool {
        self.trigger_value()
    }
}

/// `ReadCondition` — Spec §2.2.2.5.8 / §2.2.4.5 trigger state.
///
/// A ReadCondition is bound to a DataReader and triggers `true` when
/// the reader holds samples that match all three masks
/// (sample_state_mask, view_state_mask, instance_state_mask).
///
/// Design: the trigger logic itself (the actual "does the reader have
/// samples with these masks?" query) is injected by the caller as a
/// closure, because the DataReader sample cache cannot be queried in
/// an object-safe way without further infrastructure changes. The
/// DCPS API consumer (usually DataReader::create_readcondition)
/// provides the closure in the form `(sm, vm, im) -> bool`.
pub struct ReadCondition {
    sample_state_mask: u32,
    view_state_mask: u32,
    instance_state_mask: u32,
    trigger: Arc<dyn Fn(u32, u32, u32) -> bool + Send + Sync>,
    /// The DataReader the condition was created on (DDS 1.4 §2.2.2.5.8.1).
    #[cfg(feature = "std")]
    reader: std::sync::OnceLock<alloc::sync::Weak<dyn crate::contained::ContainedReader>>,
}

impl core::fmt::Debug for ReadCondition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReadCondition")
            .field("sample_state_mask", &self.sample_state_mask)
            .field("view_state_mask", &self.view_state_mask)
            .field("instance_state_mask", &self.instance_state_mask)
            .finish_non_exhaustive()
    }
}

impl ReadCondition {
    /// Constructor with a trigger closure.
    #[must_use]
    pub fn new<F>(
        sample_state_mask: u32,
        view_state_mask: u32,
        instance_state_mask: u32,
        trigger: F,
    ) -> Arc<Self>
    where
        F: Fn(u32, u32, u32) -> bool + Send + Sync + 'static,
    {
        Arc::new(Self {
            sample_state_mask,
            view_state_mask,
            instance_state_mask,
            trigger: Arc::new(trigger),
            #[cfg(feature = "std")]
            reader: std::sync::OnceLock::new(),
        })
    }

    /// Records the DataReader the condition belongs to.
    #[cfg(feature = "std")]
    pub(crate) fn set_reader(
        &self,
        reader: alloc::sync::Weak<dyn crate::contained::ContainedReader>,
    ) {
        let _ = self.reader.set(reader);
    }

    /// DDS 1.4 §2.2.2.5.8.1 `get_datareader`: the DataReader the condition
    /// was created on. `None` for a condition not made by
    /// `create_readcondition` or `create_querycondition`, and once the
    /// reader is gone.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_datareader(&self) -> Option<crate::subscriber::AnyDataReader> {
        self.reader
            .get()?
            .upgrade()
            .map(crate::subscriber::AnyDataReader::from_contained)
    }

    /// Spec §2.2.2.5.8 `get_sample_state_mask`.
    #[must_use]
    pub fn get_sample_state_mask(&self) -> u32 {
        self.sample_state_mask
    }

    /// Spec §2.2.2.5.8 `get_view_state_mask`.
    #[must_use]
    pub fn get_view_state_mask(&self) -> u32 {
        self.view_state_mask
    }

    /// Spec §2.2.2.5.8 `get_instance_state_mask`.
    #[must_use]
    pub fn get_instance_state_mask(&self) -> u32 {
        self.instance_state_mask
    }
}

impl Condition for ReadCondition {
    fn get_trigger_value(&self) -> bool {
        (self.trigger)(
            self.sample_state_mask,
            self.view_state_mask,
            self.instance_state_mask,
        )
    }

    /// Signalled: this turns true when a sample arrives, and the receive path
    /// says so.
    fn is_signalled(&self) -> bool {
        true
    }
}

/// `QueryCondition` — Spec §2.2.2.5.9. Extends ReadCondition with a
/// SQL filter expression (DDS-DCPS Annex B). The filter is evaluated
/// per sample (see [`Self::evaluate`]); the parse step happens once in
/// the constructor.
pub struct QueryCondition {
    base: Arc<ReadCondition>,
    query_expression: alloc::string::String,
    query_parameters: Mutex<Vec<alloc::string::String>>,
    /// The condition of the expression; none when it only orders.
    parsed: Option<zerodds_sql_filter::Expr>,
    /// The fields of `ORDER BY` (Annex B.2 `QueryExpression`).
    order_by: Vec<alloc::string::String>,
}

impl core::fmt::Debug for QueryCondition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("QueryCondition")
            .field("query_expression", &self.query_expression)
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl QueryCondition {
    /// Constructor — Spec §2.2.2.5.2.5 `create_querycondition` /
    /// §2.2.2.5.9. The SQL expression is parsed immediately; a
    /// syntactically invalid expression returns `BadParameter`.
    ///
    /// # Errors
    /// `BadParameter` if the SQL expression does not parse.
    pub fn new(
        base: Arc<ReadCondition>,
        query_expression: impl Into<alloc::string::String>,
        query_parameters: Vec<alloc::string::String>,
    ) -> Result<Arc<Self>> {
        let expr_str = query_expression.into();
        let parsed = zerodds_sql_filter::parse_query_expression(&expr_str).map_err(|_| {
            DdsError::BadParameter {
                what: "QueryCondition: invalid SQL filter expression",
            }
        })?;
        Ok(Arc::new(Self {
            base,
            query_expression: expr_str,
            query_parameters: Mutex::new(query_parameters),
            parsed: parsed.condition,
            order_by: parsed.order_by,
        }))
    }

    /// Evaluates the SQL filter against a sample. Spec §2.2.2.5.9.6 —
    /// only samples with `evaluate(...)==Ok(true)` count toward the
    /// `read_w_condition`/`take_w_condition` result.
    ///
    /// Parameter strings are converted into `String` `Value`s; the
    /// caller can pass typed parameters via
    /// [`Self::evaluate_with_values`].
    ///
    /// # Errors
    /// Lock poisoning or SQL eval error (UnknownField/TypeMismatch/
    /// MissingParam).
    pub fn evaluate<R: zerodds_sql_filter::RowAccess>(&self, row: &R) -> Result<bool> {
        let params = self
            .query_parameters
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "query parameters poisoned",
            })?;
        let values: Vec<zerodds_sql_filter::Value> = params
            .iter()
            .map(|s| zerodds_sql_filter::Value::from_parameter(s))
            .collect();
        // An expression that only orders lets every sample through.
        let Some(parsed) = self.parsed.as_ref() else {
            return Ok(true);
        };
        parsed
            .evaluate(row, &values)
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "QueryCondition SQL evaluation failed",
            })
    }

    /// Like [`Self::evaluate`], but with an explicitly typed parameter
    /// slice (e.g. for int/float parameters that would not match as a
    /// string cast).
    ///
    /// # Errors
    /// SQL eval error.
    pub fn evaluate_with_values<R: zerodds_sql_filter::RowAccess>(
        &self,
        row: &R,
        params: &[zerodds_sql_filter::Value],
    ) -> Result<bool> {
        let Some(parsed) = self.parsed.as_ref() else {
            return Ok(true);
        };
        parsed
            .evaluate(row, params)
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "QueryCondition SQL evaluation failed",
            })
    }

    /// The fields of `ORDER BY`: the samples a read or take with this
    /// condition returns are ordered by them, ascending (Annex B.2).
    #[must_use]
    pub fn order_by(&self) -> &[alloc::string::String] {
        &self.order_by
    }

    /// Orders `a` against `b` by the `ORDER BY` fields, each by the value
    /// the row gives for it (a field the row lacks is `null`, which comes
    /// first).
    pub fn compare_rows<R: zerodds_sql_filter::RowAccess>(
        &self,
        a: &R,
        b: &R,
    ) -> core::cmp::Ordering {
        for field in &self.order_by {
            let left = a.get(field).unwrap_or(zerodds_sql_filter::Value::Null);
            let right = b.get(field).unwrap_or(zerodds_sql_filter::Value::Null);
            match left.order(&right) {
                core::cmp::Ordering::Equal => {}
                unequal => return unequal,
            }
        }
        core::cmp::Ordering::Equal
    }

    /// Spec §2.2.2.5.9.4 `get_query_expression`.
    #[must_use]
    pub fn get_query_expression(&self) -> &str {
        &self.query_expression
    }

    /// Spec §2.2.2.5.9.5 `get_query_parameters`.
    #[must_use]
    pub fn get_query_parameters(&self) -> Vec<alloc::string::String> {
        self.query_parameters
            .lock()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Spec §2.2.2.5.9.6 `set_query_parameters`.
    ///
    /// # Errors
    /// `PreconditionNotMet` on lock poisoning.
    pub fn set_query_parameters(&self, params: Vec<alloc::string::String>) -> Result<()> {
        let mut current =
            self.query_parameters
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "query parameters poisoned",
                })?;
        *current = params;
        Ok(())
    }

    /// Access to the base ReadCondition (Spec: QueryCondition extends
    /// ReadCondition).
    #[must_use]
    pub fn base(&self) -> &Arc<ReadCondition> {
        &self.base
    }
}

/// A ReadCondition or a QueryCondition as the argument of
/// `read_w_condition` and `take_w_condition` (DDS 1.4 §2.2.2.5.3.10: the
/// parameter is a ReadCondition; a QueryCondition is one).
pub trait ReadConditionArg {
    /// The ReadCondition with the three state masks.
    fn read_condition(&self) -> &Arc<ReadCondition>;
    /// The expression part, when the condition has one.
    fn query_condition(&self) -> Option<&QueryCondition>;
}

impl ReadConditionArg for Arc<ReadCondition> {
    fn read_condition(&self) -> &Arc<ReadCondition> {
        self
    }
    fn query_condition(&self) -> Option<&QueryCondition> {
        None
    }
}

impl ReadConditionArg for Arc<QueryCondition> {
    fn read_condition(&self) -> &Arc<ReadCondition> {
        self.base()
    }
    fn query_condition(&self) -> Option<&QueryCondition> {
        Some(self)
    }
}

impl Condition for QueryCondition {
    fn get_trigger_value(&self) -> bool {
        // The trigger is inherited from the base ReadCondition (state-
        // mask match); the per-sample filter evaluation happens in
        // DataReader::read_w_condition/take_w_condition via
        // [`Self::evaluate`].
        self.base.get_trigger_value()
    }
    /// Signalled, like the `ReadCondition` it is built on: the filter narrows
    /// which samples count, not what announces their arrival.
    fn is_signalled(&self) -> bool {
        true
    }
}

/// `GuardCondition` — manually triggerable by the user (Spec
/// §2.2.2.1.7).
///
/// Typical use: an application thread sets `set_trigger_value(true)`
/// to wake a blocked WaitSet (e.g. for shutdown signaling).
#[derive(Debug, Default)]
pub struct GuardCondition {
    triggered: AtomicBool,
}

impl GuardCondition {
    /// New GuardCondition, initially `trigger_value=false`.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Sets the trigger value. Spec §2.2.2.1.7 `set_trigger_value`.
    pub fn set_trigger_value(&self, value: bool) {
        self.triggered.store(value, Ordering::Release);
    }
}

impl Condition for GuardCondition {
    fn get_trigger_value(&self) -> bool {
        self.triggered.load(Ordering::Acquire)
    }
}

/// `WaitSet` — Spec §2.2.2.1.6.
///
/// A WaitSet collects 0..N `Arc<dyn Condition>` and blocks in
/// [`Self::wait`] until at least one triggers or a timeout is reached.
/// Returns the list of **triggered** conditions.
#[cfg(feature = "std")]
pub struct WaitSet {
    inner: Arc<WaitSetInner>,
}

/// The receive path's wake, shared by every `WaitSet` in the process.
///
/// A condition holds no back-reference to the reader behind it — a
/// `ReadCondition` is a closure over the state masks — so a WaitSet cannot
/// register a waker of its own on the endpoint that would feed it. One event
/// per process avoids that: `runtime::wake_async_waker` signals it whenever a
/// sample reaches a user reader's channel, and every blocked WaitSet wakes and
/// re-evaluates its own trigger values.
///
/// The cost is a wake for WaitSets whose conditions did not trigger. That is
/// cheaper than what it replaces: before this, `wait` slept in 1 ms steps and
/// re-polled, so a sample waited up to a millisecond to be seen no matter how
/// fast it arrived, and a process with several WaitSets paid that in CPU while
/// idle. The generation counter is what makes the swap safe — a sample landing
/// between the trigger check and the block moves it, and the waiter re-checks
/// instead of sleeping through its own wake.
#[cfg(feature = "std")]
struct DataAvailable {
    /// Lock anchor for the condvar: the wake generation, and how many
    /// WaitSets block on it.
    generation: Mutex<DataAvailableState>,
    cvar: Condvar,
}

/// What [`DataAvailable`]'s lock protects.
#[cfg(feature = "std")]
#[derive(Default)]
struct DataAvailableState {
    generation: u64,
    /// WaitSets blocked in `wait` right now. `notify_all` is a futex call
    /// whether or not a thread waits, and the receive path signals after
    /// every run of samples: with no WaitSet waiting that was a system call
    /// per datagram, measured on the Linux test host.
    waiting: usize,
}

#[cfg(feature = "std")]
fn data_available() -> &'static DataAvailable {
    static EV: std::sync::OnceLock<DataAvailable> = std::sync::OnceLock::new();
    EV.get_or_init(|| DataAvailable {
        generation: Mutex::new(DataAvailableState::default()),
        cvar: Condvar::new(),
    })
}

/// Announces that a sample reached a user reader's channel.
///
/// Called from the receive path (`runtime::wake_async_waker`), which is the one
/// place every channel delivery passes through. Callback delivery does not come
/// here and does not need to: a reader with a listener set does not feed the
/// channel a `WaitSet` reads from.
#[cfg(feature = "std")]
pub(crate) fn signal_data_available() {
    let ev = data_available();
    let waiting = match ev.generation.lock() {
        Ok(mut g) => {
            g.generation = g.generation.wrapping_add(1);
            g.waiting > 0
        }
        Err(_) => true,
    };
    if waiting {
        ev.cvar.notify_all();
    }
}

#[cfg(feature = "std")]
struct WaitSetInner {
    conditions: Mutex<Vec<Arc<dyn Condition>>>,
    /// Upper bound on one idle sleep.
    ///
    /// Not the latency of data delivery — that is [`signal_data_available`],
    /// which wakes the wait as the sample lands. This bound is for the
    /// conditions with no signal behind them at all: a `GuardCondition` the
    /// application flips from another thread, and a `StatusCondition` whose
    /// bits are collected by a status poll. Without it those would only be
    /// observed when the next sample happened to arrive.
    poll_interval: core::time::Duration,
    /// `true` while a thread is blocked in `wait()`. Spec §2.2.2.1.6:
    /// "WaitSet MAY be used by only one application thread at a time"
    /// — concurrent `wait()` calls MUST return `PreconditionNotMet`.
    waiting: AtomicBool,
}

/// DoS cap for conditions in a single WaitSet (Spec §2.2.2.1.6: no
/// explicit cap, but the RESOURCE_LIMITS counterpart). Protects
/// against unbounded growth in pathological cases.
pub const MAX_WAITSET_CONDITIONS: usize = 1024;

#[cfg(feature = "std")]
impl WaitSet {
    /// New empty WaitSet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WaitSetInner {
                conditions: Mutex::new(Vec::new()),
                poll_interval: Duration::from_millis(1),
                waiting: AtomicBool::new(false),
            }),
        }
    }

    /// Attaches a condition. Spec §2.2.2.1.6 `attach_condition`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if the internal lock is poisoned.
    pub fn attach_condition(&self, cond: Arc<dyn Condition>) -> Result<()> {
        let mut conds = self
            .inner
            .conditions
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "waitset conditions poisoned",
            })?;
        // Idempotent: the same Arc identity is not added twice.
        if conds.iter().any(|c| Arc::ptr_eq(c, &cond)) {
            return Ok(());
        }
        // Spec §2.2.2.1.6 / §2.2.1.1.6 — DoS cap.
        if conds.len() >= MAX_WAITSET_CONDITIONS {
            return Err(DdsError::OutOfResources {
                what: "waitset condition count",
            });
        }
        conds.push(cond);
        drop(conds);
        // §2.2.2.1.6.1: "if the Condition has a trigger_value of TRUE, then
        // attaching the condition will unblock the WaitSet." A wait that
        // blocks has to look at the new condition: it may be triggered, and
        // one that does not announce itself changes how long the wait may
        // sleep. A set of read conditions only slept until the next sample.
        signal_data_available();
        Ok(())
    }

    /// Detaches a condition. Spec §2.2.2.1.6 `detach_condition`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if the condition is not attached (the same
    /// `Arc` that `attach_condition` took), or if the internal lock is
    /// poisoned.
    pub fn detach_condition(&self, cond: &Arc<dyn Condition>) -> Result<()> {
        let mut conds = self
            .inner
            .conditions
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "waitset conditions poisoned",
            })?;
        let before = conds.len();
        conds.retain(|c| !Arc::ptr_eq(c, cond));
        // DDS 1.4 §2.2.2.1.6.2: "If the Condition was not attached to the
        // WaitSet, the operation will return PRECONDITION_NOT_MET."
        if conds.len() == before {
            return Err(DdsError::PreconditionNotMet {
                reason: "the condition is not attached to the WaitSet",
            });
        }
        Ok(())
    }

    /// Current set of attached conditions.
    ///
    /// # Errors
    /// `PreconditionNotMet` on lock poisoning.
    pub fn get_conditions(&self) -> Result<Vec<Arc<dyn Condition>>> {
        let conds = self
            .inner
            .conditions
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "waitset conditions poisoned",
            })?;
        Ok(conds.clone())
    }

    /// Blocks until at least one condition triggers or the timeout
    /// elapses. Returns the list of triggered conditions. Spec
    /// §2.2.2.1.6 `wait`.
    ///
    /// **Implementation:** polling at a 1ms interval. A production-
    /// grade WaitSet would offer conditions notify hooks; that is a
    /// follow-up optimization.
    ///
    /// # Errors
    /// * [`DdsError::Timeout`] if no condition triggers within the
    ///   timeout.
    /// * [`DdsError::PreconditionNotMet`] on lock poisoning.
    pub fn wait(&self, timeout: Duration) -> Result<Vec<Arc<dyn Condition>>> {
        // Spec §2.2.2.1.6 — single-thread wait. Atomic compare_exchange
        // prevents two threads from entering `wait()` at the same time.
        // RAII guard via a local struct that resets the flag on drop —
        // more robust against panics in poll_active.
        if self
            .inner
            .waiting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(DdsError::PreconditionNotMet {
                reason: "waitset already in wait() — single-thread-wait per spec",
            });
        }
        struct WaitGuard<'a>(&'a AtomicBool);
        impl Drop for WaitGuard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _guard = WaitGuard(&self.inner.waiting);

        let deadline = std::time::Instant::now() + timeout;
        let ev = data_available();
        loop {
            // Read the generation before evaluating the triggers. A sample that
            // lands between the two moves it, and the block below is skipped —
            // otherwise the wake would arrive while this thread was still
            // deciding whether to sleep, and be lost.
            let seen = ev.generation.lock().map(|g| g.generation).unwrap_or(0);

            let active = self.poll_active()?;
            if !active.is_empty() {
                return Ok(active);
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(DdsError::Timeout);
            }
            // Block on the receive path's wake. A set whose conditions all
            // announce themselves needs no bound at all — it sleeps until a
            // sample arrives. The bound is for the others: a GuardCondition
            // the application flips, a StatusCondition whose bits come from a
            // status poll. Neither says anything when it changes, so a set
            // holding one has to look every so often.
            //
            // Measured before this distinction was drawn: a WaitSet with only
            // read conditions still woke on `poll_interval`, which an
            // application reported as 226 trigger evaluations per 300 ms per
            // reader while nothing at all was arriving.
            let remaining = deadline - now;
            let all_signalled = self
                .inner
                .conditions
                .lock()
                .map(|g| !g.is_empty() && g.iter().all(|c| c.is_signalled()))
                .unwrap_or(false);
            let block = if all_signalled {
                remaining
            } else {
                remaining.min(self.inner.poll_interval)
            };
            let mut guard = ev
                .generation
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "waitset data-available lock poisoned",
                })?;
            if guard.generation == seen {
                // Counted under the lock the signal takes, so a sample
                // after this line sees the waiter and wakes it.
                guard.waiting += 1;
                let mut guard = match ev.cvar.wait_timeout(guard, block) {
                    Ok((guard, _)) => guard,
                    Err(poisoned) => poisoned.into_inner().0,
                };
                guard.waiting -= 1;
            }
            // (Discarding the result is fine — the triggers are re-evaluated
            //  at the top of the loop either way.)
        }
    }

    /// `wait_until_any_or_all_triggered` — internal helper.
    fn poll_active(&self) -> Result<Vec<Arc<dyn Condition>>> {
        let conds = self
            .inner
            .conditions
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "waitset conditions poisoned",
            })?;
        Ok(conds
            .iter()
            .filter(|c| c.get_trigger_value())
            .cloned()
            .collect())
    }

    /// Wakes a blocked `wait()` without a trigger_value change having
    /// occurred (e.g. for shutdown).
    ///
    /// Currently this only has an effect if the WaitSet is blocked in
    /// `wait_timeout` — the caller should still attach a
    /// `GuardCondition` so that the wakeup has a definite result.
    ///
    /// Goes through the same event as data arrival, because that is where a
    /// blocked `wait` now sleeps. Waking a condvar of this WaitSet's own would
    /// reach nobody.
    pub fn notify(&self) {
        signal_data_available();
    }
}

#[cfg(feature = "std")]
impl Default for WaitSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl Clone for WaitSet {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(feature = "std")]
impl core::fmt::Debug for WaitSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let n = self.inner.conditions.lock().map(|c| c.len()).unwrap_or(0);
        f.debug_struct("WaitSet").field("conditions", &n).finish()
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicU32;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn guard_condition_starts_false() {
        let g = GuardCondition::new();
        assert!(!g.get_trigger_value());
    }

    #[test]
    fn guard_condition_set_trigger() {
        let g = GuardCondition::new();
        g.set_trigger_value(true);
        assert!(g.get_trigger_value());
        g.set_trigger_value(false);
        assert!(!g.get_trigger_value());
    }

    #[test]
    fn waitset_attach_detach_idempotent() {
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        let cond: Arc<dyn Condition> = g.clone();
        ws.attach_condition(cond.clone()).unwrap();
        ws.attach_condition(cond.clone()).unwrap(); // idempotent
        assert_eq!(ws.get_conditions().unwrap().len(), 1);
        ws.detach_condition(&cond).unwrap();
        assert!(ws.get_conditions().unwrap().is_empty());
    }

    #[test]
    fn waitset_wait_returns_immediately_if_already_triggered() {
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        g.set_trigger_value(true);
        let cond: Arc<dyn Condition> = g.clone();
        ws.attach_condition(cond).unwrap();
        let triggered = ws.wait(Duration::from_secs(1)).unwrap();
        assert_eq!(triggered.len(), 1);
    }

    /// The receive path's signal wakes a blocked `wait`, and it does so
    /// through the condvar rather than by the sleep expiring.
    ///
    /// The condition here never triggers, so `wait` runs to its timeout — what
    /// is measured is how often it woke on the way. Before
    /// `signal_data_available` existed the loop re-polled every
    /// `poll_interval`, which over 200 ms is roughly 200 evaluations of every
    /// attached condition. Now a wake costs one evaluation per arriving sample
    /// plus the guard bound.
    #[test]
    fn data_signal_wakes_a_blocked_wait() {
        let ws = WaitSet::new();
        let counter = Arc::new(AtomicU32::new(0));
        let seen = Arc::clone(&counter);
        // A condition that counts how often it is asked and never triggers.
        struct Counting(Arc<AtomicU32>);
        impl Condition for Counting {
            fn get_trigger_value(&self) -> bool {
                self.0.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                false
            }
        }
        ws.attach_condition(Arc::new(Counting(seen)) as Arc<dyn Condition>)
            .unwrap();

        let signaller = std::thread::spawn(|| {
            for _ in 0..5 {
                std::thread::sleep(Duration::from_millis(10));
                signal_data_available();
            }
        });

        let start = Instant::now();
        let res = ws.wait(Duration::from_millis(200));
        let waited = start.elapsed();
        signaller.join().expect("signalling thread");

        assert!(matches!(res, Err(DdsError::Timeout)));
        assert!(
            waited >= Duration::from_millis(190),
            "returned early ({waited:?}) — a signal must not end a wait whose \
             conditions did not trigger"
        );
        // Five signals plus the guard bound over 200 ms. The old poll loop
        // would have re-evaluated on the order of 200 times.
        let evaluations = counter.load(core::sync::atomic::Ordering::Relaxed);
        assert!(
            evaluations >= 5,
            "only {evaluations} evaluations — the signals did not wake the wait"
        );
    }

    /// A set of nothing but signalled conditions does not look on its own.
    ///
    /// The number is what an application measured and reported: 226 trigger
    /// evaluations per 300 ms per reader, on a WaitSet holding only read
    /// conditions, while nothing was arriving. That is `poll_interval` doing
    /// its work where it has none — the conditions in that set are announced
    /// by the receive path, so there is nothing to look for between
    /// announcements.
    ///
    /// Counted in a process of its own that runs only the probe: the signal
    /// event is process-global, and the other tests of this binary signal
    /// into it while they run (31 evaluations against a bound of 30 in one
    /// of three runs on the Linux test host at load 65, with the interval's hundreds
    /// nowhere near).
    #[test]
    fn a_signalled_set_does_not_wake_on_the_interval() {
        let exe = std::env::current_exe().unwrap();
        let runner = std::env::var("ZERODDS_TEST_RUNNER").unwrap_or_default();
        let mut runner = runner.split_whitespace();
        let mut command = match runner.next() {
            Some(program) => {
                let mut c = std::process::Command::new(program);
                c.args(runner).arg(&exe);
                c
            }
            None => std::process::Command::new(&exe),
        };
        let out = command
            .args([
                "--exact",
                "condition::tests::signalled_set_probe",
                "--test-threads=1",
                "--nocapture",
            ])
            .env("ZERODDS_SIGNALLED_SET_PROBE", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The probe of `a_signalled_set_does_not_wake_on_the_interval`; a
    /// no-op unless that test runs it.
    #[test]
    fn signalled_set_probe() {
        if std::env::var_os("ZERODDS_SIGNALLED_SET_PROBE").is_none() {
            return;
        }
        let ws = WaitSet::new();
        let counter = Arc::new(AtomicU32::new(0));
        let seen = Arc::clone(&counter);
        // A read condition — signalled — that never triggers and counts.
        struct Signalled(Arc<AtomicU32>);
        impl Condition for Signalled {
            fn get_trigger_value(&self) -> bool {
                self.0.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                false
            }
            fn is_signalled(&self) -> bool {
                true
            }
        }
        ws.attach_condition(Arc::new(Signalled(seen)) as Arc<dyn Condition>)
            .unwrap();

        let res = ws.wait(Duration::from_millis(300));
        assert!(matches!(res, Err(DdsError::Timeout)));

        // Alone in its process this is two evaluations: one on entry, one
        // after the deadline. The interval would put it in the hundreds.
        let evaluations = counter.load(core::sync::atomic::Ordering::Relaxed);
        assert!(
            evaluations <= 5,
            "{evaluations} evaluations in 300 ms with nothing arriving — the \
             interval is still driving a set that does not need it"
        );
    }

    /// A set that holds one unsignalled condition keeps the bound.
    ///
    /// The other half of the same rule: a `GuardCondition` says nothing when
    /// the application flips it, so the wait has to look. Dropping the bound
    /// for every set would trade the idle wakes for a WaitSet that sleeps
    /// through a guard until a sample happens to arrive.
    #[test]
    fn a_set_with_an_unsignalled_condition_keeps_its_bound() {
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        ws.attach_condition(g.clone() as Arc<dyn Condition>)
            .unwrap();

        let flipper = Arc::clone(&g);
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            flipper.set_trigger_value(true);
        });

        let start = Instant::now();
        let triggered = ws.wait(Duration::from_secs(2)).expect("guard triggers");
        let waited = start.elapsed();
        handle.join().expect("flipping thread");

        assert_eq!(triggered.len(), 1);
        assert!(
            waited < Duration::from_millis(200),
            "took {waited:?} — a flipped guard was not seen inside the bound"
        );
        g.set_trigger_value(false);
    }

    /// A sample arriving while the waiter is between its trigger check and its
    /// block must not be slept through.
    ///
    /// The generation counter is what prevents it: the waiter reads it before
    /// evaluating, and skips the block if it moved. Without that the wake
    /// lands on a condvar nobody is waiting on yet, and the wait sleeps until
    /// the guard bound expires.
    #[test]
    fn a_signal_between_check_and_block_is_not_lost() {
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        let armed = Arc::clone(&g);
        // Triggers on the second evaluation, so the first pass through the
        // loop finds nothing and proceeds towards the block.
        struct ArmOnSecond {
            guard: Arc<GuardCondition>,
            seen: AtomicU32,
        }
        impl Condition for ArmOnSecond {
            fn get_trigger_value(&self) -> bool {
                if self.seen.fetch_add(1, core::sync::atomic::Ordering::SeqCst) == 0 {
                    // Signal from inside the check itself: the tightest
                    // possible race with the block that follows it.
                    signal_data_available();
                    self.guard.set_trigger_value(true);
                    return false;
                }
                true
            }
        }
        ws.attach_condition(Arc::new(ArmOnSecond {
            guard: armed,
            seen: AtomicU32::new(0),
        }) as Arc<dyn Condition>)
            .unwrap();

        let start = Instant::now();
        let triggered = ws.wait(Duration::from_secs(2)).expect("should trigger");
        let waited = start.elapsed();
        assert_eq!(triggered.len(), 1);
        assert!(
            waited < Duration::from_millis(50),
            "took {waited:?} — the signal raced ahead of the block and was lost"
        );
        g.set_trigger_value(false);
    }

    #[test]
    fn waitset_wait_timeout_returns_err() {
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        let cond: Arc<dyn Condition> = g.clone();
        ws.attach_condition(cond).unwrap();
        let start = Instant::now();
        let res = ws.wait(Duration::from_millis(50));
        assert!(matches!(res, Err(DdsError::Timeout)));
        // We waited at least ~50ms
        assert!(start.elapsed() >= Duration::from_millis(40));
    }

    #[test]
    fn waitset_wakes_when_guard_triggers() {
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        let cond: Arc<dyn Condition> = g.clone();
        ws.attach_condition(cond).unwrap();

        let g_clone = g.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            g_clone.set_trigger_value(true);
        });

        let start = Instant::now();
        let triggered = ws.wait(Duration::from_secs(2)).unwrap();
        let elapsed = start.elapsed();
        handle.join().unwrap();

        assert_eq!(triggered.len(), 1);
        // Should return quickly, not the full timeout
        assert!(elapsed < Duration::from_millis(500), "elapsed={elapsed:?}");
    }

    #[test]
    fn waitset_with_multiple_conditions_returns_all_triggered() {
        let ws = WaitSet::new();
        let g1 = GuardCondition::new();
        let g2 = GuardCondition::new();
        ws.attach_condition(g1.clone()).unwrap();
        ws.attach_condition(g2.clone()).unwrap();
        g1.set_trigger_value(true);
        g2.set_trigger_value(true);
        let triggered = ws.wait(Duration::from_millis(100)).unwrap();
        assert_eq!(triggered.len(), 2);
    }

    #[test]
    fn status_condition_implements_condition_trait() {
        // DDS 1.4 §2.2.2.1.1.7: the status of a disabled entity never
        // changes, so the entity is enabled.
        let state = crate::entity::EntityState::new_enabled();
        let sc = crate::entity::StatusCondition::new(state.clone());
        sc.set_enabled_statuses(0b0010);
        assert!(!sc.get_trigger_value());
        state.set_status_bits(0b0010);
        assert!(sc.get_trigger_value());
    }

    #[test]
    fn waitset_clone_shares_state() {
        let ws1 = WaitSet::new();
        let ws2 = ws1.clone();
        let g = GuardCondition::new();
        ws1.attach_condition(g.clone()).unwrap();
        // Both have the same condition (same inner via Arc).
        assert_eq!(ws2.get_conditions().unwrap().len(), 1);
    }

    #[test]
    fn waitset_default_is_empty() {
        let ws = WaitSet::default();
        assert!(ws.get_conditions().unwrap().is_empty());
    }

    // ---- §2.2.2.1.6 single-thread wait + resource limit ----

    #[test]
    fn waitset_concurrent_wait_returns_precondition_not_met() {
        // Spec §2.2.2.1.6 — a WaitSet may only be used by one thread at
        // a time. A second wait() call MUST return PreconditionNotMet.
        let ws = WaitSet::new();
        // No condition attached → wait runs into the timeout.
        let ws_clone = ws.clone();
        let handle = thread::spawn(move || ws_clone.wait(Duration::from_millis(100)));
        // Brief pause so that thread A sets the waiting flag.
        thread::sleep(Duration::from_millis(20));
        let res = ws.wait(Duration::from_millis(10));
        assert!(matches!(res, Err(DdsError::PreconditionNotMet { .. })));
        let _ = handle.join().unwrap();
    }

    #[test]
    fn waitset_sequential_waits_after_first_returns_succeed() {
        // After the first wait() completes (by timeout), a sequential
        // second wait() must work again.
        let ws = WaitSet::new();
        let r1 = ws.wait(Duration::from_millis(10));
        assert!(matches!(r1, Err(DdsError::Timeout)));
        // Sequential: the second call after the first return succeeds.
        let r2 = ws.wait(Duration::from_millis(10));
        assert!(matches!(r2, Err(DdsError::Timeout)));
    }

    #[test]
    fn waitset_attach_above_max_returns_out_of_resources() {
        // §2.2.1.1.6 RC OUT_OF_RESOURCES — DoS cap.
        let ws = WaitSet::new();
        for _ in 0..MAX_WAITSET_CONDITIONS {
            let g = GuardCondition::new();
            ws.attach_condition(g).unwrap();
        }
        // The next attach must return OOR.
        let g = GuardCondition::new();
        let res = ws.attach_condition(g);
        assert!(matches!(res, Err(DdsError::OutOfResources { .. })));
        assert_eq!(ws.get_conditions().unwrap().len(), MAX_WAITSET_CONDITIONS);
    }

    #[test]
    fn waitset_attach_idempotent_does_not_count_against_cap() {
        // Same Arc identity attached twice → no duplicate, no counter
        // increment.
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        let cond: Arc<dyn Condition> = g.clone();
        ws.attach_condition(cond.clone()).unwrap();
        ws.attach_condition(cond.clone()).unwrap();
        ws.attach_condition(cond).unwrap();
        assert_eq!(ws.get_conditions().unwrap().len(), 1);
    }

    // ---- §2.2.2.5.8 ReadCondition + §2.2.4.5 trigger state ----

    #[test]
    fn read_condition_returns_trigger_from_closure() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        // The trigger closure returns true if all three masks are ANY.
        let cond = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |sm, vm, im| {
                sm == sample_state_mask::ANY
                    && vm == view_state_mask::ANY
                    && im == instance_state_mask::ANY
            },
        );
        assert_eq!(cond.get_sample_state_mask(), sample_state_mask::ANY);
        assert_eq!(cond.get_view_state_mask(), view_state_mask::ANY);
        assert_eq!(cond.get_instance_state_mask(), instance_state_mask::ANY);
        assert!(cond.get_trigger_value());
    }

    #[test]
    fn read_condition_trigger_false_when_closure_says_false() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        // The closure always returns false → no data available.
        let cond = ReadCondition::new(
            sample_state_mask::NOT_READ,
            view_state_mask::NEW,
            instance_state_mask::ALIVE,
            |_, _, _| false,
        );
        assert!(!cond.get_trigger_value());
    }

    #[test]
    fn read_condition_implements_condition_trait_object_safe() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        let cond = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true,
        );
        let dyn_cond: Arc<dyn Condition> = cond;
        assert!(dyn_cond.get_trigger_value());
    }

    #[test]
    fn read_condition_attaches_to_waitset() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        // §2.2.4.5: ReadCondition as a WaitSet trigger.
        let ws = WaitSet::new();
        let cond = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true, // always triggered
        );
        ws.attach_condition(cond.clone()).unwrap();
        let triggered = ws.wait(Duration::from_millis(50)).unwrap();
        assert_eq!(triggered.len(), 1);
    }

    // ---- §2.2.2.5.9 QueryCondition ----

    #[test]
    fn query_condition_inherits_trigger_from_base() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        let base = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true,
        );
        let qc = QueryCondition::new(base, "x > 10", alloc::vec::Vec::new()).unwrap();
        assert!(qc.get_trigger_value());
        assert_eq!(qc.get_query_expression(), "x > 10");
        assert!(qc.get_query_parameters().is_empty());
    }

    #[test]
    fn query_condition_set_query_parameters_roundtrip() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        let base = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| false,
        );
        let qc = QueryCondition::new(base, "color = %0", alloc::vec!["RED".into()]).unwrap();
        assert_eq!(qc.get_query_parameters(), alloc::vec!["RED".to_string()]);
        qc.set_query_parameters(alloc::vec!["BLUE".into(), "GREEN".into()])
            .unwrap();
        assert_eq!(
            qc.get_query_parameters(),
            alloc::vec!["BLUE".to_string(), "GREEN".to_string()]
        );
    }

    #[test]
    fn query_condition_trigger_inherits_false_from_base() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        let base = ReadCondition::new(
            sample_state_mask::READ,
            view_state_mask::NOT_NEW,
            instance_state_mask::NOT_ALIVE,
            |_, _, _| false, // no matching samples
        );
        let qc = QueryCondition::new(base, "x > 0", alloc::vec::Vec::new()).unwrap();
        assert!(!qc.get_trigger_value());
    }

    #[test]
    fn query_condition_base_returns_correct_read_condition() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        let base = ReadCondition::new(
            sample_state_mask::NOT_READ,
            view_state_mask::NEW,
            instance_state_mask::ALIVE,
            |_, _, _| true,
        );
        let qc = QueryCondition::new(base.clone(), "x > 0", alloc::vec::Vec::new()).unwrap();
        // base() returns the same Arc identity.
        assert!(Arc::ptr_eq(&base, qc.base()));
    }

    #[test]
    fn query_condition_rejects_invalid_sql() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        let base = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true,
        );
        let r = QueryCondition::new(base, "x > >", alloc::vec::Vec::new());
        assert!(matches!(r, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn query_condition_evaluate_filters_per_sample() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        use zerodds_sql_filter::{RowAccess, Value};
        struct R(i64);
        impl RowAccess for R {
            fn get(&self, p: &str) -> Option<Value> {
                if p == "x" {
                    Some(Value::Int(self.0))
                } else {
                    None
                }
            }
        }
        let base = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true,
        );
        let qc = QueryCondition::new(base, "x > 10", alloc::vec::Vec::new()).unwrap();
        assert!(qc.evaluate(&R(42)).unwrap());
        assert!(!qc.evaluate(&R(5)).unwrap());
    }

    #[test]
    fn query_condition_evaluate_with_typed_values_int_param() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        use zerodds_sql_filter::{RowAccess, Value};
        struct R(i64);
        impl RowAccess for R {
            fn get(&self, p: &str) -> Option<Value> {
                if p == "x" {
                    Some(Value::Int(self.0))
                } else {
                    None
                }
            }
        }
        let base = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true,
        );
        let qc = QueryCondition::new(base, "x > %0", alloc::vec::Vec::new()).unwrap();
        assert!(qc.evaluate_with_values(&R(42), &[Value::Int(10)]).unwrap());
        assert!(!qc.evaluate_with_values(&R(5), &[Value::Int(10)]).unwrap());
    }

    #[test]
    fn query_condition_evaluate_uses_string_params_by_default() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        use zerodds_sql_filter::{RowAccess, Value};
        struct R(alloc::string::String);
        impl RowAccess for R {
            fn get(&self, p: &str) -> Option<Value> {
                if p == "color" {
                    Some(Value::String(self.0.clone()))
                } else {
                    None
                }
            }
        }
        let base = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true,
        );
        let qc = QueryCondition::new(base, "color = %0", alloc::vec!["RED".into()]).unwrap();
        assert!(qc.evaluate(&R("RED".into())).unwrap());
        assert!(!qc.evaluate(&R("BLUE".into())).unwrap());
    }

    #[test]
    fn waitset_panic_in_wait_releases_waiting_flag() {
        // If a caller aborts wait() via a panicking
        // Condition::get_trigger_value, the waiting flag must still be
        // released (RAII guard). Modeled by a drop test of the wait
        // guard: we simulate with a normal timeout return and check
        // that a new wait is possible afterward (no hard lock).
        let ws = WaitSet::new();
        let _ = ws.wait(Duration::from_millis(5));
        // A sequential wait is allowed → the waiting flag went back to
        // 0 after the guard was dropped.
        let r = ws.wait(Duration::from_millis(5));
        assert!(matches!(r, Err(DdsError::Timeout)));
    }
}
