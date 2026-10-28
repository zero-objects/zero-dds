// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! X.509 v1 identity certificates — an explicitly switched-on deviation
//! from DDS-Security 1.2 §10.3.1.3.
//!
//! # What the spec says
//!
//! DDS-Security 1.2 §10.3.1.3 "Identity Certificate": *"An X.509 **v3**
//! Certificate \[39\] that chains up to the Identity CA."* §9.4.1.2 repeats
//! it for the file-based configuration. `rustls-webpki` implements that
//! rule — `webpki::EndEntityCert::try_from` refuses anything but v3 with
//! `UnsupportedCertVersion` — and that refusal is this stack's default and
//! stays the default.
//!
//! # What the interoperability suite ships
//!
//! `github.com/omg-dds/dds-security` carries identity certificates for six
//! vendors. Measured with `openssl x509 -noout -text`, the identity
//! certificates of RTI Connext, eProsima Fast DDS and ADLINK Vortex — and
//! `TESTONLY_permissions_ca_cert.pem` — are `Version: 1 (0x0)`. Only the
//! identity CA is v3, and webpki has an explicit v1 path for trust
//! anchors, so the CAs are not the problem: the end-entity certificates
//! are. A strict implementation cannot join the suite at all, because the
//! refusal happens before a participant exists.
//!
//! # What this module does
//!
//! It is the second validation path, reached only when
//! [`PROP_ACCEPT_X509V1_IDENTITY`] is switched on. **The version check is
//! the only thing it relaxes.** It still verifies
//!
//! 1. the validity period (`notBefore <= now <= notAfter`),
//! 2. the subject name the caller expects, when the caller names one
//!    ([`CertPolicy::expected_subject`]),
//! 3. the chain: the certificate's `issuer` must be the DER-identical
//!    `subject` of a configured identity-CA anchor,
//! 4. the certificate signature against that same anchor's public key.
//!
//! Signature verification runs through `rustls-webpki`'s own
//! `SignatureVerificationAlgorithm` set — the same algorithm
//! implementations the v3 path uses, selected by matching the
//! certificate's `signatureAlgorithm` and the anchor's
//! `SubjectPublicKeyInfo.algorithm` against each algorithm's declared
//! identifiers. No signature code is written here, and no crypto backend
//! is named here: the provider follows this crate's backend feature.
//!
//! Two checks the v3 path performs are **not** performed here, and both
//! are named where they happen rather than left to be discovered:
//!
//! * the anchor's `nameConstraints` (RFC 5280 §4.2.1.10), which
//!   `webpki::anchor_from_trusted_cert` carries and `verify_for_usage`
//!   applies. Nothing here evaluates them, so an identity CA that
//!   carries the extension **refuses** this path instead of having its
//!   constraint quietly dropped;
//! * key usage — see below, and it is the certificate format's doing,
//!   not a choice.
//!
//! # What an X.509 v1 certificate cannot carry
//!
//! A v1 certificate has no `extensions` field at all, so `keyUsage`,
//! `extendedKeyUsage` and `basicConstraints` do not exist on it. The v3
//! path asks webpki for `KeyUsage::client_auth()`; that check has nothing
//! to read on a v1 certificate and is therefore not performed. That is a
//! property of the certificate format, not a choice made here, and it is
//! one of the reasons the deviation is off by default.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use x509_cert::Certificate;
use x509_cert::der::{Decode, Encode};

use crate::identity::PkiError;

/// Vendor property that switches the deviation on. Default: absent = off.
///
/// The name says what it is. `spec_deviation` is in it so that an operator
/// who meets the string in a configuration file or in a log line knows,
/// without looking anything up, that this leaves DDS-Security 1.2
/// §10.3.1.3 behind; `zerodds` is in it because it is a vendor extension
/// and not a name the spec reserves.
///
/// Accepted values are the usual booleans (`true`, `1`, `yes`, `on`,
/// case-insensitive) — see [`property_enables_deviation`]. Anything else,
/// a typo included, leaves the deviation off.
pub const PROP_ACCEPT_X509V1_IDENTITY: &str =
    "org.zerodds.sec.auth.accept_x509v1_identity_certificate";

/// `category` of the security-log event written for every accepted v1
/// identity certificate (DDS-Security 1.2 §9.6.2.2.2 `log`).
pub const LOG_CATEGORY_X509V1_DEVIATION: &str = "auth.identity.x509v1_spec_deviation";

/// The spec clause the deviation leaves, quoted in every log line and in
/// every rejection message so that neither needs a lookup.
pub const SPEC_CLAUSE_IDENTITY_CERTIFICATE: &str = "DDS-Security 1.2 §10.3.1.3";

/// Reads [`PROP_ACCEPT_X509V1_IDENTITY`] out of a property value.
///
/// Only an explicit affirmative switches the deviation on. An
/// unrecognised value is **not** an affirmative: the default is the spec
/// behaviour, and a value nobody can read must not leave it.
#[must_use]
pub fn property_enables_deviation(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Certificate-validation policy. [`Default`] is the spec behaviour.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CertPolicy {
    /// Accept X.509 **v1** identity certificates. Off by default; see the
    /// module documentation for what stays checked when it is on.
    pub accept_x509v1_identity_certificate: bool,
    /// Subject name the caller expects on the identity certificate, in the
    /// `XN_FLAG_ONELINE` spelling `crate::identity_token` produces
    /// (`CN = alice, O = Example`). Compared with
    /// [`crate::identity_token::subject_match`], which is whitespace- and
    /// case-insensitive. `None` = the caller names no expectation, which
    /// is what the v3 path does too.
    pub expected_subject: Option<String>,
    /// The certificate revocation lists of the identity CA, DER
    /// (RFC 5280 §5). DDS-Security 1.2 §10.3.3 Table 59: validate_local_identity
    /// and the handshake "shall check a CRL and/or an OCSP (RFC 2560)
    /// responder". A certificate one of them lists is refused; their
    /// signature against the CA and their `nextUpdate` are checked. Empty =
    /// none configured. The two CRL modules had no caller, and a revoked
    /// certificate was taken.
    pub crls_der: Vec<Vec<u8>>,
    /// The OCSP settings (DDS-Security 1.2 §10.3.3 Table 59): what happens
    /// when the status of a certificate cannot be determined.
    pub ocsp: crate::ocsp::OcspPolicy,
    /// How the algorithms are named in the IdentityToken and in the handshake
    /// (DDS-Security 1.2 Table 25, Table 26 and the names of 1.1).
    pub algorithm_naming: crate::handshake_token::AlgorithmNaming,
}

impl CertPolicy {
    /// The spec policy: v3 only, no subject expectation.
    #[must_use]
    pub fn spec() -> Self {
        Self::default()
    }
}

/// Which of the two paths accepted a certificate.
///
/// Returned rather than discarded, because the caller has to log a
/// [`CertAcceptance::X509V1Deviation`] — a deviation nobody is told about
/// is worse than no deviation at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertAcceptance {
    /// The spec path: X.509 v3, validated by `rustls-webpki`.
    SpecV3,
    /// The deviation path: X.509 v1, accepted only because
    /// [`PROP_ACCEPT_X509V1_IDENTITY`] is on. Carries the certificate's
    /// subject for the log line.
    X509V1Deviation {
        /// Subject of the accepted certificate, `XN_FLAG_ONELINE` style.
        subject: String,
    },
}

