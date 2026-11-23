// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The transaction controller of `zerodds-amqp-endpoint`
//! (`txn::TxnClient` on the client connection `broker`) against the
//! listener as the transactional resource, over TCP: OASIS AMQP 1.0
//! Part 4 with both roles from this repository. The listener alone,
//! against frames written by hand, is in `txn_resource_e2e.rs`; another
//! implementation as the controller in `proton_txn_e2e.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use amqp_dds_endpoint::nodes::{Message, Nodes};
use common::{TestServer, test_handler_cfg};
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_endpoint::broker::{ConnectParams, SaslCredentials};
use zerodds_amqp_endpoint::terminus::{DeliveryState, Outcome};
use zerodds_amqp_endpoint::transactions::{TxnCapability, TxnId};
use zerodds_amqp_endpoint::txn::{TxnClient, TxnError, posted, retired};
use zerodds_amqp_endpoint::{NodeLimits, TxnLimits};

fn params(port: u16) -> ConnectParams {
    ConnectParams {
        addr: format!("127.0.0.1:{port}"),
        host: "127.0.0.1".into(),
        tls: None,
        sasl: SaslCredentials::Anonymous,
        container_id: "controller".into(),
        hostname: None,
        max_frame_size: 65_536,
        channel_max: 7,
        idle_time_out_ms: 0,
        properties: Vec::new(),
        capabilities: Vec::new(),
        timeout: Duration::from_secs(5),
    }
}

fn controller(port: u16) -> TxnClient {
    TxnClient::connect(
        &params(port),
        &[TxnCapability::LocalTransactions],
        Duration::from_secs(5),
    )
    .expect("the controller connects and the coordinator attaches")
}

fn message(body: &[u8]) -> Vec<u8> {
    MessageSection::Data(body.to_vec())
        .encode()
        .expect("a data section")
}

fn fill(nodes: &Nodes, address: &str, bodies: &[&[u8]]) {
    nodes.declare(address);
    for body in bodies {
        nodes.put(address, Message::new(0, message(body)));
    }
}

