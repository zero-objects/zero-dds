// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The listener against Apache Qpid Proton (python `qpid-proton`) as the
//! AMQP 1.0 client: connection, session, links, credit, settlement,
//! outcomes, large messages, a dynamic node, the idle time-out and a
//! refused attach (OASIS AMQP 1.0 Part 2 §2.4 to §2.6, Part 3 §3.4, §3.5).
//! No ZeroDDS code is on the client side.
//!
//! The tests need python3 with qpid-proton (`ZERODDS_AMQP_PYTHON` names
//! another interpreter); they run in the integration pillar's job
//! `integration-bridges-vendor-amqp`, where a missing program fails them:
//!
//! ```bash
//! cargo nextest run -p amqp-dds-endpoint --test proton_e2e --run-ignored all
//! ```

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

mod common;

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use amqp_dds_endpoint::nodes::Nodes;
use common::{TestServer, test_handler_cfg};

/// The Python with qpid-proton: `python3`, or the interpreter
/// `ZERODDS_AMQP_PYTHON` names (as the RabbitMQ tests of
/// `zerodds-amqp-endpoint` take it).
fn python() -> std::ffi::OsString {
    std::env::var_os("ZERODDS_AMQP_PYTHON").unwrap_or_else(|| "python3".into())
}

/// Runs `script` with qpid-proton against the listener on `port`; the
/// script has `c`, a `BlockingConnection` to it, and prints its findings.
/// Returns the lines it printed.
fn proton(port: u16, connect_args: &str, script: &str) -> Vec<String> {
    let program = format!(
        "import sys\n\
         from proton import Message, Delivery\n\
         from proton.utils import BlockingConnection, LinkDetached, ConnectionClosed\n\
         c = BlockingConnection('amqp://127.0.0.1:{port}', timeout=10{connect_args})\n\
         {script}\n\
         c.close()\n"
    );
    let out = Command::new(python())
        .arg("-c")
        .arg(&program)
        .output()
        .expect("python3 is needed for this test");
    assert!(
        out.status.success(),
        "qpid-proton failed: {}\n--- script ---\n{program}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// Part 2 §2.6: Proton opens, begins, attaches a sender and a receiver,
/// sends three messages (each send returns on the listener's accepted),
/// receives them in order within its credit and accepts them; a drain on
/// the empty node then consumes the credit (§2.6.7, §2.6.8).
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let lines = proton(
        server.port,
        "",
        "s = c.create_sender('orders')\n\
         for i in range(3):\n\
         \x20   d = s.send(Message(body='order-%d' % i))\n\
         \x20   print('sent', d.remote_state == Delivery.ACCEPTED, d.settled)\n\
         r = c.create_receiver('orders')\n\
         print('source', r.link.remote_source.address, r.link.remote_source.distribution_mode)\n\
         for i in range(3):\n\
         \x20   m = r.receive(timeout=5)\n\
         \x20   print('got', m.body)\n\
         \x20   r.accept()\n\
         r.link.drain(7)\n\
         c.wait(lambda: r.link.credit == 0, timeout=5, msg='the drain is answered')\n\
         print('drained', r.link.credit, r.link.queued)",
    );
    assert_eq!(
        lines,
        [
            "sent True True",
            "sent True True",
            "sent True True",
            // distribution-mode move is 2 in Proton (Terminus.DIST_MODE_MOVE).
            "source orders 2",
            "got order-0",
            "got order-1",
            "got order-2",
            "drained 0 0",
        ]
    );
    // Accepted by Proton: the node is empty.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while nodes.depth("orders") != Some(0) {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            nodes.depth("orders")
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    server.shutdown();
}

/// Part 2 §2.6.14: a message of 300 000 octets against a listener that
/// takes frames of 4096 octets: Proton splits it into transfer frames, the
/// listener reassembles it and sends it back in frames no larger than
/// Proton's max-frame-size; the octets are the same.
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_2_6_14_proton_sends_a_message_above_the_frame_size() {
    let mut cfg = test_handler_cfg();
    cfg.max_frame_size = 4096;
    let server = TestServer::spawn(cfg);
    let lines = proton(
        server.port,
        ", max_frame_size=2048",
        "import hashlib\n\
         body = bytes((i * 7) % 251 for i in range(300000))\n\
         s = c.create_sender('big')\n\
         d = s.send(Message(body=body))\n\
         print('sent', d.remote_state == Delivery.ACCEPTED)\n\
         r = c.create_receiver('big')\n\
         m = r.receive(timeout=10)\n\
         r.accept()\n\
         print('same', bytes(m.body) == body, len(m.body))",
    );
    assert_eq!(lines, ["sent True", "same True 300000"]);
    server.shutdown();
}

/// Part 3 §3.4.3 to §3.4.5: Proton releases a message and gets it again;
/// modifies it with delivery-failed (Proton's `release(delivered=True)` is
/// `modified`; the script sets `failed` on the delivery) and gets it with
/// delivery-count 1; rejects it and gets nothing more.
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_3_4_proton_releases_modifies_and_rejects_against_the_endpoint() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let lines = proton(
        server.port,
        "",
        "s = c.create_sender('jobs')\n\
         s.send(Message(body='job'))\n\
         r = c.create_receiver('jobs')\n\
         m = r.receive(timeout=5)\n\
         print('first', m.body, m.delivery_count)\n\
         r.release(delivered=False)\n\
         m = r.receive(timeout=5)\n\
         print('released', m.body, m.delivery_count)\n\
         r.fetcher.unsettled[0].local.failed = True\n\
         r.release(delivered=True)\n\
         m = r.receive(timeout=5)\n\
         print('modified', m.body, m.delivery_count)\n\
         r.reject()\n\
         r.link.drain(1)\n\
         c.wait(lambda: r.link.credit == 0, timeout=5, msg='the drain is answered')\n\
         print('rejected', r.link.queued)",
    );
    assert_eq!(
        lines,
        [
            "first job 0",
            "released job 0",
            "modified job 1",
            "rejected 0"
        ]
    );
    assert_eq!(nodes.depth("jobs"), Some(0));
    server.shutdown();
}