/// X.509 `version` field, as the DER encodes it: `0` = v1, `1` = v2,
/// `2` = v3. A v1 certificate omits the field entirely, so this cannot be
/// read as "0 means missing".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CertVersion(pub(crate) u8);

impl CertVersion {
    /// `true` for the version DDS-Security 1.2 §10.3.1.3 requires.
    pub(crate) fn is_v3(self) -> bool {
        self.0 == 2
    }

    /// `true` for the version the interoperability suite ships.
    pub(crate) fn is_v1(self) -> bool {
        self.0 == 0
    }

    /// `v1`/`v2`/`v3` — so a message does not make its reader decode a
    /// DER field number.
    pub(crate) fn label(self) -> String {
        format!("v{}", u16::from(self.0) + 1)
    }
}

/// Reads the `version` field out of a certificate DER.
///
/// # Errors
/// [`PkiError::CertInvalid`] if the DER does not parse as a certificate.
pub(crate) fn certificate_version(der: &[u8]) -> Result<CertVersion, PkiError> {
    let cert = parse_cert(der)?;
    Ok(CertVersion(match cert.tbs_certificate.version {
        x509_cert::Version::V1 => 0,
        x509_cert::Version::V2 => 1,
        x509_cert::Version::V3 => 2,
    }))
}

/// Validates an X.509 v1 end-entity certificate against the identity-CA
/// anchors, relaxing nothing but the version.
///
/// Returns the certificate's subject (`XN_FLAG_ONELINE` spelling) so the
/// caller can name it in the security-log event.
///
/// `now_unix_secs` is the current time in seconds since the Unix epoch —
/// passed in rather than read here, so the validity check is testable
/// against a fixed clock.
///
/// # Errors
/// [`PkiError::CertInvalid`], naming the check that failed: DER parse, the
/// validity window, the expected subject, a missing issuer match, an
/// unsupported signature/key algorithm pair, or a signature that does not
/// verify.
pub(crate) fn verify_v1_chain(
    end_entity_der: &[u8],
    trust_anchors_der: &[Vec<u8>],
    policy: &CertPolicy,
    now_unix_secs: u64,
) -> Result<String, PkiError> {
    let cert = parse_cert(end_entity_der)?;

    // (1) Validity window. Checked first, so an expired certificate is
    //     reported as expired rather than as whatever else it also is.
    let not_before = cert.tbs_certificate.validity.not_before.to_unix_duration();
    let not_after = cert.tbs_certificate.validity.not_after.to_unix_duration();
    if now_unix_secs < not_before.as_secs() {
        return Err(PkiError::CertInvalid(format!(
            "x509v1: certificate is not valid before unix {} (now {now_unix_secs})",
            not_before.as_secs()
        )));
    }
    if now_unix_secs > not_after.as_secs() {
        return Err(PkiError::CertInvalid(format!(
            "x509v1: certificate expired at unix {} (now {now_unix_secs})",
            not_after.as_secs()
        )));
    }

    // (2) The subject the caller expects, when it names one.
    let subject = crate::identity_token::subject_oneline(&cert.tbs_certificate.subject);
    if let Some(expected) = policy.expected_subject.as_deref() {
        if !crate::identity_token::subject_match(&subject, expected) {
            return Err(PkiError::CertInvalid(format!(
                "x509v1: subject '{subject}' is not the expected '{expected}'"
            )));
        }
    }

    // (3)+(4) Chain and signature. The certificate's `issuer` must be the
    //         DER-identical `subject` of one of the configured
    //         identity-CA anchors, and that same anchor's key must verify
    //         the signature. Both, on one anchor: an issuer match alone
    //         proves nothing, and a signature that verifies under an
    //         anchor whose name is not the issuer would accept a
    //         certificate naming a CA other than the one that signed it.
    let issuer_der = cert
        .tbs_certificate
        .issuer
        .to_der()
        .map_err(|e| PkiError::CertInvalid(format!("x509v1: issuer der: {e}")))?;
    let tbs_der = tbs_certificate_slice(end_entity_der)?;
    let signature_alg_der = cert
        .signature_algorithm
        .to_der()
        .map_err(|e| PkiError::CertInvalid(format!("x509v1: signature alg der: {e}")))?;
    let signature_alg_id = algorithm_identifier_contents(&signature_alg_der)?.to_vec();
    let signature = cert.signature.as_bytes().ok_or_else(|| {
        PkiError::CertInvalid("x509v1: signature is not a whole number of octets".to_string())
    })?;

    let mut matching_issuer = false;
    let mut last_error = String::new();
    for anchor_der in trust_anchors_der {
        let anchor = parse_cert(anchor_der)?;
        let anchor_subject_der = anchor
            .tbs_certificate
            .subject
            .to_der()
            .map_err(|e| PkiError::CertInvalid(format!("x509v1: anchor subject der: {e}")))?;
        if anchor_subject_der != issuer_der {
            continue;
        }
        matching_issuer = true;
        // The v3 path gets the anchor's `nameConstraints` enforced for
        // free: `webpki::anchor_from_trusted_cert` carries them into the
        // `TrustAnchor` and `verify_for_usage` applies them (RFC 5280
        // §4.2.1.10). Nothing here implements that, and skipping a
        // constraint the deployment configured would be relaxing more
        // than the version. So an anchor that carries one refuses the v1
        // path outright rather than accepting a certificate it may
        // forbid.
        if anchor_has_name_constraints(&anchor) {
            return Err(PkiError::CertInvalid(
                "x509v1: the identity CA carries a nameConstraints extension, \
                 which this path does not evaluate — refusing rather than \
                 ignoring a constraint the deployment configured"
                    .to_string(),
            ));
        }
        let spki = &anchor.tbs_certificate.subject_public_key_info;
        let key_alg_der = spki
            .algorithm
            .to_der()
            .map_err(|e| PkiError::CertInvalid(format!("x509v1: anchor spki alg der: {e}")))?;
        let key_alg_id = algorithm_identifier_contents(&key_alg_der)?;
        let Some(key_bytes) = spki.subject_public_key.as_bytes() else {
            // Not a "shorter answer" — a trust anchor whose public key is
            // not octet-aligned cannot verify anything, and saying so is
            // the only honest outcome.
            return Err(PkiError::CertInvalid(
                "x509v1: identity CA public key is not a whole number of octets".to_string(),
            ));
        };
        match verify_with_webpki_algs(&signature_alg_id, key_alg_id, key_bytes, tbs_der, signature)
        {
            Ok(()) => return Ok(subject),
            Err(e) => last_error = e,
        }
    }

    if matching_issuer {
        Err(PkiError::CertInvalid(format!(
            "x509v1: signature does not verify against the identity CA whose \
             subject is the certificate's issuer ({last_error})"
        )))
    } else {
        Err(PkiError::CertInvalid(
            "x509v1: no configured identity CA has the certificate's issuer as its subject"
                .to_string(),
        ))
    }
}

