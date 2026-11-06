// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-secure-permissions` with `zerodds-security-permissions`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-permissions`, in `main.rs`
//! (`lib.rs` names it only in its docs and its unit tests):
//! - `CmsPkcs7Verifier::new(ca_pem)`: in `sign` with the signer certificate
//!   as the Permissions CA, in `verify` with the file of `--ca`; an error is
//!   printed with `Display` (`verifier: {e}`) and ends the run with exit
//!   code 1.
//! - `XmlSignatureVerifier::verify_and_extract(p7s)`, imported for the
//!   method: in `sign` as the self-check of the document before it is
//!   written, in `verify` for the document of `--p7s`.
//!
//! What it relies on: a document `sign_xml_opaque` made with a self-signed
//! Permissions CA verifies against that CA and gives back the signed XML
//! byte for byte (`sign` compares and refuses to write otherwise); a
//! document of another CA, or one changed after signing, is refused; an
//! opaque S/MIME document OpenSSL wrote is read too (`verify` takes any
//! document a vendor tool produced); a CA file without a certificate is an
//! error, not an empty trust store.
//!
//! Oracles: DDS-Security 1.2 §10.4.1.1 (governance and permissions are
//! S/MIME documents signed by the Permissions CA), RFC 5652 §5.4 and §11.2
//! (the `messageDigest` attribute covers the content), RFC 5751 §3.1.1 (a
//! `text/plain` entity is signed in its canonical CRLF form), and octets
//! OpenSSL wrote: `crates/security-permissions/tests/fixtures/`, made by
//! `generate_fixtures.sh` with `openssl cms -sign -text -nodetach`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use zerodds_secure_permissions::sign_xml_opaque;
use zerodds_security_permissions::{CmsPkcs7Verifier, XmlSignatureVerifier};

/// The Permissions CA of the OpenSSL fixtures.
const OPENSSL_CA: &[u8] =
    include_bytes!("../../../crates/security-permissions/tests/fixtures/permissions_ca_cert.pem");
/// `permissions.xml`, signed by OpenSSL as opaque S/MIME with `-text`.
const OPENSSL_SIGNED: &[u8] = include_bytes!(
    "../../../crates/security-permissions/tests/fixtures/valid_permissions.smime.p7s"
);
/// The same document, changed after OpenSSL signed it.
const OPENSSL_TAMPERED: &[u8] = include_bytes!(
    "../../../crates/security-permissions/tests/fixtures/tampered_permissions.smime.p7s"
);
/// The XML OpenSSL signed.
const OPENSSL_XML: &[u8] =
    include_bytes!("../../../crates/security-permissions/tests/fixtures/permissions.xml");

/// A document with blank lines in both line-end forms: the self-check of
/// `sign` compares the recovered bytes with these, octet for octet.
const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\n\
<dds>\r\n\r\n<permissions><grant name=\"g\"><subject_name>CN=adapter1</subject_name>\n\
<validity><not_before>2020-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>\n\
<allow_rule><domains><id>0</id></domains><publish><topics><topic>Chatter</topic></topics></publish></allow_rule>\n\
<default>DENY</default></grant></permissions></dds>\n";

/// A self-signed P-256 Permissions CA: (certificate PEM, PKCS#8 key PEM).
fn permissions_ca(name: &str) -> (String, String) {
    let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// The opaque S/MIME document `p7s` with the first occurrence of `needle` in
/// its DER changed in one bit, re-wrapped with the same headers.
fn tamper(p7s: &[u8], needle: &[u8]) -> Vec<u8> {
    let text = std::str::from_utf8(p7s).unwrap();
    let (head, body) = text.split_once("\r\n\r\n").expect("S/MIME headers");
    let b64: String = body.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let mut der = STANDARD.decode(b64).unwrap();
    let at = der
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("the signed content is embedded");
    der[at] ^= 0x01;
    let mut out = format!("{head}\r\n\r\n");
    for chunk in STANDARD.encode(der).as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push_str("\r\n");
    }
    out.into_bytes()
}

