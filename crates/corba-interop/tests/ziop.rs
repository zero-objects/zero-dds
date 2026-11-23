// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 2 §12 ZIOP through the ORB: the server declares its
//! ZIOP policies in the `TAG_POLICIES` component of its references, the
//! client sends its client-exposed policies in `INVOCATION_POLICIES`, and
//! a request or reply goes out as a ZIOP message only when both sides
//! enable compression with a compressor in common and the sender's low
//! value and minimum ratio allow it. A relay between client and server
//! records the magic of every message on the wire.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use zerodds_corba_rust::{marshaled_body, no_arguments};

use zerodds_corba_ccm::compression::CompressionManager;
use zerodds_corba_giop::messaging::{INVOCATION_POLICIES, TAG_POLICIES, decode_policy_values};
use zerodds_corba_giop::ziop::{
    COMPRESSORID_ZLIB, CompressorIdLevel, PeerZiop, ZiopPolicies, ZiopSend, compress_message,
};
use zerodds_corba_giop::{
    Message, MessageType, Request, ResponseFlags, ServiceContext, ServiceContextList,
    TargetAddress, Version, encode_message,
};
use zerodds_corba_iiop::{Acceptor, IiopProfileBody};
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_rust::{
    CompletionStatus, CorbaConnection, CorbaException, ObjectReference, SkeletonResult,
    SystemExceptionKind,
};
use zerodds_wire::Endianness;

#[path = "support/relay.rs"]
mod relay;
use relay::{magics, relay};

fn zlib(level: u16) -> CompressorIdLevel {
    CompressorIdLevel {
        compressor_id: COMPRESSORID_ZLIB,
        compression_level: level,
    }
}

fn enabled() -> ZiopPolicies {
    ZiopPolicies {
        low_value: 0,
        ..ZiopPolicies::enabled(vec![zlib(6)])
    }
}

fn manager() -> Arc<CompressionManager> {
    Arc::new(CompressionManager::with_default_factories())
}

/// A server with an echo object `key`; ZIOP when `ziop` is given. The
/// observer records the service context ids of each request.
fn echo_server(ziop: Option<ZiopPolicies>) -> (CorbaServer, Acceptor, Arc<Mutex<Vec<Vec<u32>>>>) {
    let server = CorbaServer::new();
    server.register(b"Echo", |_op, body, _e| {
        SkeletonResult::Reply(body.bytes().to_vec())
    });
    let contexts: Arc<Mutex<Vec<Vec<u32>>>> = Arc::default();
    let c = Arc::clone(&contexts);
    server.on_request_contexts(move |list| {
        c.lock()
            .unwrap()
            .push(list.0.iter().map(|sc| sc.context_id).collect());
    });
    if let Some(p) = ziop {
        server.set_ziop(p, manager());
    }
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    (server, acceptor, contexts)
}

/// A compressible body of `n` octets (CDR octet data for the echo).
fn body(n: usize) -> Vec<u8> {
    b"ZeroDDS ZIOP ".iter().copied().cycle().take(n).collect()
}

#[test]
fn corba_p2_12_2_a_request_and_its_reply_travel_as_ziop_messages() {
    let (server, _acceptor, contexts) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", _acceptor.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    // The reference declares ZIOP (§12.4).
    let profile = IiopProfileBody::decode_encapsulation(&reference.iiop_profile).unwrap();
    let tag = profile
        .components
        .iter()
        .find(|c| c.tag == TAG_POLICIES)
        .expect("TAG_POLICIES");
    let declared =
        PeerZiop::from_policy_values(&decode_policy_values(&tag.component_data).unwrap()).unwrap();
    assert!(declared.compression_enabled);
    assert_eq!(declared.compressor_ids, vec![zlib(6)]);

    let client = IiopCorbaConnection::new().with_ziop(enabled(), manager());
    for endianness in [Endianness::Big, Endianness::Little] {
        let payload = body(8000);
        let (reply, _) = client
            .invoke(&reference, "echo", endianness, &marshaled_body(&payload, 0))
            .unwrap();
        assert_eq!(reply, payload);
    }
    assert_eq!(magics(&r.to_server), vec![*b"ZIOP"; 2]);
    assert_eq!(magics(&r.to_client), vec![*b"ZIOP"; 2]);
    // The decompressed requests carried the client's policies (§12.2).
    assert!(
        contexts
            .lock()
            .unwrap()
            .iter()
            .all(|ids| ids.contains(&INVOCATION_POLICIES))
    );
}

