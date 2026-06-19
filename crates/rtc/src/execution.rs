// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ExecutionContext` + `ExecutionContextOperations` — Spec §5.2.2.5
//! / §5.2.2.6.

use alloc::vec::Vec;

use crate::lifecycle::{ExecutionKind, LifeCycleState};
use crate::object::{ExecutionContextHandle, LightweightRtObject};
use crate::return_code::ReturnCode;

/// `ExecutionContext` Trait — Spec §5.2.2.5 (S. 22-23).
///
/// "An ExecutionContext allows the business logic of an RTC to be
/// decoupled from the thread of control in which it is executed."
///
/// This trait defines the external view; concrete realizations
/// inherit from it. The body of the trait is the `ExecutionContextOperations`
/// interface from spec §5.2.2.6.
pub trait ExecutionContextOperations {
    /// Spec §5.2.2.6.1 — `is_running`.
    fn is_running(&self) -> bool;
    /// Spec §5.2.2.6.2 — `start`. Stopped → Running.
    fn start(&mut self) -> ReturnCode;
    /// Spec §5.2.2.6.3 — `stop`. Running → Stopped.
    fn stop(&mut self) -> ReturnCode;
    /// Spec §5.2.2.6.4 — `get_rate`. Tick-Rate in Hz.
    fn get_rate(&self) -> f64;
    /// Spec §5.2.2.6.5 — `set_rate(rate)`.
    ///
    /// # Errors
    /// `BAD_PARAMETER` if `rate <= 0.0` or `NaN`.
    fn set_rate(&mut self, rate: f64) -> ReturnCode;
    /// Spec §5.2.2.6.6 — `add_component`.
    ///
    /// # Errors
    /// Returns `ReturnCode::Err` if the RTC is not alive or
    /// is already registered in the context.
    fn add_component(
        &mut self,
        component: &mut LightweightRtObject,
    ) -> Result<ExecutionContextHandle, ReturnCode>;
    /// Spec §5.2.2.6.7 — `remove_component`.
    fn remove_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode;
    /// Spec §5.2.2.6.8 — `activate_component`.
    fn activate_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode;
    /// Spec §5.2.2.6.9 — `deactivate_component`.
    fn deactivate_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode;
    /// Spec §5.2.2.6.10 — `reset_component`.
    fn reset_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode;
    /// Spec §5.2.2.6.11 — `get_component_state`.
    fn get_component_state(
        &self,
        component: &LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> LifeCycleState;
    /// Spec §5.2.2.6.12 — `get_kind`.
    fn get_kind(&self) -> ExecutionKind;
}

/// Konkrete `ExecutionContext`-Implementation — Spec §5.2.2.5
/// (local-PSM variant).
///
/// Manages:
/// * Running/Stopped-State (Spec Fig 5.7).
/// * Tick-Rate (Hz, Spec §5.2.2.6.4-§5.2.2.6.5).
/// * List of participating RTCs (identified by handle —
///   the RTCs themselves are held by the caller).
/// * `ExecutionKind` (Spec §5.2.2.7).
pub struct ExecutionContext {
    running: bool,
    rate_hz: f64,
    kind: ExecutionKind,
    participants: Vec<ExecutionContextHandle>,
}

impl ExecutionContext {
    /// Constructs a new, stopped `ExecutionContext` with the
    /// given `ExecutionKind` and a default rate of 1.0 Hz.
    #[must_use]
    pub const fn new(kind: ExecutionKind) -> Self {
        Self {
            running: false,
            rate_hz: 1.0,
            kind,
            participants: Vec::new(),
        }
    }

    /// Constructs with an explicit rate.
    ///
    /// # Errors
    /// `Err(BAD_PARAMETER)` if `rate <= 0.0` or `NaN`.
    pub fn with_rate(kind: ExecutionKind, rate_hz: f64) -> Result<Self, ReturnCode> {
        if !rate_hz.is_finite() || rate_hz <= 0.0 {
            return Err(ReturnCode::BadParameter);
        }
        Ok(Self {
            running: false,
            rate_hz,
            kind,
            participants: Vec::new(),
        })
    }
}

impl core::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("running", &self.running)
            .field("rate_hz", &self.rate_hz)
            .field("kind", &self.kind)
            .field("participants", &self.participants)
            .finish()
    }
}

impl ExecutionContextOperations for ExecutionContext {
    fn is_running(&self) -> bool {
        self.running
    }

    fn start(&mut self) -> ReturnCode {
        if self.running {
            // Spec §5.2.2.6.2 — already running is not an explicit
            // error, but we return `Ok` (idempotent).
            return ReturnCode::Ok;
        }
        self.running = true;
        ReturnCode::Ok
    }

    fn stop(&mut self) -> ReturnCode {
        if !self.running {
            return ReturnCode::Ok;
        }
        self.running = false;
        ReturnCode::Ok
    }

    fn get_rate(&self) -> f64 {
        self.rate_hz
    }

    fn set_rate(&mut self, rate: f64) -> ReturnCode {
        if !rate.is_finite() || rate <= 0.0 {
            return ReturnCode::BadParameter;
        }
        self.rate_hz = rate;
        ReturnCode::Ok
    }

    fn add_component(
        &mut self,
        component: &mut LightweightRtObject,
    ) -> Result<ExecutionContextHandle, ReturnCode> {
        let handle = component.attach_context()?;
        self.participants.push(handle);
        Ok(handle)
    }

