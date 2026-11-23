// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The listener over TCP as the transactional resource of OASIS AMQP 1.0
//! Part 4: the coordinator, declare and discharge, transactional posting,
//! retirement and acquisition, and the answers to what is not admitted.
//! The controller side of these tests writes its frames by hand; the
//! tests with the controller of `zerodds-amqp-endpoint` are in
//! `txn_controller_e2e.rs`, the ones with Apache Qpid Proton as the
//! controller in `proton_txn_e2e.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use amqp_dds_endpoint::nodes::{Message, Nodes};
use amqp_dds_endpoint::read_frame;
use common::{Frame, TestServer, connect_client, recv, send, test_handler_cfg};
use zerodds_amqp_bridge::extended_types::AmqpExtValue as V;
use zerodds_amqp_bridge::performatives::descriptor as d;
use zerodds_amqp_endpoint::terminus::{
    AmqpError, DeliveryState, OUTCOME_SYMBOLS, Outcome, Source, Target, decode_error, decode_state,
    encode_state,
};
use zerodds_amqp_endpoint::transactions::{
    Coordinator, Declare, Discharge, TransactionalState, TxnCapability, TxnId, declare_message,
    discharge_message, flow_properties,
};
use zerodds_amqp_endpoint::{NodeLimits, TxnLimits};

/// Whether the server sends nothing for a moment.
fn silent(s: &mut TcpStream) -> bool {
    s.set_read_timeout(Some(Duration::from_millis(150)))
        .expect("timeout");
    let quiet = loop {
        match read_frame(s, 1 << 20) {
            Ok(frame) if frame.body.is_empty() => {}
            Ok(_) => break false,
            Err(_) => break true,
        }
    };
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    quiet
}

fn at(fields: &[V], i: usize) -> V {
    fields.get(i).cloned().unwrap_or(V::Null)
}

fn begin(s: &mut TcpStream, channel: u16) {
    send(
        s,
        channel,
        d::BEGIN,
        vec![V::Null, V::Uint(0), V::Uint(1000), V::Uint(1000)],
        &[],
    );
    let begin = recv(s);
    assert_eq!(begin.1, d::BEGIN);
    assert_eq!(at(&begin.2, 0), V::Ushort(channel));
}

/// Header, open with `open_fields` after the container-id, and begin;
/// returns the stream with the session on channel 0.
fn session_with(port: u16, container: &str, open_fields: Vec<V>) -> TcpStream {
    let mut s = connect_client(port);
    s.write_all(b"AMQP\x00\x01\x00\x00").expect("header");
    let mut header = [0u8; 8];
    s.read_exact(&mut header).expect("the server's header");
    assert_eq!(&header, b"AMQP\x00\x01\x00\x00");
    let mut open = vec![V::Str(container.into())];
    open.extend(open_fields);
    send(&mut s, 0, d::OPEN, open, &[]);
    assert_eq!(recv(&mut s).1, d::OPEN);
    begin(&mut s, 0);
    s
}

fn session(port: u16, container: &str) -> TcpStream {
    session_with(port, container, Vec::new())
}

/// The source of a control link that supports all four outcomes.
fn source_with_outcomes() -> V {
    Source {
        outcomes: OUTCOME_SYMBOLS.iter().map(|s| (*s).to_string()).collect(),
        ..Source::default()
    }
    .encode()
}

/// Attaches a link this client sends on, on `channel`; returns the
/// server's attach and its flow.
fn attach_sender_on(
    s: &mut TcpStream,
    channel: u16,
    handle: u32,
    name: &str,
    source: V,
    target: V,
) -> (Frame, Frame) {
    send(
        s,
        channel,
        d::ATTACH,
        vec![
            V::Str(name.into()),
            V::Uint(handle),
            V::Boolean(false),
            V::Null,
            V::Null,
            source,
            target,
            V::Null,
            V::Null,
            V::Uint(0),
        ],
        &[],
    );
    let attach = recv(s);
    assert_eq!(attach.1, d::ATTACH, "{attach:?}");
    let flow = recv(s);
    (attach, flow)
}

fn attach_sender(s: &mut TcpStream, handle: u32, address: &str) {
    let (_, flow) = attach_sender_on(
        s,
        0,
        handle,
        &format!("to-{address}-{handle}"),
        Source::default().encode(),
        Target::at(address).encode(),
    );
    assert_eq!(flow.1, d::FLOW, "{flow:?}");
}

/// Attaches the control link on handle `handle`; returns the server's
/// attach.
fn attach_control(s: &mut TcpStream, handle: u32, source: V) -> Frame {
    let desired = Coordinator {
        capabilities: vec![TxnCapability::LocalTransactions],
    };
    let (attach, flow) = attach_sender_on(s, 0, handle, "txn-ctl", source, desired.encode());
    assert_eq!(flow.1, d::FLOW, "the coordinator grants credit: {flow:?}");
    assert_eq!(at(&flow.2, 4), at(&attach.2, 1), "for the control link");
    attach
}

/// Attaches a link this client receives on; returns the server's attach.
fn attach_receiver(s: &mut TcpStream, handle: u32, address: &str) -> Frame {
    send(
        s,
        0,
        d::ATTACH,
        vec![
            V::Str(format!("from-{address}-{handle}")),
            V::Uint(handle),
            V::Boolean(true),
            V::Null,
            V::Null,
            Source::at(address).encode(),
            Target::default().encode(),
        ],
        &[],
    );
    let attach = recv(s);
    assert_eq!(attach.1, d::ATTACH, "{attach:?}");
    attach
}

/// A transfer with a state on `channel`.
#[allow(clippy::too_many_arguments)]
fn transfer_on(
    s: &mut TcpStream,
    channel: u16,
    handle: u32,
    id: u32,
    settled: bool,
    more: bool,
    state: Option<&DeliveryState>,
    payload: &[u8],
) {
    send(
        s,
        channel,
        d::TRANSFER,
        vec![
            V::Uint(handle),
            V::Uint(id),
            V::Binary(id.to_be_bytes().to_vec()),
            V::Uint(0),
            V::Boolean(settled),
            V::Boolean(more),
            V::Null,
            state.map_or(V::Null, encode_state),
        ],
        payload,
    );
}

/// An unsettled transfer of one frame without a state.
fn transfer(s: &mut TcpStream, handle: u32, id: u32, payload: &[u8]) {
    transfer_on(s, 0, handle, id, false, false, None, payload);
}

fn in_txn(txn: &TxnId, outcome: Option<Outcome>) -> DeliveryState {
    DeliveryState::Transactional(TransactionalState {
        txn_id: txn.clone(),
        outcome,
    })
}

/// An unsettled transfer of one frame posted to `txn`.
fn post(s: &mut TcpStream, handle: u32, id: u32, txn: &TxnId, payload: &[u8]) {
    transfer_on(
        s,
        0,
        handle,
        id,
        false,
        false,
        Some(&in_txn(txn, None)),
        payload,
    );
}

/// The disposition of the server as receiver for delivery `id`: settled
/// and state.
fn answer(s: &mut TcpStream, id: u32) -> (bool, Option<DeliveryState>) {
    let frame = recv(s);
    assert_eq!(frame.1, d::DISPOSITION, "{frame:?}");
    assert_eq!(at(&frame.2, 0), V::Boolean(true), "role receiver");
    assert_eq!(at(&frame.2, 1), V::Uint(id), "{frame:?}");
    (
        at(&frame.2, 3) == V::Boolean(true),
        decode_state(frame.2.get(4)).expect("a delivery state"),
    )
}

