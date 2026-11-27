// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 2 §9.6 Object Location and the client side of
//! §9.4.3.2 / §9.4.6: a client follows `LOCATION_FORWARD` and
//! `OBJECT_FORWARD`, records `_PERM` forwards and falls back to the
//! original reference when the recorded agent fails, resends after
//! `NEEDS_ADDRESSING_MODE` in the requested disposition; a server acts as
//! a location agent and serves every addressing disposition (§9.4.2.1).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use zerodds_corba_interop::string_argument;

use zerodds_corba_giop::{
    AddressingDisposition, IorAddressingInfo, LocateReply, LocateRequest, LocateStatusType,
    Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContextList, TargetAddress,
    TargetProfile, Version,
};
use zerodds_corba_iiop::{Acceptor, Connection, IiopProfileBody};
use zerodds_corba_interop::runtime::{
    AmiClient, BiDirEndpoint, CorbaServer, IiopCorbaConnection, LocateOutcome, object_reference,
};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::{
    CompletionStatus, CorbaConnection, CorbaException, ObjectReference, SkeletonResult,
    SystemExceptionKind,
};
use zerodds_wire::{Endianness, Writer};

/// A server whose object `key` answers every call with `text`.
fn answering(key: &'static [u8], text: &'static str) -> (CorbaServer, Acceptor, ObjectReference) {
    let server = CorbaServer::new();
    server.register(key, move |_op, _body, _e| {
        SkeletonResult::Reply(encode_string_body(text))
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let reference = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), key);
    (server, acceptor, reference)
}

fn call(conn: &IiopCorbaConnection, target: &ObjectReference) -> Result<String, CorbaException> {
    conn.invoke(target, "ping", Endianness::Big, &string_argument("x"))
        .map(|(body, _)| decode_string_body(&body).unwrap())
}

/// A port nothing listens on.
fn dead_reference(key: &[u8]) -> ObjectReference {
    let port = zerodds_test_domains::free_tcp_port();
    object_reference("IDL:Echo:1.0", "127.0.0.1", port, key)
}

/// A GIOP peer that answers each message of its first connection with
/// what `answer` returns.
fn raw_peer<F>(answer: F) -> (u16, JoinHandle<()>)
where
    F: Fn(Message, Endianness) -> Option<Message> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut conn = Connection::from_stream(stream).unwrap();
        while let Ok((msg, e)) = conn.read_message_with_endianness() {
            if let Some(reply) = answer(msg, e) {
                conn.write_message(Version::V1_2, e, false, &reply).unwrap();
            }
        }
    });
    (port, handle)
}

fn reply(request_id: u32, reply_status: ReplyStatusType, body: Vec<u8>) -> Message {
    Message::Reply(Reply {
        request_id,
        reply_status,
        service_context: ServiceContextList::default(),
        body,
    })
}

fn disposition_body(d: AddressingDisposition, e: Endianness) -> Vec<u8> {
    let mut w = Writer::new(e);
    d.encode(&mut w);
    w.finish()
}

fn ior_body(r: &ObjectReference, e: Endianness) -> Vec<u8> {
    let mut w = Writer::new(e);
    r.encode(&mut w);
    w.finish()
}

fn profile_key(p: &TargetProfile) -> Vec<u8> {
    assert_eq!(p.tag, 0, "TAG_INTERNET_IOP");
    IiopProfileBody::decode_encapsulation(&p.profile_data)
        .unwrap()
        .object_key
}

#[test]
fn corba_p2_9_6_client_follows_a_location_forward() {
    let (_real, _real_acceptor, real_ref) = answering(b"Echo", "from real");
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let agent_obj = object_reference("IDL:Echo:1.0", "127.0.0.1", agent_port(&agent_ref), b"Obj");
    agent.forward(b"Obj", real_ref.clone(), false);

    let conn = IiopCorbaConnection::new();
    assert_eq!(call(&conn, &agent_obj).unwrap(), "from real");
    // A LOCATION_FORWARD is not recorded: the next invocation asks the
    // agent again, which now serves the object itself.
    assert_eq!(conn.forwarded_reference(&agent_obj), None);
    assert!(agent.stop_forwarding(b"Obj"));
    agent.register(b"Obj", |_op, _body, _e| {
        SkeletonResult::Reply(encode_string_body("from agent"))
    });
    assert_eq!(call(&conn, &agent_obj).unwrap(), "from agent");
}

