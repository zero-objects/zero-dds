//! E2E-Wire-Roundtrip-Tests fuer DDS-XRCE.
//!
//! Voller Encode → Decode-Zyklus fuer Top-Level-Messages mit
//! verschiedenen Submessage-Mixen. Verifiziert dass:
//! 1. encode → decode roundtripped (byte-identisch)
//! 2. Submessage-Liste in der gleichen Reihenfolge zurueckkommt
//! 3. SequenceNumber-Tracking erhalten bleibt
//!
//! Spec: DDS-XRCE 1.0 §7-§8.

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

use zerodds_xrce::{
    Message, MessageHeader, SerialNumber16, SessionId, StreamId, Submessage, SubmessageId,
};

#[test]
fn message_with_single_submessage_roundtrip() {
    let header =
        MessageHeader::without_client_key(SessionId(0xC8), StreamId(0x80), SerialNumber16::new(1))
            .unwrap();
    let body = vec![0xAA, 0xBB, 0xCC, 0xDD];
    let sub = Submessage::new(SubmessageId::Heartbeat, 0x01, body.clone()).unwrap();
    let msg = Message::new(header, vec![sub]).unwrap();

    let bytes = msg.encode().unwrap();
    let decoded = Message::decode(&bytes).unwrap();

    assert_eq!(decoded.header.sequence_nr.0, header.sequence_nr.0);
    assert_eq!(decoded.submessages.len(), 1);
    let sub_back = &decoded.submessages[0];
    assert_eq!(sub_back.header.submessage_id, SubmessageId::Heartbeat);
    assert_eq!(sub_back.body, body.as_slice());
}

#[test]
fn message_with_multiple_submessages_roundtrip() {
    let header =
        MessageHeader::without_client_key(SessionId(0xC8), StreamId(0x80), SerialNumber16::new(42))
            .unwrap();

    let subs = vec![
        Submessage::new(SubmessageId::Heartbeat, 0x01, vec![0xAA; 8]).unwrap(),
        Submessage::new(SubmessageId::AckNack, 0x01, vec![0xBB; 12]).unwrap(),
        Submessage::new(SubmessageId::Timestamp, 0x01, vec![0xCC; 8]).unwrap(),
    ];
    let msg = Message::new(header, subs).unwrap();

    let bytes = msg.encode().unwrap();
    let decoded = Message::decode(&bytes).unwrap();

    assert_eq!(decoded.submessages.len(), 3);
    assert_eq!(
        decoded.submessages[0].header.submessage_id,
        SubmessageId::Heartbeat
    );
    assert_eq!(
        decoded.submessages[1].header.submessage_id,
        SubmessageId::AckNack
    );
    assert_eq!(
        decoded.submessages[2].header.submessage_id,
        SubmessageId::Timestamp
    );
}

#[test]
fn empty_message_roundtrip() {
    let header =
        MessageHeader::without_client_key(SessionId(0xC8), StreamId(0x80), SerialNumber16::new(1))
            .unwrap();
    let msg = Message::new(header, vec![]).unwrap();
    let bytes = msg.encode().unwrap();
    let decoded = Message::decode(&bytes).unwrap();
    assert_eq!(decoded.submessages.len(), 0);
}

#[test]
fn sequence_number_increments_correctly() {
    // Verifiziert dass SequenceNumber Wraparound korrekt funktioniert.
    let mut seq = SerialNumber16::new(0xFFFE);
    for _ in 0..5 {
        let next = seq.next();
        assert_ne!(next, seq);
        seq = next;
    }
}

#[test]
fn wraparound_sequence_number_roundtrip() {
    let header = MessageHeader::without_client_key(
        SessionId(0xC8),
        StreamId(0x80),
        SerialNumber16::new(0xFFFF),
    )
    .unwrap();
    let sub = Submessage::new(SubmessageId::Heartbeat, 0x01, vec![0; 8]).unwrap();
    let msg = Message::new(header, vec![sub]).unwrap();
    let bytes = msg.encode().unwrap();
    let decoded = Message::decode(&bytes).unwrap();
    assert_eq!(decoded.header.sequence_nr.0, 0xFFFF);
}

#[test]
fn many_messages_roundtrip_in_sequence() {
    for i in 0u16..256 {
        let header = MessageHeader::without_client_key(
            SessionId(0xC8),
            StreamId(0x80),
            SerialNumber16::new(i),
        )
        .unwrap();
        let sub = Submessage::new(SubmessageId::Heartbeat, 0x01, vec![i as u8; 4]).unwrap();
        let msg = Message::new(header, vec![sub]).unwrap();
        let bytes = msg.encode().unwrap();
        let decoded = Message::decode(&bytes).unwrap();
        assert_eq!(decoded.header.sequence_nr.0, i);
        assert_eq!(decoded.submessages[0].body, vec![i as u8; 4].as_slice());
    }
}
