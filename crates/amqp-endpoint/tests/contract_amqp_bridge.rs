// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-amqp-endpoint` with `zerodds-amqp-bridge`
//! (internal/release/rc8-test-suite.md, "Crate tier"): the frame header,
//! the described lists of the performatives and message sections, and the
//! values the endpoint reads out of them, each against the octets of OASIS
//! AMQP 1.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_amqp_bridge::frame::{
    FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};
use zerodds_amqp_bridge::performatives::{
    decode_performative, descriptor, encode_performative, open,
};
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_bridge::types::AmqpValue;
use zerodds_amqp_bridge::{AmqpExtValue, TypeError};

/// Part 2 §2.3.1: SIZE, DOFF, TYPE and the channel of an AMQP frame.
#[test]
fn amqp10_2_3_1_the_frame_header_is_size_doff_type_and_channel() {
    let h = FrameHeader {
        size: 0x0102,
        doff: 2,
        frame_type: FrameType::Amqp,
        channel: 7,
    };
    let octets = [0x00, 0x00, 0x01, 0x02, 0x02, 0x00, 0x00, 0x07];
    assert_eq!(encode_frame_header(h), octets);
    assert_eq!(decode_frame_header(&octets), Ok(h));
    assert_eq!(h.body_offset(), 8);
}

/// Part 2 §2.7.1 with Part 1 §1.4: `open` is the described list 0x10 whose
/// first field is the container-id.
#[test]
fn amqp10_2_7_1_open_is_a_described_list_with_descriptor_0x10() {
    let mut octets = vec![0x00, 0x53, 0x10, 0xc0, 0x0f, 0x05, 0xa1, 0x08];
    octets.extend_from_slice(b"client-1");
    octets.extend_from_slice(&[0x40, 0x40, 0x40, 0x40]);
    assert_eq!(open("client-1").unwrap(), octets);
    let (code, body, n) = decode_performative(&octets).unwrap();
    assert_eq!((code, n), (descriptor::OPEN, octets.len()));
    let AmqpExtValue::List(fields) = body else {
        panic!("a list body")
    };
    assert_eq!(fields[0], AmqpExtValue::Str("client-1".into()));
}

/// Part 1 §1.4: any descriptor code with any list, as the broker encodes a
/// performative the endpoint answers.
#[test]
fn amqp10_1_4_a_performative_is_0x00_the_descriptor_and_the_list() {
    let body = AmqpExtValue::List(vec![AmqpExtValue::Uint(1), AmqpExtValue::Boolean(true)]);
    let octets = [0x00, 0x53, 0x16, 0xc0, 0x04, 0x02, 0x52, 0x01, 0x41];
    assert_eq!(
        encode_performative(descriptor::DETACH, &body).unwrap(),
        octets
    );
    assert_eq!(
        decode_performative(&octets).unwrap(),
        (descriptor::DETACH, body, octets.len())
    );
    assert!(decode_performative(&[0x00, 0x53]).is_err());
}

/// Part 3 §3.2.6: the data section 0x75 carries the message body as a
/// binary.
#[test]
fn amqp10_3_2_6_a_data_section_is_0x75_and_a_binary() {
    let octets = [0x00, 0x53, 0x75, 0xa0, 0x03, 0x01, 0x02, 0x03];
    assert_eq!(
        MessageSection::Data(vec![1, 2, 3]).encode().unwrap(),
        octets
    );
    assert_eq!(
        MessageSection::decode(&octets).unwrap(),
        (MessageSection::Data(vec![1, 2, 3]), octets.len())
    );
}

/// Part 1 §1.6.6 and §1.2: the descriptor codes the endpoint reads are
/// ulongs; an encoding cut short is an error, not a value.
#[test]
fn amqp10_1_6_6_a_smallulong_reads_as_a_ulong_and_a_cut_value_is_refused() {
    assert_eq!(
        zerodds_amqp_bridge::decode_value(&[0x53, 0x24]),
        Ok((AmqpValue::Ulong(0x24), 2))
    );
    assert_eq!(
        zerodds_amqp_bridge::decode_value(&[0x80, 0x00]),
        Err(TypeError::Truncated)
    );
}
