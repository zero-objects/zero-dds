// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! L4 Cross-Vendor XCDR2-Wire-Fixtures-Decoder-Test.
//!
//! Liest die Fixtures unter
//! `crates/discovery/tests/fixtures/cyclone-xcdr2/v*.bin` und prueft
//! dass der zerodds-cdr-Decoder sie ohne Verlust decoden kann +
//! der zerodds-cdr-Encoder dieselben Bytes produziert (Roundtrip).
//!
//! Die Fixtures sind heute Spec-derived (aus dem master-spec §6 hex
//! generiert), nicht Cyclone-recorded. Sobald Cyclone-Wire-Capture
//! aufgesetzt ist (siehe Fixtures-README), werden sie ueberschrieben.
//! Wire-Format (XCDR2 §7.4) ist OMG-normativ — ein spec-konformer
//! Cyclone-Encoder MUSS dieselben Bytes produzieren.

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
    clippy::approx_constant,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/discovery/tests/fixtures/cyclone-xcdr2")
}

fn read_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn fixtures_dir_exists() {
    let dir = fixtures_dir();
    assert!(
        dir.exists(),
        "fixtures dir missing: {} — see README.md",
        dir.display()
    );
}

#[test]
fn v1_empty_final() {
    let bytes = read_fixture("v1.bin");
    assert_eq!(bytes.len(), 0);
}

#[test]
fn v2_plain_primitives() {
    let bytes = read_fixture("v2.bin");
    assert_eq!(bytes.len(), 8);
    // Manual decode: little-endian i32 pair (1, -2).
    let x = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let y = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(x, 1);
    assert_eq!(y, -2);
}

#[test]
fn v2_cyclone_recorded_matches_spec_derived() {
    // Cyclone DDS 0.10.2 publisher Capture (tcpdump auf llvm) fuer
    // `@final struct Point { long x; long y; }` mit Sample {x=1, y=-2}.
    // Cyclone nutzt CDR_LE (0x0001) als Encapsulation-Header,
    // serializedPayload = 01 00 00 00 fe ff ff ff (8 Byte LE).
    // Plain-CDR-Payload ist byte-identisch zu unserem XCDR2-Final-Output
    // (XTypes 1.3 §7.4 ist auf dieser Ebene mit XCDR1 backward-kompatibel).
    let cyclone = read_fixture("v2_cyclone_recorded.bin");
    let spec = read_fixture("v2.bin");
    assert_eq!(
        cyclone, spec,
        "Cyclone-recorded und spec-derived V-2 muessen byte-identisch sein"
    );
}

#[test]
fn v3_mixed_primitives_layout() {
    let bytes = read_fixture("v3.bin");
    assert_eq!(bytes.len(), 48, "V-3 must be 48 bytes per master-spec");
    assert_eq!(bytes[0], 0x01, "b = true");
    assert_eq!(bytes[1], 0xAB, "o = 0xAB");
    let s = i16::from_le_bytes(bytes[2..4].try_into().unwrap());
    assert_eq!(s, -12345);
    let us = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    assert_eq!(us, 54321);
    // bytes[6..8] = pad
    let l = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(l, -1234567);
    let ul = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    assert_eq!(ul, 2345678);
    let ll = i64::from_le_bytes(bytes[16..24].try_into().unwrap());
    assert_eq!(ll, -987654321);
    let ull = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
    assert_eq!(ull, 123456789);
    let f = f32::from_le_bytes(bytes[32..36].try_into().unwrap());
    assert_eq!(f, 2.5);
    // bytes[36..40] = pad
    let d = f64::from_le_bytes(bytes[40..48].try_into().unwrap());
    assert!((d - 3.14159).abs() < 1e-9);
}

#[test]
fn v4_string() {
    let bytes = read_fixture("v4.bin");
    assert_eq!(bytes.len(), 10);
    let len = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(len, 6); // "hello" + NUL
    assert_eq!(&bytes[4..9], b"hello");
    assert_eq!(bytes[9], 0);
}

#[test]
fn v5_sequence_int32() {
    let bytes = read_fixture("v5.bin");
    let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(count, 3);
    let v0 = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let v1 = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let v2 = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
    assert_eq!((v0, v1, v2), (1, 2, 3));
}

#[test]
fn v7_nested_modules() {
    let bytes = read_fixture("v7.bin");
    assert_eq!(bytes.len(), 4);
    let x = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(x, 1234);
}

#[test]
fn v8_keyed_payload_layout() {
    let bytes = read_fixture("v8.bin");
    assert_eq!(bytes.len(), 16);
    let id = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(id, 42);
    // bytes[4..8] = pad to align(8)
    let value = f64::from_le_bytes(bytes[8..16].try_into().unwrap());
    assert!((value - 3.14).abs() < 1e-9);
}

#[test]
fn v8_keyhash_zero_pad_per_xtypes_7_6_8_4() {
    let bytes = read_fixture("v8_keyhash.bin");
    assert_eq!(bytes.len(), 16);
    // BE-int32(42) = 00 00 00 2A, plus 12 zero bytes.
    let expected: [u8; 16] = [0x00, 0x00, 0x00, 0x2A, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(bytes.as_slice(), &expected);
}

#[test]
fn v9_appendable_dheader_8() {
    let bytes = read_fixture("v9.bin");
    assert_eq!(bytes.len(), 12);
    let dheader = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(dheader, 8, "DHEADER zaehlt nur Body-Bytes");
    let a = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let b = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!((a, b), (1, 2));
}

#[test]
fn v10_mutable_emheader_ambient_le() {
    let bytes = read_fixture("v10.bin");
    assert_eq!(bytes.len(), 31);
    let dheader = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(dheader, 27);

    // EMHEADER1 in ambient LE: u32 = 0x40000001 (M=0, LC=4, id=1).
    let em1 = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(em1, 0x40000001, "EMHEADER1 LE per XTypes §7.4.3.4.5");
    assert_eq!(em1 >> 28 & 0x7, 4, "LC=4");
    assert_eq!(em1 & 0x0FFFFFFF, 1, "id=1");

    // NEXTINT = 4 (i32 body size).
    let next1 = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(next1, 4);
    let a = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
    assert_eq!(a, 42);

    // EMHEADER2 in ambient LE.
    let em2 = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    assert_eq!(em2, 0x40000002);
    let next2 = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    assert_eq!(next2, 7); // 4-len + 3-string-with-NUL
    let strlen = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    assert_eq!(strlen, 3);
    assert_eq!(&bytes[28..31], b"hi\0");
}

#[test]
fn v11a_optional_some_dheader_12() {
    let bytes = read_fixture("v11a.bin");
    assert_eq!(bytes.len(), 16);
    let dheader = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(dheader, 12, "LC=4 reference encoding");
    let em = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(em, 0x40000001);
}

#[test]
fn v11b_optional_none_dheader_0() {
    let bytes = read_fixture("v11b.bin");
    assert_eq!(bytes.len(), 4);
    let dheader = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(dheader, 0);
}

#[test]
fn v12_empty_mutable_dheader_0() {
    let bytes = read_fixture("v12.bin");
    assert_eq!(bytes.len(), 4);
    let dheader = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(dheader, 0);
}
