// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! PEM parsing + trust-anchor chain validation.

use alloc::string::String;
use alloc::vec::Vec;

use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, TrustAnchor};

/// One input for [`crate::PkiAuthenticationPlugin::validate_with_config`]:
/// identity certificate + associated CA (both PEM).
#[derive(Debug, Clone)]
pub struct IdentityConfig {
    /// PEM-encoded X.509 identity certificate (a single cert).
    pub identity_cert_pem: Vec<u8>,
    /// PEM-encoded CA bundle (may contain multiple trust anchors).
    pub identity_ca_pem: Vec<u8>,
    /// PKCS8-PEM-encoded private key matching the identity cert.
    /// Used for signing handshake tokens and delegation links.
    /// `None` = validation-only mode (no handshake sign
    /// possible).
    pub identity_key_pem: Option<Vec<u8>>,
}

/// Internal errors of the PKI backend. Wrapped in
/// [`zerodds_security::SecurityError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PkiError {
    /// PEM parsing failed.
    InvalidPem(String),
    /// PEM contained **no** certificates.
    NoCertInPem,
    /// Cert chain verification failed (signature, expiry, name).
    CertInvalid(String),
    /// Trust-anchor bundle empty.
    EmptyTrustAnchors,
}

impl core::fmt::Display for PkiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidPem(m) => write!(f, "invalid PEM: {m}"),
            Self::NoCertInPem => write!(f, "no certificate in PEM"),
            Self::CertInvalid(m) => write!(f, "certificate invalid: {m}"),
            Self::EmptyTrustAnchors => write!(f, "trust-anchor bundle is empty"),
        }
    }
}

impl std::error::Error for PkiError {}

/// Parsed representation of a validated identity.
#[derive(Debug, Clone)]
pub(crate) struct ParsedIdentity {
    /// Cert DER of the identity certificate. Used in C3.1 for signed
    /// handshake tokens (`c.id` property).
    pub cert_der: Vec<u8>,
    /// Trust-anchor DER list (self-contained, so the object
    /// stays movable).
    pub trust_anchors_der: Vec<Vec<u8>>,
    /// PKCS8 DER of the private key (extracted from PEM). `None` =
    /// validation-only, no sign.
    pub private_key_pkcs8_der: Option<Vec<u8>>,
    /// Detected cert key algorithm.
    pub key_algo: CertKeyAlgo,
}

/// Detected algorithm from the identity cert. Determines which
/// `c.dsign_algo` values and sign routines are used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertKeyAlgo {
    /// ECDSA P-256 with SHA-256 (rcgen default; spec standard).
    EcdsaP256Sha256,
    /// RSA-2048 PSS-SHA256 (legacy/interop).
    RsaPssSha256,
    /// Unknown — cert public-key algorithm not in the whitelist.
    Unknown,
}

impl ParsedIdentity {
    /// Parses the cert + CA bundle and verifies the signature chain.
    pub fn from_config(cfg: &IdentityConfig) -> Result<Self, PkiError> {
        let cert_der = first_cert_der(&cfg.identity_cert_pem)?;
        let trust_anchors_der = all_certs_der(&cfg.identity_ca_pem)?;
        if trust_anchors_der.is_empty() {
            return Err(PkiError::EmptyTrustAnchors);
        }
        verify_cert_chain(&cert_der, &trust_anchors_der)?;
        let key_algo = detect_cert_algo(&cert_der);
        let private_key_pkcs8_der = match cfg.identity_key_pem.as_deref() {
            Some(pem) => Some(parse_pkcs8_pem(pem)?),
            None => None,
        };
        Ok(Self {
            cert_der,
            trust_anchors_der,
            private_key_pkcs8_der,
            key_algo,
        })
    }

    /// Verifies a remote DER certificate against the stored
    /// trust anchors.
    pub fn verify_remote_der(&self, remote_cert_der: &[u8]) -> Result<(), PkiError> {
        verify_cert_chain(remote_cert_der, &self.trust_anchors_der)
    }
}

