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
//! - **SAS context state machine** (Spec Part 2 §10.3): the client and
//!   target halves that decide between a stateful context, named again
//!   by `MessageInContext`, and a stateless one re-established per
//!   request — [`SasClient`] / [`SasTarget`].
//! - **TLS mechanism OID** (Spec §24.2.6.5): `1.3.6.1.5.5.13` for
//!   `TLS_SEC_TRANS`.
//!
//! Spec: OMG CORBA 3.3 Part 3 §24.
//!
//! ## Layer position
//!
//! Layer 8 — CORBA stack (Tier A). Sits on `zerodds-wire` (wire
//! codec). Consumers are GIOP/IIOP servers (Layer 8, Tier B/C) with
//! security-stack configuration.
//!
//! ## Public API
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
pub mod authenticator;
pub mod gssup;
pub mod gssup_store;
pub mod mech_list;
pub mod sas;
pub mod session;

pub use association_options::AssociationOptions;
pub use authenticator::{
    IDENTITY_TYPE_ANONYMOUS, IDENTITY_TYPE_DISTINGUISHED_NAME, IDENTITY_TYPE_PRINCIPAL_NAME,
    IDENTITY_TYPE_X509_CERT_CHAIN, SasOutcome, authenticate_gssup,
};
pub use gssup::{GssupCredentialToken, INITIAL_CONTEXT_TOKEN_TAG};
pub use gssup_store::{GssupStoreError, GssupUserStore};
pub use mech_list::{AsContextSec, CompoundSecMech, CompoundSecMechList, SasContextSec};
pub use sas::{
    CompleteEstablishContext, ContextError, EstablishContext, IdentityToken, MessageInContext,
    SasMessage,
};
pub use session::{
    CONTEXT_ERROR_INVALID_EVIDENCE, CONTEXT_ERROR_INVALID_MECHANISM, CONTEXT_ERROR_NO_CONTEXT,
    RetainedContext, SasClient, SasTarget, TargetAction,
};
