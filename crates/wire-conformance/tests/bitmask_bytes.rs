// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bytes the generated `bitmask` and `bitset` codecs put on the
//! wire, held against the C and Go columns' own octets.
//!
//! # Why this file exists
//!
//! The Rust column was the only one of the twenty-one that refused a
//! `bitmask` as a struct member. `@bit_bound(16) bitmask Flags { READY
//! }; struct S { Flags bits; };` answered `NoWriter { member: "bits",
//! kind: "bitmask" }` while C, Go and Java produced the same model with
//! `0 error(s)`. A `bitset` member was refused the same way, and a
//! `bitmask` **declaration** produced `impl Flags { .. }` over
//! `pub type Flags = u16;` — E0390, so a model with a bitmask in it had
//! no compiling Rust output at all.
//!
//! # Where the expected octets come from
//!
//! Not from a calculation. The three constants below were produced by
//! generating the same IDL for the **C** and the **Go** column,
//! compiling each against its own runtime and running it. Both columns
//! answered the same octets, and the constants are what they answered.
//!
//! Measured 2026-09-06, `zerodds-idlc generate --lang {c,go} --encoding
//! xcdr2`, `runtime/c` under Apple clang and `runtime/go` under go1.x,
//! writing the samples this file builds:
//!
//! ```text
//! Masks   status=0 len=16 81000180010000800100000000000080
//! MaskSeq status=0 len=20 0800000002000000010000800108000007000000
//! Packet  status=0 len=8  850a000007000000
//! ```
//!
//! # What the octets settle
//!
//! * **The holder is the `@bit_bound`'s**, Table 33 of XTypes 1.3
//!   §7.4.1.1.3.2. `B8` is one octet, `B16` two, `B32` four, `B64`
//!   eight — visible in `Masks` as `81` against `0180` against
//!   `01000080`, and in the padding between them. The width is asked of
//!   `zerodds_ast_layout::plan::scalar_write`, the same crossing the
//!   other twenty columns go through, rather than computed in the Rust
//!   emitter.
//! * **A bound that is not a holder width still takes the holder.**
//!   `B12` is two octets with four bits no bit of the type can reach:
//!   `0108` in `MaskSeq`, and the `long` after it aligned to four.
//! * **A bitset's positions run cumulatively, padding included.**
//!   `Packed`'s `pb` sits at position 7 and not at position 3, because
//!   the anonymous `bitfield<4>` between them moves it: `0xa85` and not
//!   `0xad`.
//!
//! # The mutations
//!
//! * `Writer::write_bitmask`: take `9..=16 => self.write_u32(..)`
//!   instead of `write_u16`. [`the_bytes_are_the_c_and_go_columns`]
//!   fails at byte 1 — `Masks.b` starts a four-octet holder where the
//!   two columns wrote two.
//! * `crates/lang-rules/src/encode.rs`, the `TypeKind::Bitmask` arm:
//!   `bits: 32` in place of the plan's answer. The same red, from the
//!   generator instead of from the runtime.
//! * `bitset_fields`: advance `position` only for a **named** field.
//!   [`a_bitsets_positions_run_through_its_padding`] fails —
//!   `Packed` becomes `0xad`, which is what leaving the padding out of
//!   the walk means.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire_conformance::generated::{
    B8_HIGH, B8_LOW, B12_HIGH, B12_LOW, B16_HIGH, B16_LOW, B32_HIGH, B32_LOW, B64_HIGH, B64_LOW,
    MaskSeq, Masks, Packed, Packet,
};
use zerodds_wire_conformance::{Endianness, Reader, WireError, Writer};

/// The bytes one generated `encode` produces, little-endian — the order
/// the C and Go runs above were made in.
fn encoded(f: impl FnOnce(&mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut w = Writer::little_endian();
    f(&mut w).expect("every value here fits its member");
    w.finish()
}

/// A hex string as octets, so a measured line can be quoted verbatim.
fn octets(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "an odd number of hex digits");
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("a hex pair"))
        .collect()
}