/// Declares a transaction over the control link on `handle` with
/// delivery `id`; returns its identifier.
fn declare(s: &mut TcpStream, handle: u32, id: u32) -> TxnId {
    transfer(
        s,
        handle,
        id,
        &declare_message(&Declare::default()).expect("declare"),
    );
    let (settled, state) = answer(s, id);
    assert!(settled, "receiver settle mode first");
    let Some(DeliveryState::Declared(txn)) = state else {
        panic!("a declare is answered with declared: {state:?}");
    };
    txn
}

/// Sends a discharge over the control link on `handle` with delivery
/// `id`.
fn send_discharge(s: &mut TcpStream, handle: u32, id: u32, txn: &TxnId, fail: bool) {
    transfer(
        s,
        handle,
        id,
        &discharge_message(&Discharge {
            txn_id: txn.clone(),
            fail,
        })
        .expect("discharge"),
    );
}

/// Discharges and returns the outcome.
fn discharge(s: &mut TcpStream, handle: u32, id: u32, txn: &TxnId, fail: bool) -> Outcome {
    send_discharge(s, handle, id, txn, fail);
    let (settled, state) = answer(s, id);
    assert!(settled);
    state.expect("an outcome").outcome()
}

fn rejected_with(outcome: &Outcome) -> AmqpError {
    let Outcome::Rejected(Some(error)) = outcome else {
        panic!("rejected with an error: {outcome:?}");
    };
    error.clone()
}

/// A link flow of this client as receiver, with the transaction to
/// acquire in.
fn credit(
    s: &mut TcpStream,
    handle: u32,
    delivery_count: u32,
    link_credit: u32,
    txn: Option<&TxnId>,
) {
    let mut fields = vec![
        V::Uint(0),
        V::Uint(1000),
        V::Uint(0),
        V::Uint(1000),
        V::Uint(handle),
        V::Uint(delivery_count),
        V::Uint(link_credit),
    ];
    if let Some(txn) = txn {
        fields.extend([V::Null, V::Null, V::Null, flow_properties(txn)]);
    }
    send(s, 0, d::FLOW, fields, &[]);
}

/// A disposition of this client as receiver.
fn dispose(s: &mut TcpStream, first: u32, last: u32, settled: bool, state: &DeliveryState) {
    send(
        s,
        0,
        d::DISPOSITION,
        vec![
            V::Boolean(true),
            V::Uint(first),
            V::Uint(last),
            V::Boolean(settled),
            encode_state(state),
        ],
        &[],
    );
}

/// The next transfer of the server: delivery-id, state, payload.
fn delivery(s: &mut TcpStream) -> (u32, Option<DeliveryState>, Vec<u8>) {
    let frame = recv(s);
    assert_eq!(frame.1, d::TRANSFER, "{frame:?}");
    let V::Uint(id) = at(&frame.2, 1) else {
        panic!("a delivery-id: {frame:?}");
    };
    (id, decode_state(frame.2.get(7)).expect("a state"), frame.3)
}

fn expect_detach(s: &mut TcpStream) -> AmqpError {
    let frame = recv(s);
    assert_eq!(frame.1, d::DETACH, "{frame:?}");
    assert_eq!(at(&frame.2, 1), V::Boolean(true), "closed");
    decode_error(frame.2.get(2)).expect("a detach with an error")
}

fn close(mut s: TcpStream) {
    send(&mut s, 0, d::CLOSE, vec![], &[]);
    loop {
        if recv(&mut s).1 == d::CLOSE {
            break;
        }
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

fn fill(nodes: &Nodes, address: &str, bodies: &[&[u8]]) {
    nodes.declare(address);
    for body in bodies {
        nodes.put(address, Message::new(0, body.to_vec()));
    }
}

// ============================================================================
// §4.2 Declaring a transaction.
// ============================================================================

/// §4.2, Figure 4.1: the attach of a control link is answered with the
/// coordinator and its capabilities (§4.5.1, §4.5.7), the coordinator
/// grants credit, and a declare is answered with the outcome declared
/// that carries the identifier (§4.5.5), of at most 32 octets (§4.5.4);
/// each declare gets its own.
#[test]
fn amqp10_4_2_a_declare_is_answered_with_declared_and_the_identifier() {
    let server = TestServer::spawn(test_handler_cfg());
    let mut s = session(server.port, "controller");
    let attach = attach_control(&mut s, 0, source_with_outcomes());
    assert_eq!(at(&attach.2, 2), V::Boolean(true), "the resource receives");
    assert_eq!(
        Coordinator::decode(attach.2.get(6)),
        Ok(Some(Coordinator {
            capabilities: vec![
                TxnCapability::LocalTransactions,
                TxnCapability::MultiTxnsPerSsn,
                TxnCapability::MultiSsnsPerTxn,
            ]
        }))
    );
    let first = declare(&mut s, 0, 0);
    let second = declare(&mut s, 0, 1);
    assert_ne!(first, second);
    assert!(first.as_bytes().len() <= 32 && !first.as_bytes().is_empty());
    // §4.2: "Other standard message sections such as the header section
    // SHOULD be ignored."
    let mut with_header =
        zerodds_amqp_bridge::sections::MessageSection::Header(V::List(vec![V::Boolean(true)]))
            .encode()
            .expect("header");
    with_header.extend(declare_message(&Declare::default()).expect("declare"));
    transfer(&mut s, 0, 2, &with_header);
    let (_, state) = answer(&mut s, 2);
    assert!(
        matches!(state, Some(DeliveryState::Declared(_))),
        "{state:?}"
    );
    close(s);
    server.shutdown();
}

/// §4.2: a declare the coordinator cannot perform is rejected with the
/// error where the source of the control link supports rejected:
/// a global-id without the capability distributed-transactions (§4.5.2),
/// a body that is neither declare nor discharge, octets that are no
/// message. A message sent settled detaches the link with
/// amqp:illegal-state; transactional work on the control link (§4.1)
/// with amqp:not-allowed.
#[test]
fn amqp10_4_2_a_declare_the_coordinator_cannot_perform_is_rejected_with_the_error() {
    let server = TestServer::spawn(test_handler_cfg());
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    let global = declare_message(&Declare {
        global_id: Some(V::Binary(vec![1, 2, 3])),
    })
    .expect("declare");
    transfer(&mut s, 0, 0, &global);
    let (settled, state) = answer(&mut s, 0);
    assert!(settled);
    let error = rejected_with(&state.expect("an outcome").outcome());
    assert_eq!(error.condition, "amqp:not-implemented");
    assert!(
        error
            .description
            .as_deref()
            .is_some_and(|t| t.contains("amqp:distributed-transactions")),
        "{error:?}"
    );
    // A body that is no request.
    let text = zerodds_amqp_bridge::sections::MessageSection::AmqpValue(V::Str("begin".into()))
        .encode()
        .expect("a message");
    transfer(&mut s, 0, 1, &text);
    let (_, state) = answer(&mut s, 1);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:not-implemented"
    );
    // Octets that are no message.
    transfer(&mut s, 0, 2, &[0x00, 0x53]);
    let (_, state) = answer(&mut s, 2);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:decode-error"
    );
    // The link lives: a declare is still answered.
    let txn = declare(&mut s, 0, 3);
    // Transactional work on the control link.
    post(
        &mut s,
        0,
        4,
        &txn,
        &declare_message(&Declare::default()).unwrap(),
    );
    assert_eq!(expect_detach(&mut s).condition, "amqp:not-allowed");
    send(
        &mut s,
        0,
        d::DETACH,
        vec![V::Uint(0), V::Boolean(true)],
        &[],
    );
    // A declare sent settled.
    attach_control(&mut s, 1, source_with_outcomes());
    transfer_on(
        &mut s,
        0,
        1,
        5,
        true,
        false,
        None,
        &declare_message(&Declare::default()).unwrap(),
    );
    assert_eq!(expect_detach(&mut s).condition, "amqp:illegal-state");
    close(s);
    server.shutdown();
}