    fn remove_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode {
        let Some(idx) = self.participants.iter().position(|h| *h == handle) else {
            return ReturnCode::BadParameter;
        };
        let rc = component.detach_context(handle);
        if rc.is_ok() {
            self.participants.swap_remove(idx);
        }
        rc
    }

    fn activate_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode {
        if !self.participants.contains(&handle) {
            return ReturnCode::BadParameter;
        }
        component.activate(handle)
    }

    fn deactivate_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode {
        if !self.participants.contains(&handle) {
            return ReturnCode::BadParameter;
        }
        component.deactivate(handle)
    }

    fn reset_component(
        &mut self,
        component: &mut LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> ReturnCode {
        if !self.participants.contains(&handle) {
            return ReturnCode::BadParameter;
        }
        component.reset(handle)
    }

    fn get_component_state(
        &self,
        component: &LightweightRtObject,
        handle: ExecutionContextHandle,
    ) -> LifeCycleState {
        // Spec §5.2.2.6.11 — if the handle is unknown, semantically
        // unclear; we return Created as the default.
        component
            .get_context_state(handle)
            .unwrap_or(LifeCycleState::Created)
    }

    fn get_kind(&self) -> ExecutionKind {
        self.kind
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::lifecycle::ComponentAction;

    struct NoOp;
    impl ComponentAction for NoOp {}

    fn rtc() -> LightweightRtObject {
        LightweightRtObject::new(alloc::boxed::Box::new(NoOp))
    }

    #[test]
    fn fresh_context_is_stopped_with_default_rate() {
        // Spec §5.2.2.5 Fig 5.7 — Starts Stopped.
        let ec = ExecutionContext::new(ExecutionKind::Periodic);
        assert!(!ec.is_running());
        assert!((ec.get_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(ec.get_kind(), ExecutionKind::Periodic);
    }

    #[test]
    fn start_stop_round_trips_running_flag() {
        // Spec §5.2.2.6.2 + §5.2.2.6.3.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        assert_eq!(ec.start(), ReturnCode::Ok);
        assert!(ec.is_running());
        assert_eq!(ec.stop(), ReturnCode::Ok);
        assert!(!ec.is_running());
    }

    #[test]
    fn double_start_is_idempotent() {
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        ec.start();
        assert_eq!(ec.start(), ReturnCode::Ok);
    }

    #[test]
    fn set_rate_rejects_non_positive_or_nan() {
        // Spec §5.2.2.6.5.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        assert_eq!(ec.set_rate(0.0), ReturnCode::BadParameter);
        assert_eq!(ec.set_rate(-1.0), ReturnCode::BadParameter);
        assert_eq!(ec.set_rate(f64::NAN), ReturnCode::BadParameter);
        assert_eq!(ec.set_rate(f64::INFINITY), ReturnCode::BadParameter);
    }

    #[test]
    fn set_rate_accepts_positive_finite() {
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        assert_eq!(ec.set_rate(50.0), ReturnCode::Ok);
        assert!((ec.get_rate() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn with_rate_validates_rate_argument() {
        assert!(ExecutionContext::with_rate(ExecutionKind::Periodic, 100.0).is_ok());
        assert!(ExecutionContext::with_rate(ExecutionKind::Periodic, 0.0).is_err());
        assert!(ExecutionContext::with_rate(ExecutionKind::Periodic, -1.0).is_err());
    }

    #[test]
    fn add_component_attaches_and_returns_handle() {
        // Spec §5.2.2.6.6 — invokes attach_context on RTC.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        let mut r = rtc();
        r.initialize();
        let h = ec.add_component(&mut r).expect("attach");
        assert_eq!(ec.get_component_state(&r, h), LifeCycleState::Inactive);
    }

    #[test]
    fn add_uninitialized_component_fails() {
        // Spec §5.2.2.6.6 + §5.2.2.2.5.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        let mut r = rtc();
        // Not initialized → attach fails.
        assert!(ec.add_component(&mut r).is_err());
    }

    #[test]
    fn activate_then_deactivate_round_trips_state() {
        // Spec §5.2.2.6.8 + §5.2.2.6.9.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        let mut r = rtc();
        r.initialize();
        let h = ec.add_component(&mut r).expect("attach");
        assert_eq!(ec.activate_component(&mut r, h), ReturnCode::Ok);
        assert_eq!(ec.get_component_state(&r, h), LifeCycleState::Active);
        assert_eq!(ec.deactivate_component(&mut r, h), ReturnCode::Ok);
        assert_eq!(ec.get_component_state(&r, h), LifeCycleState::Inactive);
    }

    #[test]
    fn activate_with_unknown_handle_yields_bad_parameter() {
        // Spec §5.2.2.6.8 — the handle must belong to this context.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        let mut r = rtc();
        r.initialize();
        assert_eq!(
            ec.activate_component(&mut r, 99_999),
            ReturnCode::BadParameter
        );
    }

    #[test]
    fn remove_component_detaches_handle() {
        // Spec §5.2.2.6.7.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        let mut r = rtc();
        r.initialize();
        let h = ec.add_component(&mut r).expect("attach");
        assert_eq!(ec.remove_component(&mut r, h), ReturnCode::Ok);
        assert!(r.get_participating_contexts().is_empty());
    }

    #[test]
    fn cannot_remove_active_component() {
        // Spec §5.2.2.2.6 — detach when Active is forbidden.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        let mut r = rtc();
        r.initialize();
        let h = ec.add_component(&mut r).expect("attach");
        ec.activate_component(&mut r, h);
        assert_eq!(
            ec.remove_component(&mut r, h),
            ReturnCode::PreconditionNotMet
        );
    }
}
