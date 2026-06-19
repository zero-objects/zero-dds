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
//! Self-roundtrip latency benchmark over the **generated** CORBA codegen.
//!
//! Unlike `echo_bench` (hand-marshalled `serve`/`invoke_on`), this benchmark
//! measures the full codegen path:
//!   generated `EchoStub::ping` → CDR encode → IIOP/GIOP wire →
//!   `dispatch_echo` skeleton → servant → reply wire → stub CDR decode.
//!
//! This is the legitimate comparison baseline against omniORB/TAO/JacORB, which
//! likewise route their calls through generated stubs/skeletons.
//!
//! Usage: `echo_bench_codegen [payload-bytes] [iterations]` (default: 56, 50000).

use std::sync::Arc;
use std::time::Instant;

use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_rust::{CorbaConnection, CorbaException};

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::{Echo, EchoStub, dispatch_echo};

struct EchoImpl;
impl Echo for EchoImpl {
    fn ping(&self, msg: String) -> Result<String, CorbaException> {
        Ok(msg)
    }
}

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

    let key: &[u8] = b"Echo";
    let server = CorbaServer::new();
    let servant = Arc::new(EchoImpl);
    server.register(key, move |op, body, e| {
        dispatch_echo(&*servant, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let conn: Arc<dyn CorbaConnection + Send + Sync> = Arc::new(IiopCorbaConnection::new());
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), key);
    let stub = EchoStub::new(ior, conn);

    // Warmup (TCP path, caches).
    for _ in 0..2000u32 {
        assert_eq!(stub.ping(payload.clone()).unwrap(), payload);
    }

    let mut samples: Vec<u64> = Vec::with_capacity(n);
    for _ in 0..n {
        let t0 = Instant::now();
        let r = stub.ping(payload.clone()).unwrap();
        samples.push(t0.elapsed().as_nanos() as u64);
        debug_assert_eq!(r, payload);
    }
    samples.sort_unstable();
    let us = |q: f64| samples[((n as f64 * q) as usize).min(n - 1)] as f64 / 1000.0;

    println!(
        "ZeroDDS CORBA Echo roundtrip via CODEGEN (IIOP loopback, payload={payload_len}B, N={n})"
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
