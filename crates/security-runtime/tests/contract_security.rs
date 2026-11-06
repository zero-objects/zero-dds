// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-runtime` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security`, from its non-test sources:
//! - `generic_message::{encode_generic_message, decode_generic_message,
//!   ENCAPSULATION_CDR_LE, ENCAPSULATION_CDR_BE, ENCAPSULATION_HEADER_LEN}`,
//!   re-exported by `builtin_topics.rs` for the stateless and volatile
//!   topics.
//! - `access_control`: `AccessControlPlugin`, `AccessControlListener`,
//!   `AccessDecision`, `EndpointRequest`, `ParticipantRequest`,
//!   `TopicRequest`, `QosEntries`, `RelayRole`, `RemoteReaderDecision`,
//!   `PermissionsHandle`, `TopicSecurityConfig`, `EndpointSecurityConfig`,
//!   `ParticipantSecurityConfig` with `attributes_mask` and
//!   `optional_attributes_mask` (`plugin_attributes.rs`), the algorithm
//!   infos (`gate_crypto.rs`); `EndpointRequest::new` in
//!   `builtin_attributes.rs`, the rest in `shared.rs`.
//! - `cryptographic`: `CryptographicPluginInterfaces` (the plugin of the
//!   gate, `shared.rs`, `profile.rs`), the handle types,
//!   `SecureSubmessageCategory`, `ParticipantSecurityConfig`,
//!   `EndpointSecurityConfig`, the `PLUGIN_PARTICIPANT_FLAG_*` and
//!   `PLUGIN_ENDPOINT_FLAG_*` bits (`gate_crypto.rs`,
//!   `builtin_attributes.rs`); `crypto::{CryptoHandle,
//!   CryptographicPlugin}` (`gate.rs`).
//! - `algorithm::{CBIT_AES128_GCM, CBIT_AES256_GCM}` (`shared.rs`).
//! - `authentication::{AuthenticationPlugin, AlgorithmMasks,
//!   IdentityHandle, SharedSecretHandle, SharedSecretProvider,
//!   ValidationResult, ParticipantSecurityConfig}` (`profile.rs`,
//!   `shared.rs`, `endpoint_access.rs`, `gate.rs`).
//! - `token::{DataHolder, CryptoToken, class_id}` with
//!   `class_id::same_class_and_major_version` (`shared.rs`, `profile.rs`,
//!   `gate_crypto.rs`).
//! - `data_tagging::{DataTag, DataTaggingPlugin}` (`data_tagging.rs`,
//!   `bundle.rs`), `logging::{LogLevel, LoggingPlugin}` (re-exported by
//!   `lib.rs`; `bundle.rs`, `qos_properties.rs`).
//! - `properties::{Property, PropertyList}`; `AlgoNameNul` and
//!   `SubjectNameForm` (re-exported by `lib.rs`, parsed in
//!   `qos_properties.rs`); `error::{SecurityError, SecurityErrorKind}`
//!   (`shared.rs` matches `AccessDenied`).
//!
//! Oracles: the HandshakeReply an OpenDDS participant sent, captured in
//! `crates/discovery/tests/fixtures/opendds_handshake_reply_head_run.hex`;
//! DDSI-RTPS 2.5 §10.1 and §10.2 (the encapsulation header and the kinds
//! CDR_BE 0x0000, CDR_LE 0x0001, PL_CDR_LE 0x0003); DDS-Security 1.2
//! §7.5.3.5 (`GMCLASSID_SECURITY_AUTH_HANDSHAKE` = "dds.sec.auth") and
//! §7.5.5 (the `ParticipantGenericMessage`); §9.4.2.5 Tables 34 and 35;
//! §10.4.2.4 Table 67, §10.4.2.6 Table 69; §9.5.1.7 Table 41; §8.1; §7.3.3.1
//! (an algorithm name as BinaryProperty_t counts its NUL); §10.4.2.2 (class
//! id versions); §9.2.1 Table 28; Table 50 ("propagate set to FALSE"); and
//! the rustdoc of `AlgoNameNul` and `SubjectNameForm`.
#![allow(clippy::unwrap_used)]

