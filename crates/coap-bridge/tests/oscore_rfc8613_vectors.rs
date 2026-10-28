// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RFC 8613 Appendix C full-message test vectors — byte-exact through the
//! whole pipeline: CoAP codec decode → OSCORE protect → CoAP codec encode,
//! compared against the protected message octets the RFC prints, and the
//! unprotect direction back to the unprotected octets.
//!
//! * C.4 — protected GET request, keys of C.1 (empty client Sender ID).
//! * C.5 — protected GET request, keys of C.2 (Sender ID `0x00`, no salt).
//! * C.6 — protected GET request, keys of C.3 (ID Context in the option).
//! * C.7 — protected 2.05 response without Partial IV (request nonce reuse).
//! * C.8 — protected 2.05 response with the server's own Partial IV.

#![cfg(feature = "oscore")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_coap_bridge::codec::{decode, encode};
use zerodds_coap_bridge::oscore::message::{
    protect_request_message, protect_response_message, unprotect_request_message,
    unprotect_response_message,
};
use zerodds_coap_bridge::oscore::{AeadAlgorithm, SecurityContext};

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// C.1.1 client / C.1.2 server contexts (Master Salt present).
fn c1_contexts() -> (SecurityContext, SecurityContext) {
    let ms = hex("0102030405060708090a0b0c0d0e0f10");
    let salt = hex("9e7ca92223786340");
    let client = SecurityContext::derive(
        &ms,
        &salt,
        &[],
        &[0x01],
        None,
        AeadAlgorithm::AesCcm16_64_128,
    );
    let server = SecurityContext::derive(
        &ms,
        &salt,
        &[0x01],
        &[],
        None,
        AeadAlgorithm::AesCcm16_64_128,
    );
    (client, server)
}

// C.4 — OSCORE request, client, keys of C.1. Sender Sequence Number 20.
#[test]
fn c4_protected_request_is_byte_exact() {
    let (client, server) = c1_contexts();
    let unprotected = hex("44015d1f00003974396c6f63616c686f737483747631");
    let expected = hex("44025d1f00003974396c6f63616c686f7374620914ff612f1092f1776f1c1668b3825e");

    let inner = decode(&unprotected).unwrap();
    let outer = protect_request_message(&client, &[], None, &[0x14], &inner).unwrap();
    assert_eq!(encode(&outer).unwrap(), expected, "C.4 protected request");

    // Unprotect back to the unprotected octets.
    let received = decode(&expected).unwrap();
    let recovered = unprotect_request_message(&server, &received).unwrap();
    assert_eq!(encode(&recovered).unwrap(), unprotected, "C.4 unprotect");
}

// C.5 — OSCORE request, client, keys of C.2 (no Master Salt, Sender ID 0x00).
#[test]
fn c5_protected_request_is_byte_exact() {
    let ms = hex("0102030405060708090a0b0c0d0e0f10");
    let client = SecurityContext::derive(
        &ms,
        &[],
        &[0x00],
        &[0x01],
        None,
        AeadAlgorithm::AesCcm16_64_128,
    );
    let server = SecurityContext::derive(
        &ms,
        &[],
        &[0x01],
        &[0x00],
        None,
        AeadAlgorithm::AesCcm16_64_128,
    );
    let unprotected = hex("440171c30000b932396c6f63616c686f737483747631");
    let expected = hex("440271c30000b932396c6f63616c686f737463091400ff4ed339a5a379b0b8bc731fffb0");

    let inner = decode(&unprotected).unwrap();
    let outer = protect_request_message(&client, &[0x00], None, &[0x14], &inner).unwrap();
    assert_eq!(encode(&outer).unwrap(), expected, "C.5 protected request");

    let received = decode(&expected).unwrap();
    let recovered = unprotect_request_message(&server, &received).unwrap();
    assert_eq!(encode(&recovered).unwrap(), unprotected, "C.5 unprotect");
}

// C.6 — OSCORE request, client, keys of C.3 (ID Context carried as kid context).
#[test]
fn c6_protected_request_with_kid_context_is_byte_exact() {
    let ms = hex("0102030405060708090a0b0c0d0e0f10");
    let salt = hex("9e7ca92223786340");
    let idc = hex("37cbf3210017a2d3");
    let client = SecurityContext::derive(
        &ms,
        &salt,
        &[],
        &[0x01],
        Some(&idc),
        AeadAlgorithm::AesCcm16_64_128,
    );
    let server = SecurityContext::derive(
        &ms,
        &salt,
        &[0x01],
        &[],
        Some(&idc),
        AeadAlgorithm::AesCcm16_64_128,
    );
    let unprotected = hex("44012f8eef9bbf7a396c6f63616c686f737483747631");
    let expected = hex(
        "44022f8eef9bbf7a396c6f63616c686f73746b19140837cbf3210017a2d3ff72cd7273fd331ac45cffbe55c3",
    );

    let inner = decode(&unprotected).unwrap();
    let outer = protect_request_message(&client, &[], Some(&idc), &[0x14], &inner).unwrap();
    assert_eq!(encode(&outer).unwrap(), expected, "C.6 protected request");

    let received = decode(&expected).unwrap();
    let recovered = unprotect_request_message(&server, &received).unwrap();
    assert_eq!(encode(&recovered).unwrap(), unprotected, "C.6 unprotect");
}

// C.7 — OSCORE response, server, no Partial IV (request nonce reuse). Response
// to the C.4 request: request kid = empty, request Partial IV = 0x14.
#[test]
fn c7_protected_response_is_byte_exact() {
    let (client, server) = c1_contexts();
    let unprotected = hex("64455d1f00003974ff48656c6c6f20576f726c6421");
    let expected = hex("64445d1f0000397490ffdbaad1e9a7e7b2a813d3c31524378303cdafae119106");

    let inner = decode(&unprotected).unwrap();
    let outer = protect_response_message(&server, &[0x01], &[], &[0x14], None, &inner).unwrap();
    assert_eq!(encode(&outer).unwrap(), expected, "C.7 protected response");

    let received = decode(&expected).unwrap();
    let recovered = unprotect_response_message(&client, &[0x01], &[], &[0x14], &received).unwrap();
    assert_eq!(encode(&recovered).unwrap(), unprotected, "C.7 unprotect");
}

// C.8 — OSCORE response, server, with the server's own Partial IV 0x00.
#[test]
fn c8_protected_response_with_partial_iv_is_byte_exact() {
    let (client, server) = c1_contexts();
    let unprotected = hex("64455d1f00003974ff48656c6c6f20576f726c6421");
    let expected = hex("64445d1f00003974920100ff4d4c13669384b67354b2b6175ff4b8658c666a6cf88e");

    let inner = decode(&unprotected).unwrap();
    let outer =
        protect_response_message(&server, &[0x01], &[], &[0x14], Some(&[0x00]), &inner).unwrap();
    assert_eq!(encode(&outer).unwrap(), expected, "C.8 protected response");

    let received = decode(&expected).unwrap();
    let recovered = unprotect_response_message(&client, &[0x01], &[], &[0x14], &received).unwrap();
    assert_eq!(encode(&recovered).unwrap(), unprotected, "C.8 unprotect");
}
