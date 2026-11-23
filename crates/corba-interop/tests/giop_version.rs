// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! GIOP version honoring (Part 2 §9.4.1, §9.7): the ZeroDDS server
//! replies in the version of the incoming request, 1.0 to 1.4, so that a
//! GIOP 1.0/1.1 client gets a parseable reply; a higher minor version is
//! answered by a MessageError of version 1.4 and the connection closes;
//! the server's references carry IIOP 1.2 profiles unless it is set to
//! another version up to 1.4, and IIOP 1.4 profiles for an IPv6 host
//! (§7.9, §9.7.2).

use zerodds_corba_giop::{
    Message, Request, ResponseFlags, ServiceContextList, TargetAddress, Version,
};
use zerodds_corba_iiop::{Connector, ConnectorConfig, IiopProfileBody, IiopVersion};
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection};
use zerodds_corba_interop::string_argument;
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::{CorbaConnection, ObjectReference, SkeletonResult};
use zerodds_wire::Endianness;

/// Starts an Echo server (verbatim body echo for `ping`) and sends a
/// request in `req_version`; returns the version of the received reply.
fn roundtrip_reply_version(req_version: Version) -> Version {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.bytes().to_vec())
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

#[test]
fn server_replies_in_giop_1_3() {
    assert_eq!(roundtrip_reply_version(Version::V1_3), Version::V1_3);
}

#[test]
fn server_replies_in_giop_1_4() {
    assert_eq!(roundtrip_reply_version(Version::V1_4), Version::V1_4);
}

/// Part 2 §9.4.1: "A GIOP server, which receives a request having a
/// greater minor version number than it supports, should respond with an
/// error message having the highest minor version number that that server
/// supports, and then close the connection." The request is GIOP 1.5.
#[test]
fn corba_p2_9_4_1_higher_minor_gets_message_error_in_1_4_then_close() {
    use std::io::{Read, Write};
    use zerodds_corba_giop::{MessageType, encode_message};
    let acceptor = start_echo_server();
    let mut s = std::net::TcpStream::connect(acceptor.listen_addr()).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let request = Message::Request(Request {
        request_id: 1,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"Echo".to_vec()),
        operation: "ping".into(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body: encode_string_body("hi"),
    });
    let mut frame = encode_message(Version::V1_4, Endianness::Big, false, &request).unwrap();
    frame[5] = 5;
    s.write_all(&frame).unwrap();
    let answer = read_frame(&mut s);
    assert_eq!(&answer[0..4], b"GIOP");
    assert_eq!((answer[4], answer[5]), (1, 4), "MessageError version");
    assert_eq!(answer[7], MessageType::MessageError.as_u8());
    let mut rest = Vec::new();
    s.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty(), "the connection is closed");
    acceptor.shutdown();
}

/// Part 2 §9.7.2: a profile's minor version is the highest GIOP minor
/// version the server supports at publication; a ZeroDDS client sends
/// GIOP 1.4 to an IIOP 1.4 profile (it must support GIOP 1.4 as an IIOP
/// 1.4 client, §9.7). The first frame the client sends is read on a raw
/// socket.
#[test]
fn corba_p2_9_7_the_client_sends_giop_1_4_to_a_1_4_profile() {
    use std::io::Write;
    use zerodds_corba_giop::{Reply, ReplyStatusType, decode_message, encode_message};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let frame = read_frame(&mut s);
        let version = (frame[4], frame[5]);
        let (msg, _) = decode_message(&frame).unwrap();
        let Message::Request(req) = msg else {
            panic!("expected a Request, got {msg:?}");
        };
        let reply = Message::Reply(Reply {
            request_id: req.request_id,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: encode_string_body("pong"),
        });
        s.write_all(&encode_message(Version::V1_4, Endianness::Big, false, &reply).unwrap())
            .unwrap();
        version
    });
    let server = CorbaServer::new();
    server.set_profile_version(IiopVersion::V1_4).unwrap();
    let obj = server.object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Echo");
    let profile = IiopProfileBody::decode_encapsulation(&obj.iiop_profile).unwrap();
    assert_eq!(profile.iiop_version, IiopVersion::V1_4);
    let conn = IiopCorbaConnection::new();
    let (body, _) = conn
        .invoke(&obj, "ping", Endianness::Big, &string_argument("hi"))
        .unwrap();
    assert_eq!(decode_string_body(&body).unwrap(), "pong");
    assert_eq!(peer.join().unwrap(), (1, 4));
}