fn agent_port(r: &ObjectReference) -> u16 {
    IiopProfileBody::decode_encapsulation(&r.iiop_profile)
        .unwrap()
        .port
}

#[test]
fn corba_p2_9_6_forward_perm_replaces_the_reference() {
    let (_real, _real_acceptor, real_ref) = answering(b"Echo", "from real");
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let agent_obj = object_reference("IDL:Echo:1.0", "127.0.0.1", agent_port(&agent_ref), b"Obj");
    agent.forward(b"Obj", real_ref.clone(), true);

    let conn = IiopCorbaConnection::new();
    assert_eq!(call(&conn, &agent_obj).unwrap(), "from real");
    assert_eq!(conn.forwarded_reference(&agent_obj), Some(real_ref));
    // The new reference is used from now on; the agent is not asked.
    agent.stop_forwarding(b"Obj");
    agent.register(b"Obj", |_op, _body, _e| {
        SkeletonResult::Reply(encode_string_body("from agent"))
    });
    assert_eq!(call(&conn, &agent_obj).unwrap(), "from real");
}

#[test]
fn corba_p2_9_6_an_unreachable_recorded_forward_restarts_at_the_original_address() {
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let agent_obj = object_reference("IDL:Echo:1.0", "127.0.0.1", agent_port(&agent_ref), b"Obj");
    let dead = dead_reference(b"Obj");
    agent.forward(b"Obj", dead.clone(), true);

    let conn = IiopCorbaConnection::new();
    // The forward target cannot be reached in this invocation.
    let err = call(&conn, &agent_obj).unwrap_err();
    assert_eq!(
        err.as_system().map(|e| (e.kind, e.completed)),
        Some((SystemExceptionKind::Transient, CompletionStatus::No))
    );
    assert_eq!(conn.forwarded_reference(&agent_obj), Some(dead));
    // Next invocation: the recorded target fails, the client restarts at
    // the original address and drops the record.
    agent.stop_forwarding(b"Obj");
    agent.register(b"Obj", |_op, _body, _e| {
        SkeletonResult::Reply(encode_string_body("from agent"))
    });
    assert_eq!(call(&conn, &agent_obj).unwrap(), "from agent");
    assert_eq!(conn.forwarded_reference(&agent_obj), None);
}

#[test]
fn corba_p2_9_6_a_recorded_forward_without_the_object_restarts_at_the_original_address() {
    let gone = Arc::new(AtomicBool::new(false));
    let moved = CorbaServer::new();
    {
        let gone = gone.clone();
        moved.register(b"Moved", move |_op, _body, _e| {
            if gone.load(Ordering::SeqCst) {
                SkeletonResult::Exception(CorbaException::system(
                    SystemExceptionKind::ObjectNotExist,
                    0,
                    CompletionStatus::No,
                ))
            } else {
                SkeletonResult::Reply(encode_string_body("from moved"))
            }
        });
    }
    let moved_acceptor = moved.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let moved_ref = object_reference(
        "IDL:Echo:1.0",
        "127.0.0.1",
        moved_acceptor.listen_addr().port(),
        b"Moved",
    );
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let agent_obj = object_reference("IDL:Echo:1.0", "127.0.0.1", agent_port(&agent_ref), b"Obj");
    agent.forward(b"Obj", moved_ref.clone(), true);

    let conn = IiopCorbaConnection::new();
    assert_eq!(call(&conn, &agent_obj).unwrap(), "from moved");
    assert_eq!(conn.forwarded_reference(&agent_obj), Some(moved_ref));
    gone.store(true, Ordering::SeqCst);
    agent.stop_forwarding(b"Obj");
    agent.register(b"Obj", |_op, _body, _e| {
        SkeletonResult::Reply(encode_string_body("from agent"))
    });
    assert_eq!(call(&conn, &agent_obj).unwrap(), "from agent");
    assert_eq!(conn.forwarded_reference(&agent_obj), None);
}

#[test]
fn corba_p2_9_6_a_forward_cycle_ends_with_transient() {
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let looping = object_reference("IDL:Echo:1.0", "127.0.0.1", agent_port(&agent_ref), b"Loop");
    agent.forward(b"Loop", looping.clone(), false);
    let err = call(&IiopCorbaConnection::new(), &looping).unwrap_err();
    assert_eq!(
        err.as_system().map(|e| (e.kind, e.completed)),
        Some((SystemExceptionKind::Transient, CompletionStatus::No))
    );
}

