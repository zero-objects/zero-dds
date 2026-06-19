// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! UIOP (Unix domain socket transport, ZeroDDS vendor) — e2e: an Echo over
//! `serve_uds`, an IOR with `TAG_ZERODDS_UDS_TRANS`, which the client recognizes and
//! calls over the UDS transport. Vendor spec: `docs/specs/zerodds-uiop-transport-1.0.md`.

use std::time::Duration;

use zerodds_cdr::Endianness;
use zerodds_corba_interop::runtime::{
    CorbaServer, IiopCorbaConnection, object_reference_from_ior, stringify_object_ref_uds,
};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::CorbaConnection;

fn temp_socket() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "zerodds-uiop-{}-{:p}.sock",
        std::process::id(),
        &() as *const ()
    ))
}

#[test]
fn uiop_echo_roundtrip() {
    let sock = temp_socket();
    let _ = std::fs::remove_file(&sock);

    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            zerodds_corba_rust::SkeletonResult::Reply(body.to_vec())
        } else {
            zerodds_corba_rust::SkeletonResult::BadOperation
        }
    });
    let acceptor = server.serve_uds(&sock).unwrap();

    // Wait until the socket file exists.
    for _ in 0..100 {
        if sock.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(sock.exists(), "UDS socket not bound");

    // IOR with a UDS component; the client recognizes it and uses the UDS transport.
    let ior = stringify_object_ref_uds("IDL:Echo:1.0", sock.to_str().unwrap(), b"Echo");
    let obj = object_reference_from_ior(&ior).unwrap();

    let conn = IiopCorbaConnection::new();
    // Multiple calls over the SAME (pooled) UDS connection.
    for _ in 0..5 {
        let (reply, _e) = conn
            .invoke(&obj, "ping", Endianness::Big, &encode_string_body("hi-uds"))
            .unwrap();
        assert_eq!(decode_string_body(&reply), "hi-uds");
    }

    acceptor.shutdown();
    let _ = std::fs::remove_file(&sock);
}
