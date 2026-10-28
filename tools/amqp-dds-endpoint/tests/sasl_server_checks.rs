#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! The endpoint's SASL checks on the wire (DDS-AMQP 1.0 §2.1 Cl. 6/7,
//! §10.2): a raw client talks to the server and reads what comes back.
//! The server used to answer every sasl-init with outcome `ok`.

mod common;

use std::io::Read;
use std::net::TcpStream;

use amqp_dds_endpoint::auth::{CredentialStore, SaslPolicy};
use amqp_dds_endpoint::frame_io::{
    AmqpProtocol, read_frame, read_protocol_header, write_frame, write_protocol_header,
};
use common::{TestServer, connect_client, test_handler_cfg_with_tls};
use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::FrameType;
use zerodds_amqp_bridge::performatives::{self, decode_performative, encode_performative};
use zerodds_amqp_endpoint::sasl::SaslMechanism;
use zerodds_amqp_endpoint::scram;

fn store() -> CredentialStore {
    CredentialStore::parse(&CredentialStore::entry_line(
        "alice",
        "wonder",
        b"0123456789abcdef",
        4096,
    ))
    .unwrap()
}

/// SASL header, then the offered mechanisms.
fn open_sasl(s: &mut TcpStream) -> Vec<String> {
    write_protocol_header(s, AmqpProtocol::Sasl).unwrap();
    assert_eq!(
        read_protocol_header(s).unwrap().protocol,
        AmqpProtocol::Sasl
    );
    let f = read_frame(s, 65_536).unwrap();
    let (d, body, _) = decode_performative(&f.body).unwrap();
    assert_eq!(d, 0x40, "sasl-mechanisms");
    performatives::sasl_mechanisms_from_body(&body)
}

fn send(s: &mut TcpStream, descriptor: u64, fields: Vec<AmqpExtValue>) {
    let body = encode_performative(descriptor, &AmqpExtValue::List(fields)).unwrap();
    write_frame(s, FrameType::Sasl, 0, &body).unwrap();
}

/// The next SASL performative: descriptor and fields.
fn recv(s: &mut TcpStream) -> (u64, Vec<AmqpExtValue>) {
    let f = read_frame(s, 65_536).unwrap();
    let (d, body, _) = decode_performative(&f.body).unwrap();
    match body {
        AmqpExtValue::List(items) => (d, items),
        other => (d, vec![other]),
    }
}

fn outcome_code(fields: &[AmqpExtValue]) -> u8 {
    match fields.first() {
        Some(AmqpExtValue::Ubyte(c)) => *c,
        other => panic!("sasl-outcome code: {other:?}"),
    }
}

fn closed(s: &mut TcpStream) -> bool {
    let mut b = [0u8; 1];
    matches!(s.read(&mut b), Ok(0) | Err(_))
}

fn plain_init(user: &str, pass: &str) -> Vec<AmqpExtValue> {
    vec![
        AmqpExtValue::Symbol("PLAIN".into()),
        AmqpExtValue::Binary(performatives::sasl_plain_response(user, pass)),
    ]
}

/// Cl. 7: PLAIN is not offered on plaintext, and a PLAIN init there gets
/// outcome `auth` and a closed connection.
#[test]
fn a_plain_init_on_plaintext_is_auth_and_the_connection_closes() {
    let mut cfg = test_handler_cfg_with_tls(false);
    cfg.sasl = SaslPolicy {
        enabled: vec![SaslMechanism::Plain, SaslMechanism::Anonymous],
        store: Some(store()),
    };
    let server = TestServer::spawn(cfg);
    let mut s = connect_client(server.port);
    let offered = open_sasl(&mut s);
    assert!(!offered.iter().any(|m| m == "PLAIN"), "offered {offered:?}");
    send(&mut s, 0x41, plain_init("alice", "wonder"));
    let (d, fields) = recv(&mut s);
    assert_eq!((d, outcome_code(&fields)), (0x44, 1));
    assert!(closed(&mut s));
    server.shutdown();
}

/// On TLS, PLAIN is checked against the store: the right password is `ok`,
/// a wrong one `auth`.
#[test]
fn plain_on_tls_checks_the_password_against_the_store() {
    for (password, code) in [("wonder", 0u8), ("wrong", 1)] {
        let mut cfg = test_handler_cfg_with_tls(true);
        cfg.sasl = SaslPolicy {
            enabled: vec![SaslMechanism::Plain],
            store: Some(store()),
        };
        let server = TestServer::spawn(cfg);
        let mut s = connect_client(server.port);
        assert_eq!(open_sasl(&mut s), ["PLAIN"]);
        send(&mut s, 0x41, plain_init("alice", password));
        let (d, fields) = recv(&mut s);
        assert_eq!((d, outcome_code(&fields)), (0x44, code), "{password}");
        server.shutdown();
    }
}

