// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-pki`. Safety classification: **SAFE** (a pure wrapper around `rustls-webpki`; no raw-crypto code in this crate).
//!
//! PKI/X.509 backend for the DDS-Security 1.1 §8.3
//! `AuthenticationPlugin` SPI: identity validation, handshake-token
//! sign/verify, OCSP stapling liveness, CRL validation, delegation chains.
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumes `zerodds-security` (SPI).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`PkiAuthenticationPlugin`] — `AuthenticationPlugin` impl (X.509 + RSA-PSS + ECDSA-P256/P384 + Ed25519).
//! - [`IdentityConfig`] / [`IdentityHandle`] — identity loading.
//! - [`HandshakeToken`] / [`HandshakeError`] — handshake state machine (challenge/response).
//! - [`AuthRequestMessage`] — `auth_request` token codec (spec §9.3.2.4).
//! - [`IdentityStatusToken`] — identity-status subscription (spec §8.3.2.13).
//! - `ocsp` module — OCSP stapling validation (RFC 6960).
//! - `crl` module — CRL online checks + cache.
//! - `delegation` module — `DelegationLink` sign/verify + `DelegationChain`.
//! - [`PskAuthenticationPlugin`] — PSK variant (spec §10.7).
//!
//! # Architecture rationale
//!
//! All real crypto checks delegate to `rustls-webpki` (the same
//! code that rustls uses for TLS). We only adapt the input
//! (PEM → DER) and the output (OK/Err) to the `AuthenticationPlugin`
//! trait. This avoids our own crypto implementation and profits
//! from rustls' audit history.
//!
//! # Example (pseudo — real PEMs are missing in the doctest)
//!
//! ```ignore
//! use zerodds_security::AuthenticationPlugin;
//! use zerodds_security_pki::{PkiAuthenticationPlugin, IdentityConfig};
//!
//! let mut plugin = PkiAuthenticationPlugin::new();
//! let cfg = IdentityConfig {
//!     identity_cert_pem: ALICE_CERT_PEM.into(),
//!     identity_ca_pem: CA_PEM.into(),
//! };
//! let local = plugin.validate_with_config(cfg, [0xAA; 16])?;
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod auth_request;
pub mod crl;
pub mod delegation;
pub mod guid_adjust;
pub mod handshake_token;
mod identity;
pub mod identity_status;
pub mod identity_token;
mod ocsp;
mod plugin;
pub mod psk;

pub use auth_request::{
    AUTH_REQUEST_CLASS_ID, AuthRequestToken, FUTURE_CHALLENGE_KEY, auth_request_properties,
};
pub use crl::{CrlParseError, parse_crl_serials, validate_crl};
pub use delegation::{
    DELEGATION_MAGIC, DELEGATION_VERSION, DelegationChain, DelegationError, DelegationLink,
    DelegationResult, MAX_CHAIN_DEPTH_HARD_CAP, MAX_PARTITION_PATTERNS, MAX_PATTERN_LEN,
    MAX_TOPIC_PATTERNS, SignatureAlgorithm,
};
pub use guid_adjust::adjust_participant_guid_prefix;
pub use handshake_token::{
    FinalBuildInput, FinalTokenView, ReplyBuildInput, ReplyTokenView, RequestBuildInput,
    RequestTokenView, algo, build_final_token, build_reply_token, build_request_token, class_id,
    compute_hash_c, final_signing_bytes, parse_final_token, parse_reply_token, parse_request_token,
    prop, reply_signing_bytes, serialize_bin_prop_seq,
};
pub use identity::{CertKeyAlgo, IdentityConfig, PkiError, first_cert_der};
pub use identity_status::{
    IDENTITY_STATUS_CLASS_ID, IdentityStatusKind, IdentityStatusToken, KEY_EXPIRY_TIME,
    KEY_OCSP_RESPONSE, KEY_OCSP_STATUS, identity_status_properties,
};
pub use identity_token::{
    IDENTITY_TOKEN_CLASS_ID, IdentityToken, KEY_CA_ALGO, KEY_CA_SN, KEY_CERT_ALGO, KEY_CERT_SN,
    build_identity_token_from_der, build_identity_token_from_pem, subject_match,
};
pub use ocsp::{OcspStatus, parse_ocsp_status, require_good_status};
pub use plugin::PkiAuthenticationPlugin;
pub use psk::{
    HKDF_INFO_SHARED_SECRET, PROP_PSK_ID, PROP_PSK_KEY_HEX, PskAuthenticationPlugin,
    derive_psk_shared_secret,
};

// Re-exports from `zerodds-security` for easier user import.
pub use zerodds_security::authentication::{
    HandshakeHandle, HandshakeStepOutcome, IdentityHandle, SharedSecretHandle,
};
pub use zerodds_security::token::{BinaryProperty, DataHolder, WireProperty};
