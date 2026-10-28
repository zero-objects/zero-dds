// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `LightweightRTObject` + State-Machine — Spec §5.2.2.2.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::lifecycle::{ComponentAction, LifeCycleState, is_valid_transition};
use crate::return_code::ReturnCode;

/// `ExecutionContextHandle_t` (spec §5.2.2.8, p. 30) — opaque handle
/// for the association of an RTC with an execution context.
pub type ExecutionContextHandle = u32;

/// Sentinel value "no handle" (analogous to `INVALID_HANDLE_VALUE`).
pub const INVALID_HANDLE: ExecutionContextHandle = 0;

/// Monotonically increasing handle generation.
fn next_handle() -> ExecutionContextHandle {
    static COUNTER: AtomicU32 = AtomicU32::new(1);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    if n == 0 {
        COUNTER.fetch_add(1, Ordering::SeqCst)
    } else {
        n
    }
}

/// `LightweightRTObject` — spec §5.2.2.2 (p. 12-19).
///
/// Manages:
/// * Lifecycle state per execution context (spec §5.2.2.3).
/// * List of all contexts in which the RTC participates.
/// * Owner-context handle (the RTC can itself be the owner of a context
///   — autonomous RTC, spec §5.2.2.5).
/// * Reference to the ComponentAction callbacks (`Box<dyn>` so the
///   caller can plug in its own behavior).
///
/// The state machine is enforced centrally here — all operations
/// check preconditions and return `PRECONDITION_NOT_MET` on
/// error (spec §5.2.2.2.x).
pub struct LightweightRtObject {
    /// Global lifecycle state (Created → Alive → Finalized).
    /// Spec §5.2.2.2.3 (`is_alive`): "is alive or not regardless of
    /// the execution context from which it is observed".
    is_alive: bool,
    /// Per-context state (map handle → state).
    contexts: Vec<ContextEntry>,
    /// The contexts this RTC owns. Spec §5.2.2.2.8
    /// (`get_owned_contexts`) and §5.2.2.5 (ownership).
    ///
    /// A field and not an external convention: `exit` (§5.2.2.2.4)
    /// begins with "any execution contexts for which the RTC is the
    /// **owner** shall be stopped", so the object has to be able to
    /// answer which those are.
    owned: Vec<ExecutionContextHandle>,
    /// User-supplied callbacks.
    callbacks: alloc::boxed::Box<dyn ComponentAction>,
}

/// What [`LightweightRtObject::exit`] did, and what is left for the
/// caller. Spec §5.2.2.2.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitOutcome {
    /// [`ReturnCode::Ok`], or the first refusal on the way.
    pub code: ReturnCode,
    /// The contexts the RTC owned, which the clause has stopped. They
    /// are returned rather than stopped here because an
    /// `ExecutionContext` is the caller's object in this PSM.
    pub stopped: Vec<ExecutionContextHandle>,
}

/// Per-context status entry.
#[derive(Debug, Clone)]
struct ContextEntry {
    handle: ExecutionContextHandle,
    state: LifeCycleState,
}

/// The execution context as this RTC sees it. Spec §5.2.2.2.7.
///
/// `get_context` returns "a reference to the execution context
/// represented by the given handle", and the local PSM's contexts hold
/// participants by handle while the RTCs themselves are held by the
/// caller — `crates/rtc/src/execution.rs` says so of
/// [`ExecutionContext`]. So the reference this object can hand out is
/// the association it keeps: the handle, the lifecycle state the RTC is
/// in there, and whether this RTC owns that context. A `&ExecutionContext`
/// would have to come from a registry the RTC does not hold, and
/// inventing one here would put a second owner on every context.
///
/// [`ExecutionContext`]: crate::execution::ExecutionContext
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextAssociation {
    /// The handle `attach_context` returned.
    pub handle: ExecutionContextHandle,
    /// The RTC's lifecycle state in that context. Spec §5.2.2.3.
    pub state: LifeCycleState,
    /// Whether this RTC is the context's owner. Spec §5.2.2.5.
    pub owned: bool,
}

