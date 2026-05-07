// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-permissions`. Safety classification: **SAFE** (reiner XML-Parser + Topic-Match; Signatur-Validierung delegiert an [`cms`]-Modul, das `rustls-webpki` nutzt).
//!
//! Permissions/Governance-XML-Parser + `AccessControlPlugin`-Implementation
//! fuer DDS-Security 1.1 §9.4 ("Builtin Access Control Plugin").
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Konsumiert `zerodds-security` (SPI).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`PermissionsAccessControl`] — `AccessControlPlugin`-Implementation.
//! - [`xml`]-Modul — Parser fuer Permissions-XML (`<grant>` → `<allow_rule>` → `<publish>`/`<subscribe>` → `<topic>`).
//! - [`governance`]-Modul — Parser fuer Governance-XML (`<topic_access_rule>` mit `enable_discovery_protection`/`enable_liveliness_protection`/`metadata_protection_kind`/`data_protection_kind`).
//! - [`signature`]-Modul — `XmlSignatureVerifier`-Trait + `NoOpVerifier` (Dev) + `EnvelopeCheckVerifier` + `open_signed_permissions`.
//! - [`cms`]-Modul — produktiver CMS/PKCS#7-Verifier (RFC 5751/5652/5280) auf `rustls-webpki`-Basis.
//! - [`topic_match`]-Modul — Wildcard-Match `*`/`?`.
//! - [`delegation_check`]-Modul — Permissions-Delegation-Chain (Sub-CA-Validation).
//! - [`psk_access`]-Modul — Pre-Shared-Key-Access-Control fuer Out-of-Band-Setups.

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
