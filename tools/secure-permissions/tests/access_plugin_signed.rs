// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 §10.4.3 Table 70 on real signed documents: the builtin
//! `DDS:Access:Permissions` plugin validates the signed governance and
//! permissions of the local participant and of a remote one against the
//! Permissions CA, binds a grant to the subject of an identity certificate,
//! builds the tokens of §10.4.2 and, in the gate of a `SecurityProfile`,
//! decides what the runtime asks. The documents are signed here with
//! `zerodds-secure-permissions`, the certificates minted with rcgen; nothing
//! is a fixture.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rcgen::{CertificateParams, IsCa, KeyPair};
use zerodds_secure_permissions::sign_xml_opaque;
use zerodds_security::access_control::{
    AccessControlPlugin, AccessDecision, EndpointRequest, ParticipantRequest, RemoteReaderDecision,
    TopicRequest,
};
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::error::SecurityErrorKind;
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security::token::DataHolder;
use zerodds_security_permissions::{
    PROP_GOVERNANCE, PROP_IDENTITY_CERTIFICATE, PROP_PERM_CERT, PROP_PERMISSIONS_XML,
    PermissionsAccessControl, minor_code,
};
use zerodds_security_runtime::{
    EndpointAccess, RemotePermissions, SecurityProfile, SecurityProfileConfig,
    arm_endpoint_access_control,
};

const GOVERNANCE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds><domain_access_rules><domain_rule>
  <domains><id>0</id></domains>
  <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
  <enable_join_access_control>true</enable_join_access_control>
  <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
  <liveliness_protection_kind>SIGN</liveliness_protection_kind>
  <rtps_protection_kind>SIGN</rtps_protection_kind>
  <topic_access_rules>
    <topic_rule><topic_expression>Open</topic_expression>
      <enable_discovery_protection>false</enable_discovery_protection>
      <enable_liveliness_protection>false</enable_liveliness_protection>
      <enable_read_access_control>false</enable_read_access_control>
      <enable_write_access_control>false</enable_write_access_control>
      <metadata_protection_kind>NONE</metadata_protection_kind>
      <data_protection_kind>NONE</data_protection_kind></topic_rule>
    <topic_rule><topic_expression>*</topic_expression>
      <enable_discovery_protection>true</enable_discovery_protection>
      <enable_liveliness_protection>false</enable_liveliness_protection>
      <enable_read_access_control>true</enable_read_access_control>
      <enable_write_access_control>true</enable_write_access_control>
      <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
      <data_protection_kind>ENCRYPT</data_protection_kind></topic_rule>
  </topic_access_rules>
</domain_rule></domain_access_rules></dds>"#;

/// alice publishes `Chatter`; bob subscribes `Chatter`; carol has no grant.
const PERMISSIONS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds><permissions>
  <grant name="alice"><subject_name>CN=alice</subject_name>
    <validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id>0</id></domains>
      <publish><topics><topic>Chatter</topic></topics></publish></allow_rule>
    <default>DENY</default></grant>
  <grant name="bob"><subject_name>CN=bob</subject_name>
    <validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id>0</id></domains>
      <subscribe><topics><topic>Chatter</topic></topics></subscribe></allow_rule>
    <default>DENY</default></grant>
</permissions></dds>"#;

fn mint_ca(name: &str) -> (rcgen::Certificate, KeyPair) {
    let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    (cert, key)
}

/// A CA whose subject name has three attributes in certificate order
/// `C=DE`, `O=x`, `CN=y`, so that the forms of the subject name differ.
fn mint_ca_with_dn() -> (rcgen::Certificate, KeyPair) {
    let mut params = CertificateParams::new(vec!["y".to_string()]).unwrap();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CountryName, "DE");
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "x");
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "y");
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    (cert, key)
}

fn mint_leaf(name: &str, ca: &rcgen::Certificate, ca_key: &KeyPair) -> (String, String) {
    let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, name);
    params.is_ca = IsCa::NoCa;
    let key = KeyPair::generate().unwrap();
    let cert = params.signed_by(&key, ca, ca_key).unwrap();
    (cert.pem(), key.serialize_pem())
}

fn unique_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zerodds_acc_{tag}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The two CAs, the signed documents and the certificates of alice, bob and
/// carol.
struct World {
    identity_ca: rcgen::Certificate,
    identity_ca_key: KeyPair,
    permissions_ca_pem: String,
    governance_signed: String,
    permissions_signed: String,
    alice_pem: String,
    bob_pem: String,
    carol_pem: String,
}