/// §4.2, §4.3: "If the source does not support the rejected outcome, the
/// transactional resource MUST detach the link to the coordinator, with
/// the detach performative carrying the transaction-error." Here: a
/// discharge of an identifier that names no transaction, over a control
/// link whose source names no outcomes.
#[test]
fn amqp10_4_3_a_control_link_without_rejected_is_detached_with_the_error() {
    let server = TestServer::spawn(test_handler_cfg());
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, Source::default().encode());
    let txn = declare(&mut s, 0, 0);
    assert_eq!(discharge(&mut s, 0, 1, &txn, false), Outcome::Accepted);
    // Discharged once: the identifier is unknown now.
    send_discharge(&mut s, 0, 2, &txn, false);
    let error = expect_detach(&mut s);
    assert_eq!(error.condition, "amqp:transaction:unknown-id");
    close(s);
    server.shutdown();
}

/// §4.3 with §4.5.8: a discharge of an identifier that names no
/// transaction is rejected with amqp:transaction:unknown-id over a
/// control link whose source supports rejected; the link lives on. A
/// txn-id of more than 32 octets (§4.5.4) is rejected with
/// amqp:invalid-field.
#[test]
fn amqp10_4_3_a_discharge_of_an_unknown_id_is_rejected_with_unknown_id() {
    let server = TestServer::spawn(test_handler_cfg());
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    let unknown = TxnId::new(b"never declared").expect("a txn-id");
    for (id, fail) in [(0, false), (1, true)] {
        let outcome = discharge(&mut s, 0, id, &unknown, fail);
        assert_eq!(
            rejected_with(&outcome).condition,
            "amqp:transaction:unknown-id"
        );
    }
    let long = zerodds_amqp_bridge::sections::MessageSection::AmqpValue(V::Described {
        descriptor: 0x32,
        value: Box::new(V::List(vec![V::Binary(vec![7; 33])])),
    })
    .encode()
    .expect("a message");
    transfer(&mut s, 0, 2, &long);
    let (_, state) = answer(&mut s, 2);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:invalid-field"
    );
    let txn = declare(&mut s, 0, 3);
    assert_eq!(discharge(&mut s, 0, 4, &txn, true), Outcome::Accepted);
    close(s);
    server.shutdown();
}

/// §4.1: "If the control link is closed while there exist non-discharged
/// transactions it created, then all such transactions are immediately
/// rolled back, and attempts to perform further transactional work on
/// them will lead to failure." §4.2: a link to the coordinator is not
/// resumed: the link attached again knows no transaction of the first.
#[test]
fn amqp10_4_1_closing_the_control_link_rolls_back_its_transactions() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    post(&mut s, 1, 1, &txn, b"order-1");
    let (_, state) = answer(&mut s, 1);
    assert_eq!(state, Some(in_txn(&txn, Some(Outcome::Accepted))));
    // The control link closes.
    send(
        &mut s,
        0,
        d::DETACH,
        vec![V::Uint(0), V::Boolean(true)],
        &[],
    );
    assert_eq!(recv(&mut s).1, d::DETACH);
    assert_eq!(nodes.depth("orders"), Some(0), "rolled back");
    // Further work on the transaction fails.
    post(&mut s, 1, 2, &txn, b"order-2");
    let (settled, state) = answer(&mut s, 2);
    assert!(settled);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:transaction:unknown-id"
    );
    // The same link name again: a new coordinator link.
    attach_control(&mut s, 0, source_with_outcomes());
    let outcome = discharge(&mut s, 0, 3, &txn, false);
    assert_eq!(
        rejected_with(&outcome).condition,
        "amqp:transaction:unknown-id"
    );
    assert_eq!(nodes.depth("orders"), Some(0));
    close(s);
    server.shutdown();
}

// ============================================================================
// §4.4.1 Transactional posting.
// ============================================================================

/// §4.4.1, Figure 4.3: a transfer whose state names the transaction is
/// answered with a transactional-state of that transaction and the
/// outcome that applies with the commit; "the message does not become
/// available at the destination node within the transactional resource
/// until after the transaction has been successfully discharged". A
/// delivery sent settled in the transaction is held likewise.
#[test]
fn amqp10_4_4_1_a_posted_message_reaches_the_node_only_with_the_commit() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    for (id, body) in [(1u32, b"order-1"), (2, b"order-2")] {
        post(&mut s, 1, id, &txn, body);
        let (settled, state) = answer(&mut s, id);
        assert!(settled, "the resource settles (§4.4.4)");
        assert_eq!(state, Some(in_txn(&txn, Some(Outcome::Accepted))));
    }
    // Sent settled by the controller: no disposition.
    transfer_on(
        &mut s,
        0,
        1,
        3,
        true,
        false,
        Some(&in_txn(&txn, None)),
        b"order-3",
    );
    // A message outside the transaction is available at once.
    transfer(&mut s, 1, 4, b"plain");
    let (_, state) = answer(&mut s, 4);
    assert_eq!(state, Some(DeliveryState::Outcome(Outcome::Accepted)));
    assert_eq!(nodes.depth("orders"), Some(1), "only the plain message");
    assert_eq!(discharge(&mut s, 0, 5, &txn, false), Outcome::Accepted);
    assert_eq!(nodes.depth("orders"), Some(4));
    let bodies: Vec<Vec<u8>> = (0..4)
        .map(|_| nodes.take("orders", "test").expect("a message").payload)
        .collect();
    assert_eq!(
        bodies,
        [
            b"plain".to_vec(),
            b"order-1".to_vec(),
            b"order-2".to_vec(),
            b"order-3".to_vec()
        ]
    );
    close(s);
    server.shutdown();
}

/// §4.3, §4.4.1: a discharge with fail set rolls the transaction back:
/// "If the transaction is rolled back then the delivery is not made
/// available" (§4.4.4). The identifier is gone with the discharge: a
/// transfer that names it is rejected with amqp:transaction:unknown-id,
/// and one sent settled, which has no outcome to carry the error,
/// detaches its link with it.
#[test]
fn amqp10_4_4_1_a_rolled_back_posting_reaches_no_node() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    post(&mut s, 1, 1, &txn, b"order-1");
    let (_, state) = answer(&mut s, 1);
    assert_eq!(state, Some(in_txn(&txn, Some(Outcome::Accepted))));
    assert_eq!(discharge(&mut s, 0, 2, &txn, true), Outcome::Accepted);
    assert_eq!(nodes.depth("orders"), Some(0));
    post(&mut s, 1, 3, &txn, b"order-2");
    let (settled, state) = answer(&mut s, 3);
    assert!(settled);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:transaction:unknown-id"
    );
    transfer_on(
        &mut s,
        0,
        1,
        4,
        true,
        false,
        Some(&in_txn(&txn, None)),
        b"order-3",
    );
    assert_eq!(
        expect_detach(&mut s).condition,
        "amqp:transaction:unknown-id"
    );
    assert_eq!(nodes.depth("orders"), Some(0));
    close(s);
    server.shutdown();
}

