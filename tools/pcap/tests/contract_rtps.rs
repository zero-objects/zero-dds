// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-pcap` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`):
//! `datagram::decode_datagram` on the RTPS payload of a captured frame, the
//! `header.guid_prefix` of the result, its `submessages` in order, and of each
//! `SubmessageFrame` the `little_endian` and `octets_elided` flags and the
//! `ParsedSubmessage` variant, including `Unknown { id, flags, body }` for a
//! submessage it cannot name. The datagrams below are written by hand from
//! the wire diagrams of DDSI-RTPS 2.5 §9.4.4 (header), §9.4.5.1 (submessage
//! header, kinds 0x00..0x7f protocol, 0x80..0xff vendor), §9.4.5.7
//! (HEARTBEAT), §9.4.5.12 (INFO_TS).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};

const PREFIX: [u8; 12] = [0x01, 0xF0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

fn header() -> Vec<u8> {
    let mut out = b"RTPS".to_vec();
    out.extend_from_slice(&[2, 5, 0x01, 0xF0]);
    out.extend_from_slice(&PREFIX);
    out
}

/// HEARTBEAT body: readerId, writerId, firstSN, lastSN, count.
fn heartbeat_body(little: bool) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0, 0, 0, 0x01, 0x02];
    let words: [u32; 5] = [0, 1, 0, 9, 4];
    for word in words {
        body.extend_from_slice(&if little {
            word.to_le_bytes()
        } else {
            word.to_be_bytes()
        });
    }
    body
}

#[test]
fn rtps_9_4_4_the_header_names_the_sender_prefix() {
    let datagram = header();
    let parsed = decode_datagram(&datagram).unwrap();
    assert_eq!(parsed.header.guid_prefix.0, PREFIX);
    assert!(parsed.submessages.is_empty());
}

#[test]
fn rtps_9_4_5_submessages_come_back_in_order_with_their_byte_order() {
    let mut datagram = header();
    // INFO_TS, E=1: seconds 7, fraction 2^31.
    datagram.extend_from_slice(&[0x09, 0x01, 8, 0]);
    datagram.extend_from_slice(&7_u32.to_le_bytes());
    datagram.extend_from_slice(&(1_u32 << 31).to_le_bytes());
    // HEARTBEAT, E=0 (big endian), F=1.
    datagram.extend_from_slice(&[0x07, 0x02, 0, 28]);
    datagram.extend_from_slice(&heartbeat_body(false));
    let parsed = decode_datagram(&datagram).unwrap();
    assert_eq!(parsed.submessages.len(), 2);
    let first = &parsed.submessages[0];
    assert!(first.little_endian);
    match &first.submessage {
        ParsedSubmessage::InfoTimestamp(ts) => {
            assert!(!ts.invalidate);
            assert_eq!((ts.timestamp.seconds, ts.timestamp.fraction), (7, 1 << 31));
        }
        other => panic!("INFO_TS expected, got {other:?}"),
    }
    let second = &parsed.submessages[1];
    assert!(!second.little_endian);
    match &second.submessage {
        ParsedSubmessage::Heartbeat(hb) => {
            assert_eq!((hb.first_sn.0, hb.last_sn.0, hb.count), (1, 9, 4));
            assert!(hb.final_flag);
        }
        other => panic!("HEARTBEAT expected, got {other:?}"),
    }
}

#[test]
fn rtps_9_4_5_1_a_vendor_submessage_is_kept_as_unknown_with_its_body() {
    let mut datagram = header();
    datagram.extend_from_slice(&[0x80, 0x05, 4, 0, 0xde, 0xad, 0xbe, 0xef]);
    let parsed = decode_datagram(&datagram).unwrap();
    match &parsed.submessages[0].submessage {
        ParsedSubmessage::Unknown { id, flags, body } => {
            assert_eq!((*id, *flags), (0x80, 0x05));
            assert_eq!(body, &[0xde, 0xad, 0xbe, 0xef]);
        }
        other => panic!("Unknown expected, got {other:?}"),
    }
}

#[test]
fn rtps_9_4_5_1_a_last_submessage_may_elide_its_length() {
    let mut datagram = header();
    datagram.extend_from_slice(&[0x07, 0x01, 0, 0]);
    datagram.extend_from_slice(&heartbeat_body(true));
    let parsed = decode_datagram(&datagram).unwrap();
    assert!(parsed.submessages[0].octets_elided);
    assert!(matches!(
        parsed.submessages[0].submessage,
        ParsedSubmessage::Heartbeat(_)
    ));
}

#[test]
fn a_payload_that_is_not_rtps_is_an_error_not_a_datagram() {
    assert!(decode_datagram(b"HTTP/1.1 200 OK\r\n\r\n").is_err());
    assert!(decode_datagram(&header()[..10]).is_err());
}
