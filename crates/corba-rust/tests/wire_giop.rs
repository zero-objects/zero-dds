// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `ValueBase` stream (zerodds-corba-rust 1.1 §9) against the value
//! encoding of CORBA §15.3.4: the expected octets are written out by hand
//! from the spec, not taken from the writer.
//!
//! * one repository id: `value_tag` `0x7fffff02` (§15.3.4.1),
//! * a list of repository ids: `0x7fffff06`,
//! * chunked with a list: `0x7fffff0e`, chunk sizes and the end tag
//!   (§15.3.4.3).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_corba_rust::{ValueStreamReader, ValueStreamWriter, ValueTagHeader};
use zerodds_wire::{Endianness, Reader, Writer};

/// Octets from a hex string; blanks separate the fields.
fn octets(hex: &str) -> Vec<u8> {
    let digits: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    (0..digits.len() / 2)
        .map(|i| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// `IDL:Derived:1.0` and `IDL:Base:1.0`, each a CDR string: length,
/// characters, NUL.
const DERIVED_BASE_LE: &str = "10000000 49444c3a446572697665643a312e3000 \
                               0d000000 49444c3a426173653a312e3000";

#[test]
fn corbarust_9_a_value_tag_is_the_tag_and_the_repository_id() {
    // 18 characters, a CDR string of 19 octets.
    let expected = octets("02ffff7f 13000000 49444c3a44656d6f2f506f696e743a312e3000");
    let mut writer = Writer::new(Endianness::Little);
    ValueStreamWriter::new(&mut writer).write_value_tag("IDL:Demo/Point:1.0");
    assert_eq!(writer.finish(), expected);

    let mut reader = Reader::new(&expected, Endianness::Little);
    let read_id = ValueStreamReader::new(&mut reader)
        .read_value_tag()
        .expect("read tag");
    assert_eq!(read_id.as_deref(), Some("IDL:Demo/Point:1.0"));
}

#[test]
fn corbarust_9_the_null_tag_reads_as_no_value() {
    let bytes = octets("00000000");
    let mut reader = Reader::new(&bytes, Endianness::Little);
    let result = ValueStreamReader::new(&mut reader)
        .read_value_tag()
        .expect("read null");
    assert!(result.is_none(), "null tag must decode to None");
}

#[test]
fn corbarust_9_a_tag_with_a_bit_outside_15_3_4_1_is_refused() {
    // Bit 5 is none of codebase (0), type information (1-2), chunked (3).
    let bytes = octets("22ffff7f");
    let mut reader = Reader::new(&bytes, Endianness::Little);
    assert!(
        ValueStreamReader::new(&mut reader)
            .read_value_tag()
            .is_err()
    );
}

#[test]
fn corbarust_9_a_list_of_repository_ids_is_the_count_and_the_strings() {
    let expected = octets(&format!("06ffff7f 02000000 {DERIVED_BASE_LE}"));
    let mut writer = Writer::new(Endianness::Little);
    ValueStreamWriter::new(&mut writer).write_value_tag_multi(&["IDL:Derived:1.0", "IDL:Base:1.0"]);
    assert_eq!(writer.finish(), expected);

    let mut reader = Reader::new(&expected, Endianness::Little);
    let header = ValueStreamReader::new(&mut reader)
        .read_value_tag_full()
        .expect("read tag");
    assert_eq!(
        header,
        ValueTagHeader::List(vec!["IDL:Derived:1.0".into(), "IDL:Base:1.0".into()])
    );
}

#[test]
fn corbarust_9_chunks_carry_their_size_on_a_long_boundary_and_end_with_minus_1() {
    // Two chunks of 3 and 2 octets; each size and the end tag a long on a
    // 4-octet boundary, the padding outside the chunk it follows.
    let expected = octets(&format!(
        "0effff7f 02000000 {DERIVED_BASE_LE} 000000 \
         03000000 aabbcc 00 02000000 ddee 0000 ffffffff"
    ));
    let mut writer = Writer::new(Endianness::Little);
    {
        let mut vw = ValueStreamWriter::new(&mut writer);
        vw.write_chunked_value_tag(&["IDL:Derived:1.0", "IDL:Base:1.0"]);
        vw.write_chunk(&[0xAA, 0xBB, 0xCC]);
        vw.write_chunk(&[0xDD, 0xEE]);
        vw.write_chunked_end(1);
    }
    assert_eq!(writer.finish(), expected);

    let mut reader = Reader::new(&expected, Endianness::Little);
    let header = ValueStreamReader::new(&mut reader)
        .read_value_tag_full()
        .expect("read tag");
    assert_eq!(
        header,
        ValueTagHeader::ChunkedList(vec!["IDL:Derived:1.0".into(), "IDL:Base:1.0".into()])
    );
    let first = ValueStreamReader::new(&mut reader)
        .read_chunk_size()
        .expect("first chunk size");
    assert_eq!(first, 3);
    assert_eq!(
        reader.read_bytes(3).expect("first chunk"),
        [0xAA, 0xBB, 0xCC]
    );
    let second = ValueStreamReader::new(&mut reader)
        .read_chunk_size()
        .expect("second chunk size");
    assert_eq!(second, 2);
    assert_eq!(reader.read_bytes(2).expect("second chunk"), [0xDD, 0xEE]);
    let end = ValueStreamReader::new(&mut reader)
        .read_chunk_size()
        .expect("end tag");
    assert_eq!(end, -1, "the end tag of the outermost value");
}
