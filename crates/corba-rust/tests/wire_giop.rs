// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire-Conformance-Tests fuer den ValueBase-Stream (§8.3 + §4.4).
//!
//! Belegt das Roundtrip-Verhalten der ValueStreamWriter/ValueStreamReader
//! gegen byte-genaue Erwartungswerte:
//! * single-repo-id value-tag (CDR §15.3.4.2 — `0x7FFFFF02`),
//! * multi-repo-id list (CDR §15.3.4.2 — `0x7FFFFF06`),
//! * chunked-encoding mit list (CDR §15.3.4.3 — `0x7FFFFF0A`).

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

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_rust::{ValueStreamReader, ValueStreamWriter, ValueTagHeader};

#[test]
fn wire_value_tag_roundtrip() {
    let repo_id = "IDL:Demo/Point:1.0";
    let mut writer = BufferWriter::new(Endianness::Little);

    {
        let mut vw = ValueStreamWriter::new(&mut writer);
        vw.write_value_tag(repo_id).expect("write tag");
    }

    let bytes = writer.into_bytes();
    assert!(bytes.len() >= 4, "must contain at least the tag");

    // Tag-Praefix muss 0x7FFFFF02 in Little-Endian sein.
    assert_eq!(&bytes[0..4], &0x7FFF_FF02_u32.to_le_bytes());

    let mut reader = BufferReader::new(&bytes, Endianness::Little);
    let mut vr = ValueStreamReader::new(&mut reader);
    let read_id = vr.read_value_tag().expect("read tag");
    assert_eq!(read_id.as_deref(), Some(repo_id));
}

#[test]
fn wire_value_tag_null_reference() {
    // Null-Value-Tag = 0x00000000 (CDR §15.3.4.2).
    let bytes = 0x0000_0000_u32.to_le_bytes();
    let mut reader = BufferReader::new(&bytes, Endianness::Little);
    let mut vr = ValueStreamReader::new(&mut reader);
    let result = vr.read_value_tag().expect("read null");
    assert!(result.is_none(), "null tag must decode to None");
}

#[test]
fn wire_value_tag_truly_unsupported_returns_error() {
    // 0x7FFFFF22 ist im aktuellen Wire-Codec nicht implementiert
    // (z.B. codeset-Variante).
    let bytes = 0x7FFF_FF22_u32.to_le_bytes();
    let mut reader = BufferReader::new(&bytes, Endianness::Little);
    let mut vr = ValueStreamReader::new(&mut reader);
    let result = vr.read_value_tag();
    assert!(result.is_err(), "unsupported tag must error");
}

#[test]
fn wire_value_tag_multi_repo_id_list_round_trip() {
    let ids = ["IDL:Derived:1.0", "IDL:Base:1.0"];
    let mut writer = BufferWriter::new(Endianness::Little);
    {
        let mut vw = ValueStreamWriter::new(&mut writer);
        vw.write_value_tag_multi(&ids).expect("write multi tag");
    }
    let bytes = writer.into_bytes();
    assert_eq!(&bytes[0..4], &0x7FFF_FF06_u32.to_le_bytes());

    let mut reader = BufferReader::new(&bytes, Endianness::Little);
    let mut vr = ValueStreamReader::new(&mut reader);
    let header = vr.read_value_tag_full().expect("read tag");
    match header {
        ValueTagHeader::List(read_ids) => {
            assert_eq!(read_ids.len(), 2);
            assert_eq!(read_ids[0], ids[0]);
            assert_eq!(read_ids[1], ids[1]);
        }
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn wire_value_tag_chunked_with_list_round_trip() {
    // Schreibt: chunked-tag → 2 repo-ids → chunk(0xAA, 0xBB, 0xCC) →
    // chunk(0xDD, 0xEE) → end-tag (-1, outermost value).
    let ids = ["IDL:Derived:1.0", "IDL:Base:1.0"];
    let chunk1 = [0xAA_u8, 0xBB, 0xCC];
    let chunk2 = [0xDD_u8, 0xEE];

    let mut writer = BufferWriter::new(Endianness::Little);
    {
        let mut vw = ValueStreamWriter::new(&mut writer);
        vw.write_chunked_value_tag(&ids).expect("chunked tag");
        vw.write_chunk(&chunk1).expect("chunk1");
        vw.write_chunk(&chunk2).expect("chunk2");
        vw.write_chunked_end(1).expect("end tag");
    }

    let bytes = writer.into_bytes();
    assert_eq!(&bytes[0..4], &0x7FFF_FF0A_u32.to_le_bytes());

    let mut reader = BufferReader::new(&bytes, Endianness::Little);
    let mut vr = ValueStreamReader::new(&mut reader);
    let header = vr.read_value_tag_full().expect("read tag");
    let read_ids = match header {
        ValueTagHeader::ChunkedList(ids) => ids,
        other => panic!("expected ChunkedList, got {other:?}"),
    };
    assert_eq!(read_ids.len(), 2);
    assert_eq!(read_ids[0], ids[0]);

    // Erster Chunk: size = 3, dann 3 bytes.
    let s1 = vr.read_chunk_size().expect("chunk1 size");
    assert_eq!(s1, 3);
    // Zweiter Chunk: size = 2, dann 2 bytes.
    // Wir muessen die Bytes selbst aus dem Buffer lesen — dafuer
    // klammern wir den Reader auf und greifen direkt zu.
    drop(vr);
    let mut reader2 = BufferReader::new(&bytes, Endianness::Little);
    let mut vr2 = ValueStreamReader::new(&mut reader2);
    let _ = vr2.read_value_tag_full().expect("re-read tag");
    let s1b = vr2.read_chunk_size().expect("chunk1 size");
    assert_eq!(s1b, 3);
    let mut buf1 = [0_u8; 3];
    for slot in &mut buf1 {
        *slot = reader2.read_u8().expect("chunk1 byte");
    }
    assert_eq!(buf1, chunk1);

    // chunk2.
    let s2 = {
        let mut vr3 = ValueStreamReader::new(&mut reader2);
        vr3.read_chunk_size().expect("chunk2 size")
    };
    assert_eq!(s2, 2);
    let mut buf2 = [0_u8; 2];
    for slot in &mut buf2 {
        *slot = reader2.read_u8().expect("chunk2 byte");
    }
    assert_eq!(buf2, chunk2);

    // End-Tag.
    let end_raw = reader2.read_u32().expect("end tag");
    let end = end_raw as i32;
    assert_eq!(end, -1, "outermost end-tag must be -1");
}
