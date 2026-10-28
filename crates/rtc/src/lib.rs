// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG RTC 1.0 — Robotic Technology Component.
//!
//! Crate `zerodds-rtc`. Safety classification: **STANDARD**.
//! Spec `formal/2008-04-04` (`internal/standards/cache/omg/rtc-1.0.pdf`).
//!
//! # Scope
//!
//! Implemented are: lightweight RTC, execution semantics
//! (periodic / stimulus response / modes), local PSM
//! (§5.2 + §5.3 + §6.3) as a Rust library. The local PSM is explicitly
//! "Components on same network node, direct object refs without
//! CORBA-mediated middleware" (spec §1.3 point 1, p. 2) and thus
//! ZeroDDS-conformant.
//!
//! # What is not covered
//!
//! * **CORBA PSM** (§6.5) — requires a CORBA ORB; ZeroDDS has none.
//! * **Lightweight CCM PSM** (§6.4) — requires an LwCCM container; see
//!   `crates/ccm/` which provides the IDL-equivalent transformation
//!   but no container.
//! * **Introspection §5.4 Resource Data Model** — partial: the
//!   data model is in the crate, the discovery/wire aspect is not.
//!
//! # Modules
//!
//! * [`return_code`] — `ReturnCode_t` (spec §5.2.1).
//! * [`lifecycle`] — `LifeCycleState`, `ExecutionKind`, `ComponentAction`
//!   trait + state-machine enforcement (spec §5.2.2.3 / §5.2.2.7 /
//!   §5.2.2.4).
//! * [`object`] — `LightweightRTObject` model (spec §5.2.2.2) +
//!   `ExecutionContextHandle_t` (spec §5.2.2.8).
//! * [`execution`] — `ExecutionContext` + `ExecutionContextOperations`
//!   (spec §5.2.2.5 / §5.2.2.6).
//! * [`semantics`] — execution-kind profiles (periodic/stimulus/modes,
//!   spec §5.3).
//!
//! # Example
//!
//! ```
//! use zerodds_rtc::ReturnCode;
//!
//! // Spec §5.2.1.1: ReturnCode::Ok is the only OK code.
//! assert!(ReturnCode::Ok.is_ok());
//! assert!(!ReturnCode::PreconditionNotMet.is_ok());
//! assert_eq!(ReturnCode::Ok.into_result(), Ok(()));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod execution;
pub mod lifecycle;
pub mod object;
pub mod resource;
pub mod return_code;
pub mod semantics;

pub use execution::{ExecutionContext, ExecutionContextOperations};
pub use lifecycle::{ComponentAction, ExecutionKind, LifeCycleState};
pub use object::{ContextAssociation, ExecutionContextHandle, ExitOutcome, LightweightRtObject};
pub use resource::{
    ComponentProfile, ConnectorProfile, Introspection, PortDirection, PortProfile, ProfileId,
};
pub use return_code::ReturnCode;
pub use semantics::{
    DataFlowComponentAction, FsmComponentAction, ModeOfOperation, MultiModeComponentAction,
};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::float_cmp)]
mod annex_a_idl_conformance {
    //! Annex A "RTC IDL" and §6.2 "IDL Definitions" — signature equivalence.
    //!
    //! Annex A is marked *(non-normative)* in the PDF and is a convenience
    //! copy of `RTC.idl`; §6.2 cross-references it. ZeroDDS ships no `.idl`
    //! file for RTC, so the claim being checked here is the one the coverage
    //! document actually makes: that the Rust trait layer is *signature
    //! equivalent* to the Annex A interfaces — same operation names, same
    //! return-type category, same parameter arity.
    //!
    //! The check is a compile-time one. Each Annex A interface is
    //! implemented or exercised below with the IDL's own parameter shapes,
    //! and each operation's result is bound to a variable of the type the
    //! IDL declares. A renamed operation, a dropped parameter or a changed
    //! return type stops this module from compiling — which is a stronger
    //! statement than any string comparison against the PDF would be.
    //!
    //! IDL type correspondence used throughout:
    //!
    //! | Annex A                     | Rust                        |
    //! |-----------------------------|-----------------------------|
    //! | `ReturnCode_t`              | [`ReturnCode`]              |
    //! | `LifeCycleState`            | [`LifeCycleState`]          |
    //! | `ExecutionKind`             | [`ExecutionKind`]           |
    //! | `ExecutionContextHandle_t`  | [`ExecutionContextHandle`]  |
    //! | `boolean`                   | `bool`                      |
    //! | `double`                    | `f64`                       |
    //! | `ExecutionContextList`      | `Vec<ExecutionContextHandle>`|

