//! Wire-compliance tests for `ShapeType` against the XCDR2-LE spec.
//!
//! These tests verify that our encoder produces, byte-for-byte, the
//! layouts that CycloneDDS, Fast-DDS, and RTI ShapesDemo clients also
//! put on the wire. The expected byte sequences are derived by hand
//! from OMG XCDR2 §7.4 and match the values seen in pcap captures of
//! ShapesDemo traffic.

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

use zerodds_dcps::DdsType;
use zerodds_dcps::interop::ShapeType;

/// Reference sample for the "RED" ShapeType. Calculated by hand:
///
/// ```text
/// offset  0..3   : color.length = 4 (incl. null-terminator)  -> 04 00 00 00
/// offset  4..7   : "RED\0"                                    -> 52 45 44 00
/// offset  8..11  : x = 42                                     -> 2a 00 00 00
/// offset 12..15  : y = 77                                     -> 4d 00 00 00
/// offset 16..19  : shapesize = 30                             -> 1e 00 00 00
/// ```
const EXPECTED_RED_42_77_30: &[u8] = &[
    0x04, 0x00, 0x00, 0x00, // color.length
    0x52, 0x45, 0x44, 0x00, // "RED\0"
    0x2a, 0x00, 0x00, 0x00, // x = 42
    0x4d, 0x00, 0x00, 0x00, // y = 77
    0x1e, 0x00, 0x00, 0x00, // shapesize = 30
];

#[test]
fn shape_type_encode_red_matches_xcdr2_le_reference() {
    let sample = ShapeType::new("RED", 42, 77, 30);
    let mut buf = Vec::new();
    sample.encode(&mut buf).expect("encode");
    assert_eq!(
        buf.as_slice(),
        EXPECTED_RED_42_77_30,
        "encoder output deviates from the XCDR2-LE reference. Hex diff:\n  got: {}\n  exp: {}",
        hex(&buf),
        hex(EXPECTED_RED_42_77_30)
    );
}

#[test]
fn shape_type_decode_red_from_xcdr2_le_reference() {
    let decoded = ShapeType::decode(EXPECTED_RED_42_77_30).expect("decode");
    assert_eq!(decoded, ShapeType::new("RED", 42, 77, 30));
}

#[test]
fn shape_type_roundtrip_preserves_all_fields() {
    let cases = [
        ShapeType::new("BLUE", 0, 0, 30),
        ShapeType::new("GREEN", 100, 200, 30),
        ShapeType::new("MAGENTA", -50, -75, 45),
        ShapeType::new("", 0, 0, 0),   // empty string
        ShapeType::new("A", 1, 2, 3),  // 1-char (padding case)
        ShapeType::new("AB", 1, 2, 3), // 2-char
        ShapeType::new("ABCDEFG", i32::MAX, i32::MIN, 0xFF), // extremes
    ];

    for original in &cases {
        let mut buf = Vec::new();
        original.encode(&mut buf).expect("encode");
        let back = ShapeType::decode(&buf).expect("decode");
        assert_eq!(
            &back,
            original,
            "roundtrip mismatch for {original:?} (wire: {})",
            hex(&buf)
        );
    }
}

#[test]
fn shape_type_padding_after_short_color_aligns_x_to_4_bytes() {
    // "AB\0" is 3 bytes long; total after length+bytes+null is 4+3 = 7 bytes.
    // The next field (x: int32) needs 4-byte alignment → 1 byte padding.
    let sample = ShapeType::new("AB", 1, 2, 3);
    let mut buf = Vec::new();
    sample.encode(&mut buf).expect("encode");
    let expected: &[u8] = &[
        0x03, 0x00, 0x00, 0x00, // color.length = 3
        0x41, 0x42, 0x00, // "AB\0"
        0x00, // padding to 4-byte alignment
        0x01, 0x00, 0x00, 0x00, // x = 1
        0x02, 0x00, 0x00, 0x00, // y = 2
        0x03, 0x00, 0x00, 0x00, // shapesize = 3
    ];
    assert_eq!(
        buf.as_slice(),
        expected,
        "padding after a 2-byte string wrong. got: {} exp: {}",
        hex(&buf),
        hex(expected)
    );
}

#[test]
fn shape_type_type_name_matches_interop_convention() {
    // Any other spelling (zerodds::ShapeType, dds::ShapeType, ...) would
    // break SEDP topic-type matching with CycloneDDS/Fast-DDS.
    assert_eq!(ShapeType::TYPE_NAME, "ShapeType");
}

#[test]
fn shape_type_decode_truncated_fails_cleanly() {
    // 10 bytes is too few for a complete ShapeType — the decoder must
    // return an error, not panic.
    let result = ShapeType::decode(&[0x04, 0x00, 0x00, 0x00, 0x52, 0x45, 0x44, 0x00, 0x00, 0x00]);
    assert!(result.is_err(), "Truncated decode must fail");
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
