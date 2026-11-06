// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security` (only with its feature
//! `security`, see `Cargo.toml`), from its non-test sources:
//! - `runtime.rs`, the crypto token exchange over
//!   DCPSParticipantVolatileMessageSecure: `generic_message::{
//!   ParticipantGenericMessage, MessageIdentity, class_id::{
//!   PARTICIPANT_CRYPTO_TOKENS, DATAWRITER_CRYPTO_TOKENS,
//!   DATAREADER_CRYPTO_TOKENS}}` (`build_crypto_token_message`,
//!   `prepare_endpoint_crypto_tokens`, `install_crypto_token`), with one or
//!   two `token::DataHolder`s of class `DDS:Crypto:AES_GCM_GMAC` carrying
//!   the binary property `dds.cryp.keymat`, and `source_endpoint_key` and
//!   `destination_endpoint_key` as the key of a sent token.
//! - `runtime.rs`, discovery of a secure participant:
//!   `token::DataHolder::from_cdr_le` on the `identity_token` and
//!   `permissions_token` of a ParticipantBuiltinTopicData,
//!   `token::class_id::same_class_and_major_version` on the class ids of the
//!   identity tokens and `same_permissions_class_and_major_version` on those
//!   of the permissions tokens (§10.4.2.2 reads one without versions as 1.0).
//! - `runtime.rs`, the authentication builtins:
//!   `authentication::{AuthenticationPlugin, IdentityHandle,
//!   SharedSecretHandle, ParticipantSecurityConfig, AlgorithmMasks}`
//!   (`ParticipantSecurityConfig::unrestricted()` when the gate has no
//!   AccessControl configuration, else the masks of
//!   `access_control::ParticipantSecurityAlgorithmInfo` field by field);
//!   `IdentityHandle(0)` as the fallback handle.
//! - `runtime.rs`, access checks: `access_control::EndpointRequest::new(0,
//!   topic).in_partitions(..).with_data_tags(..)`;
//!   `properties::{PropertyList::new, Property::local}` for the
//!   `PropertyQosPolicy` of the participant (`check_participant_qos`);
//!   `error::{SecurityError::new, SecurityErrorKind::CryptoFailed}`;
//!   `crypto::CryptoHandle`; `data_tagging::{DataTaggingPlugin, DataTag}`.
//! - `security_log_topic.rs`: `logging::{TOPIC_LOG_TOPIC,
//!   TOPIC_LOG_TOPIC_V2}`, the topics of the BuiltinLoggingTopic writers.
//!
//! What it relies on: the token messages have the layout of the IDL with
//! the DataHolders inline and four-aligned, so that Cyclone DDS and OpenDDS
//! read them; the class ids are the strings of the specification; a token
//! of a peer of version 1.0 matches a local one of version 1.2; the
//! unrestricted configuration supports every algorithm and requires none;
//! the last `Property` of a name wins.
//!
//! Oracles: DDS-Security 1.2 §7.3.22 (`ParticipantGenericMessage`,
//! `MessageIdentity`), §7.3.3 (`DataHolder`, `Property_t`,
//! `BinaryProperty_t`), §7.5.4 (the class ids of the crypto token exchange),
//! §9.3.2.1 (a class id is `<PluginClassName>:<MajorVersion>.<MinorVersion>`;
//! Table 70 compares PluginClassName and MajorVersion), §7.3.9
//! (`CRYPTO_ALGORITHM_SET_ALL` 0xffffffff, `CRYPTO_ALGORITHM_SET_EMPTY` 0),
//! §10.6 (`DDS:Security:LogTopicV2`, `DDS:Security:LogTopic`), §9.2.1 Table
//! 28, DDSI-RTPS 2.5 §10.2 (`CDR_LE` 0x0001) and the XCDR1 alignment of
//! DDS-XTypes 1.3 §7.4.1. The expected octets below are written out from
//! the IDL by hand.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use zerodds_security::access_control::EndpointRequest;
use zerodds_security::authentication::{AlgorithmMasks, IdentityHandle, ParticipantSecurityConfig};
use zerodds_security::data_tagging::{DataTag, DataTaggingPlugin};
use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::generic_message::{
    MessageIdentity, ParticipantGenericMessage, class_id, decode_generic_message,
    encode_generic_message,
};
use zerodds_security::logging::{TOPIC_LOG_TOPIC, TOPIC_LOG_TOPIC_V2};
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security::token::{
    DataHolder,
    class_id::{same_class_and_major_version, same_permissions_class_and_major_version},
};

