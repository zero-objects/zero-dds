// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 1 §11 — Portable Object Adapter (POA).
//!
//! Crate `zerodds-corba-poa`. Safety classification: **STANDARD**.
//! `no_std + alloc`, `forbid(unsafe_code)`.
//!
//! Voller POA-Stack mit allen 7 Policies in allen Modi, POAManager-
//! State-Machine, POA-Hierarchie, Active-Object-Map, ServantManager-
//! Hooks und Policy-Compatibility-Validator.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Poa`] / [`PoaConfig`] — POA-Instanz mit Hierarchie.
//! - [`PoaManager`] / [`PoaManagerState`] — State-Machine
//!   (`Holding` / `Active` / `Discarding` / `Inactive`).
//! - [`PolicySet`] + die 7 `*Policy`-Enums (Spec §11.3.7).
//! - [`Servant`] — Servant-Trait mit `primary_interface` /
//!   `primary_repository_id` (typisiert via `corba-ir`) /
//!   `all_interfaces` / `is_a` / `invoke`.
//! - [`ActiveObjectMap`] / [`ServantId`].
//! - [`ServantActivator`] / [`ServantLocator`] (Spec §11.3.5.7-§11.3.5.8).
//! - [`ObjectId`] (Spec §11.2.1).
//! - [`PoaError`] / [`PoaResult`].
//!
//! ## Beispiel
//!
//! ```
//! use zerodds_corba_poa::policies::PolicySet;
//! let policies = PolicySet::default();
//! assert!(policies.validate().is_ok());
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod active_object_map;
pub mod error;
pub mod object_id;
pub mod poa;
pub mod poa_manager;
pub mod policies;
pub mod servant;
pub mod servant_manager;

pub use active_object_map::{ActiveObjectMap, ServantId};
pub use error::{PoaError, PoaResult};
pub use object_id::ObjectId;
pub use poa::{Poa, PoaConfig};
pub use poa_manager::{PoaManager, PoaManagerState};
pub use policies::{
    IdAssignmentPolicy, IdUniquenessPolicy, ImplicitActivationPolicy, LifespanPolicy, PolicySet,
    RequestProcessingPolicy, ServantRetentionPolicy, ThreadPolicy,
};
pub use servant::Servant;
pub use servant_manager::{ServantActivator, ServantLocator, ServantLocatorCookie};