/// The first offset two byte strings differ at, named rather than
/// printed as two arrays: a holder one width out is one offset, and a
/// message that prints both vectors does not say which.
fn same_octets(what: &str, ours: &[u8], theirs: &[u8]) {
    if let Some(at) = (0..ours.len().min(theirs.len())).find(|at| ours[*at] != theirs[*at]) {
        panic!(
            "{what}, byte {at}: this column writes {:#04x} where the C and Go columns wrote \
             {:#04x}\n  ours:   {ours:02x?}\n  theirs: {theirs:02x?}",
            ours[at], theirs[at]
        );
    }
    assert_eq!(
        ours.len(),
        theirs.len(),
        "{what}: the columns agree on every shared byte and not on how many there \
         are\n  ours:   {ours:02x?}\n  theirs: {theirs:02x?}"
    );
}

/// The lowest and the highest bit each type declares.
fn masks_sample() -> Masks {
    Masks {
        a: B8_LOW | B8_HIGH,
        b: B16_LOW | B16_HIGH,
        c: B32_LOW | B32_HIGH,
        d: B64_LOW | B64_HIGH,
    }
}

fn mask_seq_sample() -> MaskSeq {
    MaskSeq {
        flags: vec![B16_LOW, B16_HIGH],
        narrow: B12_LOW | B12_HIGH,
        tail: 7,
    }
}

fn packet_sample() -> Packet {
    Packet {
        head: Packed { pa: 5, pb: 21 },
        tail: 7,
    }
}

// -- against the two foreign columns --------------------------------------

#[test]
fn the_bytes_are_the_c_and_go_columns() {
    same_octets(
        "Masks",
        &encoded(|w| masks_sample().encode(w)),
        &octets("81000180010000800100000000000080"),
    );
    same_octets(
        "MaskSeq",
        &encoded(|w| mask_seq_sample().encode(w)),
        &octets("0800000002000000010000800108000007000000"),
    );
    same_octets(
        "Packet",
        &encoded(|w| packet_sample().encode(w)),
        &octets("850a000007000000"),
    );
}

#[test]
fn a_bitmask_takes_the_holder_its_bit_bound_names() {
    // One member at a time, so a disagreement names the width rather
    // than an offset in the combined struct.
    let widths = [
        (
            Masks {
                a: 0x81,
                b: 0,
                c: 0,
                d: 0,
            },
            1usize,
        ),
        (
            Masks {
                a: 0,
                b: 0x8001,
                c: 0,
                d: 0,
            },
            2,
        ),
        (
            Masks {
                a: 0,
                b: 0,
                c: 0x8000_0001,
                d: 0,
            },
            4,
        ),
        (
            Masks {
                a: 0,
                b: 0,
                c: 0,
                d: 0x8000_0000_0000_0001,
            },
            8,
        ),
    ];
    // The struct is four members wide whichever one carries a value, so
    // what the widths show up in is the offsets: 0, 2, 4 and 8.
    let offsets = [0usize, 2, 4, 8];
    for ((sample, width), offset) in widths.into_iter().zip(offsets) {
        let bytes = encoded(|w| sample.encode(w));
        assert_eq!(bytes.len(), 16, "the whole struct is sixteen octets");
        let set: Vec<usize> = (0..16).filter(|at| bytes[*at] != 0).collect();
        assert!(
            set.iter().all(|at| *at >= offset && *at < offset + width),
            "a holder of {width} octets at offset {offset}, and the non-zero octets are {set:?}"
        );
    }
}

