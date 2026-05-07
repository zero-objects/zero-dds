// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG RTC 1.0 — Robotic Technology Component.
//!
//! Crate `zerodds-rtc`. Safety classification: **STANDARD**.
//! Spec `formal/2008-04-04` (`docs/standards/cache/omg/rtc-1.0.pdf`).
//!
//! # Scope
//!
//! Implementiert sind: Lightweight RTC, Execution Semantics
//! (Periodic / Stimulus Response / Modes), Local PSM
//! (§5.2 + §5.3 + §6.3) als Rust-Library. Das Local PSM ist explizit
//! "Components on same network node, direct object refs without
//! CORBA-mediated middleware" (Spec §1.3 Punkt 1, S. 2) und damit
//! ZeroDDS-konform.
//!
//! # Was nicht abgedeckt ist
//!
//! * **CORBA PSM** (§6.5) — verlangt CORBA-ORB; ZeroDDS hat keinen.
//! * **Lightweight CCM PSM** (§6.4) — verlangt LwCCM-Container; siehe
//!   `crates/ccm/` welche die IDL-Equivalent-Transformation liefert,
//!   aber keinen Container bereitstellt.
//! * **Introspection §5.4 Resource Data Model** — partial: das
//!   Datenmodell ist im Crate, der Discovery-/Wire-Aspekt nicht.
//!
//! # Module
//!
//! * [`return_code`] — `ReturnCode_t` (Spec §5.2.1).
//! * [`lifecycle`] — `LifeCycleState`, `ExecutionKind`, `ComponentAction`
//!   Trait + State-Machine-Enforcement (Spec §5.2.2.3 / §5.2.2.7 /
//!   §5.2.2.4).
//! * [`object`] — `LightweightRTObject`-Modell (Spec §5.2.2.2) +
//!   `ExecutionContextHandle_t` (Spec §5.2.2.8).
//! * [`execution`] — `ExecutionContext` + `ExecutionContextOperations`
//!   (Spec §5.2.2.5 / §5.2.2.6).
//! * [`semantics`] — Execution-Kind-Profile (Periodic/Stimulus/Modes,
//!   Spec §5.3).
//!
//! # Beispiel
//!
//! ```
//! use zerodds_rtc::ReturnCode;
//!
//! // Spec §5.2.1.1: ReturnCode::Ok ist der einzige OK-Code.
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
pub use object::{ExecutionContextHandle, LightweightRtObject};
pub use resource::{
    ComponentProfile, ConnectorProfile, Introspection, PortDirection, PortProfile, ProfileId,
};
pub use return_code::ReturnCode;
pub use semantics::{
    DataFlowComponentAction, FsmComponentAction, ModeOfOperation, MultiModeComponentAction,
};
