// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! PEM parsing + trust-anchor chain validation.
//!
//! zerodds-lint: allow no_dyn_in_safe — the identity's signer is chosen at
//! run time, the PKCS#8 key in memory or a key held on a PKCS#11 token
//! (`IdentitySigner`), and the token's implementation lives in another crate.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, TrustAnchor};

use crate::x509v1_compat::{
    CertAcceptance, CertPolicy, PROP_ACCEPT_X509V1_IDENTITY, SPEC_CLAUSE_IDENTITY_CERTIFICATE,
    certificate_version, verify_v1_chain,
};

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
    /// PEM contained **no** private key in a form this crate reads
    /// (PKCS#8, PKCS#1 or SEC1).
    NoPrivateKeyInPem,
    /// Cert chain verification failed (signature, expiry, name).
    CertInvalid(String),
    /// Trust-anchor bundle empty.
    EmptyTrustAnchors,
    /// The identity certificate is not X.509 v3 and the
    /// [`PROP_ACCEPT_X509V1_IDENTITY`] deviation is not switched on.
    /// Carries the version the certificate actually has (`v1`/`v2`).
    CertVersionNotV3(String),
    /// A token-backed [`IdentitySigner`] refused to produce the handshake
    /// signature (DDS-Security 1.2 §9.3.2.5).
    SigningFailed(String),
}

impl core::fmt::Display for PkiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidPem(m) => write!(f, "invalid PEM: {m}"),
            Self::NoCertInPem => write!(f, "no certificate in PEM"),
            Self::NoPrivateKeyInPem => write!(f, "no private key in PEM"),
            Self::CertInvalid(m) => write!(f, "certificate invalid: {m}"),
            Self::EmptyTrustAnchors => write!(f, "trust-anchor bundle is empty"),
            Self::CertVersionNotV3(version) => write!(
                f,
                "certificate invalid: X.509 {version} identity certificate rejected \
                 — {SPEC_CLAUSE_IDENTITY_CERTIFICATE} requires an X.509 v3 certificate; set the property \
                 '{PROP_ACCEPT_X509V1_IDENTITY}' to accept it anyway"
            ),
            Self::SigningFailed(m) => write!(f, "signing failed: {m}"),
        }
    }
}

impl std::error::Error for PkiError {}

/// Signs the authentication-handshake tokens with a private key that never
/// enters this process as PKCS#8 bytes.
///
/// The in-memory path ([`IdentityConfig::identity_key_pem`]) parses a
/// PKCS#8 key and signs locally; a PKCS#11 token keeps the key on the
/// token and signs through `C_Sign` (RFC 7512, DDS-Security 1.2 §9.3.1
/// Table 44 `dds.sec.auth.private_key`). Both must return the signature
/// in the encoding the peer's verification expects (§9.3.2.5): ASN.1/DER
/// ECDSA, or the RSA-PSS block for an RSA key.
///
/// Implementations are `Send + Sync`: the plugin is shared between the
/// handshake driver and the crypto gate.
pub trait IdentitySigner: Send + Sync {
    /// Signs `msg` and returns the signature bytes.
    ///
    /// # Errors
    /// [`PkiError::SigningFailed`] when the token or the key refuses.
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, PkiError>;
}

/// Parsed representation of a validated identity.
#[derive(Clone)]
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
    /// Token-backed signer, when the private key stays on a PKCS#11 token
    /// instead of entering this process as PKCS#8 bytes. Mutually
    /// exclusive with `private_key_pkcs8_der`.
    pub signer: Option<Arc<dyn IdentitySigner>>,
    /// Detected cert key algorithm.
    pub key_algo: CertKeyAlgo,
    /// Validation policy this identity was accepted under. Applied again
    /// to every remote certificate that arrives on this identity's
    /// handshake, so a peer cannot get a laxer check than the local
    /// participant configured for itself.
    pub policy: CertPolicy,
    /// Which of the two validation paths accepted the local certificate.
    pub acceptance: CertAcceptance,
    /// GUID of the participant this identity belongs to — the
    /// `participant` field of the security-log event when the deviation
    /// path accepts a certificate.
    pub participant_guid: [u8; 16],
    /// The OCSP response that showed the local certificate `good`, sent as
    /// `ocsp_status` of the handshake tokens (DDS-Security 1.2 §10.3.2.3
    /// Tables 56 to 58, optional). `None` when no responder was asked.
    pub ocsp_staple: Option<Vec<u8>>,
    /// Set when the status of the local certificate could not be determined
    /// and the policy let it pass.
    pub ocsp_note: Option<String>,
    /// The DDS Domain Id `validate_local_identity` was called with
    /// (DDS-Security 1.2 §9.3.2.11.2); `None` for an identity made through
    /// an interface that does not take one.
    pub domain_id: Option<u32>,
}