#[test]
fn a_bound_that_is_not_a_holder_width_still_takes_the_holder() {
    // `B12` is twelve bits in a two-octet holder. The `long` behind it
    // is aligned to four, which is what makes the holder's width
    // visible: at one octet the pad would be three and at four there
    // would be none.
    let bytes = encoded(|w| mask_seq_sample().encode(w));
    assert_eq!(&bytes[12..14], &[0x01, 0x08], "B12 in two octets");
    assert_eq!(&bytes[14..16], &[0x00, 0x00], "the pad to the long");
    assert_eq!(&bytes[16..], &[0x07, 0x00, 0x00, 0x00], "the long");
}

#[test]
fn a_bitsets_positions_run_through_its_padding() {
    // `bitfield<3> pa; bitfield<4>; bitfield<5> pb;` — `pb` is at
    // position 7 and not at position 3. 5 | (21 << 7) is 0xa85.
    let bytes = encoded(|w| packet_sample().encode(w));
    assert_eq!(&bytes[0..2], &[0x85, 0x0a], "the anchor, little-endian");
}

// -- the refusals ---------------------------------------------------------

#[test]
fn a_bit_the_type_does_not_declare_is_not_part_of_the_value() {
    // §7.4.1.1.3.2: "the states of the remaining serialized bits are not
    // specified, and those bits are not considered to be part of the
    // bitmask", and Table 18: "Preserve bit values by index for all bits
    // identified in both T1 and T2". `B16` declares positions 0 and 15;
    // position 1 is the value 2, a bit this type has no name for, and it
    // reads as not set. The declared bits beside it are kept.
    let bytes = encoded(|w| {
        Masks {
            b: 2 | B16_HIGH,
            ..masks_sample()
        }
        .encode(w)
    });
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Masks::decode(&mut r).expect("an undeclared bit is not a reason to refuse");
    assert_eq!(read.b, B16_HIGH, "only the declared bit is read back");
}

#[test]
fn a_value_above_its_bound_is_refused_rather_than_masked() {
    // `B12`'s holder is a `u16` and holds 0x1000; the bitmask does not.
    // Masking would put a value on the wire nobody wrote and report
    // success.
    let mut w = Writer::little_endian();
    let refused = MaskSeq {
        narrow: 0x1000,
        ..mask_seq_sample()
    }
    .encode(&mut w);
    assert!(
        matches!(refused, Err(WireError::OutOfRange { .. })),
        "a value outside the bound must be refused: {refused:?}"
    );
}

#[test]
fn a_bitfield_value_wider_than_its_field_is_refused_rather_than_masked() {
    // `pa` is three bits in a `u8`, so the holder can carry 8 and the
    // field cannot. Masking would have written `0` into `pa` and the
    // rest of the value into `pb`.
    let mut w = Writer::little_endian();
    let refused = Packet {
        head: Packed { pa: 8, pb: 0 },
        tail: 0,
    }
    .encode(&mut w);
    assert!(
        matches!(refused, Err(WireError::OutOfRange { .. })),
        "a bitfield value outside its width must be refused: {refused:?}"
    );
}

// -- the round trip -------------------------------------------------------

#[test]
fn every_shape_reads_back_as_it_was_written() {
    let masks = masks_sample();
    let bytes = encoded(|w| masks.encode(w));
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(Masks::decode(&mut r).expect("Masks decodes"), masks);

    let seq = mask_seq_sample();
    let bytes = encoded(|w| seq.encode(w));
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(MaskSeq::decode(&mut r).expect("MaskSeq decodes"), seq);

    let packet = packet_sample();
    let bytes = encoded(|w| packet.encode(w));
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(Packet::decode(&mut r).expect("Packet decodes"), packet);
}

#[test]
fn table_9_gives_a_bitmask_no_bit_and_a_bitset_no_field() {
    // A bitmask is its holder integer in this column, whose Table 9
    // default is zero — no bit set. A bitset is the aggregate of its
    // fields, each of which is zero.
    assert_eq!(
        Masks::default(),
        Masks {
            a: 0,
            b: 0,
            c: 0,
            d: 0
        }
    );
    assert_eq!(Packet::default().head, Packed { pa: 0, pb: 0 });
}
