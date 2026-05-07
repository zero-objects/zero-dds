//! WP 1.10 T2 — RTPS Submessage Golden-Vector Tests.
//!
//! Liest Hex-Fixtures aus `tests/compliance/rtps/`, decodet via RTPS-
//! Submessage-Parser, encodet zurueck und prueft Byte-Identitaet.

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

#[path = "fixture_loader.rs"]
mod fixture_loader;

use zerodds_rtps::submessages::HeartbeatSubmessage;

#[test]
fn heartbeat_minimal_le_roundtrips() {
    let path = fixture_loader::compliance_root()
        .join("rtps")
        .join("heartbeat_minimal_le.hex");
    let bytes = fixture_loader::load_hex(&path).unwrap();
    assert_eq!(bytes.len(), HeartbeatSubmessage::WIRE_SIZE);

    let hb = HeartbeatSubmessage::read_body(&bytes, true, false, false, false).unwrap();
    assert_eq!(hb.reader_id.to_bytes(), [0x00, 0x00, 0x00, 0x04]);
    assert_eq!(hb.writer_id.to_bytes(), [0x00, 0x00, 0x00, 0x03]);
    assert_eq!(hb.count, 0x42);

    let (back, flags) = hb.write_body(true);
    assert_eq!(back, bytes, "heartbeat byte-identisch nach roundtrip");
    // E-Flag (little-endian) sollte gesetzt sein, F/L=false → 0x01.
    assert_eq!(flags & 0x01, 0x01);
}
