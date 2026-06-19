//! WP 0.6 W1 — Cyclone DDS wire compliance tests.
//!
//! Verifies that our decoder accepts hand-curated Cyclone-DDS-typical
//! RTPS frames and our encoder produces bytes with the same structure.
//! See `tests/fixtures/cyclone/README.md` for
//! frame descriptions and the phase-1 capture guide.

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

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram, encode_data_datagram};
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::submessages::DataSubmessage;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix, SequenceNumber, VendorId};

const FRAME_DATA_EMPTY: &str = include_str!("fixtures/cyclone/data_empty_payload.hex");
const FRAME_DATA_CDR2: &str = include_str!("fixtures/cyclone/data_with_cdr2_payload.hex");
const FRAME_HEARTBEAT: &str = include_str!("fixtures/cyclone/heartbeat.hex");
const FRAME_SEDP_PUBLICATION: &str = include_str!("fixtures/cyclone/sedp_publication.hex");

/// Parses hex text (with comment lines `#` and whitespace) into
/// `Vec<u8>`. Ignores empty lines and comments.
fn parse_hex(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for chunk in line.split_whitespace() {
            for pair in chunk.as_bytes().chunks(2) {
                let hex = std::str::from_utf8(pair).expect("ascii hex");
                let b = u8::from_str_radix(hex, 16).expect("valid hex pair");
                bytes.push(b);
            }
        }
    }
    bytes
}

// ============================================================================
// Decoder-Compliance
// ============================================================================

#[test]
fn cyclone_data_empty_payload_decodes() {
    let bytes = parse_hex(FRAME_DATA_EMPTY);
    assert_eq!(bytes.len(), 44, "fixture should be exactly 44 bytes");

    let parsed = decode_datagram(&bytes).expect("decode");
    assert_eq!(parsed.header.vendor_id, VendorId([0x01, 0x10]));
    assert_eq!(parsed.submessages.len(), 1);
    match &parsed.submessages[0] {
        ParsedSubmessage::Data(d) => {
            assert_eq!(d.writer_sn, SequenceNumber(1));
            assert!(d.serialized_payload.is_empty());
            assert_eq!(d.reader_id.entity_key, [0, 0, 1]);
            assert_eq!(d.writer_id.entity_key, [0, 0, 1]);
        }
        other => panic!("expected DATA, got {other:?}"),
    }
}

#[test]
fn cyclone_data_cdr2_payload_decodes_and_carries_payload() {
    let bytes = parse_hex(FRAME_DATA_CDR2);
    let parsed = decode_datagram(&bytes).expect("decode");
    assert_eq!(parsed.submessages.len(), 1);
    match &parsed.submessages[0] {
        ParsedSubmessage::Data(d) => {
            assert_eq!(d.writer_sn, SequenceNumber(42));
            // Payload = 4-byte encapsulation header + 4-byte CDR2 body
            assert_eq!(d.serialized_payload.len(), 8);
            // Encapsulation kind CDR2_LE = 00 11 (BE order in wire)
            assert_eq!(&d.serialized_payload[0..2], &[0x00, 0x11]);
            // Payload body: u32 LE = 0x2A
            assert_eq!(&d.serialized_payload[4..8], &[0x2A, 0x00, 0x00, 0x00]);
        }
        other => panic!("expected DATA, got {other:?}"),
    }
}

#[test]
fn cyclone_heartbeat_decodes() {
    let bytes = parse_hex(FRAME_HEARTBEAT);
    let parsed = decode_datagram(&bytes).expect("decode");
    assert_eq!(parsed.submessages.len(), 1);
    match &parsed.submessages[0] {
        ParsedSubmessage::Heartbeat(h) => {
            assert_eq!(h.first_sn, SequenceNumber(1));
            assert_eq!(h.last_sn, SequenceNumber(10));
            assert_eq!(h.count, 5);
            // Fixture has flags=0x03 = E+F: final_flag=true, liveliness=false.
            assert!(h.final_flag, "Cyclone-HB must carry F-flag from header");
            assert!(!h.liveliness_flag);
        }
        other => panic!("expected HEARTBEAT, got {other:?}"),
    }
}

#[test]
fn cyclone_frames_have_rtps_magic() {
    for (name, frame) in [
        ("empty", FRAME_DATA_EMPTY),
        ("cdr2", FRAME_DATA_CDR2),
        ("heartbeat", FRAME_HEARTBEAT),
    ] {
        let bytes = parse_hex(frame);
        assert_eq!(
            &bytes[..4],
            b"RTPS",
            "frame {name} must start with RTPS magic"
        );
    }
}

