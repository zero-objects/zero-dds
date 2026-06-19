// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! CSIv2 GSSUP authentication (spec §10/§24) e2e: a server requires
//! credentials; a client attaches a SAS EstablishContext
//! (GSSUP username/password) as service context id 15 to every request. Correct
//! credentials → the call goes through; wrong/missing → `NO_PERMISSION`.

use zerodds_cdr::Endianness;
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::{CorbaConnection, SkeletonResult};

#[test]
fn csiv2_gssup_authenticates_and_rejects() {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    // GSSUP validator: only alice/secret.
    server.require_credentials(|user, pass| user == "alice" && pass == "secret");
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo");

    // Correct credentials → the authenticated call goes through.
    let good = IiopCorbaConnection::new().with_csiv2_credentials("alice", "secret");
    let (body, _e) = good
        .invoke(&ior, "ping", Endianness::Big, &encode_string_body("hi"))
        .expect("authenticated call must succeed");
    assert_eq!(decode_string_body(&body), "hi");

    // Wrong password → NO_PERMISSION (Err).
    let bad = IiopCorbaConnection::new().with_csiv2_credentials("alice", "wrong");
    assert!(
        bad.invoke(&ior, "ping", Endianness::Big, &encode_string_body("hi"))
            .is_err(),
        "wrong password must be rejected"
    );

    // No credentials at all → NO_PERMISSION (Err).
    let anon = IiopCorbaConnection::new();
    assert!(
        anon.invoke(&ior, "ping", Endianness::Big, &encode_string_body("hi"))
            .is_err(),
        "missing security context must be rejected"
    );

    acceptor.shutdown();
}

/// Without `require_credentials` the server stays open (CSIv2 is opt-in) —
/// regression guard against accidental mandatory auth.
#[test]
fn csiv2_is_opt_in_server_open_by_default() {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo");

    let plain = IiopCorbaConnection::new();
    let (body, _e) = plain
        .invoke(&ior, "ping", Endianness::Big, &encode_string_body("hi"))
        .expect("open server accepts unauthenticated call");
    assert_eq!(decode_string_body(&body), "hi");
    acceptor.shutdown();
}
