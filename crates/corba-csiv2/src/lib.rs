// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-csiv2`. Safety classification: **STANDARD**.
//!
//! OMG CORBA 3.3 Part 3 — Common Secure Interoperability v2 (CSIv2).
//! Voller CSIv2-Stack als pure-Rust `no_std + alloc`,
//! `forbid(unsafe_code)`:
//!
//! - **Association-Options** (Spec §24.2.4) — Bitmasken
//!   `Integrity` / `Confidentiality` / `EstablishTrustInTarget` /
//!   `EstablishTrustInClient` / `IdentityAssertion` /
//!   `DelegationByClient` / `NoProtection`.
//! - **Compound-Sec-Mech-List** (Spec §24.2.6.5) als
//!   `TAG_CSI_SEC_MECH_LIST`-Component-Body (AS-Layer + SAS-Layer).
//! - **GSSUP** Username-Password-Token (Spec §24.7) mit
//!   `INITIAL_CONTEXT_TOKEN`-Wrapping.
//! - **SAS-Protocol** (Spec §24.2): EstablishContext /
//!   CompleteEstablishContext / MessageInContext / ContextError.
//! - **TLS-Mechanism-OID** (Spec §24.2.6.5): `1.3.6.1.5.5.13` fuer
//!   `TLS_SEC_TRANS`.
//!
//! Spec: OMG CORBA 3.3 Part 3 §24.
//!
//! ## Schichten-Position
//!
//! Layer 8 — CORBA-Stack (Tier-A). Sitzt auf `zerodds-cdr` (Wire-
//! Codec). Konsumenten sind GIOP-/IIOP-Server (Layer-8-Tier-B/C) mit
//! Security-Stack-Konfiguration.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`AssociationOptions`] — §24.2.4 Bitmask.
//! - [`CompoundSecMech`] / [`CompoundSecMechList`] / [`AsContextSec`]
//!   / [`SasContextSec`] — §24.2.6.5.
//! - [`GssupCredentialToken`] / [`INITIAL_CONTEXT_TOKEN_TAG`] — §24.7.
//! - [`SasMessage`] / [`EstablishContext`] /
//!   [`CompleteEstablishContext`] / [`MessageInContext`] /
//!   [`ContextError`] / [`IdentityToken`] — §24.2 SAS-Protocol.
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_corba_csiv2::AssociationOptions;
//!
//! // Spec §24.2.4 — Association-Options-Bitmask: Integrity + Confidentiality.
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
