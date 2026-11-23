// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The listener over TCP as an AMQP 1.0 container (OASIS AMQP 1.0 Part 2
//! §2.4 to §2.6, Part 3 §3.3 to §3.5): one connection sends to a node,
//! another receives from it, with credit, settlement and the outcomes.
//! The client side of these tests writes its frames by hand; the tests
//! with another implementation as the client are in `proton_e2e.rs`.

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
use amqp_dds_endpoint::{read_frame, write_frame};
use common::{Frame, TestServer, connect_client, recv, send, test_handler_cfg};
use zerodds_amqp_bridge::extended_types::AmqpExtValue as V;
use zerodds_amqp_bridge::frame::FrameType;
use zerodds_amqp_bridge::performatives::{
    decode_performative, descriptor as d, encode_performative,
};
use zerodds_amqp_endpoint::terminus::{
    Outcome, Source, Target, Terminus, decode_error, decode_outcome, encode_outcome,
};

fn at(fields: &[V], i: usize) -> V {
    fields.get(i).cloned().unwrap_or(V::Null)
}

/// Header, open and begin; returns the stream with the session on
/// channel 0.
fn session(port: u16, container: &str) -> TcpStream {
    let mut s = connect_client(port);
    s.write_all(b"AMQP\x00\x01\x00\x00").expect("header");
    let mut header = [0u8; 8];
    s.read_exact(&mut header).expect("the server's header");
    assert_eq!(&header, b"AMQP\x00\x01\x00\x00");
    send(&mut s, 0, d::OPEN, vec![V::Str(container.into())], &[]);
    assert_eq!(recv(&mut s).1, d::OPEN);
    send(
        &mut s,
        0,
        d::BEGIN,
        vec![V::Null, V::Uint(0), V::Uint(1000), V::Uint(1000)],
        &[],
    );
    let begin = recv(&mut s);
    assert_eq!(begin.1, d::BEGIN);
    assert_eq!(at(&begin.2, 0), V::Ushort(0));
    s
}

