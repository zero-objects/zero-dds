// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The listener as the transactional resource of OASIS AMQP 1.0 Part 4
//! against Apache Qpid Proton (python `qpid-proton`) as the transaction
//! controller: Proton's `Container.declare_transaction` attaches to the
//! coordinator, declares, posts and retires in transactions, commits and
//! aborts. No ZeroDDS code is on the controller side.
//!
//! The tests need python3 with qpid-proton (`ZERODDS_AMQP_PYTHON` names
//! another interpreter); they run in the integration pillar's job
//! `integration-bridges-vendor-amqp`, where a missing program fails them:
//!
//! ```bash
//! cargo nextest run -p amqp-dds-endpoint --test proton_txn_e2e --run-ignored all
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
use zerodds_amqp_endpoint::NodeLimits;

/// The Python with qpid-proton: `python3`, or the interpreter
/// `ZERODDS_AMQP_PYTHON` names.
fn python() -> std::ffi::OsString {
    std::env::var_os("ZERODDS_AMQP_PYTHON").unwrap_or_else(|| "python3".into())
}

/// Runs a Proton reactor program against the listener on `port`:
/// `handler` is the body of a class `Controller(MessagingHandler,
/// TransactionHandler)` whose `on_start` finds the connection in
/// `self.conn`; `self.finish()` ends the program. Returns the lines it
/// printed. A program that does not end within 20 s prints `timeout`.
fn proton(port: u16, handler: &str) -> Vec<String> {
    let program = format!(
        "import sys\n\
         from proton import Message, Delivery\n\
         from proton.handlers import MessagingHandler, TransactionHandler\n\
         from proton.reactor import Container\n\
         class Controller(MessagingHandler, TransactionHandler):\n\
         \x20   def __init__(self):\n\
         \x20       super(Controller, self).__init__(auto_accept=False)\n\
         \x20   def on_start(self, event):\n\
         \x20       self.container = event.container\n\
         \x20       self.timer = self.container.schedule(20, self)\n\
         \x20       self.conn = self.container.connect('amqp://127.0.0.1:{port}')\n\
         \x20       self.begin()\n\
         \x20   def on_timer_task(self, event):\n\
         \x20       print('timeout')\n\
         \x20       self.conn.close()\n\
         \x20   def finish(self):\n\
         \x20       self.timer.cancel()\n\
         \x20       self.conn.close()\n\
         \x20   def on_transaction_declare_failed(self, event):\n\
         \x20       print('declare failed')\n\
         \x20       self.finish()\n\
         \x20   def on_transaction_commit_failed(self, event):\n\
         \x20       print('commit failed')\n\
         \x20       self.finish()\n\
         \x20   def on_link_error(self, event):\n\
         \x20       print('link error', event.link.name, event.link.remote_condition.name)\n\
         \x20       self.finish()\n\
         {handler}\n\
         Container(Controller()).run()\n"
    );
    let out = Command::new(python())
        .arg("-c")
        .arg(&program)
        .output()
        .expect("python3 is needed for this test");
    assert!(
        out.status.success(),
        "qpid-proton failed: {}\n--- program ---\n{program}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

fn wait_depth(nodes: &Nodes, address: &str, n: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while nodes.depth(address) != Some(n) {
        assert!(
            std::time::Instant::now() < deadline,
            "node {address} holds {:?}, expected {n}",
            nodes.depth(address)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Part 4 §4.2 to §4.4.2 with Proton as the controller. Proton attaches a
/// sender to a coordinator target (one capability symbol), sends the
/// declare and the discharge under their symbolic descriptors
/// (`amqp:declare:list`, `amqp:discharge:list`), and reads the txn-id out
/// of the listener's declared. Three transactions in a row:
///
/// 1. two messages posted and committed: each is answered with a
///    transactional-state (0x34) that names the txn-id and the outcome
///    accepted (0x24);
/// 2. one message posted and aborted;
/// 3. the two messages of the first transaction received, accepted in
///    the transaction and committed: the listener settles them as
///    accepted after the commit.
///
/// The aborted message reached no node: Proton receives two messages, and
/// the node is empty at the end.
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_4_proton_declares_posts_aborts_retires_and_commits_against_the_endpoint() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let lines = proton(
        server.port,
        "\x20   def begin(self):\n\
         \x20       self.step = 'post'\n\
         \x20       self.outstanding = 0\n\
         \x20       self.got = 0\n\
         \x20       self.retired = 0\n\
         \x20       self.sender = self.container.create_sender(self.conn, 'orders')\n\
         \x20       self.container.declare_transaction(self.conn, handler=self)\n\
         \x20   def on_transaction_declared(self, event):\n\
         \x20       self.txn = event.transaction\n\
         \x20       print('declared', type(self.txn.id).__name__, len(self.txn.id) <= 32)\n\
         \x20       if self.step == 'post':\n\
         \x20           for i in (1, 2):\n\
         \x20               self.txn.send(self.sender, Message(body='order-%d' % i))\n\
         \x20           self.outstanding = 2\n\
         \x20       elif self.step == 'abort':\n\
         \x20           self.txn.send(self.sender, Message(body='never'))\n\
         \x20           self.outstanding = 1\n\
         \x20       else:\n\
         \x20           self.receiver = self.container.create_receiver(self.conn, 'orders')\n\
         \x20   def on_settled(self, event):\n\
         \x20       d = event.delivery\n\
         \x20       if event.link.is_receiver:\n\
         \x20           print('retired', d.remote_state == Delivery.ACCEPTED)\n\
         \x20           self.retired += 1\n\
         \x20           if self.retired == 2:\n\
         \x20               self.finish()\n\
         \x20           return\n\
         \x20       data = d.remote.data\n\
         \x20       print('posted', d.remote_state == 0x34, data[0] == self.txn.id, int(data[1].descriptor) == 0x24)\n\
         \x20       self.outstanding -= 1\n\
         \x20       if self.outstanding == 0:\n\
         \x20           if self.step == 'post':\n\
         \x20               self.txn.commit()\n\
         \x20           else:\n\
         \x20               self.txn.abort()\n\
         \x20   def on_transaction_committed(self, event):\n\
         \x20       print('committed', self.step)\n\
         \x20       if self.step == 'post':\n\
         \x20           self.step = 'abort'\n\
         \x20           self.container.declare_transaction(self.conn, handler=self)\n\
         \x20   def on_transaction_aborted(self, event):\n\
         \x20       print('aborted')\n\
         \x20       self.step = 'retire'\n\
         \x20       self.container.declare_transaction(self.conn, handler=self)\n\
         \x20   def on_message(self, event):\n\
         \x20       print('got', event.message.body)\n\
         \x20       self.txn.accept(event.delivery)\n\
         \x20       self.got += 1\n\
         \x20       if self.got == 2:\n\
         \x20           self.txn.commit()",
    );
    assert_eq!(
        lines,
        [
            "declared bytes True",
            "posted True True True",
            "posted True True True",
            "committed post",
            "declared bytes True",
            "posted True True True",
            "aborted",
            "declared bytes True",
            "got order-1",
            "got order-2",
            "committed retire",
            "retired True",
            "retired True",
        ]
    );
    wait_depth(&nodes, "orders", 0);
    server.shutdown();
}

/// Part 4 §4.3, §4.4.4 with Proton as the controller: Proton's control
/// link has a source without outcomes, so an error of a discharge is "the
/// detach performative carrying the transaction-error". The node fills
/// between the posting and the commit; the listener refuses the commit by
/// detaching the control link with amqp:transaction:rollback, which
/// Proton reports as the error of its link `txn-ctrl`. The posted message
/// is not at the node.
#[test]
#[ignore = "needs python3 with qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqp10_4_3_proton_reads_the_rollback_of_a_commit_the_endpoint_refuses() {
    let mut cfg = test_handler_cfg();
    cfg.nodes = Arc::new(Nodes::with_limits(
        true,
        NodeLimits {
            max_nodes: 8,
            max_messages: 1,
            max_octets: 1000,
        },
    ));
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let lines = proton(
        server.port,
        "\x20   def begin(self):\n\
         \x20       self.settled = 0\n\
         \x20       self.sender = self.container.create_sender(self.conn, 'small')\n\
         \x20       self.container.declare_transaction(self.conn, handler=self)\n\
         \x20   def on_transaction_declared(self, event):\n\
         \x20       self.txn = event.transaction\n\
         \x20       self.txn.send(self.sender, Message(body='in the transaction'))\n\
         \x20       self.sender.send(Message(body='outside'))\n\
         \x20   def on_settled(self, event):\n\
         \x20       print('settled', hex(event.delivery.remote_state))\n\
         \x20       self.settled += 1\n\
         \x20       if self.settled == 2:\n\
         \x20           self.txn.commit()\n\
         \x20   def on_transaction_committed(self, event):\n\
         \x20       print('committed')\n\
         \x20       self.finish()",
    );
    assert_eq!(
        lines,
        [
            "settled 0x34",
            "settled 0x24",
            "link error txn-ctrl amqp:transaction:rollback",
        ]
    );
    assert_eq!(nodes.depth("small"), Some(1), "only the message outside");
    assert_eq!(
        nodes.take("small", "test").expect("it").format,
        0,
        "an AMQP message"
    );
    server.shutdown();
}
