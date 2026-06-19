// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CMS / PKCS#7 signing of DDS-Security governance & permissions XML.
//!
//! DDS-Security 1.2 §9.4.1.3 requires the `governance` and `permissions`
//! documents to be S/MIME (CMS `SignedData`) signed by the **Permissions CA**.
//! ZeroDDS's builtin AccessControl plugin
//! ([`zerodds_security_permissions::CmsPkcs7Verifier`]) verifies them; this
//! crate produces them.
//!
//! Output format: opaque S/MIME `application/pkcs7-mime; smime-type=signed-data`
//! — a single self-contained `.p7s` carrying both the signed XML (embedded
//! `eContent`) and the signature. The signer certificate is the Permissions CA
//! itself (the de-facto cross-vendor pattern; the verifier accepts a signer that
//! is byte-identical to a configured permissions-CA trust anchor).
//!
//! Signature: ECDSA P-256 / SHA-256 with an ASN.1-DER signature value, the
//! combination the verifier checks (`ECDSA_P256_SHA256_ASN1`).
//!
//! Safety classification: **COMFORT** (deployment tooling, off the runtime path).

use base64::Engine as _;
use cms::builder::{SignedDataBuilder, SignerInfoBuilder};
use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::signed_data::{EncapsulatedContentInfo, SignerIdentifier};
use const_oid::ObjectIdentifier;
use der::asn1::Any;
use der::{Encode, Tag};
use p256::ecdsa::{DerSignature, SigningKey};
use p256::pkcs8::DecodePrivateKey;
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::der::DecodePem;

/// `id-data` content type (RFC 5652).
const OID_ID_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1");
/// `id-sha256` (NIST).
const OID_SHA_256: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1");

/// Error from signing.
#[derive(Debug)]
pub enum SignError {
    /// The signer certificate PEM could not be parsed.
    Cert(String),
    /// The signer private key PEM could not be parsed.
    Key(String),
    /// CMS / DER construction failed.
    Cms(String),
}

impl core::fmt::Display for SignError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SignError::Cert(m) => write!(f, "signer certificate: {m}"),
            SignError::Key(m) => write!(f, "signer key: {m}"),
            SignError::Cms(m) => write!(f, "cms: {m}"),
        }
    }
}

impl std::error::Error for SignError {}

/// Sign `xml` with the Permissions-CA `signer_cert_pem` + `signer_key_pkcs8_pem`,
/// returning a self-contained opaque S/MIME `application/pkcs7-mime;
/// smime-type=signed-data` document (headers + base64 DER).
///
/// The same operation applies to both `governance.xml` and `permissions.xml`.
pub fn sign_xml_opaque(
    xml: &[u8],
    signer_cert_pem: &str,
    signer_key_pkcs8_pem: &str,
) -> Result<Vec<u8>, SignError> {
    let der = sign_xml_to_der(xml, signer_cert_pem, signer_key_pkcs8_pem)?;
    Ok(smime_wrap_opaque(&der))
}

/// Like [`sign_xml_opaque`] but returns the raw DER `ContentInfo` (signedData),
/// without the S/MIME header envelope.
pub fn sign_xml_to_der(
    xml: &[u8],
    signer_cert_pem: &str,
    signer_key_pkcs8_pem: &str,
) -> Result<Vec<u8>, SignError> {
    let cert =
        Certificate::from_pem(signer_cert_pem).map_err(|e| SignError::Cert(format!("{e:?}")))?;
    let signing_key = SigningKey::from_pkcs8_pem(signer_key_pkcs8_pem).map_err(|e| {
        if signer_key_pkcs8_pem.contains("BEGIN EC PRIVATE KEY") {
            // openssl `ecparam -genkey` emits a SEC1 key; the signer needs PKCS#8.
            SignError::Key(
                "SEC1 EC key detected (`BEGIN EC PRIVATE KEY`); convert to PKCS#8 first: \
                 `openssl pkcs8 -topk8 -nocrypt -in <key>.pem -out <key>_pkcs8.pem`"
                    .to_string(),
            )
        } else {
            SignError::Key(format!("expected a PKCS#8 PEM private key: {e:?}"))
        }
    })?;

    // Opaque eContent: the XML embedded as an OCTET STRING.
    let econtent_any =
        Any::new(Tag::OctetString, xml.to_vec()).map_err(|e| SignError::Cms(format!("{e:?}")))?;
    let encap = EncapsulatedContentInfo {
        econtent_type: OID_ID_DATA,
        econtent: Some(econtent_any),
    };

    let digest_alg = AlgorithmIdentifierOwned {
        oid: OID_SHA_256,
        parameters: None,
    };

    let sid = SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
        issuer: cert.tbs_certificate.issuer.clone(),
        serial_number: cert.tbs_certificate.serial_number.clone(),
    });

    let signer_info_builder =
        SignerInfoBuilder::new(&signing_key, sid, digest_alg.clone(), &encap, None)
            .map_err(|e| SignError::Cms(format!("signer-info builder: {e:?}")))?;

    let content_info = SignedDataBuilder::new(&encap)
        .add_digest_algorithm(digest_alg)
        .map_err(|e| SignError::Cms(format!("add digest alg: {e:?}")))?
        .add_certificate(CertificateChoices::Certificate(cert))
        .map_err(|e| SignError::Cms(format!("add cert: {e:?}")))?
        .add_signer_info::<SigningKey, DerSignature>(signer_info_builder)
        .map_err(|e| SignError::Cms(format!("add signer info: {e:?}")))?
        .build()
        .map_err(|e| SignError::Cms(format!("build: {e:?}")))?;

    content_info
        .to_der()
        .map_err(|e| SignError::Cms(format!("encode: {e:?}")))
}