/// §4.4.1: "It is an error for the controller to attempt to discharge a
/// transaction against which a partial delivery has been posted. If this
/// happens, the control link MUST be terminated with the
/// transaction-rollback error." The transaction is rolled back with its
/// link (§4.1); the rest of the delivery is then rejected.
#[test]
fn amqp10_4_4_1_a_discharge_with_a_partial_delivery_terminates_the_control_link() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    post(&mut s, 1, 1, &txn, b"whole");
    answer(&mut s, 1);
    // The first of two frames.
    transfer_on(
        &mut s,
        0,
        1,
        2,
        false,
        true,
        Some(&in_txn(&txn, None)),
        b"half",
    );
    send_discharge(&mut s, 0, 3, &txn, false);
    let error = expect_detach(&mut s);
    assert_eq!(error.condition, "amqp:transaction:rollback");
    assert_eq!(nodes.depth("orders"), Some(0), "nothing was committed");
    // The second frame completes a delivery of a transaction that is
    // gone.
    transfer_on(
        &mut s,
        0,
        1,
        2,
        false,
        false,
        Some(&in_txn(&txn, None)),
        b"-rest",
    );
    let (_, state) = answer(&mut s, 2);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:transaction:unknown-id"
    );
    assert_eq!(nodes.depth("orders"), Some(0));
    close(s);
    server.shutdown();
}

/// §4.4.1: "if delivery is split across several transfer frames then all
/// frames MUST be explicitly associated with the same transaction": a
/// delivery whose second frame does not name the transaction is rejected
/// within the transaction, and is not posted with the commit; a delivery
/// with the state on both frames is.
#[test]
fn amqp10_4_4_1_every_frame_of_a_posted_delivery_names_the_transaction() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    let state = in_txn(&txn, None);
    transfer_on(&mut s, 0, 1, 1, false, true, Some(&state), b"both-");
    transfer_on(&mut s, 0, 1, 1, false, false, Some(&state), b"frames");
    let (_, answered) = answer(&mut s, 1);
    assert_eq!(answered, Some(in_txn(&txn, Some(Outcome::Accepted))));
    transfer_on(&mut s, 0, 1, 2, false, true, Some(&state), b"first-");
    transfer_on(&mut s, 0, 1, 2, false, false, None, b"only");
    let (_, answered) = answer(&mut s, 2);
    let Some(DeliveryState::Transactional(answered)) = answered else {
        panic!("an outcome of the same transaction: {answered:?}");
    };
    assert_eq!(answered.txn_id, txn);
    assert_eq!(
        rejected_with(&answered.outcome.expect("an outcome")).condition,
        "amqp:invalid-field"
    );
    assert_eq!(discharge(&mut s, 0, 3, &txn, false), Outcome::Accepted);
    assert_eq!(nodes.depth("orders"), Some(1));
    assert_eq!(
        nodes.take("orders", "test").expect("the whole one").payload,
        b"both-frames"
    );
    close(s);
    server.shutdown();
}

/// §4.4.1, the other direction: a delivery whose first frame names no
/// transaction and whose later frame does is not posted at all. It is
/// rejected with amqp:invalid-field outside any transaction, reaches no
/// node, and the transaction the later frame named does not commit
/// (§4.4.4). Sent settled, no outcome carries the error, and the link is
/// destroyed with it.
#[test]
fn amqp10_4_4_1_a_delivery_whose_later_frame_names_the_transaction_is_rejected() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    transfer_on(&mut s, 0, 1, 1, false, true, None, b"first-");
    transfer_on(
        &mut s,
        0,
        1,
        1,
        false,
        false,
        Some(&in_txn(&txn, None)),
        b"named",
    );
    let (_, answered) = answer(&mut s, 1);
    let Some(DeliveryState::Outcome(outcome)) = answered else {
        panic!("an outcome outside any transaction: {answered:?}");
    };
    assert_eq!(rejected_with(&outcome).condition, "amqp:invalid-field");
    assert_eq!(nodes.depth("orders"), Some(0), "not posted");
    let outcome = discharge(&mut s, 0, 2, &txn, false);
    assert_eq!(
        rejected_with(&outcome).condition,
        "amqp:transaction:rollback"
    );
    assert_eq!(nodes.depth("orders"), Some(0));
    // Sent settled.
    let other = declare(&mut s, 0, 3);
    transfer_on(&mut s, 0, 1, 4, true, true, None, b"a");
    transfer_on(
        &mut s,
        0,
        1,
        4,
        true,
        false,
        Some(&in_txn(&other, None)),
        b"b",
    );
    assert_eq!(expect_detach(&mut s).condition, "amqp:invalid-field");
    assert_eq!(nodes.depth("orders"), Some(0));
    close(s);
    server.shutdown();
}

/// §4.4.1: a delivery whose first frame names no transaction and whose
/// second frame names one is a partial delivery posted against that
/// transaction until its last frame. A discharge of the transaction then
/// terminates the control link with amqp:transaction:rollback.
#[test]
fn amqp10_4_4_1_a_transaction_a_later_frame_names_has_a_partial_delivery() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    transfer_on(&mut s, 0, 1, 1, false, true, None, b"one-");
    transfer_on(
        &mut s,
        0,
        1,
        1,
        false,
        true,
        Some(&in_txn(&txn, None)),
        b"two-",
    );
    send_discharge(&mut s, 0, 2, &txn, false);
    assert_eq!(expect_detach(&mut s).condition, "amqp:transaction:rollback");
    transfer_on(
        &mut s,
        0,
        1,
        1,
        false,
        false,
        Some(&in_txn(&txn, None)),
        b"three",
    );
    let (_, answered) = answer(&mut s, 1);
    assert_eq!(
        rejected_with(&answered.expect("an outcome").outcome()).condition,
        "amqp:invalid-field"
    );
    assert_eq!(nodes.depth("orders"), Some(0));
    close(s);
    server.shutdown();
}

/// §4.4.4, posting: "If the resource can no longer apply the outcome
/// that it originally indicated would be the result of a successful
/// discharge then it MUST NOT allow the successful discharge of the
/// associated transaction", "by rejecting any attempt to discharge the
/// transaction where the fail flag is not set to true". Here the node
/// fills up between the posting and the commit: the commit is rejected
/// with amqp:transaction:rollback and no message of the transaction
/// reaches a node. A delivery sent settled that the resource cannot take
/// has the same effect ("If the resource is unable to process the
/// delivery it MUST NOT allow the successful discharge").
#[test]
fn amqp10_4_4_4_a_commit_the_nodes_can_no_longer_take_is_rejected_with_rollback() {
    let mut cfg = test_handler_cfg();
    cfg.nodes = Arc::new(Nodes::with_limits(
        true,
        NodeLimits {
            max_nodes: 8,
            max_messages: 2,
            max_octets: 1000,
        },
    ));
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "small");
    attach_sender(&mut s, 2, "other");
    let txn = declare(&mut s, 0, 0);
    post(&mut s, 1, 1, &txn, b"a");
    post(&mut s, 2, 2, &txn, b"b");
    for id in [1, 2] {
        let (_, state) = answer(&mut s, id);
        assert_eq!(state, Some(in_txn(&txn, Some(Outcome::Accepted))));
    }
    // Two messages outside the transaction fill the node.
    for id in [3, 4] {
        transfer(&mut s, 1, id, b"plain");
        answer(&mut s, id);
    }
    let outcome = discharge(&mut s, 0, 5, &txn, false);
    let error = rejected_with(&outcome);
    assert_eq!(error.condition, "amqp:transaction:rollback");
    assert!(
        error
            .description
            .as_deref()
            .is_some_and(|t| t.contains("node small")),
        "{error:?}"
    );
    assert_eq!(nodes.depth("small"), Some(2));
    assert_eq!(nodes.depth("other"), Some(0), "all or none");
    // A delivery sent settled beyond what the transaction may hold for a
    // node: the transaction can only be rolled back.
    let txn = declare(&mut s, 0, 6);
    for id in [7, 8, 9] {
        transfer_on(
            &mut s,
            0,
            2,
            id,
            true,
            false,
            Some(&in_txn(&txn, None)),
            b"settled",
        );
    }
    let outcome = discharge(&mut s, 0, 10, &txn, false);
    assert_eq!(
        rejected_with(&outcome).condition,
        "amqp:transaction:rollback"
    );
    assert_eq!(nodes.depth("other"), Some(0));
    close(s);
    server.shutdown();
}

