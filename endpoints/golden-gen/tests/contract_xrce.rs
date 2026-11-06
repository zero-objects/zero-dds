// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-endpoint-golden` with `zerodds-xrce`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-xrce`:
//! - `src/main.rs`: the submessage payloads (`CreatePayload`,
//!   `WriteDataPayload`, `DataPayload`), `BaseObjectRequest`, `ObjectId`,
//!   `ObjectKind`, `SerialNumber16` and the header to write the golden
//!   frames the bindings are held to; `encoding::with_unannotated_types` for
//!   the form of an unannotated type; `transport_serial::encode_frame` for
//!   the golden serial frame; `AgentRepresentation` and `ResultStatus` for
//!   the agent's reply.
//!
//! So the crate relies on: the datagrams a foreign client and agent send
//! decoding into the header, the `CLIENT_Representation` and the
//! `STATUS_AGENT` the crate works with, a `WRITE_DATA` reading as the request, the DataWriter and the sample, and a serial frame of Annex C.
//!
//! Oracles: the datagrams eProsima Micro-XRCE-DDS 3.0.1 sent between its own
//! client and agent (`tools/wire-capture/xrce-sessions/`, provenance in its
//! `README.md`; no ZeroDDS code in either process); DDS-XRCE 1.0 §8.3.2
//! (the message header, a `session_id` above 127 carries no `ClientKey`),
//! §7.8.2.1 and Annex A (`CLIENT_Representation`), §8.3.5.5 (`STATUS_AGENT`
//! with a `ResultStatus` in front of the `AGENT_Representation`), Annex B (the `WRITE_DATA` example), Annex C.1.1 and RFC 1662 Appendix C (the serial frame and its check).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_xrce::handshake::{AgentRepresentation, ClientRepresentation};
use zerodds_xrce::submessages::{CreateClientPayload, StatusAgentPayload};
use zerodds_xrce::{Message, SubmessageId};

fn capture(file: &str) -> Vec<Vec<u8>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/wire-capture/xrce-sessions")
        .join(file);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            (0..line.len() / 2)
                .map(|i| u8::from_str_radix(&line[2 * i..2 * i + 2], 16).unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn xrce_8_3_2_a_session_id_above_127_carries_no_client_key() {
    // 80 00 0000: session 0x80, stream 0, sequence 0, and the submessage at
    // octet 4 — no ClientKey between.
    let first = &capture("client-to-agent.hex")[0];
    let message = Message::decode(first).unwrap();
    assert_eq!(message.header.session_id.0, 0x80);
    assert_eq!(message.header.stream_id.0, 0x00);
    assert!(message.header.client_key.is_none());
    assert_eq!(message.submessages.len(), 1);
}

#[test]
fn xrce_7_8_2_1_a_foreign_create_client_is_the_representation_the_crate_writes() {
    // 58524345 0100 010f ccccdddd 81 00 fc01: "XRCE", version 1.0, vendor
    // 0x010f, ClientKey, session 0x81, no properties, eProsima's MTU 508.
    let first = &capture("client-to-agent.hex")[0];
    let message = Message::decode(first).unwrap();
    let submessage = &message.submessages[0];
    assert_eq!(submessage.header.submessage_id, SubmessageId::CreateClient);
    let representation = CreateClientPayload::try_from_submessage(submessage)
        .unwrap()
        .representation;
    let expected = ClientRepresentation {
        version: [0x01, 0x00],
        vendor_id: [0x01, 0x0f],
        timestamp: None,
        client_key: zerodds_xrce::header::ClientKey([0xcc, 0xcc, 0xdd, 0xdd]),
        session_id: 0x81,
        properties: None,
        requested_mtu: Some(508),
    };
    assert_eq!(representation, expected);
    // Written by this crate, the same body.
    let ours = CreateClientPayload {
        representation: expected,
    }
    .into_submessage()
    .unwrap();
    assert_eq!(ours.body, submessage.body);
}

#[test]
fn xrce_8_3_5_5_a_foreign_status_agent_is_a_success_with_the_agents_representation() {
    // 04 01 0b00 | 0000 58524345 0100 010f 00: ResultStatus OK, then the
    // AGENT_Representation of Micro-XRCE-DDS.
    let first = &capture("agent-to-client.hex")[0];
    let message = Message::decode(first).unwrap();
    let submessage = &message.submessages[0];
    assert_eq!(submessage.header.submessage_id, SubmessageId::StatusAgent);
    let payload = StatusAgentPayload::try_from_submessage(submessage).unwrap();
    let e = submessage.header.body_endianness();
    let (result, at) = payload.result_and_position(e).unwrap();
    assert!(result.status_code.is_success());
    assert_eq!(at, 2);
    let (agent, _) = AgentRepresentation::decode_at(&payload.representation, at, e).unwrap();
    assert_eq!(agent.version, [0x01, 0x00]);
    assert_eq!(agent.vendor_id, [0x01, 0x0f]);
    assert_eq!(agent.properties, None);
}

#[test]
fn xrce_8_3_5_8_the_annex_b_write_data_is_request_object_and_sample() {
    // Annex B: WRITE_DATA, flags E | FORMAT_DATA, request 0xAA01, the
    // DataWriter 0x4405, the sample 0x1900.
    let submessage = zerodds_xrce::Submessage::new(
        SubmessageId::WriteData,
        0x01,
        vec![0xAA, 0x01, 0x44, 0x05, 0x19, 0x00],
    )
    .unwrap();
    let payload =
        zerodds_xrce::submessages::WriteDataPayload::try_from_submessage(&submessage).unwrap();
    assert_eq!(payload.base.request_id, [0xAA, 0x01]);
    assert_eq!(
        payload.base.object_id,
        zerodds_xrce::object_id::ObjectId::from_bytes(&[0x44, 0x05]).unwrap()
    );
    assert_eq!(
        payload.base.object_id.kind().unwrap(),
        zerodds_xrce::object_kind::ObjectKind::DataWriter
    );
    assert_eq!(payload.serialized_data, [0x19, 0x00]);
}

/// RFC 1662 Appendix C.2: the 16-bit frame check, reflected 0x8408,
/// initial 0xffff, complemented.
fn fcs16(bytes: &[u8]) -> u16 {
    let mut fcs: u16 = 0xffff;
    for &byte in bytes {
        fcs ^= u16::from(byte);
        for _ in 0..8 {
            fcs = if fcs & 1 == 1 {
                (fcs >> 1) ^ 0x8408
            } else {
                fcs >> 1
            };
        }
    }
    !fcs
}

#[test]
fn xrce_annex_c_1_1_a_serial_frame_is_flag_addresses_length_payload_and_check() {
    // RFC 1662's own check value of "123456789".
    assert_eq!(fcs16(b"123456789"), 0x906e);
    let frame = zerodds_xrce::transport_serial::encode_frame(1, 0, &[0x10, 0x20, 0x30]).unwrap();
    let check = fcs16(&[0x01, 0x00, 0x03, 0x00, 0x10, 0x20, 0x30]).to_le_bytes();
    assert_eq!(
        frame,
        [
            0x7e, 0x01, 0x00, 0x03, 0x00, 0x10, 0x20, 0x30, check[0], check[1]
        ]
    );
}
