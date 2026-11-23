// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xrce-agent` with `zerodds-xrce-client`
//! (internal/release/rc8-test-suite.md, "Crate tier"). Built with the
//! `daemon` feature (`required-features` in `Cargo.toml`): only the daemon's
//! federation (`src/daemon/federation.rs`, DDS-XRCE 1.0 §10.4) links it.
//!
//! What the daemon uses of `zerodds-xrce-client`: `XrceClient` with
//! `set_properties`, `set_requested_mtu`, `set_forward_replies`,
//! `set_session_id`, `reset_session`, `connect`, `forward`, `poll_held`,
//! `holds_input`, `tick`, `request_get_info` and `disconnect`;
//! `ClientEvent::{Connected, ConnectRejected, Forwarded, FragmentDiscarded}`;
//! the transports `discovery::UdpTransport` and `discovery::TcpTransport`.
//!
//! So the daemon relies on: the CREATE_CLIENT carrying the ClientKey,
//! session_id and properties it is given (§7.8.2.1), a forwarded submessage
//! leaving with its bytes on the stream it is given, with the next sequence
//! number of that stream (§8.3.2.3), a second session with another
//! session_id starting its streams at 0, and a reliable stream it holds
//! acknowledging only the messages it handed over (§8.3.5.11). Oracle: DDS-XRCE 1.0 §8.3.2 (the
//! message header; a session_id above 127 carries no ClientKey), §7.8.2.1
//! and Annex A (`CLIENT_Representation`), decoded with the `zerodds-xrce`
//! codec from the datagrams a plain UDP socket receives.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use zerodds_xrce::header::ClientKey;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::BaseObjectRequest;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::submessages::{CreateClientPayload, WriteDataPayload};
use zerodds_xrce::{Message, SubmessageId};
use zerodds_xrce_client::XrceClient;
use zerodds_xrce_client::discovery::UdpTransport;

fn receive(socket: &UdpSocket) -> Message {
    receive_from(socket).0
}

fn receive_from(socket: &UdpSocket) -> (Message, std::net::SocketAddr) {
    let mut buf = [0u8; 2048];
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        match socket.recv_from(&mut buf) {
            Ok((n, peer)) => return (Message::decode(&buf[..n]).unwrap(), peer),
            Err(_) if Instant::now() < end => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => panic!("nothing received: {e}"),
        }
    }
}