/// The class id and the property name this crate writes into a CryptoToken.
const CRYPTO_TOKEN_CLASS_ID: &str = "DDS:Crypto:AES_GCM_GMAC";
const CRYPTO_TOKEN_PROP: &str = "dds.cryp.keymat";

const LOCAL_PARTICIPANT: [u8; 16] = [
    0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x00, 0x00, 0x01, 0xc1,
];
const REMOTE_PARTICIPANT: [u8; 16] = [
    0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x00, 0x00, 0x01, 0xc1,
];
const LOCAL_WRITER: [u8; 16] = [
    0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x00, 0x00, 0x01, 0x02,
];
const REMOTE_READER: [u8; 16] = [
    0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x00, 0x00, 0x01, 0x07,
];

/// The datawriter token message of `prepare_endpoint_crypto_tokens`: the
/// submessage keymat, then the payload keymat.
fn writer_tokens() -> ParticipantGenericMessage {
    let holder = |keymat: Vec<u8>| {
        DataHolder::new(CRYPTO_TOKEN_CLASS_ID).with_binary_property(CRYPTO_TOKEN_PROP, keymat)
    };
    ParticipantGenericMessage {
        message_identity: MessageIdentity {
            source_guid: LOCAL_PARTICIPANT,
            sequence_number: 1,
        },
        related_message_identity: MessageIdentity::default(),
        destination_participant_key: REMOTE_PARTICIPANT,
        destination_endpoint_key: REMOTE_READER,
        source_endpoint_key: LOCAL_WRITER,
        message_class_id: class_id::DATAWRITER_CRYPTO_TOKENS.into(),
        message_data: vec![holder(vec![0x11; 5]), holder(vec![0x22; 3])],
    }
}

/// A CDR string as XCDR1 writes it: length with the NUL, characters, NUL.
/// Padding is written by the caller, with the offsets in the comments.
fn string_le(o: &mut Vec<u8>, s: &str) {
    o.extend_from_slice(&u32::try_from(s.len() + 1).unwrap().to_le_bytes());
    o.extend_from_slice(s.as_bytes());
    o.push(0);
}

#[test]
fn security_7_3_22_a_datawriter_token_message_has_the_layout_of_the_idl() {
    let mut o = vec![0x00, 0x01, 0x00, 0x00]; // CDR_LE, options
    // Body offsets.
    o.extend_from_slice(&LOCAL_PARTICIPANT); // 0
    o.extend_from_slice(&1i64.to_le_bytes()); // 16
    o.extend_from_slice(&[0; 16]); // 24, related: NIL
    o.extend_from_slice(&0i64.to_le_bytes()); // 40
    o.extend_from_slice(&REMOTE_PARTICIPANT); // 48
    o.extend_from_slice(&REMOTE_READER); // 64
    o.extend_from_slice(&LOCAL_WRITER); // 80
    string_le(&mut o, "dds.sec.datawriter_crypto_tokens"); // 96..133
    o.extend_from_slice(&[0, 0, 0]); // 133 -> 136
    o.extend_from_slice(&2u32.to_le_bytes()); // 136: two DataHolders
    // 140: the first DataHolder, inline, no length in front.
    string_le(&mut o, "DDS:Crypto:AES_GCM_GMAC"); // 140..168
    o.extend_from_slice(&0u32.to_le_bytes()); // 168: properties
    o.extend_from_slice(&1u32.to_le_bytes()); // 172: binary_properties
    string_le(&mut o, "dds.cryp.keymat"); // 176..196
    o.extend_from_slice(&5u32.to_le_bytes()); // 196
    o.extend_from_slice(&[0x11; 5]); // 200..205
    o.extend_from_slice(&[0, 0, 0]); // 205 -> 208
    // 208: the second DataHolder.
    string_le(&mut o, "DDS:Crypto:AES_GCM_GMAC"); // 208..236
    o.extend_from_slice(&0u32.to_le_bytes()); // 236
    o.extend_from_slice(&1u32.to_le_bytes()); // 240
    string_le(&mut o, "dds.cryp.keymat"); // 244..264
    o.extend_from_slice(&3u32.to_le_bytes()); // 264
    o.extend_from_slice(&[0x22; 3]); // 268..271

    assert_eq!(encode_generic_message(&writer_tokens()), o);
    let back = decode_generic_message(&o).unwrap();
    assert_eq!(back, writer_tokens());
    // `install_crypto_token` takes the keymat of every holder.
    let keymats: Vec<&[u8]> = back
        .message_data
        .iter()
        .map(|h| h.binary_property(CRYPTO_TOKEN_PROP).unwrap())
        .collect();
    assert_eq!(keymats, [&[0x11u8; 5][..], &[0x22u8; 3][..]]);
}

