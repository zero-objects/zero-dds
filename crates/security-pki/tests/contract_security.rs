// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-pki` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security`, from its non-test sources:
//! - `authentication`: `AuthenticationPlugin` and `AuthenticationExtensions`,
//!   implemented by `PkiAuthenticationPlugin` (`plugin.rs`),
//!   `PskAuthenticationPlugin` and `PskHandshakeAuthenticationPlugin`
//!   (`psk.rs`); `SharedSecretProvider`, implemented by both handshake
//!   plugins; the result types `ValidationResult`, `HandshakeStepOutcome`,
//!   `LocalIdentityValidation`, `RemoteIdentityValidation`, `AlgorithmMasks`,
//!   `ParticipantSecurityConfig`, `PeerAlgorithmNames`; the handles
//!   `IdentityHandle` (with `NIL`, `ocsp.rs`), `HandshakeHandle`,
//!   `SharedSecretHandle`; `IssuedTokens` for the `return_*_token`
//!   operations; `AuthenticationListener` and `AuthStatusKind`. `lib.rs`
//!   re-exports `HandshakeHandle`, `HandshakeStepOutcome`, `IdentityHandle`
//!   and `SharedSecretHandle`.
//! - `token`: `DataHolder` (re-exported by `lib.rs` and
//!   `handshake_token.rs`) with `new`, `with_property`,
//!   `with_binary_property`, `binary_property`, `property`, `to_cdr_le` and
//!   `from_cdr_le` for every handshake token, the IdentityToken and the
//!   tokens of the AccessControl plugin; `BinaryProperty`, `WireProperty`;
//!   `class_id::{ACCESS_PERMISSIONS_V12, ACCESS_PERMISSIONS_CREDENTIAL,
//!   same_class_and_major_version}` (`plugin.rs`).
//! - `logging::{LogLevel, LoggingPlugin}`: the installed logger as
//!   `Arc<dyn LoggingPlugin>`, events at `Warning` and `Error`.
//! - `properties::{Property, PropertyList}` (`PropertyList::get` of the
//!   `dds.sec.auth.*` properties, `plugin.rs`, `psk.rs`, `ocsp.rs`).
//! - `error::{SecurityError, SecurityErrorKind, SecurityResult}`.
//! - `SubjectNameForm` (`identity_token.rs`, `plugin.rs`).
//!
//! Oracles: the HandshakeReply an OpenDDS participant sent, captured in
//! `crates/discovery/tests/fixtures/opendds_handshake_reply_head_run.hex`
//! (its message data, a DataHolder of class `DDS:Auth:PKI-DH:1.0+Reply`,
//! starts at octet 124); DDS-Security 1.2 §7.3.3 and §7.3.3.1 (a
//! BinaryProperty_t value; `propagate` is `@non-serialized`), §7.3.4
//! (DataHolder), §9.3.2.11.1 Table 30 (`ValidationResult_t`),
//! §9.3.2.11.7 and §9.3.2.11.8 (after OK or OK_FINAL_MESSAGE the handshake
//! is complete), §9.3.2.11.14 to §9.3.2.11.16 (a returned token is one the
//! plugin issued), §10.3.2.1 and §10.4.2.2 (class id format and the
//! version rule), §10.4.2.1 Table 64, §10.4.2.2 Table 65, §9.6.2.1.1 (the
//! levels), §7.3.9 (`CRYPTO_ALGORITHM_SET_ALL`/`_EMPTY`), and the rustdoc of
//! `IssuedTokens`, `AuthenticationExtensions`, `PeerAlgorithmNames`,
//! `ParticipantSecurityConfig::unrestricted` and `PropertyList`.
#![allow(clippy::unwrap_used)]

use zerodds_security::authentication::{
    AuthenticationExtensions, HandshakeHandle, HandshakeStepOutcome, IdentityHandle, IssuedTokens,
    ParticipantSecurityConfig, PeerAlgorithmNames, SharedSecretHandle, ValidationResult,
};
use zerodds_security::error::SecurityErrorKind;
use zerodds_security::logging::{ERROR_LEVEL, LogLevel, WARNING_LEVEL};
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security::token::{DataHolder, class_id};

