//! C3.2 — Integration-Tests fuer den `CmsPkcs7Verifier`.
//!
//! Fixtures werden mit OpenSSL erzeugt (siehe
//! `tests/fixtures/generate_fixtures.sh`) und sind im Repo committed,
//! damit der Test ohne OpenSSL-Build-Time-Dependency laeuft.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_security::properties::{Property, PropertyList};
use zerodds_security_permissions::{
    CmsPkcs7Verifier, PROP_PERMISSIONS_CA, PermissionsError, XmlSignatureVerifier,
    open_signed_permissions,
};

const CA_PEM: &[u8] = include_bytes!("fixtures/permissions_ca_cert.pem");
const WRONG_CA_PEM: &[u8] = include_bytes!("fixtures/wrong_ca_cert.pem");

const VALID_OPAQUE: &[u8] = include_bytes!("fixtures/valid_permissions.smime.p7s");
const VALID_MULTIPART: &[u8] = include_bytes!("fixtures/valid_permissions_multipart.smime.p7s");
const VALID_PEM: &[u8] = include_bytes!("fixtures/valid_permissions_pem.p7s");
const VALID_XML: &[u8] = include_bytes!("fixtures/valid_permissions_xml.txt");
const WRONG_CA: &[u8] = include_bytes!("fixtures/wrong_ca_permissions.smime.p7s");
const TAMPERED: &[u8] = include_bytes!("fixtures/tampered_permissions.smime.p7s");
const INTERMEDIATE: &[u8] = include_bytes!("fixtures/intermediate_chain.smime.p7s");
const EXPIRED: &[u8] = include_bytes!("fixtures/expired_signer.smime.p7s");
const FUTURE: &[u8] = include_bytes!("fixtures/future_signer.smime.p7s");
const RSA_PKCS1: &[u8] = include_bytes!("fixtures/rsa_pkcs1_signer.smime.p7s");

#[test]
fn happy_path_opaque_signed_passes() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let xml = verifier
        .verify_and_extract(VALID_OPAQUE)
        .expect("verify valid opaque");
    let s = std::str::from_utf8(&xml).unwrap();
    assert!(s.contains("CN=alice"), "extracted xml: {s}");
    assert!(s.contains("Chatter"));
}

#[test]
fn happy_path_multipart_signed_passes() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let xml = verifier
        .verify_and_extract(VALID_MULTIPART)
        .expect("verify valid multipart");
    let s = std::str::from_utf8(&xml).unwrap();
    assert!(s.contains("CN=alice"));
}

#[test]
fn happy_path_then_open_signed_permissions_parses() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let perms = open_signed_permissions(VALID_OPAQUE, &verifier).expect("open");
    assert_eq!(perms.grants.len(), 1);
    assert_eq!(perms.grants[0].subject_name, "CN=alice");
    assert!(
        perms.grants[0]
            .allow_publish_topics
            .iter()
            .any(|t| t == "Chatter")
    );
}

#[test]
fn pem_pkcs7_detached_with_explicit_content_passes() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM)
        .expect("ca pem")
        .with_detached_content(VALID_XML.to_vec());
    let xml = verifier
        .verify_and_extract(VALID_PEM)
        .expect("verify pem pkcs7");
    // PEM-detached: extracted bytes sind die kanonische CRLF-Form des
    // signierten Contents (RFC 1847). Der originale `permissions.xml`
    // hat LF-Zeilenende; der Verifier liefert CRLF zurueck (und der
    // XML-Parser ist whitespace-tolerant).
    let s = std::str::from_utf8(&xml).unwrap();
    assert!(s.contains("CN=alice"));
    assert!(s.contains("\r\n") || !s.contains('\n'));
}

#[test]
fn pem_pkcs7_without_detached_content_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let err = verifier.verify_and_extract(VALID_PEM).unwrap_err();
    assert!(
        matches!(err, PermissionsError::Malformed(ref m) if m.contains("detached")),
        "unexpected: {err}"
    );
}

#[test]
fn wrong_ca_signature_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let err = verifier.verify_and_extract(WRONG_CA).unwrap_err();
    assert!(
        matches!(err, PermissionsError::Malformed(ref m) if m.contains("ca chain")),
        "unexpected error: {err}"
    );
}

#[test]
fn wrong_ca_accepts_when_using_its_own_ca() {
    // Sanity-check: wenn der Caller die rogue-CA als Trust-Anchor
    // setzt, geht die Validation durch (Cyclone-Live-Workflow waere
    // wenn ein Op die Permissions-CA versehentlich tauscht).
    let verifier = CmsPkcs7Verifier::new(WRONG_CA_PEM).expect("rogue ca pem");
    let xml = verifier
        .verify_and_extract(WRONG_CA)
        .expect("rogue verifies its own");
    let s = std::str::from_utf8(&xml).unwrap();
    assert!(s.contains("CN=alice"));
}

#[test]
fn tampered_xml_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let err = verifier.verify_and_extract(TAMPERED).unwrap_err();
    // Tampering kann auf zwei Arten kollidieren:
    //   - DER decode failure (wenn das geflippte Byte den ASN.1-
    //     Container brechen wuerde),
    //   - oder messageDigest/verify mismatch.
    // Beide muessen Malformed sein.
    assert!(
        matches!(err, PermissionsError::Malformed(_)),
        "unexpected: {err}"
    );
}

