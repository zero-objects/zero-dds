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
//! Self-roundtrip latency benchmark for the ZeroDDS CORBA IIOP path.
//!
//! Usage: `echo_bench [payload-bytes] [iterations]` (defaults: 56, 50000).
//! Measures the full client-encode → wire → POA-dispatch → servant →
//! wire → client-decode roundtrip over a TCP loopback connection.

use std::time::Instant;

use zerodds_corba_iiop::{Connector, ConnectorConfig};
use zerodds_corba_interop::{decode_string_body, echo_poa, encode_string_body, invoke_on, serve};

fn main() {
    let payload_len: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(56);
    let n: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);
    let payload: String = "x".repeat(payload_len);

    let key = b"Echo";
    let poa = echo_poa(key);
    let acceptor = serve("127.0.0.1:0".parse().unwrap(), poa).unwrap();
    let addr = acceptor.listen_addr();

    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect(&addr.ip().to_string(), addr.port())
        .unwrap();
    let conn = pooled.connection().unwrap();

    // Warmup (JIT TCP path, fill caches).
    for i in 0..2000u32 {
        let r = invoke_on(conn, i, key, "ping", encode_string_body(&payload)).unwrap();
        assert_eq!(decode_string_body(&r), payload);
    }

    let mut samples: Vec<u64> = Vec::with_capacity(n);
    for i in 0..n {
        let body = encode_string_body(&payload);
        let t0 = Instant::now();
        let r = invoke_on(conn, i as u32, key, "ping", body).unwrap();
        samples.push(t0.elapsed().as_nanos() as u64);
        debug_assert_eq!(decode_string_body(&r), payload);
    }
    samples.sort_unstable();
    let us = |q: f64| samples[((n as f64 * q) as usize).min(n - 1)] as f64 / 1000.0;

    println!("ZeroDDS CORBA Echo roundtrip (IIOP loopback, payload={payload_len}B, N={n})");
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