fn world() -> World {
    let (identity_ca, identity_ca_key) = mint_ca("Identity CA");
    let (permissions_ca, permissions_ca_key) = mint_ca_with_dn();
    let permissions_ca_pem = permissions_ca.pem();
    let key_pem = permissions_ca_key.serialize_pem();
    let sign = |xml: &str| {
        String::from_utf8(sign_xml_opaque(xml.as_bytes(), &permissions_ca_pem, &key_pem).unwrap())
            .unwrap()
    };
    World {
        governance_signed: sign(GOVERNANCE_XML),
        permissions_signed: sign(PERMISSIONS_XML),
        alice_pem: mint_leaf("alice", &identity_ca, &identity_ca_key).0,
        bob_pem: mint_leaf("bob", &identity_ca, &identity_ca_key).0,
        carol_pem: mint_leaf("carol", &identity_ca, &identity_ca_key).0,
        identity_ca,
        identity_ca_key,
        permissions_ca_pem,
    }
}

/// The properties of the local participant `identity_pem` of Table 63 and
/// Table 50.
fn local_properties(w: &World, identity_pem: &str) -> PropertyList {
    PropertyList::new()
        .with(Property::local(
            PROP_PERMISSIONS_XML,
            w.permissions_signed.clone(),
        ))
        .with(Property::local(
            PROP_GOVERNANCE,
            w.governance_signed.clone(),
        ))
        .with(Property::local(
            "dds.sec.access.permissions_ca",
            w.permissions_ca_pem.clone(),
        ))
        .with(Property::local(
            PROP_IDENTITY_CERTIFICATE,
            identity_pem.to_string(),
        ))
}

fn credential(w: &World, identity_pem: &str) -> DataHolder {
    DataHolder::new("DDS:Auth:PKI-DH:1.0")
        .with_binary_property("c.id", identity_pem.as_bytes().to_vec())
        .with_binary_property("c.perm", w.permissions_signed.as_bytes().to_vec())
}

/// Table 70 `validate_local_permissions`: the signatures of the governance
/// and of the permissions are verified against the Permissions CA, the grant
/// is the one of the identity certificate's subject, and the handle decides
/// as the governance and the grant say.
#[test]
fn the_signed_local_documents_validate_and_decide() {
    let w = world();
    let mut ac = PermissionsAccessControl::new();
    let props = local_properties(&w, &w.alice_pem);
    let alice = ac
        .validate_local_permissions(
            IdentityHandle(1),
            &ParticipantRequest::new(0, [0xA1; 16], &props),
        )
        .expect("alice's documents validate");
    let chatter = EndpointRequest::new(0, "Chatter");
    assert_eq!(
        ac.check_create_datawriter(alice, &chatter).unwrap(),
        AccessDecision::Permit
    );
    assert_eq!(
        ac.check_create_datareader(alice, &chatter).unwrap(),
        AccessDecision::Deny,
        "alice's grant has no <subscribe>"
    );
    assert_eq!(
        ac.check_create_datawriter(alice, &EndpointRequest::new(0, "Open"))
            .unwrap(),
        AccessDecision::Permit,
        "the governance leaves Open without access control"
    );
    assert_eq!(
        ac.check_create_participant(alice, &ParticipantRequest::new(0, [0xA1; 16], &props))
            .unwrap(),
        AccessDecision::Permit
    );
    assert_eq!(
        ac.check_create_participant(alice, &ParticipantRequest::new(5, [0xA1; 16], &props))
            .unwrap(),
        AccessDecision::Deny,
        "no domain rule covers domain 5"
    );
    assert_eq!(
        ac.check_create_topic(alice, &TopicRequest::new(0, "Chatter"))
            .unwrap(),
        AccessDecision::Permit
    );
    assert_eq!(
        ac.check_create_topic(alice, &TopicRequest::new(0, "Secret"))
            .unwrap(),
        AccessDecision::Deny
    );
}

