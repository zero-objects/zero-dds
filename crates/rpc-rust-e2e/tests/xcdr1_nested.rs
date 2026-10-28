// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The classic-CDR (XCDR1) form of nested `@appendable` and `@mutable`
//! members in the generated topic types, against the octets XTypes 1.3
//! §7.4.1 gives.
//!
//! * `@appendable` under XCDR1 is plain CDR: no DHEADER (§7.4.1.1,
//!   generic rule (29)).
//! * `@mutable` under XCDR1 is a parameter list (§7.4.1.2): per member a
//!   two-octet parameter id and a two-octet length, the value, padding to
//!   four octets, and `PID_LIST_END` (0x3F02 with the must-understand flag:
//!   0x7F02) with length 0 at the end.
//! * An 8-octet member aligns to 8 counted from the start of the sample
//!   (MAXALIGN of version 1), and a value that continues a sample is
//!   aligned there, not from its own first octet.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_dcps::dds_type::DdsType;
use zerodds_rpc::rpc_hash::rpc_member_hash;
use zerodds_rpc_rust_e2e::nested_bindings::nested::{Appended, Mutated, Store_Call, Store_put_In};
use zerodds_wire::{Endianness, Reader, Writer};

fn appended() -> Appended {
    Appended { a: 1, b: 2.0 }
}

fn mutated() -> Mutated {
    Mutated { m: 3, n: 4 }
}

const TWO: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0x40];

#[test]
fn an_appendable_member_has_no_dheader_under_xcdr1() {
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    appended().encode_xcdr1(&mut w).unwrap();
    let mut expected = vec![1, 0, 0, 0, 0, 0, 0, 0];
    expected.extend_from_slice(&TWO);
    assert_eq!(w.finish(), expected, "long, four octets of padding, double");

    // XCDR2 delimits it: DHEADER = 12, then the members aligned to 4.
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    appended().encode(&mut w).unwrap();
    let mut expected = vec![12, 0, 0, 0, 1, 0, 0, 0];
    expected.extend_from_slice(&TWO);
    assert_eq!(w.finish(), expected);
}

#[test]
fn a_mutable_member_is_a_parameter_list_under_xcdr1() {
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    mutated().encode_xcdr1(&mut w).unwrap();
    assert_eq!(
        w.finish(),
        vec![
            0x00, 0x00, 0x04, 0x00, 3, 0, 0, 0, // member id 0, length 4
            0x07, 0x00, 0x04, 0x00, 4, 0, 0, 0, // @id(7), length 4
            0x02, 0x7F, 0x00, 0x00, // PID_LIST_END with the must-understand flag
        ]
    );
}

#[test]
fn the_nested_members_are_read_back() {
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    mutated().encode_xcdr1(&mut w).unwrap();
    let bytes = w.finish();
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    assert_eq!(Mutated::decode_xcdr1(&mut r).unwrap(), mutated());

    let mut w = Writer::with_max_align(Endianness::Little, 8);
    appended().encode_xcdr1(&mut w).unwrap();
    let bytes = w.finish();
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    assert_eq!(Appended::decode_xcdr1(&mut r).unwrap(), appended());
}

/// The `Call` union continues a sample behind a header of 32 octets: its
/// discriminator is at 32, `a.a` at 36, and `a.b` — an 8-octet member — at
/// 40, a multiple of 8 in the sample, so there is no padding before it. The
/// standalone `Appended` above has four octets of padding there: counted
/// from the union's own first octet, `a.b` would have been placed at 8
/// (offset 40 in the sample) only by accident of this header's length.
#[test]
fn a_value_that_continues_a_sample_aligns_to_the_sample() {
    let call = Store_Call::Put(Store_put_In {
        a: appended(),
        m: mutated(),
    });
    let mut frame = vec![0u8; 32];
    call.encode_continuing(&mut frame, true).unwrap();

    let mut expected = vec![0u8; 32];
    expected.extend_from_slice(&(rpc_member_hash("put") as i32).to_le_bytes());
    expected.extend_from_slice(&[1, 0, 0, 0]); // a.a at 36
    expected.extend_from_slice(&TWO); // a.b at 40, no padding
    expected.extend_from_slice(&[
        0x00, 0x00, 0x04, 0x00, 3, 0, 0, 0, //
        0x07, 0x00, 0x04, 0x00, 4, 0, 0, 0, //
        0x02, 0x7F, 0x00, 0x00,
    ]);
    assert_eq!(frame, expected);

    assert_eq!(
        Store_Call::decode_continuing(&frame, 32, true).unwrap(),
        call
    );
}

/// The default of the Basic mapping is classic CDR, so a sample that is
/// written XCDR2 is another sample.
#[test]
fn xcdr2_frames_the_same_value_differently() {
    let call = Store_Call::Put(Store_put_In {
        a: appended(),
        m: mutated(),
    });
    let mut one = vec![0u8; 32];
    call.encode_continuing(&mut one, true).unwrap();
    let mut two = vec![0u8; 32];
    call.encode_continuing(&mut two, false).unwrap();
    assert_ne!(one, two);
    assert_eq!(
        Store_Call::decode_continuing(&two, 32, false).unwrap(),
        call
    );
}