/// Verifies `signature` over `message` with the `rustls-webpki`
/// verification algorithm whose declared `signature_alg_id` and
/// `public_key_alg_id` are the certificate's and the anchor's.
///
/// zerodds-lint: allow no_dyn_in_safe
/// (`webpki::ALL_VERIFICATION_ALGS` is a slice of
/// `&dyn SignatureVerificationAlgorithm` — the same third-party API the
/// v3 path hands to `verify_for_usage`.)
fn verify_with_webpki_algs(
    signature_alg_der: &[u8],
    public_key_alg_der: &[u8],
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    let mut known_pair = false;
    for alg in webpki::ALL_VERIFICATION_ALGS {
        if alg.signature_alg_id().as_ref() != signature_alg_der
            || alg.public_key_alg_id().as_ref() != public_key_alg_der
        {
            continue;
        }
        known_pair = true;
        if alg.verify_signature(public_key, message, signature).is_ok() {
            return Ok(());
        }
    }
    if known_pair {
        Err("signature verification failed".to_string())
    } else {
        Err("no supported algorithm for this signature/public-key pair".to_string())
    }
}

/// Strips the outer `SEQUENCE` header off an encoded `AlgorithmIdentifier`.
///
/// `rustls_pki_types::AlgorithmIdentifier` — what
/// `SignatureVerificationAlgorithm::signature_alg_id` and
/// `public_key_alg_id` return — holds the **contents** of that SEQUENCE,
/// not the whole TLV. Comparing the two forms directly matches nothing,
/// which is a silent "algorithm unsupported" rather than a wrong answer,
/// but a refusal all the same.
///
/// # Errors
/// [`PkiError::CertInvalid`] if the encoding is not a `SEQUENCE`.
fn algorithm_identifier_contents(der: &[u8]) -> Result<&[u8], PkiError> {
    let malformed = || PkiError::CertInvalid("x509v1: malformed AlgorithmIdentifier".to_string());
    let (tag, header_len, content_len) = der_tlv_header(der).ok_or_else(malformed)?;
    if tag != DER_TAG_SEQUENCE {
        return Err(malformed());
    }
    der.get(header_len..header_len + content_len)
        .ok_or_else(malformed)
}

/// `true` if the certificate carries the RFC 5280 §4.2.1.10
/// `nameConstraints` extension (OID 2.5.29.30).
fn anchor_has_name_constraints(cert: &Certificate) -> bool {
    /// `id-ce-nameConstraints`.
    const NAME_CONSTRAINTS_OID: &str = "2.5.29.30";
    cert.tbs_certificate
        .extensions
        .as_ref()
        .is_some_and(|exts| {
            exts.iter()
                .any(|ext| ext.extn_id.to_string() == NAME_CONSTRAINTS_OID)
        })
}

fn parse_cert(der: &[u8]) -> Result<Certificate, PkiError> {
    Certificate::from_der(der).map_err(|e| PkiError::CertInvalid(format!("x509v1: parse: {e}")))
}

const DER_TAG_SEQUENCE: u8 = 0x30;

/// Returns the **raw** `tbsCertificate` TLV of a certificate DER — the
/// exact bytes the signature was computed over.
///
/// Sliced out of the input rather than re-encoded from the parsed
/// structure: a re-encode is byte-identical only when the issuer produced
/// canonical DER, and a signature check that silently depended on that
/// would fail on the one certificate that did not.
///
/// # Errors
/// [`PkiError::CertInvalid`] if the outer structure is not
/// `SEQUENCE { SEQUENCE, .. }`.
fn tbs_certificate_slice(cert_der: &[u8]) -> Result<&[u8], PkiError> {
    let malformed = || PkiError::CertInvalid("x509v1: malformed certificate DER".to_string());
    let (tag, header_len, content_len) = der_tlv_header(cert_der).ok_or_else(malformed)?;
    if tag != DER_TAG_SEQUENCE {
        return Err(malformed());
    }
    let body = cert_der
        .get(header_len..header_len + content_len)
        .ok_or_else(malformed)?;
    let (tbs_tag, tbs_header_len, tbs_content_len) = der_tlv_header(body).ok_or_else(malformed)?;
    if tbs_tag != DER_TAG_SEQUENCE {
        return Err(malformed());
    }
    body.get(..tbs_header_len + tbs_content_len)
        .ok_or_else(malformed)
}

