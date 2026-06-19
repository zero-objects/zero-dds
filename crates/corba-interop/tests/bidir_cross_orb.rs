// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! Live bidirectional-GIOP cross-ORB (§15.8): as the **originator**, ZeroDDS
//! opens a BiDir connection to a JacORB `Server`, registers a ZeroDDS callback
//! object (as a marshalled IOR reference) and calls `callback_hello` — the
//! JacORB server then calls `hello()` BACK to ZeroDDS' callback over the SAME
//! connection. ZeroDDS' `BiDirEndpoint::collect_reply` serves the incoming
//! callback reentrantly. Proves the full BiDir connection-reuse flow against a
//! foreign ORB.
//!
//! Runs only with a live JacORB bidir server (IOR via `BIDIR_SERVER_IOR`, on
//! the Linux test host).

use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_iiop::{Connection, IiopProfileBody};
use zerodds_corba_interop::runtime::{BiDirEndpoint, object_reference_from_ior};
use zerodds_corba_ior::{Ior, TaggedProfile};
use zerodds_corba_rust::SkeletonResult;

const CLIENT_CB_TYPE: &str = "IDL:org/jacorb/demo/bidir/ClientCallback:1.0";

fn cdr_string(s: &str) -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Big);
    w.write_string(s).unwrap();
    w.into_bytes()
}

#[test]
#[ignore = "needs live JacORB bidir Server via BIDIR_SERVER_IOR env (codepit)"]
fn jacorb_server_callbacks_zerodds_over_shared_connection() {
    let ior = std::env::var("BIDIR_SERVER_IOR").expect("BIDIR_SERVER_IOR env");
    let oref = object_reference_from_ior(ior.trim()).expect("parse IOR");
    let prof = IiopProfileBody::decode_encapsulation(&oref.iiop_profile).expect("iiop profile");

    // Open the BiDir connection to the JacORB server (originator) + advertise a
    // listen point (must match the callback IOR so JacORB reuses the connection).
    // Force IPv4 127.0.0.1 so connection peer + listen-point host are consistent
    // (otherwise the connection may run over IPv6 and JacORB's BiDir correlation
    // won't match the IPv4 listen point).
    let stream = TcpStream::connect(("127.0.0.1", prof.port)).unwrap();
    let conn = Connection::from_stream(stream).unwrap();
    let listen_host = "127.0.0.1";
    let listen_port: u16 = 5567;
    let mut ep = BiDirEndpoint::originator(
        conn,
        vec![zerodds_corba_iiop::BiDirIiopListenPoint {
            host: listen_host.into(),
            port: listen_port,
        }],
    );

    // ZeroDDS hosts the callback object "ZeroCB" and remembers the message.
    let got = Arc::new(Mutex::new(None::<String>));
    let sink = Arc::clone(&got);
    ep.register(b"ZeroCB", move |op, body, e| {
        if op == "hello" {
            let mut r = BufferReader::new(body, e);
            *sink.lock().unwrap() = Some(r.read_string().unwrap());
            SkeletonResult::Reply(Vec::new()) // void
        } else {
            SkeletonResult::BadOperation
        }
    });

    // Build the callback IOR (ClientCallback @ our listen point, key "ZeroCB").
    let cb_profile = IiopProfileBody {
        host: listen_host.into(),
        port: listen_port,
        object_key: b"ZeroCB".to_vec(),
        ..prof.clone()
    };
    let cb_ior = Ior {
        type_id: CLIENT_CB_TYPE.into(),
        profiles: vec![TaggedProfile::iiop(&cb_profile, Endianness::Big).unwrap()],
    };
    let mut iw = BufferWriter::new(Endianness::Big);
    cb_ior.encode(&mut iw).unwrap();
    let cb_ior_bytes = iw.into_bytes();

    let server_key = &prof.object_key;

    // 1. register_callback(callback) — JacORB remembers the ref.
    ep.invoke(server_key, "register_callback", &cb_ior_bytes)
        .expect("register_callback");

    // 2. callback_hello(msg) — JacORB then calls ccb.hello(msg) BACK over the
    //    same connection; collect_reply serves the callback reentrantly.
    ep.invoke(server_key, "callback_hello", &cdr_string("Hi from ZeroDDS"))
        .expect("callback_hello");

    assert_eq!(
        got.lock().unwrap().as_deref(),
        Some("Hi from ZeroDDS"),
        "JacORB server did not call ZeroDDS' callback over the shared connection"
    );
    eprintln!(
        "cross-ORB BiDir OK: JacORB called the ZeroDDS callback hello() over the shared connection"
    );
}
