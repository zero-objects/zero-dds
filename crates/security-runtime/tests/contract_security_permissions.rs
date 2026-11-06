// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-runtime` with
//! `zerodds-security-permissions` (internal/release/rc8-test-suite.md,
//! "Crate tier").
//!
//! What this crate uses of `zerodds-security-permissions`, from its
//! non-test sources:
//! - `Governance` (re-exported by `lib.rs`), from `parse_governance_xml`
//!   (`profile.rs`): `find_domain_rule` with a `Domain { id, tag }`
//!   (`shared.rs`, `gate.rs`, `engine.rs`, `profile.rs`), `find_topic_rule`,
//!   `effective_topic_rule`, `has_topic_rule`, `read_access_controlled`,
//!   `write_access_controlled`, `check_create_participant`,
//!   `check_create_datawriter`, `check_create_datareader`,
//!   `set_partition_matching`, `delegation_profiles`; the fields
//!   `rtps_protection_kind`, `discovery_protection_kind`,
//!   `liveliness_protection_kind`, `rtps_psk_protection_kind` of a
//!   `DomainRule` and `metadata_protection_kind`, `data_protection_kind`
//!   of a `TopicRule`.
//! - `ProtectionKind` (`builtin_attributes.rs`, `gate_crypto.rs`,
//!   `policy.rs`, `profile.rs`, `qos_properties.rs`): its five variants,
//!   `Default`, `as_str` (the value of the protection-kind properties for
//!   the Cryptographic plugin).
//! - `PartitionMatching` (re-exported; `Subset`, `Legacy`).
//! - `CmsPkcs7Verifier::new` and `open_signed_permissions` (`profile.rs`),
//!   then `Permissions::for_subject` with the subject attributes of the
//!   identity certificate; `Permissions`, `PermissionsError`,
//!   `PermissionsAccessControl`, `minor_code` (`shared.rs`).
//! - `PROP_PERMISSIONS_XML`, `PROP_GOVERNANCE`, `PROP_PERMISSIONS_CA`,
//!   `PROP_IDENTITY_CERTIFICATE` (`shared.rs`), `PROP_TOKEN_CLASS_ID`
//!   (`qos_properties.rs`), `psk_access::{PROP_RTPS_PSK_PROTECTION_KIND,
//!   RtpsPskProtection}` (`qos_properties.rs`).
//! - `DelegationProfile`, `PeerClass`, `ValidatedChain`,
//!   `cn_pattern_match`, `validate_chain` (`peer_class.rs`) and
//!   `EdgeIdentityConfig` (`gateway_bridge.rs`).
//!
//! Oracles: DDS-Security 1.2 §10.4.1.2.3 (the governance XSD and its
//! ProtectionKind spellings), §10.4.1.2.5 and §10.4.1.2.7 (the first domain
//! rule whose ids and tags match applies; no `<tag>` is the tag ""),
//! §10.4.1.2.6 and §10.4.1.2.6.1 (the first matching topic rule applies;
//! fnmatch), §10.6 (the builtin rule of the logging topic), Table 50,
//! Table 63 and Table 84 (property names; `rtps_psk_protection_kind` is
//! NONE, SIGN or ENCRYPT); the S/MIME fixtures of
//! `crates/security-permissions/tests/fixtures/` signed by OpenSSL
//! (`generate_fixtures.sh`; `openssl smime -verify -CAfile
//! permissions_ca_cert.pem` accepts `valid_permissions.smime.p7s` and
//! refuses `wrong_ca_permissions.smime.p7s`); RFC 8032 §7.1 TEST 1 with an
//! Ed25519 signature over the documented layout of a delegation link,
//! computed with Python `cryptography` 43.0.3; and the rustdoc of
//! `validate_chain`, `ValidatedChain` and `cn_pattern_match`.
#![allow(clippy::unwrap_used)]

