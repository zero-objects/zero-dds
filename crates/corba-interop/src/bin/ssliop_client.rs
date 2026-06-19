// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]
//! SSLIOP interop client: calls the Echo stub over **TLS** via an SSLIOP IOR
//! (produced by ZeroDDS or a foreign ORB). The client detects the
//! `TAG_SSL_SEC_TRANS` component in the target IOR and automatically selects
//! the TLS transport to the advertised SSL port.
//!
//! Usage: `ssliop_client <SSLIOP_IOR> <ca.pem>`
//! Exit code 0 = roundtrip green, otherwise panic (interop error).

use std::sync::Arc;

use zerodds_corba_interop::runtime::{IiopCorbaConnection, object_reference_from_ior};
use zerodds_corba_rust::CorbaConnection;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::{Echo, EchoStub};

fn main() {
    let ior = std::env::args().nth(1).expect("IOR argument missing");
    let ca_path = std::env::args().nth(2).expect("ca.pem argument missing");
    let ca = std::fs::read(&ca_path).expect("ca.pem lesen");
    let obj = object_reference_from_ior(&ior).expect("IOR parse");

    // SSLIOP client: trusts the server cert (self-signed = own root), SNI
    // "localhost" must match the cert SAN.
    let conn: Arc<dyn CorbaConnection + Send + Sync> =
        Arc::new(IiopCorbaConnection::with_client_tls(&ca, "localhost").expect("TLS-ClientConfig"));
    let stub = EchoStub::new(obj, conn);

    let r = stub
        .ping("zerodds-ssliop".to_string())
        .expect("ping over TLS");
    assert_eq!(r, "zerodds-ssliop", "SSLIOP echo mismatch");
    let big = "x".repeat(4096);
    assert_eq!(
        stub.ping(big.clone()).expect("ping 4k over TLS"),
        big,
        "SSLIOP 4k mismatch"
    );
    println!("OK ssliop: echo ping roundtrip over TLS (small + 4k)");
}
