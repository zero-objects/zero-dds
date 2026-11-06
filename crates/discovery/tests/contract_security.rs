// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-discovery` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security`, from its non-test sources
//! (all under `security/`, which is behind the default feature `alloc`; the
//! handshake driver of `stack.rs` is behind the default feature `std`):
//! - `generic_message::{encode_generic_message, decode_generic_message,
//!   ENCAPSULATION_CDR_LE, ENCAPSULATION_CDR_BE, ENCAPSULATION_HEADER_LEN}`,
//!   re-exported by `codec.rs` and called by the stateless writer and reader
//!   (`stateless.rs`) and the volatile-secure writer and reader
//!   (`volatile_secure.rs`) for the `serialized_payload` of every DATA.
//! - `generic_message::{ParticipantGenericMessage, MessageIdentity,
//!   class_id::{AUTH_REQUEST, AUTH}}`: the writers stamp `message_identity`,
//!   `stack.rs` builds the handshake messages, tells a request from a
//!   reply by `message_class_id` and a NIL `related_message_identity`
//!   (`MessageIdentity::default()`), and filters on
//!   `destination_participant_key` and `destination_endpoint_key`.
//! - `token::DataHolder`: `from_cdr_le` for the SPDP permissions token and
//!   the handshake and credential tokens, `new`, `with_binary_property` and
//!   `binary_property` (`c.id`, `c.perm`, `c.dsign_algo`; with the setting
//!   `AlgoNameNul::Mirror` the replier looks at the last octet of
//!   `c.dsign_algo` to see whether the request carried the NUL).
//! - `AlgoNameNul` (`default`, `nul_for`, `Mirror`).
//! - `authentication::{AuthenticationPlugin, AuthenticationListener,
//!   AuthStatusKind, HandshakeHandle, HandshakeStepOutcome, IdentityHandle,
//!   PeerAlgorithmNames, SharedSecretHandle, ValidationResult}`: the driver
//!   holds the plugin as `Arc<Mutex<dyn AuthenticationPlugin + 'static>>`,
//!   calls the operations of Table 29 and the extensions
//!   `set_algo_nul_terminate`, `set_remote_message_auth_supported`,
//!   `set_peer_algorithm_names` and `check_identities`, and installs its own
//!   `AuthenticationListener`.
//! - `error::{SecurityError, SecurityErrorKind, SecurityResult}`
//!   (`BadArgument`, `AuthenticationFailed`, `Internal`).
//!
//! What it relies on: the generic message travels as CDR_LE with the layout
//! of the IDL, a CDR_BE message of another vendor is read too, an unknown
//! encapsulation or a short payload is an error, binary properties keep
//! every octet, the extension operations have bodies that do nothing, and
//! the handshake outcomes map onto Table 30.
//!
//! Oracles: DDS-Security 1.2 §7.3.22 (the IDL of `MessageIdentity` and
//! `ParticipantGenericMessage`), §7.3.3 (`DataHolder`, `Property_t`,
//! `BinaryProperty_t`; `propagate` is `@non-serialized`), §7.3.3.1 (an
//! algorithm name as a binary property counts the NUL: "the string
//! 'ECDSA-SHA256' shall result in an octet sequence value with length 13"),
//! §7.5.3 (the class ids of the stateless handshake), §9.2.1 Table 28
//! (`SecurityException`), §9.3.2.11.1 Table 30 (`ValidationResult_t`),
//! DDSI-RTPS 2.5 §10.2 (`CDR_BE` 0x0000, `CDR_LE` 0x0001) and the XCDR1
//! alignment rules of DDS-XTypes 1.3 §7.4.1. The expected octets below are
//! written out from the IDL by hand.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use zerodds_security::AlgoNameNul;
use zerodds_security::authentication::{
    AlgorithmMasks, AuthStatusKind, AuthenticationExtensions, AuthenticationListener,
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle,
    LocalIdentityValidation, ParticipantSecurityConfig, PeerAlgorithmNames,
    RemoteIdentityValidation, SharedSecretHandle, ValidationResult,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::generic_message::{
    ENCAPSULATION_CDR_BE, ENCAPSULATION_CDR_LE, ENCAPSULATION_HEADER_LEN, MessageIdentity,
    ParticipantGenericMessage, class_id, decode_generic_message, encode_generic_message,
};
use zerodds_security::properties::PropertyList;
use zerodds_security::token::DataHolder;

const SOURCE_GUID: [u8; 16] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x00, 0x00, 0x02, 0xc2,
];