impl core::fmt::Debug for LightweightRtObject {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LightweightRtObject")
            .field("is_alive", &self.is_alive)
            .field("contexts", &self.contexts)
            .finish_non_exhaustive()
    }
}

impl LightweightRtObject {
    /// Constructs a new, not-yet-initialized RTC in the
    /// `Created` state. Spec §5.2.2.3.1.
    #[must_use]
    pub fn new(callbacks: alloc::boxed::Box<dyn ComponentAction>) -> Self {
        Self {
            is_alive: false,
            contexts: Vec::new(),
            owned: Vec::new(),
            callbacks,
        }
    }

    /// Spec §5.2.2.2.1 — `initialize`: Created → Alive (Inactive in
    /// each attached Context).
    ///
    /// "An RTC may be initialized only while it is in the Created
    /// state. Any attempt to invoke this operation while in another
    /// state shall fail with PRECONDITION_NOT_MET."
    pub fn initialize(&mut self) -> ReturnCode {
        if self.is_alive {
            return ReturnCode::PreconditionNotMet;
        }
        let cb = self.callbacks.on_initialize();
        if !cb.is_ok() {
            return cb;
        }
        self.is_alive = true;
        ReturnCode::Ok
    }

    /// Spec §5.2.2.2.2 — `finalize`: Alive → Created (no longer
    /// attached to any context).
    ///
    /// "An RTC may not be finalized while it is participating in any
    /// execution context."
    pub fn finalize(&mut self) -> ReturnCode {
        if !self.is_alive {
            // Created → finalize: PRECONDITION_NOT_MET.
            return ReturnCode::PreconditionNotMet;
        }
        if !self.contexts.is_empty() {
            return ReturnCode::PreconditionNotMet;
        }
        let cb = self.callbacks.on_finalize();
        if !cb.is_ok() {
            return cb;
        }
        self.is_alive = false;
        ReturnCode::Ok
    }

