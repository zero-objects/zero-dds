// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-permissions`. Safety classification: **SAFE** (a pure XML parser + topic match; signature validation is delegated to the [`cms`] module, which uses `rustls-webpki`).
//!
//! Permissions/governance XML parser + `AccessControlPlugin` implementation
//! for DDS-Security 1.1 §9.4 ("Builtin Access Control Plugin").
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumes `zerodds-security` (SPI).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`PermissionsAccessControl`] — `AccessControlPlugin` implementation.
//! - [`xml`] module — parser for the permissions XML (`<grant>` → `<allow_rule>` → `<publish>`/`<subscribe>` → `<topic>`).
//! - [`governance`] module — parser for the governance XML (`<topic_access_rule>` with `enable_discovery_protection`/`enable_liveliness_protection`/`metadata_protection_kind`/`data_protection_kind`).
//! - [`signature`] module — `XmlSignatureVerifier` trait + `NoOpVerifier` (dev) + `EnvelopeCheckVerifier` + `open_signed_permissions`.
//! - [`cms`] module — production CMS/PKCS#7 verifier (RFC 5751/5652/5280) based on `rustls-webpki`.
//! - [`topic_match`] module — wildcard match `*`/`?`.
//! - [`delegation_check`] module — permissions delegation chain (sub-CA validation).
//! - [`psk_access`] module — pre-shared-key access control for out-of-band setups.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod cms;
pub mod delegation_check;
mod governance;
mod plugin;
pub mod psk_access;
mod signature;
mod topic_match;
mod xml;

pub use cms::{CmsPkcs7Verifier, PROP_PERMISSIONS_CA};
pub use delegation_check::{
    DelegationCheckError, DelegationCheckResult, DelegationProfile, TrustAnchor, TrustPolicy,
    ValidatedChain, scope_intersect, validate_chain,
};
pub use governance::{
    DEFAULT_EPHEMERAL_LIFETIME_SECS, DomainFilter, DomainRule, EdgeIdentityConfig,
    EdgeIdentityMode, Governance, InterfaceBindingRule, PeerClass, PeerClassMatch, ProtectionKind,
    TopicRule, ZERODDS_NS, cn_pattern_match, parse_governance_xml,
};
pub use plugin::PermissionsAccessControl;
pub use psk_access::{
    CLASS_ID_PSK_PERMISSIONS, PROP_PSK_GOVERNANCE_XML, PROP_PSK_PERMISSIONS_ID,
    PROP_PSK_PERMISSIONS_XML, PROP_PSK_SUBJECT_NAME, PskPermissionsAccessControl, PskProfile,
};
pub use signature::{
    EnvelopeCheckVerifier, NoOpVerifier, XmlSignatureVerifier, open_signed_permissions,
};
pub use topic_match::topic_match;
pub use xml::{Grant, Permissions, PermissionsError, Validity, parse_permissions_xml};