/// A HandshakeRequest as the stateless writer sends it: one DataHolder with
/// a binary property whose value is not a multiple of four octets.
fn request() -> ParticipantGenericMessage {
    ParticipantGenericMessage {
        message_identity: MessageIdentity {
            source_guid: SOURCE_GUID,
            sequence_number: 7,
        },
        related_message_identity: MessageIdentity::default(),
        destination_participant_key: [0xaa; 16],
        destination_endpoint_key: [0; 16],
        source_endpoint_key: [0xbb; 16],
        message_class_id: class_id::AUTH_REQUEST.to_string(),
        message_data: vec![
            DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")
                .with_binary_property("future_challenge", vec![0x55; 5]),
        ],
    }
}

/// `request()` as the IDL lays it out, in the byte order `le` names, the
/// encapsulation header first. Offsets are those of the body, where XCDR1
/// alignment starts.
fn request_octets(le: bool) -> Vec<u8> {
    let u32_ = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let i64_ = |v: i64| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut o = Vec::new();
    o.extend_from_slice(if le { &[0x00, 0x01] } else { &[0x00, 0x00] });
    o.extend_from_slice(&[0x00, 0x00]); // options
    // 0: message_identity.source_guid, 16: .sequence_number (long long)
    o.extend_from_slice(&SOURCE_GUID);
    o.extend_from_slice(&i64_(7));
    // 24: related_message_identity, NIL
    o.extend_from_slice(&[0; 16]);
    o.extend_from_slice(&i64_(0));
    // 48: destination_participant_guid, 64: destination_endpoint_guid,
    // 80: source_endpoint_guid
    o.extend_from_slice(&[0xaa; 16]);
    o.extend_from_slice(&[0; 16]);
    o.extend_from_slice(&[0xbb; 16]);
    // 96: message_class_id, 20 characters and the NUL
    o.extend_from_slice(&u32_(21));
    o.extend_from_slice(b"dds.sec.auth_request\0");
    o.extend_from_slice(&[0, 0, 0]); // 121 -> 124
    // 124: message_data, one DataHolder
    o.extend_from_slice(&u32_(1));
    // 128: class_id, 27 characters and the NUL
    o.extend_from_slice(&u32_(28));
    o.extend_from_slice(b"DDS:Auth:PKI-DH:1.2+AuthReq\0");
    // 160: properties, none (`propagate` is not serialized)
    o.extend_from_slice(&u32_(0));
    // 164: binary_properties, one
    o.extend_from_slice(&u32_(1));
    // 168: name, 16 characters and the NUL
    o.extend_from_slice(&u32_(17));
    o.extend_from_slice(b"future_challenge\0");
    o.extend_from_slice(&[0, 0, 0]); // 189 -> 192
    // 192: value, five octets
    o.extend_from_slice(&u32_(5));
    o.extend_from_slice(&[0x55; 5]);
    o
}

#[test]
fn security_7_3_22_a_generic_message_is_cdr_le_with_the_layout_of_the_idl() {
    let wire = encode_generic_message(&request());
    assert_eq!(&wire[..2], &ENCAPSULATION_CDR_LE);
    assert_eq!(ENCAPSULATION_HEADER_LEN, 4);
    assert_eq!(wire, request_octets(true));
    assert_eq!(decode_generic_message(&wire).unwrap(), request());
}