/// Attaches a link this client sends on; returns the server's attach and
/// its flow.
fn attach_sender(s: &mut TcpStream, handle: u32, name: &str, target: V) -> (Frame, Frame) {
    send(
        s,
        0,
        d::ATTACH,
        vec![
            V::Str(name.into()),
            V::Uint(handle),
            V::Boolean(false),
            V::Null,
            V::Null,
            Source::default().encode(),
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

/// Attaches a link this client receives on; returns the server's attach.
fn attach_receiver(s: &mut TcpStream, handle: u32, name: &str, source: V) -> Frame {
    send(
        s,
        0,
        d::ATTACH,
        vec![
            V::Str(name.into()),
            V::Uint(handle),
            V::Boolean(true),
            V::Null,
            V::Null,
            source,
            Target::default().encode(),
        ],
        &[],
    );
    let attach = recv(s);
    assert_eq!(attach.1, d::ATTACH, "{attach:?}");
    attach
}

fn transfer(s: &mut TcpStream, handle: u32, id: u32, settled: bool, payload: &[u8]) {
    send(
        s,
        0,
        d::TRANSFER,
        vec![
            V::Uint(handle),
            V::Uint(id),
            V::Binary(id.to_be_bytes().to_vec()),
            V::Uint(0),
            V::Boolean(settled),
        ],
        payload,
    );
}

fn credit(s: &mut TcpStream, handle: u32, delivery_count: u32, link_credit: u32, drain: bool) {
    send(
        s,
        0,
        d::FLOW,
        vec![
            V::Uint(0),
            V::Uint(1000),
            V::Uint(0),
            V::Uint(1000),
            V::Uint(handle),
            V::Uint(delivery_count),
            V::Uint(link_credit),
            V::Null,
            V::Boolean(drain),
        ],
        &[],
    );
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

/// Part 2 §2.6.12 with Part 3 §3.3: a message one connection transfers to
/// a node is accepted and settled by the listener and delivered to a
/// receiver of another connection within its credit; the receiver's
/// accepted settles it.
#[test]
fn amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);

    let mut producer = session(server.port, "producer");
    let (attach, flow) = attach_sender(&mut producer, 0, "p", Target::at("orders").encode());
    assert_eq!(at(&attach.2, 2), V::Boolean(true), "the listener receives");
    let created = Target::decode(attach.2.get(6))
        .expect("target")
        .expect("a target");
    assert_eq!(created.terminus.address.as_deref(), Some("orders"));
    assert_eq!(flow.1, d::FLOW);
    let granted = at(&flow.2, 6);
    assert!(matches!(granted, V::Uint(n) if n > 0), "{granted:?}");

    transfer(&mut producer, 0, 0, false, b"first");
    transfer(&mut producer, 0, 1, false, b"second");
    for id in 0..2u32 {
        let disposition = recv(&mut producer);
        assert_eq!(disposition.1, d::DISPOSITION);
        assert_eq!(at(&disposition.2, 0), V::Boolean(true), "role receiver");
        assert_eq!(at(&disposition.2, 1), V::Uint(id));
        assert_eq!(at(&disposition.2, 3), V::Boolean(true), "settled");
        assert_eq!(decode_outcome(disposition.2.get(4)), Outcome::Accepted);
    }
    assert_eq!(nodes.depth("orders"), Some(2));

    let mut consumer = session(server.port, "consumer");
    // The consumer asks for a durable source that copies and filters; the
    // listener reports the source as created (Part 2 §2.6.3, Part 3
    // §3.5.3): not durable, expiring with the link, distribution mode
    // move, no filter in place.
    let wanted = Source {
        terminus: Terminus {
            address: Some("orders".into()),
            durable: 2,
            ..Terminus::default()
        },
        distribution_mode: Some("copy".into()),
        filter: vec![(
            "selector".into(),
            V::DescribedSymbol {
                descriptor: "apache.org:selector-filter:string".into(),
                value: Box::new(V::Str("colour = 'red'".into())),
            },
        )],
        ..Source::default()
    };
    let attach = attach_receiver(&mut consumer, 0, "c", wanted.encode());
    assert_eq!(at(&attach.2, 2), V::Boolean(false), "the listener sends");
    assert_eq!(at(&attach.2, 9), V::Uint(0), "initial-delivery-count");
    let source = Source::decode(attach.2.get(5))
        .expect("source")
        .expect("a source");
    assert_eq!(source.terminus.address.as_deref(), Some("orders"));
    assert_eq!(source.distribution_mode.as_deref(), Some("move"));
    assert!(source.filter.is_empty(), "no filter is in place");
    assert_eq!(source.terminus.durable, 0);
    assert_eq!(
        source.terminus.expiry_policy,
        zerodds_amqp_endpoint::terminus::ExpiryPolicy::LinkDetach
    );
    assert_eq!(source.default_outcome, Some(Outcome::Released));
    assert_eq!(source.outcomes.len(), 4);

    // Credit 1: one message, not the second.
    credit(&mut consumer, 0, 0, 1, false);
    let first = recv(&mut consumer);
    assert_eq!(first.1, d::TRANSFER);
    assert_eq!(first.3, b"first");
    assert_eq!(at(&first.2, 4), V::Boolean(false), "unsettled");
    let first_id = at(&first.2, 1);
    assert_eq!(nodes.depth("orders"), Some(1));
    // Accepted and settled by the receiver.
    send(
        &mut consumer,
        0,
        d::DISPOSITION,
        vec![
            V::Boolean(true),
            first_id,
            V::Null,
            V::Boolean(true),
            encode_outcome(&Outcome::Accepted),
        ],
        &[],
    );
    // One more credit: the second message.
    credit(&mut consumer, 0, 1, 1, false);
    let second = recv(&mut consumer);
    assert_eq!(second.3, b"second");
    close(consumer);
    // The second was not settled when the connection closed: back in the
    // node (the default outcome of the source, released).
    wait_depth(&nodes, "orders", 1);
    close(producer);
    server.shutdown();
}

/// Part 3 §3.4.4, §3.4.5: released puts the message back; modified with
/// delivery-failed counts the attempt in the header; with
/// undeliverable-here the message is not sent to that link again; a
/// rejected message is gone. Part 2 §2.6.7: with drain and an empty node
/// the listener consumes the credit and says so.
#[test]
fn amqp10_3_4_the_outcomes_act_on_the_node_and_drain_consumes_the_credit() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    nodes.declare("q");
    let data = zerodds_amqp_bridge::sections::MessageSection::Data(b"m".to_vec())
        .encode()
        .expect("a data section");
    nodes.put("q", Message::new(0, data.clone()));
    let server = TestServer::spawn(cfg);
    let mut c = session(server.port, "consumer");
    attach_receiver(&mut c, 0, "c", Source::at("q").encode());

    let settle = |c: &mut TcpStream, id: V, outcome: &Outcome| {
        send(
            c,
            0,
            d::DISPOSITION,
            vec![
                V::Boolean(true),
                id,
                V::Null,
                V::Boolean(true),
                encode_outcome(outcome),
            ],
            &[],
        );
    };

    // released: delivered again, unchanged.
    credit(&mut c, 0, 0, 1, false);
    let t = recv(&mut c);
    assert_eq!(t.3, data);
    settle(&mut c, at(&t.2, 1), &Outcome::Released);
    credit(&mut c, 0, 1, 1, false);
    let t = recv(&mut c);
    assert_eq!(t.3, data, "released: the same message again");

    // modified, delivery-failed: again, with header.delivery-count 1.
    settle(
        &mut c,
        at(&t.2, 1),
        &Outcome::Modified {
            delivery_failed: true,
            undeliverable_here: false,
        },
    );
    credit(&mut c, 0, 2, 1, false);
    let t = recv(&mut c);
    let (header, used) =
        zerodds_amqp_bridge::sections::MessageSection::decode(&t.3).expect("a header");
    let zerodds_amqp_bridge::sections::MessageSection::Header(V::List(fields)) = header else {
        panic!("the message begins with a header now");
    };
    assert_eq!(at(&fields, 4), V::Uint(1), "delivery-count");
    assert_eq!(&t.3[used..], data.as_slice());

    // modified, undeliverable-here: not to this link again. With drain the
    // credit is consumed: a flow with link-credit 0 and the delivery-count
    // advanced by the credit.
    settle(
        &mut c,
        at(&t.2, 1),
        &Outcome::Modified {
            delivery_failed: false,
            undeliverable_here: true,
        },
    );
    credit(&mut c, 0, 3, 5, true);
    let flow = recv(&mut c);
    assert_eq!(flow.1, d::FLOW, "{flow:?}");
    assert_eq!(at(&flow.2, 5), V::Uint(8), "delivery-count 3 + 5");
    assert_eq!(at(&flow.2, 6), V::Uint(0), "link-credit");
    assert_eq!(
        nodes.depth("q"),
        Some(1),
        "the message stays for other links"
    );

    // Another link gets it; rejected removes it.
    let attach = attach_receiver(&mut c, 1, "other", Source::at("q").encode());
    assert_eq!(at(&attach.2, 1), V::Uint(1));
    credit(&mut c, 1, 0, 1, false);
    let t = recv(&mut c);
    assert_eq!(t.1, d::TRANSFER);
    assert_eq!(at(&t.2, 0), V::Uint(1), "the handle of the second link");
    settle(&mut c, at(&t.2, 1), &Outcome::Rejected(None));
    credit(&mut c, 1, 1, 1, true);
    let flow = recv(&mut c);
    assert_eq!(flow.1, d::FLOW);
    assert_eq!(nodes.depth("q"), Some(0));
    close(c);
    server.shutdown();
}

/// Part 2 §2.6.3 with §2.8.15: an attach to an address without a node is
/// answered with an attach without the terminus and a closing detach
/// amqp:not-found. Part 3 §3.5.3: a dynamic source is a node created for
/// the link, named in the answer, deleted with the link (§3.5.10).
#[test]
fn amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created() {
    let mut cfg = test_handler_cfg();
    cfg.nodes = Arc::new(Nodes::new(false));
    let nodes = Arc::clone(&cfg.nodes);
    let server = TestServer::spawn(cfg);
    let mut c = session(server.port, "client-7");

    send(
        &mut c,
        0,
        d::ATTACH,
        vec![
            V::Str("nowhere".into()),
            V::Uint(0),
            V::Boolean(true),
            V::Null,
            V::Null,
            Source::at("missing").encode(),
            Target::default().encode(),
        ],
        &[],
    );
    let attach = recv(&mut c);
    assert_eq!(attach.1, d::ATTACH);
    assert_eq!(at(&attach.2, 5), V::Null, "no source");
    let detach = recv(&mut c);
    assert_eq!(detach.1, d::DETACH);
    assert_eq!(at(&detach.2, 1), V::Boolean(true));
    assert_eq!(
        decode_error(detach.2.get(2)).expect("an error").condition,
        "amqp:not-found"
    );
    send(
        &mut c,
        0,
        d::DETACH,
        vec![V::Uint(0), V::Boolean(true)],
        &[],
    );

    let dynamic = Source {
        terminus: Terminus {
            dynamic: true,
            ..Terminus::default()
        },
        ..Source::default()
    };
    let attach = attach_receiver(&mut c, 1, "reply", dynamic.encode());
    let created = Source::decode(attach.2.get(5))
        .expect("source")
        .expect("a source");
    assert!(created.terminus.dynamic);
    let address = created.terminus.address.expect("the address of the node");
    assert!(
        address.contains("client-7") && address.contains("reply"),
        "{address}"
    );
    assert_eq!(created.terminus.dynamic_node_properties.len(), 1);
    assert_eq!(
        created.terminus.dynamic_node_properties[0].0,
        "lifetime-policy"
    );
    assert!(nodes.exists(&address));

    // A sender on the same connection reaches the created node.
    let (_, _) = attach_sender(&mut c, 2, "to-reply", Target::at(&address).encode());
    transfer(&mut c, 2, 0, true, b"answer");
    credit(&mut c, 1, 0, 1, false);
    let t = recv(&mut c);
    assert_eq!(t.1, d::TRANSFER, "{t:?}");
    assert_eq!(t.3, b"answer");

    // The link closes: the node goes with it.
    send(
        &mut c,
        0,
        d::DETACH,
        vec![V::Uint(1), V::Boolean(true)],
        &[],
    );
    let detach = recv(&mut c);
    assert_eq!(detach.1, d::DETACH);
    assert!(!nodes.exists(&address));
    close(c);
    server.shutdown();
}

/// Part 2 §2.4.5: the listener states half its idle time-out in open,
/// sends empty frames within half the client's, and closes a connection
/// whose client is silent with an error.
#[test]
fn amqp10_2_4_5_the_listener_sends_heartbeats_and_closes_a_silent_connection() {
    let mut cfg = test_handler_cfg();
    cfg.idle_time_out = Some(Duration::from_millis(600));
    let server = TestServer::spawn(cfg);
    let mut s = connect_client(server.port);
    s.write_all(b"AMQP\x00\x01\x00\x00").expect("header");
    let mut header = [0u8; 8];
    s.read_exact(&mut header).expect("header");
    send(
        &mut s,
        0,
        d::OPEN,
        vec![
            V::Str("quiet".into()),
            V::Null,
            V::Null,
            V::Null,
            V::Uint(200),
        ],
        &[],
    );
    let open = recv(&mut s);
    assert_eq!(at(&open.2, 4), V::Uint(300), "half of 600 ms");
    // Empty frames arrive while the client is silent, then the close.
    let mut heartbeats = 0;
    let close = loop {
        let frame = read_frame(&mut s, 1 << 20).expect("a frame");
        if frame.body.is_empty() {
            assert_eq!(frame.header.channel, 0);
            heartbeats += 1;
            continue;
        }
        let (code, value, _) = decode_performative(&frame.body).expect("decode");
        assert_eq!(code, d::CLOSE);
        break value;
    };
    assert!(
        heartbeats >= 2,
        "{heartbeats} empty frames in 600 ms at 100 ms"
    );
    let V::List(fields) = close else {
        panic!("a list")
    };
    let error = decode_error(fields.first()).expect("the close carries an error");
    assert_eq!(error.condition, "amqp:resource-limit-exceeded");
    server.shutdown();
}

/// Part 2 §2.7.1 max-frame-size: "A peer that receives an oversized frame
/// MUST close the connection with the framing-error error-code." A frame
/// of the SASL type behind the AMQP header is refused the same way.
#[test]
fn amqp10_2_7_1_an_oversized_frame_closes_with_framing_error() {
    let mut cfg = test_handler_cfg();
    cfg.max_frame_size = 1024;
    let server = TestServer::spawn(cfg);
    let mut s = session(server.port, "big");
    // A frame header that states 4096 octets.
    s.write_all(&[0x00, 0x00, 0x10, 0x00, 0x02, 0x00, 0x00, 0x00])
        .expect("write");
    let close = recv(&mut s);
    assert_eq!(close.1, d::CLOSE);
    assert_eq!(
        decode_error(close.2.first()).expect("an error").condition,
        "amqp:connection:framing-error"
    );

    let mut s = session(server.port, "sasl-frame");
    let body = encode_performative(d::CLOSE, &V::List(vec![])).expect("encode");
    write_frame(&mut s, FrameType::Sasl, 0, &body).expect("write");
    let close = recv(&mut s);
    assert_eq!(
        decode_error(close.2.first()).expect("an error").condition,
        "amqp:connection:framing-error"
    );
    server.shutdown();
}

/// Header and open with `max_frame_size`, begin with `incoming_window`;
/// the session is on channel 0.
fn narrow_session(
    port: u16,
    container: &str,
    max_frame_size: u32,
    incoming_window: u32,
) -> TcpStream {
    let mut s = connect_client(port);
    s.write_all(b"AMQP\x00\x01\x00\x00").expect("header");
    let mut header = [0u8; 8];
    s.read_exact(&mut header).expect("the server's header");
    send(
        &mut s,
        0,
        d::OPEN,
        vec![V::Str(container.into()), V::Null, V::Uint(max_frame_size)],
        &[],
    );
    assert_eq!(recv(&mut s).1, d::OPEN);
    send(
        &mut s,
        0,
        d::BEGIN,
        vec![V::Null, V::Uint(0), V::Uint(incoming_window), V::Uint(1000)],
        &[],
    );
    assert_eq!(recv(&mut s).1, d::BEGIN);
    s
}

/// A flow of the client: its session window and the credit of a link.
fn window_and_credit(
    s: &mut TcpStream,
    next_incoming_id: u32,
    incoming_window: u32,
    handle: u32,
    delivery_count: u32,
    link_credit: u32,
    echo: bool,
) {
    send(
        s,
        0,
        d::FLOW,
        vec![
            V::Uint(next_incoming_id),
            V::Uint(incoming_window),
            V::Uint(0),
            V::Uint(1000),
            V::Uint(handle),
            V::Uint(delivery_count),
            V::Uint(link_credit),
            V::Null,
            V::Boolean(false),
            V::Boolean(echo),
        ],
        &[],
    );
}

/// Whether the server stays silent for 300 ms.
fn silent(s: &mut TcpStream) -> bool {
    s.set_read_timeout(Some(Duration::from_millis(300)))
        .expect("timeout");
    let mut one = [0u8; 1];
    let quiet = matches!(
        s.peek(&mut one),
        Err(ref e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
    );
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    quiet
}

/// Part 2 §2.5.6 with §2.6.14, over TCP: a message of five transfer frames
/// for a client whose incoming window is two arrives two frames at a time,
/// each time the client's flow opens the window, and the message behind it
/// in the node follows as the next delivery.
#[test]
fn amqp10_2_5_6_the_listener_sends_a_message_above_the_window_in_steps() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    nodes.declare("big");
    let message: Vec<u8> = (0..2200u32).map(|i| (i % 253) as u8).collect();
    nodes.put("big", Message::new(0, message.clone()));
    nodes.put("big", Message::new(0, b"after".to_vec()));
    let server = TestServer::spawn(cfg);
    let mut c = narrow_session(server.port, "narrow", 512, 2);
    attach_receiver(&mut c, 0, "c", Source::at("big").encode());
    window_and_credit(&mut c, 0, 2, 0, 0, 2, false);

    let mut frames = Vec::new();
    for _ in 0..2 {
        let t = recv(&mut c);
        assert_eq!(t.1, d::TRANSFER, "{t:?}");
        frames.push(t);
    }
    assert!(silent(&mut c), "the window of two frames is used up");
    assert_eq!(nodes.depth("big"), Some(1), "the second message waits");
    // The client has taken two frames and takes two more.
    window_and_credit(&mut c, 2, 2, 0, 0, 2, false);
    for _ in 0..2 {
        frames.push(recv(&mut c));
    }
    assert!(silent(&mut c));
    // Room for the rest and for the next message.
    window_and_credit(&mut c, 4, 10, 0, 0, 2, false);
    frames.push(recv(&mut c));
    assert_eq!(frames.len(), 5);
    let mut whole = Vec::new();
    for (i, (_, code, fields, payload)) in frames.iter().enumerate() {
        assert_eq!(*code, d::TRANSFER);
        assert_eq!(at(fields, 1), at(&frames[0].2, 1), "one delivery-id");
        assert_eq!(at(fields, 5), V::Boolean(i < 4), "more on all but the last");
        whole.extend_from_slice(payload);
    }
    assert_eq!(whole, message);
    let next = recv(&mut c);
    assert_eq!(next.1, d::TRANSFER);
    assert_ne!(at(&next.2, 1), at(&frames[0].2, 1), "the next delivery");
    assert_eq!(next.3, b"after");
    close(c);
    server.shutdown();
}

/// Part 2 §2.7.4 available, over TCP: a client that grants no credit and
/// asks for the flow state is told how many messages of the node wait.
#[test]
fn amqp10_2_7_4_the_listener_states_the_messages_that_wait_for_credit() {
    let cfg = test_handler_cfg();
    let nodes = Arc::clone(&cfg.nodes);
    nodes.declare("stock");
    for body in [b"1", b"2", b"3"] {
        nodes.put("stock", Message::new(0, body.to_vec()));
    }
    let server = TestServer::spawn(cfg);
    let mut c = session(server.port, "asker");
    attach_receiver(&mut c, 0, "c", Source::at("stock").encode());
    window_and_credit(&mut c, 0, 1000, 0, 0, 0, true);
    let flow = recv(&mut c);
    assert_eq!(flow.1, d::FLOW, "{flow:?}");
    assert_eq!(at(&flow.2, 6), V::Uint(0), "link-credit");
    assert_eq!(at(&flow.2, 7), V::Uint(3), "available");
    // One message taken: two wait.
    window_and_credit(&mut c, 0, 1000, 0, 0, 1, false);
    assert_eq!(recv(&mut c).1, d::TRANSFER);
    window_and_credit(&mut c, 1, 1000, 0, 1, 0, true);
    let flow = recv(&mut c);
    assert_eq!(at(&flow.2, 7), V::Uint(2), "available");
    close(c);
    server.shutdown();
}

/// Part 1 §1.6.24 with Part 2 §2.6.3, over TCP: an attach to a missing
/// node whose source carries an empty symbol array as its capabilities is
/// answered with attach and a closing detach amqp:not-found; the
/// connection is usable afterwards.
#[test]
fn amqp10_2_6_3_a_refused_attach_with_an_empty_array_leaves_the_connection_usable() {
    let mut cfg = test_handler_cfg();
    cfg.nodes = Arc::new(Nodes::new(false));
    cfg.nodes.declare("here");
    let server = TestServer::spawn(cfg);
    let mut c = session(server.port, "arrays");
    let mut fields = vec![V::Null; 11];
    fields[0] = V::Str("missing".into());
    fields[10] = V::Array(Vec::new());
    let source = V::Described {
        descriptor: 0x28,
        value: Box::new(V::List(fields)),
    };
    let mut body = encode_performative(
        d::ATTACH,
        &V::List(vec![
            V::Str("nowhere".into()),
            V::Uint(0),
            V::Boolean(true),
            V::Null,
            V::Null,
            source,
            Target::at("mine").encode(),
        ]),
    )
    .expect("encode");
    // The array as a client writes it, with the constructor of a symbol.
    let array = body
        .windows(4)
        .position(|w| w == [0xe0, 0x02, 0x00, 0x40])
        .expect("the empty array");
    body[array + 3] = 0xa3;
    write_frame(&mut c, FrameType::Amqp, 0, &body).expect("write");
    let attach = recv(&mut c);
    assert_eq!(attach.1, d::ATTACH, "{attach:?}");
    assert_eq!(at(&attach.2, 5), V::Null, "no source was created");
    let detach = recv(&mut c);
    assert_eq!(detach.1, d::DETACH);
    assert_eq!(
        decode_error(detach.2.get(2)).expect("an error").condition,
        "amqp:not-found"
    );
    send(
        &mut c,
        0,
        d::DETACH,
        vec![V::Uint(0), V::Boolean(true)],
        &[],
    );
    let attach = attach_receiver(&mut c, 1, "next", Source::at("here").encode());
    assert!(Source::decode(attach.2.get(5)).expect("source").is_some());
    close(c);
    server.shutdown();
}

/// DDS-AMQP 1.0 Annex A `ResourceLimits` and the `<nodes>` element, as the
/// daemon takes them from its configuration file, with AMQP 1.0 Part 2
/// §2.7.1 and §2.7.2: `open` states max-frame-size, channel-max (sessions
/// minus one) and half the idle time-out, `begin` states handle-max (links
/// minus one); a channel or handle above them closes the connection with
/// amqp:connection:framing-error; one connection more than
/// max-connections is closed when it is accepted. §2.8.15: an attach that
/// would create one node more than max-nodes is refused with
/// amqp:resource-limit-exceeded, and a transfer to a node that holds
/// max-messages is rejected with it.
#[test]
fn amqp10_2_7_1_the_limits_of_the_configuration_file_bound_the_listener() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let xml = r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
  <endpoint name="limited">
    <limits max-connections="2" max-sessions-per-connection="3"
            max-links-per-session="5" max-frame-size="4096" idle-timeout-ms="8000"/>
    <nodes max-nodes="1" max-messages="2" max-octets="1000"/>
  </endpoint>
</dds-amqp>"#;
    let parsed = zerodds_amqp_endpoint::config_xml::parse_config(xml).expect("the configuration");
    let endpoint = &parsed.endpoints[0];
    let mut cfg = amqp_dds_endpoint::ServerConfig::default_listen();
    cfg.apply_limits(&endpoint.limits).expect("limits");
    cfg.nodes = Arc::new(Nodes::with_limits(true, endpoint.nodes));
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port();
    cfg.listen_addr = format!("127.0.0.1:{port}");
    let shutdown = Arc::new(AtomicBool::new(false));
    let server = {
        let shutdown = Arc::clone(&shutdown);
        let metrics = Arc::new(zerodds_amqp_endpoint::MetricsHub::new());
        std::thread::spawn(move || amqp_dds_endpoint::run_server(cfg, metrics, shutdown))
    };
    let connect = || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(s) => {
                    s.set_read_timeout(Some(Duration::from_secs(5)))
                        .expect("timeout");
                    break s;
                }
                Err(e) => {
                    assert!(std::time::Instant::now() < deadline, "no listener: {e}");
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    };
    let open = |s: &mut TcpStream, container: &str| {
        s.write_all(b"AMQP\x00\x01\x00\x00").expect("header");
        let mut header = [0u8; 8];
        s.read_exact(&mut header).expect("the server's header");
        send(s, 0, d::OPEN, vec![V::Str(container.into())], &[]);
        let open = recv(s);
        assert_eq!(open.1, d::OPEN);
        open
    };
    let begin = |s: &mut TcpStream, channel: u16| {
        send(
            s,
            channel,
            d::BEGIN,
            vec![V::Null, V::Uint(0), V::Uint(1000), V::Uint(1000)],
            &[],
        );
        recv(s)
    };

    let mut first = connect();
    let answer = open(&mut first, "first");
    assert_eq!(at(&answer.2, 2), V::Uint(4096), "max-frame-size");
    assert_eq!(at(&answer.2, 3), V::Ushort(2), "channel-max: 3 sessions");
    assert_eq!(at(&answer.2, 4), V::Uint(4000), "half of idle-timeout-ms");
    let answer = begin(&mut first, 0);
    assert_eq!(answer.1, d::BEGIN);
    assert_eq!(at(&answer.2, 4), V::Uint(4), "handle-max: 5 links");

    // The nodes: one created node, two messages in it.
    let (_, _) = attach_sender(&mut first, 0, "to-a", Target::at("a").encode());
    send(
        &mut first,
        0,
        d::ATTACH,
        vec![
            V::Str("to-b".into()),
            V::Uint(1),
            V::Boolean(false),
            V::Null,
            V::Null,
            Source::default().encode(),
            Target::at("b").encode(),
            V::Null,
            V::Null,
            V::Uint(0),
        ],
        &[],
    );
    assert_eq!(recv(&mut first).1, d::ATTACH);
    let detach = recv(&mut first);
    assert_eq!(detach.1, d::DETACH);
    assert_eq!(
        decode_error(detach.2.get(2)).expect("an error").condition,
        "amqp:resource-limit-exceeded"
    );
    for id in 0..3u32 {
        transfer(&mut first, 0, id, false, b"0123456789");
    }
    for id in 0..3u32 {
        let disposition = recv(&mut first);
        assert_eq!(disposition.1, d::DISPOSITION, "{disposition:?}");
        assert_eq!(at(&disposition.2, 1), V::Uint(id));
        match decode_outcome(disposition.2.get(4)) {
            Outcome::Accepted => assert!(id < 2, "delivery {id} accepted"),
            Outcome::Rejected(Some(error)) => {
                assert_eq!(id, 2);
                assert_eq!(error.condition, "amqp:resource-limit-exceeded");
            }
            other => panic!("delivery {id}: {other:?}"),
        }
    }

    // Two connections are served; the third is closed before a header.
    let mut second = connect();
    open(&mut second, "second");
    let mut third = connect();
    let _ = third.write_all(b"AMQP\x00\x01\x00\x00");
    let mut octet = [0u8; 1];
    assert!(
        matches!(third.read(&mut octet), Ok(0) | Err(_)),
        "the third connection got an answer"
    );

    // Channel 3 is above channel-max 2.
    let close_frame = begin(&mut second, 3);
    assert_eq!(close_frame.1, d::CLOSE, "{close_frame:?}");
    assert_eq!(
        decode_error(close_frame.2.first())
            .expect("an error")
            .condition,
        "amqp:connection:framing-error"
    );
    // Handle 5 is above handle-max 4.
    send(
        &mut first,
        0,
        d::ATTACH,
        vec![
            V::Str("sixth".into()),
            V::Uint(5),
            V::Boolean(false),
            V::Null,
            V::Null,
            Source::default().encode(),
            Target::at("a").encode(),
            V::Null,
            V::Null,
            V::Uint(0),
        ],
        &[],
    );
    let close_frame = recv(&mut first);
    assert_eq!(close_frame.1, d::CLOSE, "{close_frame:?}");
    assert_eq!(
        decode_error(close_frame.2.first())
            .expect("an error")
            .condition,
        "amqp:connection:framing-error"
    );
    shutdown.store(true, Ordering::Relaxed);
    server
        .join()
        .expect("join")
        .expect("the server ended cleanly");
}