#[test]
fn security_10_4_1_1_a_document_the_tool_signs_verifies_and_comes_back_unchanged() {
    let (ca, key) = permissions_ca("Permissions CA");
    let p7s = sign_xml_opaque(XML.as_bytes(), &ca, &key).unwrap();
    let verifier = CmsPkcs7Verifier::new(ca.as_bytes()).unwrap();
    let recovered = verifier.verify_and_extract(&p7s).unwrap();
    assert_eq!(
        recovered,
        XML.as_bytes(),
        "`sign` refuses to write a document whose recovered XML differs"
    );
}

#[test]
fn security_10_4_1_1_a_document_of_another_permissions_ca_is_refused() {
    let (ca, key) = permissions_ca("Permissions CA");
    let (other_ca, _) = permissions_ca("Other CA");
    let p7s = sign_xml_opaque(XML.as_bytes(), &ca, &key).unwrap();
    let verifier = CmsPkcs7Verifier::new(other_ca.as_bytes()).unwrap();
    let err = verifier
        .verify_and_extract(&p7s)
        .expect_err("a signer that is not the configured Permissions CA");
    assert!(!err.to_string().is_empty(), "`verify` prints the reason");
}

#[test]
fn security_10_4_1_1_a_bundle_holding_the_permissions_ca_verifies_the_document() {
    // `--ca` may name a PEM file with more than one certificate.
    let (ca, key) = permissions_ca("Permissions CA");
    let (other_ca, _) = permissions_ca("Other CA");
    let p7s = sign_xml_opaque(XML.as_bytes(), &ca, &key).unwrap();
    let bundle = format!("{other_ca}{ca}");
    let verifier = CmsPkcs7Verifier::new(bundle.as_bytes()).unwrap();
    assert_eq!(verifier.verify_and_extract(&p7s).unwrap(), XML.as_bytes());
}

#[test]
fn rfc_5652_a_content_changed_after_signing_fails_the_message_digest() {
    let (ca, key) = permissions_ca("Permissions CA");
    let p7s = sign_xml_opaque(XML.as_bytes(), &ca, &key).unwrap();
    let changed = tamper(&p7s, b"Chatter");
    let verifier = CmsPkcs7Verifier::new(ca.as_bytes()).unwrap();
    assert!(verifier.verify_and_extract(&changed).is_err());
    // The untouched document still verifies: the refusal is the change.
    assert!(verifier.verify_and_extract(&p7s).is_ok());
}

#[test]
fn rfc_5751_an_opaque_document_openssl_signed_gives_back_the_canonical_text() {
    let verifier = CmsPkcs7Verifier::new(OPENSSL_CA).unwrap();
    let recovered = verifier.verify_and_extract(OPENSSL_SIGNED).unwrap();
    // `-text` signs `Content-Type: text/plain` and the text with CRLF line
    // ends (RFC 5751 §3.1.1); the verifier hands back the text without the
    // header.
    let canonical = std::str::from_utf8(OPENSSL_XML)
        .unwrap()
        .replace('\n', "\r\n");
    assert_eq!(std::str::from_utf8(&recovered).unwrap(), canonical);
}

#[test]
fn rfc_5652_a_document_changed_after_openssl_signed_it_is_refused() {
    let verifier = CmsPkcs7Verifier::new(OPENSSL_CA).unwrap();
    assert!(verifier.verify_and_extract(OPENSSL_TAMPERED).is_err());
}

#[test]
fn a_ca_file_without_a_certificate_is_an_error() {
    for ca in [&b""[..], &b"not a certificate\n"[..]] {
        let err = CmsPkcs7Verifier::new(ca).expect_err("no trust anchor");
        assert!(!err.to_string().is_empty(), "`verifier: {{e}}` is printed");
    }
    // The key of a CA is not its certificate.
    let (_, key) = permissions_ca("Permissions CA");
    assert!(CmsPkcs7Verifier::new(key.as_bytes()).is_err());
}