impl core::fmt::Debug for ParsedIdentity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The signer is a trait object and the key material is secret;
        // print lengths and a marker, never the bytes.
        f.debug_struct("ParsedIdentity")
            .field("cert_der_len", &self.cert_der.len())
            .field("trust_anchors", &self.trust_anchors_der.len())
            .field(
                "private_key_pkcs8_der_len",
                &self.private_key_pkcs8_der.as_ref().map(Vec::len),
            )
            .field("token_signer", &self.signer.is_some())
            .field("key_algo", &self.key_algo)
            .field("policy", &self.policy)
            .field("acceptance", &self.acceptance)
            .field("participant_guid", &self.participant_guid)
            .field("domain_id", &self.domain_id)
            .finish()
    }
}

/// Detected algorithm from the identity cert. Determines which
/// `c.dsign_algo` values and sign routines are used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertKeyAlgo {
    /// ECDSA P-256 with SHA-256 (rcgen default; spec standard).
    EcdsaP256Sha256,
    /// ECDSA P-384 with SHA-384 (DDS-Security 1.2 §8.2 Table 25,
    /// `ECDSA+P384+SHA384`).
    EcdsaP384Sha384,
    /// An RSA key. The handshake signs it with RSASSA-PSS (`RSASSA-PSS-SHA256`),
    /// or with RSASSA-PKCS1-v1_5 when the peer verifies only that one; the
    /// scheme is a property of the handshake, not of the key.
    RsaPssSha256,
    /// Unknown — cert public-key algorithm not in the whitelist.
    Unknown,
}

impl ParsedIdentity {
    /// Parses the cert + CA bundle and verifies the signature chain under
    /// `policy`. [`CertPolicy::spec`] is the DDS-Security 1.2 behaviour.
    pub fn from_config(
        cfg: &IdentityConfig,
        policy: CertPolicy,
        participant_guid: [u8; 16],
    ) -> Result<Self, PkiError> {
        let cert_der = first_cert_der(&cfg.identity_cert_pem)?;
        let trust_anchors_der = all_certs_der(&cfg.identity_ca_pem)?;
        if trust_anchors_der.is_empty() {
            return Err(PkiError::EmptyTrustAnchors);
        }
        let (acceptance, outcome) =
            verify_cert_chain_checked(&cert_der, &trust_anchors_der, &policy, None)?;
        let (ocsp_staple, ocsp_note) = match outcome {
            crate::ocsp::OcspOutcome::Good(response) => (Some(response), None),
            crate::ocsp::OcspOutcome::AcceptedUndetermined(why) => (None, Some(why)),
            crate::ocsp::OcspOutcome::NotChecked => (None, None),
        };
        let key_algo = detect_cert_algo(&cert_der);
        let private_key_pkcs8_der = match cfg.identity_key_pem.as_deref() {
            Some(pem) => Some(private_key_pkcs8_der(pem)?),
            None => None,
        };
        Ok(Self {
            cert_der,
            trust_anchors_der,
            private_key_pkcs8_der,
            signer: None,
            key_algo,
            policy,
            acceptance,
            participant_guid,
            ocsp_staple,
            ocsp_note,
            domain_id: None,
        })
    }

    /// [`ParsedIdentity::from_config`] with a token-backed
    /// [`IdentitySigner`] instead of PKCS#8 bytes.
    ///
    /// `cfg.identity_key_pem` must be `None`: the private key stays on the
    /// token, so there is nothing to parse here. The cert is still
    /// validated against the CA exactly as in `from_config`, and its
    /// public-key algorithm is detected from the certificate, so the
    /// signer and the peer agree on `c.dsign_algo`.
    pub fn from_config_with_signer(
        cfg: &IdentityConfig,
        policy: CertPolicy,
        participant_guid: [u8; 16],
        signer: Arc<dyn IdentitySigner>,
    ) -> Result<Self, PkiError> {
        let mut parsed = Self::from_config(cfg, policy, participant_guid)?;
        parsed.signer = Some(signer);
        Ok(parsed)
    }

