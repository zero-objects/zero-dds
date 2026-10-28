// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! The DDS-XRCE 1.0 Annex B data examples, octet for octet: B.3.2
//! (WRITE_DATA with a sequence), B.4.1 (READ_DATA for a sample), B.5.1
//! (DATA with one sample), B.5.2 (DATA with a sequence), B.5.4 (DATA with a
//! sequence and SampleInfo) and B.5.5 (DATA with PackedSamples).
//!
//! Each test builds the octets that the figure and the table of the
//! example print, decodes them with the codecs of the crate, asserts every
//! field the example names, and re-encodes the decoded form with the
//! crate's encoder in the same byte order. The re-encoding must equal the
//! example.
//!
//! The examples contradict their own text in places. The contradictions
//! are named in the doc comment of the test that meets them. Where the
//! printed octets do not fit the printed IDL or the decoder of the crate,
//! the test builds the reading that does fit and states what differs from
//! the print; a second part of the test asserts what the print, taken
//! literally, decodes to.
//!
//! The examples do not print every value. Where a value is only named
//! ("info.state", "foo2.count"), the test fixes one and says so.

use zerodds_xrce::encoding::{Endianness, SampleInfoForm, with_sample_info_form};
use zerodds_xrce::header::{SessionId, StreamId};
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::sample_info::{SampleInfo, SampleInfoDelta, SampleInfoFlags};
use zerodds_xrce::submessages::data::{
    DataPayloadDataSeq, DataPayloadPackedSamples, DataPayloadSampleSeq,
};
use zerodds_xrce::submessages::read_data::{ReadDataPayload, ReadSpecification};
use zerodds_xrce::submessages::write_data::WriteDataPayloadDataSeq;
use zerodds_xrce::submessages::{DataFormat, DataPayload, Message, Submessage, SubmessageId};

/// The message header of the examples: session 0xDD, no ClientKey as the
/// session id is above 127 (§8.3.2.1).
fn message_header(stream_id: u8, seq: [u8; 2]) -> [u8; 4] {
    [0xDD, stream_id, seq[0], seq[1]]
}

