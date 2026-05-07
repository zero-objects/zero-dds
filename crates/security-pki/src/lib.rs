// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-pki`. Safety classification: **SAFE** (reiner Wrapper um `rustls-webpki`; kein Raw-Crypto-Code in dieser Crate).
//!
//! PKI/X.509-Backend fuer das DDS-Security 1.1 §8.3
//! `AuthenticationPlugin`-SPI: Identity-Validation, Handshake-Token-
//! Sign/Verify, OCSP-Stapling-Liveness, CRL-Validation, Delegation-Chains.
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Konsumiert `zerodds-security` (SPI).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`PkiAuthenticationPlugin`] — `AuthenticationPlugin`-Impl (X.509 + RSA-PSS + ECDSA-P256/P384 + Ed25519).
//! - [`IdentityConfig`] / [`IdentityHandle`] — Identity-Loading.
//! - [`HandshakeToken`] / [`HandshakeError`] — Handshake-State-Machine (Challenge/Response).
//! - [`AuthRequestMessage`] — `auth_request`-Token-Codec (Spec §9.3.2.4).
//! - [`IdentityStatusToken`] — Identity-Status-Subscription (Spec §8.3.2.13).
//! - `ocsp`-Modul — OCSP-Stapling-Validation (RFC 6960).
//! - `crl`-Modul — CRL-Online-Checks + Cache.
//! - `delegation`-Modul — `DelegationLink` Sign/Verify + `DelegationChain`.
//! - [`PskAuthenticationPlugin`] — PSK-Variante (Spec §10.7).
//!
//! # Architektur-Gedanke
//!
//! Alle echten Crypto-Checks delegieren an `rustls-webpki` (der gleiche
//! Code, der rustls fuer TLS nutzt). Wir adaptieren nur die Eingabe
//! (PEM → DER) und die Ausgabe (OK/Err) auf den `AuthenticationPlugin`-
//! Trait. Damit vermeiden wir eigene Crypto-Implementation und profitieren
//! von rustls' Audit-Historie.
//!
//! # Beispiel (Pseudo — echte PEMs fehlen im Doctest)
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
pub use handshake_token::{
    FinalBuildInput, FinalTokenView, ReplyBuildInput, ReplyTokenView, RequestBuildInput,
    RequestTokenView, algo, build_final_token, build_reply_token, build_request_token, class_id,
    compute_hash_c, parse_final_token, parse_reply_token, parse_request_token, prop, signing_bytes,
};
pub use identity::{CertKeyAlgo, IdentityConfig, PkiError};
pub use identity_status::{
    IDENTITY_STATUS_CLASS_ID, IdentityStatusKind, IdentityStatusToken, KEY_EXPIRY_TIME,
    KEY_OCSP_RESPONSE, KEY_OCSP_STATUS, identity_status_properties,
};
pub use identity_token::{
    IDENTITY_TOKEN_CLASS_ID, IdentityToken, KEY_CA_ALGO, KEY_CA_SN, KEY_CERT_ALGO, KEY_CERT_SN,
    build_identity_token_from_pem, subject_match,
};
pub use ocsp::{OcspStatus, parse_ocsp_status, require_good_status};
pub use plugin::PkiAuthenticationPlugin;
pub use psk::{
    HKDF_INFO_SHARED_SECRET, PROP_PSK_ID, PROP_PSK_KEY_HEX, PskAuthenticationPlugin,
    derive_psk_shared_secret,
};

// Re-Exports aus `zerodds-security` fuer einfacheren Nutzer-Import.
pub use zerodds_security::authentication::{
    HandshakeHandle, HandshakeStepOutcome, IdentityHandle, SharedSecretHandle,
};
pub use zerodds_security::token::{BinaryProperty, DataHolder, WireProperty};
