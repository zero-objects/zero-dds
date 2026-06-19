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
    /// User-supplied callbacks.
    callbacks: alloc::boxed::Box<dyn ComponentAction>,
}

/// Per-context status entry.
#[derive(Debug, Clone)]
struct ContextEntry {
    handle: ExecutionContextHandle,
    state: LifeCycleState,
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
            callbacks,
        }
    }

    /// Spec §5.2.2.2.1 — `initialize`: Created → Alive (Inactive in
    /// jedem attached Context).
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