/// Table 70: a document without a grant for the certificate's subject, a
/// signature of another CA and a tampered governance are errors.
#[test]
fn local_documents_that_do_not_validate_are_refused() {
    let w = world();
    let mut ac = PermissionsAccessControl::new();
    let carol = local_properties(&w, &w.carol_pem);
    let err = ac
        .validate_local_permissions(
            IdentityHandle(1),
            &ParticipantRequest::new(0, [0xC1; 16], &carol),
        )
        .unwrap_err();
    assert_eq!(err.kind, SecurityErrorKind::AccessDenied);
    assert_eq!(err.minor_code, minor_code::NO_GRANT_FOR_SUBJECT);

    // Another Permissions CA does not verify the documents.
    let (other_ca, _) = mint_ca("Other CA");
    let wrong_ca = local_properties(&w, &w.alice_pem).with(Property::local(
        "dds.sec.access.permissions_ca",
        other_ca.pem(),
    ));
    let err = ac
        .validate_local_permissions(
            IdentityHandle(1),
            &ParticipantRequest::new(0, [0xA1; 16], &wrong_ca),
        )
        .unwrap_err();
    assert_eq!(err.minor_code, minor_code::NOT_VERIFIED);

    // The governance changed after it was signed: its first 'A' is another
    // character, which is the signed content or its encoding.
    let mutated = w.governance_signed.replacen('A', "B", 1);
    let props_mutated =
        local_properties(&w, &w.alice_pem).with(Property::local(PROP_GOVERNANCE, mutated));
    assert!(
        ac.validate_local_permissions(
            IdentityHandle(1),
            &ParticipantRequest::new(0, [0xA1; 16], &props_mutated),
        )
        .is_err()
    );
}

/// Table 70 `get_permissions_token` and `get_permissions_credential_token`
/// (Table 65, Table 64): the class and the properties of the Permissions CA;
/// `dds.perm.cert` holds the signed document of the participant.
#[test]
fn the_tokens_are_the_ones_of_tables_64_and_65() {
    let w = world();
    let mut ac = PermissionsAccessControl::new();
    let props = local_properties(&w, &w.alice_pem);
    let alice = ac
        .validate_local_permissions(
            IdentityHandle(1),
            &ParticipantRequest::new(0, [0xA1; 16], &props),
        )
        .unwrap();
    let token = ac.get_permissions_token(alice).unwrap();
    assert_eq!(token.class_id, "DDS:Access:Permissions:1.2");
    let property = |name: &str| {
        token
            .properties
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value.clone())
    };
    // DDS-Security 1.2 §9.4.1.2.3: the string form of a subject name is
    // RFC 4514, the last RDN first.
    assert_eq!(property("dds.perm_ca.sn").as_deref(), Some("CN=y,O=x,C=DE"));
    assert_eq!(
        property("dds.perm_ca.algo").as_deref(),
        Some("ECDSA+P256+SHA256")
    );
    ac.return_permissions_token(token).unwrap();

    let credential = ac.get_permissions_credential_token(alice).unwrap();
    assert_eq!(credential.class_id, "DDS:Access:PermissionsCredential");
    assert_eq!(
        credential
            .properties
            .iter()
            .find(|p| p.name == PROP_PERM_CERT)
            .map(|p| p.value.as_str()),
        Some(w.permissions_signed.as_str())
    );
    ac.return_permissions_credential_token(credential).unwrap();
}

/// Table 70 `validate_remote_permissions` and the remote checks: the
/// AuthenticatedPeerCredentialToken (`c.id`, `c.perm`) is verified, the grant
/// is bob's, and the checks of a remote reader and writer follow it — with
/// the class of the PermissionsToken compared first.
#[test]
fn a_remote_participant_is_validated_and_checked() {
    let w = world();
    let mut ac = PermissionsAccessControl::new();
    let props = local_properties(&w, &w.alice_pem);
    ac.validate_local_permissions(
        IdentityHandle(1),
        &ParticipantRequest::new(0, [0xA1; 16], &props),
    )
    .unwrap();
    let token = DataHolder::new("DDS:Access:Permissions:1.2");
    let bob = ac
        .validate_remote_permissions(
            IdentityHandle(1),
            IdentityHandle(2),
            &token,
            &credential(&w, &w.bob_pem),
        )
        .expect("bob's credential validates");
    let chatter = EndpointRequest::new(0, "Chatter");
    assert_eq!(
        ac.check_remote_datareader(bob, &chatter).unwrap(),
        RemoteReaderDecision::Permit
    );
    assert_eq!(
        ac.check_remote_datawriter(bob, &chatter).unwrap(),
        AccessDecision::Deny,
        "bob may subscribe Chatter, not publish it"
    );
    assert_eq!(
        ac.check_remote_participant(bob, &ParticipantRequest::new(0, [0xB2; 16], &props))
            .unwrap(),
        AccessDecision::Permit
    );
    // c.id of carol, c.perm with grants for alice and bob only.
    let err = ac
        .validate_remote_permissions(
            IdentityHandle(1),
            IdentityHandle(3),
            &token,
            &credential(&w, &w.carol_pem),
        )
        .unwrap_err();
    assert_eq!(err.minor_code, minor_code::NO_GRANT_FOR_SUBJECT);
    // No c.perm in the credential.
    let err = ac
        .validate_remote_permissions(
            IdentityHandle(1),
            IdentityHandle(3),
            &token,
            &DataHolder::new("DDS:Auth:PKI-DH:1.0"),
        )
        .unwrap_err();
    assert_eq!(err.minor_code, minor_code::NO_CREDENTIAL);
    // A token of another major version fails the checks of Table 70.
    let other = ac
        .validate_remote_permissions(
            IdentityHandle(1),
            IdentityHandle(2),
            &DataHolder::new("DDS:Access:Permissions:2.0"),
            &credential(&w, &w.bob_pem),
        )
        .unwrap();
    assert_eq!(
        ac.check_remote_datareader(other, &chatter).unwrap(),
        RemoteReaderDecision::Deny
    );
}

