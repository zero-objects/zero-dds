// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `amqp-dds-endpoint` with `zerodds-amqp-endpoint`
//! (internal/release/rc8-test-suite.md, "Crate tier"): the SASL mechanism
//! names, the SCRAM-SHA-256 server side, the management addresses and the
//! connection state machine the endpoint program drives, each against its
//! specification.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_amqp_endpoint::management::{AddressKind, addresses, classify_address};
use zerodds_amqp_endpoint::sasl::SaslMechanism;
use zerodds_amqp_endpoint::scram::{ScramServerSecrets, auth_message, b64, b64_decode, sha256};
use zerodds_amqp_endpoint::session::InboundFrameKind;
use zerodds_amqp_endpoint::{ConnectionState, advance_connection};

/// The names of the SASL registry the program offers: RFC 4616, RFC 4505,
/// RFC 4422 Appendix A, RFC 7677.
#[test]
fn amqp10_5_3_3_1_the_mechanism_names_are_the_sasl_registrys() {
    for (m, name) in [
        (SaslMechanism::Plain, "PLAIN"),
        (SaslMechanism::Anonymous, "ANONYMOUS"),
        (SaslMechanism::External, "EXTERNAL"),
        (SaslMechanism::ScramSha256, "SCRAM-SHA-256"),
    ] {
        assert_eq!(m.name(), name);
        assert_eq!(SaslMechanism::from_name(name), Some(m));
    }
    assert_eq!(SaslMechanism::from_name("plain"), None);
}

/// RFC 7677 §3, the example exchange: user `user`, password `pencil`, the
/// salt and 4096 iterations of the server-first message. The server's
/// secrets verify the client's proof and give the server signature `v=`.
#[test]
fn ddsamqp_10_2_scram_sha_256_answers_the_exchange_of_rfc_7677() {
    let salt = b64_decode("W22ZaJ0SNY7soEsUEjb6gQ==").unwrap();
    let secrets = ScramServerSecrets::from_password("pencil", &salt, 4096);
    let nonce = "rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0";
    let message = auth_message(
        "n=user,r=rOprNGfwEbeRWgbNEkqO",
        &format!("r={nonce},s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096"),
        &format!("c=biws,r={nonce}"),
    );
    let proof: [u8; 32] = b64_decode("dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ=")
        .unwrap()
        .try_into()
        .unwrap();
    assert!(secrets.verify_client_proof(&message, &proof));
    assert_eq!(
        b64(&secrets.server_signature(&message)),
        "6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="
    );
    let mut wrong = proof;
    wrong[0] ^= 1;
    assert!(!secrets.verify_client_proof(&message, &wrong));
}

/// FIPS 180-4 (example "abc" of the NIST examples): the digest the program
/// writes into its credential store.
#[test]
fn ddsamqp_10_2_sha_256_of_abc_is_the_nist_example() {
    let hex: String = sha256(b"abc").iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

/// DDS-AMQP 1.0 §7.9: the management addresses `$catalog`, `$metrics`,
/// `$audit`; any other address names a topic.
#[test]
fn ddsamqp_7_9_the_management_addresses_are_catalog_metrics_audit() {
    assert_eq!(addresses::CATALOG, "$catalog");
    assert_eq!(addresses::METRICS, "$metrics");
    assert_eq!(addresses::AUDIT, "$audit");
    assert_eq!(classify_address("$catalog"), AddressKind::Catalog);
    assert_eq!(classify_address("$metrics"), AddressKind::Metrics);
    assert_eq!(classify_address("$audit"), AddressKind::Audit);
    assert_eq!(classify_address("/queues/q"), AddressKind::Topic);
}

/// OASIS AMQP 1.0 Part 2 §2.4.6: START, HDR-RCVD, HDR-EXCH, OPEN-RCVD,
/// OPENED, CLOSE-RCVD, END for the frames the program receives; a frame
/// out of order is an error.
#[test]
fn amqp10_2_4_6_the_connection_states_follow_the_received_frames() {
    let mut s = ConnectionState::Start;
    for (frame, next) in [
        (InboundFrameKind::Header, ConnectionState::HdrRcvd),
        (InboundFrameKind::Header, ConnectionState::HdrExch),
        (InboundFrameKind::Open, ConnectionState::OpenRcvd),
        (InboundFrameKind::Open, ConnectionState::Opened),
        (InboundFrameKind::Begin, ConnectionState::Opened),
        (InboundFrameKind::Close, ConnectionState::CloseRcvd),
    ] {
        s = advance_connection(s, frame).unwrap();
        assert_eq!(s, next, "after {frame:?}");
    }
    assert!(advance_connection(ConnectionState::Start, InboundFrameKind::Open).is_err());
}
