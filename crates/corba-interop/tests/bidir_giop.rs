// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Bidirectional GIOP (§15.8) e2e over a real TCP socket pair. Two
//! `BiDirEndpoint`s share ONE connection; proves:
//! * forward call (originator → acceptor),
//! * the acceptor receives the announced listen points (the BiDir service
//!   context tag 5 travels on the wire),
//! * **callback**: the acceptor invokes an object that lives on the originator,
//!   over the SAME connection (no new socket) — the core of §15.8,
//! * `request_id` parity (originator even, acceptor odd),
//! * out-of-order reply demux (stash).

use std::net::{TcpListener, TcpStream};

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_iiop::{BiDirIiopListenPoint, Connection};
use zerodds_corba_interop::runtime::BiDirEndpoint;
use zerodds_corba_rust::SkeletonResult;

fn u32_body(v: u32) -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u32(v).unwrap();
    w.into_bytes()
}
fn read_u32(b: &[u8], e: Endianness) -> u32 {
    BufferReader::new(b, e).read_u32().unwrap()
}

/// Connects two endpoints over a real loopback TCP pair: originator
/// (client that opens the connection) + acceptor (server).
fn bidir_pair(listen_points: Vec<BiDirIiopListenPoint>) -> (BiDirEndpoint, BiDirEndpoint) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let client_stream = TcpStream::connect(addr).unwrap();
    let (server_stream, _) = listener.accept().unwrap();
    let originator = BiDirEndpoint::originator(
        Connection::from_stream(client_stream).unwrap(),
        listen_points,
    );
    let acceptor = BiDirEndpoint::acceptor(Connection::from_stream(server_stream).unwrap());
    (originator, acceptor)
}

/// The acceptor calls BACK over the connection opened by the originator to an
/// object that lives on the originator (§15.8 core).
#[test]
fn acceptor_callbacks_originator_over_shared_connection() {
    let (mut client, mut server) = bidir_pair(vec![BiDirIiopListenPoint {
        host: "client.local".into(),
        port: 5555,
    }]);

    // Originator hosts a callback object; acceptor a normal object.
    client.register(b"ClientCB", |op, body, e| {
        if op == "notify" {
            SkeletonResult::Reply(u32_body(read_u32(body, e) + 1))
        } else {
            SkeletonResult::BadOperation
        }
    });
    server.register(b"ServerObj", |op, _body, e| {
        if op == "greet" {
            let mut w = BufferWriter::new(e);
            w.write_u32(0xCAFE).unwrap();
            SkeletonResult::Reply(w.into_bytes())
        } else {
            SkeletonResult::BadOperation
        }
    });

    // --- Forward: originator -> acceptor ---
    let id_c = client.invoke_async(b"ServerObj", "greet", &[]).unwrap();
    server.serve_one().unwrap(); // Acceptor dispatches greet + replies
    let (rb, re) = client.collect_reply(id_c).unwrap();
    assert_eq!(read_u32(&rb, re), 0xCAFE);
    assert_eq!(id_c % 2, 0, "originator request_id must be even (§15.8)");

    // The acceptor received the announced listen points (the BiDir SC traveled along).
    assert_eq!(server.peer_listen_points().len(), 1);
    assert_eq!(server.peer_listen_points()[0].host, "client.local");
    assert_eq!(server.peer_listen_points()[0].port, 5555);

    // --- Callback: acceptor -> originator over the SAME connection ---
    let id_s = server
        .invoke_async(b"ClientCB", "notify", &u32_body(41))
        .unwrap();
    client.serve_one().unwrap(); // Originator serves the server callback
    let (cb_b, cb_e) = server.collect_reply(id_s).unwrap();
    assert_eq!(read_u32(&cb_b, cb_e), 42, "callback result");
    assert_eq!(id_s % 2, 1, "acceptor request_id must be odd (§15.8)");
}

/// Out-of-order reply demux: the originator has two requests open; the replies
/// are collected in a different order — the stash correlates by `request_id`.
#[test]
fn out_of_order_reply_demux() {
    let (mut client, mut server) = bidir_pair(vec![]);
    server.register(b"Echo", |_op, body, _e| {
        SkeletonResult::Reply(body.to_vec())
    });

    let id_a = client.invoke_async(b"Echo", "a", &u32_body(10)).unwrap();
    let id_b = client.invoke_async(b"Echo", "b", &u32_body(20)).unwrap();
    server.serve_one().unwrap(); // answers a → Reply#id_a
    server.serve_one().unwrap(); // answers b → Reply#id_b

    // Collect id_b first: collect_reply reads Reply#id_a (≠ target) → stash, then id_b.
    let (b_body, b_e) = client.collect_reply(id_b).unwrap();
    assert_eq!(read_u32(&b_body, b_e), 20);
    // id_a now comes from the stash.
    let (a_body, a_e) = client.collect_reply(id_a).unwrap();
    assert_eq!(read_u32(&a_body, a_e), 10);
}

/// Synchronous `invoke` (send + collect reply in one step), driven by
/// alternating `serve_one` on the other side.
#[test]
fn sync_invoke_convenience() {
    let (mut client, mut server) = bidir_pair(vec![]);
    server.register(b"Sq", |_op, body, e| {
        SkeletonResult::Reply(u32_body(read_u32(body, e).pow(2)))
    });

    let id = client.invoke_async(b"Sq", "square", &u32_body(9)).unwrap();
    server.serve_one().unwrap();
    let (rb, re) = client.collect_reply(id).unwrap();
    assert_eq!(read_u32(&rb, re), 81);
}
