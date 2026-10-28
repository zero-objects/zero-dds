// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-Security 1.2 §9.4.2.9.2 `validate_remote_permissions` of the
//! [`PermissionsAccessControl`] plugin: the remote permissions document is
//! signed by the Permissions CA (§9.4.1.2.1) and the grant is the one of
//! the remote identity certificate's subject (§9.4.1.2.3.1).

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_security::access_control::AccessControlPlugin;
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::error::SecurityErrorKind;
use zerodds_security::token::DataHolder;
use zerodds_security_permissions::PermissionsAccessControl;

const CA_PEM: &[u8] = include_bytes!("fixtures/permissions_ca_cert.pem");
const WRONG_CA_PEM: &[u8] = include_bytes!("fixtures/wrong_ca_cert.pem");
const SIGNED: &[u8] = include_bytes!("fixtures/valid_permissions.smime.p7s");
const TAMPERED: &[u8] = include_bytes!("fixtures/tampered_permissions.smime.p7s");
const PLAIN_XML: &[u8] = include_bytes!("fixtures/valid_permissions_xml.txt");
const ALICE: &[u8] = include_bytes!("fixtures/alice_identity_cert.der");
const BOB: &[u8] = include_bytes!("fixtures/bob_identity_cert.der");

fn armed(ca: &[u8]) -> PermissionsAccessControl {
    let mut plugin = PermissionsAccessControl::new();
    plugin.arm_remote_permissions_verifier(ca).unwrap();
    plugin
}

fn validate(
    plugin: &mut PermissionsAccessControl,
    token: &[u8],
    credential: &[u8],
) -> Result<(), (SecurityErrorKind, String)> {
    let token_holder = DataHolder::new("DDS:Access:Permissions:1.2");
    let credential_holder = DataHolder::new("DDS:Auth:PKI-DH:1.0")
        .with_binary_property("c.id", credential.to_vec())
        .with_binary_property("c.perm", token.to_vec());
    plugin
        .validate_remote_permissions(
            IdentityHandle(1),
            IdentityHandle(2),
            &token_holder,
            &credential_holder,
        )
        .map(|_| ())
        .map_err(|e| (e.kind, e.to_string()))
}

#[test]
fn a_signed_document_with_a_grant_for_the_certificates_subject_is_accepted() {
    let mut plugin = armed(CA_PEM);
    assert_eq!(validate(&mut plugin, SIGNED, ALICE), Ok(()));
}

#[test]
fn without_an_armed_permissions_ca_a_remote_token_is_refused() {
    let mut plugin = PermissionsAccessControl::new();
    let (kind, message) = validate(&mut plugin, SIGNED, ALICE).unwrap_err();
    assert_eq!(kind, SecurityErrorKind::AccessDenied);
    assert!(message.contains("no permissions CA is armed"), "{message}");
}

#[test]
fn an_unsigned_document_is_refused_even_when_its_xml_is_valid() {
    let mut plugin = armed(CA_PEM);
    let (kind, message) = validate(&mut plugin, PLAIN_XML, ALICE).unwrap_err();
    assert_eq!(kind, SecurityErrorKind::AccessDenied);
    assert!(message.contains("does not verify"), "{message}");
}

#[test]
fn a_tampered_document_and_another_ca_are_refused() {
    let mut plugin = armed(CA_PEM);
    assert!(validate(&mut plugin, TAMPERED, ALICE).is_err());
    let mut other = armed(WRONG_CA_PEM);
    assert!(validate(&mut other, SIGNED, ALICE).is_err());
}

#[test]
fn a_certificate_whose_subject_has_no_grant_is_refused() {
    let mut plugin = armed(CA_PEM);
    let (kind, message) = validate(&mut plugin, SIGNED, BOB).unwrap_err();
    assert_eq!(kind, SecurityErrorKind::AccessDenied);
    assert!(
        message.contains("names the certificate's subject"),
        "{message}"
    );
}

#[test]
fn a_credential_that_is_no_certificate_is_refused() {
    let mut plugin = armed(CA_PEM);
    assert!(validate(&mut plugin, SIGNED, b"CN=alice").is_err());
}