#[test]
fn xrce_7_8_2_1_the_relay_session_carries_key_session_and_properties_and_forwards_unchanged() {
    let agent = UdpSocket::bind("127.0.0.1:0").unwrap();
    agent.set_nonblocking(true).unwrap();
    let transport = UdpTransport::connect(agent.local_addr().unwrap()).unwrap();
    let key = ClientKey([0xC0, 0x47, 0x00, 0x01]);
    let mut client = XrceClient::new(key, 0x81, transport);
    let property = zerodds_xrce::handshake::Property {
        name: "relay".into(),
        value: "R1".into(),
    };
    client.set_properties(Some(vec![property.clone()]));
    client.connect([0x01, 0x00]).unwrap();
    let hello = receive(&agent);
    assert_eq!(
        hello.header.session_id.0, 0x80,
        "SESSION_ID_NONE without a ClientKey"
    );
    assert!(hello.header.client_key.is_none());
    let rep = CreateClientPayload::try_from_submessage(&hello.submessages[0])
        .unwrap()
        .representation;
    assert_eq!(rep.client_key, key);
    assert_eq!(rep.session_id, 0x81);
    assert_eq!(rep.properties, Some(vec![property]));

    // Without a session, forward is refused, nothing is sent.
    let write = WriteDataPayload {
        base: BaseObjectRequest {
            request_id: [0xAB, 0xCD],
            object_id: ObjectId::new(0x0007, ObjectKind::DataWriter).unwrap(),
        },
        serialized_data: vec![1, 2, 3, 4],
    }
    .into_submessage()
    .unwrap();
    assert!(client.forward(0x80, write.clone()).is_err());

    // A second session with another session_id starts from scratch.
    client.reset_session();
    client.set_session_id(0x82).unwrap();
    client.connect([0x01, 0x00]).unwrap();
    let (again, peer) = receive_from(&agent);
    let rep = CreateClientPayload::try_from_submessage(&again.submessages[0])
        .unwrap()
        .representation;
    assert_eq!(rep.session_id, 0x82);
    assert_eq!(
        again.submessages[0].header.submessage_id,
        SubmessageId::CreateClient
    );

    // The agent accepts (STATUS_AGENT with a ResultStatus in front of the
    // AGENT_Representation, §8.3.5.5): the session is open.
    let mut body = zerodds_xrce::object_info::ResultStatus::ok()
        .encode()
        .to_vec();
    body.extend_from_slice(
        &zerodds_xrce::handshake::AgentRepresentation {
            timestamp: None,
            version: [0x01, 0x00],
            vendor_id: [0, 0],
            properties: None,
        }
        .encode()
        .unwrap(),
    );
    let accept = Message {
        header: zerodds_xrce::header::MessageHeader {
            session_id: zerodds_xrce::header::SessionId(0x82),
            stream_id: zerodds_xrce::header::StreamId::NONE,
            sequence_nr: zerodds_xrce::serial_number::SerialNumber16::new(0),
            client_key: None,
        },
        submessages: vec![
            zerodds_xrce::submessages::StatusAgentPayload {
                representation: body,
            }
            .into_submessage()
            .unwrap(),
        ],
    };
    agent.send_to(&accept.encode().unwrap(), peer).unwrap();
    let wait = Instant::now();
    while client.state() != zerodds_xrce_client::ClientState::Connected {
        let _ = client.poll(Duration::ZERO).unwrap();
        assert!(wait.elapsed() < Duration::from_secs(2), "no session");
        std::thread::sleep(Duration::from_millis(5));
    }
    client.set_forward_replies(true);
    client.forward(0x80, write.clone()).unwrap();
    client.forward(0x80, write.clone()).unwrap();
    client.forward(0x05, write.clone()).unwrap();
    let sent: Vec<Message> = (0..3).map(|_| receive(&agent)).collect();
    for m in &sent {
        assert_eq!(m.header.session_id.0, 0x82);
        assert_eq!(m.submessages, vec![write.clone()], "the bytes unchanged");
    }
    let streams: Vec<(u8, u16)> = sent
        .iter()
        .map(|m| (m.header.stream_id.0, m.header.sequence_nr.raw()))
        .collect();
    assert_eq!(streams, vec![(0x80, 0), (0x80, 1), (0x05, 0)]);
}

