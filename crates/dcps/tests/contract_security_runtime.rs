// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-security-runtime`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-runtime` (only with its
//! feature `security`, see `Cargo.toml`), from its non-test sources:
//! - `runtime.rs`: `SharedSecurityGate` as `RuntimeConfig::security`, and
//!   about seventy of its operations. The governance answers
//!   (`rtps_protection`, `discovery_protection`, `liveliness_protection`,
//!   `data_protection`, `metadata_protection`, `topic_read_protected`,
//!   `topic_write_protected`, `topic_discovery_protected`,
//!   `topic_liveliness_protected`, `allow_unauthenticated`,
//!   `is_access_protected`) decide what the participant announces and
//!   protects; `register_remote_by_guid_from_secret`,
//!   `encode_kx_datawriter_for` and `decode_kx_datawriter_from` protect the
//!   DCPSParticipantVolatileMessageSecure traffic with the Kx key; the
//!   per-endpoint key operations, the access checks
//!   (`check_create_*`, `check_remote_*`, `check_match`) and the transforms
//!   of user traffic run on the live path and are left to the integration
//!   pillar.
//! - `runtime.rs`: `plugin_attributes::{participant_security_info,
//!   endpoint_security_info, protection_level_of,
//!   endpoint_cipher_required_mask}` for `PID_PARTICIPANT_SECURITY_INFO` and
//!   `PID_ENDPOINT_SECURITY_INFO`; `builtin_attributes::*` for the secure
//!   builtin endpoints; `ProtectionLevel`, `PARTICIPANT_WIDE_TOPIC`,
//!   `EndpointAccess` (`Permit`, `Deny`, `RelayOnly`, `NoPermissions`,
//!   `is_permitted`), `RemotePermissions`, `InboundVerdict`, `IpRange`,
//!   `NetInterface`, `SecurityGateError`, `qos_properties::
//!   PROP_SYMMETRIC_CIPHER`, `arm_endpoint_access_control`, and the
//!   re-exports `LogLevel` and `LoggingPlugin` for
//!   `RuntimeConfig::security_logger`.
//! - `participant.rs` and `factory.rs`: `requests_security`,
//!   `security_bundle_from_properties_with_env`,
//!   `security_bundle_from_enclave_dir_with_env`,
//!   `SecurityPropertyError::JoinDenied`, `SecurityBundle`,
//!   `SecurityProfile`.
//!
//! What it relies on: the gate answers from the domain rule and the first
//! matching topic rule of the governance, the empty topic name standing for
//! the participant; the security info masks have the bits of the
//! specification, so that Cyclone DDS, Fast DDS and OpenDDS match the
//! endpoints; a submessage protected with the Kx key is a SEC_PREFIX,
//! SEC_BODY, SEC_POSTFIX sequence under AES256-GCM that the peer opens and
//! that does not open once changed; only `dds.sec.auth.*` and
//! `dds.sec.access.*` request security; an incomplete property list is an
//! error naming the property; only `EndpointAccess::Permit` permits.
//!
//! Oracles: DDS-Security 1.2 §10.4.1.2.5 and §10.4.1.2.6 (domain rule and
//! topic rules of the governance; the first rule whose `topic_expression`
//! matches applies, `fnmatch` patterns), §9.4.2.4 and §10.4.2.3 (the bits
//! of `ParticipantSecurityAttributesMask`, `EndpointSecurityAttributesMask`
//! and their plugin masks; `IS_VALID` 0x80000000), §7.3.6 and DDSI-RTPS
//! 2.5 §9.4.5.1 (SEC_BODY 0x30, SEC_PREFIX 0x31, SEC_POSTFIX 0x32),
//! §9.5.2 (CryptoHeader of 20 octets, CryptoFooter with a 16-octet
//! `common_mac`; `CRYPTO_TRANSFORMATION_KIND_AES256_GCM` = {0,0,0,4}),
//! §10.5.2.1.2 (the Kx key is AES256-GCM with key id 0), NIST SP 800-38D
//! (GCM: ciphertext as long as the plaintext, a changed ciphertext fails
//! authentication), §7.3.2 and §10.3.1 Table 50 / §10.4.1 Table 63 (the
//! `dds.sec.auth.` and `dds.sec.access.` namespaces), §9.4.2.9.11
//! (`relay_only`), and the rustdoc of `requests_security` (`dds.sec.log.*`
//! does not request security) and `EndpointAccess::is_permitted`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use zerodds_qos::PropertyQosPolicy;
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle, SharedSecretProvider};
use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};
use zerodds_security_permissions::parse_governance_xml;
use zerodds_security_runtime::plugin_attributes::{
    endpoint_security_info, participant_security_info,
};
use zerodds_security_runtime::{
    EndpointAccess, PARTICIPANT_WIDE_TOPIC, ProtectionLevel, SecurityPropertyError,
    SharedSecurityGate, requests_security, security_bundle_from_properties_with_env,
};

