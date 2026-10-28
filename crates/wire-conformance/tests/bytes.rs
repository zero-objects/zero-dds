// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bytes the generated code puts on the wire.
//!
//! Everything here runs generated code. No test asserts on text: the
//! question is not whether the generator wrote something that looks
//! right, it is whether what it wrote encodes the bytes the layout plan
//! said it would.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire_conformance::generated::{
    ArraySeq, Cell, Ext, ExtV2, Grid, Header, Label, MustU, NestedSeq, Newer, Older, Opt, OptV2,
    Point, Reordered, Sensor, SeqArray, TcBare, TcDiscard, TcElemTrim, TcSeqDiscard, TcSeqTrim,
    TcSeqUseDefault, TcTrim, TcUseDefault,
};
use zerodds_wire_conformance::{Endianness, ParameterRole, Reader, WireError, Writer};

/// The bytes one generated `encode` produces.
///
/// `encode` returns a `Result` because `char` and `wchar` refuse a
/// code point they cannot carry. Every caller here writes a value that
/// fits, so the refusal is an `expect` and not an arm — a test that
/// swallowed it would be asserting on the bytes of a partial sample.
fn encoded(f: impl FnOnce(&mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut w = Writer::little_endian();
    f(&mut w).expect("every value here fits its member");
    w.finish()
}

// -- the plan's own numbers ----------------------------------------------

#[test]
fn a_final_struct_of_a_long_and_a_double_encodes_to_sixteen_bytes() {
    // The layout plan says: the long at 0, four bytes of padding, the
    // double at 8, sixteen in total. This is generated code producing
    // exactly that.
    let point = Point { x: 1, y: 2.0 };
    let bytes = encoded(|w| point.encode(w));

    assert_eq!(bytes.len(), 16, "{bytes:?}");
    assert_eq!(&bytes[0..4], &[1, 0, 0, 0], "the long is not at 0");
    assert_eq!(&bytes[4..8], &[0, 0, 0, 0], "the padding is not zeroed");
    assert_eq!(&bytes[8..], &2.0f64.to_le_bytes(), "the double is not at 8");
}

#[test]
fn what_the_generated_encoder_writes_the_generated_decoder_reads() {
    let point = Point { x: -7, y: 1.5 };
    let bytes = encoded(|w| point.encode(w));

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Point::decode(&mut r).expect("decode");
    assert_eq!(read.x, -7);
    assert!((read.y - 1.5).abs() < f64::EPSILON);
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");
}

#[test]
fn an_appendable_struct_writes_a_dheader_with_the_length_that_followed() {
    let header = Header { seq: 9 };
    let bytes = encoded(|w| header.encode(w));

    assert_eq!(bytes.len(), 8, "{bytes:?}");
    assert_eq!(&bytes[0..4], &[4, 0, 0, 0], "the DHEADER is not 4");
    assert_eq!(&bytes[4..8], &[9, 0, 0, 0]);
}

#[test]
fn an_appendable_struct_round_trips_through_its_dheader() {
    let header = Header { seq: 42 };
    let bytes = encoded(|w| header.encode(w));
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(Header::decode(&mut r).expect("decode").seq, 42);
}

// -- the KeyHash ---------------------------------------------------------

#[test]
fn the_key_hash_covers_the_key_member_and_not_the_others() {
    // Two samples that differ only outside the key are one instance.
    let a = Sensor { id: 5, reading: 1 };
    let b = Sensor {
        id: 5,
        reading: 999,
    };
    assert_eq!(a.key_hash(), b.key_hash());

    let c = Sensor { id: 6, reading: 1 };
    assert_ne!(a.key_hash(), c.key_hash(), "two keys gave one handle");
}

#[test]
fn the_key_hash_is_big_endian_whatever_the_sample_is() {
    // A peer that reads the holder little-endian computes a different
    // handle for the same instance. DDSI-RTPS 2.5 §9.6.3.8.
    let sensor = Sensor { id: 1, reading: 0 };
    let hash = sensor.key_hash().expect("a long key fits");
    assert_eq!(hash[..4], [0, 0, 0, 1], "the holder is not big-endian");
    assert_eq!(hash[4..], [0u8; 12], "the padding is not zero");
}

// -- the XCDR1 parameter block -------------------------------------------

/// The generated module's PL_CDR1 header form, named rather than assumed.
///
/// `build.rs` plans every codec under `LayoutKey::data`, which is
/// `Dialect::default`. Three tests below assert byte offsets that the
/// header form decides, and the form moved once already: it was
/// `PlCdr1HeaderPolicy::AlwaysExtended` — twelve bytes a parameter —
/// while `Dialect::Cyclone` was the default. A test that only carried
/// the offsets would fail with a number and no reason.
fn assert_short_pl_cdr1_header() {
    use zerodds_ast_layout::{Dialect, PlCdr1HeaderPolicy};
    assert_eq!(
        Dialect::default().pl_cdr1_headers(),
        PlCdr1HeaderPolicy::Specification,
        "this module is generated under `LayoutKey::data`, whose dialect now writes a \
         parameter header of another form; the offsets below are §7.4.1.2.2's short header \
         and have to be re-derived, not nudged"
    );
    assert_eq!(
        Dialect::Cyclone.pl_cdr1_headers(),
        PlCdr1HeaderPolicy::AlwaysExtended,
        "`--dialect cyclone` stopped writing the extended header, which is the form the \
         measurement these tests were written from records"
    );
}

#[test]
fn a_parameter_value_is_its_own_alignment_origin_and_the_origin_is_put_back() {
    // XTypes 1.3 §7.4.1.2: each parameter is "a CDR-encapsulated block
    // of data", so `flag` counts alignment from behind its own header —
    // and everything after the block counts from the payload again.
    //
    // The offsets below are the **short** header's, which is
    // §7.4.1.2.2's form wherever the id and the length fit it. That is
    // `PlCdr1HeaderPolicy::Specification` and it is what the module was
    // generated under, because the build script plans in
    // `LayoutKey::data` and the default dialect is
    // `Dialect::Specification`. The dialect is asserted here rather
    // than the offsets taken on trust: these were the extended header's
    // — 12 bytes, block at 12, `after` at 16, 24 in total — while
    // `Dialect::Cyclone` was the default, and a bare offset mismatch
    // does not say why.
    //
    // The origin is the property under test either way. A decoder that
    // left the origin behind the header aligned `after` past where the
    // encoder wrote it, and the members of the difference came out of
    // the wrong bytes.
    assert_short_pl_cdr1_header();
    let opt = Opt {
        flag: Some(0xAB),
        after: 1.5,
    };
    let bytes = encoded(|w| opt.encode(w));

    assert_eq!(bytes.len(), 16, "{bytes:?}");
    assert_eq!(bytes[4], 0xAB, "the octet is not behind its own header");
    assert_eq!(
        &bytes[8..16],
        &1.5f64.to_le_bytes(),
        "the double is not at 8"
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Opt::decode(&mut r).expect("decode");
    assert_eq!(read.flag, Some(0xAB));
    assert_eq!(read.after.to_bits(), 1.5f64.to_bits());
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");
}

#[test]
fn an_absent_optional_member_round_trips_as_the_absence_the_vendors_write() {
    // Rule (19) and §7.4.1.2: the parameter header is written either
    // way and a declared length of zero is the absence. The measurement
    // this was written from is Cyclone's, `opt_final_absent` XCDR1,
    // 2026-08-09 on the Linux test host: `7929edff 017f0800 01000000
    // 00000000 feffffff` for `@final struct Sample { long a; @optional
    // octet b; long c; }`, the **extended** header with a declared
    // length of zero and no value behind it. `--dialect cyclone` still
    // writes that, and `crates/wire-probe/tests/vendor_bytes.rs` holds
    // it; this module is generated in `Dialect::default`, which takes
    // §7.4.1.2.2's short header, so the offsets here are four bytes of
    // header and not twelve.
    //
    // The defect the test exists for is neither: this column wrote a
    // value behind the header whatever the sample said, because its
    // field could not hold an absence.
    assert_short_pl_cdr1_header();
    let opt = Opt {
        flag: None,
        after: 1.5,
    };
    let bytes = encoded(|w| opt.encode(w));

    // Four bytes of short header whose declared length is zero, then
    // padding to the double's eight-byte offset. The present form of the
    // same type declares one and puts `0xAB` at 4; here the length word
    // is the only thing that says the member is not there.
    assert_eq!(bytes.len(), 16, "{bytes:?}");
    assert_eq!(
        &bytes[2..4],
        &0u16.to_le_bytes(),
        "the declared length is not zero: {bytes:?}"
    );
    assert_eq!(
        &bytes[4..8],
        &[0u8; 4],
        "a value was written behind a header that declares none: {bytes:?}"
    );
    assert_eq!(
        &bytes[8..16],
        &1.5f64.to_le_bytes(),
        "the double is not at 8"
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Opt::decode(&mut r).expect("decode");
    assert_eq!(read.flag, None, "the absence came back as a value");
    assert_eq!(read.after.to_bits(), 1.5f64.to_bits());
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");
}

#[test]
fn an_optional_member_under_xcdr2_is_the_marker_byte_the_three_vendors_write() {
    // Rule (20), §7.4.3.5.2. `opt_final_present` and `opt_final_absent`,
    // measured 2026-08-09 through `byte-gate/gate`: Cyclone, RTI
    // and Fast DDS all three write `7929edff 01 ab 0000 feffffff` for
    // the present form and `7929edff 00000000 feffffff` for the absent
    // one. Twelve bytes either way, with the value at offset 5 and the
    // trailing `long` at 8.
    let present = OptV2 {
        a: -1_234_567,
        b: Some(0xAB),
        c: -2,
    };
    assert_eq!(
        encoded(|w| present.encode(w)),
        vec![
            0x79, 0x29, 0xed, 0xff, 0x01, 0xab, 0x00, 0x00, 0xfe, 0xff, 0xff, 0xff
        ],
    );

    let absent = OptV2 {
        a: -1_234_567,
        b: None,
        c: -2,
    };
    let bytes = encoded(|w| absent.encode(w));
    assert_eq!(
        bytes,
        vec![
            0x79, 0x29, 0xed, 0xff, 0x00, 0x00, 0x00, 0x00, 0xfe, 0xff, 0xff, 0xff
        ],
    );

    // And read back. The absence is the one the writers describe, not a
    // refusal and not a zero octet: `b` was never sent and `c` is still
    // where the stream put it.
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = OptV2::decode(&mut r).expect("decode");
    assert_eq!(read.a, -1_234_567);
    assert_eq!(read.b, None, "the absence came back as a value");
    assert_eq!(read.c, -2, "the trailing member moved");
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");
}

#[test]
fn a_mutable_xcdr1_struct_round_trips_through_its_sentinel() {
    // Rule (23). Every parameter's block is closed and the list ends at
    // `PID_SENTINEL`; a decoder that stopped at the last member it knew
    // left the sentinel — and, before it, the padding behind the last
    // value — in the stream for whatever reads next.
    //
    // The offsets are the short header's, for the reason
    // `a_parameter_value_is_its_own_alignment_origin_and_the_origin_is_put_back`
    // gives: this module is generated in `Dialect::default`. They were
    // 12, 32 and 40 under the extended header.
    assert_short_pl_cdr1_header();
    let ext = Ext { d: 2.5, c: -3 };
    let bytes = encoded(|w| ext.encode(w));

    assert_eq!(bytes.len(), 24, "{bytes:?}");
    assert_eq!(&bytes[4..12], &2.5f64.to_le_bytes(), "`d` is not at 4");
    assert_eq!(&bytes[16..18], &(-3i16).to_le_bytes(), "`c` is not at 16");

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Ext::decode(&mut r).expect("decode");
    assert_eq!(read.d.to_bits(), 2.5f64.to_bits());
    assert_eq!(read.c, -3);
    assert_eq!(r.remaining(), 0, "the sentinel was left in the stream");
}

#[test]
fn a_parameter_list_that_does_not_end_at_its_sentinel_is_refused() {
    // The last four bytes are the sentinel. Replacing it with another
    // parameter header is a list this reader has not reached the end
    // of, and continuing would take the next sample's bytes for this
    // one's.
    let ext = Ext { d: 1.0, c: 7 };
    let mut bytes = encoded(|w| ext.encode(w));
    let end = bytes.len();
    bytes[end - 4..].copy_from_slice(&[0x09, 0x00, 0x04, 0x00]);

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert!(
        Ext::decode(&mut r).is_err(),
        "a parameter list without its sentinel decoded"
    );
}

// -- a `@mutable` list is read by its ids ---------------------------------

/// A PL_CDR2 parameter list holding whichever members are asked for, in
/// whichever order.
///
/// Built against the runtime rather than by the generated encoder: the
/// point of the tests below is a stream whose order or member set the
/// generated encoder would never produce, which is exactly what a peer
/// with a different declaration order sends.
fn pl_cdr2(members: &[(u32, u8, bool, &[u8])]) -> Vec<u8> {
    let mut w = Writer::little_endian();
    let dheader = w.write_dheader();
    for (id, length_code, must_understand, value) in members {
        let header = w
            .write_emheader(*id, *length_code, *must_understand)
            .expect("a header the plan chose");
        for byte in *value {
            w.write_u8(*byte);
        }
        w.finish_emheader(header)
            .expect("the member is the length its code declares");
    }
    w.finish_dheader(dheader);
    w.finish()
}

#[test]
fn a_mutable_member_list_is_read_by_its_ids_and_not_by_its_order() {
    // XTypes 1.3 §7.4.3.4.2 puts a member id on every EMHEADER, and
    // §7.4.1.2.3 leaves the list unordered. Two peers whose IDL declares
    // the same members in a different order write the same list in a
    // different order, and both are the same type.
    //
    // Read in plan order this stream gave `a` the two bytes of `b` and
    // then failed on the extent — or, where the widths happened to
    // match, returned each member's value under the other's name with
    // no error at all.
    let reordered = pl_cdr2(&[
        (3, 0, false, &[0xAB]),
        (2, 1, false, &(-3i16).to_le_bytes()),
        (1, 2, false, &7i32.to_le_bytes()),
    ]);

    let mut r = Reader::new(&reordered, Endianness::Little);
    let read = ExtV2::decode(&mut r).expect("decode");
    assert_eq!(read.a, 7, "`a` did not come from the header that named it");
    assert_eq!(read.b, -3);
    assert_eq!(read.c, Some(0xAB));
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");

    // The declaration order produces the same sample, which is what
    // "unordered" has to mean.
    let declared = pl_cdr2(&[
        (1, 2, false, &7i32.to_le_bytes()),
        (2, 1, false, &(-3i16).to_le_bytes()),
        (3, 0, false, &[0xAB]),
    ]);
    let mut r = Reader::new(&declared, Endianness::Little);
    let same = ExtV2::decode(&mut r).expect("decode");
    assert_eq!((same.a, same.b, same.c), (read.a, read.b, read.c));
}

#[test]
fn a_mutable_member_the_stream_omits_takes_its_default() {
    // §7.4.1.2.3: "any mutable structure member's value may be omitted.
    // ... if the receiver's member is not optional, it logically takes
    // its default value." Measured 2026-08-09 as `evo-mutable-tail` and
    // `evo-mutable-middle`: Cyclone, RTI and Fast DDS all three hand
    // back the default, in both encoding versions.
    let only_b = pl_cdr2(&[(2, 1, false, &(-3i16).to_le_bytes())]);

    let mut r = Reader::new(&only_b, Endianness::Little);
    let read = ExtV2::decode(&mut r).expect("decode");
    assert_eq!(read.a, 0, "Table 9 gives an omitted `long` zero");
    assert_eq!(read.b, -3);
    // "If the member is optional, it takes no value at all." Not Table
    // 9's zero: the writer left it out on purpose.
    assert_eq!(read.c, None);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn an_unknown_mutable_member_is_skipped_and_a_must_understand_one_is_not() {
    // §7.2.2.4.4.4.6. Id 9 is not declared by `ExtV2`; the writer that
    // sent it said whether this reader may go on without it.
    let skippable = pl_cdr2(&[
        (1, 2, false, &7i32.to_le_bytes()),
        (9, 3, false, &1.5f64.to_le_bytes()),
        (2, 1, false, &(-3i16).to_le_bytes()),
    ]);
    let mut r = Reader::new(&skippable, Endianness::Little);
    let read = ExtV2::decode(&mut r).expect("an unknown skippable member is skipped");
    assert_eq!((read.a, read.b, read.c), (7, -3, None));
    assert_eq!(r.remaining(), 0, "the skip landed somewhere else");

    let must_understand = pl_cdr2(&[
        (1, 2, false, &7i32.to_le_bytes()),
        (9, 3, true, &1.5f64.to_le_bytes()),
        (2, 1, false, &(-3i16).to_le_bytes()),
    ]);
    let mut r = Reader::new(&must_understand, Endianness::Little);
    assert!(
        ExtV2::decode(&mut r).is_err(),
        "a must-understand member this type does not declare was skipped"
    );
}

#[test]
fn one_member_id_may_not_arrive_twice() {
    // §7.4.3.4.2 gives each member one header. A stream with two for
    // one id says two things about one member, and taking the last
    // would make the sample depend on which order they arrived in.
    let twice = pl_cdr2(&[
        (1, 2, false, &7i32.to_le_bytes()),
        (1, 2, false, &9i32.to_le_bytes()),
        (2, 1, false, &(-3i16).to_le_bytes()),
    ]);
    let mut r = Reader::new(&twice, Endianness::Little);
    assert!(
        ExtV2::decode(&mut r).is_err(),
        "one member id arrived twice and the second one won in silence"
    );
}

#[test]
fn a_stream_three_vendors_wrote_reads_in_this_column_too() {
    // `mut_mustunderstand`, re-run today through byte-gate/gate:
    // `@mutable struct M { @id(1) @must_understand long a; @id(2) long b; }`
    // with `a = 7, b = -2`. Cyclone and RTI both write
    // `10000000 010000a0 07000000 02000020 feffffff` under XCDR2 and
    // Fast DDS the same with the must-understand bit clear (I11).
    //
    // Nothing had ever fed the generated Rust decoder a foreign stream.
    // It read its own plan's member order and would have taken these
    // bytes only because the writer happened to use it.
    let cyclone = [
        0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0xa0, 0x07, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
        0x20, 0xfe, 0xff, 0xff, 0xff,
    ];
    let mut r = Reader::new(&cyclone, Endianness::Little);
    let read = MustU::decode(&mut r).expect("decode Cyclone's bytes");
    assert_eq!((read.a, read.b), (7, -2));
    assert_eq!(r.remaining(), 0);

    let fastdds = [
        0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
        0x20, 0xfe, 0xff, 0xff, 0xff,
    ];
    let mut r = Reader::new(&fastdds, Endianness::Little);
    let read = MustU::decode(&mut r).expect("decode Fast DDS's bytes");
    assert_eq!((read.a, read.b), (7, -2));
    assert_eq!(r.remaining(), 0);

    // The same two parameters, in the order a peer that declared `b`
    // first would write them. A vendor front end *will* produce that —
    // see `a_reordered_list_a_vendor_actually_wrote_reads_here_too`.
    let reordered = [
        0x10, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x20, 0xfe, 0xff, 0xff, 0xff, 0x01, 0x00, 0x00,
        0xa0, 0x07, 0x00, 0x00, 0x00,
    ];
    let mut r = Reader::new(&reordered, Endianness::Little);
    let read = MustU::decode(&mut r).expect("decode the reordered list");
    assert_eq!(
        (read.a, read.b),
        (7, -2),
        "the members came back under each other's names"
    );
    assert_eq!(r.remaining(), 0);
}

#[test]
fn a_reordered_list_a_vendor_actually_wrote_reads_here_too() {
    // M1. §7.4.1.2.3 leaves a `@mutable` list unordered and §7.4.3.4.2
    // puts a member id on every EMHEADER, which is what the ids are
    // for. This was recorded as unproducible — "both models in a
    // two-IDL case order their members the same way" — and they do only
    // while the ids are implicit, because then the order *is* the
    // numbering. Write the ids out, declare them in another order, and
    // every generator writes members in declaration order.
    //
    // `@mutable struct S { @id(3) long c; @id(1) long a; @id(2) short b; }`
    // with `a = 7, b = -3, c = 9`, run 2026-08-10 through
    // `tc-gate`: Cyclone and RTI both write
    //
    //   16000000 03000020 09000000 01000020 07000000 02000010 fdff
    //
    // and Cyclone, RTI, Fast DDS and OpenDDS all four read it back as
    // `a=7 b=-3 c=9` against the declaration-order type. This is that
    // byte string, against the type that declares 1, 2, 3.
    let vendor = [
        0x16, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x20, 0x09, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x20, 0x07, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x10, 0xfd, 0xff,
    ];
    let mut r = Reader::new(&vendor, Endianness::Little);
    let read = Reordered::decode(&mut r).expect("decode the vendors' reordered list");
    assert_eq!(read.a, 7, "`a` did not come from the header that named it");
    assert_eq!(read.b, -3);
    assert_eq!(read.c, 9);
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");

    // And this column writes the declaration-order form Cyclone and RTI
    // write for the same sample, which is the other half of the claim:
    // the order is the declaration's and the reader does not care.
    let same = Reordered { a: 7, b: -3, c: 9 };
    assert_eq!(
        encoded(|w| same.encode(w)),
        vec![
            0x18, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x20, 0x07, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x10, 0xfd, 0xff, 0x00, 0x00, 0x03, 0x00, 0x00, 0x20, 0x09, 0x00, 0x00, 0x00,
        ],
    );
}

#[test]
fn an_xcdr1_parameter_list_is_read_by_its_ids_too() {
    // Rule (23), the same question one encoding version down. The list
    // ends at `PID_SENTINEL` and the order in between is the writer's.
    let mut w = Writer::little_endian();
    let pid = w
        .write_pid_extended(2, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_i16(-3);
    w.finish_pid(pid);
    let pid = w
        .write_pid_extended(1, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_f64(2.5);
    w.finish_pid(pid);
    w.write_sentinel();
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Ext::decode(&mut r).expect("decode");
    assert_eq!(read.d.to_bits(), 2.5f64.to_bits());
    assert_eq!(read.c, -3);
    assert_eq!(r.remaining(), 0, "the sentinel was left in the stream");

    // And a list that carries only the second member defaults the first.
    let mut w = Writer::little_endian();
    let pid = w
        .write_pid_extended(2, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_i16(11);
    w.finish_pid(pid);
    w.write_sentinel();
    let short = w.finish();

    let mut r = Reader::new(&short, Endianness::Little);
    let read = Ext::decode(&mut r).expect("decode");
    assert_eq!(
        read.d.to_bits(),
        0.0f64.to_bits(),
        "`d` is not Table 9's 0.0"
    );
    assert_eq!(read.c, 11);
}

// -- a collection directly inside a collection ----------------------------

#[test]
fn a_sequence_of_sequences_writes_the_bytes_cyclone_and_fast_dds_write() {
    // `docs/refused-constructs.md`, "A collection directly inside a
    // collection". `@final struct Sample { sequence<sequence<long> >
    // nested; octet tail; }` holding `[[1,2],[3]]` with `tail = 0xAB`
    // is, little endian under XCDR2:
    //
    //   18000000 02000000 02000000 01000000 02000000 01000000 03000000 ab
    //
    // from Cyclone, from Fast DDS and from this project's reflective
    // probe alike. RTI's generator refuses the anonymous inner type and
    // names the workaround, which is `ArraySeq` below.
    //
    // The rule the bytes agree on: the inner collection carries no
    // delimiter of its own. Rule (11), `PSEQUENCE_TYPE` — a sequence of
    // a primitive — writes none, and `sequence<long>` is one wherever
    // it sits.
    let sample = NestedSeq {
        rows: vec![vec![1, 2], vec![3]],
        tail: 0xAB,
    };
    let bytes = encoded(|w| sample.encode(w));
    assert_eq!(
        bytes,
        vec![
            0x18, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
            0xAB,
        ],
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = NestedSeq::decode(&mut r).expect("decode");
    assert_eq!(read.rows, vec![vec![1, 2], vec![3]]);
    assert_eq!(read.tail, 0xAB);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn an_array_of_sequences_writes_the_bytes_cyclone_and_fast_dds_write() {
    // `sequence<long> rows[2]` holding `[[1,2],[3]]`:
    //
    //   14000000 02000000 01000000 02000000 01000000 03000000 ab
    //
    // Cyclone and Fast DDS; RTI writes the same without the DHEADER,
    // which is `CollectionDelimiterPolicy` and not this question.
    let sample = SeqArray {
        rows: [vec![1, 2], vec![3]],
        tail: 0xAB,
    };
    let bytes = encoded(|w| sample.encode(w));
    assert_eq!(
        bytes,
        vec![
            0x14, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0xAB,
        ],
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = SeqArray::decode(&mut r).expect("decode");
    assert_eq!(read.rows, [vec![1, 2], vec![3]]);
    assert_eq!(read.tail, 0xAB);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn a_sequence_of_arrays_writes_the_bytes_cyclone_and_fast_dds_write() {
    // `typedef long Row[3]; sequence<Row> rows;` holding
    // `[[1,2,3],[4,5,6]]`:
    //
    //   1c000000 02000000 01000000 02000000 03000000 04000000 05000000 06000000 ab
    //
    // This is the shape RTI's generator names as the workaround for the
    // anonymous inner sequence, and it takes it without complaint.
    let sample = ArraySeq {
        rows: vec![[1, 2, 3], [4, 5, 6]],
        tail: 0xAB,
    };
    let bytes = encoded(|w| sample.encode(w));
    assert_eq!(
        bytes,
        vec![
            0x1C, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00, 0xAB,
        ],
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = ArraySeq::decode(&mut r).expect("decode");
    assert_eq!(read.rows, vec![[1, 2, 3], [4, 5, 6]]);
    assert_eq!(read.tail, 0xAB);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn a_nested_aggregate_and_a_bounded_string_compile_and_round_trip() {
    // The five rows of `docs/codec-runtime-gap-followup.md`'s table in
    // one type. Every one of them emitted a call the runtime did not
    // have, and `crates/lang-rules/tests/codec.rs` compared the text and
    // passed.
    let grid = Grid {
        cells: vec![Cell { a: 1, b: 2 }],
        pair: [Cell { a: 3, b: 4 }, Cell { a: 5, b: 6 }],
        ids: vec![7, 8],
        label: String::from("eight!!!"),
    };
    let bytes = encoded(|w| grid.encode(w));

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Grid::decode(&mut r).expect("decode");
    assert_eq!(read.cells.len(), 1);
    assert_eq!((read.cells[0].a, read.cells[0].b), (1, 2));
    assert_eq!((read.pair[1].a, read.pair[1].b), (5, 6));
    assert_eq!(read.ids, vec![7, 8]);
    assert_eq!(read.label, "eight!!!");
    assert_eq!(r.remaining(), 0);
}

#[test]
fn a_value_longer_than_its_string_bound_is_refused_and_not_truncated() {
    // AL-14. `Writer::write_string_bounded` cut the value at a character
    // boundary and said nothing; a peer then received a name nobody
    // wrote. §7.2.2.7's TRIM is the *receiver's* answer to a value that
    // does not fit, and this is the sender.
    let grid = Grid {
        cells: Vec::new(),
        pair: [Cell { a: 0, b: 0 }, Cell { a: 0, b: 0 }],
        ids: Vec::new(),
        label: String::from("nine chars"),
    };
    let mut w = Writer::little_endian();
    assert!(
        grid.encode(&mut w).is_err(),
        "a ten-octet value went into a string<8> and nothing was said"
    );
}

// -- `try_construct` ------------------------------------------------------

/// Hex to bytes, for a payload another implementation produced.
fn payload(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
        .collect()
}

/// `@mutable struct S { long a; string<32> m; long b; }` with
/// `a = 7`, a twelve-character `m` and `b = 9`, written by Cyclone under
/// XCDR2. Re-run 2026-08-10 through `tc-gate`.
const WIDE_STRING: &str = "280000000000002007000000010000500d00000048656c6c6f20576f726c64\
                           2100000000\
                           0200002009000000";

/// The same shape with `sequence<long, 32>` holding `[1..8]`.
const WIDE_SEQUENCE: &str = "3800000000000020070000000100006008000000\
                             0100000002000000030000000400000005000000060000000700000008000000\
                             0200002009000000";

#[test]
fn a_value_the_receiving_type_cannot_hold_discards_the_sample_by_default() {
    // §7.2.2.7: DISCARD is what an *absent* annotation means, and it is
    // the answer of four of the five implementations whatever the
    // annotation says — Cyclone, RTI 7.7, OpenDDS and omniORB. This
    // column used to ignore the bound on the read side entirely and
    // hand back all twelve characters in a `string<5>`, which is a
    // value of a type nobody declared.
    let bytes = payload(WIDE_STRING);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let Err(error) = TcDiscard::decode(&mut r) else {
        panic!("a string<5> took twelve characters");
    };

    // And it is an absence, not a malformed stream. §7.2.4.1 against
    // §7.2.3: the sample is dropped and the next one may well fit,
    // where every other variant says the *stream* is wrong. Cyclone
    // traces the same distinction through `ddsi_serdata_from_ser`
    // returning NULL.
    assert!(error.is_discarded(), "{error:?}");

    let bytes = payload(WIDE_SEQUENCE);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let Err(error) = TcSeqDiscard::decode(&mut r) else {
        panic!("a sequence<long,4> took eight elements");
    };
    assert!(error.is_discarded(), "{error:?}");
}

#[test]
fn use_default_substitutes_the_default_and_the_sample_survives() {
    // OpenDDS is the only implementation of USE_DEFAULT and it answers
    // `a=7 m="" b=9` and `a=7 n=0 [] b=9`. The members either side come
    // back correct, which is the property that matters: the whole value
    // is read off the wire before the bound is applied, so nothing
    // after it moves.
    let bytes = payload(WIDE_STRING);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = TcUseDefault::decode(&mut r).expect("the sample survives");
    assert_eq!((read.a, read.m.as_str(), read.b), (7, "", 9));

    let bytes = payload(WIDE_SEQUENCE);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = TcSeqUseDefault::decode(&mut r).expect("the sample survives");
    assert_eq!(read.a, 7);
    assert!(read.m.is_empty(), "{:?}", read.m);
    assert_eq!(read.b, 9);
}

#[test]
fn trim_cuts_the_value_to_what_the_receiver_holds() {
    // §7.3.1.2.1.12.2 and §7.3.1.2.1.12.3, and OpenDDS's own answers:
    // `"Hello"` and `[1 2 3 4]`. The order is preserved and the prefix
    // is taken.
    let bytes = payload(WIDE_STRING);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = TcTrim::decode(&mut r).expect("the sample survives");
    assert_eq!((read.a, read.m.as_str(), read.b), (7, "Hello", 9));

    let bytes = payload(WIDE_SEQUENCE);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = TcSeqTrim::decode(&mut r).expect("the sample survives");
    assert_eq!((read.a, read.b), (7, 9));
    assert_eq!(read.m, vec![1, 2, 3, 4]);
}

#[test]
fn a_bare_annotation_is_use_default_and_not_an_absent_one() {
    // §7.2.2.7 makes an *absent* annotation DISCARD and §7.3.1.2.1.12
    // declares the annotation's own parameter `default USE_DEFAULT`, so
    // a valueless one is USE_DEFAULT. Reading the second as the first
    // would drop samples the author asked to keep. OpenDDS constructs
    // `""` for it, which is the only implementation that demonstrates
    // the difference.
    let bytes = payload(WIDE_STRING);
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = TcBare::decode(&mut r).expect("a bare annotation keeps the sample");
    assert_eq!((read.a, read.m.as_str(), read.b), (7, "", 9));
}

#[test]
fn an_element_carries_its_own_behaviour_and_the_collection_carries_its_own() {
    // §7.3.1.2.1.12.4. `sequence<@try_construct(TRIM) String5, 10>`
    // against a writer's `sequence<String16, 10>` whose one element is
    // "Hello World". OpenDDS answers `a=7 n=1 [[Hello] ] b=9`; the
    // payload below is Cyclone's, re-run 2026-08-10.
    //
    // The two bounds are separate failures: the element trims and the
    // sequence's own length — one, against a bound of ten — never
    // overflows. Table 17 draws the split and OpenDDS's generated
    // element loop implements it.
    let bytes = payload("200000000700000014000000010000000c00000048656c6c6f20576f726c640009000000");
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = TcElemTrim::decode(&mut r).expect("the sample survives");
    assert_eq!((read.a, read.b), (7, 9));
    assert_eq!(read.m, vec![String::from("Hello")]);
}

// -- the one write that can refuse ---------------------------------------

#[test]
fn a_char_and_a_wchar_encode_to_one_byte_and_two() {
    let label = Label {
        tag: 'Z',
        wide: '\u{20AC}',
    };
    let bytes = encoded(|w| label.encode(w));
    // The `char` at 0, one byte of padding, the `wchar` aligned to 2.
    assert_eq!(bytes, [0x5A, 0x00, 0xAC, 0x20], "{bytes:?}");

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = Label::decode(&mut r).expect("decode");
    assert_eq!(read.tag, 'Z');
    assert_eq!(read.wide, '\u{20AC}');
}

#[test]
fn a_char_above_u00ff_is_refused_by_the_generated_encoder() {
    // The generated field is a Rust `char`, so the value reaches the
    // runtime whole. It used to be narrowed there: U+0161 went out as
    // 0x61 and read back as `a`.
    let label = Label {
        tag: '\u{161}',
        wide: 'x',
    };
    let mut w = Writer::little_endian();
    match label.encode(&mut w) {
        Err(WireError::OutOfRange { expected, .. }) => {
            assert!(expected.contains("U+0161"), "{expected}");
        }
        other => panic!("a two-byte code point went into one byte: {other:?}"),
    }
    assert!(w.finish().is_empty(), "a narrowed byte was written anyway");
}

#[test]
fn a_wchar_above_the_bmp_is_refused_by_the_generated_encoder() {
    let label = Label {
        tag: 'x',
        wide: '\u{1F600}',
    };
    let mut w = Writer::little_endian();
    assert!(matches!(
        label.encode(&mut w),
        Err(WireError::OutOfRange { .. })
    ));
}

#[test]
fn the_generated_key_hash_refuses_a_key_it_cannot_carry() {
    // A `@key char` puts the same question to the KeyHash, and a hash
    // over a narrowed character is a handle for an instance nobody has.
    let ok = Label {
        tag: 'k',
        wide: 'x',
    };
    assert_eq!(
        ok.key_hash().expect("`k` is one byte")[..2],
        [b'k', 0],
        "the holder is not the key member zero-padded"
    );

    let bad = Label {
        tag: '\u{100}',
        wide: 'x',
    };
    assert!(
        bad.key_hash().is_err(),
        "a KeyHash was computed over a character that was never written"
    );
}

// -- what a decoder must refuse ------------------------------------------

#[test]
fn a_truncated_sample_is_an_error_rather_than_a_default() {
    // A decoder that filled in a zero would hand the application a
    // value that was never sent.
    let mut r = Reader::new(&[1, 0, 0, 0], Endianness::Little);
    assert!(Point::decode(&mut r).is_err(), "a short sample decoded");
}

#[test]
fn an_empty_sample_is_an_error() {
    let mut r = Reader::new(&[], Endianness::Little);
    assert!(Point::decode(&mut r).is_err());
}

// -- the property the wire actually needs --------------------------------

#[test]
fn every_value_survives_the_round_trip_unchanged() {
    for x in [i32::MIN, -1, 0, 1, i32::MAX] {
        for y in [f64::MIN, -0.5, 0.0, 0.5, f64::MAX] {
            let point = Point { x, y };
            let bytes = encoded(|w| point.encode(w));
            let mut r = Reader::new(&bytes, Endianness::Little);
            let read = Point::decode(&mut r).expect("decode");
            assert_eq!(read.x, x);
            assert_eq!(read.y.to_bits(), y.to_bits(), "{y} changed");
        }
    }
}

#[test]
fn the_same_sample_encodes_to_the_same_bytes_every_time() {
    // Nothing in the encoder depends on anything but the value: no
    // ordering by hash, no timestamps, no padding left uninitialised.
    let point = Point { x: 3, y: 4.0 };
    let first = encoded(|w| point.encode(w));
    let second = encoded(|w| point.encode(w));
    assert_eq!(first, second);
}

#[test]
fn an_appendable_reader_defaults_the_tail_its_writer_did_not_have() {
    // The generated Rust decoder against a real stream, not against its
    // own generator. `Older` writes two members; `Newer` has four, and
    // §7.2.4.4.8 gives the two it did not receive their Table 9 defaults
    // — 0 for a `short` and `""` for a `string`, not whatever the
    // language calls a zero.
    //
    // Cyclone, RTI and Fast DDS all produce the same value from the same
    // bytes; measured 2026-08-09 as `evo-appendable-tail`.
    let bytes = encoded(|w| Older { a: 7, b: 1.5 }.encode(w));

    let mut r = Reader::new(&bytes, Endianness::Little);
    let grown = Newer::decode(&mut r).expect("the newer type reads the older one's bytes");
    assert_eq!(grown.a, 7);
    assert!((grown.b - 1.5).abs() < f64::EPSILON);
    assert_eq!(grown.c, 0);
    assert_eq!(grown.d, "");

    // And the other way: the older type reads the newer one's bytes and
    // steps over the tail by the delimiter. M7.
    let bytes = encoded(|w| {
        Newer {
            a: 7,
            b: 1.5,
            c: -3,
            d: String::from("tail"),
        }
        .encode(w)
    });
    let mut r = Reader::new(&bytes, Endianness::Little);
    let shrunk = Older::decode(&mut r).expect("the older type reads the newer one's bytes");
    assert_eq!(shrunk.a, 7);
    assert!((shrunk.b - 1.5).abs() < f64::EPSILON);
}
