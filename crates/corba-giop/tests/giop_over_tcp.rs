// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! A transactional CORBA invocation over a real TCP connection.
//!
//! Three of the crates that came back off `zerodds-cdr` meeting on one
//! socket: `corba-giop` frames the request and the reply, the
//! `TransactionService` service context (id 0) carries a
//! `corba-cos-transactions` `PropagationContext`, and the
//! `RTCorbaPriority` context (id 10) carries a `corba-rt` `Priority`.
//!
//! What this catches that no single-crate test does: a message is read
//! off a **stream** here, not out of a `Vec`. The 12-byte header's
//! `message_size` decides how many more octets belong to this message,
//! so a codec that miscounted the body — by a padding octet it should
//! not have written, or by an alignment origin off by four — would
//! either block waiting for octets nobody sends or read the next
//! message's header as this one's arguments. In a single-process
//! round-trip both mistakes cancel.
//!
//! The reply travels back over the same connection, and the two service
//! contexts have to come out of it holding what went in.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use zerodds_corba_cos_transactions::{Otid, PropagationContext};
use zerodds_corba_giop::{
    HEADER_SIZE, Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContext,
    ServiceContextList, ServiceContextTag, TargetAddress, Version, decode_message, encode_message,
};
use zerodds_corba_rt::{
    Priority, RT_CORBA_PRIORITY_SC_ID, decode_priority_context, encode_priority_context,
};
use zerodds_wire::Endianness;

/// Reads exactly one GIOP message off the stream: the 12-byte header
/// first, then the `message_size` octets it declares.
///
/// This is the read a real ORB does, and the reason the test is over a
/// socket: `message_size` has to be right or this hangs.
fn read_one_message(stream: &mut TcpStream) -> Vec<u8> {
    let mut header = [0u8; HEADER_SIZE];
    stream.read_exact(&mut header).expect("GIOP header");
    let size = if header[6] & 1 == 1 {
        u32::from_le_bytes([header[8], header[9], header[10], header[11]])
    } else {
        u32::from_be_bytes([header[8], header[9], header[10], header[11]])
    } as usize;
    let mut message = header.to_vec();
    message.resize(HEADER_SIZE + size, 0);
    stream
        .read_exact(&mut message[HEADER_SIZE..])
        .expect("GIOP body");
    message
}

fn transactional_request(endianness: Endianness) -> (Message, PropagationContext, Priority) {
    let context = PropagationContext::flat(30, Otid::new(7, vec![0xAA, 0xBB, 0xCC]));
    let priority = Priority::new(12_345).unwrap();

    let contexts = ServiceContextList(vec![
        ServiceContext::new(
            ServiceContextTag::TransactionService.as_u32(),
            context.to_service_context_data(endianness).unwrap(),
        ),
        ServiceContext::new(
            RT_CORBA_PRIORITY_SC_ID,
            encode_priority_context(priority, endianness).unwrap(),
        ),
    ]);

    let request = Request {
        request_id: 42,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"account/checking".to_vec()),
        operation: "withdraw".into(),
        requesting_principal: None,
        service_context: contexts,
        // The operation's arguments: `long amount = 250`, in the
        // message's byte order.
        body: match endianness {
            Endianness::Big => 250i32.to_be_bytes().to_vec(),
            Endianness::Little => 250i32.to_le_bytes().to_vec(),
        },
    };
    (Message::Request(request), context, priority)
}

