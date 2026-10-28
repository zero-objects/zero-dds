// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-permissions`. Safety classification: **SAFE** (a pure XML parser + topic match; signature validation is delegated to the `cms` module, which uses `rustls-webpki`).
//!
//! Permissions/governance XML parser + `AccessControlPlugin` implementation
//! for DDS-Security 1.2 §10.4 ("Builtin Access Control: DDS:Access:Permissions").
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumes `zerodds-security` (SPI).
//!
//! ## Public API
//!
//! - [`PermissionsAccessControl`] — `AccessControlPlugin` implementation.
//! - `xml` module — parser for the permissions XML (`<grant>` → `<allow_rule>` → `<publish>`/`<subscribe>` → `<topic>`).
//! - `governance` module — parser for the governance XML (`<topic_access_rule>` with `enable_discovery_protection`/`enable_liveliness_protection`/`metadata_protection_kind`/`data_protection_kind`).
//! - `signature` module — `XmlSignatureVerifier` trait + `NoOpVerifier` (dev) + `EnvelopeCheckVerifier` + `open_signed_permissions`.
//! - `cms` module — production CMS/PKCS#7 verifier (RFC 5751/5652/5280) based on `rustls-webpki`.
//! - [`topic_match`] module — POSIX `fnmatch()` for topic, partition, tag and subject expressions.
//! - [`delegation_check`] module — permissions delegation chain (sub-CA validation).
//! - [`psk_access`] module — pre-shared-key access control for out-of-band setups.

// Needs `std`: cms and x509-cert are built with `std`, and the plugin is built on
// security-pki and security-crypto, which need it.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

// Crypto primitive backend for production CMS signature *verify* (identical API
// across ring / aws-lc-rs). `ring` (default) or `aws-lc-rs` (FIPS, via `aws-lc`).
// Test-only key generation keeps the `ring` dev-dependency directly.
#[cfg(feature = "aws-lc")]
pub(crate) use aws_lc_rs as backend;
#[cfg(all(
    feature = "ring-backend",
    not(any(feature = "aws-lc", feature = "wolfcrypt"))
))]
pub(crate) use ring as backend;
#[cfg(all(feature = "wolfcrypt", not(feature = "aws-lc")))]
pub(crate) use wolfcrypt_compat as backend;
#[cfg(not(any(feature = "ring-backend", feature = "aws-lc", feature = "wolfcrypt")))]
compile_error!(
    "enable exactly one crypto backend: `ring-backend` (default), `aws-lc`, or `wolfcrypt`"
);

#[macro_use]
mod issued;

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
    AllowedCryptoAlgorithms, DEFAULT_EPHEMERAL_LIFETIME_SECS, Domain, DomainFilter, DomainRule,
    DomainTagMatch, EdgeIdentityConfig, EdgeIdentityMode, Governance, InterfaceBindingRule,
    PeerClass, PeerClassMatch, ProtectionKind, ReaderAccess, TopicRule, ZERODDS_NS,
    builtin_topic_rule, cn_pattern_match, parse_governance_xml,
};
pub use plugin::{
    PROP_GOVERNANCE, PROP_IDENTITY_CERTIFICATE, PROP_PERM_CERT, PROP_PERMISSIONS_XML,
    PROP_SUBJECT_NAME, PROP_TOKEN_CLASS_ID, PermissionsAccessControl, minor_code,
};
pub use psk_access::{
    CLASS_ID_PSK_PERMISSIONS, PROP_PSK_GOVERNANCE_XML, PROP_PSK_PERMISSIONS_ID,
    PROP_PSK_PERMISSIONS_XML, PROP_PSK_SUBJECT_NAME, PROP_RTPS_PSK_PROTECTION_KIND,
    PskPermissionsAccessControl, PskProfile, RtpsPskProtection,
};
pub use signature::{
    EnvelopeCheckVerifier, NoOpVerifier, XmlSignatureVerifier, open_signed_permissions,
};
pub use topic_match::{FnmatchFlags, fnmatch, topic_match};
pub use xml::{
    AccessRequest, Action, Criteria, DataTag, Decision, DomainRange, Grant, GrantSubject,
    PartitionMatching, Permissions, PermissionsError, Rule, Validity, parse_permissions_xml,
    subject_name_attributes,
};