/// Waits until the node holds `n` messages.
fn wait_depth(nodes: &Nodes, address: &str, n: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while nodes.depth(address) != Some(n) {
        assert!(
            std::time::Instant::now() < deadline,
            "node {address} holds {:?}, expected {n}",
            nodes.depth(address)
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// §4.2, §4.3, §4.5.1: the controller attaches to the coordinator, reads
/// its capabilities, declares transactions with identifiers of their own
/// and discharges them; a transaction is discharged once.
#[test]
fn amqp10_4_2_the_controller_declares_and_discharges_against_the_coordinator() {
    let server = TestServer::spawn(test_handler_cfg());
    let mut c = controller(server.port);
    let coordinator = c.coordinator();
    for capability in [
        TxnCapability::LocalTransactions,
        TxnCapability::MultiTxnsPerSsn,
        TxnCapability::MultiSsnsPerTxn,
    ] {
        assert!(coordinator.has(capability), "{capability:?}");
    }
    assert!(!coordinator.has(TxnCapability::DistributedTransactions));
    let first = c.declare().expect("declared");
    let second = c.declare().expect("declared");
    assert_ne!(first, second);
    assert_eq!(c.commit(&first), Ok(()));
    assert_eq!(c.rollback(&second), Ok(()));
    assert!(
        matches!(c.commit(&first), Err(TxnError::UnknownId(_))),
        "discharged once"
    );
    assert!(matches!(
        c.rollback(&TxnId::new(b"never declared").unwrap()),
        Err(TxnError::UnknownId(_))
    ));
    c.close();
    server.shutdown();
}

/// §4.4.1: a message sent in a transaction is answered with a
/// transactional-state of that transaction and the outcome accepted, and
/// reaches the node with the commit; one of a rolled-back transaction
/// reaches none; a delivery sent settled in a transaction is posted with
/// its commit too.
#[test]
fn amqp10_4_4_1_the_controller_posts_in_a_transaction_and_commits_or_rolls_back() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut c = controller(server.port);
    let orders = c.sender("orders").expect("a sender");
    let txn = c.declare().expect("declared");
    for body in [b"order-1", b"order-2"] {
        let state = c.send(orders, &message(body), Some(&txn)).expect("sent");
        assert_eq!(state, Some(retired(&txn, Outcome::Accepted)));
    }
    c.send_settled(orders, &message(b"order-3"), Some(&txn))
        .expect("sent settled");
    // Outside the transaction: at the node at once.
    let state = c.send(orders, &message(b"plain"), None).expect("sent");
    assert_eq!(state, Some(DeliveryState::Outcome(Outcome::Accepted)));
    assert_eq!(nodes.depth("orders"), Some(1));
    assert_eq!(c.commit(&txn), Ok(()));
    assert_eq!(nodes.depth("orders"), Some(4));
    let rolled_back = c.declare().expect("declared");
    c.send(orders, &message(b"never"), Some(&rolled_back))
        .expect("sent");
    assert_eq!(c.rollback(&rolled_back), Ok(()));
    assert_eq!(nodes.depth("orders"), Some(4));
    let bodies: Vec<Vec<u8>> = (0..4)
        .map(|_| nodes.take("orders", "test").expect("a message").payload)
        .collect();
    assert_eq!(
        bodies,
        [
            message(b"plain"),
            message(b"order-1"),
            message(b"order-2"),
            message(b"order-3")
        ]
    );
    c.close();
    server.shutdown();
}

/// §4.4.1: "if delivery is split across several transfer frames then all
/// frames MUST be explicitly associated with the same transaction". The
/// listener rejects a delivery whose later frame does not name the
/// transaction; a message of several frames from the controller is
/// accepted in the transaction and is whole at the node after the
/// commit.
#[test]
fn amqp10_4_4_1_a_message_of_several_frames_is_posted_whole() {
    let mut cfg = test_handler_cfg();
    cfg.max_frame_size = 1024;
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut c = controller(server.port);
    assert_eq!(c.remote_open().max_frame_size, 1024);
    let orders = c.sender("orders").expect("a sender");
    let txn = c.declare().expect("declared");
    let body: Vec<u8> = (0..9000u32).map(|i| (i % 251) as u8).collect();
    let state = c.send(orders, &message(&body), Some(&txn)).expect("sent");
    assert_eq!(state, Some(retired(&txn, Outcome::Accepted)));
    assert_eq!(c.commit(&txn), Ok(()));
    assert_eq!(
        nodes.take("orders", "test").expect("the message").payload,
        message(&body)
    );
    c.close();
    server.shutdown();
}

/// §4.4.2: outcomes the controller states in a transaction apply with
/// the commit: the listener settles the deliveries with the outcome and
/// the accepted messages are gone. After a rollback the delivery is
/// still with the controller, which releases it outside a transaction.
#[test]
fn amqp10_4_4_2_the_controller_retires_in_a_transaction() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1", b"order-2", b"order-3"]);
    let server = TestServer::spawn(cfg);
    let mut c = controller(server.port);
    let orders = c.receiver("orders").expect("a receiver");
    let txn = c.declare().expect("declared");
    let mut ids = Vec::new();
    for body in [b"order-1", b"order-2"] {
        let got = c
            .receive(orders, None, Duration::from_secs(5))
            .expect("receive")
            .expect("a message");
        assert_eq!(got.message, message(body));
        assert_eq!(got.state, None, "acquired outside a transaction");
        c.retire(got.delivery_id, Outcome::Accepted, Some(&txn), false)
            .expect("retired");
        ids.push(got.delivery_id);
    }
    assert_eq!(
        c.settled(ids[0], Duration::from_millis(200)).expect("io"),
        None,
        "not settled before the commit"
    );
    assert_eq!(c.commit(&txn), Ok(()));
    for id in ids {
        assert_eq!(
            c.settled(id, Duration::from_secs(5)).expect("io"),
            Some(Some(DeliveryState::Outcome(Outcome::Accepted)))
        );
    }
    assert_eq!(nodes.depth("orders"), Some(1), "the third is still there");
    // Rolled back.
    let txn = c.declare().expect("declared");
    let got = c
        .receive(orders, None, Duration::from_secs(5))
        .expect("receive")
        .expect("the third");
    c.retire(got.delivery_id, Outcome::Accepted, Some(&txn), false)
        .expect("retired");
    assert_eq!(c.rollback(&txn), Ok(()));
    assert_eq!(
        c.settled(got.delivery_id, Duration::from_millis(200))
            .expect("io"),
        None,
        "the outcome was not applied"
    );
    assert_eq!(nodes.depth("orders"), Some(0), "with the controller");
    c.retire(got.delivery_id, Outcome::Released, None, true)
        .expect("released");
    wait_depth(&nodes, "orders", 1);
    c.close();
    server.shutdown();
}

/// §4.4.3: a receive in a transaction sends its txn-id in the flow; the
/// messages arrive with a transactional-state that names it. A rollback
/// returns them to the node; the next receive without a transaction
/// brings them again, in their order, without a state. A committed
/// acquisition with its outcome removes the message.
#[test]
fn amqp10_4_4_3_the_controller_acquires_in_a_transaction() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1", b"order-2"]);
    let server = TestServer::spawn(cfg);
    let mut c = controller(server.port);
    let orders = c.receiver("orders").expect("a receiver");
    let txn = c.declare().expect("declared");
    for body in [b"order-1", b"order-2"] {
        let got = c
            .receive(orders, Some(&txn), Duration::from_secs(5))
            .expect("receive")
            .expect("a message");
        assert_eq!(got.message, message(body));
        assert_eq!(got.state, Some(posted(&txn)));
    }
    // Nothing more: the link is drained, no credit stays behind.
    assert_eq!(
        c.receive(orders, Some(&txn), Duration::from_millis(200))
            .expect("receive"),
        None
    );
    assert_eq!(nodes.depth("orders"), Some(0));
    assert_eq!(c.rollback(&txn), Ok(()));
    wait_depth(&nodes, "orders", 2);
    let first = c
        .receive(orders, None, Duration::from_secs(5))
        .expect("receive")
        .expect("again");
    assert_eq!(first.message, message(b"order-1"));
    assert_eq!(first.state, None, "the transaction of the link is cleared");
    c.retire(first.delivery_id, Outcome::Released, None, true)
        .expect("released");
    wait_depth(&nodes, "orders", 2);
    // Acquired and accepted in a transaction that commits.
    let txn = c.declare().expect("declared");
    let got = c
        .receive(orders, Some(&txn), Duration::from_secs(5))
        .expect("receive")
        .expect("a message");
    assert_eq!(got.state, Some(posted(&txn)));
    c.retire(got.delivery_id, Outcome::Accepted, Some(&txn), false)
        .expect("retired");
    assert_eq!(c.commit(&txn), Ok(()));
    assert_eq!(
        c.settled(got.delivery_id, Duration::from_secs(5))
            .expect("io"),
        Some(Some(DeliveryState::Outcome(Outcome::Accepted)))
    );
    assert_eq!(nodes.depth("orders"), Some(1));
    c.close();
    server.shutdown();
}

