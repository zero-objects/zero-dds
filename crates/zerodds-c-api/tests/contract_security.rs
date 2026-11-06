// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security` (only with its feature
//! `security`, see `Cargo.toml`):
//! - `plugin_ffi.rs` and `plugin_ffi/security_plugins.rs`: the adapters
//!   that implement `AccessControlPlugin`, `LoggingPlugin`,
//!   `DataTaggingPlugin`, `AuthenticationPlugin` (with
//!   `AuthenticationExtensions` and `SharedSecretProvider`) and the three
//!   interfaces of `CryptographicPluginInterfaces` by calling the function
//!   tables of `include/zerodds_security_plugins.h`. They translate between
//!   the C values of the header and the Rust types: the status codes into
//!   `SecurityErrorKind` (`kind_of_code`) with `SecurityError::new`,
//!   `with_minor_code` and the `code` field; the `ZERODDS_LOG_*` levels with
//!   `LogLevel::from_spec_level` and `to_spec_level`; the answers of the
//!   checks with `AccessDecision::from(bool)`, `RemoteReaderDecision`,
//!   `RelayRole`; `ValidationResult` and `HandshakeStepOutcome`;
//!   `SecureSubmessageCategory`; `Property`, `BinaryProperty`,
//!   `WireProperty`, `DataHolder` field by field; `DataTag`; the handles as
//!   their `u64`.
//! - `security_ffi.rs`: `AuthenticationPlugin::validate_local_identity` with
//!   a `PropertyList` of `Property::local` entries, retried while it returns
//!   `ValidationResult::PendingRetry` and accepted when `is_success`;
//!   `AccessControlPlugin::{validate_local_permissions,
//!   check_create_participant}` with `ParticipantRequest::new` and
//!   `AccessDecision::is_permitted`.
//!
//! What it relies on: the numbers of the header and the Rust values agree
//! both ways, a property keeps its `propagate` flag and its value through
//! the struct fields, and the handle `0` is the nil handle.
//!
//! Oracles: the published C header `include/zerodds_security_plugins.h` of
//! this crate (status codes "Table 28: 1 authentication failed ... 7
//! internal", `ZERODDS_LOG_FATAL` 0 to `ZERODDS_LOG_TRACE` 7,
//! `ZERODDS_VALIDATION_*`, `ZERODDS_PLUGIN_DENY`/`PERMIT`/`RELAY_ONLY`,
//! `ZERODDS_SUBMESSAGE_*`), which C, C++ and Java plugins are written
//! against; DDS-Security 1.2 §9.2.1 Table 28 (`SecurityException`),
//! §9.6.2.1.1 (the eight log levels), §9.3.2.11.1 Table 30
//! (`ValidationResult_t`), §7.3.3 (`propagate`), §7.3.17 (the int holder of
//! a key revision is `256*256*r[0] + 256*r[1] + r[2]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_security::access_control::{
    AccessDecision, ParticipantRequest, PermissionsHandle, RelayRole, RemoteReaderDecision,
};
use zerodds_security::algorithm::{
    CryptoTransformKeyRevisionIntHolder, key_revision_from_int, key_revision_to_int,
};
use zerodds_security::authentication::{
    AlgorithmMasks, HandshakeStepOutcome, IdentityHandle, SharedSecretHandle, ValidationResult,
};
use zerodds_security::cryptographic::{
    DatareaderCryptoHandle, DatawriterCryptoHandle, ParticipantCryptoHandle,
    SecureSubmessageCategory,
};
use zerodds_security::data_tagging::DataTag;
use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::logging::LogLevel;
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security::token::{BinaryProperty, DataHolder, WireProperty};

/// The status codes of the header, "Status" section.
const HEADER_STATUS: [(i32, SecurityErrorKind); 7] = [
    (1, SecurityErrorKind::AuthenticationFailed),
    (2, SecurityErrorKind::AccessDenied),
    (3, SecurityErrorKind::CryptoFailed),
    (4, SecurityErrorKind::InvalidConfiguration),
    (5, SecurityErrorKind::BadArgument),
    (6, SecurityErrorKind::NotImplemented),
    (7, SecurityErrorKind::Internal),
];