    /// Spec §5.2.2.2.3 — `is_alive`. "is alive or not regardless of
    /// the execution context from which it is observed".
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.is_alive
    }

    /// Spec §5.2.2.2.5 — `attach_context`: registers the RTC for
    /// a context. Returns a new handle.
    ///
    /// "This operation is intended to be invoked by
    /// ExecutionContextOperations::add_component. It is not intended
    /// for use by other clients."
    pub fn attach_context(&mut self) -> Result<ExecutionContextHandle, ReturnCode> {
        if !self.is_alive {
            return Err(ReturnCode::PreconditionNotMet);
        }
        let handle = next_handle();
        self.contexts.push(ContextEntry {
            handle,
            state: LifeCycleState::Inactive,
        });
        Ok(handle)
    }

    /// Spec §5.2.2.2.6 — `detach_context`. "may not be invoked if
    /// this RTC is Active in the indicated execution context".
    pub fn detach_context(&mut self, handle: ExecutionContextHandle) -> ReturnCode {
        let Some(idx) = self.contexts.iter().position(|c| c.handle == handle) else {
            return ReturnCode::PreconditionNotMet;
        };
        if self.contexts[idx].state == LifeCycleState::Active {
            return ReturnCode::PreconditionNotMet;
        }
        self.contexts.swap_remove(idx);
        ReturnCode::Ok
    }

    /// Spec §5.2.2.2.4 — `exit`. "Stop the RTC's execution context(s)
    /// and finalize it along with its contents."
    ///
    /// Three sentences, and this object performs the two that are its
    /// own:
    ///
    /// 1. "Any execution contexts for which the RTC is the owner shall
    ///    be stopped." The handles come back in `stopped` — an
    ///    [`ExecutionContext`] is held by the caller in this PSM, so
    ///    stopping the object is the caller's call to
    ///    [`ExecutionContextOperations::stop`] and the list is what
    ///    tells it which.
    /// 2. "If the RTC participates in any execution contexts belonging
    ///    to another RTC … it shall be deactivated in those contexts."
    ///    Done here, through the same path
    ///    [`ExecutionContextOperations::deactivate_component`] takes,
    ///    so `on_deactivated` fires exactly as it would there.
    /// 3. "After the RTC is no longer Active in any Running execution
    ///    context, it … shall be finalized." Done here: every
    ///    association is dropped and `on_finalize` fires.
    ///
    /// Constraint: "An RTC cannot be exited if it has not yet been
    /// initialized." A Created RTC answers
    /// [`ReturnCode::PreconditionNotMet`] and nothing else happens.
    ///
    /// [`ExecutionContext`]: crate::execution::ExecutionContext
    /// [`ExecutionContextOperations::stop`]: crate::execution::ExecutionContextOperations::stop
    /// [`ExecutionContextOperations::deactivate_component`]: crate::execution::ExecutionContextOperations::deactivate_component
    pub fn exit(&mut self) -> ExitOutcome {
        if !self.is_alive {
            return ExitOutcome {
                code: ReturnCode::PreconditionNotMet,
                stopped: Vec::new(),
            };
        }
        let stopped = self.owned.clone();
        // Sentence two, before sentence three: `finalize` refuses while
        // the RTC still participates anywhere, which is the same
        // constraint §5.2.2.2.2 states.
        let active: Vec<ExecutionContextHandle> = self
            .contexts
            .iter()
            .filter(|entry| entry.state == LifeCycleState::Active)
            .map(|entry| entry.handle)
            .collect();
        for handle in active {
            let code = self.deactivate(handle);
            if !code.is_ok() {
                return ExitOutcome { code, stopped };
            }
        }
        self.contexts.clear();
        self.owned.clear();
        let code = self.finalize();
        ExitOutcome { code, stopped }
    }

    /// Spec §5.2.2.2.7 — `get_context`. "Obtain a reference to the
    /// execution context represented by the given handle."
    ///
    /// `None` for a handle this RTC did not hand out: the clause says
    /// "the given handle must have been obtained by a previous call to
    /// `attach_context` on this RTC", and a handle from another RTC
    /// names an association this one does not have.
    #[must_use]
    pub fn get_context(&self, handle: ExecutionContextHandle) -> Option<ContextAssociation> {
        self.contexts
            .iter()
            .find(|entry| entry.handle == handle)
            .map(|entry| ContextAssociation {
                handle: entry.handle,
                state: entry.state,
                owned: self.owned.contains(&entry.handle),
            })
    }

    /// Spec §5.2.2.2.10 — `get_context_handle`. "The handle returned by
    /// this operation is same as the handle returned by
    /// `attach_context`."
    ///
    /// The local PSM identifies a context by that handle — an
    /// [`ExecutionContext`] holds its participants as handles and the
    /// RTCs are the caller's — so this is the identity on an attached
    /// handle and [`INVALID_HANDLE`] on anything else. It is here
    /// because the clause lists it and because the second half is a
    /// real answer: a handle this RTC never got back from
    /// `attach_context` names no context of its.
    ///
    /// [`ExecutionContext`]: crate::execution::ExecutionContext
    #[must_use]
    pub fn get_context_handle(&self, context: ExecutionContextHandle) -> ExecutionContextHandle {
        if self.contexts.iter().any(|entry| entry.handle == context) {
            context
        } else {
            INVALID_HANDLE
        }
    }

    /// Spec §5.2.2.2.8 — `get_owned_contexts`. "A list of all execution
    /// contexts owned by this RTC."
    #[must_use]
    pub fn get_owned_contexts(&self) -> Vec<ExecutionContextHandle> {
        self.owned.clone()
    }

    /// Records that this RTC owns the context behind `handle`. Spec
    /// §5.2.2.5 — an RTC that has an execution context of its own is
    /// its owner, and §5.2.2.2.4's first sentence turns on that fact.
    ///
    /// [`ReturnCode::PreconditionNotMet`] for a handle this RTC does
    /// not participate in: owning a context it is not attached to is a
    /// state the clause has no reading for. [`ReturnCode::Ok`] twice
    /// for the same handle — ownership is a property and not a count.
    pub fn own_context(&mut self, handle: ExecutionContextHandle) -> ReturnCode {
        if !self.contexts.iter().any(|entry| entry.handle == handle) {
            return ReturnCode::PreconditionNotMet;
        }
        if !self.owned.contains(&handle) {
            self.owned.push(handle);
        }
        ReturnCode::Ok
    }

    /// Drops this RTC's ownership of a context. The inverse of
    /// [`Self::own_context`]; [`ReturnCode::PreconditionNotMet`] where
    /// the RTC does not own it.
    pub fn disown_context(&mut self, handle: ExecutionContextHandle) -> ReturnCode {
        let Some(index) = self.owned.iter().position(|owned| *owned == handle) else {
            return ReturnCode::PreconditionNotMet;
        };
        self.owned.swap_remove(index);
        ReturnCode::Ok
    }

    /// Spec §5.2.2.2.9 — `get_participating_contexts`. Returns a
    /// list of the handles this RTC participates in.
    #[must_use]
    pub fn get_participating_contexts(&self) -> Vec<ExecutionContextHandle> {
        self.contexts.iter().map(|c| c.handle).collect()
    }

    /// Spec §5.2.2.6.x invoked via the caller — checks whether the RTC is in
    /// the expected state in the given context.
    #[must_use]
    pub fn get_context_state(&self, handle: ExecutionContextHandle) -> Option<LifeCycleState> {
        self.contexts
            .iter()
            .find(|c| c.handle == handle)
            .map(|c| c.state)
    }

    /// Internal: Inactive → Active in the given context. Invoked by
    /// `ExecutionContext::activate_component`. Spec §5.2.2.6.8.
    pub(crate) fn activate(&mut self, handle: ExecutionContextHandle) -> ReturnCode {
        let Some(entry) = self.contexts.iter_mut().find(|c| c.handle == handle) else {
            return ReturnCode::BadParameter;
        };
        if !is_valid_transition(entry.state, LifeCycleState::Active) {
            return ReturnCode::PreconditionNotMet;
        }
        entry.state = LifeCycleState::Active;
        let cb = self.callbacks.on_activated(handle);
        if !cb.is_ok() {
            // Spec §5.2.2.4.7: on_activated failure → Active → Error.
            entry.state = LifeCycleState::Error;
            self.callbacks.on_aborting(handle);
            return cb;
        }
        ReturnCode::Ok
    }

    /// Internal: Active → Inactive. Spec §5.2.2.6.9.
    pub(crate) fn deactivate(&mut self, handle: ExecutionContextHandle) -> ReturnCode {
        let Some(entry) = self.contexts.iter_mut().find(|c| c.handle == handle) else {
            return ReturnCode::BadParameter;
        };
        if !is_valid_transition(entry.state, LifeCycleState::Inactive) {
            return ReturnCode::PreconditionNotMet;
        }
        entry.state = LifeCycleState::Inactive;
        self.callbacks.on_deactivated(handle)
    }

    /// Internal: Error → Inactive via `reset_component`. Spec
    /// §5.2.2.6.10.
    pub(crate) fn reset(&mut self, handle: ExecutionContextHandle) -> ReturnCode {
        let Some(entry) = self.contexts.iter_mut().find(|c| c.handle == handle) else {
            return ReturnCode::BadParameter;
        };
        if entry.state != LifeCycleState::Error {
            return ReturnCode::PreconditionNotMet;
        }
        let cb = self.callbacks.on_reset(handle);
        if cb.is_ok() {
            entry.state = LifeCycleState::Inactive;
        }
        cb
    }

    /// `LightweightRTObject::reset()` (Annex A IDL:
    /// `ReturnCode_t reset();`).
    ///
    /// The no-argument form of the operation. Annex A gives the RTC itself a
    /// `reset()` with no context argument, while §5.2.2.6.10 gives the
    /// `ExecutionContext` a per-component `reset_component`; the per-context
    /// path is `Self::reset` and is driven from there. This one is what a
    /// caller holding only the RTC can invoke, and it applies the same
    /// transition to every context the object is in.
    ///
    /// Returns `Ok` when every erred context returned to `Inactive`. A
    /// context that is not in `Error` is not a failure — it has nothing to
    /// reset — so an object with no erred context returns `Ok`. If a user
    /// `on_reset` callback fails for some context, that context stays in
    /// `Error` and its code is returned; the remaining contexts are still
    /// attempted, so one failing component cannot leave the others stuck.
    pub fn reset_all(&mut self) -> ReturnCode {
        let erred: Vec<ExecutionContextHandle> = self
            .contexts
            .iter()
            .filter(|c| c.state == LifeCycleState::Error)
            .map(|c| c.handle)
            .collect();
        let mut first_failure = ReturnCode::Ok;
        for handle in erred {
            let rc = self.reset(handle);
            if !rc.is_ok() && first_failure.is_ok() {
                first_failure = rc;
            }
        }
        first_failure
    }

    /// Forces Active → Error after a callback error in user code.
    /// Spec §5.2.2.4.7 — `on_aborting` is invoked once,
    /// after which `on_error` takes over (see the periodic-tick loop).
    pub fn transition_to_error(&mut self, handle: ExecutionContextHandle) {
        if let Some(entry) = self.contexts.iter_mut().find(|c| c.handle == handle) {
            if entry.state == LifeCycleState::Active {
                entry.state = LifeCycleState::Error;
                self.callbacks.on_aborting(handle);
            }
        }
    }

    /// Returns mutable access to the callbacks. Needed by the
    /// `ExecutionContext::tick` loop to invoke the periodic
    /// `on_execute`/`on_state_update`/`on_error` callbacks.
    pub fn callbacks_mut(&mut self) -> &mut dyn ComponentAction {
        self.callbacks.as_mut()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    struct CountingCallbacks {
        initialize: u32,
        finalize: u32,
        activated: u32,
        deactivated: u32,
        reset: u32,
        force_init_fail: bool,
    }

    impl ComponentAction for CountingCallbacks {
        fn on_initialize(&mut self) -> ReturnCode {
            self.initialize += 1;
            if self.force_init_fail {
                ReturnCode::Error
            } else {
                ReturnCode::Ok
            }
        }
        fn on_finalize(&mut self) -> ReturnCode {
            self.finalize += 1;
            ReturnCode::Ok
        }
        fn on_activated(&mut self, _h: u32) -> ReturnCode {
            self.activated += 1;
            ReturnCode::Ok
        }
        fn on_deactivated(&mut self, _h: u32) -> ReturnCode {
            self.deactivated += 1;
            ReturnCode::Ok
        }
        fn on_reset(&mut self, _h: u32) -> ReturnCode {
            self.reset += 1;
            ReturnCode::Ok
        }
    }

    fn make() -> LightweightRtObject {
        LightweightRtObject::new(alloc::boxed::Box::new(CountingCallbacks {
            initialize: 0,
            finalize: 0,
            activated: 0,
            deactivated: 0,
            reset: 0,
            force_init_fail: false,
        }))
    }

    #[test]
    fn exit_in_the_created_state_is_refused() {
        // Spec §5.2.2.2.4: "An RTC cannot be exited if it has not yet
        // been initialized."
        let mut r = make();
        let outcome = r.exit();
        assert_eq!(outcome.code, ReturnCode::PreconditionNotMet);
        assert!(outcome.stopped.is_empty());
        assert!(!r.is_alive());
    }

    #[test]
    fn exit_deactivates_detaches_and_finalizes() {
        // Spec §5.2.2.2.4, its three sentences in order.
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        let a = r.attach_context().expect("attach");
        let b = r.attach_context().expect("attach");
        assert_eq!(r.activate(a), ReturnCode::Ok);
        assert_eq!(r.get_context_state(a), Some(LifeCycleState::Active));

        let outcome = r.exit();
        assert_eq!(outcome.code, ReturnCode::Ok);
        assert!(r.get_participating_contexts().is_empty());
        assert!(r.get_context(a).is_none());
        assert!(r.get_context(b).is_none());
        assert!(!r.is_alive());
    }

    #[test]
    fn exit_reports_the_contexts_it_owned_for_the_caller_to_stop() {
        // Spec §5.2.2.2.4: "Any execution contexts for which the RTC is
        // the owner shall be stopped." An `ExecutionContext` is the
        // caller's object here, so the handles come back.
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        let owned = r.attach_context().expect("attach");
        let joined = r.attach_context().expect("attach");
        assert_eq!(r.own_context(owned), ReturnCode::Ok);

        let outcome = r.exit();
        assert_eq!(outcome.code, ReturnCode::Ok);
        assert_eq!(outcome.stopped, alloc::vec![owned]);
        assert!(!outcome.stopped.contains(&joined));
        assert!(r.get_owned_contexts().is_empty());
    }

    #[test]
    fn ownership_is_a_property_and_not_a_count() {
        // Spec §5.2.2.5 — an RTC owns a context or it does not.
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        let h = r.attach_context().expect("attach");
        assert_eq!(r.own_context(h), ReturnCode::Ok);
        assert_eq!(r.own_context(h), ReturnCode::Ok);
        assert_eq!(r.get_owned_contexts(), alloc::vec![h]);
        assert_eq!(r.disown_context(h), ReturnCode::Ok);
        assert!(r.get_owned_contexts().is_empty());
        assert_eq!(r.disown_context(h), ReturnCode::PreconditionNotMet);
    }

    #[test]
    fn a_context_the_rtc_is_not_attached_to_cannot_be_owned() {
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        assert_eq!(r.own_context(4242), ReturnCode::PreconditionNotMet);
    }

    #[test]
    fn get_context_answers_only_for_a_handle_this_rtc_handed_out() {
        // Spec §5.2.2.2.7: "The given handle must have been obtained by
        // a previous call to attach_context on this RTC."
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        let h = r.attach_context().expect("attach");
        assert_eq!(r.own_context(h), ReturnCode::Ok);
        let association = r.get_context(h).expect("an association");
        assert_eq!(association.handle, h);
        assert_eq!(association.state, LifeCycleState::Inactive);
        assert!(association.owned);
        assert!(r.get_context(h.wrapping_add(1000)).is_none());
    }

    #[test]
    fn get_context_handle_is_the_attach_handle_and_invalid_otherwise() {
        // Spec §5.2.2.2.10: "The handle returned by this operation is
        // same as the handle returned by attach_context."
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        let h = r.attach_context().expect("attach");
        assert_eq!(r.get_context_handle(h), h);
        assert_eq!(r.get_context_handle(h.wrapping_add(1000)), INVALID_HANDLE);
    }

    #[test]
    fn fresh_rtc_is_not_alive() {
        // Spec §5.2.2.3.1.
        let r = make();
        assert!(!r.is_alive());
    }

    #[test]
    fn initialize_then_finalize_round_trips_alive_flag() {
        // Spec §5.2.2.2.1 + §5.2.2.2.2.
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        assert!(r.is_alive());
        assert_eq!(r.finalize(), ReturnCode::Ok);
        assert!(!r.is_alive());
    }

    #[test]
    fn double_initialize_yields_precondition_not_met() {
        // Spec §5.2.2.2.1 — initialize only valid in Created state.
        let mut r = make();
        assert_eq!(r.initialize(), ReturnCode::Ok);
        assert_eq!(r.initialize(), ReturnCode::PreconditionNotMet);
    }

    #[test]
    fn finalize_in_created_state_yields_precondition_not_met() {
        // Spec §5.2.2.2.2.
        let mut r = make();
        assert_eq!(r.finalize(), ReturnCode::PreconditionNotMet);
    }

    #[test]
    fn finalize_with_attached_context_yields_precondition_not_met() {
        // Spec §5.2.2.2.2 — must remove with detach first.
        let mut r = make();
        r.initialize();
        let _ = r.attach_context().expect("attach ok");
        assert_eq!(r.finalize(), ReturnCode::PreconditionNotMet);
    }

    #[test]
    fn attach_context_in_created_state_fails() {
        // Spec §5.2.2.2.5 + §5.2.2.5 — implicit pre-condition is_alive.
        let mut r = make();
        assert!(matches!(
            r.attach_context(),
            Err(ReturnCode::PreconditionNotMet)
        ));
    }

    #[test]
    fn attach_then_detach_works() {
        let mut r = make();
        r.initialize();
        let h = r.attach_context().expect("attach");
        assert_eq!(r.get_participating_contexts(), alloc::vec![h]);
        assert_eq!(r.detach_context(h), ReturnCode::Ok);
        assert!(r.get_participating_contexts().is_empty());
    }

    #[test]
    fn detach_unknown_handle_yields_precondition_not_met() {
        // Spec §5.2.2.2.6.
        let mut r = make();
        r.initialize();
        assert_eq!(r.detach_context(99_999), ReturnCode::PreconditionNotMet);
    }

    #[test]
    fn detach_active_rtc_yields_precondition_not_met() {
        // Spec §5.2.2.2.6: "may not be invoked if this RTC is Active".
        let mut r = make();
        r.initialize();
        let h = r.attach_context().expect("attach");
        assert_eq!(r.activate(h), ReturnCode::Ok);
        assert_eq!(r.detach_context(h), ReturnCode::PreconditionNotMet);
    }

    #[test]
    fn activate_inactive_rtc_invokes_on_activated() {
        let mut r = make();
        r.initialize();
        let h = r.attach_context().expect("attach");
        assert_eq!(r.activate(h), ReturnCode::Ok);
        assert_eq!(r.get_context_state(h), Some(LifeCycleState::Active));
    }

    #[test]
    fn deactivate_active_rtc_invokes_on_deactivated() {
        let mut r = make();
        r.initialize();
        let h = r.attach_context().expect("attach");
        r.activate(h);
        assert_eq!(r.deactivate(h), ReturnCode::Ok);
        assert_eq!(r.get_context_state(h), Some(LifeCycleState::Inactive));
    }

    #[test]
    fn reset_only_works_from_error_state() {
        // Spec §5.2.2.6.10.
        let mut r = make();
        r.initialize();
        let h = r.attach_context().expect("attach");
        // Inactive → reset = PRECONDITION_NOT_MET.
        assert_eq!(r.reset(h), ReturnCode::PreconditionNotMet);
        // Activate then force into Error.
        r.activate(h);
        r.transition_to_error(h);
        assert_eq!(r.get_context_state(h), Some(LifeCycleState::Error));
        // Now reset works.
        assert_eq!(r.reset(h), ReturnCode::Ok);
        assert_eq!(r.get_context_state(h), Some(LifeCycleState::Inactive));
    }

    #[test]
    fn initialize_failure_keeps_rtc_in_created_state() {
        // Spec §5.2.2.2.1 — if on_initialize fails, the
        // RTC stays in the Created state.
        let mut r = LightweightRtObject::new(alloc::boxed::Box::new(CountingCallbacks {
            initialize: 0,
            finalize: 0,
            activated: 0,
            deactivated: 0,
            reset: 0,
            force_init_fail: true,
        }));
        assert_eq!(r.initialize(), ReturnCode::Error);
        assert!(!r.is_alive());
    }

    #[test]
    fn handles_are_unique_across_attaches() {
        let mut r = make();
        r.initialize();
        let h1 = r.attach_context().expect("attach1");
        let h2 = r.attach_context().expect("attach2");
        assert_ne!(h1, h2);
        assert_ne!(h1, INVALID_HANDLE);
        assert_ne!(h2, INVALID_HANDLE);
    }
}
