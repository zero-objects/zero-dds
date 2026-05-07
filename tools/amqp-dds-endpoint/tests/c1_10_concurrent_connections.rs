#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.10 — Concurrent Connections.
//!
//! Spec §C.1.10: der Endpoint SHALL mindestens
//! `max_connections` parallele Connections akzeptieren.

mod common;

use amqp_dds_endpoint::client::{ClientConfig, ReconnectConfig, connect_with_reconnect};
use common::{TestServer, test_handler_cfg};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;
use zerodds_amqp_endpoint::MetricsHub;

#[test]
fn c1_10_server_accepts_multiple_concurrent_connections() {
    let server = TestServer::spawn(test_handler_cfg());
    let port = server.port;
    let metrics = server.metrics.clone();

    const N: usize = 8;
    let mut handles = Vec::new();
    for i in 0..N {
        handles.push(thread::spawn(move || {
            let cfg = ClientConfig {
                upstream_addr: format!("127.0.0.1:{port}"),
                container_id: format!("c1-10-client-{i}"),
                max_frame_size: 65_536,
                tls_active: false,
                plain_credentials: None,
                io_timeout: Some(Duration::from_secs(3)),
            };
            let m = Arc::new(MetricsHub::new());
            let s = Arc::new(AtomicBool::new(false));
            connect_with_reconnect(
                &cfg,
                &ReconnectConfig {
                    max_attempts: Some(2),
                    ..ReconnectConfig::default()
                },
                &s,
                &m,
            )
        }));
    }

    let mut succeeded = 0;
    for h in handles {
        match h.join().expect("thread join") {
            Ok(_) => succeeded += 1,
            Err(e) => eprintln!("client connect failed: {e}"),
        }
    }
    assert_eq!(succeeded, N, "expected {N} connections, got {succeeded}");

    // Server-Side hat alle N gezaehlt.
    thread::sleep(Duration::from_millis(100));
    let total = metrics.snapshot("connections.total").unwrap_or(0);
    assert!(
        total >= N as i64,
        "expected ≥{N} total connections, got {total}"
    );

    server.shutdown();
}