/// Extracts the DER of the **first** certificate from a PEM block.
///
/// # Errors
/// `PkiError::NoCertInPem` if no certificate is contained.
pub fn first_cert_der(pem: &[u8]) -> Result<Vec<u8>, PkiError> {
    let certs = all_certs_der(pem)?;
    certs.into_iter().next().ok_or(PkiError::NoCertInPem)
}

fn all_certs_der(pem: &[u8]) -> Result<Vec<Vec<u8>>, PkiError> {
    // `rustls-pki-types` >= 1.9 ships an integrated PEM parser
    // (RUSTSEC-2025-0134 → we removed rustls-pemfile).
    let mut out = Vec::new();
    for item in CertificateDer::pem_slice_iter(pem) {
        let cert = item.map_err(|e| PkiError::InvalidPem(alloc::format!("{e:?}")))?;
        out.push(cert.as_ref().to_vec());
    }
    Ok(out)
}

fn verify_cert_chain(end_entity_der: &[u8], trust_anchors_der: &[Vec<u8>]) -> Result<(), PkiError> {
    let ee = CertificateDer::from_slice(end_entity_der);
    let end_entity = webpki::EndEntityCert::try_from(&ee)
        .map_err(|e| PkiError::CertInvalid(alloc::format!("parse: {e:?}")))?;

    // Derive the TrustAnchors from the DER bytes. CertificateDer must
    // live long enough for the anchor derived from it to be valid —
    // therefore first materialize all CertificateDer wrappers, then
    // the TrustAnchors on top of them.
    let ta_certs: Vec<CertificateDer<'_>> = trust_anchors_der
        .iter()
        .map(|b| CertificateDer::from_slice(b))
        .collect();
    let mut anchors: Vec<TrustAnchor<'_>> = Vec::with_capacity(ta_certs.len());
    for ta_cert in &ta_certs {
        let ta = webpki::anchor_from_trusted_cert(ta_cert)
            .map_err(|e| PkiError::CertInvalid(alloc::format!("trust-anchor: {e:?}")))?;
        anchors.push(ta);
    }

    // Current time (no no_std here — `std` feature assumed).
    let now = rustls_pki_types::UnixTime::now();

    // Accepted signature algorithms: the ring default set.
    let algs = webpki::ALL_VERIFICATION_ALGS;

    // No intermediate certs in the default path — the identity cert is directly
    // CA-signed (sub-CA setups run over the delegation-chain path
    // in the `delegation` module plus `security-permissions::delegation_check`).
    end_entity
        .verify_for_usage(
            algs,
            &anchors,
            &[],
            now,
            webpki::KeyUsage::client_auth(),
            None,
            None,
        )
        .map_err(|e| PkiError::CertInvalid(alloc::format!("verify: {e:?}")))?;

    Ok(())
}

/// Crate-internal re-export so that `plugin.rs` can run the OID detection on
/// peer cert DER again (no need for its own helper).
pub(crate) fn detect_cert_algo_pub(der: &[u8]) -> CertKeyAlgo {
    detect_cert_algo(der)
}

