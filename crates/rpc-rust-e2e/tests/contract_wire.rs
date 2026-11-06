// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rpc-rust-e2e` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`: `src/lib.rs` includes the
//! DDS-RPC bindings `build.rs` generates and gives each module
//! `use zerodds_wire::{Reader, WireError, Writer}` (plus `ParameterRole`
//! for the modules with a `@mutable` type, `Narrowed` and `TryConstruct`
//! for the type-pair module). The generated code calls:
//! - `Writer::with_max_align(Endianness::Little, 4)` and
//!   `Reader::with_max_align(.., 4)` in each type's XCDR2 `DdsType`
//!   methods, `(.., 8)` in the XCDR1 ones;
//! - `Writer::continuing(Little, 8 or 4, prefix)` and `finish` in
//!   `encode_continuing`, which writes the `Call` union behind the
//!   `RequestHeader` (DDS-RPC 1.0 §7.5.1.1.6), and
//!   `Reader::with_max_align` with `skip_to(offset)` in
//!   `decode_continuing`;
//! - in the struct codecs: `write_pid`/`finish_pid`/`write_sentinel` and
//!   `read_pid`/`finish_pid` for a `@mutable` type under XCDR1,
//!   `write_emheader`/`finish_emheader`/`read_emheader` under XCDR2, and
//!   the primitive calls.
//!
//! The tests call `zerodds-wire` the way the generated code does; the
//! generated types are exercised by this crate's other targets.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.3.2.1 Table 36 (alignment is relative to
//! the origin, the start of the sample), §7.4.3.2.2 Table 37 (MAXALIGN 8
//! for version 1, 4 for version 2), §7.4.1.2 and Table 34 (PL_CDR: a
//! 16-bit id and 16-bit length per parameter, `PID_LIST_END` 0x3F02 with
//! the must-understand flag, length 0), §7.4.3.4.2 (EMHEADER1 =
//! (M_FLAG << 31) + (LC << 28) + id; LC 2 is a 4-octet member), §7.4.3.4.1.
#![allow(clippy::unwrap_used)]

use zerodds_wire::{Endianness, ParameterRole, Reader, WireError, Writer};

const TWO: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0x40];

fn continuing(prefix_len: usize, max_align: usize) -> Vec<u8> {
    let mut w = Writer::continuing(Endianness::Little, max_align, vec![0xee; prefix_len]);
    w.write_i32(0x11);
    w.write_i32(1);
    w.write_f64(2.0);
    w.finish()
}