    /// The check of a remote certificate that is run again while the
    /// participant is authenticated: chain, validity and the configured CRLs
    /// (DDS-Security 1.2 §9.3.2.12, "identity certificates can expire"). No
    /// OCSP query: it would block the periodic tick.
    pub fn recheck_remote_der(&self, remote_cert_der: &[u8]) -> Result<(), PkiError> {
        verify_cert_chain(remote_cert_der, &self.trust_anchors_der, &self.policy).map(|_| ())
    }

    /// Verifies a remote DER certificate against the stored trust anchors,
    /// under the policy the local identity was accepted with, and does the
    /// OCSP check of DDS-Security 1.2 §10.3.3 Table 59: the `ocsp_status` of the token, when it has one, else
    /// the responder the certificate names.
    pub fn verify_remote_der_checked(
        &self,
        remote_cert_der: &[u8],
        ocsp_status: &[u8],
    ) -> Result<(CertAcceptance, crate::ocsp::OcspOutcome), PkiError> {
        verify_cert_chain_checked(
            remote_cert_der,
            &self.trust_anchors_der,
            &self.policy,
            Some(ocsp_status),
        )
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

/// The certificate revocation lists of `material`, DER (RFC 5280 §5): the
/// `X509 CRL` blocks of a PEM text, or one DER list.
///
/// # Errors
/// `PkiError::InvalidPem` for a PEM text without a readable CRL block.
pub fn crls_der(material: &[u8]) -> Result<Vec<Vec<u8>>, PkiError> {
    use rustls_pki_types::pem::PemObject;
    if !material.starts_with(b"-----BEGIN") {
        return Ok(alloc::vec![material.to_vec()]);
    }
    let mut out = Vec::new();
    for item in rustls_pki_types::CertificateRevocationListDer::pem_slice_iter(material) {
        let crl = item.map_err(|e| PkiError::InvalidPem(alloc::format!("{e:?}")))?;
        out.push(crl.as_ref().to_vec());
    }
    if out.is_empty() {
        return Err(PkiError::InvalidPem("no X509 CRL block".into()));
    }
    Ok(out)
}

/// Detects the public-key algorithm of a certificate.
///
/// Exposed for callers that hold the certificate but not a parsed
/// identity and have to pick a signing mechanism from it — a PKCS#11
/// token signer (DDS-Security 1.2 §9.3.1 Table 44 `private_key`), for
/// instance.
#[must_use]
pub fn cert_key_algo(cert_der: &[u8]) -> CertKeyAlgo {
    detect_cert_algo(cert_der)
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

/// [`verify_cert_chain`], then the OCSP part of DDS-Security 1.2 §10.3.3
/// Table 59 ([`crate::ocsp::check_identity`]) for the certificates it
/// accepted. A CRL (`policy.crls_der`) was checked by the chain validation.
pub(crate) fn verify_cert_chain_checked(
    end_entity_der: &[u8],
    trust_anchors_der: &[Vec<u8>],
    policy: &CertPolicy,
    ocsp_status: Option<&[u8]>,
) -> Result<(CertAcceptance, crate::ocsp::OcspOutcome), PkiError> {
    let acceptance = verify_cert_chain(end_entity_der, trust_anchors_der, policy)?;
    let now = rustls_pki_types::UnixTime::now().as_secs();
    let outcome = crate::ocsp::check_identity(
        end_entity_der,
        trust_anchors_der,
        ocsp_status,
        !policy.crls_der.is_empty(),
        &policy.ocsp,
        now,
    )
    .map_err(PkiError::CertInvalid)?;
    Ok((acceptance, outcome))
}

/// Validates an end-entity certificate against the identity-CA anchors.
///
/// Two paths, and which one runs is decided by the certificate's version
/// and by `policy` alone:
///
/// * **X.509 v3** — `rustls-webpki`, always, in both modes. This is the
///   DDS-Security 1.2 §10.3.1.3 path and `policy` does not alter it.
/// * **X.509 v1** — refused outright unless
///   `policy.accept_x509v1_identity_certificate` is on; then
///   [`verify_v1_chain`], which relaxes the version check and nothing
///   else. See `crate::x509v1_compat`.
///
/// X.509 v2 is refused in both modes: the deviation exists for the
/// certificates the OMG interoperability suite ships, and those are v1.
pub(crate) fn verify_cert_chain(
    end_entity_der: &[u8],
    trust_anchors_der: &[Vec<u8>],
    policy: &CertPolicy,
) -> Result<CertAcceptance, PkiError> {
    // The version is read first and from the certificate itself, so the
    // refusal below can name it. webpki's own answer is
    // `UnsupportedCertVersion`, which does not say which version it found.
    let version = certificate_version(end_entity_der)?;
    if !version.is_v3() {
        if !(version.is_v1() && policy.accept_x509v1_identity_certificate) {
            return Err(PkiError::CertVersionNotV3(version.label()));
        }
        // The v1 path validates without webpki, which checks the CRLs; a
        // revocation list it would pass over is refused instead.
        if !policy.crls_der.is_empty() {
            return Err(PkiError::CertInvalid(
                "a CRL is configured, and an X.509 v1 certificate is not checked against it".into(),
            ));
        }
        let now = rustls_pki_types::UnixTime::now().as_secs();
        let subject = verify_v1_chain(end_entity_der, trust_anchors_der, policy, now)?;
        return Ok(CertAcceptance::X509V1Deviation { subject });
    }

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
    // DDS-Security 1.2 §10.3.3 Table 59: "The operation shall check a CRL
    // and/or an OCSP (RFC 2560) responder." webpki checks the signature of a
    // CRL against the issuer, its nextUpdate and the serial of the
    // certificate; one of another CA has no say on it.
    let crls = policy
        .crls_der
        .iter()
        .map(|der| {
            webpki::OwnedCertRevocationList::from_der(der)
                .map(webpki::CertRevocationList::from)
                .map_err(|e| PkiError::CertInvalid(alloc::format!("crl: {e:?}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let crl_refs: Vec<&webpki::CertRevocationList<'_>> = crls.iter().collect();
    let revocation = webpki::RevocationOptionsBuilder::new(&crl_refs)
        .ok()
        .map(|builder| {
            builder
                .with_depth(webpki::RevocationCheckDepth::EndEntity)
                .with_status_policy(webpki::UnknownStatusPolicy::Allow)
                .with_expiration_policy(webpki::ExpirationPolicy::Enforce)
                .build()
        });
    end_entity
        .verify_for_usage(
            algs,
            &anchors,
            &[],
            now,
            webpki::KeyUsage::client_auth(),
            revocation,
            None,
        )
        .map_err(|e| PkiError::CertInvalid(alloc::format!("verify: {e:?}")))?;

    // A v3 certificate carries a subject expectation through the same gate
    // the v1 path uses, so the two modes do not differ in what they check
    // beyond the version. webpki's `verify_for_usage` checks no subject
    // name at all, so without this the v3 path would be the laxer of the
    // two.
    if let Some(expected) = policy.expected_subject.as_deref() {
        let subject = crate::identity_token::subject_of_der(end_entity_der)?;
        if !crate::identity_token::subject_match(&subject, expected) {
            return Err(PkiError::CertInvalid(alloc::format!(
                "subject '{subject}' is not the expected '{expected}'"
            )));
        }
    }

    Ok(CertAcceptance::SpecV3)
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
    // secp384r1: 1.3.132.0.34 = 06 05 2B 81 04 00 22
    const P384_OID: &[u8] = &[0x06, 0x05, 0x2B, 0x81, 0x04, 0x00, 0x22];
    const RSA_OID: &[u8] = &[
        0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01,
    ];
    if contains_subseq(der, ECDSA_OID) && contains_subseq(der, P256_OID) {
        CertKeyAlgo::EcdsaP256Sha256
    } else if contains_subseq(der, ECDSA_OID) && contains_subseq(der, P384_OID) {
        CertKeyAlgo::EcdsaP384Sha384
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

/// The first private key in a PEM file, as PKCS#8 DER.
///
/// DDS-Security 1.2 §10.3.1 Table 50 names the private key by URI and
/// says nothing about its form, and the field has three: PKCS#8
/// (`BEGIN PRIVATE KEY`), PKCS#1 (`BEGIN RSA PRIVATE KEY`) and SEC1
/// (`BEGIN EC PRIVATE KEY`). Every vendor key of the OMG `dds-security`
/// interoperability suite is PKCS#1, and `openssl ecparam -genkey` writes
/// SEC1; the signing backends take PKCS#8 only. The two older forms are
/// wrapped here, not refused: a wrapper is a byte-exact re-framing of
/// the same key (RFC 5208 §5, RFC 5915 §3), so nothing about the key
/// changes.
///
/// # Errors
/// `PkiError::NoPrivateKeyInPem` when the file holds no key block;
/// `PkiError::InvalidPem` when the block does not parse.
pub fn private_key_pkcs8_der(pem: &[u8]) -> Result<Vec<u8>, PkiError> {
    use rustls_pki_types::PrivateKeyDer;
    let key = PrivateKeyDer::pem_slice_iter(pem)
        .next()
        .ok_or(PkiError::NoPrivateKeyInPem)?
        .map_err(|e| PkiError::InvalidPem(alloc::format!("{e:?}")))?;
    match key {
        PrivateKeyDer::Pkcs8(k) => Ok(k.secret_pkcs8_der().to_vec()),
        PrivateKeyDer::Pkcs1(k) => pkcs1_as_pkcs8(k.secret_pkcs1_der()),
        PrivateKeyDer::Sec1(k) => sec1_as_pkcs8(k.secret_sec1_der()),
        _ => Err(PkiError::InvalidPem(alloc::string::String::from(
            "a private-key form this crate does not read",
        ))),
    }
}

/// The decrypted body of an OpenSSL-encrypted PEM block, as PKCS#8 DER:
/// the block's label says whether the body is PKCS#1, SEC1 or already
/// PKCS#8.
pub(crate) fn sec1_or_pkcs1_as_pkcs8(label: &str, der: &[u8]) -> Result<Vec<u8>, PkiError> {
    match label {
        "RSA PRIVATE KEY" => pkcs1_as_pkcs8(der),
        "EC PRIVATE KEY" => sec1_as_pkcs8(der),
        "PRIVATE KEY" => Ok(der.to_vec()),
        other => Err(PkiError::InvalidPem(alloc::format!(
            "an encrypted `{other}` block, which is not a private-key form this crate reads"
        ))),
    }
}

/// RFC 5208 §5: `PrivateKeyInfo { version 0, rsaEncryption NULL,
/// privateKey = the PKCS#1 RSAPrivateKey }`.
fn pkcs1_as_pkcs8(pkcs1: &[u8]) -> Result<Vec<u8>, PkiError> {
    use pkcs8::der::Encode as _;
    use pkcs8::der::asn1::AnyRef;
    const RSA_ENCRYPTION: pkcs8::ObjectIdentifier =
        pkcs8::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");
    let info = pkcs8::PrivateKeyInfo::new(
        pkcs8::AlgorithmIdentifierRef {
            oid: RSA_ENCRYPTION,
            parameters: Some(AnyRef::NULL),
        },
        pkcs1,
    );
    info.to_der()
        .map_err(|e| PkiError::InvalidPem(alloc::format!("PKCS#1 key: {e}")))
}

/// RFC 5915 §3: `PrivateKeyInfo { version 0, id-ecPublicKey namedCurve,
/// privateKey = the ECPrivateKey }`. The curve comes out of the SEC1
/// structure's own `parameters`; a key that names none cannot be
/// wrapped, because the algorithm identifier has to name it.
fn sec1_as_pkcs8(sec1_der: &[u8]) -> Result<Vec<u8>, PkiError> {
    use pkcs8::der::Encode as _;
    use pkcs8::der::asn1::AnyRef;
    use sec1::der::Decode as _;
    const ID_EC_PUBLIC_KEY: pkcs8::ObjectIdentifier =
        pkcs8::ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
    let key = sec1::EcPrivateKey::from_der(sec1_der)
        .map_err(|e| PkiError::InvalidPem(alloc::format!("SEC1 key: {e}")))?;
    let Some(sec1::EcParameters::NamedCurve(curve)) = key.parameters else {
        return Err(PkiError::InvalidPem(alloc::string::String::from(
            "SEC1 key names no curve, so no PKCS#8 algorithm identifier can be built for it",
        )));
    };
    let info = pkcs8::PrivateKeyInfo::new(
        pkcs8::AlgorithmIdentifierRef {
            oid: ID_EC_PUBLIC_KEY,
            parameters: Some(AnyRef::from(&curve)),
        },
        sec1_der,
    );
    info.to_der()
        .map_err(|e| PkiError::InvalidPem(alloc::format!("SEC1 key: {e}")))
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod crl_tests {
    use super::*;
    use rcgen::{
        BasicConstraints, CertificateParams, CertificateRevocationListParams, DnType, IsCa,
        KeyIdMethod, KeyPair, KeyUsagePurpose, RevocationReason, RevokedCertParams, SerialNumber,
        date_time_ymd,
    };

    struct Pki {
        ca: rcgen::Certificate,
        ca_key: KeyPair,
    }

    fn ca(name: &str) -> Pki {
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params.distinguished_name.push(DnType::CommonName, name);
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = alloc::vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let ca_key = KeyPair::generate().unwrap();
        let ca = params.self_signed(&ca_key).unwrap();
        Pki { ca, ca_key }
    }

    fn leaf(pki: &Pki, name: &str, serial: u8) -> Vec<u8> {
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params.distinguished_name.push(DnType::CommonName, name);
        params.serial_number = Some(SerialNumber::from(alloc::vec![serial]));
        let key = KeyPair::generate().unwrap();
        params
            .signed_by(&key, &pki.ca, &pki.ca_key)
            .unwrap()
            .der()
            .to_vec()
    }

    fn crl(pki: &Pki, revoked: &[u8], next_update_year: i32) -> Vec<u8> {
        CertificateRevocationListParams {
            this_update: date_time_ymd(2024, 1, 1),
            next_update: date_time_ymd(next_update_year, 1, 1),
            crl_number: SerialNumber::from(1u64),
            issuing_distribution_point: None,
            revoked_certs: revoked
                .iter()
                .map(|serial| RevokedCertParams {
                    serial_number: SerialNumber::from(alloc::vec![*serial]),
                    revocation_time: date_time_ymd(2024, 6, 1),
                    reason_code: Some(RevocationReason::KeyCompromise),
                    invalidity_date: None,
                })
                .collect(),
            key_identifier_method: KeyIdMethod::Sha256,
        }
        .signed_by(&pki.ca, &pki.ca_key)
        .unwrap()
        .der()
        .to_vec()
    }

    fn policy(crls_der: Vec<Vec<u8>>) -> CertPolicy {
        CertPolicy {
            crls_der,
            ..CertPolicy::spec()
        }
    }

    /// DDS-Security 1.2 §10.3.3 Table 59: "The operation shall check a CRL
    /// and/or an OCSP (RFC 2560) responder." A certificate the CRL of its CA
    /// lists is refused, one it does not list is taken. A revoked
    /// certificate was taken.
    #[test]
    fn a_certificate_the_crl_lists_is_refused() {
        let pki = ca("ZeroDDS Test CRL CA");
        let anchors = alloc::vec![pki.ca.der().to_vec()];
        let alice = leaf(&pki, "alice", 0x0A);
        let bob = leaf(&pki, "bob", 0x0B);
        let list = crl(&pki, &[0x0B], 2100);
        assert!(verify_cert_chain(&alice, &anchors, &policy(alloc::vec![list.clone()])).is_ok());
        let err = verify_cert_chain(&bob, &anchors, &policy(alloc::vec![list])).unwrap_err();
        assert!(alloc::format!("{err}").contains("Revoked"), "{err}");
        assert!(verify_cert_chain(&bob, &anchors, &CertPolicy::spec()).is_ok());
    }

    /// An expired CRL (its `nextUpdate` passed) is refused; one of another
    /// CA has no say on the certificate.
    #[test]
    fn an_expired_crl_is_refused_and_one_of_another_ca_has_no_say() {
        let pki = ca("ZeroDDS Test CRL CA");
        let other = ca("ZeroDDS Other CA");
        let anchors = alloc::vec![pki.ca.der().to_vec()];
        let alice = leaf(&pki, "alice", 0x0A);
        assert!(
            verify_cert_chain(&alice, &anchors, &policy(alloc::vec![crl(&pki, &[], 2025)]))
                .is_err(),
            "expired"
        );
        assert!(
            verify_cert_chain(
                &alice,
                &anchors,
                &policy(alloc::vec![crl(&other, &[0x0A], 2100)])
            )
            .is_ok()
        );
    }

    /// The PEM form carries one or more `X509 CRL` blocks.
    #[test]
    fn crls_are_read_from_pem_and_der() {
        let pki = ca("ZeroDDS Test CRL CA");
        let der = crl(&pki, &[0x0B], 2100);
        assert_eq!(crls_der(&der).unwrap(), alloc::vec![der.clone()]);
        let b64 = {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(&der)
        };
        let text = alloc::format!("-----BEGIN X509 CRL-----\n{b64}\n-----END X509 CRL-----\n");
        assert_eq!(crls_der(text.as_bytes()).unwrap(), alloc::vec![der]);
        assert!(
            crls_der(b"-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----\n").is_err()
        );
    }
}
