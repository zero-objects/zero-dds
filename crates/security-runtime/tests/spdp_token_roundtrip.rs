//! C3.5 — Cross-Crate-Smoke: IdentityToken/PermissionsToken werden
//! im SPDP-PL_CDR_LE-Stream byte-identisch durchgereicht und der
//! Security-Layer (`zerodds_security::token::DataHolder`) parst die Bytes
//! wieder zurueck zur Spec-konformen Token-Struktur.

//!
//! Das ist die integrative Validierung fuer die getrennte Schichtung
//! "rtps reicht Bytes durch, security parst sie".
//!
//! Spec: DDS-Security 1.2 §7.4.1.4 (IdentityToken Tab.16),
//! §7.4.1.5 (PermissionsToken Tab.17), §10.3.2.1 (PKI-DH-Properties).

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

use zerodds_rtps::participant_data::{Duration, ParticipantBuiltinTopicData};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, ProtocolVersion, VendorId};
use zerodds_security::token::{DataHolder, IdentityToken};

fn make_baseline(prefix: u8) -> ParticipantBuiltinTopicData {
    ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes([prefix; 12]), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locator: None,
        default_multicast_locator: None,
        metatraffic_unicast_locator: None,
        metatraffic_multicast_locator: None,
        domain_id: Some(7),
        builtin_endpoint_set: 0,
        lease_duration: Duration::from_secs(100),
        user_data: Vec::new(),
        properties: Default::default(),
        identity_token: None,
        permissions_token: None,
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
    }
}

#[test]
fn identity_token_pki_dh_v12_roundtrips_through_spdp_pl_cdr_le() {
    let token =
        IdentityToken::pki_dh_v12("01:23:45:67", "ECDSA-SHA256", "FA:CE:0B:01", "RSA-SHA256");
    let mut data = make_baseline(0x11);
    data.identity_token = Some(token.to_cdr_le());

    let bytes = data.to_pl_cdr_le();
    let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();

    let raw = decoded
        .identity_token
        .as_ref()
        .expect("PID_IDENTITY_TOKEN durchgereicht");
    let parsed = DataHolder::from_cdr_le(raw).expect("DataHolder parsable");

    assert_eq!(parsed.class_id, "DDS:Auth:PKI-DH:1.2");
    assert_eq!(parsed.property("dds.cert.sn"), Some("01:23:45:67"));
    assert_eq!(parsed.property("dds.cert.algo"), Some("ECDSA-SHA256"));
    assert_eq!(parsed.property("dds.ca.sn"), Some("FA:CE:0B:01"));
    assert_eq!(parsed.property("dds.ca.algo"), Some("RSA-SHA256"));
    assert!(parsed.binary_properties.is_empty());
}

#[test]
fn permissions_token_v12_roundtrips_through_spdp() {
    let token = IdentityToken::permissions_v12("DE:AD:BE:EF", "ECDSA-SHA256");
    let mut data = make_baseline(0x22);
    data.permissions_token = Some(token.to_cdr_le());

    let bytes = data.to_pl_cdr_le();
    let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();

    let raw = decoded
        .permissions_token
        .as_ref()
        .expect("PID_PERMISSIONS_TOKEN durchgereicht");
    let parsed = DataHolder::from_cdr_le(raw).unwrap();

    assert_eq!(parsed.class_id, "DDS:Access:Permissions:1.2");
    assert_eq!(parsed.property("dds.perm_ca.sn"), Some("DE:AD:BE:EF"));
    assert_eq!(parsed.property("dds.perm_ca.algo"), Some("ECDSA-SHA256"));
}

#[test]
fn all_three_tokens_announced_together() {
    let mut data = make_baseline(0x33);
    data.identity_token = Some(IdentityToken::pki_dh_v12("AA", "RSA", "BB", "RSA").to_cdr_le());
    data.permissions_token = Some(IdentityToken::permissions_v12("CC", "RSA").to_cdr_le());
    data.identity_status_token = Some(
        DataHolder::new("DDS:Auth:PKI-DH:1.2")
            .with_property("dds.ocsp.status", "good")
            .to_cdr_le(),
    );

    let bytes = data.to_pl_cdr_le();
    let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();

    assert!(decoded.identity_token.is_some());
    assert!(decoded.permissions_token.is_some());
    assert!(decoded.identity_status_token.is_some());

    let id = DataHolder::from_cdr_le(decoded.identity_token.as_ref().unwrap()).unwrap();
    assert_eq!(id.class_id, "DDS:Auth:PKI-DH:1.2");

    let perms = DataHolder::from_cdr_le(decoded.permissions_token.as_ref().unwrap()).unwrap();
    assert_eq!(perms.class_id, "DDS:Access:Permissions:1.2");

    let status = DataHolder::from_cdr_le(decoded.identity_status_token.as_ref().unwrap()).unwrap();
    assert_eq!(status.property("dds.ocsp.status"), Some("good"));
}

#[test]
fn legacy_peer_without_tokens_decodes_to_none() {
    let data = make_baseline(0x44);
    let bytes = data.to_pl_cdr_le();
    let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
    assert_eq!(decoded.identity_token, None);
    assert_eq!(decoded.permissions_token, None);
    assert_eq!(decoded.identity_status_token, None);
}

#[test]
fn token_with_binary_property_survives_spdp_padding() {
    // Binary-Property-Werte koennen non-multiple-of-4 sein. Der RTPS
    // ParameterList-Codec paddet auf 4 byte und der Security-Layer-
    // Decoder muss die echte Property-Laenge aus dem CDR-Length-
    // Prefix lesen — Padding-Bytes duerfen nicht in den Wert leaken.
    let cert_blob = vec![0xCAu8, 0xFE, 0xBA, 0xBE, 0xDE, 0xAD]; // 6 byte
    let token = DataHolder::new("DDS:Auth:PKI-DH:1.2")
        .with_property("dds.cert.sn", "01:02")
        .with_binary_property("dds.cert.bytes", cert_blob.clone());

    let mut data = make_baseline(0x55);
    data.identity_token = Some(token.to_cdr_le());
    let bytes = data.to_pl_cdr_le();
    let decoded = ParticipantBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();

    let parsed = DataHolder::from_cdr_le(decoded.identity_token.as_ref().unwrap()).unwrap();
    assert_eq!(
        parsed.binary_property("dds.cert.bytes"),
        Some(&cert_blob[..])
    );
}
