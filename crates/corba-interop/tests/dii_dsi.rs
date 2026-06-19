// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! DII live invoke (spec §7) + DSI server bind (spec §12) e2e over real
//! GIOP/IIOP: a DII `Request` builds in-args, calls `invoke` over the
//! `CorbaConnection`, and the reply lands as the return value. DSI: a dynamic
//! servant is registered with a `CorbaServer` via `dispatch_dsi`.

use std::sync::Arc;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_ccm::dynamic_api::{DsiServant, Request, ServerRequest, dispatch_dsi};
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_rust::SkeletonResult;

fn long_bytes(v: i32) -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_u32(v as u32).unwrap();
    w.into_bytes()
}

/// DII against an `add(in long a, in long b) -> long` server: two CDR long
/// in-args, `invoke` over IIOP, the return value decodes to the sum.
#[test]
fn dii_invoke_add_returns_sum() {
    let server = CorbaServer::new();
    server.register(b"Bench", |op, body, e| {
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
        "IDL:Bench:1.0",
        &addr.ip().to_string(),
        addr.port(),
        b"Bench",
    );
    let conn = IiopCorbaConnection::new();

    let mut req = Request::new("add");
    req.add_in_arg("a", long_bytes(2));
    req.add_in_arg("b", long_bytes(3));
    req.invoke(&conn, &ior, Endianness::Big).unwrap();

    let result = req.result.expect("DII result set");
    let mut r = BufferReader::new(&result.value, Endianness::Big);
    assert_eq!(r.read_u32().unwrap() as i32, 5);
    acceptor.shutdown();
}

/// DSI: a dynamic servant (reverses the request body) is registered via
/// `dispatch_dsi`; a DII client calls it → the ServerRequest runs
/// through `dynamic_invoke`, and the reversed body comes back as the reply. Tests
/// both dynamic paths (DII client + DSI server) in one roundtrip.
#[test]
fn dsi_dispatch_via_server_request() {
    struct ReverseDsi;
    impl DsiServant for ReverseDsi {
        fn dynamic_invoke(&self, req: &mut ServerRequest) {
            let mut body = req.input_body();
            body.reverse();
            req.set_result(body);
        }
    }

    let server = CorbaServer::new();
    let dsi = Arc::new(ReverseDsi);
    server.register(b"Dsi", move |op, body, e| dispatch_dsi(&*dsi, op, body, e));
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let ior = object_reference("IDL:Dsi:1.0", &addr.ip().to_string(), addr.port(), b"Dsi");
    let conn = IiopCorbaConnection::new();

    let mut req = Request::new("reverse");
    req.add_in_arg("x", vec![1, 2, 3, 4]);
    req.invoke(&conn, &ior, Endianness::Big).unwrap();

    assert_eq!(req.result.expect("DSI reply").value, vec![4, 3, 2, 1]);
    acceptor.shutdown();
}