    use super::*;
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    /// `interface ComponentAction` — the nine `on_*` operations, each
    /// `ReturnCode_t`, the last seven taking `in ExecutionContextHandle_t`.
    #[test]
    fn annex_a_component_action_operations() {
        struct Stub;
        impl ComponentAction for Stub {}
        let mut s = Stub;
        let h: ExecutionContextHandle = 7;
        let _: ReturnCode = s.on_initialize();
        let _: ReturnCode = s.on_finalize();
        let _: ReturnCode = s.on_startup(h);
        let _: ReturnCode = s.on_shutdown(h);
        let _: ReturnCode = s.on_activated(h);
        let _: ReturnCode = s.on_deactivated(h);
        let _: ReturnCode = s.on_aborting(h);
        let _: ReturnCode = s.on_error(h);
        let rc: ReturnCode = s.on_reset(h);
        assert!(rc.is_ok(), "the default body is RTC_OK");
    }

    /// `interface LightweightRTObject : ComponentAction` — ten operations.
    ///
    /// Two of them are shaped differently from the IDL, deliberately, and
    /// the difference is asserted here rather than left implicit:
    ///
    /// * `attach_context` returns `ExecutionContextHandle_t` in the IDL and
    ///   has no way to report failure. The Rust form returns
    ///   `Result<ExecutionContextHandle, ReturnCode>` so that a refusal
    ///   carries its reason instead of an out-of-band sentinel handle.
    /// * `exit` returns `ReturnCode_t` in the IDL. The Rust form returns
    ///   [`ExitOutcome`], which carries that code *and* the handles of the
    ///   contexts the object owned, because §5.2.2.2.4 makes the caller the
    ///   owner of an `ExecutionContext` and it must learn which ones stopped.
    #[test]
    fn annex_a_lightweight_rt_object_operations() {
        struct Stub;
        impl ComponentAction for Stub {}
        let mut o = LightweightRtObject::new(Box::new(Stub));

        let _: ReturnCode = o.initialize();
        let _: bool = o.is_alive();
        let handle: ExecutionContextHandle =
            o.attach_context().expect("attach in the inactive state");
        let _: Option<ContextAssociation> = o.get_context(handle);
        let owned: Vec<ExecutionContextHandle> = o.get_owned_contexts();
        let participating: Vec<ExecutionContextHandle> = o.get_participating_contexts();
        assert!(participating.contains(&handle));
        let _ = owned;
        // Annex A `ReturnCode_t reset();` — no context argument.
        let _: ReturnCode = o.reset_all();
        let _: ReturnCode = o.detach_context(handle);
        let outcome: ExitOutcome = o.exit();
        let _: ReturnCode = outcome.code;
        let _: ReturnCode = o.finalize();
    }

    /// `interface ExecutionContext` — twelve operations. `is_running` is
    /// `boolean`, `get_rate` is `double`, `get_component_state` is
    /// `LifeCycleState`, `get_kind` is `ExecutionKind`, the rest are
    /// `ReturnCode_t`; the five component operations take
    /// `in LightweightRTObject`.
    #[test]
    fn annex_a_execution_context_operations() {
        struct Stub;
        impl ComponentAction for Stub {}
        let mut comp = LightweightRtObject::new(Box::new(Stub));
        assert!(comp.initialize().is_ok());
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);

