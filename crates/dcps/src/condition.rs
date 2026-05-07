// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Condition`-Hierarchie + `WaitSet` (DDS DCPS 1.4 §2.2.2.1.6).
//!
//! Conditions sind die DDS-Variante von "future-readyness": ein
//! `Condition`-Objekt traegt einen `trigger_value()`-Boolean, der
//! `true` wird, wenn ein bestimmtes Ereignis eintritt. Mehrere
//! Conditions werden in einem [`WaitSet`] gesammelt, der dann
//! mit [`WaitSet::wait`] blockiert, bis irgendeine Condition triggert
//! (oder ein Timeout erreicht ist).
//!
//! # Spec-Hierarchie
//!
//! - `Condition` (Base, nur trigger_value)
//!   - `StatusCondition` (siehe [`crate::entity::StatusCondition`])
//!   - `GuardCondition` (manuell setzbar, hier definiert)
//!   - `ReadCondition` (basiert auf SampleInfo-State, hier definiert)
//!   - `QueryCondition` (ReadCondition + SQL-Filter, hier definiert)
//!
//! Der Trait ist object-safe; WaitSet haelt `Arc<dyn Condition>`.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;

#[cfg(feature = "std")]
use std::sync::{Condvar, Mutex};

use crate::error::{DdsError, Result};

/// Condition-Trait — Spec §2.2.2.1.6 Base-Class.
pub trait Condition: Send + Sync {
    /// True wenn das Ereignis dieser Condition aktuell ansteht.
    /// Spec §2.2.2.1.6 `get_trigger_value`.
    fn get_trigger_value(&self) -> bool;
}

// Re-export der StatusCondition aus entity.rs damit Caller nur condition::*
// importieren muessen.
pub use crate::entity::StatusCondition;

impl Condition for StatusCondition {
    fn get_trigger_value(&self) -> bool {
        self.trigger_value()
    }
}

/// `ReadCondition` — Spec §2.2.2.5.8 / §2.2.4.5 Trigger State.
///
/// Eine ReadCondition ist an einen DataReader gebunden und triggert
/// `true`, wenn der Reader Samples enthaelt, die in alle drei Masks
/// (sample_state_mask, view_state_mask, instance_state_mask) passen.
///
/// Design: Die Trigger-Logik selbst (das eigentliche
/// "hat der Reader Samples mit diesen Masks?"-Query) ist vom Caller
/// als Closure injiziert, weil der DataReader-Sample-Cache nicht
/// objekt-safe gequeried werden kann ohne weitere Infrastructure-
/// Aenderungen. Der DCPS-API-Konsument (idR DataReader::create_readcondition)
/// liefert die Closure in Form `(sm, vm, im) -> bool`.
pub struct ReadCondition {
    sample_state_mask: u32,
    view_state_mask: u32,
    instance_state_mask: u32,
    trigger: Arc<dyn Fn(u32, u32, u32) -> bool + Send + Sync>,
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
    /// Konstruktor mit Trigger-Closure.
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
        })
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
}

/// `QueryCondition` — Spec §2.2.2.5.9. Erweitert ReadCondition um
/// einen SQL-Filter-Ausdruck (DDS-DCPS Annex B). Der Filter wird
/// pro Sample evaluiert (siehe [`Self::evaluate`]); der parse-Schritt
/// passiert einmalig im Konstruktor.
pub struct QueryCondition {
    base: Arc<ReadCondition>,
    query_expression: alloc::string::String,
    query_parameters: Mutex<Vec<alloc::string::String>>,
    parsed: zerodds_sql_filter::Expr,
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
    /// Konstruktor — Spec §2.2.2.5.2.5 `create_querycondition` /
    /// §2.2.2.5.9. Der SQL-Ausdruck wird sofort geparst; eine syntaktisch
    /// ungueltige Expression liefert `BadParameter`.
    ///
    /// # Errors
    /// `BadParameter` wenn der SQL-Ausdruck nicht parst.
    pub fn new(
        base: Arc<ReadCondition>,
        query_expression: impl Into<alloc::string::String>,
        query_parameters: Vec<alloc::string::String>,
    ) -> Result<Arc<Self>> {
        let expr_str = query_expression.into();
        let parsed = zerodds_sql_filter::parse(&expr_str).map_err(|_| DdsError::BadParameter {
            what: "QueryCondition: invalid SQL filter expression",
        })?;
        Ok(Arc::new(Self {
            base,
            query_expression: expr_str,
            query_parameters: Mutex::new(query_parameters),
            parsed,
        }))
    }