fn run_one_invocation(endianness: Endianness) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().unwrap();

    let (message, expected_context, expected_priority) = transactional_request(endianness);
    let sent = encode_message(Version::V1_2, endianness, false, &message).unwrap();

    // The server: read one request, check what its service contexts
    // carry, answer.
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let raw = read_one_message(&mut stream);
        let (received, rest) = decode_message(&raw).expect("decode request");
        assert!(rest.is_empty(), "a message read by its own length is whole");

        let Message::Request(request) = received else {
            panic!("expected a Request");
        };
        assert_eq!(request.request_id, 42);
        assert_eq!(request.operation, "withdraw");
        assert_eq!(
            request.target,
            TargetAddress::Key(b"account/checking".to_vec())
        );

        let transaction = request
            .service_context
            .0
            .iter()
            .find(|c| c.context_id == ServiceContextTag::TransactionService.as_u32())
            .expect("TransactionService context");
        let context = PropagationContext::from_service_context_data(&transaction.context_data)
            .expect("PropagationContext");

        let rt = request
            .service_context
            .0
            .iter()
            .find(|c| c.context_id == RT_CORBA_PRIORITY_SC_ID)
            .expect("RTCorbaPriority context");
        let priority = decode_priority_context(&rt.context_data).expect("Priority");

        // The reply carries the same transaction back, which is what a
        // server that joined it does.
        let reply = Message::Reply(Reply {
            request_id: request.request_id,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList(vec![ServiceContext::new(
                ServiceContextTag::TransactionService.as_u32(),
                context.to_service_context_data(endianness).unwrap(),
            )]),
            body: request.body.clone(),
        });
        let out = encode_message(Version::V1_2, endianness, false, &reply).unwrap();
        stream.write_all(&out).expect("write reply");
        stream.flush().ok();
        (context, priority, request.body)
    });

    let mut client = TcpStream::connect(address).expect("connect");
    client.write_all(&sent).expect("write request");
    client.flush().ok();

    let raw = read_one_message(&mut client);
    let (reply, rest) = decode_message(&raw).expect("decode reply");
    assert!(rest.is_empty());

    let (server_context, server_priority, server_body) = server.join().unwrap();
    assert_eq!(server_context, expected_context, "propagated transaction");
    assert_eq!(server_priority, expected_priority, "propagated priority");
    assert_eq!(server_body.len(), 4, "the operation's `long` argument");

    let Message::Reply(reply) = reply else {
        panic!("expected a Reply");
    };
    assert_eq!(reply.request_id, 42);
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    let returned =
        PropagationContext::from_service_context_data(&reply.service_context.0[0].context_data)
            .expect("the reply's PropagationContext");
    assert_eq!(returned, expected_context, "the transaction came back");
}

#[test]
fn transactional_invocation_big_endian() {
    run_one_invocation(Endianness::Big);
}

#[test]
fn transactional_invocation_little_endian() {
    run_one_invocation(Endianness::Little);
}

/// Two requests written in one `write_all`, the way a client that
/// pipelines does. The second is only readable if the first's
/// `message_size` is exact.
#[test]
fn two_pipelined_requests_are_separated_by_their_declared_size() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().unwrap();

    let (first, _, _) = transactional_request(Endianness::Little);
    // A second request with an *empty* body — the case where the ORBs
    // write no trailing alignment padding at all, so its declared size
    // is the one a codec that padded anyway would get wrong.
    let second = Message::Request(Request {
        request_id: 43,
        response_flags: ResponseFlags::SYNC_NONE,
        target: TargetAddress::Key(b"account/checking".to_vec()),
        operation: "balance".into(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body: Vec::new(),
    });

    let mut wire = encode_message(Version::V1_2, Endianness::Little, false, &first).unwrap();
    wire.extend_from_slice(
        &encode_message(Version::V1_2, Endianness::Little, false, &second).unwrap(),
    );

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let a = read_one_message(&mut stream);
        let b = read_one_message(&mut stream);
        let (first, rest_a) = decode_message(&a).expect("first");
        assert!(rest_a.is_empty());
        let (second, rest_b) = decode_message(&b).expect("second");
        assert!(rest_b.is_empty());
        (first, second)
    });

    let mut client = TcpStream::connect(address).expect("connect");
    client.write_all(&wire).expect("write");
    client.flush().ok();
    drop(client);

    let (a, b) = server.join().unwrap();
    let Message::Request(a) = a else {
        panic!("expected a Request")
    };
    let Message::Request(b) = b else {
        panic!("expected a Request")
    };
    assert_eq!(a.request_id, 42);
    assert_eq!(a.operation, "withdraw");
    assert_eq!(b.request_id, 43);
    assert_eq!(b.operation, "balance");
    assert!(
        b.body.is_empty(),
        "an operation with no arguments has no body"
    );
}
