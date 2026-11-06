// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-permissions` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security`, from its non-test sources:
//! - `access_control`: `AccessControlPlugin`, implemented by
//!   `PermissionsAccessControl` (`plugin.rs`) and
//!   `PskPermissionsAccessControl` (`psk_access.rs`), with
//!   `AccessControlListener` (held as `Arc<dyn _>`, called with
//!   `on_revoke_permissions`), `AccessDecision`, `EndpointRequest`,
//!   `ParticipantRequest`, `TopicRequest`, `RelayRole`,
//!   `RemoteReaderDecision`, `PermissionsHandle`,
//!   `ParticipantSecurityConfig`, `TopicSecurityConfig`,
//!   `EndpointSecurityConfig`, the algorithm infos
//!   (`ParticipantSecurityAlgorithmInfo`, `DigitalSignatureAlgorithmInfo`,
//!   `KeyEstablishmentAlgorithmInfo`, `SymmetricCipherAlgorithmInfo`,
//!   `CryptoAlgorithmRequirements`, `EndpointSecurityAlgorithmInfo`,
//!   `EndpointSymmetricCipherAlgorithmInfo`) and the bit modules
//!   `plugin_participant_attributes_mask` and
//!   `plugin_endpoint_attributes_mask`.
//! - `algorithm`: `CRYPTO_ALGORITHM_COMPATIBILITY_MODE`,
//!   `CRYPTO_ALGORITHM_SET_ALL`, `CRYPTO_ALGORITHM_SET_EMPTY` (`plugin.rs`,
//!   `psk_access.rs`), and the `CNAME_*`/`CBIT_*` pairs of Tables 22, 25
//!   and 26 that `governance.rs` reads `<allowed_crypto_algorithms>` with.
//! - `authentication::{AlgorithmMasks, IdentityHandle}` (the identity
//!   algorithms of a `ParticipantRequest`).
//! - `crypto::{ENDPOINT_DATA_PROTECTION_KIND,
//!   ENDPOINT_METADATA_PROTECTION_KIND,
//!   PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION}`: the names of the properties
//!   `plugin.rs` puts into the configurations for the Cryptographic plugin.
//! - `token::{DataHolder, PermissionsToken, PermissionsCredentialToken,
//!   class_id, prop}`: the tokens of Tables 64 and 65, `class_id::
//!   same_permissions_class_and_major_version` (Table 70, §10.4.2.2),
//!   `binary_property("c.id")` and
//!   `binary_property("c.perm")` of the remote credential token, and
//!   `to_cdr_le` (`psk_access.rs`).
//! - `logging::{TOPIC_LOG_TOPIC, TOPIC_LOG_TOPIC_V2}`: the builtin topic
//!   rule of the logging topic (`governance.rs`).
//! - `properties::{Property, PropertyList}` (`cms.rs` reads
//!   `dds.sec.access.permissions_ca` with `PropertyList::get`).
//! - `error::{SecurityError, SecurityErrorKind, SecurityResult}`, with
//!   `with_minor_code` and `bad_argument` (`issued.rs`).
//! - `SubjectNameForm` (the form of `dds.perm_ca.sn`).
//!
//! Oracles: DDS-Security 1.2 §7.3.4 (the IDL of `DataHolder`, serialized as
//! CDR with 4-octet aligned lengths, DDSI-RTPS 2.5 §9.4.2.1), §7.3.8 and
//! §7.3.9 (`CRYPTO_ALGORITHM_COMPATIBILITY_MODE`, the sets), §8.1 Table 22,
//! §8.2 Table 25, §8.3 Table 26, §10.4.1.2.3 (the governance XSD
//! enumerations `DigitalSignatureKind`, `KeyEstablishmentKind`,
//! `SymmetricCipherKind`), §10.3.2.1 (the class id format
//! `<PluginClassName>:<MajorVersion>.<MinorVersion>`), §10.4.2.1 Table 64,
//! §10.4.2.2 Table 65, §10.4.2.4 Table 67, §10.4.2.6 Table 69, §10.3.2.3
//! Table 54 (`c.id`, `c.perm`), §10.6 (the logging topic names), §9.2.1
//! Table 28, §7.3.2 (vendor property names), and the rustdoc of
//! `ParticipantRequest::new`, `EndpointRequest`, `TopicRequest::new`,
//! `AccessDecision` and `SubjectNameForm`.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use zerodds_security::SubjectNameForm;
use zerodds_security::access_control::{
    AccessControlListener, AccessDecision, EndpointRequest, ParticipantRequest, PermissionsHandle,
    RelayRole, RemoteReaderDecision, TopicRequest, plugin_endpoint_attributes_mask,
    plugin_participant_attributes_mask,
};
use zerodds_security::algorithm::{
    CBIT_AES128_GCM, CBIT_AES256_GCM, CBIT_DHE_MODP_2048_256, CBIT_ECDHE_CEUM_P256,
    CBIT_ECDHE_CEUM_P384, CBIT_ECDSA_P256_SHA256, CBIT_ECDSA_P384_SHA384,
    CBIT_RSASSA_PKCS1_V15_2048_SHA256, CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256, CNAME_AES128_GCM,
    CNAME_AES256_GCM, CNAME_DHE_MODP_2048_256, CNAME_ECDHE_CEUM_P256, CNAME_ECDHE_CEUM_P384,
    CNAME_ECDSA_P256_SHA256_NAME, CNAME_ECDSA_P384_SHA384, CNAME_RSASSA_PKCS1_V15_2048_SHA256,
    CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256, CRYPTO_ALGORITHM_COMPATIBILITY_MODE,
    CRYPTO_ALGORITHM_SET_ALL, CRYPTO_ALGORITHM_SET_EMPTY,
};
use zerodds_security::authentication::AlgorithmMasks;
use zerodds_security::crypto::{
    ENDPOINT_DATA_PROTECTION_KIND, ENDPOINT_METADATA_PROTECTION_KIND,
    PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::logging::{TOPIC_LOG_TOPIC, TOPIC_LOG_TOPIC_V2};
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security::token::{
    DataHolder, PermissionsCredentialToken, PermissionsToken, class_id, prop,
};

#[test]
fn security_10_4_1_2_3_the_governance_algorithm_names_and_their_bits() {
    // The XSD enumeration `DigitalSignatureKind` and the bits of Table 25.
    let signature = [
        (
            CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256,
            CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256,
        ),
        (
            CNAME_RSASSA_PKCS1_V15_2048_SHA256,
            CBIT_RSASSA_PKCS1_V15_2048_SHA256,
        ),
        (CNAME_ECDSA_P256_SHA256_NAME, CBIT_ECDSA_P256_SHA256),
        (CNAME_ECDSA_P384_SHA384, CBIT_ECDSA_P384_SHA384),
    ];
    assert_eq!(
        signature,
        [
            ("RSASSA-PSS-MGF1SHA256+2048+SHA256", 1 << 0),
            ("RSASSA-PKCS1-V1_5+2048+SHA256", 1 << 1),
            ("ECDSA+P256+SHA256", 1 << 2),
            ("ECDSA+P384+SHA384", 1 << 3),
        ]
    );
    // `KeyEstablishmentKind` and Table 26.
    let key_establishment = [
        (CNAME_DHE_MODP_2048_256, CBIT_DHE_MODP_2048_256),
        (CNAME_ECDHE_CEUM_P256, CBIT_ECDHE_CEUM_P256),
        (CNAME_ECDHE_CEUM_P384, CBIT_ECDHE_CEUM_P384),
    ];
    assert_eq!(
        key_establishment,
        [
            ("DHE+MODP-2048-256", 1 << 0),
            ("ECDHE-CEUM+P256", 1 << 1),
            ("ECDHE-CEUM+P384", 1 << 2),
        ]
    );
    // `SymmetricCipherKind` and Table 22.
    let symmetric = [
        (CNAME_AES128_GCM, CBIT_AES128_GCM),
        (CNAME_AES256_GCM, CBIT_AES256_GCM),
    ];
    assert_eq!(symmetric, [("AES128+GCM", 1 << 0), ("AES256+GCM", 1 << 1)]);
}

#[test]
fn security_7_3_9_the_algorithm_sets_and_the_compatibility_mode_bit() {
    // §7.3.9: CRYPTO_ALGORITHM_SET_ALL = 0xffffffff, _EMPTY = 0x00000000.
    assert_eq!(CRYPTO_ALGORITHM_SET_ALL, 0xffff_ffff);
    assert_eq!(CRYPTO_ALGORITHM_SET_EMPTY, 0);
    // §7.3.8: the highest bit is the compatibility mode and no algorithm
    // ("The highest bit of a CryptoAlgorithmSet does not represent an
    // algorithm identifier"). `plugin.rs` masks it off before it compares
    // algorithm bits.
    assert_eq!(CRYPTO_ALGORITHM_COMPATIBILITY_MODE, 1 << 31);
    let every_algorithm = CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256
        | CBIT_RSASSA_PKCS1_V15_2048_SHA256
        | CBIT_ECDSA_P256_SHA256
        | CBIT_ECDSA_P384_SHA384
        | CBIT_DHE_MODP_2048_256
        | CBIT_ECDHE_CEUM_P256
        | CBIT_ECDHE_CEUM_P384
        | CBIT_AES128_GCM
        | CBIT_AES256_GCM;
    assert_eq!(every_algorithm & CRYPTO_ALGORITHM_COMPATIBILITY_MODE, 0);
}

#[test]
fn security_10_4_2_plugin_attribute_bits() {
    // Table 67.
    use plugin_participant_attributes_mask as p;
    assert_eq!(
        [
            p::IS_RTPS_AXK_ENCRYPTED,
            p::IS_DISCOVERY_ENCRYPTED,
            p::IS_LIVELINESS_ENCRYPTED,
            p::IS_RTPS_ORIGIN_AUTHENTICATED,
            p::IS_DISCOVERY_ORIGIN_AUTHENTICATED,
            p::IS_LIVELINESS_ORIGIN_AUTHENTICATED,
            p::IS_RTPS_PSK_ENCRYPTED,
        ],
        [1 << 0, 1 << 1, 1 << 2, 1 << 3, 1 << 4, 1 << 5, 1 << 6]
    );
    // Table 69.
    use plugin_endpoint_attributes_mask as e;
    assert_eq!(
        [
            e::IS_SUBMESSAGE_ENCRYPTED,
            e::IS_PAYLOAD_ENCRYPTED,
            e::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
        ],
        [1 << 0, 1 << 1, 1 << 2]
    );
}

#[test]
fn security_10_4_2_the_permissions_tokens_of_tables_64_and_65() {
    // Table 65: class "DDS:Access:Permissions:1.2", properties
    // `dds.perm_ca.sn` and `dds.perm_ca.algo`.
    assert_eq!(
        class_id::ACCESS_PERMISSIONS_V12,
        "DDS:Access:Permissions:1.2"
    );
    assert_eq!(prop::PERM_CA_SN, "dds.perm_ca.sn");
    assert_eq!(prop::PERM_CA_ALGO, "dds.perm_ca.algo");
    // Table 64: class "DDS:Access:PermissionsCredential".
    assert_eq!(
        class_id::ACCESS_PERMISSIONS_CREDENTIAL,
        "DDS:Access:PermissionsCredential"
    );

    // Built as `get_permissions_token` builds it.
    let token: PermissionsToken = DataHolder::new(class_id::ACCESS_PERMISSIONS_V12)
        .with_property(prop::PERM_CA_SN, "C=DE,O=ZeroDDS,CN=ZeroDDS Permissions CA")
        .with_property(prop::PERM_CA_ALGO, "ECDSA+P256+SHA256");
    assert_eq!(
        token.property(prop::PERM_CA_SN),
        Some("C=DE,O=ZeroDDS,CN=ZeroDDS Permissions CA")
    );
    assert_eq!(
        token.property(prop::PERM_CA_ALGO),
        Some("ECDSA+P256+SHA256")
    );
    assert_eq!(token.property("dds.perm.cert"), None);
    assert!(token.binary_properties.is_empty());

    let credential: PermissionsCredentialToken =
        DataHolder::new(class_id::ACCESS_PERMISSIONS_CREDENTIAL)
            .with_property("dds.perm.cert", "MIME-Version: 1.0");
    assert_eq!(
        credential.property("dds.perm.cert"),
        Some("MIME-Version: 1.0")
    );

    // Table 54: the remote credential token carries `c.id` and `c.perm` as
    // binary properties.
    let remote = DataHolder::new("DDS:Auth:PKI-DH:1.0")
        .with_binary_property("c.id", b"-----BEGIN CERTIFICATE-----".to_vec())
        .with_binary_property("c.perm", b"MIME-Version: 1.0\0".to_vec());
    assert_eq!(
        remote.binary_property("c.id"),
        Some(&b"-----BEGIN CERTIFICATE-----"[..])
    );
    assert_eq!(
        remote.binary_property("c.perm"),
        Some(&b"MIME-Version: 1.0\0"[..])
    );
    assert_eq!(remote.property("c.id"), None);
}

#[test]
fn security_10_3_2_1_class_ids_compare_by_class_name_and_major_version() {
    // `<PluginClassName>:<MajorVersion>.<MinorVersion>`, the class name
    // separated by the last ':'. Table 70 refuses a remote token whose class
    // name or major version differs from the local one; a class without the
    // versions is 1.0 (§10.4.2.2).
    let local = class_id::ACCESS_PERMISSIONS_V12;
    assert!(class_id::same_permissions_class_and_major_version(
        local,
        "DDS:Access:Permissions"
    ));
    assert!(class_id::same_permissions_class_and_major_version(
        local, local
    ));
    assert!(class_id::same_permissions_class_and_major_version(
        local,
        "DDS:Access:Permissions:1.0"
    ));
    assert!(class_id::same_permissions_class_and_major_version(
        local,
        "DDS:Access:Permissions:1.7"
    ));
    assert!(!class_id::same_permissions_class_and_major_version(
        local,
        "DDS:Access:Permissions:2.0"
    ));
    assert!(!class_id::same_permissions_class_and_major_version(
        local,
        "DDS:Access:PSK:1.2"
    ));
    assert!(!class_id::same_permissions_class_and_major_version(
        local,
        "DDS:Auth:PKI-DH:1.2"
    ));
    // `set_token_class_id` refuses a class of another major version.
    assert!(!class_id::same_permissions_class_and_major_version(
        "DDS:Access:Permissions:2.1",
        local
    ));
}

#[test]
fn security_7_3_4_a_token_serializes_as_the_cdr_of_the_data_holder_idl() {
    // `psk_access.rs` sends `DataHolder::new(class).with_property(..)
    // .to_cdr_le()`. The IDL of §7.3.4 is a string, a sequence of
    // Property_t (two strings) and a sequence of BinaryProperty_t; a CDR
    // string is its length with the NUL, the octets and the NUL, and every
    // length is aligned to 4 octets.
    let token = DataHolder::new("DDS:Access:PSK:1.2").with_property("x.id", "ab");
    #[rustfmt::skip]
    let expected: [u8; 52] = [
        0x13, 0x00, 0x00, 0x00, // class_id: 19 octets with the NUL
        b'D', b'D', b'S', b':', b'A', b'c', b'c', b'e', b's', b's', b':',
        b'P', b'S', b'K', b':', b'1', b'.', b'2', 0x00,
        0x00,                   // padding to 24
        0x01, 0x00, 0x00, 0x00, // one Property_t
        0x05, 0x00, 0x00, 0x00, b'x', b'.', b'i', b'd', 0x00,
        0x00, 0x00, 0x00,       // padding to 40
        0x03, 0x00, 0x00, 0x00, b'a', b'b', 0x00,
        0x00,                   // padding to 48
        0x00, 0x00, 0x00, 0x00, // no BinaryProperty_t
    ];
    assert_eq!(token.to_cdr_le(), expected);
    assert_eq!(DataHolder::from_cdr_le(&expected).unwrap(), token);
}

#[test]
fn security_10_6_the_logging_topics_of_the_builtin_topic_rule() {
    // §10.6: the BuiltinLoggingTopic and the legacy topic.
    assert_eq!(TOPIC_LOG_TOPIC_V2, "DDS:Security:LogTopicV2");
    assert_eq!(TOPIC_LOG_TOPIC, "DDS:Security:LogTopic");
}

#[test]
fn security_7_3_2_the_properties_for_the_cryptographic_plugin_are_vendor_names() {
    // §7.3.2: "dds.sec." is the specification's; a vendor uses its domain.
    for name in [
        ENDPOINT_DATA_PROTECTION_KIND,
        ENDPOINT_METADATA_PROTECTION_KIND,
        PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION,
    ] {
        assert!(name.starts_with("org.zerodds."), "{name}");
    }
    // `cms.rs` reads its CA from the participant properties.
    let props = PropertyList::new().with(Property::local(
        "dds.sec.access.permissions_ca",
        "file:/etc/dds/perm_ca.pem",
    ));
    assert_eq!(
        props.get("dds.sec.access.permissions_ca"),
        Some("file:/etc/dds/perm_ca.pem")
    );
    assert_eq!(props.get("dds.sec.access.governance"), None);
}

#[test]
fn security_9_4_2_9_the_requests_and_decisions_of_the_checks() {
    // Rustdoc of `ParticipantRequest::new`: empty DomainTag, no algorithms.
    let props = PropertyList::new();
    let p = ParticipantRequest::new(7, [0x11; 16], &props);
    assert_eq!(
        (p.domain_id, p.domain_tag, p.participant_guid),
        (7, "", [0x11; 16])
    );
    assert!(p.identity_algorithms.is_none());
    let with_algorithms = ParticipantRequest {
        identity_algorithms: Some(AlgorithmMasks {
            trust_chain: (CBIT_ECDSA_P256_SHA256, CBIT_ECDSA_P256_SHA256),
            message_auth: (CRYPTO_ALGORITHM_SET_ALL, CBIT_ECDSA_P256_SHA256),
            key_establishment: (CRYPTO_ALGORITHM_SET_ALL, CBIT_ECDHE_CEUM_P256),
        }),
        ..p
    };
    assert_eq!(
        with_algorithms.identity_algorithms.unwrap().trust_chain.1,
        CBIT_ECDSA_P256_SHA256
    );

    // `EndpointRequest::new` has no partitions, data tags or QoS; the
    // builders set one each and keep the rest.
    let e = EndpointRequest::new(0, "Square");
    assert!(e.partitions.is_empty() && e.data_tags.is_empty() && e.qos.is_empty());
    let partitions = ["A*"];
    let tags = [("level", "secret")];
    let qos = [("reliability", "RELIABLE")];
    let e = e
        .in_partitions(&partitions)
        .with_data_tags(&tags)
        .with_qos(&qos);
    assert_eq!(e.topic_name, "Square");
    assert_eq!(e.partitions, ["A*"]);
    assert_eq!(e.data_tags, [("level", "secret")]);
    assert_eq!(e.qos, [("reliability", "RELIABLE")]);
    let t = TopicRequest::new(3, "Circle");
    assert_eq!((t.domain_id, t.topic_name, t.qos.len()), (3, "Circle", 0));

    // `Ok(Permit)` is TRUE, `Ok(Deny)` FALSE without an exception.
    assert_eq!(AccessDecision::from(true), AccessDecision::Permit);
    assert_eq!(AccessDecision::from(false), AccessDecision::Deny);
    assert!(AccessDecision::Permit.is_permitted());
    assert!(!AccessDecision::Deny.is_permitted());
    // §9.4.2.9.11: the out parameter `relay_only` is a third answer.
    assert_ne!(
        RemoteReaderDecision::RelayOnly,
        RemoteReaderDecision::Permit
    );
    assert_ne!(RemoteReaderDecision::RelayOnly, RemoteReaderDecision::Deny);
    assert_ne!(RelayRole::Reader, RelayRole::Writer);
    assert!(PermissionsHandle::NIL.is_nil());
    assert_eq!(PermissionsHandle::NIL, PermissionsHandle(0));
}

/// A listener as `plugin.rs` holds one.
#[derive(Default)]
struct Revocations(Mutex<Vec<(String, PermissionsHandle)>>);

impl AccessControlListener for Revocations {
    fn on_revoke_permissions(&self, plugin: &str, handle: PermissionsHandle) -> bool {
        self.0.lock().unwrap().push((plugin.to_string(), handle));
        true
    }
}

#[test]
fn security_9_4_2_10_the_listener_is_told_the_plugin_and_the_handle() {
    let revocations = Arc::new(Revocations::default());
    let listener: Arc<dyn AccessControlListener> = revocations.clone();
    assert!(listener.on_revoke_permissions(class_id::ACCESS_PERMISSIONS_V12, PermissionsHandle(4)));
    assert_eq!(
        *revocations.0.lock().unwrap(),
        vec![(
            "DDS:Access:Permissions:1.2".to_string(),
            PermissionsHandle(4)
        )]
    );
}

#[test]
fn security_9_2_1_refusals_carry_a_minor_code() {
    // `plugin.rs` refuses a remote token with `AccessDenied` and a minor
    // code per cause; `issued.rs` refuses a configuration it did not issue
    // with `bad_argument`.
    let e = SecurityError::new(SecurityErrorKind::AccessDenied, "permissions: no grant")
        .with_minor_code(4);
    assert_eq!(e.kind, SecurityErrorKind::AccessDenied);
    assert_eq!(
        (e.code, e.minor_code),
        (SecurityErrorKind::AccessDenied.code(), 4)
    );
    assert_eq!(e.detail, "permissions: no grant");
    let e = SecurityError::bad_argument("permissions: a configuration it did not issue");
    assert_eq!((e.kind, e.minor_code), (SecurityErrorKind::BadArgument, 0));
}

#[test]
fn the_subject_name_forms_of_the_permissions_token() {
    // Rustdoc of `SubjectNameForm`: RFC 4514 is the default; the two OpenSSL
    // forms are named settings.
    assert_eq!(SubjectNameForm::default(), SubjectNameForm::Rfc4514);
    assert_eq!(
        SubjectNameForm::parse("rfc4514"),
        Some(SubjectNameForm::Rfc4514)
    );
    assert_eq!(
        SubjectNameForm::parse("x509_name_oneline"),
        Some(SubjectNameForm::X509NameOneline)
    );
    assert_eq!(
        SubjectNameForm::parse("xn_flag_oneline"),
        Some(SubjectNameForm::XnFlagOneline)
    );
    assert_eq!(SubjectNameForm::parse("oneline"), None);
}
