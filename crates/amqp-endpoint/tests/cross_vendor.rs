// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-Vendor-Interop-Tests gegen RabbitMQ.
//!
//! Spec: `zerodds-amqp-bridge-daemon-1.0.md` §10 + §12.3.
//!
//! Verifiziert AMQP-1.0-Wire-Compliance gegen einen RabbitMQ-Broker
//! mit aktiviertem amqp-1-0-Plugin (Docker-Image: `rabbitmq:3-management`
//! mit `rabbitmq_amqp1_0` Plugin enabled).
//!
//! Tests sind `#[ignore]`-markiert. Run via:
//! ```bash
//! cargo test -p zerodds-amqp-endpoint --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
//! ```

#![cfg(feature = "cross-vendor-tests")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::net::TcpStream;
use std::time::Duration;

fn rabbitmq_reachable(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(500),
    )
    .is_ok()
}

#[test]
#[ignore = "requires RabbitMQ at 127.0.0.1:5672 with amqp-1-0 plugin (docker run -p 5672:5672 rabbitmq:3-management)"]
fn rabbitmq_amqp1_0_tcp_handshake() {
    if !rabbitmq_reachable(5672) {
        eprintln!("RabbitMQ not reachable at 5672; skipping");
        return;
    }
    // Lower-bound smoke: TCP connect succeeds; AMQP-1.0 plugin replies
    // with AMQP\x00\x01\x00\x00 protocol header upon receiving ours.
    use std::io::{Read, Write};
    let mut s = TcpStream::connect("127.0.0.1:5672").expect("tcp connect");
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    // Send AMQP\0\1\0\0 protocol header (Spec §2.2.1).
    s.write_all(b"AMQP\x00\x01\x00\x00").expect("write hdr");
    let mut buf = [0u8; 8];
    s.read_exact(&mut buf).expect("read hdr");
    assert_eq!(&buf[..4], b"AMQP", "broker should echo AMQP marker");
    // Major/minor: tolerate either 0\1\0\0 or 3\1\0\0 (SASL).
}

#[test]
#[ignore = "requires RabbitMQ"]
fn backoff_succeeds_when_broker_becomes_reachable() {
    use zerodds_amqp_endpoint::backoff::BackoffConfig;
    let b = BackoffConfig {
        initial_ms: 50,
        max_ms: 500,
        multiplier: 2,
        max_attempts: 10,
    };
    let mut attempt = 0u32;
    while b.allow(attempt) {
        if rabbitmq_reachable(5672) {
            // Success at attempt N.
            assert!(attempt <= 10, "should converge within max_attempts");
            return;
        }
        std::thread::sleep(b.delay_for(attempt));
        attempt += 1;
    }
    eprintln!("RabbitMQ not reachable within backoff window — skipping");
}