    /// Evaluiert den SQL-Filter gegen ein Sample. Spec §2.2.2.5.9.6 —
    /// nur Samples mit `evaluate(...)==Ok(true)` zaehlen fuer das
    /// `read_w_condition`/`take_w_condition`-Resultat.
    ///
    /// Parameter-Strings werden in `String`-`Value`s konvertiert; der
    /// Caller kann typed Parameter ueber [`Self::evaluate_with_values`]
    /// uebergeben.
    ///
    /// # Errors
    /// Lock-Poisoning oder SQL-Eval-Error (UnknownField/TypeMismatch/
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
            .map(|s| zerodds_sql_filter::Value::String(s.clone()))
            .collect();
        self.parsed
            .evaluate(row, &values)
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "QueryCondition SQL evaluation failed",
            })
    }

    /// Wie [`Self::evaluate`], aber mit explizit typed Parameter-Slice
    /// (z.B. fuer Int/Float-Parameter, die als String-Cast nicht
    /// matchen wuerden).
    ///
    /// # Errors
    /// SQL-Eval-Error.
    pub fn evaluate_with_values<R: zerodds_sql_filter::RowAccess>(
        &self,
        row: &R,
        params: &[zerodds_sql_filter::Value],
    ) -> Result<bool> {
        self.parsed
            .evaluate(row, params)
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "QueryCondition SQL evaluation failed",
            })
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
    /// `PreconditionNotMet` bei Lock-Poisoning.
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

    /// Zugriff auf die Base-ReadCondition (Spec: QueryCondition
    /// extends ReadCondition).
    #[must_use]
    pub fn base(&self) -> &Arc<ReadCondition> {
        &self.base
    }
}

impl Condition for QueryCondition {
    fn get_trigger_value(&self) -> bool {
        // Trigger erbt von der Base-ReadCondition (State-Mask-Match);
        // die per-Sample-Filter-Evaluierung passiert in DataReader::
        // read_w_condition/take_w_condition via [`Self::evaluate`].
        self.base.get_trigger_value()
    }
}

/// `GuardCondition` — vom User manuell triggerbar (Spec §2.2.2.1.7).
///
/// Typische Verwendung: Application-Thread setzt `set_trigger_value(true)`,
/// um einen blockierten WaitSet aufzuwecken (z.B. fuer Shutdown-Signaling).
#[derive(Debug, Default)]
pub struct GuardCondition {
    triggered: AtomicBool,
}

