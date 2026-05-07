//! Wire-Compliance-Tests fuer `ShapeType` gegen XCDR2-LE-Spec.
//!
//! Diese Tests pruefen, dass unser Encoder byte-genau die Layouts
//! produziert, die auch Cyclone-, Fast-DDS- und RTI-ShapesDemo-Clients
//! auf die Wire legen. Die erwarteten Byte-Sequenzen sind von Hand
//! aus OMG XCDR2 §7.4 hergeleitet und entsprechen den Werten, die
//! pcap-Captures aus ShapesDemo-Traffic zeigen.

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

/// Referenz-Sample fuer "RED"-ShapeType. Von Hand kalkuliert:
///
/// ```text
/// offset  0..3   : color.length = 4 (inkl. null-terminator)  -> 04 00 00 00
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
        "Encoder-Output weicht von XCDR2-LE Referenz ab. Hex-Diff:\n  got: {}\n  exp: {}",
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
    // "AB\0" ist 3 Bytes lang, Gesamt nach length+bytes+null ist 4+3 = 7 Bytes.
    // Das naechste Feld (x: int32) braucht 4-Byte-Alignment → 1 Byte Padding.
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
        "Padding nach 2-Byte-String falsch. got: {} exp: {}",
        hex(&buf),
        hex(expected)
    );
}

#[test]
fn shape_type_type_name_matches_interop_convention() {
    // Jede andere Schreibweise (zerodds::ShapeType, dds::ShapeType, ...)
    // wuerde SEDP-Topic-Type-Matching mit Cyclone/Fast-DDS brechen.
    assert_eq!(ShapeType::TYPE_NAME, "ShapeType");
}

#[test]
fn shape_type_decode_truncated_fails_cleanly() {
    // 10 Byte ist zu wenig fuer einen vollstaendigen ShapeType — Decoder
    // muss Error liefern, nicht panic.
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