/// The octets of the captured OpenDDS HandshakeReply (serialized payload
/// with its encapsulation, `00 01 00 00`).
fn opendds_reply() -> Vec<u8> {
    let text = include_str!("../../discovery/tests/fixtures/opendds_handshake_reply_head_run.hex");
    let text = text.trim();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// Where the DataHolder of the message data starts: the encapsulation (4),
/// two MessageIdentity (2 x 24), three GUIDs (3 x 16), the string
/// "dds.sec.auth" (4 + 13, padded to 20) and the sequence length (4).
const HOLDER_AT: usize = 4 + 48 + 48 + 20 + 4;

#[test]
fn security_7_3_4_the_handshake_reply_of_opendds_reads_as_a_data_holder() {
    let reply = opendds_reply();
    let octets = &reply[HOLDER_AT..];
    let (holder, consumed) = DataHolder::from_cdr_le_consumed(octets).unwrap();
    assert_eq!(consumed, octets.len());
    assert_eq!(holder.class_id, "DDS:Auth:PKI-DH:1.0+Reply");
    assert!(holder.properties.is_empty());
    let names: Vec<&str> = holder
        .binary_properties
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "c.id",
            "c.perm",
            "c.pdata",
            "c.dsign_algo",
            "c.kagree_algo",
            "dh2",
            "challenge1",
            "challenge2",
            "signature",
        ]
    );
    let lens: Vec<usize> = holder
        .binary_properties
        .iter()
        .map(|p| p.value.len())
        .collect();
    assert_eq!(lens, [645, 2439, 340, 13, 21, 65, 32, 32, 71]);
    // §7.3.3.1: OpenDDS counts the NUL of an algorithm name.
    assert_eq!(
        holder.binary_property("c.dsign_algo"),
        Some(&b"ECDSA-SHA256\0"[..])
    );
    assert_eq!(
        holder.binary_property("c.kagree_algo"),
        Some(&b"ECDH+prime256v1-CEUM\0"[..])
    );
    assert_eq!(holder.binary_property("dh2").map(|v| v[0]), Some(0x04));
    assert!(
        holder
            .binary_property("c.id")
            .unwrap()
            .starts_with(b"-----BEGIN CERTIFICATE-----")
    );
    // `propagate` is not on the wire: a property read from it is local.
    assert!(holder.binary_properties.iter().all(|p| !p.propagate));

    // Written back, the holder is the octets OpenDDS sent.
    assert_eq!(holder.to_cdr_le(), octets);
    assert_eq!(DataHolder::from_cdr_le(octets).unwrap(), holder);
    // A cut token is refused.
    assert_eq!(
        DataHolder::from_cdr_le(&octets[..octets.len() - 1])
            .unwrap_err()
            .kind,
        SecurityErrorKind::BadArgument
    );
}

#[test]
fn security_9_3_2_11_1_the_validation_results_of_the_handshake_steps() {
    // Table 30 and the rustdoc of `HandshakeStepOutcome::validation_result`.
    let send = HandshakeStepOutcome::SendMessage { token: vec![1, 2] };
    let fin = HandshakeStepOutcome::SendFinalMessage { token: vec![3] };
    let complete = HandshakeStepOutcome::Complete {
        secret: SharedSecretHandle(9),
    };
    assert_eq!(
        send.validation_result(),
        ValidationResult::PendingHandshakeMessage
    );
    assert_eq!(fin.validation_result(), ValidationResult::OkFinalMessage);
    assert_eq!(complete.validation_result(), ValidationResult::Ok);
    assert_eq!(
        HandshakeStepOutcome::WaitingForPeer.validation_result(),
        ValidationResult::PendingHandshakeMessage
    );
    assert_eq!(
        HandshakeStepOutcome::PendingRetry.validation_result(),
        ValidationResult::PendingRetry
    );
    // The message to send.
    assert_eq!(send.message_out(), Some(&[1u8, 2][..]));
    assert_eq!(fin.message_out(), Some(&[3u8][..]));
    assert_eq!(complete.message_out(), None);
    assert_eq!(HandshakeStepOutcome::WaitingForPeer.message_out(), None);

    // §9.3.2.11.7: complete after OK or OK_FINAL_MESSAGE only.
    assert!(ValidationResult::Ok.is_success());
    assert!(ValidationResult::OkFinalMessage.is_success());
    for pending in [
        ValidationResult::Failed,
        ValidationResult::PendingRetry,
        ValidationResult::PendingHandshakeRequest,
        ValidationResult::PendingHandshakeMessage,
    ] {
        assert!(!pending.is_success(), "{pending:?}");
    }
}

#[test]
fn security_9_3_2_11_14_a_token_is_taken_back_as_often_as_it_was_issued() {
    let mut issued = IssuedTokens::default();
    assert_eq!(issued.issue(1, vec![0xaa]), vec![0xaa]);
    issued.issue(1, vec![0xaa]);
    issued.issue(2, vec![0xaa]);
    assert_eq!(issued.outstanding(), 3);
    // The kind is part of the token's identity.
    assert!(issued.take_back(3, &[0xaa]).is_err());
    issued.take_back(1, &[0xaa]).unwrap();
    issued.take_back(1, &[0xaa]).unwrap();
    assert_eq!(
        issued.take_back(1, &[0xaa]).unwrap_err().kind,
        SecurityErrorKind::BadArgument
    );
    issued.take_back(2, &[0xaa]).unwrap();
    assert_eq!(issued.outstanding(), 0);
    assert!(issued.take_back(2, &[0xbb]).is_err());
}

/// An authentication plugin that keeps every default of
/// `AuthenticationExtensions`.
struct Defaults;

