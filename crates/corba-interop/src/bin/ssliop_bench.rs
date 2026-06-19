// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
//! SSLIOP **steady-state** latency benchmark: measures the Echo roundtrip over
//! a TLS connection established ONCE (no handshake per call). Mirrors
//! `echo_bench` (plain IIOP, hand-marshalled, one established connection)
//! exactly — same `invoke_on`/`encode_string_body` path, only TLS transport.
//! The difference between the two p50s = pure TLS transport overhead per
//! roundtrip.
//!
//! Usage: `ssliop_bench <cert.pem> <key.pem> [payload-bytes] [iterations]`
//! (default: 56, 50000).

use std::sync::Arc;
use std::time::Instant;

use zerodds_corba_interop::runtime::CorbaServer;
use zerodds_corba_interop::{decode_string_body, encode_string_body, invoke_on};
use zerodds_corba_rust::CorbaException;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::{Echo, dispatch_echo};

struct EchoImpl;
impl Echo for EchoImpl {
    fn ping(&self, msg: String) -> Result<String, CorbaException> {
        Ok(msg)
    }
}

fn main() {
    let cert_path = std::env::args().nth(1).expect("cert.pem argument missing");
    let key_path = std::env::args().nth(2).expect("key.pem argument missing");
    let payload_len: usize = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(56);
    let n: usize = std::env::args()
        .nth(4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);
    let payload: String = "x".repeat(payload_len);

    let cert = std::fs::read(&cert_path).expect("cert.pem lesen");
    let key = std::fs::read(&key_path).expect("key.pem lesen");
    let server_cfg =
        zerodds_corba_iiop::tls::load_server_config(&cert, &key).expect("TLS-ServerConfig");
    let client_cfg =
        zerodds_corba_iiop::tls::load_client_config_trusting(&cert).expect("TLS-ClientConfig");

    // Server: Echo via serve_tls (codegen dispatch, wire-identical to invoke_on).
    let key_id: &[u8] = b"Echo";
    let server = CorbaServer::new();
    let servant = Arc::new(EchoImpl);
    server.register(key_id, move |op, body, e| {
        dispatch_echo(&*servant, op, body, e)
    });
    let acceptor = server
        .serve_tls("127.0.0.1:0".parse().unwrap(), server_cfg)
        .unwrap();
    let addr = acceptor.listen_addr();

    // Client: ONE TLS connection, then hand-marshalled invoke_on in a loop.
    let mut conn = zerodds_corba_iiop::tls::connect_tls(
        &addr.ip().to_string(),
        addr.port(),
        "localhost",
        client_cfg,
    )
    .expect("connect_tls");

    for i in 0..2000u32 {
        let r = invoke_on(&mut conn, i, key_id, "ping", encode_string_body(&payload)).unwrap();
        assert_eq!(decode_string_body(&r), payload);
    }

    let mut samples: Vec<u64> = Vec::with_capacity(n);
    for i in 0..n {
        let body = encode_string_body(&payload);
        let t0 = Instant::now();
        let r = invoke_on(&mut conn, i as u32, key_id, "ping", body).unwrap();
        samples.push(t0.elapsed().as_nanos() as u64);
        debug_assert_eq!(decode_string_body(&r), payload);
    }
    samples.sort_unstable();
    let us = |q: f64| samples[((n as f64 * q) as usize).min(n - 1)] as f64 / 1000.0;

    println!(
        "ZeroDDS CORBA Echo roundtrip via SSLIOP/TLS, established conn (loopback, payload={payload_len}B, N={n})"
    );
    println!(
        "  min={:.1}us  p50={:.1}us  p90={:.1}us  p99={:.1}us  p99.9={:.1}us",
        samples[0] as f64 / 1000.0,
        us(0.50),
        us(0.90),
        us(0.99),
        us(0.999),
    );
    acceptor.shutdown();
}