#[test]
fn corba_p2_9_6_the_forwarded_request_carries_the_service_contexts() {
    // The CSIv2 EstablishContext (and the code set context) go out again
    // with the forwarded request: the target checks the credentials.
    let (real, _real_acceptor, real_ref) = answering(b"Echo", "authenticated");
    real.require_credentials(|user, pass| user == "alice" && pass == "secret");
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let agent_obj = object_reference("IDL:Echo:1.0", "127.0.0.1", agent_port(&agent_ref), b"Obj");
    agent.forward(b"Obj", real_ref, false);

    let with = IiopCorbaConnection::new().with_csiv2_credentials("alice", "secret");
    assert_eq!(call(&with, &agent_obj).unwrap(), "authenticated");
    let without = IiopCorbaConnection::new();
    let err = call(&without, &agent_obj).unwrap_err();
    assert_eq!(
        err.as_system().map(|e| e.kind),
        Some(SystemExceptionKind::NoPermission)
    );
}

/// A peer that refuses `KeyAddr` with `NEEDS_ADDRESSING_MODE(want)` and
/// answers a request in `want` with the key it named.
fn readdressing_peer(want: AddressingDisposition) -> (u16, JoinHandle<()>, Arc<AtomicUsize>) {
    let refusals = Arc::new(AtomicUsize::new(0));
    let counter = refusals.clone();
    let (port, handle) = raw_peer(move |msg, e| {
        let Message::Request(req) = msg else {
            return None;
        };
        let key = match (&req.target, want) {
            (TargetAddress::Profile(p), AddressingDisposition::ProfileAddr) => profile_key(p),
            (TargetAddress::Reference(info), AddressingDisposition::ReferenceAddr) => {
                assert_eq!(info.type_id, "IDL:Echo:1.0");
                profile_key(info.selected_profile().unwrap())
            }
            _ => {
                counter.fetch_add(1, Ordering::SeqCst);
                return Some(reply(
                    req.request_id,
                    ReplyStatusType::NeedsAddressingMode,
                    disposition_body(want, e),
                ));
            }
        };
        Some(reply(
            req.request_id,
            ReplyStatusType::NoException,
            encode_string_body(&String::from_utf8(key).unwrap()),
        ))
    });
    (port, handle, refusals)
}

#[test]
fn corba_p2_9_4_needs_addressing_mode_is_resent_with_the_profile() {
    let (port, _peer, refusals) = readdressing_peer(AddressingDisposition::ProfileAddr);
    let target = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"ByProfile");
    let conn = IiopCorbaConnection::new();
    assert_eq!(call(&conn, &target).unwrap(), "ByProfile");
    assert_eq!(refusals.load(Ordering::SeqCst), 1);
}

#[test]
fn corba_p2_9_4_needs_addressing_mode_is_resent_with_the_ior() {
    let (port, _peer, refusals) = readdressing_peer(AddressingDisposition::ReferenceAddr);
    let target = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"ByReference");
    let conn = IiopCorbaConnection::new();
    assert_eq!(call(&conn, &target).unwrap(), "ByReference");
    assert_eq!(refusals.load(Ordering::SeqCst), 1);
}

#[test]
fn corba_p2_9_4_an_agent_asking_again_for_the_disposition_it_got_is_transient() {
    // The peer asks for KeyAddr while it is addressed with KeyAddr.
    let (port, _peer) = raw_peer(|msg, e| match msg {
        Message::Request(req) => Some(reply(
            req.request_id,
            ReplyStatusType::NeedsAddressingMode,
            disposition_body(AddressingDisposition::KeyAddr, e),
        )),
        _ => None,
    });
    let target = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Obj");
    let err = call(&IiopCorbaConnection::new(), &target).unwrap_err();
    assert_eq!(
        err.as_system().map(|e| (e.kind, e.completed)),
        Some((SystemExceptionKind::Transient, CompletionStatus::No))
    );
}

