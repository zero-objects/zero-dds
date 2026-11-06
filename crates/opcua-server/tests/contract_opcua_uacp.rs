// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-server` with `zerodds-opcua-uacp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-opcua-uacp`: `connection::{HelloMessage,
//! AcknowledgeMessage, MessageHeader, MessageType}` for the connection
//! handshake of `client.rs` and `server.rs`; `securechannel::split_messages`
//! to cut a TCP read into UACP messages; `securechannel::node_ids` for the
//! DefaultBinary encoding id of every service message `services.rs` writes
//! and reads; `SECURITY_POLICY_NONE` and `DATETIME_TICKS_AT_UNIX_EPOCH` for
//! the unsecured channel and the timestamps of `ResponseHeader`.
//!
//! So the crate relies on: a Hello and an Acknowledge being the octets of
//! the UACP layout; a buffer of several messages being cut at each
//! `MessageSize`; every encoding id being the one of the OPC UA nodeset;
//! the two constants being the ones of the specification.
//!
//! Oracles: OPC 10000-6 v1.05 §5.2.2 (little-endian integers, a String as
//! Int32 length and UTF-8 octets), §7.1.2.2 to §7.1.2.4 (message header,
//! Hello, Acknowledge), §6.7.2.2 (chunk type `F`), §5.2.2.5 (DateTime: 100 ns
//! intervals since 1601-01-01); OPC 10000-7 (SecurityPolicy URI of `None`);
//! `NodeIds.csv` of OPCFoundation/UA-Nodeset (the `_Encoding_DefaultBinary`
//! ids below).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_pubsub::UaReader;
use zerodds_opcua_uacp::connection::{
    AcknowledgeMessage, HelloMessage, MessageHeader, MessageType,
};
use zerodds_opcua_uacp::securechannel::{
    DATETIME_TICKS_AT_UNIX_EPOCH, SECURITY_POLICY_NONE, node_ids, split_messages,
};

/// A UACP message as §7.1.2 lays it out: three ASCII octets, `F`, the size
/// of the whole message little-endian, the body.
fn uacp(kind: &[u8; 3], body: &[u8]) -> Vec<u8> {
    let mut out = kind.to_vec();
    out.push(b'F');
    out.extend_from_slice(&u32::try_from(8 + body.len()).unwrap().to_le_bytes());
    out.extend_from_slice(body);
    out
}