/// Reads one DER TLV header at the start of `buf` into
/// `(tag, header_len, content_len)`.
///
/// Returns `None` for a truncated header, an indefinite length (not DER),
/// a length field wider than four octets, or a content run that does not
/// fit in `buf`. Every one of those is a malformed certificate, not a
/// shorter one, and the caller turns each into a refusal.
fn der_tlv_header(buf: &[u8]) -> Option<(u8, usize, usize)> {
    let tag = *buf.first()?;
    let first_len_octet = *buf.get(1)?;
    let (header_len, content_len) = if first_len_octet < 0x80 {
        (2usize, usize::from(first_len_octet))
    } else {
        let width = usize::from(first_len_octet & 0x7f);
        if width == 0 || width > 4 {
            return None;
        }
        let mut len = 0usize;
        for i in 0..width {
            len = (len << 8) | usize::from(*buf.get(2 + i)?);
        }
        (2 + width, len)
    };
    let end = header_len.checked_add(content_len)?;
    if buf.len() < end {
        return None;
    }
    Some((tag, header_len, content_len))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Only an explicit affirmative switches the deviation on. A typo
    /// leaves the spec behaviour in place instead of guessing at intent.
    #[test]
    fn only_an_explicit_affirmative_enables_the_deviation() {
        for on in ["true", "TRUE", " 1 ", "yes", "On"] {
            assert!(property_enables_deviation(on), "{on} must enable it");
        }
        for off in ["false", "0", "no", "", "ture", "enabled"] {
            assert!(!property_enables_deviation(off), "{off} must not enable it");
        }
    }

    /// The property name is a vendor extension under the vendor prefix
    /// and says in its own text that it accepts a certificate of version 1.
    #[test]
    fn the_property_name_reads_as_a_deviation() {
        assert_eq!(
            PROP_ACCEPT_X509V1_IDENTITY,
            "org.zerodds.sec.auth.accept_x509v1_identity_certificate"
        );
        // A name outside the specification starts with a reversed domain name
        // (§7.3.2) and says in its own text what it accepts.
        assert!(PROP_ACCEPT_X509V1_IDENTITY.starts_with("org.zerodds.sec.auth."));
        assert!(PROP_ACCEPT_X509V1_IDENTITY.contains("accept_x509v1"));
        assert_eq!(
            SPEC_CLAUSE_IDENTITY_CERTIFICATE,
            "DDS-Security 1.2 §10.3.1.3"
        );
    }

    /// The version labels are the ones a `openssl x509 -text` reader
    /// expects: DER `0` prints as `Version: 1`.
    #[test]
    fn der_version_numbers_map_onto_the_printed_labels() {
        assert_eq!(CertVersion(0).label(), "v1");
        assert_eq!(CertVersion(1).label(), "v2");
        assert_eq!(CertVersion(2).label(), "v3");
        assert!(CertVersion(0).is_v1());
        assert!(!CertVersion(0).is_v3());
        assert!(CertVersion(2).is_v3());
        assert!(!CertVersion(2).is_v1());
    }

    /// The TBS slice is the input's own bytes, header included, and stops
    /// where the `signatureAlgorithm` field starts.
    #[test]
    fn the_tbs_slice_is_the_first_inner_tlv_of_the_certificate() {
        // SEQUENCE { SEQUENCE { NULL }, NULL }
        let der = [0x30, 0x06, 0x30, 0x02, 0x05, 0x00, 0x05, 0x00];
        assert_eq!(
            tbs_certificate_slice(&der).expect("well formed"),
            &[0x30, 0x02, 0x05, 0x00]
        );
    }

    /// A malformed outer structure is refused, not silently shortened.
    #[test]
    fn a_malformed_der_is_refused_rather_than_truncated() {
        // Length says six octets; only two follow.
        assert!(tbs_certificate_slice(&[0x30, 0x06, 0x30, 0x02]).is_err());
        // Not a SEQUENCE at all.
        assert!(tbs_certificate_slice(&[0x02, 0x01, 0x00]).is_err());
        // Indefinite length is BER, not DER.
        assert!(der_tlv_header(&[0x30, 0x80, 0x00, 0x00]).is_none());
        // A length field wider than four octets.
        assert!(der_tlv_header(&[0x30, 0x85, 0, 0, 0, 0, 0]).is_none());
    }

    /// Multi-octet lengths are read, not rejected — a real certificate is
    /// longer than 127 octets and every one of them takes this branch.
    #[test]
    fn multi_octet_lengths_are_read() {
        let mut der = alloc::vec![0x30, 0x82, 0x01, 0x00];
        der.extend(core::iter::repeat_n(0u8, 0x100));
        assert_eq!(der_tlv_header(&der), Some((0x30, 4, 0x100)));
    }
}

/// Test material for the deviation: X.509 **v1** certificates, which no
/// certificate generator in this tree can emit.
///
/// `rcgen` writes v3 unconditionally (`rcgen::CertificateParams` always
/// encodes the `[0] version` field and an extensions block), so a v1
/// certificate has to be assembled field by field. It is built here with
/// `x509-cert`'s own `TbsCertificate` — `version: Version::V1`, which the
/// DER encoder omits because v1 is the ASN.1 default, and
/// `extensions: None`, which is the only thing a v1 certificate can say —
/// and signed with this crate's configured crypto backend. The result is
/// a genuine v1 certificate rather than a fixture copied from somewhere,
/// so the tests do not depend on a file this repository must not carry
/// (the OMG suite's certificates are not vendored here).
///
/// The issuing CA is an ordinary `rcgen` v3 CA, which is what the OMG
/// suite ships as its identity CA too.
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
pub(crate) mod testonly_v1_certs {
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::time::Duration;

    use x509_cert::der::asn1::{BitString, GeneralizedTime, UtcTime};
    use x509_cert::der::{Decode, Encode};
    use x509_cert::name::Name;
    use x509_cert::serial_number::SerialNumber;
    use x509_cert::spki::{AlgorithmIdentifierOwned, SubjectPublicKeyInfoOwned};
    use x509_cert::time::{Time, Validity};
    use x509_cert::{Certificate, TbsCertificate, Version};

    /// `ecdsa-with-SHA256`, the algorithm `rcgen::KeyPair::generate`
    /// produces and the one the OMG suite's certificates carry.
    const ECDSA_WITH_SHA256: &str = "1.2.840.10045.4.3.2";

    /// A CA: its certificate PEM, its certificate DER, its PKCS#8 key and
    /// the `rcgen` handles that sign a v3 end entity under it.
    pub(crate) struct TestCa {
        pub(crate) cert_pem: Vec<u8>,
        pub(crate) cert_der: Vec<u8>,
        pub(crate) key_pkcs8_der: Vec<u8>,
        rcgen_cert: rcgen::Certificate,
        rcgen_key: rcgen::KeyPair,
    }

