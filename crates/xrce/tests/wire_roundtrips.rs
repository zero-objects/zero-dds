//! A message is a header and at least one submessage (DDS-XRCE 1.0 §8.3.1).

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
    Message, MessageHeader, SESSION_ID_NONE_WITHOUT_CLIENT_KEY, SerialNumber16, SessionId, StreamId,
};

/// DDS-XRCE 1.0 §8.3.1: "An XRCE message is composed of a message Header
/// followed by one or more Submessages". A header alone is no message.
#[test]
fn xrce_8_3_1_a_message_without_a_submessage_is_refused() {
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
