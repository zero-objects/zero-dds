// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lifecycle model — spec §5.2.2.3 / §5.2.2.4 / §5.2.2.7.

use crate::return_code::ReturnCode;

/// `LifeCycleState` (spec §5.2.2.3, p. 19) — lifecycle states of an
/// RTC within an `ExecutionContext`.
///
/// Spec-Zitat (§5.2.2.3.1-§5.2.2.3.4): "CREATED — instantiated but not
/// yet initialized; INACTIVE — Alive but not invoked; ACTIVE — Alive
/// and will be invoked when context is Running; ERROR — encountered
/// problem and cannot continue without reset."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifeCycleState {
    /// `CREATED` (§5.2.2.3.1).
    Created,
    /// `INACTIVE` (§5.2.2.3.2).
    Inactive,
    /// `ACTIVE` (§5.2.2.3.3).
    Active,
    /// `ERROR` (§5.2.2.3.4).
    Error,
}

/// `ExecutionKind` (spec §5.2.2.7, p. 30-31) — defines when the
/// execution context invokes the component callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionKind {
    /// `PERIODIC` (§5.2.2.7.1) — `on_execute` + `on_state_update` on
    /// every tick.
    Periodic,
    /// `EVENT_DRIVEN` (§5.2.2.7.2) — `on_action` on a discrete event.
    EventDriven,
    /// `OTHER` (§5.2.2.7.3) — implementor-specific.
    Other,
}

/// `ComponentAction` Trait — Spec §5.2.2.4 (S. 20-21).
///
/// Spec-Zitat: "ComponentAction interface provides callbacks
/// corresponding to the execution of the lifecycle operations of
/// LightweightRTObject and ExecutionContext."
///
/// The default implementations return `ReturnCode::Ok` — users
/// implement only the callbacks relevant to their domain
/// (spec §5.2.2.4: "An RTC developer may implement these callback
/// operations in order to execute application-specific logic").
pub trait ComponentAction {
    /// Spec §5.2.2.4.1 — RTC has been initialized and entered Alive
    /// state.
    fn on_initialize(&mut self) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.2 — RTC is being destroyed.
    fn on_finalize(&mut self) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.3 — Execution-Context transitioned Stopped →
    /// Running.
    fn on_startup(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.4 — Execution-Context transitioned Running →
    /// Stopped.
    fn on_shutdown(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.5 — RTC has been activated.
    fn on_activated(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.6 — RTC has been deactivated.
    fn on_deactivated(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.7 — RTC transitions Active → Error (single
    /// invocation).
    fn on_aborting(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.8 — RTC remains in Error state (repeated per
    /// tick when `ExecutionKind::Periodic`).
    fn on_error(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.2.2.4.9 — reset attempt from the error state back to
    /// inactive.
    fn on_reset(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
}

/// Spec §5.2.2.3 (Figure 5.4 + 5.5) — valid state transitions in the
/// lifecycle. Returns `true` if the transition is allowed per spec,
/// otherwise `false`.
///
/// Transitions:
/// * `Created → Inactive` via `initialize`/`attach_context`.
/// * `Inactive → Active` via `activate_component`.
/// * `Active → Inactive` via `deactivate_component`.
/// * `Active → Error` automatically on `on_*` return != Ok.
/// * `Error → Inactive` via `reset_component` (when `on_reset`
///   returns `ReturnCode::Ok`).
/// * No transition out of `Created` except via `initialize`.
#[must_use]
pub const fn is_valid_transition(from: LifeCycleState, to: LifeCycleState) -> bool {
    use LifeCycleState::{Active, Created, Error, Inactive};
    matches!(
        (from, to),
        (Created, Inactive)
            | (Inactive, Active)
            | (Active, Inactive)
            | (Active, Error)
            | (Error, Inactive)
            | (Error, Error)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions_match_spec_state_machine() {
        // Spec §5.2.2.3 Fig 5.4 + §5.2.2.5 Fig 5.5.
        for (from, to, expected) in [
            (LifeCycleState::Created, LifeCycleState::Inactive, true),
            (LifeCycleState::Inactive, LifeCycleState::Active, true),
            (LifeCycleState::Active, LifeCycleState::Inactive, true),
            (LifeCycleState::Active, LifeCycleState::Error, true),
            (LifeCycleState::Error, LifeCycleState::Inactive, true),
            // Forbidden:
            (LifeCycleState::Created, LifeCycleState::Active, false),
            (LifeCycleState::Created, LifeCycleState::Error, false),
            (LifeCycleState::Inactive, LifeCycleState::Created, false),
            (LifeCycleState::Inactive, LifeCycleState::Error, false),
            (LifeCycleState::Active, LifeCycleState::Created, false),
            (LifeCycleState::Error, LifeCycleState::Active, false),
            (LifeCycleState::Error, LifeCycleState::Created, false),
        ] {
            assert_eq!(
                is_valid_transition(from, to),
                expected,
                "{from:?} -> {to:?}"
            );
        }
    }

    #[test]
    fn default_component_action_returns_ok_for_all_callbacks() {
        // Spec §5.2.2.4 — Default callbacks are no-ops returning OK.
        struct Stub;
        impl ComponentAction for Stub {}
        let mut s = Stub;
        assert_eq!(s.on_initialize(), ReturnCode::Ok);
        assert_eq!(s.on_finalize(), ReturnCode::Ok);
        assert_eq!(s.on_startup(0), ReturnCode::Ok);
        assert_eq!(s.on_shutdown(0), ReturnCode::Ok);
        assert_eq!(s.on_activated(0), ReturnCode::Ok);
        assert_eq!(s.on_deactivated(0), ReturnCode::Ok);
        assert_eq!(s.on_aborting(0), ReturnCode::Ok);
        assert_eq!(s.on_error(0), ReturnCode::Ok);
        assert_eq!(s.on_reset(0), ReturnCode::Ok);
    }

    #[test]
    fn execution_kind_distinguishes_three_modes() {
        // Spec §5.2.2.7.
        let kinds = [
            ExecutionKind::Periodic,
            ExecutionKind::EventDriven,
            ExecutionKind::Other,
        ];
        // Definitely distinct.
        assert_ne!(kinds[0], kinds[1]);
        assert_ne!(kinds[1], kinds[2]);
        assert_ne!(kinds[0], kinds[2]);
    }

    #[test]
    fn error_self_loop_allowed() {
        // Spec §5.2.2.4.8: on_error is invoked repeatedly for the error
        // state.
        assert!(is_valid_transition(
            LifeCycleState::Error,
            LifeCycleState::Error
        ));
    }
}