/// The octets of a message with one submessage: header, submessage header
/// (the length is little-endian whatever the E flag says, §8.3.4), body.
fn frame(header: [u8; 4], id: u8, flags: u8, length: u16, body: &[u8]) -> Vec<u8> {
    let mut out = header.to_vec();
    out.push(id);
    out.push(flags);
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// Decodes a datagram that holds exactly one submessage and checks the
/// message header octets.
fn decode_one(bytes: &[u8], session: u8, stream: u8, seq: u16) -> Submessage {
    let msg = Message::decode(bytes).expect("the example decodes as a message");
    assert_eq!(msg.header.session_id, SessionId(session));
    assert_eq!(msg.header.stream_id, StreamId(stream));
    assert!(msg.header.client_key.is_none());
    assert_eq!(msg.header.sequence_nr.raw(), seq);
    assert_eq!(msg.submessages.len(), 1);
    msg.submessages.into_iter().next().unwrap()
}

/// The octets of the message that the crate's encoder writes for the
/// submessage `(id, flags, body)` under the header of `bytes`.
fn reencode(bytes: &[u8], id: SubmessageId, flags: u8, body: Vec<u8>) -> Vec<u8> {
    let msg = Message::decode(bytes).expect("decodes");
    let sm = Submessage::new(id, flags, body).expect("submessage");
    Message::new(msg.header, vec![sm])
        .expect("message")
        .encode()
        .expect("encode")
}

fn foo(count: u32) -> [u8; 4] {
    count.to_be_bytes()
}

// ===========================================================================
// B.3.2 WRITE_DATA with a sequence without SampleInfo
// ===========================================================================

/// Annex B.3.2, Table 26 and the figure before it: `WRITE_DATA` of four
/// `short` as `WRITE_DATA_Payload_DataSeq`, flags 0x09, little-endian.
///
/// Reading used: the octets of the figure and of Table 26 as printed, with
/// `submessageLength` 16. Bytes 8 to 23 are the body: `BaseObjectRequest`
/// (4), `data_seq.length` (4), four `short` (8).
///
/// What is off in the example:
///
/// * Table 26 prints `submessageLength = 6 = 0x0006`, {0x06, 0x00}. That is
///   the length of B.3.1 (a 4-octet request and one `short`). The table
///   itself lists the payload as bytes 8 to 23, which are 16 octets. The
///   test builds 16. The second part asserts that the printed 6 does not
///   decode: the datagram then continues at byte 16 with {0x14, ...},
///   which is not a submessage id.
/// * The text says "five values" and "two values" and lists 20, 17, 26 and
///   40. The octets list four values, and the last one is {0x24, 0x00},
///   which is 36 and not 40 (0x28). The test uses the octets.
/// * Table 26 says "Bytes 12-13 are used for SampleData" and then lists
///   bytes 12 to 15 for `data_seq.length`. The length is 4 octets at
///   bytes 12 to 15.
/// * Table 26 is titled "READ_DATA (single sample) example bytes"; it is
///   the table of B.3.2.
#[test]
fn b_3_2_write_data_with_a_sequence_without_sample_info() {
    let header = message_header(0x80, [0x01, 0x00]);
    let body: [u8; 16] = [
        0xAA, 0x01, 0x44, 0x05, // request_id, object_id
        0x04, 0x00, 0x00, 0x00, // data_seq.length = 4, little-endian
        0x14, 0x00, 0x11, 0x00, 0x1A, 0x00, 0x24, 0x00, // 20, 17, 26, 36
    ];
    let bytes = frame(header, 0x07, 0x09, 16, &body);
    assert_eq!(bytes.len(), 24);

    let sm = decode_one(&bytes, 0xDD, 0x80, 1);
    assert_eq!(sm.header.submessage_id, SubmessageId::WriteData);
    assert_eq!(sm.header.flags, 0x09);
    assert!(sm.header.is_little_endian());
    assert_eq!(
        DataFormat::from_flag_bits(sm.header.flags).unwrap(),
        DataFormat::DataSeq
    );
    assert_eq!(sm.header.submessage_length, 16);

    let p = WriteDataPayloadDataSeq::try_from_submessage(&sm).expect("WRITE_DATA data_seq");
    assert_eq!(p.base.request_id, [0xAA, 0x01]);
    assert_eq!(p.base.object_id, ObjectId::from_raw(0x4405));
    assert_eq!(p.base.object_id.to_bytes(), [0x44, 0x05]);
    assert_eq!(p.base.object_id.kind().unwrap(), ObjectKind::DataWriter);
    assert_eq!(p.sample_count, 4);
    assert_eq!(p.elements, [0x14, 0x00, 0x11, 0x00, 0x1A, 0x00, 0x24, 0x00]);
    // The elements are `Temperature { short value }`, little-endian.
    let values: Vec<i16> = p
        .elements
        .chunks(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    assert_eq!(values, [20, 17, 26, 36]);

    // Same byte order (little-endian): the encoder writes the example.
    let body_again = p.encode_body(Endianness::Little).unwrap();
    assert_eq!(body_again, body);
    let again = reencode(&bytes, SubmessageId::WriteData, 0x09, body_again);
    assert_eq!(again, bytes);
    assert_eq!(p.clone().into_submessage().unwrap(), sm);

    // The length as printed: 6.
    let printed = frame(header, 0x07, 0x09, 6, &body);
    assert!(Message::decode(&printed).is_err());
}

// ===========================================================================
// B.4.1 READ_DATA for a sample
// ===========================================================================

/// Annex B.4.1, Table 29: `READ_DATA` for the DataReader {0x44, 0x06},
/// `request_id` {0xAA, 0x01}, no filter, no delivery control, flags 0x01.
///
/// Reading used: bytes 8 to 15, all eight payload octets, with
/// `submessageId` 0x08 and `submessageLength` 8.
///
/// What is off in the example:
///
/// * Table 29 prints `submessageId = READ_DATA = 0x07`. 0x07 is
///   `WRITE_DATA` (§8.3.5, the id of the previous examples); `READ_DATA` is
///   0x08. The test builds 0x08. The second part asserts that the printed
///   0x07 decodes as `WriteData` and is not a `READ_DATA`.
/// * Table 29 prints `submessageLength = 7 = 0x0007`. The payload is
///   bytes 8 to 15, which are 8 octets: request (4), `preferred_stream_id`
///   (1), `data_format` (1), the presence octet of `content_filter_expression`
///   (1) and the presence octet of `delivery_control` (1). The test builds
///   8. With 7, the message still decodes, as the last octet counts as
///   padding after the last submessage, but the body lacks the octet of
///   `delivery_control`, and the payload does not decode. The second part
///   asserts that.
/// * Table 29 calls bytes 14 and 15 "content_filter_expression? = 0x00" and
///   "delivery_control? = 0x00"; the figure labels them
///   `content_filter?` and `delivery_contro?`. Both are the `@optional`
///   presence octets, as the crate reads them.
#[test]
fn b_4_1_read_data_for_a_sample() {
    let header = message_header(0x80, [0x01, 0x00]);
    let body: [u8; 8] = [
        0xAA, 0x01, 0x44, 0x06, // request_id, object_id
        0x00, // preferred_stream_id = STREAMID_NONE
        0x00, // data_format = FORMAT_DATA
        0x00, // content_filter_expression absent
        0x00, // delivery_control absent
    ];
    let bytes = frame(header, 0x08, 0x01, 8, &body);
    assert_eq!(bytes.len(), 16);

    let sm = decode_one(&bytes, 0xDD, 0x80, 1);
    assert_eq!(sm.header.submessage_id, SubmessageId::ReadData);
    assert_eq!(sm.header.flags, 0x01);
    assert!(sm.header.is_little_endian());
    assert_eq!(sm.header.submessage_length, 8);
    assert_eq!(sm.body, body);

    let p = ReadDataPayload::try_from_submessage(&sm).expect("READ_DATA");
    assert_eq!(p.base.request_id, [0xAA, 0x01]);
    assert_eq!(p.base.object_id, ObjectId::from_raw(0x4406));
    assert_eq!(p.base.object_id.to_bytes(), [0x44, 0x06]);
    assert_eq!(p.base.object_id.kind().unwrap(), ObjectKind::DataReader);
    assert_eq!(
        p.read_specification,
        ReadSpecification {
            preferred_stream_id: 0x00,
            data_format: DataFormat::Data,
            content_filter_expression: None,
            delivery_control: None,
        }
    );

    let body_again = p.encode_body(Endianness::Little).unwrap();
    assert_eq!(body_again, body);
    let again = reencode(&bytes, SubmessageId::ReadData, 0x01, body_again);
    assert_eq!(again, bytes);
    assert_eq!(p.clone().into_submessage().unwrap(), sm);

    // The id as printed: 0x07.
    let printed_id = frame(header, 0x07, 0x01, 8, &body);
    let sm_id = decode_one(&printed_id, 0xDD, 0x80, 1);
    assert_eq!(sm_id.header.submessage_id, SubmessageId::WriteData);
    assert!(ReadDataPayload::try_from_submessage(&sm_id).is_err());

    // The length as printed: 7. The eighth octet is padding to the message,
    // the body has no presence octet for `delivery_control`.
    let printed_len = frame(header, 0x08, 0x01, 7, &body);
    let sm_len = decode_one(&printed_len, 0xDD, 0x80, 1);
    assert_eq!(sm_len.body.len(), 7);
    assert!(ReadDataPayload::try_from_submessage(&sm_len).is_err());
}

// ===========================================================================
// B.5.1 DATA with one sample
// ===========================================================================

/// Annex B.5.1, Table 31: `DATA` for `FooType { long count }` with
/// `foo.count` = 19, big-endian, flags 0x00, on the best-effort stream.
///
/// Reading used: the octets as printed. `submessageId` 0x09 is `DATA`,
/// flags 0x00 is the E flag clear (big-endian body) and bits 1 to 3 clear
/// (FORMAT_DATA), `submessageLength` 8. Bytes 12 to 15 are the XCDR
/// serialization of `foo` in big-endian, {0x00, 0x00, 0x00, 0x13}.
///
/// What is off in the example: nothing that changes the octets. Table 31
/// writes "Represented in little endian ... as {0x08, 0x00}" for the
/// length of a big-endian submessage; §8.3.4 fixes the length as
/// little-endian whatever the E flag says, so the octets are right. The
/// crate's `DataPayload::flags()` is 0x01 (the agent sends little-endian),
/// so the re-encoding sets the flags of the example itself; the body of
/// `DataPayload` is the opaque serialization, which the codec does not
/// byte-swap.
#[test]
fn b_5_1_data_with_one_sample() {
    let header = message_header(0x01, [0x01, 0x00]);
    let body: [u8; 8] = [
        0xAA, 0x01, 0x44, 0x06, // request_id, object_id
        0x00, 0x00, 0x00, 0x13, // foo.count = 19, big-endian
    ];
    let bytes = frame(header, 0x09, 0x00, 8, &body);
    assert_eq!(bytes.len(), 16);

    let sm = decode_one(&bytes, 0xDD, 0x01, 1);
    assert_eq!(sm.header.submessage_id, SubmessageId::Data);
    assert_eq!(sm.header.flags, 0x00);
    assert!(!sm.header.is_little_endian());
    assert_eq!(sm.header.body_endianness(), Endianness::Big);
    assert_eq!(
        DataFormat::from_flag_bits(sm.header.flags).unwrap(),
        DataFormat::Data
    );
    assert_eq!(sm.header.submessage_length, 8);

    let p = DataPayload::try_from_submessage(&sm).expect("DATA");
    assert_eq!(p.base.request_id, [0xAA, 0x01]);
    assert_eq!(p.base.object_id, ObjectId::from_raw(0x4406));
    assert_eq!(p.base.object_id.kind().unwrap(), ObjectKind::DataReader);
    assert_eq!(p.serialized_data, [0x00, 0x00, 0x00, 0x13]);
    assert_eq!(
        u32::from_be_bytes(p.serialized_data.clone().try_into().unwrap()),
        19
    );

    let body_again = p.encode_body(Endianness::Big).unwrap();
    assert_eq!(body_again, body);
    let again = reencode(&bytes, SubmessageId::Data, 0x00, body_again);
    assert_eq!(again, bytes);
}

// ===========================================================================
// B.5.2 DATA with a sequence without SampleInfo
// ===========================================================================

/// Annex B.5.2, Table 32 and the figures: `DATA_Payload_DataSeq` with two
/// `FooType`, big-endian, on the best-effort stream.
///
/// Reading used: the E flag of the print (clear, so big-endian: the
/// `data_seq.length` and both `long` are written big-endian), the
/// `submessageId` 0x09 and the format bits of FORMAT_DATA_SEQ, so the flags
/// are 0x08. Body: request (4), `data_seq.length` = 2 (4), two `long` (8),
/// which is `submessageLength` 16 as printed. Values: `foo1.count` = 1,
/// `foo2.count` = 2.
///
/// What is off in the example:
///
/// * Table 32 prints `submessageId = DATA = 0x08`. `DATA` is 0x09 (B.5.1,
///   §8.3.5); 0x08 is `READ_DATA`. The test builds 0x09. The second part
///   asserts that the printed 0x08 decodes as `ReadData`.
/// * Table 32 prints `flags = 0x00 (big endian)`. The format bits 1 to 3 of
///   the flags select the format (Table 11), and 0x00 is FORMAT_DATA, not
///   FORMAT_DATA_SEQ (0x08). The test sets the format bits and keeps the E
///   flag clear. The second part asserts that the printed flags make the
///   body a FORMAT_DATA sample of 12 octets.
/// * Table 32 prints `sequenceNr = 1` as {0x0A, 0x00}. {0x0A, 0x00} is 10.
///   The test builds the printed octets and asserts 10.
/// * Bytes 8 to 9 are {0xAA, 0x01} in the table, `request_id` {0xAA, 0x02}
///   in the text above it. The test uses the table.
/// * The text sets "foo1.count is set to 1 and foo2.count is set to 1"; the
///   figure labels the second as "foo1.count = 2". The example prints no
///   octets for the values; the test uses 1 and 2.
/// * Table 32 lists the payload as bytes 8 to 23 (16 octets) and
///   `submessageLength` = 16: they agree.
#[test]
fn b_5_2_data_with_a_sequence_without_sample_info() {
    let header = message_header(0x01, [0x0A, 0x00]);
    let body: [u8; 16] = [
        0xAA, 0x01, 0x44, 0x06, // request_id, object_id
        0x00, 0x00, 0x00, 0x02, // data_seq.length = 2, big-endian
        0x00, 0x00, 0x00, 0x01, // data_seq[0].data, foo1.count = 1
        0x00, 0x00, 0x00, 0x02, // data_seq[1].data, foo2.count = 2
    ];
    let bytes = frame(header, 0x09, 0x08, 16, &body);
    assert_eq!(bytes.len(), 24);

    let sm = decode_one(&bytes, 0xDD, 0x01, 10);
    assert_eq!(sm.header.submessage_id, SubmessageId::Data);
    assert_eq!(sm.header.flags, 0x08);
    assert_eq!(sm.header.body_endianness(), Endianness::Big);
    assert_eq!(
        DataFormat::from_flag_bits(sm.header.flags).unwrap(),
        DataFormat::DataSeq
    );
    assert_eq!(sm.header.submessage_length, 16);

    let p = DataPayloadDataSeq::try_from_submessage(&sm).expect("DATA data_seq");
    assert_eq!(p.base.request_id, [0xAA, 0x01]);
    assert_eq!(p.base.object_id, ObjectId::from_raw(0x4406));
    assert_eq!(p.sample_count, 2);
    assert_eq!(p.elements.len(), 8);
    let counts: Vec<u32> = p
        .elements
        .chunks(4)
        .map(|c| u32::from_be_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(counts, [1, 2]);
    assert_eq!(p.elements, [foo(1), foo(2)].concat());

    let body_again = p.encode_body(Endianness::Big).unwrap();
    assert_eq!(body_again, body);
    let again = reencode(&bytes, SubmessageId::Data, 0x08, body_again);
    assert_eq!(again, bytes);

    // The id as printed: 0x08.
    let printed_id = frame(header, 0x08, 0x00, 16, &body);
    let sm_id = decode_one(&printed_id, 0xDD, 0x01, 10);
    assert_eq!(sm_id.header.submessage_id, SubmessageId::ReadData);
    assert!(DataPayloadDataSeq::try_from_submessage(&sm_id).is_err());

    // The flags as printed: 0x00 is FORMAT_DATA. The sequence reader
    // refuses it; the sample reader takes the twelve octets after the
    // request as one sample.
    let printed_flags = frame(header, 0x09, 0x00, 16, &body);
    let sm_flags = decode_one(&printed_flags, 0xDD, 0x01, 10);
    assert!(DataPayloadDataSeq::try_from_submessage(&sm_flags).is_err());
    let as_data = DataPayload::try_from_submessage(&sm_flags).expect("FORMAT_DATA");
    assert_eq!(as_data.serialized_data, body[4..]);
}

// ===========================================================================
// B.5.4 DATA with a sequence with SampleInfo
// ===========================================================================

/// Annex B.5.4, Table 33 and the figures: `DATA_Payload_SampleSeq` with two
/// `Sample`, each `SampleInfo` (12 octets) and `FooType` (4), big-endian, on
/// the best-effort stream.
///
/// Reading used: E flag clear (big-endian), `submessageId` 0x09, the format
/// bits of FORMAT_SAMPLE_SEQ, so the flags are 0x0A. `SampleInfo` is the
/// flat form of §7.7.1: `state` in the first row with three octets of
/// padding, then `sequence_number` and `session_time_offset` as `unsigned
/// long`. Body: request (4), `sample_seq.length` (4), two times 16 octets:
/// `submessageLength` 40 as printed, bytes 8 to 47 as the table lists them.
///
/// The test pins `SampleInfoForm::Flat` (the figure's rows; octets 1 to 3 of
/// the first row are zero padding), as the default form of the crate is the
/// union of Annex A, which reads octet 1 as its discriminator.
///
/// The example prints no values. The test fixes: `state` = VIEW_STATE_NEW,
/// `sequence_number` 1 and 2, `session_time_offset` 100 and 200,
/// `foo1.count` = 1, `foo2.count` = 2.
///
/// What is off in the example:
///
/// * `submessageId` DATA = 0x08, flags 0x00 and `sequenceNr` {0x0A, 0x00}
///   as in B.5.2, and handled as there: the test builds 0x09 and the
///   format bits, and asserts 10 for the printed {0x0A, 0x00}. The second
///   part asserts that the printed id decodes as `ReadData` and the printed
///   flags as FORMAT_DATA.
/// * Bytes 8 to 9 are {0xAA, 0x01} in the table, `request_id` {0xAA, 0x02}
///   in the text above it. The test uses the table.
/// * The IDL above the figure of B.5.3 defines `SampleInfo` with a
///   `SampleInfoDetail` union; the figure has three 4-octet rows
///   (`state`, `sequence_number`, `session_time_offset`). The 12 octets
///   per `SampleInfo` of Table 33 (bytes 16 to 27) fit the figure, and
///   the length 40 fits only the figure. The test reads the figure.
#[test]
fn b_5_4_data_with_a_sequence_with_sample_info() {
    with_sample_info_form(
        SampleInfoForm::Flat,
        b_5_4_data_with_a_sequence_with_sample_info_flat,
    );
}

fn b_5_4_data_with_a_sequence_with_sample_info_flat() {
    let header = message_header(0x01, [0x0A, 0x00]);
    let body: [u8; 40] = [
        0xAA, 0x01, 0x44, 0x06, // request_id, object_id
        0x00, 0x00, 0x00, 0x02, // sample_seq.length = 2
        0x04, 0x00, 0x00, 0x00, // [0].info.state = VIEW_STATE_NEW, padding
        0x00, 0x00, 0x00, 0x01, // [0].info.sequence_number = 1
        0x00, 0x00, 0x00, 0x64, // [0].info.session_time_offset = 100
        0x00, 0x00, 0x00, 0x01, // [0].data, foo1.count = 1
        0x04, 0x00, 0x00, 0x00, // [1].info.state
        0x00, 0x00, 0x00, 0x02, // [1].info.sequence_number = 2
        0x00, 0x00, 0x00, 0xC8, // [1].info.session_time_offset = 200
        0x00, 0x00, 0x00, 0x02, // [1].data, foo2.count = 2
    ];
    let bytes = frame(header, 0x09, 0x0A, 40, &body);
    assert_eq!(bytes.len(), 48);

    let sm = decode_one(&bytes, 0xDD, 0x01, 10);
    assert_eq!(sm.header.submessage_id, SubmessageId::Data);
    assert_eq!(sm.header.flags, 0x0A);
    assert_eq!(sm.header.body_endianness(), Endianness::Big);
    assert_eq!(
        DataFormat::from_flag_bits(sm.header.flags).unwrap(),
        DataFormat::SampleSeq
    );
    assert_eq!(sm.header.submessage_length, 40);

    let p = DataPayloadSampleSeq::try_from_submessage(&sm).expect("DATA sample_seq");
    assert_eq!(p.base.request_id, [0xAA, 0x01]);
    assert_eq!(p.base.object_id, ObjectId::from_raw(0x4406));
    assert_eq!(p.sample_count, 2);
    assert_eq!(p.elements.len(), 32);

    let e = sm.header.body_endianness();
    let mut rest = p.elements.as_slice();
    let mut samples = Vec::new();
    for _ in 0..p.sample_count {
        let (info, n) = SampleInfo::decode(rest, e).expect("SampleInfo");
        assert_eq!(n, 12);
        let count = u32::from_be_bytes(rest[n..n + 4].try_into().unwrap());
        samples.push((info, count));
        rest = &rest[n + 4..];
    }
    assert!(rest.is_empty());
    let new = SampleInfoFlags(SampleInfoFlags::VIEW_STATE_NEW);
    assert_eq!(
        samples,
        [
            (
                SampleInfo {
                    state: new,
                    sequence_number: 1,
                    session_time_offset: 100,
                },
                1
            ),
            (
                SampleInfo {
                    state: new,
                    sequence_number: 2,
                    session_time_offset: 200,
                },
                2
            ),
        ]
    );
    assert!(samples[0].0.state.is_new());

    // Re-encoding with the codecs of the elements, big-endian.
    let mut elements = Vec::new();
    for (info, count) in &samples {
        info.encode_into(e, &mut elements).unwrap();
        elements.extend_from_slice(&foo(*count));
    }
    assert_eq!(elements, p.elements);
    let body_again = p.encode_body(Endianness::Big).unwrap();
    assert_eq!(body_again, body);
    let again = reencode(&bytes, SubmessageId::Data, 0x0A, body_again);
    assert_eq!(again, bytes);

    // The id and the flags as printed.
    let printed_id = frame(header, 0x08, 0x00, 40, &body);
    let sm_id = decode_one(&printed_id, 0xDD, 0x01, 10);
    assert_eq!(sm_id.header.submessage_id, SubmessageId::ReadData);
    assert!(DataPayloadSampleSeq::try_from_submessage(&sm_id).is_err());
    let printed_flags = frame(header, 0x09, 0x00, 40, &body);
    let sm_flags = decode_one(&printed_flags, 0xDD, 0x01, 10);
    assert!(DataPayloadSampleSeq::try_from_submessage(&sm_flags).is_err());
    assert_eq!(
        DataFormat::from_flag_bits(sm_flags.header.flags).unwrap(),
        DataFormat::Data
    );
}

// ===========================================================================
// B.5.5 DATA with PackedSamples
// ===========================================================================

/// Annex B.5.5, Table 34 and the figures: `DATA_Payload_PackedSamples` with
/// one `info_base` and two `SampleDelta`, each `SampleInfoDelta` (4 octets)
/// and `FooType` (4), big-endian, on the reliable builtin stream.
///
/// Reading used: the figure. Body: `request_id` and `object_id` (4),
/// `info_base` (12), `sample_delta_seq.length` = 2 (4), two times 8 octets,
/// which are 36 octets: `submessageLength` 36 as printed. `submessageId`
/// 0x09, flags 0x0E (E flag clear, FORMAT_PACKED_SAMPLES). `request_id` is
/// {0xAA, 0x03} as in the text, the table does not list it.
///
/// The test pins `SampleInfoForm::Flat`, as in B.5.4.
///
/// The example prints no values. The test fixes: `info_base` with `state`
/// VIEW_STATE_NEW, `sequence_number` 5, `session_time_offset` 1000; deltas
/// (`state` 0, `seq_number_delta` 1, `timestamp_delta` 10) and (0, 2, 20);
/// `foo1.count` = 1, `foo2.count` = 2. The resolved infos follow §7.7.1:
/// sequence numbers 6 and 7, offsets 2000 and 3000 milliseconds
/// (`timestamp_delta` counts tenths of a second).
///
/// What is off in the example:
///
/// * Table 34 lists the payload as bytes 8 to 47 and `info_base` as bytes 8
///   to 19, without `request_id` and `object_id`. The figure has the
///   `BaseObjectRequest` row before `info_base`, and the length 36 fits
///   the figure (4 + 12 + 4 + 16) and not a 40-octet listing. With the
///   request, the table's own byte numbers move by four: `info_base` is
///   bytes 12 to 23, `sample_delta_seq.length` bytes 24 to 27, and so on to
///   byte 43; the message is 44 octets. The test reads the figure.
/// * `submessageId` DATA = 0x08, flags 0x00 and `sequenceNr` {0x0A, 0x00}
///   as in B.5.2; the stream is 0x80 here. Handled as in B.5.2.
/// * The text sets "foo1.count is set to 1 and foo2.count is set to 1";
///   the figure labels the second as "foo1.count = 2".
#[test]
fn b_5_5_data_with_packed_samples() {
    with_sample_info_form(SampleInfoForm::Flat, b_5_5_data_with_packed_samples_flat);
}

fn b_5_5_data_with_packed_samples_flat() {
    let header = message_header(0x80, [0x0A, 0x00]);
    let body: [u8; 36] = [
        0xAA, 0x03, 0x44, 0x06, // request_id, object_id
        0x04, 0x00, 0x00, 0x00, // info_base.state = VIEW_STATE_NEW, padding
        0x00, 0x00, 0x00, 0x05, // info_base.sequence_number = 5
        0x00, 0x00, 0x03, 0xE8, // info_base.session_time_offset = 1000
        0x00, 0x00, 0x00, 0x02, // sample_delta_seq.length = 2
        0x00, 0x01, 0x00, 0x0A, // [0].info_delta: state 0, +1, +10 ds
        0x00, 0x00, 0x00, 0x01, // [0].data, foo1.count = 1
        0x00, 0x02, 0x00, 0x14, // [1].info_delta: state 0, +2, +20 ds
        0x00, 0x00, 0x00, 0x02, // [1].data, foo2.count = 2
    ];
    let bytes = frame(header, 0x09, 0x0E, 36, &body);
    assert_eq!(bytes.len(), 44);

    let sm = decode_one(&bytes, 0xDD, 0x80, 10);
    assert_eq!(sm.header.submessage_id, SubmessageId::Data);
    assert_eq!(sm.header.flags, 0x0E);
    assert_eq!(sm.header.body_endianness(), Endianness::Big);
    assert_eq!(
        DataFormat::from_flag_bits(sm.header.flags).unwrap(),
        DataFormat::PackedSamples
    );
    assert_eq!(sm.header.submessage_length, 36);

    let p = DataPayloadPackedSamples::try_from_submessage(&sm).expect("DATA packed");
    assert_eq!(p.base.request_id, [0xAA, 0x03]);
    assert_eq!(p.base.object_id, ObjectId::from_raw(0x4406));
    let new = SampleInfoFlags(SampleInfoFlags::VIEW_STATE_NEW);
    let info_base = SampleInfo {
        state: new,
        sequence_number: 5,
        session_time_offset: 1000,
    };
    assert_eq!(p.info_base, info_base);
    assert_eq!(p.sample_count, 2);
    assert_eq!(p.elements.len(), 16);

    let e = sm.header.body_endianness();
    let mut rest = p.elements.as_slice();
    let mut samples = Vec::new();
    for _ in 0..p.sample_count {
        let (delta, n) = SampleInfoDelta::decode(rest, e).expect("SampleInfoDelta");
        assert_eq!(n, 4);
        let count = u32::from_be_bytes(rest[n..n + 4].try_into().unwrap());
        samples.push((delta, count));
        rest = &rest[n + 4..];
    }
    assert!(rest.is_empty());
    assert_eq!(
        samples,
        [
            (
                SampleInfoDelta {
                    state: SampleInfoFlags(0),
                    seq_number_delta: 1,
                    timestamp_delta: 10,
                },
                1
            ),
            (
                SampleInfoDelta {
                    state: SampleInfoFlags(0),
                    seq_number_delta: 2,
                    timestamp_delta: 20,
                },
                2
            ),
        ]
    );
    let resolved: Vec<(u32, u32)> = samples
        .iter()
        .map(|(d, _)| {
            let r = d.resolve(&p.info_base);
            (r.sequence_number, r.session_time_offset)
        })
        .collect();
    assert_eq!(resolved, [(6, 2000), (7, 3000)]);

    // Re-encoding with the codecs of the elements, big-endian.
    let mut elements = Vec::new();
    for (delta, count) in &samples {
        delta.encode_into(e, &mut elements).unwrap();
        elements.extend_from_slice(&foo(*count));
    }
    assert_eq!(elements, p.elements);
    let body_again = p.encode_body(Endianness::Big).unwrap();
    assert_eq!(body_again, body);
    let again = reencode(&bytes, SubmessageId::Data, 0x0E, body_again);
    assert_eq!(again, bytes);

    // The id and the flags as printed.
    let printed_id = frame(header, 0x08, 0x00, 36, &body);
    let sm_id = decode_one(&printed_id, 0xDD, 0x80, 10);
    assert_eq!(sm_id.header.submessage_id, SubmessageId::ReadData);
    assert!(DataPayloadPackedSamples::try_from_submessage(&sm_id).is_err());
    let printed_flags = frame(header, 0x09, 0x00, 36, &body);
    let sm_flags = decode_one(&printed_flags, 0xDD, 0x80, 10);
    assert!(DataPayloadPackedSamples::try_from_submessage(&sm_flags).is_err());
}