#[test]
fn corba_p2_9_6_locate_follows_object_forward_and_reports_unknown_object() {
    let (_real, _real_acceptor, real_ref) = answering(b"Echo", "from real");
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let port = agent_port(&agent_ref);
    let agent_obj = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Obj");
    agent.forward(b"Obj", real_ref.clone(), true);

    let conn = IiopCorbaConnection::new();
    assert_eq!(
        conn.locate(&agent_obj).unwrap(),
        LocateOutcome::Here(real_ref.clone())
    );
    // OBJECT_FORWARD_PERM is recorded like LOCATION_FORWARD_PERM.
    assert_eq!(conn.forwarded_reference(&agent_obj), Some(real_ref.clone()));
    assert_eq!(
        conn.locate(&real_ref).unwrap(),
        LocateOutcome::Here(real_ref)
    );
    let unknown = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Nobody");
    assert_eq!(conn.locate(&unknown).unwrap(), LocateOutcome::Unknown);
}

#[test]
fn corba_p2_9_4_6_loc_needs_addressing_mode_resends_the_locate_request() {
    let (port, _peer) = raw_peer(|msg, e| match msg {
        Message::LocateRequest(lr) => Some(Message::LocateReply(match &lr.target {
            TargetAddress::Profile(p) if profile_key(p) == b"Obj" => LocateReply {
                request_id: lr.request_id,
                locate_status: LocateStatusType::ObjectHere,
                body: Vec::new(),
            },
            _ => LocateReply {
                request_id: lr.request_id,
                locate_status: LocateStatusType::LocNeedsAddressingMode,
                body: disposition_body(AddressingDisposition::ProfileAddr, e),
            },
        })),
        _ => None,
    });
    let target = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Obj");
    assert_eq!(
        IiopCorbaConnection::new().locate(&target).unwrap(),
        LocateOutcome::Here(target)
    );
}

#[test]
fn corba_p2_9_4_6_loc_system_exception_is_raised() {
    let (port, _peer) = raw_peer(|msg, e| match msg {
        Message::LocateRequest(lr) => {
            let mut w = Writer::new(e);
            zerodds_corba_rust::SystemException::new(
                SystemExceptionKind::NoPermission,
                3,
                CompletionStatus::No,
            )
            .encode(&mut w);
            Some(Message::LocateReply(LocateReply {
                request_id: lr.request_id,
                locate_status: LocateStatusType::LocSystemException,
                body: w.finish(),
            }))
        }
        _ => None,
    });
    let target = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Obj");
    let err = IiopCorbaConnection::new().locate(&target).unwrap_err();
    assert_eq!(
        err.as_system().map(|e| (e.kind, e.minor)),
        Some((SystemExceptionKind::NoPermission, 3))
    );
}