use zerodds_security::access_control::{
    ParticipantSecurityConfig, participant_attributes_mask, participant_optional_attributes_mask,
};
use zerodds_security::algorithm::{CBIT_AES128_GCM, CBIT_AES256_GCM};
use zerodds_security::cryptographic::{
    PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED, PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED,
    PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED, SecureSubmessageCategory,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::generic_message::{
    ENCAPSULATION_CDR_BE, ENCAPSULATION_CDR_LE, ENCAPSULATION_HEADER_LEN,
    class_id as message_class, decode_generic_message, encode_generic_message,
};
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security::token::class_id;
use zerodds_security::{AlgoNameNul, SubjectNameForm};

/// The octets of the captured OpenDDS HandshakeReply.
fn opendds_reply() -> Vec<u8> {
    let text = include_str!("../../discovery/tests/fixtures/opendds_handshake_reply_head_run.hex");
    let text = text.trim();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn security_7_5_5_the_handshake_reply_of_opendds_decodes_and_encodes_to_its_octets() {
    // DDSI-RTPS 2.5 §10.1/§10.2: two octets of kind, two of options.
    assert_eq!(ENCAPSULATION_HEADER_LEN, 4);
    assert_eq!(ENCAPSULATION_CDR_LE, [0x00, 0x01]);
    assert_eq!(ENCAPSULATION_CDR_BE, [0x00, 0x00]);

    let wire = opendds_reply();
    assert_eq!(wire[..4], [0x00, 0x01, 0x00, 0x00]);
    let message = decode_generic_message(&wire).unwrap();
    // §7.5.3.5: the reply is GMCLASSID_SECURITY_AUTH_HANDSHAKE.
    assert_eq!(message.message_class_id, "dds.sec.auth");
    assert_eq!(message_class::AUTH, "dds.sec.auth");
    assert_eq!(message_class::AUTH_REQUEST, "dds.sec.auth_request");
    assert!(!message.message_identity.is_nil());
    // The reply relates to the first request ZeroDDS sent.
    assert_eq!(message.related_message_identity.sequence_number, 1);
    assert_eq!(message.message_data.len(), 1);
    assert_eq!(
        message.message_data[0].class_id,
        "DDS:Auth:PKI-DH:1.0+Reply"
    );
    assert_eq!(message.message_data[0].binary_properties.len(), 9);

    // Written back, the message is the octets OpenDDS sent.
    assert_eq!(encode_generic_message(&message), wire);
}

#[test]
fn ddsi_rtps_10_2_only_the_cdr_encapsulations_are_taken() {
    let mut wire = opendds_reply();
    // PL_CDR_LE (0x0003) is a ParameterList, not a ParticipantGenericMessage.
    wire[1] = 0x03;
    assert_eq!(
        decode_generic_message(&wire).unwrap_err().kind,
        SecurityErrorKind::BadArgument
    );
    // Shorter than the encapsulation header, or cut in the body.
    assert!(decode_generic_message(&[0x00, 0x01, 0x00]).is_err());
    let whole = opendds_reply();
    assert!(decode_generic_message(&whole[..whole.len() - 1]).is_err());
}

#[test]
fn security_9_4_2_5_the_participant_masks_of_tables_34_and_35() {
    use participant_attributes_mask as m;
    // Table 34.
    assert_eq!(
        [
            m::IS_RTPS_AXK_PROTECTED,
            m::IS_DISCOVERY_PROTECTED,
            m::IS_LIVELINESS_PROTECTED,
            m::IS_KEY_REVISION_ENABLED,
            m::IS_RTPS_PSK_PROTECTED,
            m::IS_VALID,
        ],
        [1 << 0, 1 << 1, 1 << 2, 1 << 3, 1 << 4, 1 << 31]
    );
    // `plugin_attributes.rs` announces `attributes_mask` of the config:
    // the valid bit and the bit of each protected item, nothing for the
    // fields Table 34 does not map.
    let config = ParticipantSecurityConfig {
        allow_unauthenticated_participants: true,
        is_access_protected: true,
        is_discovery_protected: true,
        is_key_revision_enabled: true,
        ..ParticipantSecurityConfig::default()
    };
    assert_eq!(
        config.attributes_mask(),
        m::IS_VALID | m::IS_DISCOVERY_PROTECTED | m::IS_KEY_REVISION_ENABLED
    );
    assert_eq!(
        ParticipantSecurityConfig::default().attributes_mask(),
        m::IS_VALID
    );

    // Table 35: the two bits apply to `is_set` and `value`.
    use participant_optional_attributes_mask as o;
    assert_eq!(
        (
            o::ALLOW_UNAUTHENTICATED_PARTICIPANTS,
            o::IS_ACCESS_PROTECTED
        ),
        (1 << 0, 1 << 1)
    );
    assert_eq!(config.optional_attributes_mask(), (0b11, 0b11));
    let closed = ParticipantSecurityConfig {
        is_access_protected: true,
        ..ParticipantSecurityConfig::default()
    };
    assert_eq!(closed.optional_attributes_mask(), (0b11, 0b10));
}

#[test]
fn security_10_4_2_the_plugin_bits_and_the_categories_of_the_gate() {
    // Table 67 and Table 69, the bits `gate_crypto.rs` sets.
    assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED, 1 << 0);
    assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED, 1 << 3);
    assert_eq!(PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED, 1 << 0);
    assert_eq!(PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED, 1 << 1);
    assert_eq!(
        PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
        1 << 2
    );
    // §8.1, the widths `shared.rs` requires.
    assert_eq!((CBIT_AES128_GCM, CBIT_AES256_GCM), (1, 2));
    // Table 41: three categories.
    assert_ne!(
        SecureSubmessageCategory::DatawriterSubmessage,
        SecureSubmessageCategory::DatareaderSubmessage
    );
    assert_ne!(
        SecureSubmessageCategory::InfoSubmessage,
        SecureSubmessageCategory::DatawriterSubmessage
    );
}

