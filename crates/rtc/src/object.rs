// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `LightweightRTObject` + State-Machine — Spec §5.2.2.2.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::lifecycle::{ComponentAction, LifeCycleState, is_valid_transition};
use crate::return_code::ReturnCode;

/// `ExecutionContextHandle_t` (Spec §5.2.2.8, S. 30) — opaque Handle
/// fuer die Assoziation eines RTC mit einem Execution-Context.
pub type ExecutionContextHandle = u32;

/// Sentinel-Wert "kein Handle" (analog `INVALID_HANDLE_VALUE`).
pub const INVALID_HANDLE: ExecutionContextHandle = 0;

/// Monoton wachsende Handle-Generation.
fn next_handle() -> ExecutionContextHandle {
    static COUNTER: AtomicU32 = AtomicU32::new(1);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    if n == 0 {
        COUNTER.fetch_add(1, Ordering::SeqCst)
    } else {
        n
    }
}

/// `LightweightRTObject` — Spec §5.2.2.2 (S. 12-19).
///
/// Verwaltet:
/// * Lifecycle-State pro Execution-Context (Spec §5.2.2.3).
/// * Liste aller Contexts in denen das RTC participates.
/// * Owner-Context-Handle (das RTC kann selbst Owner eines Contexts
///   sein — autonomous RTC, Spec §5.2.2.5).
/// * Reference auf den ComponentAction-Callbacks (`Box<dyn>` damit
///   Caller eigene Behavior einbauen kann).
///
/// State-Machine wird zentral hier durchgesetzt — alle Operations
/// pruefen Pre-Conditions und liefern `PRECONDITION_NOT_MET` im
/// Fehlerfall (Spec §5.2.2.2.x).
pub struct LightweightRtObject {
    /// Globaler Lifecycle-State (Created → Alive → Finalized).
    /// Spec §5.2.2.2.3 (`is_alive`): "is alive or not regardless of
    /// the execution context from which it is observed".
    is_alive: bool,
    /// Pro-Context-State (Map handle → state).
    contexts: Vec<ContextEntry>,
    /// User-supplied callbacks.
    callbacks: alloc::boxed::Box<dyn ComponentAction>,
}

/// Per-Context-Status-Eintrag.
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
    /// Konstruiert ein neues, noch nicht initialisiertes RTC im
    /// `Created`-Zustand. Spec §5.2.2.3.1.
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

    /// Spec §5.2.2.2.5 — `attach_context`: registriert das RTC fuer
    /// einen Context. Liefert ein neues Handle.
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

    /// Spec §5.2.2.2.9 — `get_participating_contexts`. Liefert eine
    /// Liste der Handles in denen dieses RTC participates.
    #[must_use]
    pub fn get_participating_contexts(&self) -> Vec<ExecutionContextHandle> {
        self.contexts.iter().map(|c| c.handle).collect()
    }

    /// Spec §5.2.2.6.x via Caller invoked — Ueberprueft ob das RTC im
    /// gegebenen Context im erwarteten State ist.
    #[must_use]
    pub fn get_context_state(&self, handle: ExecutionContextHandle) -> Option<LifeCycleState> {
        self.contexts
            .iter()
            .find(|c| c.handle == handle)
            .map(|c| c.state)
    }

    /// Internal: Inactive → Active im gegebenen Context. Wird von
    /// `ExecutionContext::activate_component` invoked. Spec §5.2.2.6.8.
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
            // Spec §5.2.2.4.7: on_activated-Failure → Active → Error.
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

    /// Forciert Active → Error nach Callback-Fehler in User-Code.
    /// Spec §5.2.2.4.7 — `on_aborting` wird einmalig invoked,
    /// danach uebernimmt `on_error` (siehe Periodic-Tick-Loop).
    pub fn transition_to_error(&mut self, handle: ExecutionContextHandle) {
        if let Some(entry) = self.contexts.iter_mut().find(|c| c.handle == handle) {
            if entry.state == LifeCycleState::Active {
                entry.state = LifeCycleState::Error;
                self.callbacks.on_aborting(handle);
            }
        }
    }

    /// Liefert mutable-Zugriff auf die Callbacks. Wird vom
    /// `ExecutionContext::tick`-Loop benoetigt, um die periodischen
    /// `on_execute`/`on_state_update`/`on_error`-Callbacks zu invoken.
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
        // Spec §5.2.2.2.1 — wenn on_initialize fehlschlaegt, bleibt
        // RTC im Created-State.
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
