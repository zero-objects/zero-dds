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
pub use object::{ExecutionContextHandle, LightweightRtObject};
pub use resource::{
    ComponentProfile, ConnectorProfile, Introspection, PortDirection, PortProfile, ProfileId,
};
pub use return_code::ReturnCode;
pub use semantics::{
    DataFlowComponentAction, FsmComponentAction, ModeOfOperation, MultiModeComponentAction,
};
