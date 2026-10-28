// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-pki`. Safety classification: **SAFE** (a pure wrapper around `rustls-webpki`; no raw-crypto code in this crate).
//!
//! PKI/X.509 backend for the DDS-Security 1.2 §9.3/§10.3
//! `AuthenticationPlugin` SPI: identity validation, handshake-token
//! sign/verify, OCSP stapling liveness, CRL validation, delegation chains.
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumes `zerodds-security` (SPI).
//!
//! ## Public API
//!
//! - [`PkiAuthenticationPlugin`] — `AuthenticationPlugin` impl (X.509 + RSA-PSS + ECDSA-P256/P384 + Ed25519).
//! - [`IdentityConfig`] / [`IdentityHandle`] — identity loading.
//! - [`handshake_token`] — the three `HandshakeMessageToken`s of §10.3.2.5: [`build_request_token`] / [`parse_request_token`] and the reply and final pairs.
//! - [`AuthRequestToken`] — `auth_request` token codec (spec §9.3.2.4).
//! - [`IdentityStatusToken`] — identity-status subscription (spec §8.3.2.13).
//! - `ocsp` module — OCSP stapling validation (RFC 6960).
//! - `crl` module — CRL online checks + cache.
//! - `delegation` module — `DelegationLink` sign/verify + `DelegationChain`.
//! - [`PskAuthenticationPlugin`] — `DDS:Auth:PSK` (spec §10.7), and
//!   [`PskHandshakeAuthenticationPlugin`], a PSK handshake of ZeroDDS.
//! - `x509v1_compat` module — the explicitly switched-on X.509 v1
//!   identity-certificate deviation from §10.3.1.3, off by default.
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

// Needs `std`: the plugin state sits behind `std::sync`, OCSP goes over the network, and
// x509-cert is built with its `std` feature.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

// Crypto primitive backend: `ring` (default) or `aws-lc-rs` (FIPS, via `aws-lc`).
// Symmetric/verify APIs are identical; the asymmetric *signing* keypair APIs
// differ slightly between ring 0.17 and aws-lc-rs — bridged in `compat`.
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

/// Backend API deltas: ring 0.17 dropped `EcdsaKeyPair::from_pkcs8`'s `rng`
/// arg and exposes RSA modulus length via `RsaKeyPair::public().modulus_len()`,
/// while aws-lc-rs (and wolfcrypt-ring-compat, which mirrors its API) kept the
/// rng-less ctor and offers `public_modulus_len()`.
pub(crate) mod compat {
    use crate::backend::error::KeyRejected;
    use crate::backend::rand::SecureRandom;
    use crate::backend::signature::{EcdsaKeyPair, EcdsaSigningAlgorithm, RsaKeyPair};

    // zerodds-lint: allow no_dyn_in_safe — `&dyn SecureRandom` mirrors ring's
    // `EcdsaKeyPair::from_pkcs8` API signature (external crate boundary, not our
    // own dynamic dispatch); the aws-lc/wolfcrypt arms below don't use it.
    #[cfg(not(any(feature = "aws-lc", feature = "wolfcrypt")))]
    pub(crate) fn ecdsa_from_pkcs8(
        alg: &'static EcdsaSigningAlgorithm,
        pkcs8: &[u8],
        rng: &dyn SecureRandom,
    ) -> Result<EcdsaKeyPair, KeyRejected> {
        EcdsaKeyPair::from_pkcs8(alg, pkcs8, rng)
    }
    #[cfg(any(feature = "aws-lc", feature = "wolfcrypt"))]
    pub(crate) fn ecdsa_from_pkcs8(
        alg: &'static EcdsaSigningAlgorithm,
        pkcs8: &[u8],
        _rng: &dyn SecureRandom,
    ) -> Result<EcdsaKeyPair, KeyRejected> {
        EcdsaKeyPair::from_pkcs8(alg, pkcs8)
    }

    #[cfg(not(any(feature = "aws-lc", feature = "wolfcrypt")))]
    pub(crate) fn rsa_modulus_len(k: &RsaKeyPair) -> usize {
        k.public().modulus_len()
    }
    #[cfg(any(feature = "aws-lc", feature = "wolfcrypt"))]
    pub(crate) fn rsa_modulus_len(k: &RsaKeyPair) -> usize {
        k.public_modulus_len()
    }
}

pub mod auth_request;
pub mod crl;
pub mod delegation;
pub mod encrypted_key;
pub mod guid_adjust;
pub mod handshake_token;
mod identity;
pub mod identity_status;
pub mod identity_token;
mod ocsp;
mod plugin;
pub mod psk;
pub mod subject;
pub mod x509v1_compat;

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
    RequestTokenView, algo, build_auth_request_token, build_final_token, build_reply_token,
    build_request_token, class_id, compute_hash_c, final_signing_bytes, parse_auth_request_token,
    parse_final_token, parse_reply_token, parse_request_token, prop, reply_signing_bytes,
    serialize_bin_prop_seq,
};
pub use identity::{
    CertKeyAlgo, IdentityConfig, IdentitySigner, PkiError, cert_key_algo, crls_der, first_cert_der,
    private_key_pkcs8_der,
};
pub use identity_status::{
    IDENTITY_STATUS_CLASS_ID, IdentityStatusKind, IdentityStatusToken, KEY_EXPIRY_TIME,
    KEY_OCSP_RESPONSE, KEY_OCSP_STATUS, identity_status_properties,
};
pub use identity_token::{
    IDENTITY_TOKEN_CLASS_ID, IdentityToken, KEY_CA_ALGO, KEY_CA_SN, KEY_CERT_ALGO, KEY_CERT_SN,
    build_identity_token_from_der, build_identity_token_from_der_named,
    build_identity_token_from_der_with, build_identity_token_from_pem,
    build_identity_token_from_pem_named, certificate_algorithm_name, format_subject_name,
    subject_match,
};
pub use ocsp::{
    OcspOutcome, OcspPolicy, OcspStatus, OcspUnreachable, OcspVerdict, build_request,
    check_identity, parse_ocsp_status, query_responder, require_good_status, responder_urls,
    verify_staple,
};
pub use plugin::{
    AlgorithmNames, PERMISSIONS_CREDENTIAL_CERT, PkiAuthenticationPlugin, verify_der_signature,
};
pub use psk::{
    HKDF_INFO_SHARED_SECRET, PROP_PSK_ID, PROP_PSK_KEY_HEX, PskAuthenticationPlugin,
    PskHandshakeAuthenticationPlugin, SPEC_CLASS_ID as PSK_SPEC_CLASS_ID, derive_psk_shared_secret,
};
pub use subject::subject_attributes;
pub use x509v1_compat::{
    CertAcceptance, CertPolicy, LOG_CATEGORY_X509V1_DEVIATION, PROP_ACCEPT_X509V1_IDENTITY,
    SPEC_CLAUSE_IDENTITY_CERTIFICATE, property_enables_deviation,
};

// The key-agreement suite `PkiAuthenticationPlugin::set_preferred_kx_suite`
// takes. Re-exported so a caller configuring the plugin from a
// `dds.sec.auth.key_establishment_algorithm` property (DDS-Security 1.2
// §10.3.1 Table 50) does not need a second dependency just to name the type.
pub use zerodds_security_keyexchange::KxSuite;

// Re-exports from `zerodds-security` for easier user import.
pub use zerodds_security::authentication::{
    HandshakeHandle, HandshakeStepOutcome, IdentityHandle, SharedSecretHandle,
};
pub use zerodds_security::token::{BinaryProperty, DataHolder, WireProperty};