/// Detects the public-key algorithm from the cert DER. This is
/// a pragmatic SPKI match — we look for OID bytes in the
/// first 200 bytes of the DER stream.
///
/// * 1.2.840.10045.2.1 (id-ecPublicKey) + 1.2.840.10045.3.1.7 (P-256) → ECDSA P-256.
/// * 1.2.840.113549.1.1.1 (rsaEncryption) → RSA.
fn detect_cert_algo(der: &[u8]) -> CertKeyAlgo {
    // OID encoded as DER: 06 LL ...
    // ECDSA P-256 SPKI has: 1.2.840.10045.2.1 = 06 07 2A 86 48 CE 3D 02 01
    // and params 1.2.840.10045.3.1.7 = 06 08 2A 86 48 CE 3D 03 01 07
    const ECDSA_OID: &[u8] = &[0x06, 0x07, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];
    const P256_OID: &[u8] = &[0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
    const RSA_OID: &[u8] = &[
        0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01,
    ];
    if contains_subseq(der, ECDSA_OID) && contains_subseq(der, P256_OID) {
        CertKeyAlgo::EcdsaP256Sha256
    } else if contains_subseq(der, RSA_OID) {
        CertKeyAlgo::RsaPssSha256
    } else {
        CertKeyAlgo::Unknown
    }
}

fn contains_subseq(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn parse_pkcs8_pem(pem: &[u8]) -> Result<Vec<u8>, PkiError> {
    use rustls_pki_types::PrivatePkcs8KeyDer;
    PrivatePkcs8KeyDer::pem_slice_iter(pem)
        .next()
        .ok_or(PkiError::NoCertInPem)?
        .map(|k| k.secret_pkcs8_der().to_vec())
        .map_err(|e| PkiError::InvalidPem(alloc::format!("{e:?}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod mutation_killers {
    use super::*;

    /// Display format of all error variants. Catches mutation
    /// `replace fmt with Ok(Default::default())`.
    #[test]
    fn pki_error_display_messages_are_specific() {
        assert_eq!(
            alloc::format!(
                "{}",
                PkiError::InvalidPem(alloc::string::String::from("bad"))
            ),
            "invalid PEM: bad"
        );
        assert_eq!(
            alloc::format!("{}", PkiError::NoCertInPem),
            "no certificate in PEM"
        );
        assert_eq!(
            alloc::format!(
                "{}",
                PkiError::CertInvalid(alloc::string::String::from("expired"))
            ),
            "certificate invalid: expired"
        );
        assert_eq!(
            alloc::format!("{}", PkiError::EmptyTrustAnchors),
            "trust-anchor bundle is empty"
        );
    }

    /// `detect_cert_algo` needs BOTH OIDs (id-ecPublicKey AND the P-256
    /// curve), not just one. Catches the `&&` -> `||` mutation.
    #[test]
    fn detect_cert_algo_requires_both_ecdsa_oids() {
        let only_ecdsa = [0x06, 0x07, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];
        assert_eq!(detect_cert_algo(&only_ecdsa), CertKeyAlgo::Unknown);

        let only_p256 = [0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
        assert_eq!(detect_cert_algo(&only_p256), CertKeyAlgo::Unknown);

        let mut both = Vec::new();
        both.extend_from_slice(&only_ecdsa);
        both.extend_from_slice(&only_p256);
        assert_eq!(detect_cert_algo(&both), CertKeyAlgo::EcdsaP256Sha256);
    }

    /// `contains_subseq` is NOT constant `true`. Catches the
    /// `replace contains_subseq -> bool with true` mutation.
    #[test]
    fn contains_subseq_not_constant_true() {
        assert!(!contains_subseq(b"abc", b"xyz"));
        assert!(!contains_subseq(b"", b"x"));
        assert!(!contains_subseq(b"a", b"abc"));
    }

    /// `contains_subseq` empty needle: `needle.is_empty() || hay<needle.len()`
    /// must return false for an empty needle. Catches `||` -> `&&`.
    #[test]
    fn contains_subseq_empty_needle_returns_false() {
        assert!(!contains_subseq(b"abc", b""));
        assert!(!contains_subseq(b"", b""));
    }

    /// `contains_subseq` hay==needle must return true.
    /// Catches `<` -> `==` and `<` -> `<=` on the length pre-check branch.
    #[test]
    fn contains_subseq_exact_match_at_equal_length() {
        assert!(contains_subseq(b"abc", b"abc"));
        assert!(contains_subseq(b"\x06\x07\x2A", b"\x06\x07\x2A"));
    }

    /// `contains_subseq` without a match returns false. Catches the `==` -> `!=`
    /// mutation in the windows.any() check.
    #[test]
    fn contains_subseq_no_match_returns_false() {
        assert!(!contains_subseq(b"abcde", b"xyz"));
        assert!(!contains_subseq(b"\x01\x02\x03\x04", b"\x05\x06"));
    }
}