#[test]
fn security_7_5_4_the_class_ids_of_the_crypto_token_exchange() {
    assert_eq!(
        class_id::PARTICIPANT_CRYPTO_TOKENS,
        "dds.sec.participant_crypto_tokens"
    );
    assert_eq!(
        class_id::DATAWRITER_CRYPTO_TOKENS,
        "dds.sec.datawriter_crypto_tokens"
    );
    assert_eq!(
        class_id::DATAREADER_CRYPTO_TOKENS,
        "dds.sec.datareader_crypto_tokens"
    );
    assert!(MessageIdentity::default().is_nil());
}

#[test]
fn security_9_3_2_1_an_identity_token_from_spdp_gives_its_class_id() {
    // An IdentityToken of a peer of version 1.0 with two properties, as the
    // value of PID_IDENTITY_TOKEN.
    let mut o = Vec::new();
    string_le(&mut o, "DDS:Auth:PKI-DH:1.0"); // 0..24
    o.extend_from_slice(&2u32.to_le_bytes()); // 24: properties
    string_le(&mut o, "dds.cert.sn"); // 28..44
    string_le(&mut o, "CN=a"); // 44..53
    o.extend_from_slice(&[0, 0, 0]); // 53 -> 56
    string_le(&mut o, "dds.cert.algo"); // 56..74
    o.extend_from_slice(&[0, 0]); // 74 -> 76
    string_le(&mut o, "EC-prime256v1"); // 76..94
    o.extend_from_slice(&[0, 0]); // 94 -> 96
    o.extend_from_slice(&0u32.to_le_bytes()); // 96: binary_properties

    let token = DataHolder::from_cdr_le(&o).unwrap();
    assert_eq!(token.class_id, "DDS:Auth:PKI-DH:1.0");
    assert_eq!(token.property("dds.cert.sn"), Some("CN=a"));
    assert_eq!(token.property("dds.cert.algo"), Some("EC-prime256v1"));
    assert_eq!(token.to_cdr_le(), o);
    // A blob that is no DataHolder is not read as one: the participant is
    // then reported as announcing no readable token.
    assert!(DataHolder::from_cdr_le(&o[..30]).is_err());
}

#[test]
fn security_9_3_2_1_class_ids_compare_by_plugin_class_and_major_version() {
    for (local, remote, compatible) in [
        ("DDS:Auth:PKI-DH:1.2", "DDS:Auth:PKI-DH:1.0", true),
        ("DDS:Auth:PKI-DH:1.2", "DDS:Auth:PKI-DH:1.2", true),
        (
            "DDS:Access:Permissions:1.2",
            "DDS:Access:Permissions:1.0",
            true,
        ),
        ("DDS:Auth:PKI-DH:1.2", "DDS:Auth:PKI-DH:2.0", false),
        ("DDS:Auth:PKI-DH:1.2", "DDS:Access:Permissions:1.2", false),
        ("DDS:Auth:PKI-DH:1.2", "", false),
    ] {
        assert_eq!(
            same_class_and_major_version(local, remote),
            compatible,
            "{local} / {remote}"
        );
        assert_eq!(same_class_and_major_version(remote, local), compatible);
    }
    // §10.4.2.2 for the PermissionsToken, which §10.3.2.1 does not give the
    // IdentityToken.
    assert!(same_permissions_class_and_major_version(
        "DDS:Access:Permissions:1.2",
        "DDS:Access:Permissions"
    ));
    assert!(!same_class_and_major_version(
        "DDS:Auth:PKI-DH:1.2",
        "DDS:Auth:PKI-DH"
    ));
}

