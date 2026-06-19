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

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_iiop::Connection;
use zerodds_corba_interop::runtime::{AmhEndpoint, AmiClient, object_reference};
use zerodds_corba_rust::CorbaException;

fn u32_body(v: u32) -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u32(v).unwrap();
    w.into_bytes()
}
fn read_u32(b: &[u8], e: Endianness) -> u32 {
    BufferReader::new(b, e).read_u32().unwrap()
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
fn amh_deferred_out_of_order_replies() {
    let (mut client, server) = pair();

    let id_slow = client.send_poll("slow", &u32_body(1)).unwrap();
    let id_fast = client.send_poll("fast", &u32_body(2)).unwrap();

    // Server accepts BOTH before sending any reply (= core of AMH).
    let req1 = server.accept_request().unwrap().unwrap();
    let req2 = server.accept_request().unwrap().unwrap();
    assert_eq!(req1.operation, "slow");
    assert_eq!(req2.operation, "fast");

    // Reply deferred + out-of-order: second request first, then the first.
    req2.handler
        .send_reply(u32_body(read_u32(&req2.body, req2.endianness) * 10))
        .unwrap();
    req1.handler
        .send_reply(u32_body(read_u32(&req1.body, req1.endianness) * 10))
        .unwrap();

    // Client collects both, correctly correlated.
    let (fast_b, fast_e) = client.get_reply(id_fast).unwrap().expect("ok");
    assert_eq!(read_u32(&fast_b, fast_e), 20);
    let (slow_b, slow_e) = client.get_reply(id_slow).unwrap().expect("ok");
    assert_eq!(read_u32(&slow_b, slow_e), 10);
}

/// Server answers a parked request with an exception via the handler.
#[test]
fn amh_deferred_exception() {
    let (mut client, server) = pair();
    let id = client.send_poll("boom", &u32_body(0)).unwrap();
    let req = server.accept_request().unwrap().unwrap();
    req.handler
        .send_exception(CorbaException::SystemException {
            minor: 7,
            message: "CORBA INTERNAL: boom",
        })
        .unwrap();
    assert!(
        client.get_reply(id).unwrap().is_err(),
        "exception must arrive as Err"
    );
}