/// Domain 0: discovery ENCRYPT, liveliness SIGN, RTPS ENCRYPT with origin
/// authentication; `Open*` open, everything else protected.
const GOVERNANCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds><domain_access_rules><domain_rule>
  <domains><id>0</id></domains>
  <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
  <enable_join_access_control>true</enable_join_access_control>
  <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
  <liveliness_protection_kind>SIGN</liveliness_protection_kind>
  <rtps_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</rtps_protection_kind>
  <topic_access_rules>
    <topic_rule><topic_expression>Open*</topic_expression>
      <enable_discovery_protection>false</enable_discovery_protection>
      <enable_liveliness_protection>false</enable_liveliness_protection>
      <enable_read_access_control>false</enable_read_access_control>
      <enable_write_access_control>false</enable_write_access_control>
      <metadata_protection_kind>NONE</metadata_protection_kind>
      <data_protection_kind>NONE</data_protection_kind></topic_rule>
    <topic_rule><topic_expression>*</topic_expression>
      <enable_discovery_protection>true</enable_discovery_protection>
      <enable_liveliness_protection>true</enable_liveliness_protection>
      <enable_read_access_control>true</enable_read_access_control>
      <enable_write_access_control>false</enable_write_access_control>
      <metadata_protection_kind>SIGN</metadata_protection_kind>
      <data_protection_kind>ENCRYPT</data_protection_kind></topic_rule>
  </topic_access_rules>
</domain_rule></domain_access_rules></dds>"#;

/// The handshake result both peers hold: the same secret and challenges.
struct Handshake;

impl SharedSecretProvider for Handshake {
    fn get_shared_secret(&self, _handle: SharedSecretHandle) -> Option<Vec<u8>> {
        Some(vec![0x33; 32])
    }

    fn get_shared_secret_challenges(
        &self,
        _handle: SharedSecretHandle,
    ) -> Option<([u8; 32], [u8; 32])> {
        Some(([0x01; 32], [0x02; 32]))
    }
}

fn gate(domain_id: u32) -> SharedSecurityGate {
    SharedSecurityGate::new(
        domain_id,
        parse_governance_xml(GOVERNANCE).unwrap(),
        Box::new(AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            Arc::new(Handshake) as Arc<dyn SharedSecretProvider>,
        )),
    )
}

#[test]
fn security_10_4_1_2_5_the_gate_answers_from_the_domain_rule() {
    let domain_0 = gate(0);
    // Origin authentication is a refinement of ENCRYPT.
    assert_eq!(
        domain_0.rtps_protection().unwrap(),
        ProtectionLevel::Encrypt
    );
    assert_eq!(
        domain_0.discovery_protection().unwrap(),
        ProtectionLevel::Encrypt
    );
    assert_eq!(
        domain_0.liveliness_protection().unwrap(),
        ProtectionLevel::Sign
    );
    assert!(!domain_0.allow_unauthenticated().unwrap());
    assert!(domain_0.is_access_protected().unwrap());

    // A domain without a rule protects nothing and admits no one
    // unauthenticated.
    let other = gate(7);
    assert_eq!(other.rtps_protection().unwrap(), ProtectionLevel::None);
    assert_eq!(other.discovery_protection().unwrap(), ProtectionLevel::None);
    assert_eq!(
        other.liveliness_protection().unwrap(),
        ProtectionLevel::None
    );
    assert!(!other.allow_unauthenticated().unwrap());
}