#[test]
fn security_7_3_9_the_unrestricted_configuration_supports_all_and_requires_none() {
    let config = ParticipantSecurityConfig::unrestricted();
    let all_none = (0xffff_ffff, 0);
    assert_eq!(
        config.algorithm_info,
        AlgorithmMasks {
            trust_chain: all_none,
            message_auth: all_none,
            key_establishment: all_none,
        }
    );
}

#[test]
fn an_endpoint_request_carries_topic_partitions_and_tags() {
    let partitions = ["sensors", ""];
    let tags = [("classification", "secret")];
    let request = EndpointRequest::new(0, "Square")
        .in_partitions(&partitions)
        .with_data_tags(&tags);
    assert_eq!(request.domain_id, 0);
    assert_eq!(request.topic_name, "Square");
    assert_eq!(request.partitions, ["sensors", ""]);
    assert_eq!(request.data_tags, [("classification", "secret")]);
    assert!(request.qos.is_empty());
    // Without partitions the request is in the default partition.
    assert!(EndpointRequest::new(0, "Square").partitions.is_empty());
}

#[test]
fn the_last_property_of_a_name_wins_and_stays_local() {
    // `check_participant_qos` copies the PropertyQosPolicy in order.
    let mut list = PropertyList::new();
    for (name, value) in [
        ("dds.sec.access.governance", "file:a.p7s"),
        ("dds.sec.access.permissions", "file:p.p7s"),
        ("dds.sec.access.governance", "file:b.p7s"),
    ] {
        list.set(Property::local(name.to_string(), value.to_string()));
    }
    assert_eq!(list.get("dds.sec.access.governance"), Some("file:b.p7s"));
    assert_eq!(list.entries().len(), 2);
    assert_eq!(list.propagatable().count(), 0);
}

#[test]
fn security_9_2_1_a_crypto_failure_is_an_exception_with_its_code() {
    let error = SecurityError::new(SecurityErrorKind::CryptoFailed, "decode refused");
    assert_eq!(error.kind, SecurityErrorKind::CryptoFailed);
    assert_eq!(error.code, SecurityErrorKind::CryptoFailed.code());
    assert_eq!(error.minor_code, 0);
    assert!(error.to_string().contains("decode refused"));
    assert_eq!(IdentityHandle(0), IdentityHandle::NIL);
}

#[test]
fn security_10_6_the_builtin_logging_topic_names() {
    assert_eq!(TOPIC_LOG_TOPIC_V2, "DDS:Security:LogTopicV2");
    assert_eq!(TOPIC_LOG_TOPIC, "DDS:Security:LogTopic");
}

/// A tagging plugin that keeps the tags per endpoint.
#[derive(Default)]
struct Tags(Vec<([u8; 16], Vec<DataTag>)>);

impl DataTaggingPlugin for Tags {
    fn set_tags(&mut self, endpoint_guid: [u8; 16], tags: Vec<DataTag>) {
        self.0.retain(|(guid, _)| *guid != endpoint_guid);
        self.0.push((endpoint_guid, tags));
    }

    fn get_tags(&self, endpoint_guid: [u8; 16]) -> Vec<DataTag> {
        self.0
            .iter()
            .find(|(guid, _)| *guid == endpoint_guid)
            .map(|(_, tags)| tags.clone())
            .unwrap_or_default()
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Tagging:DDS_Discovery"
    }
}

#[test]
fn the_tagging_plugin_is_driven_through_a_shared_trait_object() {
    let plugin: Arc<Mutex<dyn DataTaggingPlugin>> = Arc::new(Mutex::new(Tags::default()));
    let tag = DataTag {
        name: "classification".to_string(),
        value: "secret".to_string(),
    };
    let mut guard = plugin.lock().unwrap();
    guard.set_tags(LOCAL_WRITER, vec![tag.clone()]);
    assert_eq!(guard.get_tags(LOCAL_WRITER), [tag]);
    // An endpoint that goes away is set to no tags.
    guard.set_tags(LOCAL_WRITER, Vec::new());
    assert!(guard.get_tags(LOCAL_WRITER).is_empty());
}
