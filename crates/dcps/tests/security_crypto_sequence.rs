// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §9.8.9 and §9.8.11: the operations the middleware calls on
//! the Cryptographic plugin of two participants that exchange the key material
//! of a DataWriter and a DataReader and then send protected data from the one
//! to the other. The plugin is `RecordingCrypto`, which records each call and
//! delegates to the builtin plugin.
//!
//! * §9.8.9.2, §9.8.9.4: `register_local_datawriter` /
//!   `register_local_datareader`, `register_matched_remote_participant`,
//!   `register_matched_remote_datareader` / `_datawriter`, the CryptoTokens.
//! * §9.8.11.1, §9.8.11.2 (Figures 31, 32): payload, then submessage, then
//!   RTPS message are encoded.
//! * §9.8.11.3, §9.8.11.4 (Figures 33, 34): the RTPS message is decoded,
//!   the submessage is preprocessed and decoded, the payload is decoded.

#![cfg(feature = "security")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_rtps::wire_types::EntityId;
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security_runtime::{ProtectionLevel, SharedSecurityGate};

#[path = "common/recording_crypto.rs"]
mod recording_crypto;

const GOVERNANCE: &str = r#"<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><rtps_protection_kind>ENCRYPT</rtps_protection_kind><topic_access_rules><topic_rule><topic_expression>*</topic_expression><metadata_protection_kind>ENCRYPT</metadata_protection_kind><data_protection_kind>ENCRYPT</data_protection_kind></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#;

const GOVERNANCE_DISCOVERY: &str = r#"<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><discovery_protection_kind>ENCRYPT</discovery_protection_kind><liveliness_protection_kind>ENCRYPT</liveliness_protection_kind><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#;

const ALICE: [u8; 12] = [0xAA; 12];
const BOB: [u8; 12] = [0xBB; 12];

fn gate() -> (SharedSecurityGate, recording_crypto::Log) {
    let (crypto, log) = recording_crypto::RecordingCrypto::new();
    let gate = SharedSecurityGate::new(
        0,
        zerodds_security_permissions::parse_governance_xml(GOVERNANCE).unwrap(),
        Box::new(crypto),
    );
    (gate, log)
}

fn operations(log: &recording_crypto::Log) -> Vec<&'static str> {
    log.lock().unwrap().iter().map(|c| c.operation).collect()
}

/// `wanted` occurs in `ops` in this order (other calls in between are fine).
fn in_order(ops: &[&str], wanted: &[&str]) -> bool {
    let mut at = 0;
    for op in ops {
        if at < wanted.len() && *op == wanted[at] {
            at += 1;
        }
    }
    at == wanted.len()
}

#[test]
fn the_key_material_of_two_endpoints_and_the_protected_data_follow_the_figures() {
    let (alice, alice_log) = gate();
    let (bob, bob_log) = gate();
    let writer = alice.register_local_endpoint(true).expect("writer key");
    let reader = bob.register_local_endpoint(false).expect("reader key");

    // §9.8.9.2: each side registers the other participant.
    alice
        .register_remote_by_guid_from_secret(BOB, IdentityHandle(2), SharedSecretHandle(1))
        .unwrap();
    bob.register_remote_by_guid_from_secret(ALICE, IdentityHandle(1), SharedSecretHandle(1))
        .unwrap();
    bob.set_remote_data_token_by_guid(&ALICE, &alice.local_token_for(&BOB).unwrap())
        .unwrap();
    alice
        .set_remote_data_token_by_guid(&BOB, &bob.local_token_for(&ALICE).unwrap())
        .unwrap();

    // §9.8.9.4: the matched remote DataReader is registered for the DataWriter
    // and its CryptoTokens are made; the DataReader side mirrors it.
    let remote_reader = [&BOB[..], &[0, 0, 1, 4]].concat();
    let remote_writer = [&ALICE[..], &[0, 0, 1, 2]].concat();
    let writer_token = alice
        .create_endpoint_token_for(writer, remote_reader.clone().try_into().unwrap())
        .unwrap();
    let payload_token = alice.endpoint_payload_token(writer);
    let reader_token = bob
        .create_endpoint_token_for(reader, remote_writer.try_into().unwrap())
        .unwrap();
    bob.install_remote_endpoint_token_from(&ALICE, &writer_token, false)
        .unwrap();
    if let Some(token) = &payload_token {
        bob.install_remote_endpoint_token_from(&ALICE, token, false)
            .unwrap();
    }
    alice
        .install_remote_endpoint_token_from(&BOB, &reader_token, true)
        .unwrap();

    let alice_ops = operations(&alice_log);
    assert!(
        in_order(
            &alice_ops,
            &[
                "register_local_datawriter",
                "register_matched_remote_participant",
                "register_matched_remote_datareader",
                "create_local_datawriter_crypto_tokens",
                "set_remote_datareader_crypto_tokens",
            ]
        ),
        "{alice_ops:?}"
    );
    let bob_ops = operations(&bob_log);
    assert!(
        in_order(
            &bob_ops,
            &[
                "register_local_datareader",
                "register_matched_remote_participant",
                "register_matched_remote_datawriter",
                "create_local_datareader_crypto_tokens",
                "set_remote_datawriter_crypto_tokens",
            ]
        ),
        "{bob_ops:?}"
    );

    // §9.8.11.1, §9.8.11.2: payload, submessage, RTPS message.
    let payload = b"a serialized payload";
    let mut submessage = vec![0x15, 0x01, 0x00, 0x00];
    submessage.extend_from_slice(payload);
    let len = (submessage.len() - 4) as u16;
    submessage[2..4].copy_from_slice(&len.to_le_bytes());
    alice_log.lock().unwrap().clear();
    let encoded_payload = alice.encode_serialized_payload(writer, payload).unwrap();
    let encoded_submessage = alice
        .encode_data_datawriter_by_handle(writer, &submessage)
        .unwrap();
    let mut message = b"RTPS\x02\x05\x01\x02".to_vec();
    message.extend_from_slice(&ALICE);
    message.extend_from_slice(&encoded_submessage);
    let wire = alice
        .transform_outbound_for(&BOB, &message, ProtectionLevel::Encrypt)
        .unwrap();
    assert_eq!(
        operations(&alice_log),
        [
            "encode_serialized_payload",
            "encode_datawriter_submessage",
            "encode_rtps_message"
        ]
    );

    // §9.8.11.3, §9.8.11.4: RTPS message, preprocess and submessage, payload.
    bob_log.lock().unwrap().clear();
    let message_back = bob.transform_inbound_from(&ALICE, &wire).unwrap();
    assert_eq!(message_back, message);
    let submessage_back = bob
        .decode_data_by_key_id(&ALICE, &message_back[20..])
        .unwrap();
    assert_eq!(submessage_back, submessage);
    let payload_back = bob
        .decode_serialized_payload(&ALICE, &encoded_payload)
        .unwrap();
    assert_eq!(payload_back, payload);
    assert_eq!(
        operations(&bob_log),
        [
            "decode_rtps_message",
            "preprocess_secure_submsg",
            "decode_datawriter_submessage",
            "decode_serialized_payload"
        ]
    );
}

