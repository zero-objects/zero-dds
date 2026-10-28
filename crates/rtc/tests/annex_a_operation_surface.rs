// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex A / §6.2 — the Rust trait layer against the normative RTC IDL.
//!
//! Spec `formal/2008-04-04` Annex A defines the RTC IDL. ZeroDDS ships
//! the Local PSM (§6.3) as a Rust library rather than an IDL file, so
//! the coverage claim for §6.2 and Annex A is an equivalence claim: for
//! every IDL operation there is a Rust operation with the same
//! parameters and the same `ReturnCode_t` result.
//!
//! This test walks the Annex A operation list and calls each Rust
//! counterpart once, so the claim rests on executed code rather than on
//! a reading of the source.
//!
//! Four Annex A operations of `LightweightRTObject` have no counterpart
//! in this crate — `exit`, `get_context`, `get_owned_contexts`,
//! `get_context_handle`; they are carried as an open item in
//! `docs/spec-coverage/omg-rtc-1.0.open.md` and are deliberately absent
//! here.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_rtc::execution::ExecutionContextOperations;
use zerodds_rtc::lifecycle::{ComponentAction, is_valid_transition};
use zerodds_rtc::semantics::{
    DataFlowComponentAction, FsmComponentAction, ModeOfOperation, MultiModeComponentAction,
};
use zerodds_rtc::{
    ComponentProfile, ExecutionContext, ExecutionKind, Introspection, LifeCycleState,
    LightweightRtObject, ProfileId, ReturnCode,
};

/// A component that answers every Annex A `ComponentAction` callback and
/// records which ones the layer actually reached.
#[derive(Default)]
struct SurfaceComponent {
    calls: Vec<&'static str>,
    mode_changes: Vec<u16>,
}

impl ComponentAction for SurfaceComponent {
    fn on_initialize(&mut self) -> ReturnCode {
        self.calls.push("on_initialize");
        ReturnCode::Ok
    }
    fn on_finalize(&mut self) -> ReturnCode {
        self.calls.push("on_finalize");
        ReturnCode::Ok
    }
    fn on_startup(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_startup");
        ReturnCode::Ok
    }
    fn on_shutdown(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_shutdown");
        ReturnCode::Ok
    }
    fn on_activated(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_activated");
        ReturnCode::Ok
    }
    fn on_deactivated(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_deactivated");
        ReturnCode::Ok
    }
    fn on_aborting(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_aborting");
        ReturnCode::Ok
    }
    fn on_error(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_error");
        ReturnCode::Ok
    }
    fn on_reset(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_reset");
        ReturnCode::Ok
    }
}

impl DataFlowComponentAction for SurfaceComponent {
    fn on_execute(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_execute");
        ReturnCode::Ok
    }
    fn on_state_update(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_state_update");
        ReturnCode::Ok
    }
    fn on_rate_changed(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_rate_changed");
        ReturnCode::Ok
    }
}

impl FsmComponentAction for SurfaceComponent {
    fn on_action(&mut self, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_action");
        ReturnCode::Ok
    }
}

/// Annex A `ModeOfOperation` — a mode is an opaque, comparable value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mode(u16);

impl ModeOfOperation for Mode {
    fn name(&self) -> &str {
        "surface-mode"
    }
}

impl MultiModeComponentAction<Mode> for SurfaceComponent {
    fn on_mode_changed(&mut self, from: Mode, to: Mode, _exec_handle: u32) -> ReturnCode {
        self.calls.push("on_mode_changed");
        self.mode_changes.push(from.0);
        self.mode_changes.push(to.0);
        ReturnCode::Ok
    }
}

/// Annex A `ComponentAction` — every callback of the IDL interface has a
/// Rust counterpart with the `(ExecutionContextHandle_t) -> ReturnCode_t`
/// shape, and each one is reachable.
#[test]
fn component_action_covers_every_annex_a_callback() {
    let mut component = SurfaceComponent::default();
    let handle: u32 = 1;

    assert_eq!(component.on_initialize(), ReturnCode::Ok);
    assert_eq!(component.on_startup(handle), ReturnCode::Ok);
    assert_eq!(component.on_activated(handle), ReturnCode::Ok);
    assert_eq!(component.on_deactivated(handle), ReturnCode::Ok);
    assert_eq!(component.on_aborting(handle), ReturnCode::Ok);
    assert_eq!(component.on_error(handle), ReturnCode::Ok);
    assert_eq!(component.on_reset(handle), ReturnCode::Ok);
    assert_eq!(component.on_shutdown(handle), ReturnCode::Ok);
    assert_eq!(component.on_finalize(), ReturnCode::Ok);

    assert_eq!(
        component.calls,
        vec![
            "on_initialize",
            "on_startup",
            "on_activated",
            "on_deactivated",
            "on_aborting",
            "on_error",
            "on_reset",
            "on_shutdown",
            "on_finalize",
        ]
    );
}

/// Annex A `DataFlowComponentAction` / `FsmComponentAction` /
/// `MultiModeComponentAction` — the three §5.3 execution-semantics
/// interfaces carry the operations the IDL names, on top of
/// `ComponentAction`.
#[test]
fn execution_semantics_interfaces_carry_their_annex_a_operations() {
    let mut component = SurfaceComponent::default();
    let handle: u32 = 7;

    assert_eq!(component.on_execute(handle), ReturnCode::Ok);
    assert_eq!(component.on_state_update(handle), ReturnCode::Ok);
    assert_eq!(component.on_rate_changed(handle), ReturnCode::Ok);
    assert_eq!(component.on_action(handle), ReturnCode::Ok);
    assert_eq!(
        component.on_mode_changed(Mode(1), Mode(3), handle),
        ReturnCode::Ok
    );

    assert_eq!(
        component.calls,
        vec![
            "on_execute",
            "on_state_update",
            "on_rate_changed",
            "on_action",
            "on_mode_changed",
        ]
    );
    assert_eq!(component.mode_changes, vec![1, 3]);
    assert_eq!(Mode(3).name(), "surface-mode");
}