    /// Generates a self-signed v3 CA with `CN=<common_name>`.
    pub(crate) fn ca(common_name: &str) -> TestCa {
        ca_with_name_constraints(common_name, None)
    }

    /// The same, optionally carrying an RFC 5280 §4.2.1.10
    /// `nameConstraints` extension with one permitted DNS subtree.
    pub(crate) fn ca_with_name_constraints(
        common_name: &str,
        permitted_dns_subtree: Option<&str>,
    ) -> TestCa {
        use rcgen::{
            BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair,
        };
        let mut params = CertificateParams::new(Vec::new()).expect("ca params");
        if let Some(subtree) = permitted_dns_subtree {
            params.name_constraints = Some(rcgen::NameConstraints {
                permitted_subtrees: alloc::vec![rcgen::GeneralSubtree::DnsName(
                    alloc::string::ToString::to_string(subtree)
                )],
                excluded_subtrees: Vec::new(),
            });
        }
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, common_name);
        params.distinguished_name = dn;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let key = KeyPair::generate().expect("ca key");
        let cert = params.self_signed(&key).expect("self-signed ca");
        TestCa {
            cert_pem: cert.pem().into_bytes(),
            cert_der: cert.der().to_vec(),
            key_pkcs8_der: key.serialize_der(),
            rcgen_cert: cert,
            rcgen_key: key,
        }
    }

    /// Generates an end-entity **v3** certificate under `issuer`, the way
    /// every spec-conforming deployment does.
    pub(crate) fn v3_end_entity(issuer: &TestCa, common_name: &str) -> (Vec<u8>, Vec<u8>) {
        use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};
        let mut params = CertificateParams::new(Vec::new()).expect("ee params");
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, common_name);
        params.distinguished_name = dn;
        params.is_ca = IsCa::NoCa;
        let key = KeyPair::generate().expect("ee key");
        let cert = params
            .signed_by(&key, &issuer.rcgen_cert, &issuer.rcgen_key)
            .expect("signed ee");
        (cert.pem().into_bytes(), key.serialize_pem().into_bytes())
    }

    /// Mints an X.509 **v1** end-entity certificate.
    ///
    /// `signing_ca` provides the key that signs; `named_issuer` provides
    /// the `issuer` field. They are separate parameters on purpose: a test
    /// that proves the deviation still checks the chain needs a
    /// certificate that *names* the trusted CA and is signed by another
    /// key, and no honest generator produces one.
    ///
    /// `not_before`/`not_after` are seconds since the Unix epoch, so a
    /// test can mint an expired certificate without waiting for one.
    pub(crate) fn v1_end_entity(
        signing_ca: &TestCa,
        named_issuer: &TestCa,
        common_name: &str,
        not_before: u64,
        not_after: u64,
    ) -> (Vec<u8>, Vec<u8>) {
        use rcgen::KeyPair;
        let subject_key = KeyPair::generate().expect("subject key");
        let der = v1_der(
            signing_ca,
            named_issuer,
            common_name,
            not_before,
            not_after,
            &subject_key.public_key_der(),
        );
        (
            pem_certificate(&der).into_bytes(),
            subject_key.serialize_pem().into_bytes(),
        )
    }

    fn v1_der(
        signing_ca: &TestCa,
        named_issuer: &TestCa,
        common_name: &str,
        not_before: u64,
        not_after: u64,
        subject_spki_der: &[u8],
    ) -> Vec<u8> {
        let issuer_cert = Certificate::from_der(&named_issuer.cert_der).expect("issuer der");
        let subject: Name = common_name_to_name(common_name);
        let algorithm = AlgorithmIdentifierOwned {
            oid: ECDSA_WITH_SHA256.parse().expect("oid"),
            parameters: None,
        };
        let tbs = TbsCertificate {
            version: Version::V1,
            serial_number: SerialNumber::new(&[0x2a]).expect("serial"),
            signature: algorithm.clone(),
            issuer: issuer_cert.tbs_certificate.subject.clone(),
            validity: Validity {
                not_before: utc(not_before),
                not_after: utc(not_after),
            },
            subject,
            subject_public_key_info: SubjectPublicKeyInfoOwned::from_der(subject_spki_der)
                .expect("subject spki"),
            issuer_unique_id: None,
            subject_unique_id: None,
            // A v1 certificate has no extensions field. That is what makes
            // the certificate v1 and it is the whole point of the fixture.
            extensions: None,
        };
        let tbs_der = tbs.to_der().expect("tbs der");
        let signature = sign_ecdsa_p256(&signing_ca.key_pkcs8_der, &tbs_der);
        let cert = Certificate {
            tbs_certificate: tbs,
            signature_algorithm: algorithm,
            signature: BitString::from_bytes(&signature).expect("signature bits"),
        };
        cert.to_der().expect("certificate der")
    }

    fn common_name_to_name(common_name: &str) -> Name {
        use core::str::FromStr;
        Name::from_str(&alloc::format!("CN={common_name}")).expect("subject name")
    }

    /// RFC 5280 §4.1.2.5: dates through 2049 are `UTCTime`, from 2050 on
    /// `GeneralizedTime`. `UtcTime::from_unix_duration` refuses anything
    /// later, which is what makes this a branch and not a preference.
    fn utc(unix_secs: u64) -> Time {
        /// 2050-01-01T00:00:00Z.
        const UTC_TIME_HORIZON: u64 = 2_524_608_000;
        let d = Duration::from_secs(unix_secs);
        if unix_secs < UTC_TIME_HORIZON {
            Time::UtcTime(UtcTime::from_unix_duration(d).expect("utc time"))
        } else {
            Time::GeneralTime(GeneralizedTime::from_unix_duration(d).expect("generalized time"))
        }
    }

    /// ECDSA-P-256/SHA-256 with an ASN.1 signature — through this crate's
    /// configured backend, so the fixture generator follows the same
    /// `ring`/`aws-lc-rs`/`wolfCrypt` selection the library does.
    fn sign_ecdsa_p256(pkcs8_der: &[u8], message: &[u8]) -> Vec<u8> {
        use crate::backend::rand::SystemRandom;
        use crate::backend::signature::ECDSA_P256_SHA256_ASN1_SIGNING;
        let rng = SystemRandom::new();
        let key = crate::compat::ecdsa_from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8_der, &rng)
            .expect("ca signing key");
        key.sign(&rng, message).expect("sign tbs").as_ref().to_vec()
    }

    /// PEM `CERTIFICATE` block. `rustls-pki-types` parses PEM but does not
    /// write it, and the configuration path this is fed into takes PEM.
    pub(crate) fn pem_certificate(der: &[u8]) -> String {
        let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
        let b64 = base64_standard(der);
        for chunk in b64.as_bytes().chunks(64) {
            out.push_str(core::str::from_utf8(chunk).expect("base64 is ascii"));
            out.push('\n');
        }
        out.push_str("-----END CERTIFICATE-----\n");
        out
    }

    fn base64_standard(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let b0 = u32::from(chunk[0]);
            let b1 = chunk.get(1).copied().map_or(0, u32::from);
            let b2 = chunk.get(2).copied().map_or(0, u32::from);
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(char::from(ALPHABET[((n >> 18) & 0x3f) as usize]));
            out.push(char::from(ALPHABET[((n >> 12) & 0x3f) as usize]));
            if chunk.len() > 1 {
                out.push(char::from(ALPHABET[((n >> 6) & 0x3f) as usize]));
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(char::from(ALPHABET[(n & 0x3f) as usize]));
            } else {
                out.push('=');
            }
        }
        out
    }
}

