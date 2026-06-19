// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-csiv2`. Safety classification: **STANDARD**.
//!
//! OMG CORBA 3.3 Part 3 — Common Secure Interoperability v2 (CSIv2).
//! Full CSIv2 stack as pure-Rust `no_std + alloc`,
//! `forbid(unsafe_code)`:
//!
//! - **Association options** (Spec §24.2.4) — bitmasks
//!   `Integrity` / `Confidentiality` / `EstablishTrustInTarget` /
//!   `EstablishTrustInClient` / `IdentityAssertion` /
//!   `DelegationByClient` / `NoProtection`.
//! - **Compound sec-mech list** (Spec §24.2.6.5) as the
//!   `TAG_CSI_SEC_MECH_LIST` component body (AS layer + SAS layer).
//! - **GSSUP** username/password token (Spec §24.7) with
//!   `INITIAL_CONTEXT_TOKEN` wrapping.
//! - **SAS protocol** (Spec §24.2): EstablishContext /
//!   CompleteEstablishContext / MessageInContext / ContextError.
//! - **TLS mechanism OID** (Spec §24.2.6.5): `1.3.6.1.5.5.13` for
//!   `TLS_SEC_TRANS`.
//!
//! Spec: OMG CORBA 3.3 Part 3 §24.
//!
//! ## Layer position
//!
//! Layer 8 — CORBA stack (Tier A). Sits on `zerodds-cdr` (wire
//! codec). Consumers are GIOP/IIOP servers (Layer 8, Tier B/C) with
//! security-stack configuration.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`AssociationOptions`] — §24.2.4 bitmask.
//! - [`CompoundSecMech`] / [`CompoundSecMechList`] / [`AsContextSec`]
//!   / [`SasContextSec`] — §24.2.6.5.
//! - [`GssupCredentialToken`] / [`INITIAL_CONTEXT_TOKEN_TAG`] — §24.7.
//! - [`SasMessage`] / [`EstablishContext`] /
//!   [`CompleteEstablishContext`] / [`MessageInContext`] /
//!   [`ContextError`] / [`IdentityToken`] — §24.2 SAS protocol.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_corba_csiv2::AssociationOptions;
//!
//! // Spec §24.2.4 — association-options bitmask: Integrity + Confidentiality.
//! let opts = AssociationOptions(AssociationOptions::INTEGRITY | AssociationOptions::CONFIDENTIALITY);
//! assert!(opts.0 & AssociationOptions::INTEGRITY != 0);
//! assert!(opts.0 & AssociationOptions::CONFIDENTIALITY != 0);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod association_options;
pub mod gssup;
pub mod mech_list;
pub mod sas;

pub use association_options::AssociationOptions;
pub use gssup::{GssupCredentialToken, INITIAL_CONTEXT_TOKEN_TAG};
pub use mech_list::{AsContextSec, CompoundSecMech, CompoundSecMechList, SasContextSec};
pub use sas::{
    CompleteEstablishContext, ContextError, EstablishContext, IdentityToken, MessageInContext,
    SasMessage,
};