// ============================================================================
// §4.4.2 Transactional retirement.
// ============================================================================

/// §4.4.2, Figure 4.4: a disposition whose state is a
/// transactional-state associates the outcome with the transaction. "On
/// a successful discharge, the resource will apply the given outcome and
/// can immediately settle the transfers": the deliveries stay unsettled
/// until the commit, are settled by the listener with it, and the
/// accepted messages are gone from the node.
#[test]
fn amqp10_4_4_2_an_outcome_in_a_transaction_applies_with_the_commit() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1", b"order-2", b"order-3"]);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_receiver(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    credit(&mut s, 1, 0, 3, None);
    let mut ids = Vec::new();
    for body in [b"order-1", b"order-2", b"order-3"] {
        let (id, state, payload) = delivery(&mut s);
        assert_eq!(state, None, "acquired outside the transaction");
        assert_eq!(payload, body);
        ids.push(id);
    }
    // Two accepted, one released, in the transaction.
    dispose(
        &mut s,
        ids[0],
        ids[1],
        false,
        &in_txn(&txn, Some(Outcome::Accepted)),
    );
    dispose(
        &mut s,
        ids[2],
        ids[2],
        false,
        &in_txn(&txn, Some(Outcome::Released)),
    );
    assert!(silent(&mut s), "nothing is settled before the discharge");
    assert_eq!(
        nodes.depth("orders"),
        Some(0),
        "all three are with the link"
    );
    assert_eq!(discharge(&mut s, 0, 1, &txn, false), Outcome::Accepted);
    // The listener settles the three with the outcome that was applied.
    let mut settled = Vec::new();
    for _ in 0..3 {
        let frame = recv(&mut s);
        assert_eq!(frame.1, d::DISPOSITION, "{frame:?}");
        assert_eq!(at(&frame.2, 0), V::Boolean(false), "role sender");
        assert_eq!(at(&frame.2, 3), V::Boolean(true), "settled");
        let V::Uint(id) = at(&frame.2, 1) else {
            panic!("first: {frame:?}");
        };
        settled.push((
            id,
            decode_state(frame.2.get(4))
                .expect("a state")
                .expect("an outcome")
                .outcome(),
        ));
    }
    settled.sort_by_key(|(id, _)| *id);
    assert_eq!(
        settled,
        [
            (ids[0], Outcome::Accepted),
            (ids[1], Outcome::Accepted),
            (ids[2], Outcome::Released)
        ]
    );
    // The released one is back at the node, the accepted ones are gone.
    wait_depth(&nodes, "orders", 1);
    assert_eq!(
        nodes.take("orders", "test").expect("released").payload,
        b"order-3"
    );
    close(s);
    server.shutdown();
}

/// §4.4.2, §4.4.4 retirement: after a rollback "the outcome will not be
/// applied, and the deliveries will still be 'live' and will remain
/// acquired by the controller": the listener settles nothing, and a
/// later outcome outside a transaction applies. A delivery the controller
/// settled with its transactional outcome takes the default outcome of
/// the source, released ("the resource MUST apply the default outcome").
#[test]
fn amqp10_4_4_2_a_rolled_back_outcome_is_not_applied() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1", b"order-2"]);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_receiver(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    credit(&mut s, 1, 0, 2, None);
    let (first, _, _) = delivery(&mut s);
    let (second, _, _) = delivery(&mut s);
    // The first not settled, the second settled by the controller.
    dispose(
        &mut s,
        first,
        first,
        false,
        &in_txn(&txn, Some(Outcome::Accepted)),
    );
    dispose(
        &mut s,
        second,
        second,
        true,
        &in_txn(&txn, Some(Outcome::Accepted)),
    );
    assert_eq!(discharge(&mut s, 0, 1, &txn, true), Outcome::Accepted);
    // The settled one: released, back at the node. No credit is left, so
    // it stays there.
    wait_depth(&nodes, "orders", 1);
    assert!(silent(&mut s), "the first delivery is not settled");
    // The first is still with the controller: it accepts it now, outside
    // a transaction, and the listener settles.
    dispose(
        &mut s,
        first,
        first,
        false,
        &DeliveryState::Outcome(Outcome::Accepted),
    );
    let frame = recv(&mut s);
    assert_eq!(frame.1, d::DISPOSITION, "{frame:?}");
    assert_eq!(at(&frame.2, 1), V::Uint(first));
    assert_eq!(at(&frame.2, 3), V::Boolean(true));
    assert_eq!(nodes.depth("orders"), Some(1));
    assert_eq!(
        nodes.take("orders", "test").expect("released").payload,
        b"order-2"
    );
    close(s);
    server.shutdown();
}

/// §4.4.2: "the delivery MUST NOT be associated with a different
/// non-discharged transaction than the outcome. If this happens then the
/// control link MUST be terminated with a transaction-rollback error."
/// Here a delivery retired in one transaction is retired in a second
/// one. The transactions of the link are rolled back with it: the
/// delivery stays with the controller.
#[test]
fn amqp10_4_4_2_an_outcome_in_another_transaction_terminates_the_control_link() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1"]);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_receiver(&mut s, 1, "orders");
    let one = declare(&mut s, 0, 0);
    let two = declare(&mut s, 0, 1);
    credit(&mut s, 1, 0, 1, None);
    let (id, _, _) = delivery(&mut s);
    dispose(
        &mut s,
        id,
        id,
        false,
        &in_txn(&one, Some(Outcome::Accepted)),
    );
    dispose(
        &mut s,
        id,
        id,
        false,
        &in_txn(&two, Some(Outcome::Accepted)),
    );
    let error = expect_detach(&mut s);
    assert_eq!(error.condition, "amqp:transaction:rollback");
    // Both transactions are gone; the delivery was not accepted: when its
    // link goes, the message is back at the node.
    assert_eq!(nodes.depth("orders"), Some(0));
    send(
        &mut s,
        0,
        d::DETACH,
        vec![V::Uint(1), V::Boolean(true)],
        &[],
    );
    wait_depth(&nodes, "orders", 1);
    close(s);
    server.shutdown();
}

