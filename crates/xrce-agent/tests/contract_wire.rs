// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xrce-agent` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier"). Built with the
//! `daemon` feature (`required-features` in `Cargo.toml`): only the daemon
//! links `zerodds-wire`.
//!
//! What the daemon uses of `zerodds-wire` (`src/daemon/binary_forms.rs`):
//! it reads the `REPRESENTATION_IN_BINARY` structures of DDS-XRCE 1.0
//! §7.7.3 with `Reader::with_max_align(bytes, Big or Little, 4)` (XCDR2,
//! §7.7.3.1.3). The Annex A form: `read_dheader`, `position`, the members,
//! then `skip_to(end)` over appended members and `remaining() == 0`. The
//! form of the clauses: the members to the end of the blob. Members are
//! read with `read_bool` (the `is_present` flag of each `@optional`
//! member), `read_string`, `read_u16`, `read_u32`, `read_i32` and
//! `read_u8`; a `WireError` becomes a `ReprError::Malformed` through its
//! `Display`. An error of any read makes the daemon try the next form.
//!
//! Oracle: DDS-XRCE 1.0 §7.7.3.1.3 (the binary representations are XCDR
//! version 2), §7.7.3.7.3 (`OBJK_Topic_Binary`), §7.7.3.10.2
//! (`OBJK_DataWriter_Binary`); DDS-XTypes 1.3 §7.4.2 (DELIMITED_CDR: a
//! DHEADER ahead of the members), §7.4.3.4.1, §7.4.3.5.2 (an optional
//! member in PLAIN_CDR2 is a Boolean `is_present` then the value),
//! §7.4.1.1.1 Table 31 (Boolean 0 or 1, UInt16 aligned to 2, UInt32 to 4,
//! MAXALIGN 4).
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, Reader, WireError};

fn xcdr2(bytes: &[u8], endianness: Endianness) -> Reader<'_> {
    Reader::with_max_align(bytes, endianness, 4)
}

/// `OBJK_Topic_Binary` in Annex A's form, little-endian: `topic_name` "T",
/// `type_reference` present "X", `type_identifier` absent, followed by
/// four octets of a member a later version appended.
const TOPIC_ANNEX_A: [u8; 24] = [
    0x14, 0x00, 0x00, 0x00, // DHEADER: 20
    0x02, 0x00, 0x00, 0x00, b'T', 0x00, // topic_name
    0x01, // type_reference is present
    0x00, // padding
    0x02, 0x00, 0x00, 0x00, b'X', 0x00, // type_reference
    0x00, // type_identifier is absent
    0x00, // padding
    0xaa, 0xbb, 0xcc, 0xdd, // appended member
];

#[test]
fn xtypes_7_4_3_4_1_annex_a_form_reads_inside_its_dheader_and_skips_the_rest() {
    let mut r = xcdr2(&TOPIC_ANNEX_A, Endianness::Little);
    let length = r.read_dheader().unwrap();
    assert_eq!(length, 20);
    let end = r.position() + length;
    assert_eq!(end, 24);
    assert_eq!(r.read_string().unwrap(), "T");
    assert!(r.read_bool().unwrap());
    assert_eq!(r.read_string().unwrap(), "X");
    assert!(!r.read_bool().unwrap());
    assert_eq!(r.position(), 19);
    r.skip_to(end).unwrap();
    assert_eq!(r.remaining(), 0);
}

/// `OBJK_DataWriter_Binary` in the form of the clauses, big-endian:
/// `topic_name` "T", `qos_flags` 0x0102, `history_depth` 5,
/// `deadline_msec` 100, no `lifespan_msec`, `user_data` [aa bb], no
/// `ownership_strength`.
const WRITER_CLAUSES_BE: [u8; 31] = [
    0x00, 0x00, 0x00, 0x02, b'T', 0x00, // topic_name, 0..6
    0x01, 0x02, // qos_flags at 6
    0x01, 0x00, 0x00, 0x05, // history_depth present, padding, u16 at 10
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64, // deadline present, u32 at 16
    0x00, // lifespan absent
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xaa, 0xbb, // user_data, count at 24
    0x00, // ownership_strength absent
];

#[test]
fn xtypes_7_4_3_5_2_optional_members_are_a_flag_then_the_value_aligned_in_the_stream() {
    let mut r = xcdr2(&WRITER_CLAUSES_BE, Endianness::Big);
    assert_eq!(r.read_string().unwrap(), "T");
    assert_eq!(r.read_u16().unwrap(), 0x0102);
    assert!(r.read_bool().unwrap());
    assert_eq!(r.read_u16().unwrap(), 5);
    assert!(r.read_bool().unwrap());
    assert_eq!(r.read_u32().unwrap(), 100);
    assert!(!r.read_bool().unwrap());
    assert!(r.read_bool().unwrap());
    let n = r.read_u32().unwrap() as usize;
    assert_eq!(n, 2);
    assert!(n <= r.remaining());
    assert_eq!((r.read_u8().unwrap(), r.read_u8().unwrap()), (0xaa, 0xbb));
    assert!(!r.read_bool().unwrap());
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_4_1_1_1_the_clause_form_domain_id_is_an_int32() {
    let mut r = xcdr2(&[0xff, 0xff, 0xff, 0xfe, 0x00], Endianness::Big);
    assert_eq!(r.read_i32().unwrap(), -2);
    assert!(!r.read_bool().unwrap());
}

#[test]
fn reads_that_do_not_fit_the_form_are_errors_so_the_next_form_is_tried() {
    // A boolean that is neither 0 nor 1.
    let mut r = xcdr2(&[0x02], Endianness::Little);
    assert!(matches!(
        r.read_bool(),
        Err(WireError::OutOfRange { at: 0, .. })
    ));

    // A DHEADER longer than the blob.
    let mut r = xcdr2(&[0x08, 0x00, 0x00, 0x00, 0x00], Endianness::Little);
    let error = r.read_dheader().unwrap_err();
    assert!(matches!(error, WireError::ImpossibleLength { at: 0, .. }));
    assert!(!error.to_string().is_empty());

    // An end behind the cursor.
    let mut r = xcdr2(&TOPIC_ANNEX_A, Endianness::Little);
    r.read_dheader().unwrap();
    r.read_string().unwrap();
    assert!(matches!(r.skip_to(4), Err(WireError::OutOfRange { .. })));

    // The Annex A blob read as the clause form: the DHEADER is taken for a
    // string length that runs past the end.
    let mut r = xcdr2(&TOPIC_ANNEX_A, Endianness::Little);
    assert!(r.read_string().is_err());
}