#[test]
fn rtps_10_2_a_big_endian_generic_message_is_read_in_its_byte_order() {
    let wire = request_octets(false);
    assert_eq!(&wire[..2], &ENCAPSULATION_CDR_BE);
    assert_eq!(decode_generic_message(&wire).unwrap(), request());
}

#[test]
fn rtps_10_2_another_encapsulation_or_a_short_payload_is_bad_argument() {
    let full = request_octets(true);
    let mut pl_cdr = full.clone();
    pl_cdr[1] = 0x03; // PL_CDR_LE
    for payload in [&pl_cdr[..], &full[..3], &full[..150]] {
        let err = decode_generic_message(payload).expect_err("not a generic message");
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}

#[test]
fn security_7_3_3_1_an_algorithm_name_keeps_its_nul_through_the_token_codec() {
    // `Mirror` reads the last octet of the request's `c.dsign_algo`.
    let with_nul = DataHolder::new("DDS:Auth:PKI-DH:1.2+Req")
        .with_binary_property("c.dsign_algo", b"ECDSA-SHA256\0".to_vec());
    let mut octets = Vec::new();
    octets.extend_from_slice(&24u32.to_le_bytes());
    octets.extend_from_slice(b"DDS:Auth:PKI-DH:1.2+Req\0");
    octets.extend_from_slice(&0u32.to_le_bytes()); // properties
    octets.extend_from_slice(&1u32.to_le_bytes()); // binary_properties
    octets.extend_from_slice(&13u32.to_le_bytes());
    octets.extend_from_slice(b"c.dsign_algo\0");
    octets.extend_from_slice(&[0, 0, 0]); // 53 -> 56
    octets.extend_from_slice(&13u32.to_le_bytes()); // length 13, §7.3.3.1
    octets.extend_from_slice(b"ECDSA-SHA256\0");
    assert_eq!(with_nul.to_cdr_le(), octets);

    let back = DataHolder::from_cdr_le(&octets).unwrap();
    let name = back.binary_property("c.dsign_algo").unwrap();
    assert_eq!(name.len(), 13);
    assert_eq!(name.last(), Some(&0));

    // The form of a peer that leaves the NUL out.
    let without = DataHolder::new("DDS:Auth:PKI-DH:1.2+Req")
        .with_binary_property("c.dsign_algo", b"ECDSA-SHA256".to_vec());
    let back = DataHolder::from_cdr_le(&without.to_cdr_le()).unwrap();
    let name = back.binary_property("c.dsign_algo").unwrap();
    assert_eq!(name, b"ECDSA-SHA256");
    assert_ne!(name.last(), Some(&0));
}

#[test]
fn security_7_3_3_1_the_default_form_writes_the_nul_for_every_peer() {
    assert_eq!(AlgoNameNul::default(), AlgoNameNul::Spec);
    for peer_is_opendds in [false, true] {
        assert!(AlgoNameNul::default().nul_for(peer_is_opendds));
        // `Mirror` starts with the NUL; the replier drops it only for a
        // request without one.
        assert!(AlgoNameNul::Mirror.nul_for(peer_is_opendds));
    }
    assert_eq!(AlgoNameNul::parse("mirror"), Some(AlgoNameNul::Mirror));
}

#[test]
fn security_7_5_3_the_handshake_class_ids_and_the_nil_identity() {
    assert_eq!(class_id::AUTH_REQUEST, "dds.sec.auth_request");
    assert_eq!(class_id::AUTH, "dds.sec.auth");
    // A request carries a NIL related_message_identity; replies and the
    // final message name the message they answer.
    let nil = MessageIdentity::default();
    assert!(nil.is_nil());
    assert_eq!(nil.source_guid, [0; 16]);
    assert_eq!(nil.sequence_number, 0);
    assert!(
        !MessageIdentity {
            source_guid: SOURCE_GUID,
            sequence_number: 1,
        }
        .is_nil()
    );
}

#[test]
fn security_9_2_1_an_error_keeps_its_kind_code_and_message() {
    for kind in [
        SecurityErrorKind::BadArgument,
        SecurityErrorKind::AuthenticationFailed,
        SecurityErrorKind::Internal,
    ] {
        let error = SecurityError::new(kind, "stateless: wire decode failed");
        assert_eq!(error.kind, kind);
        assert_eq!(error.code, kind.code());
        assert_eq!(error.minor_code, 0);
        assert!(error.to_string().contains("stateless: wire decode failed"));
        // The driver keeps failures and hands them out later.
        let kept = error.clone();
        assert_eq!(kept, error);
    }
}

#[test]
fn security_9_3_2_11_1_handshake_outcomes_map_onto_table_30() {
    let send = HandshakeStepOutcome::SendMessage { token: vec![1, 2] };
    let fin = HandshakeStepOutcome::SendFinalMessage { token: vec![3] };
    let done = HandshakeStepOutcome::Complete {
        secret: SharedSecretHandle(9),
    };
    assert_eq!(
        send.validation_result(),
        ValidationResult::PendingHandshakeMessage
    );
    assert_eq!(fin.validation_result(), ValidationResult::OkFinalMessage);
    assert_eq!(done.validation_result(), ValidationResult::Ok);
    assert_eq!(
        HandshakeStepOutcome::WaitingForPeer.validation_result(),
        ValidationResult::PendingHandshakeMessage
    );
    assert_eq!(
        HandshakeStepOutcome::PendingRetry.validation_result(),
        ValidationResult::PendingRetry
    );
    assert_eq!(send.message_out(), Some(&[1u8, 2][..]));
    assert_eq!(fin.message_out(), Some(&[3u8][..]));
    assert_eq!(done.message_out(), None);
    // Only VALIDATION_OK and VALIDATION_OK_FINAL_MESSAGE complete it.
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

/// A plugin with the operations of Table 29 only; the extensions keep their
/// default bodies.
struct TablePlugin;

impl AuthenticationExtensions for TablePlugin {
    fn plugin_class_id(&self) -> &str {
        "DDS:Auth:Contract:1.2"
    }
}

impl AuthenticationPlugin for TablePlugin {
    fn validate_local_identity(
        &mut self,
        _domain_id: u32,
        _participant_qos: &PropertyList,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        Ok(LocalIdentityValidation {
            result: ValidationResult::Ok,
            identity: IdentityHandle(1),
            adjusted_participant_guid: candidate_participant_guid,
        })
    }

    fn get_identity_token(&mut self, _handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    fn get_identity_status_token(&mut self, _handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    fn set_participant_security_config(
        &mut self,
        _handle: IdentityHandle,
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<AlgorithmMasks> {
        Ok(participant_security_config.algorithm_info)
    }

    fn set_permissions_credential_and_token(
        &mut self,
        _handle: IdentityHandle,
        _permissions_credential_token: &[u8],
        _permissions_token: &[u8],
    ) -> SecurityResult<()> {
        Ok(())
    }

    fn validate_remote_identity(
        &mut self,
        _local: IdentityHandle,
        _remote_participant_guid: [u8; 16],
        _remote_identity_token: &[u8],
        _remote_auth_request_token: Option<&[u8]>,
    ) -> SecurityResult<RemoteIdentityValidation> {
        Ok(RemoteIdentityValidation {
            result: ValidationResult::PendingHandshakeRequest,
            identity: IdentityHandle(2),
            local_auth_request_token: None,
        })
    }

    fn begin_handshake_request(
        &mut self,
        _initiator: IdentityHandle,
        _replier: IdentityHandle,
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        Ok((
            HandshakeHandle(3),
            HandshakeStepOutcome::SendMessage { token: vec![0xa1] },
        ))
    }

    fn begin_handshake_reply(
        &mut self,
        _replier: IdentityHandle,
        _initiator: IdentityHandle,
        _request_token: &[u8],
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        Ok((
            HandshakeHandle(4),
            HandshakeStepOutcome::SendMessage { token: vec![0xa2] },
        ))
    }

    fn process_handshake(
        &mut self,
        _handshake: HandshakeHandle,
        _token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        Ok(HandshakeStepOutcome::Complete {
            secret: SharedSecretHandle(5),
        })
    }

    fn shared_secret(&self, _handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        Ok(SharedSecretHandle(5))
    }

    fn get_authenticated_peer_credential_token(
        &mut self,
        _handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    fn set_listener(
        &mut self,
        _listener: Option<Arc<dyn AuthenticationListener>>,
    ) -> SecurityResult<()> {
        Ok(())
    }

    fn return_identity_token(&mut self, _token: &[u8]) -> SecurityResult<()> {
        Ok(())
    }

    fn return_identity_status_token(&mut self, _token: &[u8]) -> SecurityResult<()> {
        Ok(())
    }

    fn return_authenticated_peer_credential_token(&mut self, _token: &[u8]) -> SecurityResult<()> {
        Ok(())
    }

    fn return_handshake_handle(&mut self, _handshake: HandshakeHandle) -> SecurityResult<()> {
        Ok(())
    }

    fn return_identity_handle(&mut self, _handle: IdentityHandle) -> SecurityResult<()> {
        Ok(())
    }

    fn return_sharedsecret_handle(&mut self, _handle: SharedSecretHandle) -> SecurityResult<()> {
        Ok(())
    }
}

/// The listener the driver installs.
struct Events(Mutex<Vec<(IdentityHandle, Option<AuthStatusKind>)>>);

impl AuthenticationListener for Events {
    fn on_revoke_identity(&self, handle: IdentityHandle) -> SecurityResult<()> {
        self.0.lock().unwrap().push((handle, None));
        Ok(())
    }

    fn on_status_changed(&self, handle: IdentityHandle, status_kind: AuthStatusKind) {
        self.0.lock().unwrap().push((handle, Some(status_kind)));
    }
}

#[test]
fn a_plugin_of_table_29_alone_is_driven_through_a_shared_trait_object() {
    let plugin: Arc<Mutex<dyn AuthenticationPlugin + 'static>> = Arc::new(Mutex::new(TablePlugin));
    let mut guard = plugin.lock().unwrap();
    // The settings the driver gives per peer before a step: the defaults of
    // the extensions take them and do nothing.
    guard.set_algo_nul_terminate(AlgoNameNul::default().nul_for(false));
    guard.set_remote_message_auth_supported(None);
    guard.set_peer_algorithm_names(PeerAlgorithmNames::Legacy);
    guard.check_identities(0);
    assert!(
        guard.get_permissions_token().is_empty(),
        "no PermissionsToken unless the plugin has one"
    );
    assert_eq!(PeerAlgorithmNames::default(), PeerAlgorithmNames::Spec);

    let events = Arc::new(Events(Mutex::new(Vec::new())));
    guard
        .set_listener(Some(events.clone() as Arc<dyn AuthenticationListener>))
        .unwrap();
    let remote = guard
        .validate_remote_identity(IdentityHandle(1), [0xaa; 16], &[], None)
        .unwrap();
    assert_eq!(remote.result, ValidationResult::PendingHandshakeRequest);
    let (handle, step) = guard
        .begin_handshake_request(IdentityHandle(1), remote.identity, &[])
        .unwrap();
    assert_eq!(step.message_out(), Some(&[0xa1u8][..]));
    let done = guard.process_handshake(handle, &[0xb1]).unwrap();
    assert!(done.validation_result().is_success());
    assert_eq!(guard.shared_secret(handle).unwrap(), SharedSecretHandle(5));
    drop(guard);

    let listener: Arc<dyn AuthenticationListener> = events.clone();
    listener.on_status_changed(IdentityHandle(2), AuthStatusKind::IdentityStatus);
    listener.on_revoke_identity(IdentityHandle(2)).unwrap();
    assert_eq!(
        *events.0.lock().unwrap(),
        [
            (IdentityHandle(2), Some(AuthStatusKind::IdentityStatus)),
            (IdentityHandle(2), None),
        ]
    );
}
