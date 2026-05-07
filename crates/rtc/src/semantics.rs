// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Execution-Semantics-Profile — Spec §5.3.
//!
//! Erweiterungen ueber Lightweight RTC hinaus, die domain-spezifische
//! Verhaltens-Pattern definieren:
//! * §5.3.1 Periodic Sampled Data Processing → `DataFlowComponentAction`.
//! * §5.3.2 Stimulus Response Processing → `FsmComponentAction`.
//! * §5.3.3 Modes of Operation → `ModeOfOperation` +
//!   `MultiModeComponentAction`.

use crate::lifecycle::ComponentAction;
use crate::return_code::ReturnCode;

/// Spec §5.3.1 — `DataFlowComponentAction`. Erweitert
/// `ComponentAction` um Periodic-Sampled-Data-Processing-Callbacks.
///
/// Pro Tick (mit Frequenz `ExecutionContext::get_rate`) werden
/// `on_execute` (Read-Only-Phase) gefolgt von `on_state_update`
/// (State-Mutation-Phase) invoked.
pub trait DataFlowComponentAction: ComponentAction {
    /// Spec §5.3.1.x — Read-Only-Phase eines Periodic-Tick.
    fn on_execute(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.3.1.x — State-Mutation-Phase eines Periodic-Tick.
    fn on_state_update(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
    /// Spec §5.3.1.x — `on_rate_changed` wenn `ExecutionContext::
    /// set_rate` invoked.
    fn on_rate_changed(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
}

/// Spec §5.3.2 — `FsmComponentAction`. Stimulus-Response-Processing
/// als Finite-State-Machine.
pub trait FsmComponentAction: ComponentAction {
    /// Spec §5.3.2 — wird invoked wenn Event eintrifft (Caller-defined
    /// Event-Type).
    fn on_action(&mut self, _exec_handle: u32) -> ReturnCode {
        ReturnCode::Ok
    }
}

/// `ModeOfOperation` (Spec §5.3.3) — abstrakte Mode-Identifikation.
/// Implementor definiert konkretes Mode-Set (z.B. `enum AutonomousMode
/// { Idle, Driving, Charging }`).
pub trait ModeOfOperation: core::fmt::Debug + Copy + PartialEq + Eq {
    /// Mode-Name (Spec-konform stringbasiert in Introspection).
    fn name(&self) -> &str;
}

/// Spec §5.3.3 — `MultiModeComponentAction`. Erweitert
/// `ComponentAction` um Mode-Wechsel-Callbacks.
pub trait MultiModeComponentAction<M: ModeOfOperation>: ComponentAction {
    /// Spec §5.3.3.x — `on_mode_changed`. Wird beim Wechsel von
    /// `from_mode` nach `to_mode` invoked.
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
        // Spec §5.3.1 — getrennte execute / state_update Phasen.
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
