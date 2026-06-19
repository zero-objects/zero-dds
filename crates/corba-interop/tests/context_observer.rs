// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Default-runnable coverage for `CorbaServer::on_request_contexts`: the server
// observes the incoming ServiceContextList of every request before dispatch.
// (The live OTS use — capturing a JacORB PropagationContext in SC id=0 — is in
// `jacorb_live_ots_handshake`, codepit-gated.)

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Default-runnable coverage for `CorbaServer::on_request_contexts`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use zerodds_cdr::Endianness;
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::{CorbaConnection, SkeletonResult};

#[test]
fn server_observes_incoming_request_contexts() {
    let observed = Arc::new(AtomicU32::new(0));

    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    {
        let obs = Arc::clone(&observed);
        // The observer fires once per request (inspection only — dispatch proceeds).
        server.on_request_contexts(move |_ctxs| {
            obs.fetch_add(1, Ordering::SeqCst);
        });
    }
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo");

    let conn = IiopCorbaConnection::new();
    let (body, _e) = conn
        .invoke(&ior, "ping", Endianness::Big, &encode_string_body("hi"))
        .expect("call succeeds despite the observer");
    assert_eq!(decode_string_body(&body), "hi");

    acceptor.shutdown();
    assert!(
        observed.load(Ordering::SeqCst) >= 1,
        "the context observer must fire for the request"
    );
}