#[test]
fn security_7_3_3_1_the_nul_forms_of_the_algorithm_names() {
    // §7.3.3.1: "the string 'ECDSA-SHA256' shall result in an octet
    // sequence value with length 13" — the spec form, the default.
    assert_eq!(AlgoNameNul::default(), AlgoNameNul::Spec);
    for opendds in [false, true] {
        assert!(AlgoNameNul::Spec.nul_for(opendds));
        assert!(!AlgoNameNul::Omit.nul_for(opendds));
    }
    assert!(AlgoNameNul::ByVendor.nul_for(true));
    assert!(!AlgoNameNul::ByVendor.nul_for(false));
    // The settings `qos_properties.rs` reads.
    for (text, form) in [
        ("spec", AlgoNameNul::Spec),
        ("omit", AlgoNameNul::Omit),
        ("mirror", AlgoNameNul::Mirror),
        ("by_vendor", AlgoNameNul::ByVendor),
    ] {
        assert_eq!(AlgoNameNul::parse(text), Some(form));
    }
    assert_eq!(AlgoNameNul::parse("Spec"), None);
    assert_eq!(SubjectNameForm::default(), SubjectNameForm::Rfc4514);
    assert_eq!(
        SubjectNameForm::parse("xn_flag_oneline"),
        Some(SubjectNameForm::XnFlagOneline)
    );
    assert_eq!(SubjectNameForm::parse("RFC4514"), None);
}

#[test]
fn security_10_4_2_2_a_remote_permissions_token_is_compared_by_class_and_major_version() {
    // `shared.rs` compares the token class of a remote participant with
    // its own.
    assert!(class_id::same_class_and_major_version(
        class_id::ACCESS_PERMISSIONS_V12,
        "DDS:Access:Permissions:1.0"
    ));
    assert!(!class_id::same_class_and_major_version(
        class_id::ACCESS_PERMISSIONS_V12,
        "DDS:Access:PSK:1.2"
    ));
}

#[test]
fn security_9_2_1_the_gate_tells_a_refusal_by_its_kind() {
    // `shared.rs` turns an `AccessDenied` of the plugin into a refusal and
    // every other kind into an error.
    let denied = SecurityError::new(SecurityErrorKind::AccessDenied, "no grant");
    let failed = SecurityError::new(SecurityErrorKind::Internal, "lock poisoned");
    assert_eq!(denied.kind, SecurityErrorKind::AccessDenied);
    assert_ne!(failed.kind, SecurityErrorKind::AccessDenied);
    assert_eq!(denied.code, SecurityErrorKind::AccessDenied.code());
    assert_ne!(denied.code, failed.code);
}

#[test]
fn the_properties_the_runtime_writes_are_local() {
    // Table 50 and Table 63: "all these properties shall have propagate set
    // to FALSE"; `Property::local` is that.
    let mut list = PropertyList::new();
    list.set(Property::local("dds.sec.auth.identity_ca", "file:ca.pem"));
    list.set(Property::local("dds.sec.access.governance", "file:gov.p7s"));
    assert_eq!(list.get("dds.sec.access.governance"), Some("file:gov.p7s"));
    assert_eq!(list.propagatable().count(), 0);
    assert!(list.entries().iter().all(|p| !p.propagate));
}