#[test]
fn truncated_smime_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    // Schneide nach dem Header, lass den Body leer.
    let split = VALID_OPAQUE
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(50)
        + 4;
    let truncated = &VALID_OPAQUE[..split];
    let err = verifier.verify_and_extract(truncated).unwrap_err();
    assert!(
        matches!(err, PermissionsError::Malformed(_)),
        "unexpected: {err}"
    );
}

#[test]
fn missing_content_type_header_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let bad = b"X-Foo: bar\r\n\r\nABCD";
    let err = verifier.verify_and_extract(bad).unwrap_err();
    assert!(matches!(err, PermissionsError::Malformed(ref m) if m.contains("Content-Type")));
}

#[test]
fn multipart_without_two_parts_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    // Boundary deklariert, aber nur eine Boundary-Marke im Body.
    let bad = b"Content-Type: multipart/signed; boundary=\"BB\"\r\n\r\n\
                --BB\r\nbody1\r\n";
    let err = verifier.verify_and_extract(bad).unwrap_err();
    assert!(
        matches!(err, PermissionsError::Malformed(ref m) if m.contains("Boundaries") || m.contains("Body-Parts"))
    );
}

#[test]
fn intermediate_ca_chain_is_accepted() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let xml = verifier
        .verify_and_extract(INTERMEDIATE)
        .expect("verify intermediate");
    let s = std::str::from_utf8(&xml).unwrap();
    assert!(s.contains("CN=alice"));
}

#[test]
fn expired_signer_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let err = verifier.verify_and_extract(EXPIRED).unwrap_err();
    assert!(
        matches!(err, PermissionsError::Malformed(ref m) if m.contains("ca chain")),
        "unexpected: {err}"
    );
}

#[test]
fn future_signer_is_rejected() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let err = verifier.verify_and_extract(FUTURE).unwrap_err();
    assert!(
        matches!(err, PermissionsError::Malformed(ref m) if m.contains("ca chain")),
        "unexpected: {err}"
    );
}

#[test]
fn rsa_pkcs1_signer_is_accepted() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let xml = verifier
        .verify_and_extract(RSA_PKCS1)
        .expect("verify rsa pkcs1");
    let s = std::str::from_utf8(&xml).unwrap();
    assert!(s.contains("CN=alice"));
}

#[test]
fn property_driven_constructor_works() {
    let pem = std::str::from_utf8(CA_PEM).unwrap().to_string();
    let props = PropertyList::new().with(Property::local(PROP_PERMISSIONS_CA, pem));
    let verifier = CmsPkcs7Verifier::from_property_list(&props).expect("from props");
    let xml = verifier
        .verify_and_extract(VALID_OPAQUE)
        .expect("verify via property-list");
    assert!(std::str::from_utf8(&xml).unwrap().contains("CN=alice"));
}

#[test]
fn property_driven_missing_ca_is_rejected() {
    let props = PropertyList::new();
    let err = CmsPkcs7Verifier::from_property_list(&props).unwrap_err();
    assert!(matches!(err, PermissionsError::Malformed(ref m) if m.contains(PROP_PERMISSIONS_CA)));
}

#[test]
fn empty_signature_in_signer_info_is_rejected() {
    // Konstruieren eines Synth-Mini-PKCS#7 mit leerer Signatur ist
    // aufwaendig — wir testen die Code-Path indirekt: nimm das
    // valid-opaque-File und ersetze die Signatur-Bytes durch Nullen
    // gleicher Laenge. Das produziert einen DER-syntaktisch validen
    // Container mit zerstoerter Signatur — der ring-Verify schlaegt
    // fehl, was als Malformed kommt.
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let mut bytes = VALID_OPAQUE.to_vec();
    // Brute: alle `0x30` SEQUENCE-Tags lassen wir, aber wir flippen
    // die letzten 64 Body-Bytes — landen typisch in der base64-
    // Signatur-Region.
    let len = bytes.len();
    for b in &mut bytes[len - 200..len - 50] {
        *b = match *b {
            b'A'..=b'Z' => b'A',
            b'a'..=b'z' => b'a',
            b'0'..=b'9' => b'0',
            b'+' => b'+',
            b'/' => b'/',
            b'=' => b'=',
            other => other,
        };
    }
    let err = verifier.verify_and_extract(&bytes).unwrap_err();
    assert!(matches!(err, PermissionsError::Malformed(_)));
}

#[test]
fn ca_pem_parser_accepts_multi_anchor_bundle() {
    // CA + WRONG_CA in einer Datei → beide werden Trust-Anchors.
    let mut combined = CA_PEM.to_vec();
    combined.extend_from_slice(WRONG_CA_PEM);
    let verifier = CmsPkcs7Verifier::new(&combined).expect("multi-anchor");
    // Beide Inputs muessen jetzt validieren.
    assert!(verifier.verify_and_extract(VALID_OPAQUE).is_ok());
    assert!(verifier.verify_and_extract(WRONG_CA).is_ok());
}

#[test]
fn empty_input_is_rejected_cleanly() {
    let verifier = CmsPkcs7Verifier::new(CA_PEM).expect("ca pem");
    let err = verifier.verify_and_extract(b"").unwrap_err();
    assert!(matches!(err, PermissionsError::Malformed(_)));
}
