// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Productive TLS 1.2/1.3 e2e (feature `tls`): a real rustls handshake between
//! [`RustlsTlsServer`] and [`RustlsTlsClient`] over loopback TCP, then an XRCE
//! [`Message`] round-trip through the [`XrceTlsStream`] trait. Proves the
//! §3.1.10 TLS transport profile carries XRCE messages over an encrypted stream
//! (not the skeleton `XrceTlsClient` that returned a not-implemented error).

#![cfg(feature = "tls")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::thread;

use zerodds_xrce::header::{ClientKey, MessageHeader, SessionId, StreamId};
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::{CreateClientPayload, Message};
use zerodds_xrce::transport_tls::{RustlsTlsClient, RustlsTlsServer, XrceTlsStream};

#[test]
fn tls_handshake_and_message_roundtrip() {
    let server =
        RustlsTlsServer::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))).unwrap();
    let addr = server.local_addr().unwrap();
    let cert = server.cert_der().to_vec();

    // Server: accept one TLS connection, receive one XRCE message, echo it back.
    let srv = thread::spawn(move || {
        let mut s = server.accept().unwrap();
        let msg = s.recv_message().unwrap();
        s.send_message(&msg).unwrap();
        msg
    });

    let header = MessageHeader::with_client_key(
        SessionId(0),
        StreamId::BUILTIN_RELIABLE,
        SerialNumber16::new(1),
        ClientKey([0xCA, 0xFE, 0xBA, 0xBE]),
    )
    .unwrap();
    let sent = Message::new(
        header,
        vec![
            CreateClientPayload {
                representation: vec![b'X', b'R', b'C', b'E', 1, 0],
            }
            .into_submessage()
            .unwrap(),
        ],
    )
    .unwrap();

    // Client: dial, trusting the server's self-signed cert, send + receive echo.
    let mut client = RustlsTlsClient::connect(addr, "localhost", &cert).unwrap();
    client.send_message(&sent).unwrap();
    let echoed = client.recv_message().unwrap();
    client.close().ok();

    let server_saw = srv.join().unwrap();
    assert_eq!(
        server_saw, sent,
        "server decrypted the client's XRCE message"
    );
    assert_eq!(echoed, sent, "client decrypted the echoed message");
}