/// A remote document is not read when the plugin knows no Permissions CA, or
/// when another one signed it.
#[test]
fn a_remote_document_needs_the_permissions_ca() {
    let w = world();
    let mut ac = PermissionsAccessControl::new();
    let token = DataHolder::new("DDS:Access:Permissions:1.2");
    let err = ac
        .validate_remote_permissions(
            IdentityHandle(1),
            IdentityHandle(2),
            &token,
            &credential(&w, &w.bob_pem),
        )
        .unwrap_err();
    assert_eq!(err.minor_code, minor_code::NO_VERIFIER);
    let (other, _) = mint_ca("Other CA");
    ac.arm_remote_permissions_verifier(other.pem().as_bytes())
        .unwrap();
    let err = ac
        .validate_remote_permissions(
            IdentityHandle(1),
            IdentityHandle(2),
            &token,
            &credential(&w, &w.bob_pem),
        )
        .unwrap_err();
    assert_eq!(err.minor_code, minor_code::NOT_VERIFIED);
}

/// The gate of a profile built from files: armed with the plugin built from
/// the signed documents, it gives the config of the participant, the tokens,
/// the checks and the remote validation through the plugin.
#[test]
fn a_profiles_gate_goes_through_the_builtin_plugin() {
    let w = world();
    let dir = unique_dir("gate");
    let (alice_pem, alice_key) = mint_leaf("alice", &w.identity_ca, &w.identity_ca_key);
    let write = |name: &str, bytes: &[u8]| -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    };
    let cfg = SecurityProfileConfig {
        domain_id: 0,
        identity_ca_pem: write("identity_ca.pem", w.identity_ca.pem().as_bytes()).into(),
        identity_cert_pem: write("identity.pem", alice_pem.as_bytes()).into(),
        identity_key_pem: write("identity_key.pem", alice_key.as_bytes()).into(),
        permissions_ca_pem: write("permissions_ca.pem", w.permissions_ca_pem.as_bytes()).into(),
        governance_p7s: write("governance.p7s", w.governance_signed.as_bytes()).into(),
        permissions_p7s: write("permissions.p7s", w.permissions_signed.as_bytes()).into(),
        identity_key_password: None,
        identity_key_password_reading: Default::default(),
        symmetric_cipher_bits: 256,
        algo_name_nul: Default::default(),
        algorithm_names: Default::default(),
        subject_name_form: Default::default(),
        origin_authentication_fallback: false,
        max_blocks_per_session: None,
        partition_matching: zerodds_security_permissions::PartitionMatching::Subset,
        plaintext_loopback: false,
        compatibility_checks: true,
        kx_key_fallback: false,
        identity_crl: None,
    };
    let profile = SecurityProfile::from_files(&cfg, [0xA1; 16]).expect("the profile builds");
    let gate = &profile.gate;
    assert!(!gate.has_permissions().unwrap(), "not armed yet");
    arm_endpoint_access_control(&profile).expect("arming the gate");
    assert!(gate.has_permissions().unwrap());
    assert!(!gate.application_access_control().unwrap());

    // §9.4.2.9.22 get_participant_security_config, from the governance.
    let config = gate.participant_security_config().unwrap();
    assert!(!config.allow_unauthenticated_participants);
    assert!(config.is_access_protected);
    assert!(config.is_rtps_axk_protected);
    assert!(config.is_discovery_protected);
    assert!(config.is_liveliness_protected);
    assert!(!config.is_rtps_psk_protected && !config.is_key_revision_enabled);
    assert!(gate.is_access_protected().unwrap());
    assert!(!gate.allow_unauthenticated().unwrap());

    // The tokens of §10.4.2: the class, the Permissions CA, the signed document.
    let token = DataHolder::from_cdr_le(&gate.local_permissions_token().unwrap()).unwrap();
    assert_eq!(token.class_id, "DDS:Access:Permissions:1.2");
    assert!(token.properties.iter().any(|p| p.name == "dds.perm_ca.sn"));
    let credential_token =
        DataHolder::from_cdr_le(&gate.local_permissions_credential_token().unwrap()).unwrap();
    assert_eq!(
        credential_token
            .properties
            .iter()
            .find(|p| p.name == PROP_PERM_CERT)
            .map(|p| p.value.as_str()),
        Some(w.permissions_signed.as_str())
    );

    // The local checks, through the plugin.
    let chatter = EndpointRequest::new(0, "Chatter");
    assert_eq!(
        gate.check_create_datawriter(&chatter).unwrap(),
        EndpointAccess::Permit
    );
    assert_eq!(
        gate.check_create_datareader(&chatter).unwrap(),
        EndpointAccess::Deny
    );
    assert!(gate.check_create_participant(&PropertyList::new()).unwrap());
    assert_eq!(
        gate.endpoint_security_config(false, &chatter)
            .unwrap()
            .attributes_mask()
            & zerodds_security::access_control::endpoint_attributes_mask::IS_PAYLOAD_PROTECTED,
        zerodds_security::access_control::endpoint_attributes_mask::IS_PAYLOAD_PROTECTED
    );

    // §9.4.2.9.2: bob's credential through the gate; his reader is allowed,
    // his writer is not; a peer without a validated document is denied.
    let bob_key: [u8; 12] = [0xB2; 12];
    assert_eq!(
        gate.set_remote_permissions(
            bob_key,
            IdentityHandle(2),
            Some("DDS:Access:Permissions:1.2"),
            &credential(&w, &w.bob_pem)
        )
        .unwrap(),
        RemotePermissions::Granted
    );
    assert_eq!(
        gate.check_remote_datareader(bob_key, &chatter).unwrap(),
        EndpointAccess::Permit
    );
    assert_eq!(
        gate.check_remote_datawriter(bob_key, &chatter).unwrap(),
        EndpointAccess::Deny
    );
    assert!(gate.check_remote_participant(bob_key).unwrap());
    let stranger = [0x99u8; 12];
    assert_eq!(
        gate.check_remote_datareader(stranger, &chatter).unwrap(),
        EndpointAccess::Deny
    );
    assert!(!gate.check_remote_participant(stranger).unwrap());
    assert_eq!(
        gate.set_remote_permissions(
            [0x98; 12],
            IdentityHandle(3),
            Some("DDS:Access:Permissions:1.2"),
            &credential(&w, &w.carol_pem)
        )
        .unwrap(),
        RemotePermissions::NoGrantForSubject
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// The forms of `dds.perm_ca.sn`. The expected strings come from the sources
/// of the vendors, not from this code: Fast DDS 3.6 compares the received
/// value with `X509_NAME_oneline(name, 0, 0)` of its Permissions CA
/// (`FileProvider.cpp:82`, `Permissions.cpp:1007`), which is
/// `/C=DE/O=x/CN=y`; OpenDDS writes `X509_NAME_print_ex(.., XN_FLAG_ONELINE)`
/// (`AuthenticationBuiltInImpl.cpp`), `C = DE, O = x, CN = y`; the
/// specification (§9.4.1.2.3) says RFC 4514, `CN=y,O=x,C=DE`.
#[test]
fn the_subject_name_of_the_permissions_ca_has_the_form_that_is_set() {
    use zerodds_security::SubjectNameForm;
    let w = world();
    let token_sn = |form: Option<SubjectNameForm>| {
        let mut ac = PermissionsAccessControl::new();
        if let Some(form) = form {
            ac.set_subject_name_form(form);
        }
        let props = local_properties(&w, &w.alice_pem);
        let alice = ac
            .validate_local_permissions(
                IdentityHandle(1),
                &ParticipantRequest::new(0, [0xA1; 16], &props),
            )
            .unwrap();
        let token = ac.get_permissions_token(alice).unwrap();
        token
            .properties
            .iter()
            .find(|p| p.name == "dds.perm_ca.sn")
            .map(|p| p.value.to_string())
            .unwrap()
    };
    assert_eq!(token_sn(None), "CN=y,O=x,C=DE", "the default is RFC 4514");
    assert_eq!(token_sn(Some(SubjectNameForm::Rfc4514)), "CN=y,O=x,C=DE");
    assert_eq!(
        token_sn(Some(SubjectNameForm::X509NameOneline)),
        "/C=DE/O=x/CN=y"
    );
    assert_eq!(
        token_sn(Some(SubjectNameForm::XnFlagOneline)),
        "C = DE, O = x, CN = y"
    );
}
