// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The transaction controller of this crate (`txn::TxnClient`) against the
//! scripted broker of `support/scripted_broker.rs`, which sends what a
//! real broker sends only now and then: a flow that asks a link of the
//! client to drain, and flows that ask for the state of a link or of the
//! session (OASIS AMQP 1.0 Part 2 §2.6.7, §2.7.4).

#![cfg(feature = "std")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "support/scripted_broker.rs"]
mod scripted_broker;

use std::time::Duration;

use scripted_broker::{Broker, Frame, STEP, accepted};
use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::performatives::descriptor;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_endpoint::broker::{ConnectParams, SaslCredentials};
use zerodds_amqp_endpoint::transactions::TxnCapability;
use zerodds_amqp_endpoint::txn::TxnClient;

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

/// The `declared` outcome of Part 4 §4.5.3 with the txn-id `id`.
fn declared(id: &[u8]) -> AmqpExtValue {
    AmqpExtValue::Described {
        descriptor: 0x33,
        value: Box::new(AmqpExtValue::List(vec![AmqpExtValue::Binary(id.to_vec())])),
    }
}

/// The next frame, a flow of the link with the client's handle `handle`:
/// its delivery-count, link-credit, `available` and `drain`.
fn link_flow(conn: &mut scripted_broker::Conn, handle: u32) -> (u32, u32, Option<u32>, bool) {
    let f: Frame = conn.next();
    assert_eq!(f.code, descriptor::FLOW, "{f:?}");
    assert_eq!(f.uint(4), Some(handle), "{f:?}");
    assert_ne!(f.boolean(9), Some(true), "an answer asks for no echo");
    (
        f.uint(5).unwrap(),
        f.uint(6).unwrap(),
        f.uint(7),
        f.boolean(8).unwrap_or(false),
    )
}

/// Part 2 §2.6.7: a sending link of the client asked to drain with no
/// message waiting consumes its credit and sends its flow state; one whose
/// message waits for credit sends it first and drains the credit left.
/// Part 2 §2.7.4: a flow with `echo` is answered with the state of the
/// sending link, of the receiving link, and with no handle of the session.
#[test]
fn amqp10_2_6_7_the_txn_client_drains_its_sending_link_and_answers_echoes() {
    let broker = Broker::bind();
    let port = broker.port;
    let client = std::thread::spawn(move || {
        let mut c = TxnClient::connect(
            &params(port),
            &[TxnCapability::LocalTransactions],
            Duration::from_secs(10),
        )
        .expect("the controller connects");
        let sender = c.sender("q").expect("the sending link");
        c.receiver("r").expect("the receiving link");
        // The broker drains the sending link while the declare waits for
        // its answer, and asks for echoes while the commit waits.
        let txn = c.declare().expect("the declare");
        c.commit(&txn).expect("the commit");
        // A message that waits for credit while the broker asks to drain.
        let message = MessageSection::Data(b"m".to_vec()).encode().unwrap();
        c.send_settled(sender, &message, None)
            .expect("the message once credit came");
        c.close();
    });
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let begin = conn.next_of(descriptor::BEGIN);
    conn.answer_begin(&begin);
    let attach = conn.next_of(descriptor::ATTACH);
    let control = conn.answer_attach(&attach, 0);
    conn.flow(&control, 0, 10);
    let attach = conn.next_of(descriptor::ATTACH);
    let snd = conn.answer_attach(&attach, 0);
    assert!(snd.daemon_sends);
    conn.flow(&snd, 0, 3);
    let attach = conn.next_of(descriptor::ATTACH);
    let rcv = conn.answer_attach(&attach, 0);
    assert!(!rcv.daemon_sends);

    // The declare: the drain of a link with nothing to send.
    let declare = conn.next_of(descriptor::TRANSFER);
    assert_eq!(declare.uint(0), Some(control.handle));
    conn.flow_with(&snd, 0, 3, true, false);
    assert_eq!(
        link_flow(&mut conn, snd.handle),
        (3, 0, Some(0), true),
        "delivery-count advanced by the credit, link-credit 0, available 0, drain"
    );
    let id = declare.uint(1).unwrap();
    conn.disposition(0, true, id, id, true, declared(b"t1"));

    // The commit: echoes of both links and of the session.
    let discharge = conn.next_of(descriptor::TRANSFER);
    conn.flow_with(&snd, 3, 0, false, true);
    assert_eq!(link_flow(&mut conn, snd.handle), (3, 0, Some(0), false));
    conn.flow_with(&rcv, 0, 0, false, true);
    let (count, credit, available, _) = link_flow(&mut conn, rcv.handle);
    assert_eq!((count, credit), (0, 0), "the receiver's state");
    assert_eq!(available, None, "only the sender states available");
    conn.session_echo(0);
    let session = conn.next();
    assert_eq!(session.code, descriptor::FLOW, "{session:?}");
    assert_eq!(session.field(4), &AmqpExtValue::Null, "no handle");
    let id = discharge.uint(1).unwrap();
    conn.disposition(0, true, id, id, true, accepted());

    // The message: it waits for credit (the client reads the flow only
    // while it waits for it); the broker grants 2 and asks to drain. The
    // message goes out, then the credit left is drained.
    conn.flow_with(&snd, 3, 2, true, false);
    let t = conn.next();
    assert_eq!(t.code, descriptor::TRANSFER, "{t:?}");
    assert_eq!(t.uint(0), Some(snd.handle));
    assert_eq!(
        link_flow(&mut conn, snd.handle),
        (5, 0, Some(0), true),
        "one delivery, then the credit left drained"
    );
    client.join().expect("the client thread");
}
