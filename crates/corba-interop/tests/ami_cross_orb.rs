// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! Cross-ORB client AMI (CORBA Messaging §22): the ZeroDDS `AmiClient` fires
//! **asynchronously** against a FOREIGN bench server (JacORB/omniORB/TAO). AMI
//! is purely client-side — the server sees normal GIOP requests; this test
//! proves the async client speaks byte-identical standard requests that a
//! foreign ORB answers, and that the reply correlation (request_id) holds over
//! the foreign connection.
//!
//! Runs only with a live bench server whose stringified IOR is passed via
//! `BENCH_IOR` (see `competitors/run_interop.sh` / the Linux test host).
//! Ignored by default.

use std::sync::{Arc, Mutex};

use zerodds_cdr::{BufferReader, BufferWriter, CdrEncode, Endianness};
use zerodds_corba_interop::runtime::{AmiClient, object_reference_from_ior};

fn two_longs(a: i32, b: i32) -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Big);
    a.encode(&mut w).unwrap();
    b.encode(&mut w).unwrap();
    w.into_bytes()
}

#[test]
#[ignore = "needs live foreign Bench server via BENCH_IOR env (codepit)"]
fn ami_cross_orb_against_foreign_bench() {
    let ior = std::env::var("BENCH_IOR").expect("BENCH_IOR env (stringified Bench-IOR)");
    let oref = object_reference_from_ior(ior.trim()).expect("parse IOR");
    let mut client = AmiClient::connect(&oref).expect("connect foreign Bench");

    // --- Callback model against foreign ORB: add(7,5) -> 12 ---
    let got = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&got);
    client
        .send(
            "add",
            &two_longs(7, 5),
            Box::new(move |reply| {
                let (body, e) = reply.expect("foreign add reply");
                *sink.lock().unwrap() =
                    Some(BufferReader::new(&body, e).read_u32().unwrap() as i32);
            }),
        )
        .unwrap();
    client.perform_all().unwrap();
    assert_eq!(*got.lock().unwrap(), Some(12), "cross-ORB AMI callback add");

    // --- Polling model against foreign ORB: divmod(17,5) -> (3,2) (out,out) ---
    let id = client.send_poll("divmod", &two_longs(17, 5)).unwrap();
    let (body, e) = client.get_reply(id).unwrap().expect("foreign divmod reply");
    let mut r = BufferReader::new(&body, e);
    let q = r.read_u32().unwrap() as i32;
    let rem = r.read_u32().unwrap() as i32;
    assert_eq!((q, rem), (3, 2), "cross-ORB AMI polling divmod");

    eprintln!("cross-ORB AMI OK: add(7,5)=12, divmod(17,5)=(3,2)");
}