/// §9.8.9.3: each DataWriter and DataReader of the builtin secure endpoints
/// (the VolatileMessageSecure writer apart, which `register_local_participant`
/// precedes) is registered with `register_local_datawriter` or
/// `register_local_datareader`, with the kinds §10.4.1.2.5 gives it; the
/// matched remote counterpart is registered, and the CryptoTokens are made.
#[test]
fn the_builtin_secure_endpoints_are_registered_and_matched() {
    let (crypto, log) = recording_crypto::RecordingCrypto::new();
    let gate = SharedSecurityGate::new(
        0,
        zerodds_security_permissions::parse_governance_xml(GOVERNANCE_DISCOVERY).unwrap(),
        Box::new(crypto),
    );
    gate.register_remote_by_guid_from_secret(BOB, IdentityHandle(2), SharedSecretHandle(1))
        .unwrap();
    let remote = |entity: EntityId| {
        let mut guid = [0u8; 16];
        guid[..12].copy_from_slice(&BOB);
        guid[12..].copy_from_slice(&entity.to_bytes());
        guid
    };
    let writers = [
        EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
        EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
    ];
    let readers = [
        EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
        EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
    ];
    for (eids, is_writer) in [(writers, true), (readers, false)] {
        for eid in eids {
            log.lock().unwrap().clear();
            let (metadata, data) =
                zerodds_security_runtime::builtin_attributes::builtin_endpoint_kinds(&gate, eid)
                    .unwrap();
            assert_ne!(
                metadata,
                zerodds_security_permissions::ProtectionKind::None,
                "{eid:?}: protected by the governance"
            );
            let handle = gate
                .register_local_endpoint_with_kinds(is_writer, metadata, data)
                .unwrap();
            // The remote counterpart: the reader of a writer and vice versa.
            let counterpart = if is_writer {
                EntityId::from_bytes(
                    writers
                        .iter()
                        .position(|w| *w == eid)
                        .map(|i| readers[i].to_bytes())
                        .unwrap(),
                )
            } else {
                EntityId::from_bytes(
                    readers
                        .iter()
                        .position(|r| *r == eid)
                        .map(|i| writers[i].to_bytes())
                        .unwrap(),
                )
            };
            gate.create_endpoint_token_for(handle, remote(counterpart))
                .unwrap();
            let ops = operations(&log);
            let expected: &[&str] = if is_writer {
                &[
                    "register_local_datawriter",
                    "register_matched_remote_datareader",
                    "create_local_datawriter_crypto_tokens",
                ]
            } else {
                &[
                    "register_local_datareader",
                    "register_matched_remote_datawriter",
                    "create_local_datareader_crypto_tokens",
                ]
            };
            assert_eq!(ops, expected, "{eid:?}");
        }
    }
}