#[test]
fn corba_p2_12_2_no_ziop_to_a_peer_without_the_policies() {
    // A server without ZIOP: plain reference, the client sends GIOP and
    // still tells the server its policies.
    let (_server, plain, contexts) = echo_server(None);
    let r = relay("127.0.0.1", plain.listen_addr().port());
    let reference = object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let client = IiopCorbaConnection::new().with_ziop(enabled(), manager());
    let payload = body(8000);
    assert_eq!(
        client
            .invoke(
                &reference,
                "echo",
                Endianness::Big,
                &marshaled_body(&payload, 0)
            )
            .unwrap()
            .0,
        payload
    );
    assert_eq!(magics(&r.to_server), vec![*b"GIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"GIOP"]);
    assert!(contexts.lock().unwrap()[0].contains(&INVOCATION_POLICIES));

    // A ZIOP server and a client without ZIOP policies: GIOP both ways.
    let (server, z, _) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", z.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let plain_client = IiopCorbaConnection::new();
    assert_eq!(
        plain_client
            .invoke(
                &reference,
                "echo",
                Endianness::Big,
                &marshaled_body(&payload, 0)
            )
            .unwrap()
            .0,
        payload
    );
    assert_eq!(magics(&r.to_server), vec![*b"GIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"GIOP"]);

    // A server that lists only gzip: no compressor in common with a zlib
    // client, GIOP both ways.
    let gzip_only = ZiopPolicies {
        low_value: 0,
        ..ZiopPolicies::enabled(vec![CompressorIdLevel {
            compressor_id: zerodds_corba_ccm::compression::COMPRESSORID_GZIP,
            compression_level: 6,
        }])
    };
    let (server, g, _) = echo_server(Some(gzip_only));
    let r = relay("127.0.0.1", g.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    assert_eq!(
        client
            .invoke(
                &reference,
                "echo",
                Endianness::Big,
                &marshaled_body(&payload, 0)
            )
            .unwrap()
            .0,
        payload
    );
    assert_eq!(magics(&r.to_server), vec![*b"GIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"GIOP"]);

    // A client that disabled compression declares it disabled and gets
    // GIOP replies from the ZIOP server.
    let (server, z, _) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", z.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let off = IiopCorbaConnection::new().with_ziop(
        ZiopPolicies {
            compression_enabled: false,
            ..enabled()
        },
        manager(),
    );
    off.invoke(
        &reference,
        "echo",
        Endianness::Big,
        &marshaled_body(&payload, 0),
    )
    .unwrap();
    assert_eq!(magics(&r.to_server), vec![*b"GIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"GIOP"]);
}

#[test]
fn corba_p2_12_3_low_value_and_min_ratio_keep_a_message_uncompressed() {
    // Server: low value above the reply body. Client: compresses.
    let (server, a, _) = echo_server(Some(ZiopPolicies {
        low_value: 100_000,
        ..enabled()
    }));
    let r = relay("127.0.0.1", a.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let client = IiopCorbaConnection::new().with_ziop(enabled(), manager());
    let payload = body(8000);
    assert_eq!(
        client
            .invoke(
                &reference,
                "echo",
                Endianness::Big,
                &marshaled_body(&payload, 0)
            )
            .unwrap()
            .0,
        payload
    );
    assert_eq!(magics(&r.to_server), vec![*b"ZIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"GIOP"]);

    // Client: a minimum ratio no zlib stream of this body reaches; the
    // server compresses its reply.
    let (server, b, _) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", b.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let strict = IiopCorbaConnection::new().with_ziop(
        ZiopPolicies {
            min_ratio: Some(0.0001),
            ..enabled()
        },
        manager(),
    );
    assert_eq!(
        strict
            .invoke(
                &reference,
                "echo",
                Endianness::Big,
                &marshaled_body(&payload, 0)
            )
            .unwrap()
            .0,
        payload
    );
    assert_eq!(magics(&r.to_server), vec![*b"GIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"ZIOP"]);

    // The ORB default low value (1024 octets) keeps a short request GIOP.
    let (server, c, _) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", c.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let defaults =
        IiopCorbaConnection::new().with_ziop(ZiopPolicies::enabled(vec![zlib(6)]), manager());
    let short = body(300);
    assert_eq!(
        defaults
            .invoke(
                &reference,
                "echo",
                Endianness::Big,
                &marshaled_body(&short, 0)
            )
            .unwrap()
            .0,
        short
    );
    assert_eq!(magics(&r.to_server), vec![*b"GIOP"]);
}

#[test]
fn corba_p2_12_1_a_oneway_request_is_compressed_too() {
    let (server, a, contexts) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", a.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let client = IiopCorbaConnection::new().with_ziop(enabled(), manager());
    client
        .invoke_oneway(
            &reference,
            "notify",
            Endianness::Little,
            &marshaled_body(&body(5000), 0),
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while contexts.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(contexts.lock().unwrap().len(), 1);
    assert_eq!(
        r.to_server.lock().unwrap().as_slice(),
        &[(*b"ZIOP", MessageType::Request.as_u8())]
    );
}

fn raw_request(service_context: Vec<ServiceContext>, payload: Vec<u8>) -> Vec<u8> {
    encode_message(
        Version::V1_2,
        Endianness::Big,
        false,
        &Message::Request(Request {
            request_id: 5,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(b"Echo".to_vec()),
            operation: "echo".into(),
            requesting_principal: None,
            service_context: ServiceContextList(service_context),
            body: payload,
        }),
    )
    .unwrap()
}

/// [`raw_request`] in GIOP `version`, addressed by key as GIOP 1.0
/// and 1.1 require.
fn raw_request_in(version: Version, service_context: Vec<ServiceContext>) -> Vec<u8> {
    encode_message(
        version,
        Endianness::Big,
        false,
        &Message::Request(Request {
            request_id: 9,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(b"Echo".to_vec()),
            operation: "echo".into(),
            requesting_principal: None,
            service_context: ServiceContextList(service_context),
            body: body(16),
        }),
    )
    .unwrap()
}

fn read_frame(s: &mut TcpStream) -> Vec<u8> {
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
fn corba_p2_12_2_a_ziop_request_to_a_server_without_ziop_is_a_message_error() {
    let (_server, a, contexts) = echo_server(None);
    let frame = raw_request(Vec::new(), body(4000));
    let send = ZiopSend {
        compressor: zlib(6),
        low_value: 0,
        min_ratio: None,
    };
    let z = compress_message(&frame, &send, manager().as_ref())
        .unwrap()
        .unwrap();
    let mut s = TcpStream::connect(a.listen_addr()).unwrap();
    s.write_all(&z).unwrap();
    let answer = read_frame(&mut s);
    assert_eq!(&answer[..4], b"GIOP");
    assert_eq!(answer[7], MessageType::MessageError.as_u8());
    // The connection is closed after the MessageError.
    let mut rest = Vec::new();
    s.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
    assert!(contexts.lock().unwrap().is_empty());
}

#[test]
fn corba_p2_12_2_invocation_policies_that_do_not_decode_are_marshal() {
    let (_server, a, contexts) = echo_server(Some(enabled()));
    let broken = ServiceContext::new(INVOCATION_POLICIES, vec![0, 0, 0, 0, 0, 0, 0, 9]);
    let mut s = TcpStream::connect(a.listen_addr()).unwrap();
    s.write_all(&raw_request(vec![broken], body(16))).unwrap();
    let answer = read_frame(&mut s);
    let (msg, _) = zerodds_corba_giop::decode_message(&answer).unwrap();
    let Message::Reply(reply) = msg else {
        panic!("{msg:?}")
    };
    assert_eq!(
        reply.reply_status,
        zerodds_corba_giop::ReplyStatusType::SystemException
    );
    let mut r = zerodds_wire::Reader::new(&reply.body, Endianness::Big);
    assert_eq!(r.read_string().unwrap(), "IDL:omg.org/CORBA/MARSHAL:1.0");
    assert!(contexts.lock().unwrap().is_empty());
}

#[test]
fn corba_p2_12_2_a_broken_tag_policies_component_is_inv_objref() {
    let mut profile = IiopProfileBody::decode_encapsulation(
        &object_reference("IDL:Echo:1.0", "127.0.0.1", 9, b"Echo").iiop_profile,
    )
    .unwrap();
    profile
        .components
        .push(zerodds_corba_iiop::TaggedComponent {
            tag: TAG_POLICIES,
            component_data: vec![0, 0, 0, 0, 0, 0, 0, 7],
        });
    let reference = ObjectReference {
        type_id: "IDL:Echo:1.0".into(),
        iiop_profile: profile.encode_encapsulation(Endianness::Big).unwrap(),
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    };
    let client = IiopCorbaConnection::new().with_ziop(enabled(), manager());
    match client.invoke(
        &reference,
        "echo",
        Endianness::Big,
        &marshaled_body(&body(10), 0),
    ) {
        Err(CorbaException::System(e)) => {
            assert_eq!(e.kind, SystemExceptionKind::InvObjref);
            assert_eq!(e.completed, CompletionStatus::No);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn corba_p2_12_2_the_client_policies_hold_for_the_connection() {
    // A client that sends its ZIOP policies in the first request of a
    // connection only, as omniORB does: both replies are ZIOP messages.
    let (_server, a, _) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", a.listen_addr().port());
    let stream = TcpStream::connect(("127.0.0.1", r.port)).unwrap();
    let mut conn = zerodds_corba_iiop::Connection::from_stream(stream).unwrap();
    conn.set_ziop_codec(Some(manager()));
    let policies = zerodds_corba_giop::messaging::invocation_policies_context(
        &enabled().client_exposed(Endianness::Big).unwrap(),
        Endianness::Big,
    )
    .unwrap();
    for contexts in [vec![policies], Vec::new()] {
        let request = Message::Request(Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(b"Echo".to_vec()),
            operation: "echo".into(),
            requesting_principal: None,
            service_context: ServiceContextList(contexts),
            body: body(4000),
        });
        conn.write_message(Version::V1_2, Endianness::Big, false, &request)
            .unwrap();
        let Message::Reply(reply) = conn.read_message().unwrap() else {
            panic!("no reply")
        };
        assert_eq!(reply.body, body(4000));
    }
    assert_eq!(magics(&r.to_client), vec![*b"ZIOP"; 2]);
}

/// CORBA 3.3 Part 2 §7.9, Table 7.2: `INVOCATION_POLICIES` (Asynch
/// Messaging) is part of GIOP from 1.2 on. A client with ZIOP policies
/// sends it to an IIOP 1.2 profile and not to an IIOP 1.0 or 1.1
/// profile of the same object.
#[test]
fn corba_p2_7_9_the_client_sends_invocation_policies_from_giop_1_2_only() {
    use zerodds_corba_iiop::IiopVersion;
    let (_server, a, contexts) = echo_server(Some(enabled()));
    let client = IiopCorbaConnection::new().with_ziop(enabled(), manager());
    let versions = [IiopVersion::V1_0, IiopVersion::V1_1, IiopVersion::V1_2];
    for version in versions {
        let profile = IiopProfileBody::new(
            version,
            "127.0.0.1".into(),
            a.listen_addr().port(),
            b"Echo".to_vec(),
        );
        let reference = ObjectReference {
            type_id: "IDL:Echo:1.0".into(),
            iiop_profile: profile.encode_encapsulation(Endianness::Big).unwrap(),
            policy_overrides: Vec::new(),
            profiles: Vec::new(),
        };
        let (reply, _) = client
            .invoke(&reference, "echo", Endianness::Big, &|w| {
                w.write_bytes(&body(16));
                Ok(())
            })
            .unwrap();
        assert_eq!(reply, body(16), "{version:?}");
    }
    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), versions.len());
    assert!(!contexts[0].contains(&INVOCATION_POLICIES), "GIOP 1.0");
    assert!(!contexts[1].contains(&INVOCATION_POLICIES), "GIOP 1.1");
    assert!(contexts[2].contains(&INVOCATION_POLICIES), "GIOP 1.2");
}

/// CORBA 3.3 Part 2 §7.9, Table 7.2: a ZIOP server answers a GIOP 1.0 or
/// 1.1 request in its version without `INVOCATION_POLICIES`, and a GIOP
/// 1.2 request with it.
#[test]
fn corba_p2_7_9_the_server_sends_invocation_policies_in_giop_1_2_replies_only() {
    let (_server, a, _) = echo_server(Some(enabled()));
    for (version, expected) in [
        (Version::V1_0, false),
        (Version::V1_1, false),
        (Version::V1_2, true),
    ] {
        let mut s = TcpStream::connect(a.listen_addr()).unwrap();
        s.write_all(&raw_request_in(version, Vec::new())).unwrap();
        let answer = read_frame(&mut s);
        assert_eq!(&answer[..4], b"GIOP", "{version:?}");
        assert_eq!((answer[4], answer[5]), (version.major, version.minor));
        let (msg, _) = zerodds_corba_giop::decode_message(&answer).unwrap();
        let Message::Reply(reply) = msg else {
            panic!("{msg:?}")
        };
        assert_eq!(
            reply.reply_status,
            zerodds_corba_giop::ReplyStatusType::NoException
        );
        assert_eq!(reply.body, body(16));
        let has = reply
            .service_context
            .0
            .iter()
            .any(|c| c.context_id == INVOCATION_POLICIES);
        assert_eq!(has, expected, "{version:?}");
    }
}

// -- Part 2 §12.2: client ZIOP policies at ORB, thread and reference level --

use zerodds_corba_ccm::orb_core::{ObjectRef, Orb};
use zerodds_corba_ccm::policy_management::SetOverrideType;
use zerodds_corba_ccm::portable_interceptor::Policy;
use zerodds_corba_rt::{AnyValue, CorbaAny, TypeCode};

/// The ZIOP policies through `CORBA::ORB::create_policy` (Part 2 §12.3):
/// compression on or off, zlib at level 6, a low value of 0.
fn created_policies(orb: &Orb, on: bool) -> Vec<Policy> {
    const PAIR: &str = "IDL:omg.org/Compression/CompressorIdLevel:1.0";
    let element = TypeCode::Struct {
        repo_id: PAIR.into(),
        name: "CompressorIdLevel".into(),
        members: vec![
            ("compressor_id".into(), TypeCode::UShort),
            ("compression_level".into(), TypeCode::UShort),
        ],
        is_except: false,
    };
    let zlib6 = AnyValue::Struct {
        repo_id: PAIR.into(),
        name: "CompressorIdLevel".into(),
        members: vec![
            ("compressor_id".into(), AnyValue::UShort(COMPRESSORID_ZLIB)),
            ("compression_level".into(), AnyValue::UShort(6)),
        ],
    };
    vec![
        orb.create_policy(64, &CorbaAny(AnyValue::Boolean(on)))
            .unwrap(),
        orb.create_policy(
            65,
            &CorbaAny(AnyValue::Seq {
                element,
                items: vec![zlib6],
            }),
        )
        .unwrap(),
        orb.create_policy(66, &CorbaAny(AnyValue::ULong(0)))
            .unwrap(),
    ]
}

fn new_orb(name: &str) -> Arc<Orb> {
    zerodds_corba_interop::runtime::orb_init(&mut Vec::new(), name).unwrap()
}

/// A ZIOP echo server behind a relay; its reference points at the relay.
fn ziop_target() -> (CorbaServer, Acceptor, relay::Relay, ObjectReference) {
    let (server, acceptor, _) = echo_server(Some(enabled()));
    let r = relay("127.0.0.1", acceptor.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    (server, acceptor, r, reference)
}

fn echo(client: &IiopCorbaConnection, reference: &ObjectReference) {
    let payload = body(8000);
    assert_eq!(
        client
            .invoke(
                reference,
                "echo",
                Endianness::Big,
                &marshaled_body(&payload, 0)
            )
            .unwrap()
            .0,
        payload
    );
}

#[test]
fn corba_p2_12_2_client_ziop_policies_at_the_orb_level_through_create_policy() {
    // The client's policies only through create_policy and the ORB's
    // PolicyManager: no with_ziop.
    let orb = new_orb("ziop-orb-level-client");
    orb.policy_manager()
        .set_policy_overrides(&created_policies(&orb, true), SetOverrideType::SetOverride)
        .unwrap();
    let client = IiopCorbaConnection::new().with_orb(&orb);
    let (_server, _a, r, reference) = ziop_target();
    echo(&client, &reference);
    assert_eq!(magics(&r.to_server), vec![*b"ZIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"ZIOP"]);
    // A server bound to an ORB whose PolicyManager holds ZIOP policies,
    // without set_ziop: its references declare them, it compresses.
    let server_orb = new_orb("ziop-orb-level-server");
    server_orb
        .policy_manager()
        .set_policy_overrides(
            &created_policies(&server_orb, true),
            SetOverrideType::SetOverride,
        )
        .unwrap();
    let server = CorbaServer::new();
    server.register(b"Echo", |_op, body, _e| {
        SkeletonResult::Reply(body.bytes().to_vec())
    });
    server.set_orb(Arc::clone(&server_orb));
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let r = relay("127.0.0.1", acceptor.listen_addr().port());
    let reference = server.object_reference("IDL:Echo:1.0", "127.0.0.1", r.port, b"Echo");
    let profile = IiopProfileBody::decode_encapsulation(&reference.iiop_profile).unwrap();
    assert!(profile.components.iter().any(|c| c.tag == TAG_POLICIES));
    echo(&client, &reference);
    assert_eq!(magics(&r.to_server), vec![*b"ZIOP"]);
    assert_eq!(magics(&r.to_client), vec![*b"ZIOP"]);
    // Without ZIOP overrides at the ORB scope the server declares none.
    let plain_orb = new_orb("ziop-orb-level-plain");
    let plain = CorbaServer::new();
    plain.set_orb(plain_orb);
    let profile = IiopProfileBody::decode_encapsulation(
        &plain
            .object_reference("IDL:Echo:1.0", "127.0.0.1", 1, b"Echo")
            .iiop_profile,
    )
    .unwrap();
    assert!(!profile.components.iter().any(|c| c.tag == TAG_POLICIES));
}

#[test]
fn corba_p2_12_2_client_ziop_policies_at_the_thread_level() {
    let orb = new_orb("ziop-thread-level");
    let client = Arc::new(IiopCorbaConnection::new().with_orb(&orb));
    let (_server, _a, r, reference) = ziop_target();
    // No overrides anywhere: GIOP.
    echo(&client, &reference);
    orb.policy_current()
        .set_policy_overrides(&created_policies(&orb, true), SetOverrideType::SetOverride)
        .unwrap();
    // This thread compresses, another thread of the same client does not.
    echo(&client, &reference);
    let (c, rf) = (Arc::clone(&client), reference.clone());
    std::thread::spawn(move || echo(&c, &rf)).join().unwrap();
    assert_eq!(magics(&r.to_server), vec![*b"GIOP", *b"ZIOP", *b"GIOP"]);
    // The thread scope comes before the ORB scope.
    orb.policy_manager()
        .set_policy_overrides(&created_policies(&orb, true), SetOverrideType::SetOverride)
        .unwrap();
    orb.policy_current()
        .set_policy_overrides(&created_policies(&orb, false), SetOverrideType::SetOverride)
        .unwrap();
    echo(&client, &reference);
    assert_eq!(magics(&r.to_server)[3], *b"GIOP");
}

#[test]
fn corba_p2_12_2_client_ziop_policies_at_the_reference_level() {
    let orb = new_orb("ziop-reference-level");
    let client = IiopCorbaConnection::new().with_orb(&orb);
    let (_server, _a, r, reference) = ziop_target();
    // Object::set_policy_overrides gives a new reference; the old one
    // keeps none.
    let compressed = ObjectRef::Reference(reference.clone())
        .set_policy_overrides(&created_policies(&orb, true), SetOverrideType::SetOverride)
        .unwrap()
        .reference()
        .unwrap()
        .clone();
    echo(&client, &compressed);
    echo(&client, &reference);
    assert_eq!(magics(&r.to_server), vec![*b"ZIOP", *b"GIOP"]);
    // The reference scope comes before the thread scope.
    orb.policy_current()
        .set_policy_overrides(&created_policies(&orb, true), SetOverrideType::SetOverride)
        .unwrap();
    let off = ObjectRef::Reference(reference.clone())
        .set_policy_overrides(&created_policies(&orb, false), SetOverrideType::SetOverride)
        .unwrap()
        .reference()
        .unwrap()
        .clone();
    echo(&client, &off);
    assert_eq!(magics(&r.to_server)[2], *b"GIOP");
    // A client without an ORB and without its own policies follows the
    // overrides of the reference with the ORB's built-in compressors.
    let bare = IiopCorbaConnection::new();
    echo(&bare, &compressed);
    assert_eq!(magics(&r.to_server)[3], *b"ZIOP");
}

#[test]
fn corba_p1_8_9_1_a_servant_runs_without_the_thread_overrides() {
    let orb = new_orb("policy-current-dispatch");
    let server = CorbaServer::new();
    let o = Arc::clone(&orb);
    server.register(b"Count", move |_op, _body, e| {
        let current = o.policy_current();
        let seen = current.get_policy_overrides(&[]).len();
        current
            .set_policy_overrides(&created_policies(&o, true), SetOverrideType::SetOverride)
            .unwrap();
        let mut w = zerodds_wire::Writer::new(e);
        w.write_u32(u32::try_from(seen).unwrap());
        SkeletonResult::Reply(w.finish())
    });
    server.set_orb(Arc::clone(&orb));
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let reference = object_reference(
        "IDL:Count:1.0",
        "127.0.0.1",
        acceptor.listen_addr().port(),
        b"Count",
    );
    // Two requests on one pooled connection, served by one thread: what
    // the first servant call set is gone for the second.
    let client = IiopCorbaConnection::new();
    for _ in 0..2 {
        let (reply, e) = client
            .invoke(&reference, "count", Endianness::Big, &no_arguments)
            .unwrap();
        assert_eq!(zerodds_wire::Reader::new(&reply, e).read_u32().unwrap(), 0);
    }
}
