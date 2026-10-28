//! Integration test: all 16 submessages are packed into one message,
//! encoded and decoded. Validates the padding between
//! submessages and the outer header variant "without ClientKey".

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

use zerodds_xrce::handshake::{ClientRepresentation, XRCE_VERSION};
use zerodds_xrce::submessages::{
    AckNackPayload, CreateClientPayload, CreatePayload, DataFormat, DataPayload, DeletePayload,
    FragmentPayload, GetInfoPayload, HeartbeatPayload, InfoPayload, ReadDataPayload,
    ReadSpecification, ResetPayload, StatusAgentPayload, StatusPayload, TimePoint,
    TimestampPayload, TimestampReplyPayload, WriteDataPayload,
};
use zerodds_xrce::{
    BaseObjectRequest, ClientKey, Message, MessageHeader, ObjectId,
    SESSION_ID_NONE_WITHOUT_CLIENT_KEY, SerialNumber16, SessionId, StreamId, Submessage,
};

fn build_all_16_submessages() -> Vec<Submessage> {
    vec![
        CreateClientPayload {
            representation: ClientRepresentation {
                timestamp: None,
                version: XRCE_VERSION,
                vendor_id: [0, 0],
                client_key: ClientKey([0, 0, 0, 0]),
                session_id: 0,
                properties: None,
                requested_mtu: None,
            },
        }
        .into_submessage()
        .unwrap(),
        CreatePayload {
            representation: vec![1; 8],
            reuse: true,
            replace: false,
        }
        .into_submessage()
        .unwrap(),
        GetInfoPayload {
            representation: vec![2; 4],
        }
        .into_submessage()
        .unwrap(),
        DeletePayload {
            representation: vec![3; 4],
        }
        .into_submessage()
        .unwrap(),
        StatusAgentPayload {
            representation: vec![4; 12],
        }
        .into_submessage()
        .unwrap(),
        StatusPayload {
            representation: vec![5; 8],
        }
        .into_submessage()
        .unwrap(),
        InfoPayload {
            representation: vec![6; 8],
        }
        .into_submessage()
        .unwrap(),
        WriteDataPayload {
            base: BaseObjectRequest {
                request_id: [0x00, 0x07],
                object_id: ObjectId::from_raw(0x00D5),
            },
            serialized_data: vec![7; 4],
        }
        .into_submessage()
        .unwrap(),
        ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0x00, 0x08],
                object_id: ObjectId::from_raw(0x00D6),
            },
            read_specification: ReadSpecification {
                preferred_stream_id: 0x01,
                data_format: DataFormat::Data,
                content_filter_expression: None,
                delivery_control: None,
            },
        }
        .into_submessage()
        .unwrap(),
        DataPayload {
            base: BaseObjectRequest {
                request_id: [0x00, 0x08],
                object_id: ObjectId::from_raw(0x00D6),
            },
            serialized_data: vec![9; 4],
        }
        .into_submessage()
        .unwrap(),
        AckNackPayload {
            first_unacked_seq_num: 10,
            nack_bitmap: [0xAA, 0x55],
            stream_id: 0x80,
        }
        .into_submessage()
        .unwrap(),
        HeartbeatPayload {
            first_unacked_seq_nr: 11,
            last_unacked_seq_nr: 100,
            stream_id: 0x80,
        }
        .into_submessage()
        .unwrap(),
        ResetPayload.into_submessage().unwrap(),
        FragmentPayload {
            data: vec![13; 16],
            last_fragment: true,
        }
        .into_submessage()
        .unwrap(),
        TimestampPayload {
            transmit_timestamp: TimePoint {
                seconds: 14,
                nanoseconds: 0,
            },
        }
        .into_submessage()
        .unwrap(),
        TimestampReplyPayload {
            transmit_timestamp: TimePoint {
                seconds: 15,
                nanoseconds: 1,
            },
            receive_timestamp: TimePoint {
                seconds: 15,
                nanoseconds: 2,
            },
            originate_timestamp: TimePoint {
                seconds: 15,
                nanoseconds: 3,
            },
        }
        .into_submessage()
        .unwrap(),
    ]
}

#[test]
fn all_16_submessages_in_one_message_roundtrip() {
    let header = MessageHeader::without_client_key(
        SessionId(SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16::new(0),
    )
    .unwrap();
    let submessages = build_all_16_submessages();
    assert_eq!(submessages.len(), 16);
    let msg = Message::new(header, submessages).unwrap();
    let bytes = msg.encode().unwrap();
    let decoded = Message::decode(&bytes).unwrap();
    assert_eq!(decoded, msg);
    assert_eq!(decoded.submessages.len(), 16);
}

/// DDS-XRCE 1.0 §8.3.1: "An XRCE message is composed of a message Header
/// followed by one or more Submessages". A header alone is no message.
#[test]
fn a_message_without_a_submessage_is_refused() {
    let header = MessageHeader::without_client_key(
        SessionId(SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
        StreamId::NONE,
        SerialNumber16::new(0),
    )
    .unwrap();
    assert!(Message::new(header, vec![]).is_err());
    let hand_built = Message {
        header,
        submessages: vec![],
    };
    assert!(hand_built.encode().is_err());
    // A datagram that is only the 4-octet header.
    let mut bytes = [0u8; 4];
    let n = header.write_to(&mut bytes).unwrap();
    assert_eq!(n, 4);
    assert!(Message::decode(&bytes).is_err());
}