impl AuthenticationExtensions for Defaults {
    fn plugin_class_id(&self) -> &str {
        "DDS:Auth:contract:1.0"
    }
}

#[test]
fn the_authentication_extensions_default_to_the_specification() {
    let mut plugin = Defaults;
    // Rustdoc of `get_permissions_token`: empty when no permissions are
    // configured, and the token is left out of SPDP.
    assert!(plugin.get_permissions_token().is_empty());
    // The setters have bodies that do nothing for a plugin they do not
    // apply to.
    plugin.set_algo_nul_terminate(true);
    plugin.set_remote_message_auth_supported(Some(1));
    plugin.set_peer_algorithm_names(PeerAlgorithmNames::Legacy);
    plugin.check_identities(1_800_000_000);
    assert_eq!(plugin.plugin_class_id(), "DDS:Auth:contract:1.0");
    // The names of Tables 25 and 26 are the default.
    assert_eq!(PeerAlgorithmNames::default(), PeerAlgorithmNames::Spec);

    // §7.3.9: everything supported, nothing required.
    let config = ParticipantSecurityConfig::unrestricted();
    for (supported, required) in [
        config.algorithm_info.trust_chain,
        config.algorithm_info.message_auth,
        config.algorithm_info.key_establishment,
    ] {
        assert_eq!((supported, required), (0xffff_ffff, 0));
    }
}

#[test]
fn security_10_3_2_1_the_class_ids_the_plugin_compares() {
    // Table 64 and Table 65.
    assert_eq!(
        class_id::ACCESS_PERMISSIONS_CREDENTIAL,
        "DDS:Access:PermissionsCredential"
    );
    assert_eq!(
        class_id::ACCESS_PERMISSIONS_V12,
        "DDS:Access:Permissions:1.2"
    );
    // §10.3.2.1: the IdentityToken of Table 51 has the class 1.2, the text
    // below it names MajorVersion 1 and MinorVersion 0; both have the same
    // class name and major version.
    assert!(class_id::same_class_and_major_version(
        "DDS:Auth:PKI-DH:1.2",
        "DDS:Auth:PKI-DH:1.0"
    ));
    assert!(!class_id::same_class_and_major_version(
        "DDS:Auth:PKI-DH:1.0",
        "DDS:Auth:PKI-DH:2.0"
    ));
    assert!(!class_id::same_class_and_major_version(
        "DDS:Auth:PKI-DH:1.0",
        "DDS:Auth:PSK:1.0"
    ));
    // §10.3.2.1 gives the IdentityToken no rule for a class id without
    // versions (§10.4.2.2 has one for the PermissionsToken alone), and the
    // plugin refuses `DDS:Auth:PKI-DH` (its tests
    // `security_9_3_2_11_validate_remote_identity_*`). A class id without
    // versions read as 1.0 for every token broke those tests while this
    // contract stayed green (pipeline 8708).
    assert!(!class_id::same_class_and_major_version(
        "DDS:Auth:PKI-DH",
        "DDS:Auth:PKI-DH:1.0"
    ));
    assert!(!class_id::same_class_and_major_version(
        "DDS:Auth:PKI-DH:1.2",
        "DDS:Auth:PKI-DH"
    ));
    // The handles' nil values.
    assert_eq!(IdentityHandle::NIL, IdentityHandle(0));
    assert_eq!(HandshakeHandle::NIL, HandshakeHandle(0));
    assert_eq!(SharedSecretHandle::NIL, SharedSecretHandle(0));
}

#[test]
fn security_9_6_2_1_1_the_levels_of_the_events_of_this_crate() {
    // A deviation is logged at WARNING_LEVEL, a failed listener at
    // ERROR_LEVEL.
    assert_eq!(LogLevel::Warning.to_spec_level(), WARNING_LEVEL);
    assert_eq!(LogLevel::Error.to_spec_level(), ERROR_LEVEL);
    assert_eq!(LogLevel::Warning.spec_name(), "WARNING_LEVEL");
    assert!(LogLevel::Error < LogLevel::Warning);
}

#[test]
fn the_participant_properties_are_read_by_name_and_the_last_one_wins() {
    // Rustdoc of `PropertyList`: on a duplicate the last entry wins.
    let mut props = PropertyList::new()
        .with(Property::local("dds.sec.auth.identity_ca", "file:old.pem"))
        .with(Property::local(
            "dds.sec.auth.identity_certificate",
            "file:id.pem",
        ));
    props.set(Property::local("dds.sec.auth.identity_ca", "file:ca.pem"));
    assert_eq!(props.get("dds.sec.auth.identity_ca"), Some("file:ca.pem"));
    assert_eq!(
        props.get("dds.sec.auth.identity_certificate"),
        Some("file:id.pem")
    );
    assert_eq!(props.get("dds.sec.auth.private_key"), None);
    assert_eq!(props.entries().len(), 2);
    // A local property is not propagated.
    assert_eq!(props.propagatable().count(), 0);
}