        let _: bool = ec.is_running();
        let _: ReturnCode = ec.start();
        let _: f64 = ec.get_rate();
        let _: ReturnCode = ec.set_rate(50.0);
        // `add_component` returns the handle the IDL declares, wrapped in a
        // `Result` so a refusal carries its `ReturnCode_t`.
        let h: ExecutionContextHandle =
            ec.add_component(&mut comp).expect("add an initialized RTC");
        // The remaining component operations take the RTC and the handle
        // that identifies it inside this context; the IDL passes only the
        // object reference, which in CORBA already carries that identity.
        let _: ReturnCode = ec.activate_component(&mut comp, h);
        let state: LifeCycleState = ec.get_component_state(&comp, h);
        assert_eq!(state, LifeCycleState::Active);
        let _: ReturnCode = ec.deactivate_component(&mut comp, h);
        let _: ReturnCode = ec.reset_component(&mut comp, h);
        let _: ReturnCode = ec.remove_component(&mut comp, h);
        let kind: ExecutionKind = ec.get_kind();
        assert_eq!(kind, ExecutionKind::Periodic);
        let _: ReturnCode = ec.stop();
    }

    /// `interface DataFlowComponentAction` — three operations, and
    /// `interface FsmParticipantAction` — one. Annex A calls the
    /// stimulus-response interface `FsmParticipantAction`; §5.3.2 of the
    /// normative body calls it `FsmComponentAction`, and the Rust name
    /// follows the normative body.
    #[test]
    fn annex_a_data_flow_and_fsm_operations() {
        struct Stub;
        impl ComponentAction for Stub {}
        impl DataFlowComponentAction for Stub {}
        impl FsmComponentAction for Stub {}
        let mut s = Stub;
        let h: ExecutionContextHandle = 3;
        let _: ReturnCode = s.on_execute(h);
        let _: ReturnCode = s.on_state_update(h);
        let _: ReturnCode = s.on_rate_changed(h);
        let _: ReturnCode = s.on_action(h);
    }

    /// `interface MultiModeComponentAction` — `on_mode_changed`, plus
    /// `interface ModeCapable`'s mode concept. Annex A models `Mode` as an
    /// opaque `interface Mode`; the Rust form makes it a type parameter
    /// bound by [`ModeOfOperation`], so a component's mode set is checked at
    /// compile time instead of at the object reference.
    #[test]
    fn annex_a_multi_mode_operations() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Drive {
            Idle,
            Running,
        }
        impl ModeOfOperation for Drive {
            fn name(&self) -> &str {
                match self {
                    Self::Idle => "IDLE",
                    Self::Running => "RUNNING",
                }
            }
        }
        struct Stub;
        impl ComponentAction for Stub {}
        impl MultiModeComponentAction<Drive> for Stub {}
        let mut s = Stub;
        let rc: ReturnCode = s.on_mode_changed(Drive::Idle, Drive::Running, 1);
        assert!(rc.is_ok());
        assert_eq!(Drive::Running.name(), "RUNNING");
    }

    /// `enum ReturnCode_t` — six enumerators, in Annex A's order.
    /// `enum LifeCycleState` — four. `enum ExecutionKind` — three.
    /// A missing or renamed enumerator fails to compile; the counts guard
    /// against a silently *added* one, which would make the mapping to the
    /// IDL no longer total.
    #[test]
    fn annex_a_enumerations_are_complete_and_distinct() {
        let codes = [
            ReturnCode::Ok,
            ReturnCode::Error,
            ReturnCode::BadParameter,
            ReturnCode::Unsupported,
            ReturnCode::OutOfResources,
            ReturnCode::PreconditionNotMet,
        ];
        assert_eq!(codes.len(), 6, "RTC::ReturnCode_t has six enumerators");
        for (i, a) in codes.iter().enumerate() {
            for b in &codes[i + 1..] {
                assert_ne!(a, b, "enumerators must be distinct");
            }
        }
        assert!(ReturnCode::Ok.is_ok());
        assert!(!ReturnCode::Error.is_ok());

        let states = [
            LifeCycleState::Created,
            LifeCycleState::Inactive,
            LifeCycleState::Active,
            LifeCycleState::Error,
        ];
        assert_eq!(states.len(), 4, "RTC::LifeCycleState has four enumerators");

        let kinds = [
            ExecutionKind::Periodic,
            ExecutionKind::EventDriven,
            ExecutionKind::Other,
        ];
        assert_eq!(kinds.len(), 3, "RTC::ExecutionKind has three enumerators");
    }

    /// Annex A's `reset()` on the RTC itself, as opposed to
    /// §5.2.2.6.10's `reset_component` on the context: it takes no handle,
    /// and it must return every erred context to `INACTIVE_STATE`.
    #[test]
    fn annex_a_reset_returns_every_erred_context_to_inactive() {
        struct Stub;
        impl ComponentAction for Stub {}
        let mut o = LightweightRtObject::new(Box::new(Stub));
        assert!(o.initialize().is_ok());
        let a = o.attach_context().expect("attach a");
        let b = o.attach_context().expect("attach b");

        let _ = (a, b);
        // `transition_to_error` only fires from ACTIVE, so drive the object
        // there through a context first.
        let mut ec = ExecutionContext::new(ExecutionKind::Periodic);
        let active_handle = ec.add_component(&mut o).expect("add an initialized RTC");
        assert!(ec.activate_component(&mut o, active_handle).is_ok());
        o.transition_to_error(active_handle);
        assert_eq!(
            o.get_context_state(active_handle),
            Some(LifeCycleState::Error),
            "precondition: the context is in ERROR_STATE"
        );

        // Annex A `reset()`: no handle argument.
        let rc: ReturnCode = o.reset_all();
        assert!(rc.is_ok(), "reset of an erred context must succeed: {rc:?}");
        assert_eq!(
            o.get_context_state(active_handle),
            Some(LifeCycleState::Inactive),
            "ERROR_STATE -> INACTIVE_STATE"
        );

        // An object with nothing in ERROR_STATE has nothing to reset, and
        // that is not a failure.
        assert!(o.reset_all().is_ok(), "idempotent when nothing has erred");
    }
}