fn ack_body() -> Vec<u8> {
    [0u32, 65_536, 65_536, 0, 0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

#[test]
fn uacs_7_1_2_3_a_hello_is_the_octets_of_the_layout() {
    let url = "opc.tcp://localhost:4840";
    let hello = HelloMessage {
        protocol_version: 0,
        receive_buffer_size: 65_536,
        send_buffer_size: 65_536,
        max_message_size: 0,
        max_chunk_count: 0,
        endpoint_url: url.to_string(),
    };
    let mut body = ack_body();
    body.extend_from_slice(&i32::try_from(url.len()).unwrap().to_le_bytes());
    body.extend_from_slice(url.as_bytes());
    let wire = uacp(b"HEL", &body);
    assert_eq!(hello.encode().unwrap(), wire);
    let mut r = UaReader::new(&wire);
    let header = MessageHeader::decode(&mut r).unwrap();
    assert_eq!(header.message_type, MessageType::Hello);
    assert_eq!(header.message_size as usize, wire.len());
    assert_eq!(HelloMessage::decode_body(&mut r).unwrap(), hello);
}

#[test]
fn uacs_7_1_2_4_an_acknowledge_is_read_from_the_octets_of_the_layout() {
    let wire = uacp(b"ACK", &ack_body());
    let mut r = UaReader::new(&wire);
    let header = MessageHeader::decode(&mut r).unwrap();
    assert_eq!(header.message_type, MessageType::Acknowledge);
    let ack = AcknowledgeMessage::decode_body(&mut r).unwrap();
    assert_eq!(
        ack,
        AcknowledgeMessage {
            protocol_version: 0,
            receive_buffer_size: 65_536,
            send_buffer_size: 65_536,
            max_message_size: 0,
            max_chunk_count: 0,
        }
    );
    assert_eq!(ack.encode().unwrap(), wire);
}

#[test]
fn uacs_7_1_2_2_a_buffer_of_two_messages_is_cut_at_each_message_size() {
    let first = uacp(b"ACK", &ack_body());
    let second = uacp(b"ERR", &[0x00, 0x00, 0x1f, 0x80, 0, 0, 0, 0]);
    let both = [first.clone(), second.clone()].concat();
    assert_eq!(split_messages(&both).unwrap(), [&first[..], &second[..]]);
    // A MessageSize past the end of the buffer is refused, not cut short.
    assert!(split_messages(&both[..both.len() - 1]).is_err());
}

#[test]
fn uacs_5_2_2_5_the_constants_are_the_ones_of_the_specification() {
    // 1970-01-01 is 11 644 473 600 s after 1601-01-01, in 100 ns ticks.
    assert_eq!(DATETIME_TICKS_AT_UNIX_EPOCH, 11_644_473_600 * 10_000_000);
    assert_eq!(
        SECURITY_POLICY_NONE,
        "http://opcfoundation.org/UA/SecurityPolicy#None"
    );
}

#[test]
fn the_encoding_ids_are_the_ones_of_the_nodeset() {
    for (id, number) in [
        (node_ids::ACTIVATE_SESSION_REQUEST, 467),
        (node_ids::ACTIVATE_SESSION_RESPONSE, 470),
        (node_ids::BROWSE_NEXT_REQUEST, 533),
        (node_ids::BROWSE_NEXT_RESPONSE, 536),
        (node_ids::BROWSE_REQUEST, 527),
        (node_ids::BROWSE_RESPONSE, 530),
        (node_ids::CALL_REQUEST, 712),
        (node_ids::CALL_RESPONSE, 715),
        (node_ids::CLOSE_SESSION_REQUEST, 473),
        (node_ids::CLOSE_SESSION_RESPONSE, 476),
        (node_ids::CREATE_MONITORED_ITEMS_REQUEST, 751),
        (node_ids::CREATE_MONITORED_ITEMS_RESPONSE, 754),
        (node_ids::CREATE_SESSION_REQUEST, 461),
        (node_ids::CREATE_SESSION_RESPONSE, 464),
        (node_ids::CREATE_SUBSCRIPTION_REQUEST, 787),
        (node_ids::CREATE_SUBSCRIPTION_RESPONSE, 790),
        (node_ids::DATA_CHANGE_NOTIFICATION, 811),
        (node_ids::DELETE_MONITORED_ITEMS_REQUEST, 781),
        (node_ids::DELETE_MONITORED_ITEMS_RESPONSE, 784),
        (node_ids::DELETE_SUBSCRIPTIONS_REQUEST, 847),
        (node_ids::DELETE_SUBSCRIPTIONS_RESPONSE, 850),
        (node_ids::FIND_SERVERS_REQUEST, 422),
        (node_ids::FIND_SERVERS_RESPONSE, 425),
        (node_ids::GET_ENDPOINTS_REQUEST, 428),
        (node_ids::GET_ENDPOINTS_RESPONSE, 431),
        (node_ids::HISTORY_DATA, 658),
        (node_ids::HISTORY_READ_REQUEST, 664),
        (node_ids::HISTORY_READ_RESPONSE, 667),
        (node_ids::MODIFY_MONITORED_ITEMS_REQUEST, 763),
        (node_ids::MODIFY_MONITORED_ITEMS_RESPONSE, 766),
        (node_ids::MODIFY_SUBSCRIPTION_REQUEST, 793),
        (node_ids::MODIFY_SUBSCRIPTION_RESPONSE, 796),
        (node_ids::PUBLISH_REQUEST, 826),
        (node_ids::PUBLISH_RESPONSE, 829),
        (node_ids::READ_AT_TIME_DETAILS, 655),
        (node_ids::READ_RAW_MODIFIED_DETAILS, 649),
        (node_ids::READ_REQUEST, 631),
        (node_ids::READ_RESPONSE, 634),
        (node_ids::REPUBLISH_REQUEST, 832),
        (node_ids::REPUBLISH_RESPONSE, 835),
        (node_ids::SERVICE_FAULT, 397),
        (node_ids::SET_MONITORING_MODE_REQUEST, 769),
        (node_ids::SET_MONITORING_MODE_RESPONSE, 772),
        (node_ids::SET_PUBLISHING_MODE_REQUEST, 799),
        (node_ids::SET_PUBLISHING_MODE_RESPONSE, 802),
        (node_ids::SET_TRIGGERING_REQUEST, 775),
        (node_ids::SET_TRIGGERING_RESPONSE, 778),
        (node_ids::STATUS_CHANGE_NOTIFICATION, 820),
        (node_ids::TRANSFER_SUBSCRIPTIONS_REQUEST, 841),
        (node_ids::TRANSFER_SUBSCRIPTIONS_RESPONSE, 844),
        (node_ids::WRITE_REQUEST, 673),
        (node_ids::WRITE_RESPONSE, 676),
        (node_ids::OPEN_SECURE_CHANNEL_REQUEST, 446),
        (node_ids::OPEN_SECURE_CHANNEL_RESPONSE, 449),
        (node_ids::CLOSE_SECURE_CHANNEL_REQUEST, 452),
    ] {
        assert_eq!(id, NodeId::numeric(0, number));
    }
}
