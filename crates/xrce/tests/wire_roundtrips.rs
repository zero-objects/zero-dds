//! Integrationstest: alle 16 Submessages werden in eine Message
//! gepackt, encodiert und decodiert. Validiert das Padding zwischen
//! Submessages und die Outer-Header-Variante "ohne ClientKey".

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

use zerodds_xrce::submessages::{
    AckNackPayload, CreateClientPayload, CreatePayload, DataFormat, DataPayload, DeletePayload,
    FragmentPayload, GetInfoPayload, HeartbeatPayload, InfoPayload, ReadDataPayload, ResetPayload,
    StatusAgentPayload, StatusPayload, TimePoint, TimestampPayload, TimestampReplyPayload,
    WriteDataPayload,
};
use zerodds_xrce::{
    Message, MessageHeader, SESSION_ID_NONE_WITHOUT_CLIENT_KEY, SerialNumber16, SessionId,
    StreamId, Submessage,
};

fn build_all_16_submessages() -> Vec<Submessage> {
    vec![
        CreateClientPayload {
            representation: vec![b'X', b'R', b'C', b'E', 1, 0],
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
            representation: vec![7; 4],
            data_format: DataFormat::Data,
        }
        .into_submessage()
        .unwrap(),
        ReadDataPayload {
            representation: vec![8; 4],
        }
        .into_submessage()
        .unwrap(),
        DataPayload {
            representation: vec![9; 4],
            data_format: DataFormat::PackedSamples,
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

#[test]
fn empty_message_with_no_submessages_roundtrip() {
    let header = MessageHeader::without_client_key(
        SessionId(SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
        StreamId::NONE,
        SerialNumber16::new(0),
    )
    .unwrap();
    let msg = Message::new(header, vec![]).unwrap();
    let bytes = msg.encode().unwrap();
    assert_eq!(bytes.len(), 4); // nur Header
    let decoded = Message::decode(&bytes).unwrap();
    assert_eq!(decoded, msg);
    assert!(decoded.submessages.is_empty());
}