/// §4.4.2 with §4.5.8: an outcome in a transaction that does not exist
/// cannot be associated: the link of the delivery is detached with
/// amqp:transaction:unknown-id, and the delivery takes the default
/// outcome of the source.
#[test]
fn amqp10_4_4_2_an_outcome_in_an_unknown_transaction_detaches_the_link_with_unknown_id() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1"]);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_receiver(&mut s, 1, "orders");
    credit(&mut s, 1, 0, 1, None);
    let (id, _, _) = delivery(&mut s);
    let unknown = TxnId::new(&[9, 9]).expect("a txn-id");
    dispose(
        &mut s,
        id,
        id,
        false,
        &in_txn(&unknown, Some(Outcome::Accepted)),
    );
    assert_eq!(
        expect_detach(&mut s).condition,
        "amqp:transaction:unknown-id"
    );
    wait_depth(&nodes, "orders", 1);
    close(s);
    server.shutdown();
}

// ============================================================================
// §4.4.3 Transactional acquisition.
// ============================================================================

/// §4.4.3, Figure 4.5: the txn-id in the properties of a flow is the
/// transaction the link acquires in: its transfers carry a
/// transactional-state that names it. With a rollback "both the
/// acquisition and outcome are rolled back" (§4.4.4): the messages are
/// back at the node, in their order, and the listener settles the
/// deliveries as released. "When a transaction is discharged, the txn-id
/// of any link endpoints will be cleared": the next transfers carry no
/// state.
#[test]
fn amqp10_4_4_3_messages_acquired_in_a_transaction_return_with_the_rollback() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1", b"order-2", b"order-3"]);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_receiver(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    credit(&mut s, 1, 0, 2, Some(&txn));
    let mut ids = Vec::new();
    for body in [b"order-1", b"order-2"] {
        let (id, state, payload) = delivery(&mut s);
        assert_eq!(
            state,
            Some(in_txn(&txn, None)),
            "acquired in the transaction"
        );
        assert_eq!(payload, body);
        ids.push(id);
    }
    // An outcome of the same transaction for the first (§4.4.4).
    dispose(
        &mut s,
        ids[0],
        ids[0],
        false,
        &in_txn(&txn, Some(Outcome::Accepted)),
    );
    assert_eq!(nodes.depth("orders"), Some(1));
    send_discharge(&mut s, 0, 1, &txn, true);
    // Both deliveries are settled by the listener as released, then the
    // discharge is answered.
    let mut released = Vec::new();
    for _ in 0..2 {
        let frame = recv(&mut s);
        assert_eq!(frame.1, d::DISPOSITION, "{frame:?}");
        assert_eq!(at(&frame.2, 0), V::Boolean(false), "role sender");
        assert_eq!(at(&frame.2, 3), V::Boolean(true));
        assert_eq!(
            decode_state(frame.2.get(4)).expect("a state"),
            Some(DeliveryState::Outcome(Outcome::Released))
        );
        released.push(at(&frame.2, 1));
    }
    released.sort_by_key(|v| match v {
        V::Uint(id) => *id,
        _ => u32::MAX,
    });
    assert_eq!(released, [V::Uint(ids[0]), V::Uint(ids[1])]);
    let (settled, state) = answer(&mut s, 1);
    assert!(settled);
    assert_eq!(state, Some(DeliveryState::Outcome(Outcome::Accepted)));
    assert_eq!(nodes.depth("orders"), Some(3));
    // The link acquires in no transaction now: credit without a txn-id
    // brings the messages again, in their order, without a state.
    credit(&mut s, 1, 2, 3, None);
    for body in [b"order-1", b"order-2", b"order-3"] {
        let (_, state, payload) = delivery(&mut s);
        assert_eq!(state, None);
        assert_eq!(payload, body);
    }
    close(s);
    server.shutdown();
}

/// §4.4.3, §4.4.4 acquisition: with a commit the acquisition stands and
/// an outcome of the same transaction applies: the accepted message is
/// gone, the listener settles it; a message acquired without an outcome
/// stays with the link, to be retired later.
#[test]
fn amqp10_4_4_3_a_committed_acquisition_stands_and_its_outcome_applies() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1", b"order-2"]);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_receiver(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    credit(&mut s, 1, 0, 2, Some(&txn));
    let (first, _, _) = delivery(&mut s);
    let (second, state, _) = delivery(&mut s);
    assert_eq!(state, Some(in_txn(&txn, None)));
    dispose(
        &mut s,
        first,
        first,
        false,
        &in_txn(&txn, Some(Outcome::Accepted)),
    );
    assert_eq!(discharge(&mut s, 0, 1, &txn, false), Outcome::Accepted);
    let frame = recv(&mut s);
    assert_eq!(frame.1, d::DISPOSITION, "{frame:?}");
    assert_eq!(at(&frame.2, 1), V::Uint(first));
    assert_eq!(at(&frame.2, 3), V::Boolean(true));
    assert_eq!(nodes.depth("orders"), Some(0));
    // The second is still with the link: released now, outside any
    // transaction, it goes back.
    dispose(
        &mut s,
        second,
        second,
        true,
        &DeliveryState::Outcome(Outcome::Released),
    );
    wait_depth(&nodes, "orders", 1);
    assert_eq!(
        nodes.take("orders", "test").expect("released").payload,
        b"order-2"
    );
    close(s);
    server.shutdown();
}

/// §4.4.3 with §4.5.8: a flow whose txn-id names no transaction cannot
/// set the state of the link: the link is detached with
/// amqp:transaction:unknown-id. §4.4.4: "An outcome sent by the
/// controller before the transaction has discharged MUST be associated
/// with the same transaction": an outcome outside the transaction for a
/// delivery acquired in it terminates the control link with
/// amqp:transaction:rollback, and the message is back at its node.
#[test]
fn amqp10_4_4_3_acquisition_in_an_unknown_transaction_and_an_outcome_outside_it_are_refused() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    fill(&nodes, "orders", &[b"order-1"]);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_receiver(&mut s, 1, "orders");
    let unknown = TxnId::new(&[9, 9]).expect("a txn-id");
    credit(&mut s, 1, 0, 1, Some(&unknown));
    assert_eq!(
        expect_detach(&mut s).condition,
        "amqp:transaction:unknown-id"
    );
    send(
        &mut s,
        0,
        d::DETACH,
        vec![V::Uint(1), V::Boolean(true)],
        &[],
    );
    assert_eq!(nodes.depth("orders"), Some(1), "nothing was acquired");
    // An outcome outside the transaction.
    attach_receiver(&mut s, 2, "orders");
    let txn = declare(&mut s, 0, 0);
    credit(&mut s, 2, 0, 1, Some(&txn));
    let (id, state, _) = delivery(&mut s);
    assert_eq!(state, Some(in_txn(&txn, None)));
    dispose(
        &mut s,
        id,
        id,
        false,
        &DeliveryState::Outcome(Outcome::Accepted),
    );
    // The endpoint settles the plain outcome; the control link goes.
    let mut detached = None;
    for _ in 0..3 {
        let frame = recv(&mut s);
        if frame.1 == d::DETACH {
            detached = decode_error(frame.2.get(2));
            break;
        }
    }
    assert_eq!(
        detached.expect("the control link is detached").condition,
        "amqp:transaction:rollback"
    );
    wait_depth(&nodes, "orders", 1);
    close(s);
    server.shutdown();
}