#[test]
fn security_9_2_1_the_status_codes_of_the_header_are_the_codes_of_the_kinds() {
    for (status, kind) in HEADER_STATUS {
        assert_eq!(kind.code(), status, "{kind:?}");
        // What `check` builds from a failed C call.
        let mut error =
            SecurityError::new(kind, "the plugin of the application failed").with_minor_code(42);
        error.code = status;
        assert_eq!(error.kind, kind);
        assert_eq!(error.code, status);
        assert_eq!(error.minor_code, 42);
        assert!(
            error
                .to_string()
                .contains("the plugin of the application failed")
        );
    }
    // A new error starts with the code of its kind and minor code 0.
    let error = SecurityError::new(SecurityErrorKind::NotImplemented, "no operation");
    assert_eq!((error.code, error.minor_code), (6, 0));
}

#[test]
fn security_9_6_2_1_1_the_log_levels_of_the_header_round_trip() {
    // ZERODDS_LOG_FATAL = 0 ... ZERODDS_LOG_TRACE = 7.
    let header = [
        (0, LogLevel::Emergency),
        (1, LogLevel::Critical),
        (2, LogLevel::Error),
        (3, LogLevel::Warning),
        (4, LogLevel::Notice),
        (5, LogLevel::Informational),
        (6, LogLevel::Debug),
        (7, LogLevel::Trace),
    ];
    for (number, level) in header {
        assert_eq!(LogLevel::from_spec_level(number), Some(level), "{number}");
        assert_eq!(level.to_spec_level(), number);
    }
    // ALERT has no number of its own; it goes out as SEVERE.
    assert_eq!(LogLevel::Alert.to_spec_level(), 1);
    // A number outside the header is refused (the listener returns -1).
    for number in [-1, 8, i32::MAX] {
        assert_eq!(LogLevel::from_spec_level(number), None, "{number}");
    }
}

#[test]
fn security_9_3_2_11_1_the_validation_results_and_steps_of_the_header() {
    // ZERODDS_VALIDATION_OK = 0 ... OK_FINAL_MESSAGE = 5, in the order of
    // Table 30; only OK and OK_FINAL_MESSAGE are a success.
    let header = [
        (0, ValidationResult::Ok, true),
        (1, ValidationResult::Failed, false),
        (2, ValidationResult::PendingRetry, false),
        (3, ValidationResult::PendingHandshakeRequest, false),
        (4, ValidationResult::PendingHandshakeMessage, false),
        (5, ValidationResult::OkFinalMessage, true),
    ];
    for (number, result, success) in header {
        assert_eq!(result.is_success(), success, "{number}: {result:?}");
    }
    // ZERODDS_HANDSHAKE_SEND_MESSAGE = 0 ... PENDING_RETRY = 4.
    let steps = [
        (
            HandshakeStepOutcome::SendMessage { token: vec![1] },
            ValidationResult::PendingHandshakeMessage,
        ),
        (
            HandshakeStepOutcome::SendFinalMessage { token: vec![2] },
            ValidationResult::OkFinalMessage,
        ),
        (
            HandshakeStepOutcome::Complete {
                secret: SharedSecretHandle(99),
            },
            ValidationResult::Ok,
        ),
        (
            HandshakeStepOutcome::WaitingForPeer,
            ValidationResult::PendingHandshakeMessage,
        ),
        (
            HandshakeStepOutcome::PendingRetry,
            ValidationResult::PendingRetry,
        ),
    ];
    for (step, result) in steps {
        assert_eq!(step.validation_result(), result, "{step:?}");
    }
}