use zerodds_security_permissions::psk_access::{PROP_RTPS_PSK_PROTECTION_KIND, RtpsPskProtection};
use zerodds_security_permissions::{
    CmsPkcs7Verifier, DelegationProfile, Domain, PROP_GOVERNANCE, PROP_IDENTITY_CERTIFICATE,
    PROP_PERMISSIONS_CA, PROP_PERMISSIONS_XML, PROP_TOKEN_CLASS_ID, PartitionMatching,
    ProtectionKind, TrustAnchor, cn_pattern_match, open_signed_permissions, parse_governance_xml,
    validate_chain,
};
use zerodds_security_pki::{
    DelegationChain, DelegationLink, SignatureAlgorithm, subject_attributes,
};

/// A governance document of the XSD of §10.4.1.2.3: domains 0 and 10 to
/// 20 without a tag, then domain 0 with tags matching `AGV/*`, then a rule
/// for domain 0 that the first one shadows.
const GOVERNANCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains>
        <id>0</id>
        <id_range><min>10</min><max>20</max></id_range>
      </domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>true</enable_join_access_control>
      <enable_key_revision>false</enable_key_revision>
      <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
      <liveliness_protection_kind>SIGN</liveliness_protection_kind>
      <rtps_protection_kind>SIGN_WITH_ORIGIN_AUTHENTICATION</rtps_protection_kind>
      <rtps_psk_protection_kind>ENCRYPT</rtps_psk_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>Square*</topic_expression>
          <enable_discovery_protection>true</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>true</enable_read_access_control>
          <enable_write_access_control>true</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>Sq?are</topic_expression>
          <enable_discovery_protection>false</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>false</enable_read_access_control>
          <enable_write_access_control>false</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>Circle</topic_expression>
          <enable_discovery_protection>true</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>false</enable_read_access_control>
          <enable_write_access_control>true</enable_write_access_control>
          <metadata_protection_kind>SIGN</metadata_protection_kind>
          <data_protection_kind>SIGN</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
    <domain_rule>
      <domains>
        <id>0</id>
        <tag_expression>AGV/*</tag_expression>
      </domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>false</enable_join_access_control>
      <enable_key_revision>false</enable_key_revision>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
      <rtps_psk_protection_kind>NONE</rtps_psk_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>false</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>false</enable_read_access_control>
          <enable_write_access_control>false</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
    <domain_rule>
      <domains><id>0</id></domains>
      <allow_unauthenticated_participants>true</allow_unauthenticated_participants>
      <enable_join_access_control>false</enable_join_access_control>
      <enable_key_revision>false</enable_key_revision>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <rtps_psk_protection_kind>NONE</rtps_psk_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>false</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>false</enable_read_access_control>
          <enable_write_access_control>false</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#;

#[test]
fn security_10_4_1_2_7_the_first_domain_rule_whose_ids_and_tags_match_applies() {
    let governance = parse_governance_xml(GOVERNANCE).unwrap();
    let rtps = |domain: Domain<'_>| {
        governance
            .find_domain_rule(domain)
            .map(|r| r.rtps_protection_kind)
    };
    // The first rule: ids 0 and 10..=20, the tag "" (no <tag> element).
    for id in [0, 10, 15, 20] {
        assert_eq!(
            rtps(Domain::from(id)),
            Some(ProtectionKind::SignWithOriginAuthentication),
            "{id}"
        );
    }
    // A participant with a tag matches the rule of its tag expression.
    assert_eq!(
        rtps(Domain::from((0, "AGV/7"))),
        Some(ProtectionKind::Encrypt)
    );
    // No rule: another id, or a tag no rule names.
    assert_eq!(rtps(Domain::from(9)), None);
    assert_eq!(rtps(Domain::from(21)), None);
    assert_eq!(rtps(Domain::from((15, "AGV/7"))), None);
    assert_eq!(
        rtps(Domain {
            id: 0,
            tag: "Robot15"
        }),
        None
    );

    let rule = governance.find_domain_rule(0).unwrap();
    assert_eq!(rule.discovery_protection_kind, ProtectionKind::Encrypt);
    assert_eq!(rule.liveliness_protection_kind, ProtectionKind::Sign);
    assert_eq!(rule.rtps_psk_protection_kind, ProtectionKind::Encrypt);
}

#[test]
fn security_10_4_1_2_6_the_first_matching_topic_rule_applies() {
    let governance = parse_governance_xml(GOVERNANCE).unwrap();
    let kinds = |topic: &str| {
        governance
            .find_topic_rule(0, topic)
            .map(|r| (r.metadata_protection_kind, r.data_protection_kind))
    };
    // fnmatch: `Square*` matches "Square" and "SquareA" and comes before
    // `Sq?are`, which would match "Square" too.
    assert_eq!(
        kinds("Square"),
        Some((
            ProtectionKind::EncryptWithOriginAuthentication,
            ProtectionKind::Encrypt
        ))
    );
    assert_eq!(kinds("SquareA").map(|k| k.1), Some(ProtectionKind::Encrypt));
    assert_eq!(
        kinds("Sqbare"),
        Some((ProtectionKind::None, ProtectionKind::None))
    );
    assert_eq!(
        kinds("Circle"),
        Some((ProtectionKind::Sign, ProtectionKind::Sign))
    );
    // "Circle" is no pattern of "Circles"; no rule matches.
    assert_eq!(kinds("Circles"), None);
    assert!(!governance.has_topic_rule(0, "Circles"));
    assert!(governance.write_access_controlled(0, "Circle"));
    assert!(!governance.read_access_controlled(0, "Circle"));
    // Without a rule, the effective rule protects nothing.
    let none = governance.effective_topic_rule(0, "Circles");
    assert_eq!(none.data_protection_kind, ProtectionKind::None);

    // §10.6: the builtin rule of the logging topic wins over the document.
    let log = governance.effective_topic_rule(0, "DDS:Security:LogTopicV2");
    assert!(log.enable_read_access_control);
    assert!(!log.enable_write_access_control);
    assert_eq!(log.metadata_protection_kind, ProtectionKind::Sign);
    assert_eq!(log.data_protection_kind, ProtectionKind::Encrypt);
    assert!(governance.has_topic_rule(0, "DDS:Security:LogTopic"));
}

#[test]
fn security_10_4_1_2_3_protection_kinds_are_the_spellings_of_the_xsd() {
    // `profile.rs` passes `as_str` to the Cryptographic plugin.
    let all = [
        (ProtectionKind::None, "NONE"),
        (ProtectionKind::Sign, "SIGN"),
        (ProtectionKind::Encrypt, "ENCRYPT"),
        (
            ProtectionKind::SignWithOriginAuthentication,
            "SIGN_WITH_ORIGIN_AUTHENTICATION",
        ),
        (
            ProtectionKind::EncryptWithOriginAuthentication,
            "ENCRYPT_WITH_ORIGIN_AUTHENTICATION",
        ),
    ];
    for (kind, text) in all {
        assert_eq!(kind.as_str(), text);
    }
    assert_eq!(ProtectionKind::default(), ProtectionKind::None);

    // A value outside the enumeration is no governance document: it must
    // not turn into NONE.
    let typo = GOVERNANCE.replacen(
        "<data_protection_kind>ENCRYPT</data_protection_kind>",
        "<data_protection_kind>ENCRYPTED</data_protection_kind>",
        1,
    );
    assert!(parse_governance_xml(&typo).is_err());
    // `data_protection_kind` is a BasicProtectionKind.
    let not_basic = GOVERNANCE.replacen(
        "<data_protection_kind>ENCRYPT</data_protection_kind>",
        "<data_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</data_protection_kind>",
        1,
    );
    assert!(parse_governance_xml(&not_basic).is_err());
}

#[test]
fn the_partition_matching_of_the_profile() {
    let mut governance = parse_governance_xml(GOVERNANCE).unwrap();
    assert_eq!(PartitionMatching::default(), PartitionMatching::Subset);
    governance.set_partition_matching(PartitionMatching::Legacy);
    assert_eq!(governance.partition_matching(), PartitionMatching::Legacy);
    // The gate keeps its own copy (`profile.rs`).
    let copy = governance.clone();
    assert_eq!(copy.partition_matching(), PartitionMatching::Legacy);
}

#[test]
fn the_property_names_of_tables_50_63_and_84() {
    assert_eq!(PROP_PERMISSIONS_XML, "dds.sec.access.permissions");
    assert_eq!(PROP_GOVERNANCE, "dds.sec.access.governance");
    assert_eq!(PROP_PERMISSIONS_CA, "dds.sec.access.permissions_ca");
    assert_eq!(
        PROP_IDENTITY_CERTIFICATE,
        "dds.sec.auth.identity_certificate"
    );
    assert_eq!(
        PROP_RTPS_PSK_PROTECTION_KIND,
        "dds.sec.access.rtps_psk_protection_kind"
    );
    // §7.3.2: a vendor property starts with the vendor's domain.
    assert!(PROP_TOKEN_CLASS_ID.starts_with("org.zerodds."));
    // Table 84: "NONE", "SIGN" or "ENCRYPT".
    assert_eq!(
        RtpsPskProtection::parse("NONE").unwrap(),
        RtpsPskProtection::None
    );
    assert_eq!(
        RtpsPskProtection::parse("SIGN").unwrap(),
        RtpsPskProtection::Sign
    );
    assert_eq!(
        RtpsPskProtection::parse("ENCRYPT").unwrap(),
        RtpsPskProtection::Encrypt
    );
    assert!(RtpsPskProtection::parse("ENCRYPT_WITH_ORIGIN_AUTHENTICATION").is_err());
}

const PERMISSIONS_CA: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/permissions_ca_cert.pem");
const WRONG_CA: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/wrong_ca_cert.pem");
const VALID: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/valid_permissions.smime.p7s");
const WRONG_CA_SIGNED: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/wrong_ca_permissions.smime.p7s");
const TAMPERED: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/tampered_permissions.smime.p7s");
const ALICE: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/alice_identity_cert.der");
const BOB: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/bob_identity_cert.der");

#[test]
fn a_permissions_document_signed_by_the_permissions_ca_opens_for_its_subject() {
    // `profile.rs`: verifier of the CA, the signed document, the grant of
    // the identity certificate's subject.
    let verifier = CmsPkcs7Verifier::new(PERMISSIONS_CA).unwrap();
    let permissions = open_signed_permissions(VALID, &verifier).unwrap();
    let alice = permissions
        .for_subject(&subject_attributes(ALICE).unwrap())
        .unwrap();
    assert_eq!(alice.grants.len(), 1);
    assert_eq!(alice.grants[0].name, "signed-alice");
    assert!(
        permissions
            .for_subject(&subject_attributes(BOB).unwrap())
            .is_none()
    );

    // OpenSSL refuses these two; so does the verifier.
    assert!(open_signed_permissions(WRONG_CA_SIGNED, &verifier).is_err());
    assert!(open_signed_permissions(TAMPERED, &verifier).is_err());
    let wrong = CmsPkcs7Verifier::new(WRONG_CA).unwrap();
    assert!(open_signed_permissions(VALID, &wrong).is_err());
    // No certificate, no verifier.
    assert!(CmsPkcs7Verifier::new(b"no certificate").is_err());
}

/// RFC 8032 §7.1 TEST 1, PUBLIC KEY.
const RFC8032_TEST1_PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

/// Ed25519 signature with the SECRET KEY of RFC 8032 §7.1 TEST 1 over the
/// signing layout of `signed_link()` (Python `cryptography` 43.0.3).
const LINK_SIGNATURE: [u8; 64] = [
    0x92, 0xae, 0x12, 0xee, 0x74, 0xd5, 0xf2, 0x63, 0xb3, 0x2a, 0x8c, 0x82, 0xed, 0x62, 0x35, 0xdf,
    0xd1, 0x14, 0x95, 0x48, 0xfb, 0xc6, 0xd9, 0xfc, 0x80, 0x37, 0x0e, 0x8c, 0x19, 0xcb, 0x25, 0xeb,
    0xa2, 0x11, 0x82, 0x97, 0x1f, 0xee, 0x8c, 0xf3, 0x57, 0x21, 0xc1, 0x2d, 0x49, 0xef, 0xdf, 0x7c,
    0xca, 0xea, 0xbb, 0xd1, 0x7f, 0x42, 0x54, 0x42, 0xa4, 0xeb, 0xf7, 0xee, 0xe9, 0x19, 0x45, 0x0b,
];

/// A link from the gateway `0x11..` to the edge `0x22..` for `sensor/*` in
/// partition `fleet`, valid from 1_700_000_000 to 1_800_000_000.
fn signed_link() -> DelegationLink {
    let mut link = DelegationLink::new(
        [0x11; 16],
        [0x22; 16],
        vec!["sensor/*".to_string()],
        vec!["fleet".to_string()],
        1_700_000_000,
        1_800_000_000,
        SignatureAlgorithm::Ed25519,
    )
    .unwrap();
    link.signature = LINK_SIGNATURE.to_vec();
    link
}

#[test]
fn a_delegation_chain_signed_by_the_trust_anchor_validates() {
    let profile = DelegationProfile::default_with_anchor(
        "gateway".to_string(),
        TrustAnchor {
            subject_guid: [0x11; 16],
            verify_public_key: RFC8032_TEST1_PUBLIC.to_vec(),
            algorithm: SignatureAlgorithm::Ed25519,
        },
    );
    let chain = DelegationChain::new([0x11; 16], vec![signed_link()]).unwrap();
    let no_resolver = |_: &[u8; 16]| None;

    let validated = validate_chain(&chain, &profile, 1_750_000_000, no_resolver).unwrap();
    assert_eq!(validated.origin_guid, [0x11; 16]);
    assert_eq!(validated.edge_guid, [0x22; 16]);
    assert_eq!(validated.chain_depth, 1);
    assert!(validated.allows_topic("sensor/temperature"));
    assert!(!validated.allows_topic("actuator/brake"));
    assert!(validated.allows_partition("fleet"));
    assert!(!validated.allows_partition("depot"));

    // Outside the time window of the link.
    assert!(validate_chain(&chain, &profile, 1_800_000_001, no_resolver).is_err());
    // An origin that is not the delegator of the first link.
    let other_origin = DelegationChain::new([0x33; 16], vec![signed_link()]).unwrap();
    assert!(validate_chain(&other_origin, &profile, 1_750_000_000, no_resolver).is_err());
    // A link changed after signing.
    let mut changed = signed_link();
    changed.allowed_topic_patterns = vec!["*".to_string()];
    let forged = DelegationChain::new([0x11; 16], vec![changed]).unwrap();
    assert!(validate_chain(&forged, &profile, 1_750_000_000, no_resolver).is_err());
}

#[test]
fn certificate_common_name_patterns() {
    // Rustdoc of `cn_pattern_match`: `*` is the only joker and matches any
    // number of characters, `.` included.
    assert!(cn_pattern_match("*.fast.example", "w1.fast.example"));
    assert!(cn_pattern_match("*.fast.example", "a.b.fast.example"));
    assert!(!cn_pattern_match("*.fast.example", "fast.example"));
    assert!(cn_pattern_match("gateway-*", "gateway-7"));
    assert!(!cn_pattern_match("gateway-?", "gateway-7"));
    assert!(cn_pattern_match("", ""));
    assert!(!cn_pattern_match("", "x"));
}
