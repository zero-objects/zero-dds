// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Execution-Semantics-Profile — Spec §5.3.
//!
//! Extensions beyond Lightweight RTC that define domain-specific
//! behavior patterns:
//! * §5.3.1 Periodic Sampled Data Processing → `DataFlowComponentAction`.
//! * §5.3.2 Stimulus Response Processing → `FsmComponentAction`.
//! * §5.3.3 Modes of Operation → `ModeOfOperation` +
//!   `MultiModeComponentAction`.

use crate::lifecycle::ComponentAction;
use crate::return_code::ReturnCode;

/// Spec §5.3.1 — `DataFlowComponentAction`. Extends
/// `ComponentAction` with periodic-sampled-data-processing callbacks.
///
/// Per tick (at the frequency `ExecutionContext::get_rate`),
/// `on_execute` (read-only phase) followed by `on_state_update`
/// (state-mutation phase) is invoked.
pub trait DataFlowComponentAction: ComponentAction {
    /// Spec §5.3.1.x — read-only phase of a periodic tick.
    fn on_execute(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.3.1.x — state-mutation phase of a periodic tick.
    fn on_state_update(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.3.1.x — `on_rate_changed` when `ExecutionContext::
    /// set_rate` is invoked.
    fn on_rate_changed(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
}

/// Spec §5.3.2 — `FsmComponentAction`. Stimulus-response processing
/// as a finite state machine.
pub trait FsmComponentAction: ComponentAction {
    /// Spec §5.3.2 — invoked when an event arrives (caller-defined
    /// event type).
    fn on_action(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
}

/// `ModeOfOperation` (spec §5.3.3) — abstract mode identification.
/// The implementor defines the concrete mode set (e.g. `enum AutonomousMode
/// { Idle, Driving, Charging }`).
pub trait ModeOfOperation: core::fmt::Debug + Copy + PartialEq + Eq {
    /// Mode name (spec-conformant, string-based in introspection).
    fn name(&self) -> &str;
}

/// Spec §5.3.3 — `MultiModeComponentAction`. Extends
/// `ComponentAction` with mode-change callbacks.
pub trait MultiModeComponentAction<M: ModeOfOperation>: ComponentAction {
    /// Spec §5.3.3.x — `on_mode_changed`. Invoked on the change from
    /// `from_mode` to `to_mode`.
    fn on_mode_changed(&mut self, _from: M, _to: M, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    struct DataFlowStub {
        executes: u32,
        state_updates: u32,
        rate_changes: u32,
    }
    impl ComponentAction for DataFlowStub {}
    impl DataFlowComponentAction for DataFlowStub {
        fn on_execute(&mut self, _h: u32) -> ReturnCode {
            self.executes += 1;
            ReturnCode::Ok
        }
        fn on_state_update(&mut self, _h: u32) -> ReturnCode {
            self.state_updates += 1;
            ReturnCode::Ok
        }
        fn on_rate_changed(&mut self, _h: u32) -> ReturnCode {
            self.rate_changes += 1;
            ReturnCode::Ok
        }
    }

    #[test]
    fn data_flow_callbacks_are_invoked_independently() {
        // Spec §5.3.1 — separate execute / state_update phases.
        let mut s = DataFlowStub {
            executes: 0,
            state_updates: 0,
            rate_changes: 0,
        };
        s.on_execute(0);
        s.on_execute(0);
        s.on_state_update(0);
        s.on_rate_changed(0);
        assert_eq!(s.executes, 2);
        assert_eq!(s.state_updates, 1);
        assert_eq!(s.rate_changes, 1);
    }

    struct FsmStub {
        actions: u32,
    }
    impl ComponentAction for FsmStub {}
    impl FsmComponentAction for FsmStub {
        fn on_action(&mut self, _h: u32) -> ReturnCode {
            self.actions += 1;
            ReturnCode::Ok
        }
    }

    #[test]
    fn fsm_on_action_is_invoked_per_event() {
        // Spec §5.3.2.
        let mut s = FsmStub { actions: 0 };
        s.on_action(0);
        s.on_action(0);
        s.on_action(0);
        assert_eq!(s.actions, 3);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AutoMode {
        Idle,
        Driving,
        Charging,
    }

    impl ModeOfOperation for AutoMode {
        fn name(&self) -> &str {
            match self {
                Self::Idle => "Idle",
                Self::Driving => "Driving",
                Self::Charging => "Charging",
            }
        }
    }

    #[test]
    fn mode_of_operation_provides_string_name() {
        // Spec §5.3.3.
        assert_eq!(AutoMode::Idle.name(), "Idle");
        assert_eq!(AutoMode::Driving.name(), "Driving");
        assert_eq!(AutoMode::Charging.name(), "Charging");
    }

    struct ModeStub {
        transitions: alloc::vec::Vec<(AutoMode, AutoMode)>,
    }
    impl ComponentAction for ModeStub {}
    impl MultiModeComponentAction<AutoMode> for ModeStub {
        fn on_mode_changed(&mut self, from: AutoMode, to: AutoMode, _h: u32) -> ReturnCode {
            self.transitions.push((from, to));
            ReturnCode::Ok
        }
    }

    #[test]
    fn multi_mode_on_mode_changed_records_transition() {
        // Spec §5.3.3.x.
        let mut s = ModeStub {
            transitions: alloc::vec::Vec::new(),
        };
        s.on_mode_changed(AutoMode::Idle, AutoMode::Driving, 1);
        s.on_mode_changed(AutoMode::Driving, AutoMode::Charging, 1);
        assert_eq!(
            s.transitions,
            alloc::vec![
                (AutoMode::Idle, AutoMode::Driving),
                (AutoMode::Driving, AutoMode::Charging),
            ]
        );
    }
}