/// Part 2 §9.7.2: a profile may name any version the server accepts.
/// The references carry IIOP 1.2 unless the server is told another
/// version from 1.1 to 1.4 (omniORB 4.3 and JacORB 3.9 refuse a 1.4
/// profile, `omniorb_cross_orb.rs` and `jacorb_cross_orb.rs`); 1.0 has no
/// place for the code set component and is refused, as is 1.5.
#[test]
fn corba_p2_9_7_2_the_profile_version_is_1_2_unless_set() {
    use zerodds_corba_interop::runtime::{DEFAULT_PROFILE_VERSION, object_reference};
    let version_of = |obj: &ObjectReference| {
        IiopProfileBody::decode_encapsulation(&obj.iiop_profile)
            .unwrap()
            .iiop_version
    };
    assert_eq!(DEFAULT_PROFILE_VERSION, IiopVersion::V1_2);
    assert_eq!(
        version_of(&object_reference("IDL:X:1.0", "127.0.0.1", 1, b"k")),
        IiopVersion::V1_2
    );
    let server = CorbaServer::new();
    assert_eq!(
        version_of(&server.object_reference("IDL:X:1.0", "127.0.0.1", 1, b"k")),
        IiopVersion::V1_2
    );
    for v in [IiopVersion::V1_1, IiopVersion::V1_3, IiopVersion::V1_4] {
        server.set_profile_version(v).unwrap();
        assert_eq!(
            version_of(&server.object_reference("IDL:X:1.0", "127.0.0.1", 1, b"k")),
            v
        );
    }
    assert!(server.set_profile_version(IiopVersion::V1_0).is_err());
    assert!(server.set_profile_version(IiopVersion::new(1, 5)).is_err());
}

/// Table 7.2: `_component` from GIOP 1.3 (Part 2 §9.4.2.1). A facet whose
/// component reference the server knows answers with that reference, a
/// key without one with the nil reference (Part 3 §6.4.3.1).
#[test]
fn corba_p2_7_9_component_operation_answers_the_component_reference() {
    use zerodds_corba_interop::runtime::object_reference;
    use zerodds_wire::Reader;
    let server = CorbaServer::new();
    let echo = |op: &str, body: &zerodds_corba_giop::AlignedBody, e: Endianness| {
        zerodds_corba_rust::runtime::dispatch_object_operation(op, body, e, "IDL:Echo:1.0", &[])
            .unwrap_or(SkeletonResult::BadOperation)
    };
    server.register(b"Facet", echo);
    server.register(b"Plain", echo);
    let component = object_reference("IDL:demo/Trader:1.0", "127.0.0.1", 4711, b"Trader");
    server.set_component_reference(b"Facet", component.clone());
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect(&addr.ip().to_string(), addr.port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    let mut ask = |key: &[u8], id: u32| {
        let request = Message::Request(Request {
            request_id: id,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(key.to_vec()),
            operation: "_component".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: Vec::new(),
        });
        conn.write_message(Version::V1_3, Endianness::Little, false, &request)
            .unwrap();
        let (reply, e, v) = conn.read_message_full().unwrap();
        assert_eq!(v, Version::V1_3);
        let Message::Reply(reply) = reply else {
            panic!("expected a Reply");
        };
        let mut r = Reader::new(&reply.body, e);
        ObjectReference::decode(&mut r).unwrap()
    };
    let got = ask(b"Facet", 1);
    assert_eq!(got.type_id, component.type_id);
    assert_eq!(got.iiop_profiles(), component.iiop_profiles());
    let nil = ask(b"Plain", 2);
    assert!(nil.type_id.is_empty() && nil.iiop_profiles().is_empty());
    acceptor.shutdown();
}

/// Table 7.2 "IPV6 addresses in IOR" (GIOP 1.4): a server bound to the
/// IPv6 loopback publishes `::1` in an IIOP 1.4 profile, whatever its
/// default profile version, and given `::1` or `[::1]` writes the address
/// without brackets (the brackets belong to the URL form, §7.6.10.3); the
/// client reaches it through that host.
#[test]
fn corba_p2_7_9_ipv6_host_in_a_1_4_profile_is_dialed() {
    use zerodds_corba_interop::runtime::object_reference;
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.bytes().to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    let acceptor = server.serve("[::1]:0".parse().unwrap()).unwrap();
    let port = acceptor.listen_addr().port();
    for host in ["::1", "[::1]"] {
        let obj = object_reference("IDL:Echo:1.0", host, port, b"Echo");
        let profile = IiopProfileBody::decode_encapsulation(&obj.iiop_profile).unwrap();
        assert_eq!(profile.iiop_version, IiopVersion::V1_4);
        assert!(profile.iiop_version.allows_ipv6_host());
        assert_eq!(profile.host, "::1", "{host}");
        let conn = IiopCorbaConnection::new();
        let (body, _) = conn
            .invoke(&obj, "ping", Endianness::Big, &string_argument("v6"))
            .unwrap();
        assert_eq!(decode_string_body(&body).unwrap(), "v6");
    }
    acceptor.shutdown();
}

/// Echo server that returns the `ping` body verbatim.
fn start_echo_server() -> zerodds_corba_iiop::Acceptor {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.bytes().to_vec())
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
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    };

    let conn = IiopCorbaConnection::new();
    let (reply_body, reply_e) = conn
        .invoke(&obj, "ping", Endianness::Big, &string_argument("hi-1.0"))
        .unwrap();
    let _ = reply_e;
    assert_eq!(decode_string_body(&reply_body).unwrap(), "hi-1.0");
    acceptor.shutdown();
}

