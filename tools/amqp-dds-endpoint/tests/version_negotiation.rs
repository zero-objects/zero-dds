// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OASIS AMQP 1.0 Part 2 §2.2 version negotiation, the server side: a
//! protocol header the endpoint does not support or cannot parse is
//! answered with a header it supports, then the socket is closed (Figure
//! 2.12, cases 2 and 3).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

mod common;

use std::io::{Read, Write};

use common::{TestServer, connect_client, test_handler_cfg};

/// Sends `octets`, returns everything the server wrote until it closed the
/// socket.
fn answer_to(port: u16, octets: &[u8]) -> Vec<u8> {
    let mut client = connect_client(port);
    client.write_all(octets).expect("write");
    let mut answer = Vec::new();
    // `read_to_end` returns at the end of the stream: the server closed.
    client
        .read_to_end(&mut answer)
        .expect("the server closes the socket after its header");
    answer
}

/// §2.2: "If the requested protocol version is not supported, the server
/// MUST send a protocol header with a supported protocol version and then
/// close the socket." 1.1.0, 2.0.0 and revision 1 are answered with 1.0.0.
#[test]
fn amqp10_2_2_an_unsupported_version_is_answered_with_the_supported_header() {
    let server = TestServer::spawn(test_handler_cfg());
    for requested in [
        b"AMQP\x00\x01\x01\x00",
        b"AMQP\x00\x02\x00\x00",
        b"AMQP\x00\x01\x00\x01",
        b"AMQP\x00\x00\x09\x01",
    ] {
        assert_eq!(
            answer_to(server.port, requested),
            b"AMQP\x00\x01\x00\x00",
            "{requested:?}"
        );
    }
    // A protocol id the endpoint does not speak (here 9).
    assert_eq!(
        answer_to(server.port, b"AMQP\x09\x01\x00\x00"),
        b"AMQP\x00\x01\x00\x00"
    );
    server.shutdown();
}

/// §2.2: "If the server cannot parse the protocol header, the server MUST
/// send a valid protocol header with a supported protocol version and then
/// close the socket." (Figure 2.12, case 3: HTTP.)
#[test]
fn amqp10_2_2_octets_that_are_no_header_are_answered_with_the_header() {
    let server = TestServer::spawn(test_handler_cfg());
    assert_eq!(
        answer_to(server.port, b"GET / HTTP/1.1\r\n\r\n"),
        b"AMQP\x00\x01\x00\x00"
    );
    assert_eq!(
        answer_to(server.port, b"amqp\x00\x01\x00\x00"),
        b"AMQP\x00\x01\x00\x00"
    );
    server.shutdown();
}

/// §2.2 with §5.3.1: an endpoint that requires the SASL layer supports the
/// SASL header first, and answers with it.
#[test]
fn amqp10_2_2_an_endpoint_that_requires_sasl_answers_with_the_sasl_header() {
    let mut cfg = test_handler_cfg();
    // Without ANONYMOUS the SASL layer cannot be skipped.
    cfg.sasl.enabled = vec![zerodds_amqp_endpoint::sasl::SaslMechanism::External];
    let server = TestServer::spawn(cfg);
    assert_eq!(
        answer_to(server.port, b"AMQP\x03\x02\x00\x00"),
        b"AMQP\x03\x01\x00\x00"
    );
    server.shutdown();
}

/// §2.2, the header after the SASL layer: an unsupported one is answered
/// with the AMQP header.
#[test]
fn amqp10_2_2_an_unsupported_header_after_sasl_is_answered_with_the_amqp_header() {
    use zerodds_amqp_bridge::extended_types::AmqpExtValue;
    use zerodds_amqp_bridge::frame::FrameType;
    use zerodds_amqp_bridge::performatives;

    let server = TestServer::spawn(test_handler_cfg());
    let mut client = connect_client(server.port);
    client.write_all(b"AMQP\x03\x01\x00\x00").expect("write");
    let mut header = [0u8; 8];
    client.read_exact(&mut header).expect("the SASL header");
    assert_eq!(&header, b"AMQP\x03\x01\x00\x00");
    let mechanisms = amqp_dds_endpoint::read_frame(&mut client, 65_536).expect("mechanisms");
    assert_eq!(mechanisms.header.frame_type, FrameType::Sasl);
    let init = performatives::encode_performative(
        0x41,
        &AmqpExtValue::List(vec![AmqpExtValue::Symbol("ANONYMOUS".into())]),
    )
    .expect("sasl-init");
    amqp_dds_endpoint::write_frame(&mut client, FrameType::Sasl, 0, &init).expect("write");
    let outcome = amqp_dds_endpoint::read_frame(&mut client, 65_536).expect("outcome");
    let (descriptor, body, _) = performatives::decode_performative(&outcome.body).expect("decode");
    assert_eq!(descriptor, 0x44);
    assert_eq!(performatives::sasl_outcome_code(&body), Some(0));

    client.write_all(b"AMQP\x00\x01\x01\x00").expect("write");
    let mut answer = Vec::new();
    client.read_to_end(&mut answer).expect("closed");
    assert_eq!(answer, b"AMQP\x00\x01\x00\x00");
    server.shutdown();
}