impl GuardCondition {
    /// Neuer GuardCondition, initial `trigger_value=false`.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Setzt den Trigger-Wert. Spec §2.2.2.1.7 `set_trigger_value`.
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
/// Ein WaitSet sammelt 0..N `Arc<dyn Condition>` und blockiert in
/// [`Self::wait`] bis mindestens eine triggert oder ein Timeout
/// erreicht wird. Liefert die Liste der **getriggerten** Conditions
/// zurueck.
#[cfg(feature = "std")]
pub struct WaitSet {
    inner: Arc<WaitSetInner>,
}

#[cfg(feature = "std")]
struct WaitSetInner {
    conditions: Mutex<Vec<Arc<dyn Condition>>>,
    /// Notify-Pair fuer poll-based wait. Conditions, die ihren
    /// trigger_value setzen, koennen via `notify()` den WaitSet
    /// aufwecken — im aktuellen Stand pollen wir aber nur, weil
    /// wir die Conditions nicht zwingen wollen, einen Backref zu halten.
    cvar: Condvar,
    /// Idle-Sleep zwischen Poll-Ticks (1ms — kurz genug fuer Latenz,
    /// lang genug fuer geringe CPU-Last).
    poll_interval: core::time::Duration,
    /// Mutex-Guard fuer Condvar.
    locked: Mutex<()>,
    /// `true` solange ein Thread in `wait()` haengt. Spec §2.2.2.1.6:
    /// "WaitSet MAY be used by only one application thread at a time"
    /// — konkurrente `wait()`-Aufrufe MUESSEN `PreconditionNotMet`
    /// liefern.
    waiting: AtomicBool,
}

/// DoS-Cap fuer Conditions in einem einzelnen WaitSet
/// (Spec §2.2.2.1.6: kein expliziter Cap, aber RESOURCE_LIMITS-
/// Pendant). Schuetzt vor unbegrenztem Aufbau bei Pathologien.
pub const MAX_WAITSET_CONDITIONS: usize = 1024;

#[cfg(feature = "std")]
impl WaitSet {
    /// Neuer leerer WaitSet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WaitSetInner {
                conditions: Mutex::new(Vec::new()),
                cvar: Condvar::new(),
                poll_interval: Duration::from_millis(1),
                locked: Mutex::new(()),
                waiting: AtomicBool::new(false),
            }),
        }
    }

    /// Haengt eine Condition an. Spec §2.2.2.1.6 `attach_condition`.
    ///
    /// # Errors
    /// `PreconditionNotMet` wenn der interne Lock vergiftet ist.
    pub fn attach_condition(&self, cond: Arc<dyn Condition>) -> Result<()> {
        let mut conds = self
            .inner
            .conditions
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "waitset conditions poisoned",
            })?;
        // Idempotent: gleiche Arc-Identitaet wird nicht doppelt
        // hinzugefuegt.
        if conds.iter().any(|c| Arc::ptr_eq(c, &cond)) {
            return Ok(());
        }
        // Spec §2.2.2.1.6 / §2.2.1.1.6 — DoS-Cap.
        if conds.len() >= MAX_WAITSET_CONDITIONS {
            return Err(DdsError::OutOfResources {
                what: "waitset condition count",
            });
        }
        conds.push(cond);
        Ok(())
    }

    /// Loest eine Condition. Spec §2.2.2.1.6 `detach_condition`.
    ///
    /// # Errors
    /// `PreconditionNotMet` wenn der interne Lock vergiftet ist.
    pub fn detach_condition(&self, cond: &Arc<dyn Condition>) -> Result<()> {
        let mut conds = self
            .inner
            .conditions
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "waitset conditions poisoned",
            })?;
        conds.retain(|c| !Arc::ptr_eq(c, cond));
        Ok(())
    }

    /// Aktuelle Anzahl der angehaengten Conditions.
    ///
    /// # Errors
    /// `PreconditionNotMet` bei Lock-Poisoning.
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

    /// Blockiert bis mindestens eine Condition triggert oder das
    /// Timeout abgelaufen ist. Liefert die Liste der getriggerten
    /// Conditions. Spec §2.2.2.1.6 `wait`.
    ///
    /// **Implementation:** Polling mit 1ms Intervall. Production-grade
    /// WaitSet wuerde Conditions Notify-Hooks anbieten; das ist Folge-
    /// Optimierung in .
    ///
    /// # Errors
    /// * [`DdsError::Timeout`] wenn keine Condition innerhalb des Timeouts
    ///   triggert.
    /// * [`DdsError::PreconditionNotMet`] bei Lock-Poisoning.
    pub fn wait(&self, timeout: Duration) -> Result<Vec<Arc<dyn Condition>>> {
        // Spec §2.2.2.1.6 — Single-Thread-Wait. Atomic compare_exchange
        // verhindert dass zwei Threads gleichzeitig `wait()` betreten.
        // RAII-Guard via lokales Struct, das beim Drop den Flag
        // zuruecksetzt — robuster gegen Panics in poll_active.
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
        loop {
            let active = self.poll_active()?;
            if !active.is_empty() {
                return Ok(active);
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(DdsError::Timeout);
            }
            // Sleep bis zum naechsten Poll-Tick oder Deadline.
            let sleep_for = (deadline - now).min(self.inner.poll_interval);
            let cvlock = self
                .inner
                .locked
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "waitset locked poisoned",
                })?;
            let _ = self.inner.cvar.wait_timeout(cvlock, sleep_for);
            // (das Result verwerfen ist ok — wir pollen sowieso erneut
            //  am Schleifenanfang.)
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

    /// Weckt einen blockierten `wait()` ohne dass ein
    /// trigger_value-Wechsel passiert ist (z.B. fuer Shutdown).
    ///
    /// Aktuell hat das nur Effekt, wenn der WaitSet
    /// gerade in `wait_timeout` haengt — Caller sollte trotzdem eine
    /// `GuardCondition` anhaengen, damit der wakeup ein definites
    /// Ergebnis hat.
    pub fn notify(&self) {
        self.inner.cvar.notify_all();
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

    #[test]
    fn waitset_wait_timeout_returns_err() {
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        let cond: Arc<dyn Condition> = g.clone();
        ws.attach_condition(cond).unwrap();
        let start = Instant::now();
        let res = ws.wait(Duration::from_millis(50));
        assert!(matches!(res, Err(DdsError::Timeout)));
        // Wir haben mindestens ~50ms gewartet
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
        // Sollte schnell zurueckkehren, nicht das volle Timeout
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
        let state = crate::entity::EntityState::new();
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
        // Beide haben dieselbe Condition (gleicher Inner via Arc).
        assert_eq!(ws2.get_conditions().unwrap().len(), 1);
    }

    #[test]
    fn waitset_default_is_empty() {
        let ws = WaitSet::default();
        assert!(ws.get_conditions().unwrap().is_empty());
    }

    // ---- §2.2.2.1.6 Single-Thread-Wait + Resource-Limit ----

    #[test]
    fn waitset_concurrent_wait_returns_precondition_not_met() {
        // Spec §2.2.2.1.6 — WaitSet darf nur von einem Thread gleichzeitig
        // benutzt werden. Zweiter wait()-Aufruf MUSS PreconditionNotMet
        // liefern.
        let ws = WaitSet::new();
        // Keine Condition attached → wait laeuft ins Timeout.
        let ws_clone = ws.clone();
        let handle = thread::spawn(move || ws_clone.wait(Duration::from_millis(100)));
        // Kurze Pause, damit Thread A den waiting-Flag setzt.
        thread::sleep(Duration::from_millis(20));
        let res = ws.wait(Duration::from_millis(10));
        assert!(matches!(res, Err(DdsError::PreconditionNotMet { .. })));
        let _ = handle.join().unwrap();
    }

    #[test]
    fn waitset_sequential_waits_after_first_returns_succeed() {
        // Nach Abschluss des ersten wait() (per Timeout) muss ein
        // sequentieller zweiter wait() wieder funktionieren.
        let ws = WaitSet::new();
        let r1 = ws.wait(Duration::from_millis(10));
        assert!(matches!(r1, Err(DdsError::Timeout)));
        // Sequentiell: zweiter Aufruf nach erstem Return geht durch.
        let r2 = ws.wait(Duration::from_millis(10));
        assert!(matches!(r2, Err(DdsError::Timeout)));
    }

    #[test]
    fn waitset_attach_above_max_returns_out_of_resources() {
        // §2.2.1.1.6 RC OUT_OF_RESOURCES — DoS-Cap.
        let ws = WaitSet::new();
        for _ in 0..MAX_WAITSET_CONDITIONS {
            let g = GuardCondition::new();
            ws.attach_condition(g).unwrap();
        }
        // Nächste Attach muss OOR liefern.
        let g = GuardCondition::new();
        let res = ws.attach_condition(g);
        assert!(matches!(res, Err(DdsError::OutOfResources { .. })));
        assert_eq!(ws.get_conditions().unwrap().len(), MAX_WAITSET_CONDITIONS);
    }

    #[test]
    fn waitset_attach_idempotent_does_not_count_against_cap() {
        // Selbe Arc-Identitaet zweimal attached → kein Duplikat,
        // kein Counter-Increment.
        let ws = WaitSet::new();
        let g = GuardCondition::new();
        let cond: Arc<dyn Condition> = g.clone();
        ws.attach_condition(cond.clone()).unwrap();
        ws.attach_condition(cond.clone()).unwrap();
        ws.attach_condition(cond).unwrap();
        assert_eq!(ws.get_conditions().unwrap().len(), 1);
    }

    // ---- §2.2.2.5.8 ReadCondition + §2.2.4.5 Trigger State ----

    #[test]
    fn read_condition_returns_trigger_from_closure() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        // Trigger-Closure liefert true wenn alle drei Masks ANY sind.
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
        // Closure liefert immer false → keine Daten verfuegbar.
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
        // §2.2.4.5: ReadCondition als WaitSet-Trigger.
        let ws = WaitSet::new();
        let cond = ReadCondition::new(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            |_, _, _| true, // immer triggered
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
            |_, _, _| false, // keine matchenden Samples
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
        // base() liefert dieselbe Arc-Identitaet.
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
        // Wenn ein Caller mit einem panicking Condition::get_trigger_value
        // den wait() abbricht, muss der waiting-Flag trotzdem freigegeben
        // werden (RAII-Guard). Modelliert durch Drop-Test des
        // Wait-Guards: wir simulieren mit einem normalen Timeout-Return
        // und pruefen, dass danach ein neuer wait moeglich ist (kein
        // hartes Lock).
        let ws = WaitSet::new();
        let _ = ws.wait(Duration::from_millis(5));
        // Sequentieller wait ist erlaubt → waiting-Flag wurde wieder
        // 0 nach Drop des Guards.
        let r = ws.wait(Duration::from_millis(5));
        assert!(matches!(r, Err(DdsError::Timeout)));
    }
}