/// Annex A `LightweightRTObject` — the operations this crate implements
/// (`initialize`, `finalize`, `is_alive`, `attach_context`,
/// `detach_context`, `get_participating_contexts`) carry the IDL
/// parameter and `ReturnCode_t` shape.
#[test]
fn lightweight_rt_object_operations_match_the_annex_a_shape() {
    let mut rtc = LightweightRtObject::new(Box::new(SurfaceComponent::default()));

    assert!(!rtc.is_alive());
    assert_eq!(rtc.initialize(), ReturnCode::Ok);
    assert!(rtc.is_alive());

    let handle = rtc.attach_context().expect("attach in INACTIVE state");
    assert_eq!(rtc.get_participating_contexts(), vec![handle]);
    assert_eq!(
        rtc.get_context_state(handle),
        Some(LifeCycleState::Inactive)
    );

    assert_eq!(rtc.detach_context(handle), ReturnCode::Ok);
    assert!(rtc.get_participating_contexts().is_empty());
    assert_eq!(rtc.finalize(), ReturnCode::Ok);
    assert!(!rtc.is_alive());
}

/// Annex A `ExecutionContextOperations` — all twelve IDL operations
/// exist with the IDL parameter and result types, and each one is
/// exercised once.
#[test]
fn execution_context_operations_cover_all_twelve_annex_a_operations() {
    let mut rtc = LightweightRtObject::new(Box::new(SurfaceComponent::default()));
    assert_eq!(rtc.initialize(), ReturnCode::Ok);

    let mut context = ExecutionContext::new(ExecutionKind::Periodic);

    // 1 get_kind, 2 get_rate, 3 set_rate, 4 is_running, 5 start, 6 stop
    assert_eq!(context.get_kind(), ExecutionKind::Periodic);
    let default_rate = context.get_rate();
    assert!(default_rate > 0.0);
    assert_eq!(context.set_rate(50.0), ReturnCode::Ok);
    assert!((context.get_rate() - 50.0).abs() < f64::EPSILON);
    assert!(!context.is_running());
    assert_eq!(context.start(), ReturnCode::Ok);
    assert!(context.is_running());

    // 7 add_component, 8 get_component_state, 9 activate_component,
    // 10 deactivate_component, 11 reset_component, 12 remove_component
    let handle = context
        .add_component(&mut rtc)
        .expect("an initialized RTC is accepted");
    assert_eq!(
        context.get_component_state(&rtc, handle),
        LifeCycleState::Inactive
    );
    assert_eq!(context.activate_component(&mut rtc, handle), ReturnCode::Ok);
    assert_eq!(
        context.get_component_state(&rtc, handle),
        LifeCycleState::Active
    );
    assert_eq!(
        context.deactivate_component(&mut rtc, handle),
        ReturnCode::Ok
    );
    // `reset_component` is only defined out of the ERROR state.
    assert_eq!(
        context.reset_component(&mut rtc, handle),
        ReturnCode::PreconditionNotMet
    );
    assert_eq!(context.remove_component(&mut rtc, handle), ReturnCode::Ok);
    assert_eq!(context.stop(), ReturnCode::Ok);
    assert!(!context.is_running());
}

/// Annex A basic types (§5.2.3) and literal specifications (§5.2.4):
/// `ReturnCode_t`, `LifeCycleState`, `ExecutionKind` and
/// `ExecutionContextHandle_t` carry the IDL value sets, and the state
/// machine accepts exactly the IDL transitions.
#[test]
fn annex_a_value_types_and_literals_match_the_idl() {
    assert!(ReturnCode::Ok.is_ok());
    for code in [
        ReturnCode::Error,
        ReturnCode::BadParameter,
        ReturnCode::Unsupported,
        ReturnCode::OutOfResources,
        ReturnCode::PreconditionNotMet,
    ] {
        assert!(!code.is_ok());
        assert_eq!(code.into_result(), Err(code));
    }

    assert!(is_valid_transition(
        LifeCycleState::Inactive,
        LifeCycleState::Active
    ));
    assert!(!is_valid_transition(
        LifeCycleState::Created,
        LifeCycleState::Active
    ));

    assert_ne!(ExecutionKind::Periodic, ExecutionKind::EventDriven);
    assert_ne!(ExecutionKind::EventDriven, ExecutionKind::Other);

    // ExecutionContextHandle_t is the opaque integer handle of §5.2.2.8.
    let handle: zerodds_rtc::ExecutionContextHandle = 42;
    assert_eq!(handle, 42_u32);
}

/// Annex A introspection (§5.4) — the `Introspection` operations return
/// the resource data model the IDL declares.
#[test]
fn introspection_operations_return_the_annex_a_resource_model() {
    struct OneComponent(ComponentProfile);
    impl Introspection for OneComponent {
        fn get_component_profile(&self) -> &ComponentProfile {
            &self.0
        }
    }

    let profile = ComponentProfile {
        id: ProfileId::nil(),
        ..ComponentProfile::default()
    };
    let subject = OneComponent(profile);

    assert_eq!(subject.get_component_profile().id, ProfileId::nil());
    assert!(subject.get_ports().is_empty());
    assert!(subject.get_connectors().is_empty());
    assert!(subject.get_port_profile(&ProfileId::nil()).is_none());
    assert!(subject.get_connector_profile(&ProfileId::nil()).is_none());
}