/// One request over a raw connection; the reply.
fn raw_call(conn: &mut Connection, request_id: u32, target: TargetAddress) -> Reply {
    let req = Message::Request(Request {
        request_id,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target,
        operation: "ping".into(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body: encode_string_body("x"),
    });
    conn.write_message(Version::V1_2, Endianness::Little, false, &req)
        .unwrap();
    match conn.read_message().unwrap() {
        Message::Reply(r) => r,
        other => panic!("expected a Reply, got {:?}", other.message_type()),
    }
}

#[test]
fn corba_p2_9_4_2_1_a_server_serves_every_addressing_disposition() {
    let (_server, acceptor, reference) = answering(b"Echo", "served");
    let stream = std::net::TcpStream::connect(acceptor.listen_addr()).unwrap();
    let mut conn = Connection::from_stream(stream).unwrap();
    let iiop = TargetProfile {
        tag: 0,
        profile_data: reference.iiop_profile.clone(),
    };
    for (id, target) in [
        (1, TargetAddress::Profile(iiop.clone())),
        (
            2,
            TargetAddress::Reference(IorAddressingInfo {
                selected_profile_index: 1,
                type_id: reference.type_id.clone(),
                profiles: vec![
                    TargetProfile {
                        tag: 1,
                        profile_data: vec![0],
                    },
                    iiop.clone(),
                ],
            }),
        ),
    ] {
        let r = raw_call(&mut conn, id, target);
        assert_eq!(r.reply_status, ReplyStatusType::NoException);
        // The servant writes its body big-endian whatever the request's
        // byte order was.
        assert_eq!(decode_string_body(&r.body).unwrap(), "served");
    }
    // A profile that is not IIOP names no key this server can read: it
    // asks for KeyAddr instead of dropping the request.
    let r = raw_call(
        &mut conn,
        3,
        TargetAddress::Profile(TargetProfile {
            tag: 1,
            profile_data: vec![0],
        }),
    );
    assert_eq!(r.reply_status, ReplyStatusType::NeedsAddressingMode);
    assert_eq!(
        r.body,
        disposition_body(AddressingDisposition::KeyAddr, Endianness::Little)
    );
    // A LocateRequest by profile.
    conn.write_message(
        Version::V1_2,
        Endianness::Big,
        false,
        &Message::LocateRequest(LocateRequest {
            request_id: 4,
            target: TargetAddress::Profile(iiop),
        }),
    )
    .unwrap();
    match conn.read_message().unwrap() {
        Message::LocateReply(l) => assert_eq!(l.locate_status, LocateStatusType::ObjectHere),
        other => panic!("expected a LocateReply, got {:?}", other.message_type()),
    }
}

#[test]
fn corba_p2_9_6_a_location_agent_answers_locate_with_object_forward() {
    let (_real, _real_acceptor, real_ref) = answering(b"Echo", "from real");
    let (agent, agent_acceptor, _agent_ref) = answering(b"Agent", "unused");
    agent.forward(b"Obj", real_ref.clone(), true);
    let stream = std::net::TcpStream::connect(agent_acceptor.listen_addr()).unwrap();
    let mut conn = Connection::from_stream(stream).unwrap();
    for (version, status) in [
        (Version::V1_2, LocateStatusType::ObjectForwardPerm),
        // _PERM does not exist before GIOP 1.2.
        (Version::V1_0, LocateStatusType::ObjectForward),
    ] {
        conn.write_message(
            version,
            Endianness::Big,
            false,
            &Message::LocateRequest(LocateRequest {
                request_id: 9,
                target: TargetAddress::Key(b"Obj".to_vec()),
            }),
        )
        .unwrap();
        let (msg, e) = conn.read_message_with_endianness().unwrap();
        let Message::LocateReply(l) = msg else {
            panic!("expected a LocateReply");
        };
        assert_eq!(l.locate_status, status);
        let mut r = zerodds_wire::Reader::new(&l.body, e);
        assert_eq!(ObjectReference::decode(&mut r).unwrap(), real_ref);
    }
}

#[test]
fn corba_p2_9_6_an_ami_request_follows_a_location_forward() {
    let (_real, _real_acceptor, real_ref) = answering(b"Echo", "from real");
    let (agent, _agent_acceptor, agent_ref) = answering(b"Agent", "unused");
    let agent_obj = object_reference("IDL:Echo:1.0", "127.0.0.1", agent_port(&agent_ref), b"Obj");
    agent.forward(b"Obj", real_ref.clone(), true);
    let mut ami = AmiClient::connect(&agent_obj).unwrap();
    let id = ami.send_poll("ping", &string_argument("x")).unwrap();
    let (body, _) = ami.get_reply(id).unwrap().unwrap();
    assert_eq!(decode_string_body(body.bytes()).unwrap(), "from real");
    assert_eq!(ami.forwarded_reference(), Some(real_ref));
}

#[test]
fn corba_p2_9_4_an_ami_request_is_readdressed_and_keeps_its_id() {
    let (port, _peer, refusals) = readdressing_peer(AddressingDisposition::ProfileAddr);
    let target = object_reference("IDL:Echo:1.0", "127.0.0.1", port, b"Ami");
    let mut ami = AmiClient::connect(&target).unwrap();
    let id = ami.send_poll("ping", &string_argument("x")).unwrap();
    assert_eq!(ami.perform_work().unwrap(), id);
    let (body, _) = ami.get_reply(id).unwrap().unwrap();
    assert_eq!(decode_string_body(body.bytes()).unwrap(), "Ami");
    assert_eq!(refusals.load(Ordering::SeqCst), 1);
}

#[test]
fn corba_p2_9_6_a_bidir_request_follows_a_location_forward() {
    let (_real, _real_acceptor, real_ref) = answering(b"Echo", "from real");
    let forward_to = real_ref.clone();
    let (port, _peer) = raw_peer(move |msg, e| match msg {
        Message::Request(req) => Some(reply(
            req.request_id,
            ReplyStatusType::LocationForward,
            ior_body(&forward_to, e),
        )),
        _ => None,
    });
    let stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut endpoint =
        BiDirEndpoint::originator(Connection::from_stream(stream).unwrap(), Vec::new());
    let (body, _) = endpoint
        .invoke(b"Callback", "ping", &encode_string_body("x"))
        .unwrap();
    assert_eq!(decode_string_body(body.bytes()).unwrap(), "from real");
}