/// §4.5.8, §4.3: the errors of the coordinator reach the caller by their
/// condition: a transaction past its time (amqp:transaction:timeout), a
/// commit the node can no longer take (amqp:transaction:rollback), a
/// declare beyond the bound of the configuration
/// (amqp:resource-limit-exceeded).
#[test]
fn amqp10_4_5_8_the_controller_reads_the_transaction_errors() {
    let mut cfg = test_handler_cfg();
    cfg.transactions = TxnLimits {
        max_transactions: 2,
        timeout_ms: 300,
    };
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
    let mut c = controller(server.port);
    let small = c.sender("small").expect("a sender");
    // Rollback: the node fills between the posting and the commit.
    let txn = c.declare().expect("declared");
    c.send(small, &message(b"in the transaction"), Some(&txn))
        .expect("sent");
    c.send(small, &message(b"outside"), None).expect("sent");
    let Err(TxnError::Rollback(reason)) = c.commit(&txn) else {
        panic!("the node has no room for the posted message");
    };
    assert!(reason.contains("node small"), "{reason}");
    assert_eq!(nodes.depth("small"), Some(1));
    // The bound.
    let _one = c.declare().expect("declared");
    let two = c.declare().expect("declared");
    let Err(TxnError::Refused(error)) = c.declare() else {
        panic!("a third transaction beyond max-transactions 2");
    };
    assert_eq!(error.condition, "amqp:resource-limit-exceeded");
    // Time-out.
    std::thread::sleep(Duration::from_millis(600));
    assert!(matches!(c.commit(&two), Err(TxnError::Timeout(_))));
    c.close();
    server.shutdown();
}