#[test]
fn security_10_4_1_2_6_the_first_matching_topic_rule_applies() {
    let gate = gate(0);
    // `OpenSquare` matches both rules; the first one applies.
    assert_eq!(
        gate.data_protection("OpenSquare").unwrap(),
        ProtectionLevel::None
    );
    assert_eq!(
        gate.metadata_protection("OpenSquare").unwrap(),
        ProtectionLevel::None
    );
    assert!(!gate.topic_read_protected("OpenSquare").unwrap());
    assert!(!gate.topic_discovery_protected("OpenSquare").unwrap());
    assert!(!gate.topic_liveliness_protected("OpenSquare").unwrap());

    assert_eq!(
        gate.data_protection("Square").unwrap(),
        ProtectionLevel::Encrypt
    );
    assert_eq!(
        gate.metadata_protection("Square").unwrap(),
        ProtectionLevel::Sign
    );
    assert!(gate.topic_read_protected("Square").unwrap());
    assert!(!gate.topic_write_protected("Square").unwrap());
    assert!(gate.topic_discovery_protected("Square").unwrap());
    assert!(gate.topic_liveliness_protected("Square").unwrap());

    // The participant-wide name matches the catch-all rule only.
    assert_eq!(PARTICIPANT_WIDE_TOPIC, "");
    assert_eq!(
        gate.data_protection(PARTICIPANT_WIDE_TOPIC).unwrap(),
        ProtectionLevel::Encrypt
    );
}

/// `IS_VALID` of every mask (§9.4.2.4).
const IS_VALID: u32 = 0x8000_0000;

#[test]
fn security_9_4_2_4_the_participant_masks_follow_the_domain_rule() {
    // Without a gate: the valid bit alone.
    let plain = participant_security_info(None);
    assert_eq!(plain.participant_security_attributes, IS_VALID);
    assert_eq!(plain.plugin_participant_security_attributes, IS_VALID);

    let gate = gate(0);
    let info = participant_security_info(Some(&gate));
    // is_rtps_protected 0x1, is_discovery_protected 0x2,
    // is_liveliness_protected 0x4.
    assert_eq!(info.participant_security_attributes, IS_VALID | 0x7);
    // is_rtps_encrypted 0x1, is_discovery_encrypted 0x2 (liveliness is SIGN,
    // not encrypted), is_rtps_origin_authenticated 0x8.
    assert_eq!(
        info.plugin_participant_security_attributes,
        IS_VALID | 0x1 | 0x2 | 0x8
    );
}

#[test]
fn security_9_4_2_4_the_endpoint_masks_have_the_bits_of_the_specification() {
    // metadata SIGN: is_submessage_protected 0x8; nothing encrypted.
    let sign = endpoint_security_info(
        ProtectionLevel::Sign,
        ProtectionLevel::None,
        false,
        false,
        false,
        false,
        false,
    );
    assert_eq!(sign.endpoint_security_attributes, IS_VALID | 0x08);
    assert_eq!(sign.plugin_endpoint_security_attributes, IS_VALID);

    // data SIGN: is_payload_protected 0x10.
    let payload = endpoint_security_info(
        ProtectionLevel::None,
        ProtectionLevel::Sign,
        false,
        false,
        false,
        false,
        false,
    );
    assert_eq!(payload.endpoint_security_attributes, IS_VALID | 0x10);

    // Everything: read 0x1, write 0x2, discovery 0x4, submessage 0x8,
    // payload 0x10, key 0x20 (data ENCRYPT), liveliness 0x40; plugin:
    // submessage encrypted 0x1, payload encrypted 0x2, submessage origin
    // authenticated 0x4.
    let all = endpoint_security_info(
        ProtectionLevel::Encrypt,
        ProtectionLevel::Encrypt,
        true,
        true,
        true,
        true,
        true,
    );
    assert_eq!(all.endpoint_security_attributes, IS_VALID | 0x7f);
    assert_eq!(all.plugin_endpoint_security_attributes, IS_VALID | 0x07);
}

/// A DATA submessage of the volatile secure writer to the reader, as the
/// volatile channel hands it to the Kx protection (DDSI-RTPS 2.5 §9.4.5.3;
/// entity ids 0xff0202c3 and 0xff0202c4).
const VOLATILE_DATA: [u8; 24] = [
    0x15, 0x01, 20, 0, // DATA, little endian, 20 octets
    0, 0, 16, 0, // extraFlags, octetsToInlineQos
    0xff, 0x02, 0x02, 0xc4, // readerId
    0xff, 0x02, 0x02, 0xc3, // writerId
    0, 0, 0, 0, 1, 0, 0, 0, // writerSN 1
];

/// (submessage id, body) of each submessage of `wire`.
fn submessages(wire: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 4 <= wire.len() {
        let id = wire[at];
        let len = if wire[at + 1] & 0x01 != 0 {
            u16::from_le_bytes([wire[at + 2], wire[at + 3]])
        } else {
            u16::from_be_bytes([wire[at + 2], wire[at + 3]])
        };
        let end = at + 4 + usize::from(len);
        out.push((id, &wire[at + 4..end]));
        at = end;
    }
    assert_eq!(at, wire.len(), "the submessages fill the octets");
    out
}