/// DDS-Security 1.2 §10.3.1.3 — the deviation's own clause tests.
///
/// Five of them, and the third is the one that carries the claim: with
/// the deviation on, a v1 certificate whose chain does not hold is still
/// refused. A mode that relaxed more than the version would pass the
/// first two and fail that one.
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod clause_10_3_1_3 {
    use super::testonly_v1_certs as fixtures;
    use super::*;
    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use alloc::vec;

    use crate::identity::{IdentityConfig, PkiError};
    use crate::plugin::PkiAuthenticationPlugin;
    use zerodds_security::authentication::AuthenticationPlugin;
    use zerodds_security::logging::{LogLevel, LoggingPlugin};
    use zerodds_security::properties::{Property, PropertyList};

    /// 2000-01-01T00:00:00Z and 2100-01-01T00:00:00Z — a window that
    /// contains every plausible run of this test suite.
    const YEAR_2000: u64 = 946_684_800;
    const YEAR_2100: u64 = 4_102_444_800;
    /// 1990-01-01 to 1991-01-01: over long before this code existed.
    const YEAR_1990: u64 = 631_152_000;
    const YEAR_1991: u64 = 662_688_000;

    /// A `LoggingPlugin` that keeps what it was handed, so a test can
    /// read the security-log event instead of a captured stderr.
    /// One security-log event, as `LoggingPlugin::log` was handed it.
    type CapturedEvent = (LogLevel, [u8; 16], String, String);

    #[derive(Default)]
    struct CapturingLogger {
        events: std::sync::Mutex<Vec<CapturedEvent>>,
    }

    impl LoggingPlugin for CapturingLogger {
        // Test double: it records `log` calls only; the Table 46 operations
        // have nothing to configure.
        fn set_log_options(
            &self,
            _options: zerodds_security::logging::LogOptions,
        ) -> Result<(), zerodds_security::error::SecurityError> {
            Ok(())
        }
        fn enable_logging(&self) -> Result<(), zerodds_security::error::SecurityError> {
            Ok(())
        }
        fn set_listener(
            &self,
            _listener: Option<std::sync::Arc<dyn zerodds_security::logging::LoggerListener>>,
        ) -> Result<(), zerodds_security::error::SecurityError> {
            Ok(())
        }
        fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
            if let Ok(mut events) = self.events.lock() {
                events.push((
                    level,
                    participant,
                    category.to_string(),
                    message.to_string(),
                ));
            }
        }
        fn plugin_class_id(&self) -> &str {
            "DDS:Logging:TestCapture"
        }
    }

    fn identity(cert_pem: Vec<u8>, ca_pem: Vec<u8>, key_pem: Vec<u8>) -> IdentityConfig {
        IdentityConfig {
            identity_cert_pem: cert_pem,
            identity_ca_pem: ca_pem,
            identity_key_pem: Some(key_pem),
        }
    }

    /// (1) Without the switch, an X.509 v1 identity certificate is
    /// refused, and the refusal names the version it found and the clause
    /// that requires v3. A message that only said "unsupported" would send
    /// its reader to webpki's source to learn what happened.
    #[test]
    fn without_the_deviation_a_v1_identity_certificate_is_refused_by_version() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);

        let mut plugin = PkiAuthenticationPlugin::new();
        assert!(
            !plugin.cert_policy().accept_x509v1_identity_certificate,
            "the deviation is off until something switches it on"
        );
        let err = plugin
            .validate_with_config(identity(cert, ca.cert_pem.clone(), key), [0xAA; 16])
            .expect_err("a v1 identity certificate is not a v3 one");
        let text = alloc::format!("{err}");
        assert!(text.contains("v1"), "the version must be named: {text}");
        assert!(
            text.contains("§10.3.1.3"),
            "the clause must be named: {text}"
        );
        assert!(
            text.contains(PROP_ACCEPT_X509V1_IDENTITY),
            "the way out must be named: {text}"
        );
    }

    /// The same refusal reaches the property-configured path, which is
    /// how a deployment actually configures the plugin (Table 50).
    #[test]
    fn without_the_deviation_the_property_path_refuses_a_v1_certificate_too() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);

        let props = PropertyList::new()
            .with(Property::local(
                "dds.sec.auth.identity_certificate",
                String::from_utf8(cert).expect("pem is utf-8"),
            ))
            .with(Property::local(
                "dds.sec.auth.identity_ca",
                String::from_utf8(ca.cert_pem.clone()).expect("pem is utf-8"),
            ))
            .with(Property::local(
                "dds.sec.auth.private_key",
                String::from_utf8(key).expect("pem is utf-8"),
            ));

        let mut plugin = PkiAuthenticationPlugin::new();
        let err = plugin
            .validate_local_identity(0, &props, [0xAA; 16])
            .expect_err("no deviation property, so the spec rule holds");
        assert!(alloc::format!("{err}").contains("§10.3.1.3"));
    }

    /// (2) With the switch on, the same certificate is accepted — and the
    /// acceptance produces the §8.6 log event, naming the subject and the
    /// clause. An accepted deviation nobody is told about would be worse
    /// than the refusal.
    #[test]
    fn with_the_deviation_a_v1_identity_certificate_is_accepted_and_logged() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);

        let logger = Arc::new(CapturingLogger::default());
        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_security_logger(Arc::clone(&logger) as Arc<dyn LoggingPlugin>);
        plugin.set_accept_x509v1_identity_certificates(true);
        plugin
            .validate_with_config(identity(cert, ca.cert_pem.clone(), key), [0xAA; 16])
            .expect("the deviation accepts a v1 certificate whose chain holds");

        let events = logger.events.lock().expect("no panic in the logger");
        assert_eq!(events.len(), 1, "exactly one event per accepted identity");
        let (level, participant, category, message) = &events[0];
        assert_eq!(*level, LogLevel::Warning);
        assert_eq!(*participant, [0xAA; 16]);
        assert_eq!(category, LOG_CATEGORY_X509V1_DEVIATION);
        assert!(
            message.contains("CN = alice"),
            "the subject must be in the event: {message}"
        );
        assert!(
            message.contains("§10.3.1.3"),
            "the clause must be in the event: {message}"
        );
        assert!(
            message.contains(PROP_ACCEPT_X509V1_IDENTITY),
            "the switch that caused it must be in the event: {message}"
        );
    }

    /// The switch is reachable the way every other `dds.sec.*` setting
    /// is: a property on the participant's `PropertyQosPolicy`.
    #[test]
    fn the_property_switches_the_deviation_on() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);

        let base = PropertyList::new()
            .with(Property::local(
                "dds.sec.auth.identity_certificate",
                String::from_utf8(cert).expect("pem is utf-8"),
            ))
            .with(Property::local(
                "dds.sec.auth.identity_ca",
                String::from_utf8(ca.cert_pem.clone()).expect("pem is utf-8"),
            ))
            .with(Property::local(
                "dds.sec.auth.private_key",
                String::from_utf8(key).expect("pem is utf-8"),
            ));

        let mut on = PkiAuthenticationPlugin::new();
        on.set_security_logger(Arc::new(CapturingLogger::default()) as Arc<dyn LoggingPlugin>);
        let props = base
            .clone()
            .with(Property::local(PROP_ACCEPT_X509V1_IDENTITY, "true"));
        on.validate_local_identity(0, &props, [0xAA; 16])
            .expect("the property switches the deviation on");

        // A value that is not an affirmative leaves the spec rule in
        // place rather than guessing what the operator meant.
        let mut typo = PkiAuthenticationPlugin::new();
        let props = base.with(Property::local(PROP_ACCEPT_X509V1_IDENTITY, "ture"));
        assert!(
            typo.validate_local_identity(0, &props, [0xAA; 16]).is_err(),
            "a misspelled value must not switch a spec deviation on"
        );
    }

    /// (3) **The test that carries the claim.** With the deviation on, a
    /// v1 certificate whose chain does not hold is still refused — twice
    /// over, because a chain breaks in two different ways.
    #[test]
    fn with_the_deviation_a_v1_certificate_with_a_broken_chain_is_still_refused() {
        let trusted = fixtures::ca("ZeroDDS Trusted Identity CA");
        let rogue = fixtures::ca("Rogue CA");

        // (a) Signed by the rogue CA and naming it: no configured anchor
        //     has that issuer.
        let (cert, key) = fixtures::v1_end_entity(&rogue, &rogue, "mallory", YEAR_2000, YEAR_2100);
        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        let err = plugin
            .validate_with_config(identity(cert, trusted.cert_pem.clone(), key), [0xBB; 16])
            .expect_err("a certificate from an untrusted CA is not an identity");
        assert!(
            alloc::format!("{err}").contains("issuer"),
            "the refusal must say the chain failed: {err}"
        );

        // (b) Signed by the rogue CA but *naming the trusted one* — the
        //     shape an attacker actually sends, and the one an issuer-name
        //     check alone would let through. The signature must be checked
        //     against the anchor whose name it claims.
        let (cert, key) =
            fixtures::v1_end_entity(&rogue, &trusted, "mallory", YEAR_2000, YEAR_2100);
        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        let err = plugin
            .validate_with_config(identity(cert, trusted.cert_pem.clone(), key), [0xBB; 16])
            .expect_err("naming the trusted CA is not being signed by it");
        assert!(
            alloc::format!("{err}").contains("signature does not verify"),
            "the refusal must say the signature failed: {err}"
        );
    }

    /// The same, one level down: a single flipped byte in the signed body
    /// breaks the signature. This is what proves the check reads the
    /// certificate's own bytes and not a re-encoding of them.
    #[test]
    fn with_the_deviation_a_tampered_v1_certificate_is_refused() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert_pem, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);
        let mut der = crate::identity::first_cert_der(&cert_pem).expect("a certificate");
        // The serial number sits early in the TBS; flipping it changes
        // the signed bytes without changing the structure.
        let serial = der
            .windows(1)
            .position(|w| w == [0x2a])
            .expect("the fixture's serial number");
        der[serial] = 0x2b;
        let tampered = fixtures::pem_certificate(&der).into_bytes();

        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        let err = plugin
            .validate_with_config(identity(tampered, ca.cert_pem.clone(), key), [0xCC; 16])
            .expect_err("a tampered certificate must not authenticate anybody");
        assert!(
            alloc::format!("{err}").contains("signature does not verify"),
            "{err}"
        );
    }

    /// An identity CA that carries `nameConstraints` refuses the
    /// deviation path rather than having its constraint silently
    /// dropped. The v3 path enforces the extension; this path does not
    /// evaluate it, and skipping it would relax more than the version.
    #[test]
    fn with_the_deviation_an_anchor_with_name_constraints_refuses_the_v1_path() {
        let ca = fixtures::ca_with_name_constraints(
            "ZeroDDS Constrained Identity CA",
            Some("example.test"),
        );
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);

        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        let err = plugin
            .validate_with_config(identity(cert, ca.cert_pem.clone(), key), [0xAB; 16])
            .expect_err("a constraint this path cannot evaluate is not a constraint to ignore");
        assert!(
            alloc::format!("{err}").contains("nameConstraints"),
            "the refusal must name what it could not evaluate: {err}"
        );
    }

    /// (4) With the deviation on, an expired v1 certificate is refused.
    /// The validity window is not what the deviation relaxes.
    #[test]
    fn with_the_deviation_an_expired_v1_certificate_is_refused() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_1990, YEAR_1991);

        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        let err = plugin
            .validate_with_config(identity(cert, ca.cert_pem.clone(), key), [0xDD; 16])
            .expect_err("an expired certificate is expired in both modes");
        assert!(
            alloc::format!("{err}").contains("expired"),
            "the refusal must say the certificate expired: {err}"
        );
    }

    /// The other end of the window: a certificate that is not valid yet.
    #[test]
    fn with_the_deviation_a_not_yet_valid_v1_certificate_is_refused() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2100, YEAR_2100 + 86_400);

        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        let err = plugin
            .validate_with_config(identity(cert, ca.cert_pem.clone(), key), [0xDD; 16])
            .expect_err("a certificate from the future is not valid now");
        assert!(
            alloc::format!("{err}").contains("not valid before"),
            "{err}"
        );
    }

    /// The subject the caller expects is checked in the deviation path,
    /// not skipped along with the version.
    #[test]
    fn with_the_deviation_the_expected_subject_is_still_checked() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);

        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        plugin.set_expected_certificate_subject(Some("CN = bob".to_string()));
        let err = plugin
            .validate_with_config(
                identity(cert.clone(), ca.cert_pem.clone(), key.clone()),
                [0xEE; 16],
            )
            .expect_err("alice is not bob");
        assert!(alloc::format!("{err}").contains("expected"), "{err}");

        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_accept_x509v1_identity_certificates(true);
        plugin.set_expected_certificate_subject(Some("CN=alice".to_string()));
        plugin
            .validate_with_config(identity(cert, ca.cert_pem.clone(), key), [0xEE; 16])
            .expect("the expected subject is the one on the certificate");
    }

    /// (5) A v3 certificate takes the same path in both modes: accepted
    /// as [`CertAcceptance::SpecV3`], with no log event, and refused for
    /// the same reason when its chain does not hold. The deviation does
    /// not reach the spec path at all.
    #[test]
    fn a_v3_certificate_takes_the_same_path_in_both_modes() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let rogue = fixtures::ca("Rogue CA");
        let (good_cert, good_key) = fixtures::v3_end_entity(&ca, "alice");
        let (rogue_cert, rogue_key) = fixtures::v3_end_entity(&rogue, "mallory");

        let mut refusals = Vec::new();
        for deviation in [false, true] {
            let logger = Arc::new(CapturingLogger::default());
            let mut plugin = PkiAuthenticationPlugin::new();
            plugin.set_security_logger(Arc::clone(&logger) as Arc<dyn LoggingPlugin>);
            plugin.set_accept_x509v1_identity_certificates(deviation);

            plugin
                .validate_with_config(
                    identity(good_cert.clone(), ca.cert_pem.clone(), good_key.clone()),
                    [0xAA; 16],
                )
                .expect("a v3 certificate under its own CA is valid in both modes");
            assert!(
                logger.events.lock().expect("no panic").is_empty(),
                "the spec path is not a deviation and writes no event \
                 (deviation switch = {deviation})"
            );

            let err = plugin
                .validate_with_config(
                    identity(rogue_cert.clone(), ca.cert_pem.clone(), rogue_key.clone()),
                    [0xBB; 16],
                )
                .expect_err("a v3 certificate from an untrusted CA is refused in both modes");
            refusals.push(alloc::format!("{err}"));
        }
        assert_eq!(
            refusals[0], refusals[1],
            "the switch must not change what happens to a v3 certificate"
        );
        assert!(
            !refusals[0].contains("x509v1"),
            "a v3 certificate must never reach the deviation path: {}",
            refusals[0]
        );
    }

    /// The version check is the *only* thing the deviation relaxes, said
    /// once at the level of the error type: v2 is refused in both modes,
    /// because the deviation exists for what the OMG suite ships and the
    /// OMG suite ships v1.
    #[test]
    fn the_deviation_does_not_extend_to_x509_v2() {
        // A v2 certificate cannot be minted through the fixture (a v2
        // certificate is a v1 body with unique identifiers), so this is
        // checked where the decision is made.
        let policy = CertPolicy {
            accept_x509v1_identity_certificate: true,
            expected_subject: None,
            crls_der: Vec::new(),
            ocsp: Default::default(),
            algorithm_naming: Default::default(),
        };
        assert!(policy.accept_x509v1_identity_certificate);
        assert!(!CertVersion(1).is_v1(), "v2 is not v1");
        assert!(!CertVersion(1).is_v3(), "v2 is not v3 either");
    }

    /// `PkiError::CertVersionNotV3` is its own variant rather than a
    /// string inside `CertInvalid`, so a caller can tell "wrong version"
    /// from "bad signature" without parsing English.
    #[test]
    fn the_version_refusal_is_its_own_error_variant() {
        let ca = fixtures::ca("ZeroDDS Test Identity CA");
        let (cert, _key) = fixtures::v1_end_entity(&ca, &ca, "alice", YEAR_2000, YEAR_2100);
        let der = crate::identity::first_cert_der(&cert).expect("a certificate");
        let anchors = vec![crate::identity::first_cert_der(&ca.cert_pem).expect("the ca")];
        let err = crate::identity::verify_cert_chain(&der, &anchors, &CertPolicy::spec())
            .expect_err("v1 without the deviation");
        assert!(matches!(err, PkiError::CertVersionNotV3(ref v) if v == "v1"));

        let ok = crate::identity::verify_cert_chain(
            &der,
            &anchors,
            &CertPolicy {
                accept_x509v1_identity_certificate: true,
                expected_subject: None,
                crls_der: Vec::new(),
                ocsp: Default::default(),
                algorithm_naming: Default::default(),
            },
        )
        .expect("with the deviation");
        assert_eq!(
            ok,
            CertAcceptance::X509V1Deviation {
                subject: "CN = alice".to_string()
            }
        );
    }

    /// The `Box<dyn LoggingPlugin>` shape the §8.6 wiring uses works too —
    /// the plugin slot is not restricted to one concrete sink.
    #[test]
    fn the_logger_slot_takes_any_section_8_6_plugin() {
        let boxed: Box<dyn LoggingPlugin> = Box::new(CapturingLogger::default());
        let arc: Arc<dyn LoggingPlugin> = Arc::from(boxed);
        let mut plugin = PkiAuthenticationPlugin::new();
        plugin.set_security_logger(arc);
    }
}