#[test]
fn xtypes_7_4_3_2_1_a_continuing_writer_aligns_from_the_start_of_the_prefix() {
    // XCDR1, header of 28: the double goes to 40, four octets of padding.
    let mut expected = vec![0xee; 28];
    expected.extend_from_slice(&[0x11, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
    expected.extend_from_slice(&TWO);
    assert_eq!(continuing(28, 8), expected);

    // XCDR1, header of 32: the double is at 40 with no padding.
    let mut expected = vec![0xee; 32];
    expected.extend_from_slice(&[0x11, 0, 0, 0, 1, 0, 0, 0]);
    expected.extend_from_slice(&TWO);
    assert_eq!(continuing(32, 8), expected);

    // XCDR2, header of 28: MAXALIGN 4 puts the double at 36.
    let mut expected = vec![0xee; 28];
    expected.extend_from_slice(&[0x11, 0, 0, 0, 1, 0, 0, 0]);
    expected.extend_from_slice(&TWO);
    assert_eq!(continuing(28, 4), expected);
}

#[test]
fn xtypes_7_4_3_2_1_a_reader_skipped_to_an_offset_aligns_from_the_start_of_the_sample() {
    let mut frame = vec![0xee; 28];
    frame.extend_from_slice(&[0x11, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
    frame.extend_from_slice(&TWO);

    let mut r = Reader::with_max_align(&frame, Endianness::Little, 8);
    r.skip_to(28).unwrap();
    assert_eq!(r.read_i32().unwrap(), 0x11);
    assert_eq!(r.read_i32().unwrap(), 1);
    assert_eq!(r.read_f64().unwrap().to_bits(), 2.0_f64.to_bits());
    assert_eq!(r.remaining(), 0);

    // A frame shorter than the header offset is an error.
    let mut r = Reader::with_max_align(&frame[..20], Endianness::Little, 8);
    assert!(matches!(r.skip_to(28), Err(WireError::Truncated { .. })));
}

/// `@mutable struct Mutated { long m; @id(7) long n; }` with m = 3, n = 4.
const MUTATED_PL_CDR: [u8; 20] = [
    0x00, 0x00, 0x04, 0x00, 0x03, 0x00, 0x00, 0x00, // id 0, length 4
    0x07, 0x00, 0x04, 0x00, 0x04, 0x00, 0x00, 0x00, // id 7, length 4
    0x02, 0x7f, 0x00, 0x00, // PID_LIST_END | must understand, length 0
];

#[test]
fn xtypes_7_4_1_2_a_mutable_type_under_xcdr1_is_a_parameter_list_with_a_sentinel() {
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    let m = w.write_pid(0, ParameterRole::Member, false).unwrap();
    w.write_i32(3);
    w.finish_pid(m);
    let n = w.write_pid(7, ParameterRole::Member, false).unwrap();
    w.write_i32(4);
    w.finish_pid(n);
    w.write_sentinel();
    assert_eq!(w.finish(), MUTATED_PL_CDR);

    let mut r = Reader::with_max_align(&MUTATED_PL_CDR, Endianness::Little, 8);
    let header = r.read_pid().unwrap();
    assert_eq!((header.pid, header.length, header.end), (0, 4, false));
    assert_eq!(r.read_i32().unwrap(), 3);
    r.finish_pid(header).unwrap();
    let header = r.read_pid().unwrap();
    assert_eq!((header.pid, header.length, header.end), (7, 4, false));
    assert_eq!(r.read_i32().unwrap(), 4);
    r.finish_pid(header).unwrap();
    assert!(r.read_pid().unwrap().end);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_type_under_xcdr2_has_one_emheader_per_member() {
    let expected = [
        0x10, 0x00, 0x00, 0x00, // DHEADER: 16
        0x00, 0x00, 0x00, 0x20, 0x03, 0x00, 0x00, 0x00, // LC 2, id 0
        0x07, 0x00, 0x00, 0x20, 0x04, 0x00, 0x00, 0x00, // LC 2, id 7
    ];
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    let dheader = w.write_dheader();
    let m = w.write_emheader(0, 2, false).unwrap();
    w.write_i32(3);
    w.finish_emheader(m).unwrap();
    let n = w.write_emheader(7, 2, false).unwrap();
    w.write_i32(4);
    w.finish_emheader(n).unwrap();
    w.finish_dheader(dheader);
    assert_eq!(w.finish(), expected);

    let mut r = Reader::with_max_align(&expected, Endianness::Little, 4);
    assert_eq!(r.read_dheader().unwrap(), 16);
    let header = r.read_emheader().unwrap();
    assert_eq!(
        (header.member_id, header.length_code, header.extent),
        (0, 2, 4)
    );
    assert!(!header.must_understand);
    assert_eq!(r.read_i32().unwrap(), 3);
    let header = r.read_emheader().unwrap();
    assert_eq!(
        (header.member_id, header.length_code, header.extent),
        (7, 2, 4)
    );
    assert_eq!(r.read_i32().unwrap(), 4);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn xtypes_7_4_3_4_2_a_member_of_another_size_than_its_length_code_is_refused() {
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    let header = w.write_emheader(1, 2, false).unwrap();
    w.write_u16(5);
    assert!(matches!(
        w.finish_emheader(header),
        Err(WireError::OutOfRange { .. })
    ));
}
