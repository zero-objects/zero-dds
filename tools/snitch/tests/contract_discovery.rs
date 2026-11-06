// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-snitch` with `zerodds-discovery`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-discovery` (`src/main.rs`,
//! `render_text` and `render_json`): the participants
//! `DcpsRuntime::discovered_participants` hands it, as
//! `spdp::DiscoveredParticipant`, of which it prints `sender_prefix.0` (12
//! octets) as `prefix`, `sender_vendor.0` (2 octets) as `vendor`,
//! `data.guid.to_bytes()` (16 octets) as `key` and `data.user_data.len()`.
//!
//! The contract is where each of those comes from. A participant is
//! obtained here the way the runtime obtains it, from an SPDP datagram
//! through `spdp::SpdpReader::parse_datagram`; the datagram is written by
//! hand. Oracle: DDSI-RTPS 2.5 §8.3.3.1 and §9.4.4 (the message header:
//! `RTPS`, version, vendorId, guidPrefix), §9.4.5.3 (DATA: flags E and D,
//! `octetsToInlineQos` 16), §9.3.1.3 Table 9.2 (the SPDP writer and reader
//! entity ids), §9.6.2.2.2 Table 9.12 (`PID_PARTICIPANT_GUID` 0x0050,
//! `PID_USER_DATA` 0x002c, `PID_SENTINEL` 0x0001), §10.6 (PL_CDR_LE
//! `00 03`). No socket is opened.
#![allow(clippy::unwrap_used)]

use zerodds_discovery::spdp::{DiscoveredParticipant, SpdpReader};

/// An SPDP announcement: header with `vendor` and `header_prefix`, one
/// DATA from the SPDP writer whose payload carries `guid` and three octets
/// of user data.
fn spdp_datagram(vendor: [u8; 2], header_prefix: [u8; 12], guid: [u8; 16]) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(b"RTPS");
    d.extend_from_slice(&[0x02, 0x05]); // protocol version 2.5
    d.extend_from_slice(&vendor);
    d.extend_from_slice(&header_prefix);
    // DATA, flags E (little-endian) and D (data present).
    d.extend_from_slice(&[0x15, 0x05]);
    d.extend_from_slice(&60u16.to_le_bytes()); // octetsToNextHeader
    d.extend_from_slice(&[0x00, 0x00]); // extraFlags
    d.extend_from_slice(&16u16.to_le_bytes()); // octetsToInlineQos
    d.extend_from_slice(&[0x00, 0x01, 0x00, 0xc7]); // SPDP reader
    d.extend_from_slice(&[0x00, 0x01, 0x00, 0xc2]); // SPDP writer
    d.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0]); // writerSN 1
    d.extend_from_slice(&[0x00, 0x03, 0x00, 0x00]); // PL_CDR_LE
    d.extend_from_slice(&[0x50, 0x00, 0x10, 0x00]); // PID_PARTICIPANT_GUID, 16
    d.extend_from_slice(&guid);
    d.extend_from_slice(&[0x2c, 0x00, 0x08, 0x00]); // PID_USER_DATA, 8
    d.extend_from_slice(&[0x03, 0x00, 0x00, 0x00, b'u', b'd', b'!', 0x00]);
    d.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // PID_SENTINEL
    assert_eq!(d.len(), 20 + 4 + 60);
    d
}

const PREFIX: [u8; 12] = [
    0x01, 0x0f, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa,
];

fn participant_guid(prefix: [u8; 12]) -> [u8; 16] {
    let mut guid = [0u8; 16];
    guid[..12].copy_from_slice(&prefix);
    guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xc1]); // ENTITYID_PARTICIPANT
    guid
}

/// What `render_text` and `render_json` read.
fn printed(p: &DiscoveredParticipant) -> ([u8; 12], [u8; 2], [u8; 16], usize) {
    (
        p.sender_prefix.0,
        p.sender_vendor.0,
        p.data.guid.to_bytes(),
        p.data.user_data.len(),
    )
}

#[test]
fn rtps_9_4_4_prefix_and_vendor_are_the_message_headers_and_key_the_participant_guid() {
    let guid = participant_guid(PREFIX);
    let datagram = spdp_datagram([0x01, 0x0f], PREFIX, guid);
    let participant = SpdpReader::new().parse_datagram(&datagram).unwrap();
    assert_eq!(printed(&participant), (PREFIX, [0x01, 0x0f], guid, 3));
    assert_eq!(participant.data.user_data, b"ud!");
}

#[test]
fn rtps_8_3_3_1_a_header_prefix_that_disagrees_with_the_guid_is_printed_as_sent() {
    // `prefix` is the sender's header, `key` the announced GUID; the two
    // are separate columns because they can differ.
    let other = [0x01, 0x0f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02];
    let guid = participant_guid(PREFIX);
    let datagram = spdp_datagram([0x01, 0x10], other, guid);
    let participant = SpdpReader::new().parse_datagram(&datagram).unwrap();
    let (prefix, vendor, key, _) = printed(&participant);
    assert_eq!(prefix, other);
    assert_eq!(vendor, [0x01, 0x10]);
    assert_eq!(key, guid);
}
