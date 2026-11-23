// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! AMH — server-side Asynchronous Method Handling (§22.9) e2e over real
//! GIOP/IIOP. The `AmhEndpoint` accepts requests and hands them out together
//! with a deferred `AmhResponseHandler`, WITHOUT replying inline. Proves:
//! * several **parked** requests at once,
//! * **out-of-order** reply (server answers the 2nd request first),
//! * exception path via the handler,
//!
//! Client side is the `AmiClient` (request_id demux).

use std::net::TcpListener;
use zerodds_corba_rust::marshaled_body;

use zerodds_corba_iiop::Connection;
use zerodds_corba_interop::runtime::{AmhEndpoint, AmiClient, object_reference};
use zerodds_corba_rust::{
    AlignedBody, CompletionStatus, CorbaException, SystemException, SystemExceptionKind,
};
use zerodds_wire::{Endianness, Writer};

fn u32_body(v: u32) -> Vec<u8> {
    let mut w = Writer::new(Endianness::Big);
    w.write_u32(v);
    w.finish()
}
/// The `unsigned long` of a reply body, read at its offset in the reply.
fn read_u32(b: &zerodds_corba_rust::AlignedBody, e: Endianness) -> u32 {
    b.reader(e).read_u32().unwrap()
}
/// The `unsigned long` argument of a request, read at the body's offset.
fn read_u32_arg(b: &zerodds_corba_rust::AlignedBody, e: Endianness) -> u32 {
    b.reader(e).read_u32().unwrap()
}

/// Connects an `AmiClient` (client) to an `AmhEndpoint` (server) over a real
/// loopback TCP pair.
fn pair() -> (AmiClient, AmhEndpoint) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let ior = object_reference("IDL:X:1.0", &addr.ip().to_string(), addr.port(), b"X");
    let client = AmiClient::connect(&ior).unwrap();
    let (server_stream, _) = listener.accept().unwrap();
    let server = AmhEndpoint::new(Connection::from_stream(server_stream).unwrap());
    (client, server)
}

/// Server parks two requests and answers them **deferred + out-of-order**; the
/// client correlates the replies by request_id.
#[test]
fn corbarust_6_amh_deferred_out_of_order_replies() {
    let (mut client, server) = pair();

    let id_slow = client
        .send_poll("slow", &marshaled_body(&u32_body(1), 0))
        .unwrap();
    let id_fast = client
        .send_poll("fast", &marshaled_body(&u32_body(2), 0))
        .unwrap();

    // Server accepts BOTH before sending any reply (= core of AMH).
    let req1 = server.accept_request().unwrap().unwrap();
    let req2 = server.accept_request().unwrap().unwrap();
    assert_eq!(req1.operation, "slow");
    assert_eq!(req2.operation, "fast");

    // Reply deferred + out-of-order: second request first, then the first.
    req2.handler
        .send_reply(u32_body(read_u32_arg(&req2.body, req2.endianness) * 10))
        .unwrap();
    req1.handler
        .send_reply(u32_body(read_u32_arg(&req1.body, req1.endianness) * 10))
        .unwrap();

    // Client collects both, correctly correlated.
    let (fast_b, fast_e) = client.get_reply(id_fast).unwrap().expect("ok");
    assert_eq!(read_u32(&fast_b, fast_e), 20);
    let (slow_b, slow_e) = client.get_reply(id_slow).unwrap().expect("ok");
    assert_eq!(read_u32(&slow_b, slow_e), 10);
}

/// Server answers a parked request with an exception via the handler.
#[test]
fn corbarust_6_amh_deferred_exception() {
    let (mut client, server) = pair();
    let id = client
        .send_poll("boom", &marshaled_body(&u32_body(0), 0))
        .unwrap();
    let req = server.accept_request().unwrap().unwrap();
    req.handler
        .send_exception(CorbaException::system(
            SystemExceptionKind::Internal,
            7,
            CompletionStatus::Yes,
        ))
        .unwrap();
    // The exception crosses with its identity, minor code and completion
    // status (CORBA 3.3 Part 1 §8.12).
    assert_eq!(
        client.get_reply(id).unwrap(),
        Err(CorbaException::System(SystemException::new(
            SystemExceptionKind::Internal,
            7,
            CompletionStatus::Yes
        ))),
    );
}

/// CORBA 3.3 Part 2 §9.4.1, §9.4.3: a parked request answered with a user
/// exception the servant received in the other byte order than the
/// request's. Its body cannot be read in another byte order: the
/// `AmhResponseHandler` writes the reply in the exception's byte order, and
/// the client reads the repository id and the member.
#[test]
fn corba_p2_9_4_3_an_amh_user_exception_goes_out_in_its_own_byte_order() {
    let (mut client, server) = pair();
    let id = client
        .send_poll("boom", &marshaled_body(&u32_body(0), 0))
        .unwrap();
    let req = server.accept_request().unwrap().unwrap();
    let other = match req.endianness {
        Endianness::Big => Endianness::Little,
        Endianness::Little => Endianness::Big,
    };
    let mut w = Writer::new(other);
    w.write_string("IDL:Boom:1.0");
    w.write_u32(7);
    req.handler
        .send_exception(CorbaException::UserException {
            repository_id: "IDL:Boom:1.0".into(),
            body: AlignedBody::new(&w.finish(), 0),
            endianness: other,
        })
        .unwrap();
    let Err(CorbaException::UserException {
        repository_id,
        body,
        endianness,
    }) = client.get_reply(id).unwrap()
    else {
        panic!("a user exception");
    };
    assert_eq!(repository_id, "IDL:Boom:1.0");
    assert_eq!(endianness, other);
    let mut r = body.reader(endianness);
    assert_eq!(r.read_string().unwrap(), "IDL:Boom:1.0");
    assert_eq!(r.read_u32().unwrap(), 7);
}