#[test]
fn the_answers_of_the_checks_follow_the_header() {
    assert_eq!(AccessDecision::from(true), AccessDecision::Permit);
    assert_eq!(AccessDecision::from(false), AccessDecision::Deny);
    assert!(AccessDecision::Permit.is_permitted());
    assert!(!AccessDecision::Deny.is_permitted());
    // ZERODDS_PLUGIN_DENY = 0, PERMIT = 1, RELAY_ONLY = 2: three distinct
    // answers of check_remote_datareader.
    let answers = [
        RemoteReaderDecision::Deny,
        RemoteReaderDecision::Permit,
        RemoteReaderDecision::RelayOnly,
    ];
    for (i, a) in answers.iter().enumerate() {
        for b in &answers[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // ZERODDS_PLUGIN_RELAY_READER = 0, RELAY_WRITER = 1.
    assert_ne!(RelayRole::Reader, RelayRole::Writer);
    // ZERODDS_SUBMESSAGE_INFO = 0, DATAWRITER = 1, DATAREADER = 2.
    let categories = [
        SecureSubmessageCategory::InfoSubmessage,
        SecureSubmessageCategory::DatawriterSubmessage,
        SecureSubmessageCategory::DatareaderSubmessage,
    ];
    assert_ne!(categories[0], categories[1]);
    assert_ne!(categories[1], categories[2]);
    assert_ne!(categories[0], categories[2]);
}

#[test]
fn security_7_3_3_properties_keep_their_value_and_propagate_flag() {
    // A ZeroDdsPluginProperty becomes a Property field by field.
    let local = Property::local("dds.sec.auth.identity_ca", "file:/ca.pem");
    let shared = Property::propagated("dds.sec.access.permissions_hash", "ab");
    assert!(!local.propagate);
    assert!(shared.propagate);
    assert_eq!(local.name, "dds.sec.auth.identity_ca");
    assert_eq!(local.value, "file:/ca.pem");

    let mut list = PropertyList::new();
    list.set(local);
    list.set(shared);
    // The configuration path sets each Table 50 property once; a second set
    // of the same name replaces the first.
    list.set(Property::local(
        "dds.sec.auth.identity_ca",
        "file:/other.pem",
    ));
    assert_eq!(
        list.get("dds.sec.auth.identity_ca"),
        Some("file:/other.pem")
    );
    assert_eq!(list.entries().len(), 2);
    let propagated: Vec<&str> = list.propagatable().map(|p| &*p.name).collect();
    assert_eq!(propagated, ["dds.sec.access.permissions_hash"]);

    let binary = BinaryProperty::new("c.id", vec![1, 2, 3]);
    assert!(!binary.propagate);
    assert!(BinaryProperty::propagated("c.perm", vec![4]).propagate);

    // A ZeroDdsPluginDataHolder becomes a DataHolder field by field.
    let holder = DataHolder {
        class_id: "DDS:Access:Permissions:1.2".to_string(),
        properties: vec![WireProperty {
            name: "dds.perm_ca.sn".to_string(),
            value: "CN=ca".to_string(),
        }],
        binary_properties: vec![binary],
    };
    assert_eq!(holder.property("dds.perm_ca.sn"), Some("CN=ca"));
    assert_eq!(holder.binary_property("c.id"), Some(&[1u8, 2, 3][..]));
    assert_eq!(
        DataHolder::from_cdr_le(&holder.to_cdr_le()).unwrap(),
        holder,
        "a token handed out by a C plugin crosses the wire unchanged"
    );

    let tag = DataTag {
        name: "classification".to_string(),
        value: "secret".to_string(),
    };
    // A ZeroDdsPluginPair becomes a DataTag and back.
    assert_eq!(
        (tag.name.as_str(), tag.value.as_str()),
        ("classification", "secret")
    );
}

#[test]
fn the_handles_are_their_u64_and_zero_is_nil() {
    assert_eq!(IdentityHandle(0), IdentityHandle::NIL);
    assert_eq!(IdentityHandle(7).0, 7);
    assert!(PermissionsHandle(0).is_nil());
    assert!(!PermissionsHandle(3).is_nil());
    assert!(ParticipantCryptoHandle(0).is_nil());
    assert!(DatawriterCryptoHandle(0).is_nil());
    assert!(DatareaderCryptoHandle(0).is_nil());
    assert_eq!(SharedSecretHandle(99).0, 99);
}

#[test]
fn security_7_3_17_the_key_revision_int_holder() {
    let holder: CryptoTransformKeyRevisionIntHolder = key_revision_to_int([0x01, 0x02, 0x03]);
    assert_eq!(holder, 256 * 256 + 2 * 256 + 3);
    assert_eq!(key_revision_from_int(holder), [0x01, 0x02, 0x03]);
    assert_eq!(key_revision_to_int([0xff, 0xff, 0xff]), 0x00ff_ffff);
}

#[test]
fn a_participant_request_names_the_domain_guid_and_properties() {
    // `install_access_control` asks with an empty property list.
    let properties = PropertyList::new();
    let request = ParticipantRequest::new(5, [0x11; 16], &properties);
    assert_eq!(request.domain_id, 5);
    assert_eq!(request.participant_guid, [0x11; 16]);
    assert_eq!(request.domain_tag, "");
    assert!(request.properties.entries().is_empty());
    assert!(request.identity_algorithms.is_none());
    // ZeroDdsPluginAlgorithmMasks: (supported, required) per field.
    let masks = AlgorithmMasks {
        trust_chain: (0b111, 0b001),
        message_auth: (0b110, 0b010),
        key_establishment: (0b011, 0b001),
    };
    let ParticipantRequest {
        identity_algorithms,
        ..
    } = ParticipantRequest {
        identity_algorithms: Some(masks),
        ..request
    };
    assert_eq!(identity_algorithms, Some(masks));
}