fn read_frame(s: &mut std::net::TcpStream) -> Vec<u8> {
    use std::io::Read;
    let mut header = [0u8; 12];
    s.read_exact(&mut header).unwrap();
    let size = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    let mut out = header.to_vec();
    let mut body = vec![0u8; size as usize];
    s.read_exact(&mut body).unwrap();
    out.extend_from_slice(&body);
    out
}

#[test]
fn corba_p2_9_4_the_server_takes_every_message_type_explicitly() {
    use std::io::{Read, Write};
    use zerodds_corba_giop::{
        CancelRequest, CloseConnection, LocateReply, LocateRequest, LocateStatusType, MessageType,
        Reply, ReplyStatusType, encode_message,
    };
    let acceptor = start_echo_server();
    let at = acceptor.listen_addr();
    let frame = |m: &Message| encode_message(Version::V1_2, Endianness::Big, false, m).unwrap();
    let connect = || {
        let s = std::net::TcpStream::connect(at).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        s
    };
    let locate = frame(&Message::LocateRequest(LocateRequest {
        request_id: 2,
        target: TargetAddress::Key(b"Echo".to_vec()),
    }));
    // §9.4.4: a CancelRequest has nothing left to cancel; the connection
    // stays open and the next message is answered.
    let mut s = connect();
    s.write_all(&frame(&Message::CancelRequest(CancelRequest {
        request_id: 1,
    })))
    .unwrap();
    s.write_all(&locate).unwrap();
    let (answer, _) = zerodds_corba_giop::decode_message(&read_frame(&mut s)).unwrap();
    assert!(
        matches!(
            answer,
            Message::LocateReply(LocateReply {
                request_id: 2,
                locate_status: LocateStatusType::ObjectHere,
                ..
            })
        ),
        "{answer:?}"
    );
    // §9.4.7: CloseConnection from the client ends the connection.
    s.write_all(&frame(&Message::CloseConnection(CloseConnection)))
        .unwrap();
    let mut rest = Vec::new();
    s.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
    // A Reply and a LocateReply are no messages for a server: MessageError,
    // then the connection ends (§9.4.8).
    for refused in [
        Message::Reply(Reply {
            request_id: 3,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: Vec::new(),
        }),
        Message::LocateReply(LocateReply {
            request_id: 4,
            locate_status: LocateStatusType::ObjectHere,
            body: Vec::new(),
        }),
    ] {
        let mut s = connect();
        s.write_all(&frame(&refused)).unwrap();
        let answer = read_frame(&mut s);
        assert_eq!(answer[7], MessageType::MessageError.as_u8(), "{refused:?}");
        let mut rest = Vec::new();
        s.read_to_end(&mut rest).unwrap();
        assert!(rest.is_empty(), "{refused:?}");
    }
}