/// Part 3 §3.5.3 `dynamic`: Proton asks for a dynamic source, the listener
/// creates a node and names it in its attach; a message sent to that
/// address arrives on the link.
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_3_5_3_proton_gets_a_dynamic_node_from_the_endpoint() {
    let server = TestServer::spawn(test_handler_cfg());
    let lines = proton(
        server.port,
        "",
        "r = c.create_receiver(None, dynamic=True, name='replies')\n\
         address = r.link.remote_source.address\n\
         print('dynamic', r.link.remote_source.dynamic, 'replies' in address)\n\
         s = c.create_sender(address)\n\
         s.send(Message(body='reply'))\n\
         print('got', r.receive(timeout=5).body)\n\
         r.accept()",
    );
    assert_eq!(lines, ["dynamic True True", "got reply"]);
    server.shutdown();
}

/// Part 2 §2.4.5: Proton with an idle time-out of one second stays
/// connected for three seconds without traffic of its own, because the
/// listener sends empty frames; a send works afterwards.
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_2_4_5_proton_with_an_idle_time_out_is_kept_alive_by_the_endpoint() {
    let server = TestServer::spawn(test_handler_cfg());
    let lines = proton(
        server.port,
        ", heartbeat=1",
        "from proton import Timeout\n\
         s = c.create_sender('later')\n\
         try:\n\
         \x20   c.wait(lambda: False, timeout=3)\n\
         except Timeout:\n\
         \x20   pass\n\
         d = s.send(Message(body='still here'))\n\
         print('sent', d.remote_state == Delivery.ACCEPTED)",
    );
    assert_eq!(lines, ["sent True"]);
    server.shutdown();
}

/// Part 2 §2.6.3 with §2.8.15: an attach to an address without a node is
/// refused; Proton sees the link detached with amqp:not-found, and the
/// connection stays usable.
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_2_6_3_proton_sees_a_refused_attach_as_not_found() {
    let mut cfg = test_handler_cfg();
    cfg.nodes = Arc::new(Nodes::new(false));
    cfg.nodes.declare("there");
    let server = TestServer::spawn(cfg);
    let lines = proton(
        server.port,
        "",
        "try:\n\
         \x20   c.create_sender('missing')\n\
         \x20   print('attached')\n\
         except LinkDetached as e:\n\
         \x20   print('detached', e.condition)\n\
         try:\n\
         \x20   c.create_receiver('missing')\n\
         \x20   print('attached')\n\
         except LinkDetached as e:\n\
         \x20   print('detached', e.condition)\n\
         s = c.create_sender('there')\n\
         d = s.send(Message(body='x'))\n\
         print('sent', d.remote_state == Delivery.ACCEPTED)",
    );
    assert_eq!(
        lines,
        [
            "detached amqp:not-found",
            "detached amqp:not-found",
            "sent True"
        ]
    );
    server.shutdown();
}