/// §4.4.2, §4.1: the listener terminates the control link with
/// amqp:transaction:rollback when a delivery retired in one transaction
/// is retired in another. The controller learns it: its next request
/// fails with the error of the detach, and the transactions are gone.
#[test]
fn amqp10_4_1_the_controller_learns_that_its_control_link_was_terminated() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1"]);
    let server = TestServer::spawn(cfg);
    let mut c = controller(server.port);
    let orders = c.receiver("orders").expect("a receiver");
    let one = c.declare().expect("declared");
    let two = c.declare().expect("declared");
    let got = c
        .receive(orders, None, Duration::from_secs(5))
        .expect("receive")
        .expect("a message");
    c.retire(got.delivery_id, Outcome::Accepted, Some(&one), false)
        .expect("retired");
    c.retire(got.delivery_id, Outcome::Accepted, Some(&two), false)
        .expect("retired");
    // The detach of the control link arrives.
    assert_eq!(
        c.settled(got.delivery_id, Duration::from_millis(300))
            .expect("io"),
        None
    );
    let Err(TxnError::Closed(Some(error))) = c.commit(&one) else {
        panic!("the control link is gone");
    };
    assert_eq!(error.condition, "amqp:transaction:rollback");
    assert!(matches!(c.declare(), Err(TxnError::Closed(Some(_)))));
    c.close();
    server.shutdown();
}

/// §4.5.1: "it is the responsibility of the transaction controller to
/// verify that the capabilities of the controller meet its requirements".
/// The listener's coordinator has no amqp:distributed-transactions; a
/// controller that desires it does not connect, and says what is lacking.
#[test]
fn amqp10_4_5_1_the_controller_refuses_a_coordinator_without_a_desired_capability() {
    let server = TestServer::spawn(test_handler_cfg());
    let Err(error) = TxnClient::connect(
        &params(server.port),
        &[
            TxnCapability::LocalTransactions,
            TxnCapability::DistributedTransactions,
        ],
        Duration::from_secs(5),
    ) else {
        panic!("the coordinator lacks amqp:distributed-transactions");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "{error}");
    let text = error.to_string();
    assert!(text.contains("amqp:distributed-transactions"), "{text}");
    assert!(!text.contains("amqp:local-transactions"), "{text}");
    // The capabilities it has are accepted.
    let c = TxnClient::connect(
        &params(server.port),
        &[
            TxnCapability::LocalTransactions,
            TxnCapability::MultiTxnsPerSsn,
            TxnCapability::MultiSsnsPerTxn,
        ],
        Duration::from_secs(5),
    )
    .expect("every desired capability is there");
    c.close();
    server.shutdown();
}
