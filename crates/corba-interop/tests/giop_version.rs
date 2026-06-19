// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! GIOP version honoring (§15.4.1): the ZeroDDS server replies in the
//! version of the incoming request (capped at the max supported 1.2),
//! not necessarily 1.2 — so that GIOP 1.0/1.1 clients get a parseable
//! reply.

use zerodds_cdr::Endianness;
use zerodds_corba_giop::{
    Message, Request, ResponseFlags, ServiceContextList, TargetAddress, Version,
};
use zerodds_corba_iiop::{Connector, ConnectorConfig, IiopProfileBody, IiopVersion};
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::{CorbaConnection, ObjectReference, SkeletonResult};

/// Starts an Echo server (verbatim body echo for `ping`) and sends a
/// request in `req_version`; returns the version of the received reply.
fn roundtrip_reply_version(req_version: Version) -> Version {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect(&addr.ip().to_string(), addr.port())
        .unwrap();
    let conn = pooled.connection().unwrap();

    let request = Message::Request(Request {
        request_id: 1,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"Echo".to_vec()),
        operation: "ping".into(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body: encode_string_body("hi"),
    });
    conn.write_message(req_version, Endianness::Big, false, &request)
        .unwrap();
    let (reply, _e, reply_version) = conn.read_message_full().unwrap();
    assert!(matches!(reply, Message::Reply(_)), "expected a Reply");
    acceptor.shutdown();
    reply_version
}

#[test]
fn server_replies_in_giop_1_0() {
    assert_eq!(roundtrip_reply_version(Version::V1_0), Version::V1_0);
}

#[test]
fn server_replies_in_giop_1_1() {
    assert_eq!(roundtrip_reply_version(Version::V1_1), Version::V1_1);
}

#[test]
fn server_replies_in_giop_1_2() {
    assert_eq!(roundtrip_reply_version(Version::V1_2), Version::V1_2);
}

/// Echo server that returns the `ping` body verbatim.
fn start_echo_server() -> zerodds_corba_iiop::Acceptor {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    server.serve("127.0.0.1:0".parse().unwrap()).unwrap()
}

/// Client path: when the target IOR carries a **GIOP 1.0 profile** (no component
/// block, 1.0 request layout), the stub connection `send` derives the
/// request version from it and the full echo roundtrip works over the
/// 1.0 wire encoding (request 1.0 → reply 1.0).
#[test]
fn client_speaks_giop_1_0_from_ior_profile() {
    let acceptor = start_echo_server();
    let addr = acceptor.listen_addr();

    // IOR with an IIOP 1.0 ProfileBody (1.0 has no TaggedComponents).
    let body = IiopProfileBody::new(
        IiopVersion::V1_0,
        addr.ip().to_string(),
        addr.port(),
        b"Echo".to_vec(),
    );
    let iiop_profile = body.encode_encapsulation(Endianness::Big).unwrap();
    let obj = ObjectReference {
        type_id: "IDL:Echo:1.0".into(),
        iiop_profile,
    };

    let conn = IiopCorbaConnection::new();
    let (reply_body, reply_e) = conn
        .invoke(&obj, "ping", Endianness::Big, &encode_string_body("hi-1.0"))
        .unwrap();
    let _ = reply_e;
    assert_eq!(decode_string_body(&reply_body), "hi-1.0");
    acceptor.shutdown();
}