/// A controller connection whose frames are 512 octets, with a session
/// whose incoming window admits two transfer frames for now.
fn controller_two_frame_window(port: u16) -> TcpStream {
    let mut s = connect_client(port);
    s.write_all(b"AMQP\x00\x01\x00\x00").expect("header");
    let mut header = [0u8; 8];
    s.read_exact(&mut header).expect("the server's header");
    send(
        &mut s,
        0,
        d::OPEN,
        vec![V::Str("controller".into()), V::Null, V::Uint(512)],
        &[],
    );
    assert_eq!(recv(&mut s).1, d::OPEN);
    send(
        &mut s,
        0,
        d::BEGIN,
        vec![V::Null, V::Uint(0), V::Uint(2), V::Uint(1000)],
        &[],
    );
    assert_eq!(recv(&mut s).1, d::BEGIN);
    s
}

/// Credit for one message on the receiver of handle 1, in the transaction
/// `txn` (§4.4.3), the session window still at two.
fn credit_in_transaction(s: &mut TcpStream, txn: &TxnId) {
    send(
        s,
        0,
        d::FLOW,
        vec![
            V::Uint(0),
            V::Uint(2),
            V::Uint(1),
            V::Uint(1000),
            V::Uint(1),
            V::Uint(0),
            V::Uint(1),
            V::Null,
            V::Null,
            V::Null,
            flow_properties(txn),
        ],
        &[],
    );
}

/// §4.4.3: "If the discharge indicates the failure of the transaction
/// the resource MUST abort or complete sending the remainder of the
/// message before completing the discharge." The controller's session
/// window admits one transfer frame of a message of several; the
/// discharge with fail set is answered only after the window opened and
/// the listener sent the abort.
#[test]
fn amqp10_4_4_3_a_message_sent_in_part_is_aborted_before_the_rollback_is_answered() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let big: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    fill(&nodes, "orders", &[&big]);
    let server = TestServer::spawn(cfg);
    let mut s = controller_two_frame_window(server.port);
    attach_control(&mut s, 0, source_with_outcomes());
    attach_receiver(&mut s, 1, "orders");
    let txn = declare(&mut s, 0, 0);
    // Credit and the transaction, with the session window still at two.
    credit_in_transaction(&mut s, &txn);
    for _ in 0..2 {
        let frame = recv(&mut s);
        assert_eq!(frame.1, d::TRANSFER, "{frame:?}");
        assert_eq!(at(&frame.2, 5), V::Boolean(true), "more");
        assert_eq!(
            decode_state(frame.2.get(7)).expect("a state"),
            Some(in_txn(&txn, None)),
            "the state on every frame"
        );
    }
    assert_eq!(nodes.depth("orders"), Some(0));
    send_discharge(&mut s, 0, 1, &txn, true);
    assert!(
        silent(&mut s),
        "the discharge is not answered while the message is sent in part"
    );
    // The window opens: the abort, then the answer.
    send(
        &mut s,
        0,
        d::FLOW,
        vec![V::Uint(2), V::Uint(100), V::Uint(2), V::Uint(1000)],
        &[],
    );
    let abort = recv(&mut s);
    assert_eq!(abort.1, d::TRANSFER, "{abort:?}");
    assert_eq!(at(&abort.2, 9), V::Boolean(true), "aborted");
    let (settled, state) = answer(&mut s, 1);
    assert!(settled);
    assert_eq!(state, Some(DeliveryState::Outcome(Outcome::Accepted)));
    // The message is whole at its node again.
    assert_eq!(nodes.depth("orders"), Some(1));
    assert_eq!(nodes.take("orders", "test").expect("back").payload, big);
    close(s);
    server.shutdown();
}

/// A controller session whose frames are 512 octets and whose incoming
/// window admits two transfer frames, with the control link on handle 0
/// (its source `source`), a receiver from "orders" on handle 1 and a
/// sender to "inbox" on handle 2. The receiver acquires a message of
/// several frames in a transaction; the two frames the window admits
/// arrive, the rest waits. Returns the stream and the transaction.
fn acquisition_in_flight(port: u16, nodes: &Nodes, source: V) -> (TcpStream, TxnId, Vec<u8>) {
    let big: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    fill(nodes, "orders", &[&big]);
    nodes.declare("inbox");
    let mut s = controller_two_frame_window(port);
    attach_control(&mut s, 0, source);
    attach_receiver(&mut s, 1, "orders");
    attach_sender(&mut s, 2, "inbox");
    let txn = declare(&mut s, 0, 0);
    credit_in_transaction(&mut s, &txn);
    for _ in 0..2 {
        let frame = recv(&mut s);
        assert_eq!(frame.1, d::TRANSFER, "{frame:?}");
        assert_eq!(at(&frame.2, 5), V::Boolean(true), "more");
    }
    assert_eq!(nodes.depth("orders"), Some(0));
    (s, txn, big)
}

/// Opens the controller's window; the next frames must be the abort of
/// the message acquired in part and then the detach of the control link
/// with amqp:transaction:rollback. The message is whole at its node.
/// `next_outgoing`: the transfer-id of the controller's next frame.
fn abort_then_detach(s: &mut TcpStream, nodes: &Nodes, big: &[u8], next_outgoing: u32) {
    send(
        s,
        0,
        d::FLOW,
        vec![
            V::Uint(2),
            V::Uint(100),
            V::Uint(next_outgoing),
            V::Uint(1000),
        ],
        &[],
    );
    let abort = recv(s);
    assert_eq!(abort.1, d::TRANSFER, "the abort first: {abort:?}");
    assert_eq!(at(&abort.2, 9), V::Boolean(true), "aborted");
    let detach = recv(s);
    assert_eq!(detach.1, d::DETACH, "then the detach: {detach:?}");
    assert_eq!(at(&detach.2, 0), V::Uint(0), "of the control link");
    assert_eq!(
        decode_error(detach.2.get(2)).expect("an error").condition,
        "amqp:transaction:rollback"
    );
    assert_eq!(nodes.depth("orders"), Some(1));
    assert_eq!(nodes.take("orders", "test").expect("back").payload, big);
}

/// §4.4.3: "If the discharge indicates the failure of the transaction the
/// resource MUST abort or complete sending the remainder of the message
/// before completing the discharge." The control link's source has no
/// outcomes, so a refused commit is the detach of the link (§4.3). The
/// commit is refused because a delivery sent settled in the transaction
/// was not taken (§4.4.4); the detach waits until the abort of the
/// message acquired in part is sent.
#[test]
fn amqp10_4_4_3_a_refused_commit_detaches_the_control_link_after_the_abort() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let (mut s, txn, big) = acquisition_in_flight(server.port, &nodes, Source::default().encode());
    // Sent settled in the transaction, its second frame without the
    // state (§4.4.1): not taken, and the transaction cannot commit.
    transfer_on(&mut s, 0, 2, 1, true, true, Some(&in_txn(&txn, None)), b"x");
    transfer_on(&mut s, 0, 2, 1, true, false, None, b"y");
    send_discharge(&mut s, 0, 2, &txn, false);
    assert!(
        silent(&mut s),
        "no detach while the message is sent in part"
    );
    // Transfer frames 0 to 3: the declare, two frames, the discharge.
    abort_then_detach(&mut s, &nodes, &big, 4);
    assert_eq!(nodes.depth("inbox"), Some(0));
    close(s);
    server.shutdown();
}