#[test]
fn security_10_5_2_1_2_a_kx_protected_submessage_opens_on_the_peer_only() {
    let (a_prefix, b_prefix) = ([0xa7; 12], [0xb7; 12]);
    let (a, b) = (gate(0), gate(0));
    a.register_remote_by_guid_from_secret(b_prefix, IdentityHandle(2), SharedSecretHandle(1))
        .unwrap();
    b.register_remote_by_guid_from_secret(a_prefix, IdentityHandle(1), SharedSecretHandle(1))
        .unwrap();

    let wire = a
        .encode_kx_datawriter_for(&b_prefix, &VOLATILE_DATA)
        .unwrap();
    let parts = submessages(&wire);
    let ids: Vec<u8> = parts.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [0x31, 0x30, 0x32], "SEC_PREFIX, SEC_BODY, SEC_POSTFIX");
    let header = parts[0].1;
    assert_eq!(header.len(), 20, "CryptoHeader");
    assert_eq!(&header[..4], &[0, 0, 0, 4], "AES256_GCM");
    assert_eq!(&header[4..8], &[0, 0, 0, 0], "the Kx key has key id 0");
    let body = parts[1].1;
    let content = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    assert_eq!(
        usize::try_from(content).unwrap(),
        VOLATILE_DATA.len(),
        "GCM ciphertext is as long as the plaintext"
    );
    assert_ne!(&body[4..4 + VOLATILE_DATA.len()], &VOLATILE_DATA[..]);
    // common_mac and an empty receiver_specific_macs.
    assert_eq!(parts[2].1.len(), 16 + 4);

    assert_eq!(
        b.decode_kx_datawriter_from(&a_prefix, &wire).unwrap(),
        VOLATILE_DATA
    );

    // A changed ciphertext does not authenticate.
    let mut changed = wire.clone();
    let first_ciphertext_octet = 4 + 20 + 4 + 4;
    changed[first_ciphertext_octet] ^= 0x01;
    assert!(b.decode_kx_datawriter_from(&a_prefix, &changed).is_err());

    // A participant that holds no key of A opens nothing.
    let c = gate(0);
    assert!(c.decode_kx_datawriter_from(&a_prefix, &wire).is_err());
}

#[test]
fn security_7_3_2_only_authentication_and_access_control_properties_request_security() {
    let with = |name: &str| PropertyQosPolicy::new().with(name, "file:x");
    assert!(requests_security(&with("dds.sec.auth.identity_ca")));
    assert!(requests_security(&with("dds.sec.access.governance")));
    // A logger configures observability, not security.
    assert!(!requests_security(&with("dds.sec.log.plugin")));
    assert!(!requests_security(&with("org.zerodds.sec.algo_name_nul")));
    assert!(!requests_security(&PropertyQosPolicy::new()));
}

#[test]
fn security_10_3_1_an_incomplete_property_list_is_refused_by_name() {
    let props = PropertyQosPolicy::new().with("dds.sec.auth.identity_ca", "file:/nowhere/ca.pem");
    match security_bundle_from_properties_with_env(&props, 0, [0x01; 16], None, &|_| None) {
        Err(SecurityPropertyError::MissingProperty(name)) => {
            assert!(name.starts_with("dds.sec."), "{name}");
            assert_eq!(props.get(name), None, "{name} is the missing one");
        }
        Err(other) => panic!("expected a missing property, got {other:?}"),
        Ok(_) => panic!("a bundle from one property"),
    }
}

#[test]
fn security_9_4_2_9_11_only_permit_permits() {
    assert!(EndpointAccess::Permit.is_permitted());
    // A relay-only reader is matched for relaying, not as a subscriber.
    assert!(!EndpointAccess::RelayOnly.is_permitted());
    assert!(!EndpointAccess::Deny.is_permitted());
    assert!(!EndpointAccess::NoPermissions.is_permitted());
}

#[test]
fn the_reexported_logging_items_are_those_of_the_security_crate() {
    use core::any::TypeId;
    assert_eq!(
        TypeId::of::<zerodds_security_runtime::LogLevel>(),
        TypeId::of::<zerodds_security::logging::LogLevel>()
    );
    assert_eq!(
        TypeId::of::<dyn zerodds_security_runtime::LoggingPlugin>(),
        TypeId::of::<dyn zerodds_security::logging::LoggingPlugin>()
    );
}