#[test]
fn cyclone_frames_use_rtps_2_5_or_compatible() {
    // Accept everything from version 2.x onward.
    for (name, frame) in [
        ("empty", FRAME_DATA_EMPTY),
        ("cdr2", FRAME_DATA_CDR2),
        ("heartbeat", FRAME_HEARTBEAT),
    ] {
        let bytes = parse_hex(frame);
        let header = RtpsHeader::from_bytes(&bytes).expect("decode");
        assert_eq!(
            header.protocol_version.major, 2,
            "frame {name} must be RTPS 2.x"
        );
    }
}

// ============================================================================
// Encoder compliance: ZeroDDS output is structurally equivalent
// ============================================================================

#[test]
fn zerodds_writer_produces_compatible_data_layout() {
    // We manually build a DATA datagram with the same fields as
    // the empty-payload fixture and compare relevant bytes.
    let header = RtpsHeader {
        protocol_version: zerodds_rtps::wire_types::ProtocolVersion::V2_5,
        vendor_id: VendorId([0x01, 0x10]),
        guid_prefix: GuidPrefix::from_bytes([
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
        ]),
    };
    let data = DataSubmessage {
        extra_flags: 0,
        reader_id: EntityId::user_reader_with_key([0, 0, 1]),
        writer_id: EntityId::user_writer_with_key([0, 0, 1]),
        writer_sn: SequenceNumber(1),
        inline_qos: None,
        key_flag: false,
        non_standard_flag: false,
        serialized_payload: Vec::new().into(),
    };
    let our_bytes = encode_data_datagram(header, &[data]).unwrap();
    let cyclone_bytes = parse_hex(FRAME_DATA_EMPTY);

    // Header (20 byte) must be byte-identical.
    assert_eq!(
        &our_bytes[..20],
        &cyclone_bytes[..20],
        "RTPS-Header bytes must match Cyclone-Reference exactly"
    );
    // Submessage header (4 byte) must be byte-identical.
    assert_eq!(
        &our_bytes[20..24],
        &cyclone_bytes[20..24],
        "Submessage-Header bytes must match"
    );
    // DATA body (20 bytes) must be byte-identical.
    assert_eq!(
        &our_bytes[24..44],
        &cyclone_bytes[24..44],
        "DATA body bytes must match"
    );
    assert_eq!(our_bytes.len(), cyclone_bytes.len());
}

#[test]
fn zerodds_writer_produces_compatible_data_with_payload() {
    let header = RtpsHeader {
        protocol_version: zerodds_rtps::wire_types::ProtocolVersion::V2_5,
        vendor_id: VendorId([0x01, 0x10]),
        guid_prefix: GuidPrefix::from_bytes([
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
        ]),
    };
    // Payload: encapsulation (CDR2_LE) + u32 LE = 42
    let payload = vec![0x00, 0x11, 0x00, 0x00, 0x2A, 0x00, 0x00, 0x00];
    let data = DataSubmessage {
        extra_flags: 0,
        reader_id: EntityId::user_reader_with_key([0, 0, 1]),
        writer_id: EntityId::user_writer_with_key([0, 0, 1]),
        writer_sn: SequenceNumber(42),
        inline_qos: None,
        key_flag: false,
        non_standard_flag: false,
        serialized_payload: payload.into(),
    };
    let our_bytes = encode_data_datagram(header, &[data]).unwrap();
    let cyclone_bytes = parse_hex(FRAME_DATA_CDR2);
    assert_eq!(our_bytes, cyclone_bytes);
}

// ============================================================================
// Roundtrip: Cyclone → Decode → Encode → bit-identisch
// ============================================================================

#[test]
fn cyclone_data_empty_roundtrip_bit_identical() {
    let cyclone_bytes = parse_hex(FRAME_DATA_EMPTY);
    let parsed = decode_datagram(&cyclone_bytes).unwrap();
    let data = match &parsed.submessages[0] {
        ParsedSubmessage::Data(d) => d.clone(),
        other => panic!("expected DATA, got {other:?}"),
    };
    let our_bytes = encode_data_datagram(parsed.header, &[data]).unwrap();
    assert_eq!(our_bytes, cyclone_bytes);
}