/// §4.4.1 with §4.4.3: the control link terminated with
/// amqp:transaction:rollback for a discharge against a partial delivery
/// goes after the abort of what its transaction acquired.
#[test]
fn amqp10_4_4_3_a_control_link_terminated_for_a_partial_delivery_goes_after_the_abort() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let (mut s, txn, big) = acquisition_in_flight(server.port, &nodes, source_with_outcomes());
    transfer_on(
        &mut s,
        0,
        2,
        1,
        false,
        true,
        Some(&in_txn(&txn, None)),
        b"half",
    );
    send_discharge(&mut s, 0, 2, &txn, false);
    assert!(
        silent(&mut s),
        "no detach while the message is sent in part"
    );
    abort_then_detach(&mut s, &nodes, &big, 3);
    close(s);
    server.shutdown();
}

// ============================================================================
// §4.5.7 capabilities, §4.5.8 time-out, the configuration.
// ============================================================================

/// §4.5.7 multi-txns-per-ssn and multi-ssns-per-txn, which the
/// coordinator states: two transactions are active on one session at a
/// time and discharged apart, and a transaction declared on one session
/// takes work from a link of another session of the connection.
#[test]
fn amqp10_4_5_7_several_transactions_on_a_session_and_one_across_sessions() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let one = declare(&mut s, 0, 0);
    let two = declare(&mut s, 0, 1);
    post(&mut s, 1, 2, &one, b"in-one");
    post(&mut s, 1, 3, &two, b"in-two");
    answer(&mut s, 2);
    answer(&mut s, 3);
    // A second session of the connection posts into transaction two.
    begin(&mut s, 1);
    let (attach, flow) = attach_sender_on(
        &mut s,
        1,
        0,
        "other-session",
        Source::default().encode(),
        Target::at("orders").encode(),
    );
    assert_eq!(attach.0, 1, "on the second session");
    assert_eq!(flow.1, d::FLOW);
    transfer_on(
        &mut s,
        1,
        0,
        0,
        false,
        false,
        Some(&in_txn(&two, None)),
        b"from-session-1",
    );
    let frame = recv(&mut s);
    assert_eq!((frame.0, frame.1), (1, d::DISPOSITION), "{frame:?}");
    assert_eq!(
        decode_state(frame.2.get(4)).expect("a state"),
        Some(in_txn(&two, Some(Outcome::Accepted)))
    );
    // One rolls back, two commits.
    assert_eq!(discharge(&mut s, 0, 4, &one, true), Outcome::Accepted);
    assert_eq!(nodes.depth("orders"), Some(0));
    assert_eq!(discharge(&mut s, 0, 5, &two, false), Outcome::Accepted);
    assert_eq!(nodes.depth("orders"), Some(2));
    assert_eq!(nodes.take("orders", "t").expect("first").payload, b"in-two");
    assert_eq!(
        nodes.take("orders", "t").expect("second").payload,
        b"from-session-1"
    );
    close(s);
    server.shutdown();
}

/// §4.5.8 transaction-timeout ("The work represented by this transaction
/// took too long") with `<transactions timeout-ms>`: a transaction not
/// discharged in time is rolled back; work for it is rejected with
/// amqp:transaction:timeout, its commit likewise; its rollback is
/// accepted (§4.3: "the coordinator MUST always be able to complete a
/// discharge where the fail flag is set to true").
#[test]
fn amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout() {
    let mut cfg = test_handler_cfg();
    cfg.transactions = TxnLimits {
        max_transactions: 8,
        timeout_ms: 300,
    };
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    attach_sender(&mut s, 1, "orders");
    let commit = declare(&mut s, 0, 0);
    let rollback = declare(&mut s, 0, 1);
    post(&mut s, 1, 2, &commit, b"order-1");
    let (_, state) = answer(&mut s, 2);
    assert_eq!(state, Some(in_txn(&commit, Some(Outcome::Accepted))));
    std::thread::sleep(Duration::from_millis(600));
    post(&mut s, 1, 3, &commit, b"order-2");
    let (_, state) = answer(&mut s, 3);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:transaction:timeout"
    );
    let outcome = discharge(&mut s, 0, 4, &commit, false);
    assert_eq!(
        rejected_with(&outcome).condition,
        "amqp:transaction:timeout"
    );
    assert_eq!(discharge(&mut s, 0, 5, &rollback, true), Outcome::Accepted);
    assert_eq!(nodes.depth("orders"), Some(0));
    // A transaction discharged in time commits.
    let fresh = declare(&mut s, 0, 6);
    post(&mut s, 1, 7, &fresh, b"order-3");
    answer(&mut s, 7);
    assert_eq!(discharge(&mut s, 0, 8, &fresh, false), Outcome::Accepted);
    assert_eq!(nodes.depth("orders"), Some(1));
    close(s);
    server.shutdown();
}

/// `<transactions>` of the configuration file bounds the coordinator: a
/// declare beyond max-transactions is rejected with
/// amqp:resource-limit-exceeded until a discharge makes room; with
/// max-transactions 0 the attach of a control link is refused with
/// amqp:not-implemented.
#[test]
fn amqp10_4_2_the_transactions_element_of_the_configuration_bounds_the_coordinator() {
    let parse = |xml: &str| {
        zerodds_amqp_endpoint::config_xml::parse_config(xml)
            .expect("the configuration")
            .endpoints
            .remove(0)
            .transactions
    };
    let limits = parse(
        r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
  <endpoint name="bounded">
    <transactions max-transactions="2" timeout-ms="60000"/>
  </endpoint>
</dds-amqp>"#,
    );
    assert_eq!(
        limits,
        TxnLimits {
            max_transactions: 2,
            timeout_ms: 60_000
        }
    );
    assert_eq!(
        parse(r#"<dds-amqp><endpoint name="default"/></dds-amqp>"#),
        TxnLimits {
            max_transactions: 64,
            timeout_ms: 300_000
        }
    );
    let refused = zerodds_amqp_endpoint::config_xml::parse_config(
        r#"<dds-amqp><endpoint><transactions max="2"/></endpoint></dds-amqp>"#,
    )
    .expect_err("an unknown attribute")
    .to_string();
    assert!(refused.contains("unknown attribute max"), "{refused}");

    let mut cfg = test_handler_cfg();
    cfg.transactions = limits;
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    attach_control(&mut s, 0, source_with_outcomes());
    let first = declare(&mut s, 0, 0);
    let _second = declare(&mut s, 0, 1);
    transfer(&mut s, 0, 2, &declare_message(&Declare::default()).unwrap());
    let (_, state) = answer(&mut s, 2);
    assert_eq!(
        rejected_with(&state.expect("an outcome").outcome()).condition,
        "amqp:resource-limit-exceeded"
    );
    assert_eq!(discharge(&mut s, 0, 3, &first, true), Outcome::Accepted);
    declare(&mut s, 0, 4);
    close(s);
    server.shutdown();

    let mut cfg = test_handler_cfg();
    cfg.transactions = TxnLimits {
        max_transactions: 0,
        timeout_ms: 0,
    };
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "controller");
    send(
        &mut s,
        0,
        d::ATTACH,
        vec![
            V::Str("txn-ctl".into()),
            V::Uint(0),
            V::Boolean(false),
            V::Null,
            V::Null,
            source_with_outcomes(),
            Coordinator::default().encode(),
            V::Null,
            V::Null,
            V::Uint(0),
        ],
        &[],
    );
    let attach = recv(&mut s);
    assert_eq!(attach.1, d::ATTACH);
    assert_eq!(at(&attach.2, 6), V::Null, "no coordinator");
    assert_eq!(expect_detach(&mut s).condition, "amqp:not-implemented");
    close(s);
    server.shutdown();
}