/// Wrap a DER PKCS#7 `signedData` blob in an opaque S/MIME envelope that
/// [`zerodds_security_permissions::CmsPkcs7Verifier`] accepts.
fn smime_wrap_opaque(der: &[u8]) -> Vec<u8> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::new();
    out.push_str("MIME-Version: 1.0\r\n");
    out.push_str("Content-Disposition: attachment; filename=\"smime.p7m\"\r\n");
    out.push_str(
        "Content-Type: application/pkcs7-mime; smime-type=signed-data; name=\"smime.p7m\"\r\n",
    );
    out.push_str("Content-Transfer-Encoding: base64\r\n");
    out.push_str("\r\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(core::str::from_utf8(chunk).unwrap_or(""));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use x509_cert::builder::{Builder, CertificateBuilder, Profile};
    use x509_cert::der::EncodePem;
    use x509_cert::name::Name;
    use x509_cert::serial_number::SerialNumber;
    use x509_cert::spki::EncodePublicKey;
    use x509_cert::time::Validity;
    use zerodds_security_permissions::{CmsPkcs7Verifier, XmlSignatureVerifier};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Generate a self-signed P-256 Permissions CA → (cert_pem, key_pkcs8_pem).
    fn gen_permissions_ca() -> Result<(String, String), Box<dyn std::error::Error>> {
        use std::str::FromStr;
        let signing_key = SigningKey::random(&mut rand_core::OsRng);
        let verifying_key = signing_key.verifying_key();
        let spki_der = verifying_key
            .to_public_key_der()
            .map_err(|e| format!("spki: {e:?}"))?
            .into_vec();
        let spki = spki::SubjectPublicKeyInfoOwned::try_from(spki_der.as_slice())
            .map_err(|e| format!("spki parse: {e:?}"))?;

        let serial = SerialNumber::from(1u32);
        let validity = Validity::from_now(Duration::from_secs(3650 * 24 * 3600))
            .map_err(|e| format!("validity: {e:?}"))?;
        let subject = Name::from_str("CN=ZeroDDS Permissions CA,O=ZeroDDS,C=DE")
            .map_err(|e| format!("name: {e:?}"))?;

        let builder =
            CertificateBuilder::new(Profile::Root, serial, validity, subject, spki, &signing_key)
                .map_err(|e| format!("cert builder: {e:?}"))?;
        let cert = builder
            .build::<DerSignature>()
            .map_err(|e| format!("build cert: {e:?}"))?;
        let cert_pem = cert
            .to_pem(der::pem::LineEnding::LF)
            .map_err(|e| format!("cert pem: {e:?}"))?;

        let key_pem =
            p256::pkcs8::EncodePrivateKey::to_pkcs8_pem(&signing_key, der::pem::LineEnding::LF)
                .map_err(|e| format!("key pem: {e:?}"))?
                .to_string();
        Ok((cert_pem, key_pem))
    }

    const PERMISSIONS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds><permissions><grant name="Adapter1"><subject_name>CN=adapter1</subject_name></grant></permissions></dds>"#;

    #[test]
    fn sign_roundtrips_through_real_verifier() -> TestResult {
        let (ca_cert_pem, ca_key_pem) = gen_permissions_ca()?;
        let p7s = sign_xml_opaque(PERMISSIONS_XML.as_bytes(), &ca_cert_pem, &ca_key_pem)?;

        // The real ZeroDDS AccessControl verifier must accept it.
        let verifier = CmsPkcs7Verifier::new(ca_cert_pem.as_bytes())
            .map_err(|e| format!("verifier: {e:?}"))?;
        let recovered = verifier
            .verify_and_extract(&p7s)
            .map_err(|e| format!("verify should accept our signature: {e:?}"))?;
        assert_eq!(
            recovered,
            PERMISSIONS_XML.as_bytes(),
            "recovered XML mismatch"
        );
        Ok(())
    }

    #[test]
    fn wrong_ca_is_rejected() -> TestResult {
        let (ca_cert_pem, ca_key_pem) = gen_permissions_ca()?;
        let (other_ca_pem, _) = gen_permissions_ca()?;
        let p7s = sign_xml_opaque(PERMISSIONS_XML.as_bytes(), &ca_cert_pem, &ca_key_pem)?;
        let verifier = CmsPkcs7Verifier::new(other_ca_pem.as_bytes())
            .map_err(|e| format!("verifier: {e:?}"))?;
        assert!(
            verifier.verify_and_extract(&p7s).is_err(),
            "wrong CA must reject"
        );
        Ok(())
    }

    #[test]
    fn tampered_content_is_rejected() -> TestResult {
        let (ca_cert_pem, ca_key_pem) = gen_permissions_ca()?;
        let der = sign_xml_to_der(PERMISSIONS_XML.as_bytes(), &ca_cert_pem, &ca_key_pem)?;
        // Flip a byte in the DER → signature/structure broken.
        let mut bad = der.clone();
        let n = bad.len();
        bad[n / 2] ^= 0xff;
        let smime = smime_wrap_opaque(&bad);
        let verifier = CmsPkcs7Verifier::new(ca_cert_pem.as_bytes())
            .map_err(|e| format!("verifier: {e:?}"))?;
        assert!(
            verifier.verify_and_extract(&smime).is_err(),
            "tampered must reject"
        );
        Ok(())
    }
}