#[test]
fn cyclone_data_cdr2_roundtrip_bit_identical() {
    let cyclone_bytes = parse_hex(FRAME_DATA_CDR2);
    let parsed = decode_datagram(&cyclone_bytes).unwrap();
    let data = match &parsed.submessages[0] {
        ParsedSubmessage::Data(d) => d.clone(),
        other => panic!("expected DATA, got {other:?}"),
    };
    let our_bytes = encode_data_datagram(parsed.header, &[data]).unwrap();
    assert_eq!(our_bytes, cyclone_bytes);
}

// ============================================================================
// Hex-Parser-Tests
// ============================================================================

#[test]
fn parse_hex_ignores_comments_and_whitespace() {
    let text = "# kommentar\n  AB CD  \nEF";
    assert_eq!(parse_hex(text), vec![0xAB, 0xCD, 0xEF]);
}

#[test]
fn parse_hex_handles_inline_comments() {
    assert_eq!(parse_hex("12 34 # rest ignored"), vec![0x12, 0x34]);
}

// ============================================================================
// WP 1.4 T1: SEDP-Publication-Compliance
// ============================================================================

#[test]
fn cyclone_sedp_publication_datagram_decodes() {
    // Decodes all submessages in the Cyclone SEDP publication datagram.
    // Expected: 4 DATA submessages with SEDP Publications writer_id.
    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let parsed = decode_datagram(&bytes).expect("decode");
    let data_count = parsed
        .submessages
        .iter()
        .filter(|s| matches!(s, ParsedSubmessage::Data(_)))
        .count();
    assert_eq!(data_count, 4, "expected 4 DATA submessages");

    for sub in &parsed.submessages {
        if let ParsedSubmessage::Data(d) = sub {
            assert_eq!(
                d.writer_id,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
                "SEDP publications writer_id mismatch"
            );
        }
    }
}

#[test]
fn cyclone_sedp_publication_parameter_list_parses() {
    // Takes the first DATA submessage from the Cyclone datagram and
    // parses its serialized_payload as PublicationBuiltinTopicData.
    // Erwartet: topic_name "DDSPerfCPUStats", type_name "CPUStats".
    use zerodds_rtps::publication_data::PublicationBuiltinTopicData;

    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let parsed = decode_datagram(&bytes).expect("decode");
    let first_data = parsed
        .submessages
        .iter()
        .find_map(|s| {
            if let ParsedSubmessage::Data(d) = s {
                Some(d)
            } else {
                None
            }
        })
        .expect("at least one DATA");

    let pub_data = PublicationBuiltinTopicData::from_pl_cdr_le(&first_data.serialized_payload)
        .expect("parse PublicationBuiltinTopicData");

    assert_eq!(pub_data.topic_name, "DDSPerfCPUStats");
    assert_eq!(pub_data.type_name, "CPUStats");
    // endpoint_guid must carry the expected writer-EntityId suffix
    // (Cyclone uses 0x02 = USER_WRITER_WITH_KEY for DataWriters,
    // but ddsperf builds internal EntityIds with kind 0x02).
    // We only check structurally here: prefix not all-zero + EntityId
    // not PARTICIPANT.
    use zerodds_rtps::wire_types::GuidPrefix;
    assert_ne!(pub_data.key.prefix, GuidPrefix::UNKNOWN);
    assert_ne!(pub_data.key.entity_id, EntityId::PARTICIPANT);
}

#[test]
fn cyclone_sedp_publication_all_four_topics_parse() {
    // Alle 4 Publications im Datagramm sollen parsen.
    use zerodds_rtps::publication_data::PublicationBuiltinTopicData;

    let bytes = parse_hex(FRAME_SEDP_PUBLICATION);
    let parsed = decode_datagram(&bytes).expect("decode");
    let topics: Vec<String> = parsed
        .submessages
        .iter()
        .filter_map(|s| {
            if let ParsedSubmessage::Data(d) = s {
                Some(d)
            } else {
                None
            }
        })
        .map(|d| {
            PublicationBuiltinTopicData::from_pl_cdr_le(&d.serialized_payload)
                .expect("parse")
                .topic_name
        })
        .collect();
    assert_eq!(
        topics,
        vec![
            "DDSPerfCPUStats",
            "DDSPerfRPingKS",
            "DDSPerfRDataKS",
            "DDSPerfRPongKS"
        ]
    );
}
