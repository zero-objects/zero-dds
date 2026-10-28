// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The HandshakeReply an OpenDDS participant sent to ZeroDDS, as captured
//! (UDP payload of the stateless DATA after the RTPS header: the serialized
//! payload with its encapsulation). The octets are the capture's; they carry
//! no secret (certificates, public DH value, signature). Two runs: the one
//! the handshake completed with and the one it did not.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use zerodds_security::generic_message::{class_id, decode_generic_message};
use zerodds_security_pki::handshake_token::{
    parse_reply_token, parse_request_token, reply_signing_bytes,
};
use zerodds_security_pki::verify_der_signature;

fn unhex(text: &str) -> Vec<u8> {
    let text = text.trim();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

fn check(fixture: &str) {
    let payload = unhex(fixture);
    let message = decode_generic_message(&payload).expect("the reply decodes");
    assert_eq!(message.message_class_id, class_id::AUTH);
    assert_eq!(message.message_data.len(), 1);
    let token = message.message_data[0].to_cdr_le();
    let view = parse_reply_token(&token).expect("the reply token parses");
    assert_eq!(view.dsign_algo, "ECDSA-SHA256");
    assert_eq!(view.kagree_algo, "ECDH+prime256v1-CEUM");
}

#[test]
fn the_reply_of_the_completed_run_parses() {
    check(include_str!(
        "fixtures/opendds_handshake_reply_base_run.hex"
    ));
}

#[test]
fn the_reply_of_the_stalled_run_parses() {
    check(include_str!(
        "fixtures/opendds_handshake_reply_head_run.hex"
    ));
}

/// The signature of the reply of the stalled run verifies with the
/// certificate the reply carries, over the signed content of DDS-Security 1.2
/// §9.3.2.5.2.2 — `hash_c1` and `dh1` from the request ZeroDDS sent in that
/// run (the reply does not carry them).
#[test]
fn the_signature_of_the_stalled_reply_verifies() {
    let request = decode_generic_message(&unhex(include_str!(
        "fixtures/zerodds_handshake_request_head_run.hex"
    )))
    .expect("the request decodes");
    let request = parse_request_token(&request.message_data[0].to_cdr_le())
        .expect("the request token parses");
    let reply = decode_generic_message(&unhex(include_str!(
        "fixtures/opendds_handshake_reply_head_run.hex"
    )))
    .expect("the reply decodes");
    let view = parse_reply_token(&reply.message_data[0].to_cdr_le()).expect("parses");
    assert_eq!(view.challenge1, request.challenge1);
    let signed = reply_signing_bytes(
        &view.hash_c2,
        &view.challenge2,
        &view.dh2,
        &view.challenge1,
        &request.dh1,
        &request.hash_c1,
    );
    // `c.id` carries the certificate as PEM.
    let cert_der =
        zerodds_security_pki::first_cert_der(&view.cert_der).expect("c.id holds a certificate");
    verify_der_signature(&cert_der, &signed, &view.signature)
        .expect("the signature of OpenDDS verifies");
}

/// The reply names, in `related_message_identity`, the message identity of
/// the request ZeroDDS sent (DDS-Security 1.2 §9.3.2.11.6: `process_handshake`
/// is called for a message that relates to the last one sent). OpenDDS echoes
/// the identity octet for octet, the writer GUID of ZeroDDS included.
#[test]
fn the_reply_of_opendds_relates_to_the_request_of_zerodds() {
    let request = decode_generic_message(&unhex(include_str!(
        "fixtures/zerodds_handshake_request_head_run.hex"
    )))
    .expect("the request decodes");
    let reply = decode_generic_message(&unhex(include_str!(
        "fixtures/opendds_handshake_reply_head_run.hex"
    )))
    .expect("the reply decodes");
    assert_eq!(request.message_identity.sequence_number, 1);
    assert_eq!(
        request.message_identity, reply.related_message_identity,
        "request {:?}, reply relates to {:?}",
        request.message_identity, reply.related_message_identity
    );
    assert_eq!(
        reply.destination_participant_key[..12],
        request.message_identity.source_guid[..12]
    );
}