/// SCRAM-SHA-256 (RFC 7677) end to end: challenge, response, outcome `ok`
/// with the server signature as additional data, which the client checks.
#[test]
fn scram_sha_256_authenticates_and_proves_the_server() {
    let mut cfg = test_handler_cfg_with_tls(false);
    cfg.sasl = SaslPolicy {
        enabled: vec![SaslMechanism::ScramSha256],
        store: Some(store()),
    };
    let server = TestServer::spawn(cfg);
    let mut s = connect_client(server.port);
    assert_eq!(open_sasl(&mut s), ["SCRAM-SHA-256"]);
    let client_first_bare = "n=alice,r=clientnonce123";
    send(
        &mut s,
        0x41,
        vec![
            AmqpExtValue::Symbol("SCRAM-SHA-256".into()),
            AmqpExtValue::Binary(format!("n,,{client_first_bare}").into_bytes()),
        ],
    );
    let (d, fields) = recv(&mut s);
    assert_eq!(d, 0x42, "sasl-challenge");
    let server_first = match &fields[0] {
        AmqpExtValue::Binary(b) => String::from_utf8(b.clone()).unwrap(),
        other => panic!("{other:?}"),
    };
    let field = |k: &str| {
        server_first
            .split(',')
            .find_map(|p| p.strip_prefix(k))
            .unwrap()
            .to_string()
    };
    let nonce = field("r=");
    assert!(nonce.starts_with("clientnonce123") && nonce.len() > 14);
    let salt = scram::b64_decode(&field("s=")).unwrap();
    let iterations: u32 = field("i=").parse().unwrap();
    let without_proof = format!("c=biws,r={nonce}");
    let am = scram::auth_message(client_first_bare, &server_first, &without_proof);
    let proof = scram::client_proof("wonder", &salt, iterations, &am);
    send(
        &mut s,
        0x43,
        vec![AmqpExtValue::Binary(
            format!("{without_proof},p={}", scram::b64(&proof)).into_bytes(),
        )],
    );
    let (d, fields) = recv(&mut s);
    assert_eq!((d, outcome_code(&fields)), (0x44, 0));
    let expected = scram::ScramServerSecrets::from_password("wonder", &salt, iterations);
    match fields.get(1) {
        Some(AmqpExtValue::Binary(v)) => assert_eq!(
            String::from_utf8(v.clone()).unwrap(),
            format!("v={}", scram::b64(&expected.server_signature(&am)))
        ),
        other => panic!("additional-data: {other:?}"),
    }
    server.shutdown();
}

/// A mechanism that is not offered is `auth`.
#[test]
fn an_init_with_a_mechanism_not_offered_is_auth() {
    let mut cfg = test_handler_cfg_with_tls(false);
    cfg.sasl = SaslPolicy {
        enabled: vec![SaslMechanism::ScramSha256],
        store: Some(store()),
    };
    let server = TestServer::spawn(cfg);
    let mut s = connect_client(server.port);
    open_sasl(&mut s);
    send(&mut s, 0x41, vec![AmqpExtValue::Symbol("ANONYMOUS".into())]);
    let (d, fields) = recv(&mut s);
    assert_eq!((d, outcome_code(&fields)), (0x44, 1));
    server.shutdown();
}

/// AMQP 1.0 §5.3.1: without ANONYMOUS, a client that skips SASL gets the
/// SASL header back and a closed connection.
#[test]
fn skipping_sasl_is_refused_when_anonymous_is_not_enabled() {
    let mut cfg = test_handler_cfg_with_tls(false);
    cfg.sasl = SaslPolicy {
        enabled: vec![SaslMechanism::ScramSha256],
        store: Some(store()),
    };
    let server = TestServer::spawn(cfg);
    let mut s = connect_client(server.port);
    write_protocol_header(&mut s, AmqpProtocol::Amqp).unwrap();
    assert_eq!(
        read_protocol_header(&mut s).unwrap().protocol,
        AmqpProtocol::Sasl
    );
    assert!(closed(&mut s));
    server.shutdown();
}