/// What the relay relies on when its client has no room (DDS-XRCE 1.0
/// §8.4.14, §10.4): `poll_held` hands over no more answers of a reliable
/// stream than the caller has room for, the ACKNACK to the agent's HEARTBEAT
/// names the first held message as the first unacknowledged one and asks for
/// none of them again (§8.3.5.11), `holds_input` says that it holds, and the
/// poll that hands them over sends the acknowledgement at once.
#[test]
fn xrce_8_4_14_2_a_held_stream_leaves_the_agents_window_full() {
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::object_info::{BaseObjectReply, ResultStatus};
    use zerodds_xrce::serial_number::SerialNumber16;
    use zerodds_xrce::submessages::{AckNackPayload, HeartbeatPayload, StatusPayload};
    use zerodds_xrce_client::{ClientEvent, ClientState};

    let agent = UdpSocket::bind("127.0.0.1:0").unwrap();
    agent.set_nonblocking(true).unwrap();
    let transport = UdpTransport::connect(agent.local_addr().unwrap()).unwrap();
    let mut client = XrceClient::new(ClientKey([0xC0, 0x47, 0x00, 0x02]), 0x81, transport);
    client.connect([0x01, 0x00]).unwrap();
    let (_, peer) = receive_from(&agent);
    let header = |stream: u8, seq: u16| MessageHeader {
        session_id: SessionId(0x81),
        stream_id: StreamId(stream),
        sequence_nr: SerialNumber16::new(seq),
        client_key: None,
    };
    let mut accept = zerodds_xrce::object_info::ResultStatus::ok()
        .encode()
        .to_vec();
    accept.extend_from_slice(
        &zerodds_xrce::handshake::AgentRepresentation {
            timestamp: None,
            version: [0x01, 0x00],
            vendor_id: [0, 0],
            properties: None,
        }
        .encode()
        .unwrap(),
    );
    let send = |m: Message| {
        agent.send_to(&m.encode().unwrap(), peer).unwrap();
    };
    send(Message {
        header: header(0, 0),
        submessages: vec![
            zerodds_xrce::submessages::StatusAgentPayload {
                representation: accept,
            }
            .into_submessage()
            .unwrap(),
        ],
    });
    let wait = Instant::now();
    while client.state() != ClientState::Connected {
        let _ = client.poll(Duration::ZERO).unwrap();
        assert!(wait.elapsed() < Duration::from_secs(2), "no session");
        std::thread::sleep(Duration::from_millis(5));
    }
    client.set_forward_replies(true);

    let status = |request: u8| {
        StatusPayload {
            representation: BaseObjectReply {
                related_request: BaseObjectRequest {
                    request_id: [0, request],
                    object_id: ObjectId::new(0x0007, ObjectKind::DataReader).unwrap(),
                },
                result: ResultStatus::ok(),
            }
            .encode(),
        }
        .into_submessage()
        .unwrap()
    };
    for seq in 0..4u16 {
        send(Message {
            header: header(0x80, seq),
            submessages: vec![status(seq as u8)],
        });
    }
    // Room for one: the other three stay.
    std::thread::sleep(Duration::from_millis(100));
    let handed = client.poll_held(Duration::ZERO, &mut |_| 1).unwrap();
    assert!(client.holds_input());
    assert_eq!(
        handed,
        vec![ClientEvent::Forwarded {
            stream_id: 0x80,
            submessage: status(0),
        }]
    );

    let acknack_of = |m: &Message| {
        m.submessages
            .iter()
            .find(|sm| sm.header.submessage_id == SubmessageId::AckNack)
            .map(|sm| AckNackPayload::try_from_submessage(sm).unwrap())
    };
    send(Message {
        header: header(0, 0),
        submessages: vec![
            HeartbeatPayload {
                first_unacked_seq_nr: 0,
                last_unacked_seq_nr: 3,
                stream_id: 0x80,
            }
            .into_submessage()
            .unwrap(),
        ],
    });
    let wait = Instant::now();
    let ack = loop {
        let _ = client.poll_held(Duration::ZERO, &mut |_| 0).unwrap();
        let mut buf = [0u8; 2048];
        if let Ok((n, _)) = agent.recv_from(&mut buf)
            && let Some(ack) = acknack_of(&Message::decode(&buf[..n]).unwrap())
        {
            break ack;
        }
        assert!(wait.elapsed() < Duration::from_secs(2), "no ACKNACK");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(ack.stream_id, 0x80);
    assert_eq!(ack.first_unacked_seq_num, 1, "the first held message");
    assert_eq!(ack.nack_bitmap, [0, 0], "none of the held asked for again");
    assert!(client.holds_input());

    let events = client.poll_held(Duration::ZERO, &mut |_| 16).unwrap();
    assert_eq!(
        events,
        (1..4u8)
            .map(|r| ClientEvent::Forwarded {
                stream_id: 0x80,
                submessage: status(r),
            })
            .collect::<Vec<_>>()
    );
    assert!(!client.holds_input());
    let ack = acknack_of(&receive(&agent)).expect("the acknowledgement");
    assert_eq!((ack.first_unacked_seq_num, ack.nack_bitmap), (4, [0, 0]));
}
