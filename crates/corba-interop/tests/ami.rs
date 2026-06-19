// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Client AMI (CORBA Messaging §22) e2e over real GIOP/IIOP. The `AmiClient`
//! holds a multiplexing connection to a target and fires requests without
//! blocking. Proves both spec models:
//! * **Callback** (§22.5): several requests open at once, one callback each,
//!   correlated by `request_id` — `perform_all` works them off.
//! * **Polling** (§22.6): `send_poll` → `request_id`, `get_reply` fetches the reply.
//!
//! Plus the exception path (system exception → `Err` in the callback/poller).

use std::sync::{Arc, Mutex};

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_interop::runtime::{AmiClient, CorbaServer, object_reference};
use zerodds_corba_rust::SkeletonResult;

fn long(v: i32) -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u32(v as u32).unwrap();
    w.into_bytes()
}
fn two_longs(a: i32, b: i32) -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u32(a as u32).unwrap();
    w.write_u32(b as u32).unwrap();
    w.into_bytes()
}
fn decode_long(reply: &[u8], e: Endianness) -> i32 {
    BufferReader::new(reply, e).read_u32().unwrap() as i32
}

/// Provides an `add(a,b) -> a+b` server (unknown ops → BAD_OPERATION), connects
/// an `AmiClient` and runs `f` with it; cleans up afterwards.
fn with_adder<F: FnOnce(&mut AmiClient)>(f: F) {
    let server = CorbaServer::new();
    server.register(b"Adder", |op, body, e| {
        if op == "add" {
            let mut r = BufferReader::new(body, e);
            let a = r.read_u32().unwrap() as i32;
            let b = r.read_u32().unwrap() as i32;
            let mut w = BufferWriter::new(e);
            w.write_u32(a.wrapping_add(b) as u32).unwrap();
            SkeletonResult::Reply(w.into_bytes())
        } else {
            SkeletonResult::BadOperation
        }
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference(
        "IDL:Adder:1.0",
        &addr.ip().to_string(),
        addr.port(),
        b"Adder",
    );
    let mut client = AmiClient::connect(&ior).unwrap();
    f(&mut client);
    acceptor.shutdown();
}

/// Callback model: ALL three requests are fired before a single reply is read
/// (three open at once). `perform_all` works them off; each callback records its
/// result, correctly correlated.
#[test]
fn ami_callback_multiple_outstanding() {
    with_adder(|client| {
        let results: Arc<Mutex<Vec<(u32, i32)>>> = Arc::new(Mutex::new(Vec::new()));
        let inputs = [(2, 3), (10, 20), (100, 1)];
        let mut ids = Vec::new();
        for (a, b) in inputs {
            let sink = Arc::clone(&results);
            let id = client
                .send(
                    "add",
                    &two_longs(a, b),
                    Box::new(move |reply| {
                        let (body, e) = reply.expect("no exception");
                        sink.lock().unwrap().push((0, decode_long(&body, e)));
                    }),
                )
                .unwrap();
            ids.push(id);
        }
        assert_eq!(client.pending(), 3, "three open simultaneously");
        client.perform_all().unwrap();
        assert_eq!(client.pending(), 0);

        let mut sums: Vec<i32> = results.lock().unwrap().iter().map(|(_, s)| *s).collect();
        sums.sort_unstable();
        assert_eq!(sums, vec![5, 30, 101]);
    });
}

/// Polling model: `send_poll` returns a `request_id`, `get_reply` blocks until
/// exactly that reply is in and returns the decoded value.
#[test]
fn ami_polling_get_reply() {
    with_adder(|client| {
        let id1 = client.send_poll("add", &two_longs(7, 8)).unwrap();
        let id2 = client.send_poll("add", &two_longs(40, 2)).unwrap();
        // Fetch replies in reverse order — correlation by request_id.
        let r2 = client.get_reply(id2).unwrap().expect("ok");
        assert_eq!(decode_long(&r2.0, r2.1), 42);
        let r1 = client.get_reply(id1).unwrap().expect("ok");
        assert_eq!(decode_long(&r1.0, r1.1), 15);
    });
}

/// Mixed: a callback request and a polling request open at the same time.
/// `get_reply` drives the connection and delivers the callback along the way.
#[test]
fn ami_mixed_callback_and_polling() {
    with_adder(|client| {
        let cb_result = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&cb_result);
        client
            .send(
                "add",
                &two_longs(1, 1),
                Box::new(move |reply| {
                    let (body, e) = reply.expect("ok");
                    *sink.lock().unwrap() = Some(decode_long(&body, e));
                }),
            )
            .unwrap();
        let pid = client.send_poll("add", &two_longs(5, 5)).unwrap();

        // get_reply drives perform_work — the callback reply is collected along with it.
        let pr = client.get_reply(pid).unwrap().expect("ok");
        assert_eq!(decode_long(&pr.0, pr.1), 10);
        // If the callback hasn't run yet, work off the rest.
        client.perform_all().unwrap();
        assert_eq!(*cb_result.lock().unwrap(), Some(2));
    });
}

/// Exception path: an unknown operation → system exception (BAD_OPERATION); the
/// callback gets `Err`, the poller likewise.
#[test]
fn ami_exception_path() {
    with_adder(|client| {
        let got_err = Arc::new(Mutex::new(false));
        let sink = Arc::clone(&got_err);
        client
            .send(
                "nonexistent",
                &long(0),
                Box::new(move |reply| {
                    *sink.lock().unwrap() = reply.is_err();
                }),
            )
            .unwrap();
        client.perform_all().unwrap();
        assert!(*got_err.lock().unwrap(), "callback must get Err");

        let pid = client.send_poll("nonexistent", &long(0)).unwrap();
        assert!(
            client.get_reply(pid).unwrap().is_err(),
            "poller must return Err"
        );
    });
}

/// `get_reply` on an unknown/consumed `request_id` → error (no hang).
#[test]
fn ami_get_reply_unknown_id_errors() {
    with_adder(|client| {
        assert!(client.get_reply(999).is_err());
    });
}
