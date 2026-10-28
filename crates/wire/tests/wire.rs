// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bytes, against what the specifications say they are.
//!
//! Every assertion is on the bytes themselves. A test on a round trip
//! alone passes when the writer and the reader are wrong in the same
//! way, which is the failure mode a codec has.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire::{
    Endianness, FixedDigits, ParameterRole, PidFlagPolicy, PlCdr1Framing, PlCdr1Reading, Reader,
    WireError, Writer,
};

#[test]
fn a_long_after_a_char_sits_at_four_not_at_one() {
    // XCDR aligns each primitive to its own size counted from the
    // encapsulation. This is the rule a hand-written encoder gets wrong.
    let mut w = Writer::little_endian();
    w.write_char('a').expect("'a' is one byte");
    w.write_i32(1);
    assert_eq!(w.as_bytes(), &[0x61, 0, 0, 0, 1, 0, 0, 0]);
}

#[test]
fn a_double_aligns_to_eight() {
    let mut w = Writer::little_endian();
    w.write_u8(0xFF);
    w.write_f64(1.0);
    assert_eq!(w.as_bytes().len(), 16, "{:?}", w.as_bytes());
    assert_eq!(&w.as_bytes()[..8], &[0xFF, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn a_reader_skips_exactly_the_padding_the_writer_wrote() {
    let mut w = Writer::little_endian();
    w.write_char('a').expect("'a' is one byte");
    w.write_i32(-1);
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_char().expect("char"), 'a');
    assert_eq!(r.read_i32().expect("long"), -1);
    assert_eq!(r.remaining(), 0, "padding was left over");
}

#[test]
fn the_two_byte_orders_write_the_same_value_differently() {
    let mut little = Writer::little_endian();
    little.write_u32(0x0102_0304);
    assert_eq!(little.as_bytes(), &[0x04, 0x03, 0x02, 0x01]);

    let mut big = Writer::big_endian();
    big.write_u32(0x0102_0304);
    assert_eq!(big.as_bytes(), &[0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn a_string_length_counts_its_terminator() {
    // The one place XCDR differs from every other length-prefixed
    // encoding: "ab" is length 3, not 2.
    let mut w = Writer::little_endian();
    w.write_string("ab");
    assert_eq!(w.as_bytes(), &[3, 0, 0, 0, b'a', b'b', 0]);
}

#[test]
fn an_empty_string_is_a_length_of_one_and_a_nul() {
    let mut w = Writer::little_endian();
    w.write_string("");
    assert_eq!(w.as_bytes(), &[1, 0, 0, 0, 0]);
}

#[test]
fn a_string_round_trips_with_its_terminator_consumed() {
    let mut w = Writer::little_endian();
    w.write_string("hello");
    w.write_u8(0xAB);
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_string().expect("string"), "hello");
    assert_eq!(
        r.read_u8().expect("marker"),
        0xAB,
        "the NUL was not consumed"
    );
}

#[test]
fn a_value_longer_than_its_string_bound_is_refused_and_not_truncated() {
    // AL-14. This cut the value at a character boundary and reported
    // nothing, which put a name on the wire nobody wrote. §7.2.2.7's
    // TRIM is the *receiver's* answer to a value that does not fit;
    // this is the sender, and a sender has nothing to trim towards.
    //
    // The bound counts octets, which is what `string<N>` means to every
    // peer that has to hold the value. "äöü" is six of them.
    let mut w = Writer::little_endian();
    assert!(
        w.write_string_bounded("äöü", 3).is_err(),
        "six octets went into a string<3> and nothing was said"
    );

    let mut w = Writer::little_endian();
    w.write_string_bounded("äöü", 6)
        .expect("six octets fit six");
    let bytes = w.finish();
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_string().expect("the value"), "äöü");
}

#[test]
fn a_dheader_holds_the_length_of_what_followed_it() {
    let mut w = Writer::little_endian();
    let header = w.write_dheader();
    w.write_i32(7);
    w.write_i32(8);
    w.finish_dheader(header);

    let bytes = w.finish();
    assert_eq!(&bytes[..4], &[8, 0, 0, 0], "the DHEADER is not 8");

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_dheader().expect("dheader"), 8);
}

/// A writer appending to a buffer that already holds octets writes what
/// a fresh writer's bytes appended there are: alignment counts from the
/// buffer's end, and the DHEADER is patched where it was written.
#[test]
fn an_appending_writer_writes_what_a_fresh_one_appended_would() {
    fn body(w: &mut Writer) {
        let header = w.write_dheader();
        w.write_u8(1);
        w.write_u64(0x0102_0304_0506_0708);
        w.write_string("ab");
        w.write_u16(9);
        w.finish_dheader(header);
    }
    for (endianness, max_align) in [(Endianness::Little, 4), (Endianness::Big, 8)] {
        let prefix = vec![0xEE; 3];
        let mut fresh = Writer::with_max_align(endianness, max_align);
        body(&mut fresh);
        let mut expected = prefix.clone();
        expected.extend_from_slice(fresh.as_bytes());

        let mut appending = Writer::appending(endianness, max_align, prefix);
        body(&mut appending);
        assert_eq!(
            appending.finish(),
            expected,
            "{endianness:?}, MAXALIGN {max_align}"
        );
    }
}

#[test]
fn an_emheader_packs_its_id_code_and_flag_into_one_word() {
    // XTypes 1.3 §7.4.3.4.2: must-understand in bit 31, length code in
    // 30..28, member id in the low 28.
    let mut w = Writer::little_endian();
    let header = w
        .write_emheader(3, 4, true)
        .expect("a header the plan chose");
    w.write_i32(9);
    w.finish_emheader(header)
        .expect("the member is the length its code declares");

    let bytes = w.finish();
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = r.read_emheader().expect("emheader");
    assert_eq!(read.member_id, 3);
    assert_eq!(read.length_code, 4);
    assert!(read.must_understand);
    assert_eq!(
        read.next_int,
        Some(4),
        "LC4 carries a NEXTINT of the length"
    );
}

/// §7.2.2.4.4.4.4: member IDs "have a range that can be represented in 28
/// bits: from zero to 268,435,455 (0x0FFFFFFF)". An EMHEADER keeps the id in
/// the low 28 bits (§7.4.3.4.2); a larger id is refused and not masked.
#[test]
fn an_emheader_member_id_past_28_bits_is_refused() {
    let mut w = Writer::little_endian();
    w.write_emheader(0x0FFF_FFFF, 2, false)
        .expect("the largest id the 28 bits hold");
    for id in [0x1000_0000_u32, 0x8000_0001, u32::MAX] {
        let error = w
            .write_emheader(id, 2, true)
            .expect_err("an id past 28 bits");
        assert!(
            matches!(error, WireError::OutOfRange { .. }),
            "{id:#x}: {error:?}"
        );
    }
}

#[test]
fn a_length_code_below_four_carries_no_next_int() {
    // LC0 to LC3 carry the length in the code itself; writing a NEXTINT
    // for them would put four bytes on the wire that no peer expects.
    let mut w = Writer::little_endian();
    let header = w
        .write_emheader(1, 2, false)
        .expect("a header the plan chose");
    w.write_i32(5);
    w.finish_emheader(header)
        .expect("the member is the length its code declares");

    let bytes = w.finish();
    assert_eq!(bytes.len(), 8, "a NEXTINT was written for LC2: {bytes:?}");
}

/// The short-header framing: Fast DDS's, and the only one under which
/// `write_pid` is used at all — Cyclone's, which is not the default (that is `Dialect::Specification`), writes
/// `PID_EXTENDED` for every member and its reader requires one.
/// `zerodds_ast_layout::PlCdr1HeaderPolicy` carries the measurements.
const SHORT_FORM: PlCdr1Framing = PlCdr1Framing {
    pid_flags: PidFlagPolicy::Bare,
    highest_short_id: 0x3f00,
    pad_declared_length: false,
    pad_declared_optional_length: false,
    member_must_understand: true,
    require_extended: false,
    pad_after_value: false,
};

#[test]
fn a_parameter_length_excludes_the_padding_that_follows_it() {
    // Under Fast DDS's framing the length field carries the unpadded
    // value length; the padding to the next four-byte boundary is not
    // counted. Cyclone and RTI round it up — the case below.
    let mut w = Writer::little_endian().with_pl_cdr1_framing(SHORT_FORM);
    let pid = w
        .write_pid(5, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_u8(0xAA);
    w.finish_pid(pid);

    let bytes = w.finish();
    let mut r = Reader::new(&bytes, Endianness::Little).with_pl_cdr1_framing(SHORT_FORM);
    let read = r.read_pid().expect("pid");
    assert_eq!(read.pid, 5);
    assert_eq!(read.length, 1, "the padding was counted into the length");
    // And no trailing padding either: a final parameter ends the
    // stream where its value does — Fast DDS measured at 22 octets
    // where RTI writes 24 on `annotation-must-understand-unannotated`.
    // Between two parameters the next header aligns itself, which the
    // vendor rows keep honest.
    assert_eq!(
        bytes.len(),
        5,
        "a trailing pad was written under short-true-length"
    );
}

/// DDSI-RTPS 2.5 §9.4.2.11's reading of the length, which Cyclone's and
/// RTI's serialisers write: rounded up to a multiple of four.
const ROUNDED: PlCdr1Framing = PlCdr1Framing {
    pad_declared_length: true,
    pad_declared_optional_length: true,
    ..PlCdr1Framing::DEFAULT
};

/// The same parameter under the framing Cyclone's and RTI's serialisers
/// write.
///
/// One byte of value, a declared length of four, and the same eight
/// bytes out of the serialiser. `vendor_bytes.md`'s `ext_mutable`
/// measures the split on its trailing `short`; no capture has read a
/// PL_CDR1 header off a socket.
#[test]
fn a_parameter_length_covers_the_padding_where_the_framing_says_so() {
    let mut w = Writer::little_endian().with_pl_cdr1_framing(ROUNDED);
    let pid = w
        .write_pid(5, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_u8(0xAA);
    w.finish_pid(pid);
    let bytes = w.finish();
    assert_eq!(bytes.len(), 8);

    // Read back under Fast DDS's framing, the declared four does not
    // match the one byte that followed, and the error names the setting
    // the stream does fit rather than reading a shorter value.
    let mut r = Reader::new(&bytes, Endianness::Little).with_pl_cdr1_framing(SHORT_FORM);
    let header = r.read_pid().expect("the header");
    assert_eq!(header.length, 4);
    let _ = r.read_u8().expect("the value");
    match r.finish_pid(header) {
        Err(WireError::WrongFraming {
            fits: Some(fits), ..
        }) => assert!(
            fits.contains("short-rounded-length"),
            "the error does not name the framing that fits: {fits}"
        ),
        other => panic!("a padded length read as unpadded came back {other:?}"),
    }
}

/// XTypes 1.3 §7.4.1.2, S. 140: "the value of the parameter length is the
/// exact length of the serialized member. It does not account for any
/// padding bytes that may follow the serialized member. Padding bytes may
/// be added in order to start the next parameterID at a 4 byte offset
/// relative to the previous parameterID."
#[test]
fn the_default_parameter_length_is_the_exact_length_of_the_member() {
    let mut w = Writer::little_endian();
    let first = w
        .write_pid(5, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_u8(0xAA);
    w.finish_pid(first);
    let second = w
        .write_pid(6, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_u16(0xBBCC);
    w.finish_pid(second);
    w.write_sentinel();
    let bytes = w.finish();
    assert_eq!(
        bytes,
        [
            0x05, 0x00, 0x01, 0x00, // id 5, length 1 (not 4)
            0xAA, 0x00, 0x00, 0x00, // the value and the padding after it
            0x06, 0x00, 0x02, 0x00, // id 6, length 2 (not 4)
            0xCC, 0xBB, 0x00, 0x00, // the value and the padding after it
            0x02, 0x7f, 0x00, 0x00, // PID_LIST_END
        ],
        "the next parameter ID starts on a four-byte offset"
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    let header = r.read_pid().expect("first header");
    assert_eq!((header.pid, header.length), (5, 1));
    assert_eq!(r.read_u8().expect("value"), 0xAA);
    r.finish_pid(header)
        .expect("the exact length is the default");
    let header = r.read_pid().expect("second header");
    assert_eq!((header.pid, header.length), (6, 2));
    assert_eq!(r.read_u16().expect("value"), 0xBBCC);
    r.finish_pid(header)
        .expect("the exact length is the default");
    assert!(r.read_pid().expect("sentinel").end);
}

/// The DDSI-RTPS 2.5 §9.4.2.11 reading is a named setting: a default
/// reader reports the rounded length and names the framing that fits.
#[test]
fn a_default_reader_refuses_the_rounded_length_and_names_the_setting() {
    let mut w = Writer::little_endian().with_pl_cdr1_framing(ROUNDED);
    let pid = w
        .write_pid(5, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_u8(0xAA);
    w.finish_pid(pid);
    let bytes = w.finish();
    assert_eq!(&bytes[..4], &[0x05, 0x00, 0x04, 0x00]);

    let mut r = Reader::new(&bytes, Endianness::Little);
    let header = r.read_pid().expect("the header");
    let _ = r.read_u8().expect("the value");
    match r.finish_pid(header) {
        Err(WireError::WrongFraming {
            fits: Some(fits), ..
        }) => assert!(fits.contains("short-rounded-length"), "{fits}"),
        other => panic!("the rounded length was taken by the strict reader: {other:?}"),
    }
}

/// XTypes 1.3 §7.4.1.2.1: a reader shall be "robust to any setting of the
/// FLAG_MUST_UNDERSTAND bit and accept the parameter nevertheless", and
/// Table 34: "Implementations shall be robust to receiving parameter ID
/// 0x3F02 to indicate the end of a list as well." The strict reader
/// reports the flags of another framing; the tolerant reader takes them.
#[test]
fn a_tolerant_reader_takes_any_flags_on_the_sentinel() {
    for pid in [0x3f02u16, 0x7f02, 0xbf02, 0xff02] {
        let [lo, hi] = pid.to_le_bytes();
        let bytes = [lo, hi, 0x00, 0x00];
        let strict_expects = Reader::new(&bytes, Endianness::Little)
            .with_pl_cdr1_framing(PlCdr1Framing::DEFAULT)
            .read_pid();
        // The default framing writes the sentinel with the bit set.
        assert_eq!(
            strict_expects.is_ok(),
            pid == 0x7f02,
            "the strict reader's verdict on {pid:#06x}: {strict_expects:?}"
        );
        let mut tolerant =
            Reader::new(&bytes, Endianness::Little).with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
        assert!(
            tolerant.read_pid().expect("a tolerant reader").end,
            "{pid:#06x} did not end the list"
        );
        let mut bare = Reader::new(&bytes, Endianness::Little).with_pl_cdr1_framing(SHORT_FORM);
        assert_eq!(bare.read_pid().is_ok(), pid == 0x3f02, "{pid:#06x}");
    }
}

#[test]
fn a_tolerant_reader_takes_any_flags_on_the_extended_marker() {
    for marker in [0x3f01u16, 0x7f01, 0xbf01, 0xff01] {
        let [lo, hi] = marker.to_le_bytes();
        let bytes = [
            lo, hi, 0x08, 0x00, // PID_EXTENDED with some flags, length 8
            0x09, 0x00, 0x00, 0x00, // extended id 9
            0x04, 0x00, 0x00, 0x00, // extended length 4
            0x2a, 0x00, 0x00, 0x00, // the value
        ];
        let strict = Reader::new(&bytes, Endianness::Little).read_pid();
        assert_eq!(
            strict.is_ok(),
            marker == 0x7f01,
            "the strict reader's verdict on {marker:#06x}: {strict:?}"
        );
        let mut tolerant =
            Reader::new(&bytes, Endianness::Little).with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
        let header = tolerant.read_pid().expect("a tolerant reader");
        assert_eq!((header.pid, header.length), (9, 4), "{marker:#06x}");
        assert_eq!(tolerant.read_u32().expect("value"), 42);
        tolerant.finish_pid(header).expect("the block ends");
    }
}

/// XTypes 1.3 §7.4.1.2.2: "if a parameter could be described using a
/// short parameter header or an extended header, the short and extended
/// expressions of that header shall be considered totally equivalent."
#[test]
fn a_tolerant_reader_takes_the_short_form_where_the_extended_one_is_expected() {
    let always_extended = PlCdr1Framing {
        require_extended: true,
        ..PlCdr1Framing::DEFAULT
    };
    let short = [
        0x01, 0x00, 0x04, 0x00, // id 1, length 4
        0x07, 0x00, 0x00, 0x00, // the value
        0x02, 0x7f, 0x00, 0x00, // PID_LIST_END
    ];
    let strict = Reader::new(&short, Endianness::Little)
        .with_pl_cdr1_framing(always_extended)
        .read_pid();
    assert!(
        matches!(strict, Err(WireError::WrongFraming { .. })),
        "{strict:?}"
    );

    let mut tolerant = Reader::new(&short, Endianness::Little)
        .with_pl_cdr1_framing(always_extended)
        .with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
    let header = tolerant.read_pid().expect("the short form");
    assert_eq!((header.pid, header.length), (1, 4));
    assert_eq!(tolerant.read_i32().expect("value"), 7);
    tolerant.finish_pid(header).expect("the block ends");
    assert!(tolerant.read_pid().expect("sentinel").end);

    // The extended form of the same parameter reads to the same member.
    let extended = [
        0x01, 0x7f, 0x08, 0x00, // PID_EXTENDED, length 8
        0x01, 0x00, 0x00, 0x00, // id 1
        0x04, 0x00, 0x00, 0x00, // length 4
        0x07, 0x00, 0x00, 0x00, // the value
    ];
    let mut r =
        Reader::new(&extended, Endianness::Little).with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
    let header = r.read_pid().expect("the extended form");
    assert_eq!((header.pid, header.length), (1, 4));
}

#[test]
fn a_tolerant_reader_takes_either_length_convention_and_no_third() {
    let mut w = Writer::little_endian().with_pl_cdr1_framing(ROUNDED);
    let pid = w
        .write_pid(5, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_u8(0xAA);
    w.finish_pid(pid);
    let rounded = w.finish();

    let mut r =
        Reader::new(&rounded, Endianness::Little).with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
    let header = r.read_pid().expect("the header");
    assert_eq!(header.length, 4);
    assert_eq!(r.read_u8().expect("value"), 0xAA);
    r.finish_pid(header).expect("the rounded length");

    let mut exact = rounded.clone();
    exact[2] = 1;
    let mut r = Reader::new(&exact, Endianness::Little)
        .with_pl_cdr1_framing(ROUNDED)
        .with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
    let header = r.read_pid().expect("the header");
    assert_eq!(r.read_u8().expect("value"), 0xAA);
    r.finish_pid(header).expect("the exact length");

    // A length that is neither is a malformed stream under both readings.
    let mut neither = rounded.clone();
    neither[2] = 2;
    let mut r =
        Reader::new(&neither, Endianness::Little).with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
    let header = r.read_pid().expect("the header");
    let _ = r.read_u8().expect("value");
    assert!(matches!(
        r.finish_pid(header),
        Err(WireError::OutOfRange { .. })
    ));
}

#[test]
fn the_extended_parameter_form_carries_a_thirty_two_bit_id() {
    let mut w = Writer::little_endian();
    let pid = w
        .write_pid_extended(0x0001_0002, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_i32(1);
    w.finish_pid(pid);

    let bytes = w.finish();
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = r.read_pid().expect("pid");
    assert_eq!(read.pid, 0x0001_0002);
    assert_eq!(read.length, 4);
}

#[test]
fn the_sentinel_ends_a_parameter_list() {
    let mut w = Writer::little_endian();
    w.write_sentinel();
    let bytes = w.finish();
    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = r.read_pid().expect("sentinel");
    assert!(read.end, "the sentinel did not read as the list's end");
}

#[test]
fn a_sequence_writes_a_count_and_an_array_does_not() {
    let values = [1i32, 2, 3];

    let mut sequence = Writer::little_endian();
    sequence
        .write_sequence(&values, |w, v| {
            w.write_i32(*v);
            Ok(())
        })
        .expect("a long never refuses");
    assert_eq!(sequence.as_bytes().len(), 16, "count plus three longs");

    let mut array = Writer::little_endian();
    array
        .write_array(&values, |w, v| {
            w.write_i32(*v);
            Ok(())
        })
        .expect("a long never refuses");
    assert_eq!(array.as_bytes().len(), 12, "an array wrote a count");
}

#[test]
fn a_sequence_round_trips() {
    let mut w = Writer::little_endian();
    w.write_sequence(&[7i32, 8, 9], |w, v| {
        w.write_i32(*v);
        Ok(())
    })
    .expect("a long never refuses");
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = r.read_sequence(Reader::read_i32).expect("sequence");
    assert_eq!(read, vec![7, 8, 9]);
}

#[test]
fn a_truncated_stream_is_an_error_not_a_zero() {
    // A reader that returned a zero would hand the caller a value that
    // was never sent.
    let mut r = Reader::new(&[1, 2], Endianness::Little);
    assert!(matches!(r.read_i32(), Err(WireError::Truncated { .. })));
}

#[test]
fn a_length_larger_than_the_stream_is_refused_before_anything_is_allocated() {
    // Allocating for a claimed length is how a reader turns a bad
    // message into an out-of-memory.
    let bytes = [0xFF, 0xFF, 0xFF, 0x7F];
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert!(matches!(
        r.read_string(),
        Err(WireError::ImpossibleLength { .. })
    ));

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert!(matches!(
        r.read_sequence(Reader::read_i32),
        Err(WireError::ImpossibleLength { .. })
    ));
}

#[test]
fn a_boolean_that_is_neither_zero_nor_one_is_refused() {
    let mut r = Reader::new(&[2], Endianness::Little);
    assert!(matches!(r.read_bool(), Err(WireError::OutOfRange { .. })));
}

#[test]
fn a_string_of_bytes_that_are_not_utf8_is_refused() {
    let bytes = [3, 0, 0, 0, 0xFF, 0xFE, 0];
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert!(matches!(r.read_string(), Err(WireError::NotUtf8 { .. })));
}

#[test]
fn a_short_holder_is_the_hash_zero_padded() {
    let mut w = Writer::big_endian();
    w.write_i32(1);
    let hash = w.zero_padded();
    assert_eq!(hash[..4], [0, 0, 0, 1]);
    assert_eq!(hash[4..], [0u8; 12]);
}

#[test]
fn the_digest_matches_the_published_vectors() {
    // RFC 1321 appendix A.5. Checking against them is what makes this
    // MD5 rather than something that agrees with itself.
    assert_eq!(
        hex(&zerodds_wire::md5(b"abc")),
        "900150983cd24fb0d6963f7d28e17f72"
    );
    assert_eq!(
        hex(&zerodds_wire::md5(b"")),
        "d41d8cd98f00b204e9800998ecf8427e"
    );
    // The rest of A.5, which cross every padding boundary the digest has:
    // one block, a rest of 26 and 62 octets, and three full blocks plus 16.
    for (input, digest) in [
        (&b"a"[..], "0cc175b9c0f1b6a831c399e269772661"),
        (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        (
            b"abcdefghijklmnopqrstuvwxyz",
            "c3fcd3d76192e4007dfb496cca67e13b",
        ),
        (
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            "d174ab98d277d9f5a5611c2c9f419d9f",
        ),
        (
            b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
            "57edf4a22be3c955ac49da2e2107b67a",
        ),
    ] {
        assert_eq!(hex(&zerodds_wire::md5(input)), digest, "{input:?}");
    }
}

/// Lengths 55, 56 and 64 are where the padding changes from one final
/// block to two and where a whole block is followed by a padding-only
/// one. Checked against the digest computed one step at a time, as RFC
/// 1321 §3 describes it, over the same octets.
#[test]
fn the_digest_is_the_same_at_every_padding_boundary() {
    for length in [0usize, 1, 55, 56, 57, 63, 64, 65, 119, 120, 128] {
        let input: Vec<u8> = (0..length).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(
            zerodds_wire::md5(&input),
            reference_md5(&input),
            "length {length}"
        );
    }
}

/// RFC 1321 §3 literally: pad the message on the heap, then one `match`
/// per step. Slow and plain on purpose; it is the comparison.
fn reference_md5(input: &[u8]) -> [u8; 16] {
    let s: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // RFC 1321 §3.4: K[i] = floor(abs(sin(i + 1)) * 2^32).
    let k: Vec<u32> = (0..64)
        .map(|i| ((f64::from(i + 1)).sin().abs() * 4_294_967_296.0) as u32)
        .collect();
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((input.len() as u64) * 8).to_le_bytes());
    let mut h = [0x6745_2301_u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    for chunk in message.chunks_exact(64) {
        let m: Vec<u32> = chunk
            .chunks_exact(4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let sum = a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(sum.rotate_left(s[i]));
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

fn hex(bytes: &[u8; 16]) -> String {
    let mut out = String::new();
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// -- the reader's own silent drops -----------------------------------
//
// Each of these failed before the fix named at it, and each failed by
// returning a plausible value rather than an error. `vendor_reader.rs`
// found three of them against foreign bytes; the rest are shapes no
// vendor sample in `vendor_bytes.md` happens to contain, so they are
// built here from the writer's own output and from the clause.

/// A length code of 5 to 7 leaves its NEXTINT where the member starts.
///
/// XTypes 1.3 §7.4.3.4.2: the NEXTINT of codes 5, 6 and 7 "shall be
/// reused also as part of the serialized member ... Therefore the use of
/// length codes 5 to 7 saves 4 bytes in the serialization."
/// `write_emheader` reserves no placeholder for them, for that reason.
/// The reader consumed one anyway, so the sequence behind the header
/// lost its own length word and everything after it was read four bytes
/// out — with no error, because four bytes were there to read.
#[test]
fn a_length_code_of_five_leaves_its_next_int_for_the_member() {
    let mut w = Writer::little_endian();
    let header = w
        .write_emheader(2, 5, false)
        .expect("a header the plan chose");
    // The member is a `sequence<octet>` of three: its own length word is
    // the NEXTINT the header refers to.
    w.write_u32(3);
    w.write_u8(7);
    w.write_u8(8);
    w.write_u8(9);
    w.finish_emheader(header)
        .expect("the member is the length its code declares");
    let bytes = w.finish();
    assert_eq!(bytes.len(), 11, "a placeholder was reserved: {bytes:?}");

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = r.read_emheader().expect("emheader");
    assert_eq!(read.length_code, 5);
    assert_eq!(read.next_int, Some(3), "the NEXTINT was not seen");
    assert_eq!(read.extent, 7, "four bytes of length plus three of body");
    assert_eq!(r.position(), 4, "the NEXTINT was consumed");
    assert_eq!(
        r.read_sequence(Reader::read_u8).expect("the member"),
        [7, 8, 9]
    );
}

/// A NEXTINT larger than the stream is refused where it was read.
#[test]
fn an_emheader_next_int_that_cannot_be_true_is_refused() {
    // LC4, id 1, then a NEXTINT of all ones.
    let bytes = [0x01, 0x00, 0x00, 0x40, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_emheader() {
        Err(WireError::ImpossibleLength { at, claimed, .. }) => {
            assert_eq!(at, 4);
            assert_eq!(claimed, 0xFFFF_FFFF);
        }
        other => panic!("a NEXTINT of all ones was accepted: {other:?}"),
    }
}

/// A delimited collection cannot run past its own delimiter.
///
/// The delimiter was read and thrown away, so a sequence whose count
/// reached beyond it went on consuming the next member's bytes and
/// returned them as elements of its own.
#[test]
fn a_delimited_sequence_cannot_run_past_its_delimiter() {
    let mut w = Writer::little_endian();
    // A delimiter of 8: a count of one and one element. Then a member
    // that is not part of the collection.
    w.write_u32(8);
    w.write_u32(1);
    w.write_u32(0x1111_1111);
    w.write_u32(0x2222_2222);
    let mut bytes = w.finish();
    // Overwrite the count with two: the second element would be the
    // member behind the delimiter.
    bytes[4] = 2;

    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_delimited_sequence(Reader::read_u32) {
        Err(WireError::Truncated { .. }) => {}
        other => panic!("the sequence read past its delimiter: {other:?}"),
    }
}

/// A delimited collection that does not fill its delimiter is an error.
#[test]
fn a_delimited_sequence_that_stops_short_of_its_delimiter_is_refused() {
    let mut w = Writer::little_endian();
    w.write_u32(12); // room for a count and two elements
    w.write_u32(1); // but only one element follows
    w.write_u32(0x1111_1111);
    w.write_u32(0x2222_2222);
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_delimited_sequence(Reader::read_u32) {
        Err(WireError::OutOfRange { at, .. }) => assert_eq!(at, 12),
        other => panic!("four bytes inside the delimiter went unread: {other:?}"),
    }
}

/// A string whose last byte is not a NUL is a frame disagreeing with
/// itself. §7.4.1.1.2 puts the terminator there and the length counts it.
#[test]
fn a_string_whose_terminator_is_not_a_nul_is_refused() {
    let bytes = [4, 0, 0, 0, b'a', b'b', b'c', b'X'];
    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_string() {
        Err(WireError::OutOfRange { at, .. }) => assert_eq!(at, 7),
        other => panic!("a string ran on past its terminator: {other:?}"),
    }
}

/// A NUL inside a string is refused, because a C peer stops at it.
///
/// Both readers accept the bytes and return different values — this one
/// a three-character string, the peer a one-character one. Two peers
/// disagreeing about a sample neither rejected is the class.
#[test]
fn a_string_with_a_nul_inside_it_is_refused() {
    let bytes = [4, 0, 0, 0, b'a', 0, b'c', 0];
    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_string() {
        Err(WireError::OutOfRange { at, .. }) => assert_eq!(at, 5),
        other => panic!("a string with an interior NUL was accepted: {other:?}"),
    }
}

/// A parameter length longer than the stream is refused.
#[test]
fn a_parameter_length_that_cannot_be_true_is_refused() {
    // id 5, length 0xFFFF, and four bytes behind it.
    let bytes = [5, 0, 0xFF, 0xFF, 1, 2, 3, 4];
    let mut r = Reader::new(&bytes, Endianness::Little).with_pl_cdr1_framing(SHORT_FORM);
    match r.read_pid() {
        Err(WireError::ImpossibleLength { at, claimed, .. }) => {
            assert_eq!(at, 2);
            assert_eq!(claimed, 0xFFFF);
        }
        other => panic!("a parameter length of all ones was accepted: {other:?}"),
    }
}

/// The must-understand bit is a flag, not part of the id.
///
/// XTypes 1.3 §7.4.1.2 puts the member id in the low fourteen bits.
/// Cyclone sets bit 14 on every parameter it writes and RTI sets it on
/// the sentinel, so `PID_EXTENDED` goes out as `0x7f01` and the sentinel
/// as `0x7f02` — `crates/ast-layout/tests/fixtures/vendor_bytes.md` §2.
/// Matching the whole word recognised neither, so a Cyclone parameter
/// list read as data and ran on past its own end.
#[test]
fn a_flagged_sentinel_and_a_flagged_extended_header_are_recognised() {
    // Cyclone's shape: PID_EXTENDED with the flag, then the sentinel
    // with the flag.
    let bytes = [
        0x01, 0x7f, 0x08, 0x00, // 0x7f01, length 8
        0x09, 0x00, 0x00, 0x00, // extended id 9
        0x04, 0x00, 0x00, 0x00, // extended length 4
        0x2a, 0x00, 0x00, 0x00, // the value
        0x02, 0x7f, 0x00, 0x00, // 0x7f02, the sentinel
    ];
    let mut r = Reader::new(&bytes, Endianness::Little);
    let header = r.read_pid().expect("the extended header");
    assert_eq!(header.pid, 9, "the flag was read as part of the id");
    assert_eq!(header.length, 4);
    // The marker's flag is not the member's. Cyclone sets `0x7f01` on
    // every parameter it writes and `FLAG_2` of the `eMemberHeader`
    // only where the member declares must-understand; taking the
    // member's property off the marker would make the whole list
    // must-understand. XTypes 1.3 §7.4.1.2.1.
    assert!(!header.must_understand);
    assert_eq!(r.read_u32().expect("the value"), 42);
    r.finish_pid(header).expect("the block ends");
    assert!(
        r.read_pid().expect("the sentinel").end,
        "a flagged sentinel was read as a parameter"
    );
}

/// `FLAG_2` of the extended `eMemberHeader` is the member's
/// must-understand, and the four flag bits are not part of the id.
///
/// XTypes 1.3 §7.4.1.2.1. Reading the whole word as the id gave a
/// Cyclone member declared `@must_understand` an id of `0x40000001`,
/// which matches nothing a model declares — and under XCDR1 an
/// unmatched parameter was skipped, so the member was silently dropped.
#[test]
fn flag_two_of_an_extended_header_is_the_members_must_understand() {
    let mut w = Writer::little_endian();
    let pid = w
        .write_pid_extended(1, ParameterRole::Member, true)
        .expect("a header the plan chose");
    w.write_i32(7);
    w.finish_pid(pid);
    // Cyclone's own bytes for `@id(1) @must_understand long a = 7`,
    // `vendor_bytes.md` §2.
    assert_eq!(
        w.as_bytes(),
        &[
            0x01, 0x7f, 0x08, 0x00, // 0x7f01, length 8
            0x01, 0x00, 0x00, 0x40, // FLAG_2 | id 1
            0x04, 0x00, 0x00, 0x00, // length 4
            0x07, 0x00, 0x00, 0x00, // the value
        ]
    );

    let bytes = w.finish();
    let mut r = Reader::new(&bytes, Endianness::Little);
    let header = r.read_pid().expect("the extended header");
    assert_eq!(header.pid, 1, "the flag bits were read as part of the id");
    assert!(header.must_understand);
}

/// A parameter's value counts alignment from its own block.
///
/// XTypes 1.3 §7.4.1.2: each parameter "is simply a CDR-encapsulated
/// block of data". `Writer::write_pid` was given this and the reader was
/// not, so a `double` at the head of a block got four bytes of padding
/// no vendor wrote and came out of the following bytes.
#[test]
fn a_parameter_value_counts_alignment_from_its_own_block() {
    let mut w = Writer::new(Endianness::Little).with_pl_cdr1_framing(SHORT_FORM);
    let pid = w
        .write_pid(1, ParameterRole::Member, false)
        .expect("a header the plan chose");
    w.write_f64(1.5);
    w.finish_pid(pid);
    let bytes = w.finish();
    assert_eq!(bytes.len(), 12, "padding inside the block: {bytes:?}");

    let mut r = Reader::new(&bytes, Endianness::Little).with_pl_cdr1_framing(SHORT_FORM);
    let header = r.read_pid().expect("the header");
    assert_eq!(header.length, 8);
    assert_eq!(
        r.read_f64().expect("the value"),
        1.5,
        "the double was read from the wrong offset"
    );
    r.finish_pid(header).expect("the block ends");
    assert_eq!(r.remaining(), 0);
}

/// Padding that is not there is an error at the offset it began.
#[test]
fn padding_the_stream_does_not_have_is_a_named_error() {
    // A char, then nothing where a long's padding and body belong.
    let bytes = [b'a'];
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_char().expect("char"), 'a');
    match r.read_i32() {
        Err(WireError::Truncated { at, needed, .. }) => {
            assert_eq!(at, 1, "the offset must be where the padding began");
            assert_eq!(needed, 3);
        }
        other => panic!("{other:?}"),
    }
}

// -- the writer-time half of a length code -------------------------------

/// An LC4 header whose member turns out to fit a short code.
///
/// `zerodds_ast_layout::LengthCodePolicy::ShortWhereItFits` is Fast
/// DDS's, and half of it cannot be planned: a `sequence<short>` of two
/// elements is eight bytes and a third element makes it ten. Measured
/// 2026-08-09, case `mut_lc_enum`: Fast DDS writes `01000030` over the
/// eight-byte sequence where Cyclone writes `01000050` over a DHEADER.
///
/// The NEXTINT placeholder is removed and the code raised, so the
/// member's own bytes move back four and change in nothing. That is
/// safe only because `MAXALIGN` is four under XCDR2: both candidate
/// start offsets are four-aligned, so no padding inside the member was
/// computed against an offset that moved.
#[test]
fn a_short_length_code_replaces_the_next_int_when_the_member_fits_one() {
    for (length, code, body) in [
        (1usize, 0u8, vec![0xABu8]),
        (2, 1, vec![0xAB, 0xCD]),
        (4, 2, vec![1, 0, 0, 0]),
        (8, 3, vec![1, 0, 0, 0, 2, 0, 0, 0]),
    ] {
        let mut w = Writer::with_max_align(Endianness::Little, 4).with_short_length_codes(true);
        let header = w
            .write_emheader(1, 4, false)
            .expect("a header the plan chose");
        for byte in &body {
            w.write_u8(*byte);
        }
        w.finish_emheader(header)
            .expect("the member is the length its code declares");
        let bytes = w.finish();
        assert_eq!(
            bytes.len(),
            4 + length,
            "the NEXTINT survived a {length}-byte member: {bytes:?}"
        );
        let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
        let read = r.read_emheader().expect("emheader");
        assert_eq!(read.length_code, code, "{length} bytes");
        assert_eq!(read.member_id, 1);
        assert_eq!(read.extent, length);
    }
}

/// A length no code can spell keeps its NEXTINT.
#[test]
fn a_member_that_fits_no_short_code_keeps_its_next_int() {
    let mut w = Writer::with_max_align(Endianness::Little, 4).with_short_length_codes(true);
    let header = w
        .write_emheader(1, 4, false)
        .expect("a header the plan chose");
    for _ in 0..3 {
        w.write_u8(7);
    }
    w.finish_emheader(header)
        .expect("the member is the length its code declares");
    let bytes = w.finish();
    assert_eq!(bytes.len(), 11, "{bytes:?}");
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
    let read = r.read_emheader().expect("emheader");
    assert_eq!(read.length_code, 4);
    assert_eq!(read.next_int, Some(3));
}

/// Without the flag nothing is shortened, which is Cyclone and RTI.
#[test]
fn the_default_writer_leaves_an_lc4_header_alone() {
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    let header = w
        .write_emheader(1, 4, false)
        .expect("a header the plan chose");
    w.write_i32(5);
    w.finish_emheader(header)
        .expect("the member is the length its code declares");
    assert_eq!(w.finish(), [1, 0, 0, 0x40, 4, 0, 0, 0, 5, 0, 0, 0]);
}

/// The must-understand bit survives the rewrite, and so does big endian.
#[test]
fn shortening_a_header_keeps_its_id_and_its_flag_in_both_orders() {
    for endianness in [Endianness::Little, Endianness::Big] {
        let mut w = Writer::with_max_align(endianness, 4).with_short_length_codes(true);
        let header = w
            .write_emheader(0x0ABCDEF, 4, true)
            .expect("a header the plan chose");
        w.write_i32(5);
        w.finish_emheader(header)
            .expect("the member is the length its code declares");
        let bytes = w.finish();
        assert_eq!(bytes.len(), 8, "{endianness:?}: {bytes:?}");
        let mut r = Reader::with_max_align(&bytes, endianness, 4);
        let read = r.read_emheader().expect("emheader");
        assert_eq!(read.member_id, 0x0ABCDEF, "{endianness:?}");
        assert_eq!(read.length_code, 2, "{endianness:?}");
        assert!(read.must_understand, "{endianness:?}");
    }
}

// -- the writer-time half of a parameter header form ---------------------

/// A short parameter header whose value turns out not to fit one.
///
/// §7.4.1.2.2's `ssize` is sixteen bits. Which members overrun it is a
/// property of the sample, not of the type — a `string` is short in
/// almost every sample and over 64 KiB in some — so the plan writes the
/// short form for an unbounded member and the writer falls back here.
/// RTI and Fast DDS write the short form for `mut_string`'s `string s`;
/// `zerodds_ast_layout::needs_extended_pid` carries that run.
#[test]
fn a_short_parameter_header_becomes_extended_when_the_value_overruns_it() {
    let mut w = Writer::with_max_align(Endianness::Little, 8).with_pl_cdr1_framing(SHORT_FORM);
    let header = w
        .write_pid(1, ParameterRole::Member, false)
        .expect("a header the plan chose");
    // A value one byte past what a sixteen-bit length can state.
    let body = vec![0xABu8; 0x1_0000];
    for byte in &body {
        w.write_u8(*byte);
    }
    w.finish_pid(header);
    let bytes = w.finish();

    // `PID_EXTENDED`, `slength` of 8, the 32-bit id, the 32-bit length.
    assert_eq!(&bytes[..4], &[0x01, 0x3f, 0x08, 0x00], "the marker");
    assert_eq!(&bytes[4..8], &[0x01, 0, 0, 0], "the member id");
    assert_eq!(&bytes[8..12], &[0x00, 0x00, 0x01, 0x00], "the length");
    assert_eq!(bytes.len(), 12 + body.len());
    assert_eq!(&bytes[12..14], &[0xAB, 0xAB], "the value moved intact");

    let mut r =
        Reader::with_max_align(&bytes, Endianness::Little, 8).with_pl_cdr1_framing(SHORT_FORM);
    let read = r.read_pid().expect("a parameter header");
    assert_eq!(read.pid, 1);
    assert_eq!(read.length, body.len());
}

/// One byte less and the short form stands.
#[test]
fn a_value_that_fits_sixteen_bits_keeps_the_short_header() {
    let mut w = Writer::with_max_align(Endianness::Little, 8).with_pl_cdr1_framing(SHORT_FORM);
    let header = w
        .write_pid(1, ParameterRole::Member, false)
        .expect("a header the plan chose");
    for _ in 0..0xFFFF {
        w.write_u8(0xAB);
    }
    w.finish_pid(header);
    let bytes = w.finish();
    assert_eq!(&bytes[..4], &[0x01, 0x00, 0xff, 0xff], "the short header");
    // The value and nothing after it: the declared length is the true
    // one under this framing, and so is the stream's end — the next
    // id, where one follows, aligns itself.
    assert_eq!(bytes.len(), 4 + 0xFFFF);
}

/// The promoted header carries the member's must-understand bit, in the
/// place the extended form puts it.
#[test]
fn a_promoted_header_moves_the_must_understand_bit_to_flag_two() {
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    let header = w
        .write_pid(1, ParameterRole::Member, true)
        .expect("a header the plan chose");
    for _ in 0..0x1_0000 {
        w.write_u8(0);
    }
    w.finish_pid(header);
    let bytes = w.finish();
    // The default framing sets the marker's bit, so `0x7f01`, and
    // `FLAG_2` of the eMemberHeader rather than bit 14 of a short id.
    assert_eq!(&bytes[..4], &[0x01, 0x7f, 0x08, 0x00]);
    assert_eq!(&bytes[4..8], &[0x01, 0x00, 0x00, 0x40]);
}

/// `fixed<P, S>` as BCD — CORBA 3.3 part 2 §9.3.2.8.
///
/// The clause is the oracle and there is no other: no DDS vendor generates
/// code for `fixed`. Measured across the corpus, Cyclone, Fast DDS and RTI
/// emit nothing for it and OpenDDS does not compile it — all sixteen of its
/// rows in `vendor_bytes.rows.tsv` read `ungenerated` or `uncompiled`. What
/// holds the shape is the clause, and that the twenty other columns of this
/// project write it the same way through `zerodds-wire-codec`.
#[test]
fn a_fixed_is_bcd_nibbles_high_first_with_the_sign_last() {
    fn bytes(text: &str, digits: u8, scale: u8, policy: FixedDigits) -> Vec<u8> {
        let mut w = Writer::new(Endianness::Little);
        w.write_fixed(text, digits, scale, policy)
            .expect("a decimal that fits");
        w.finish()
    }

    // Five digits and a sign is six nibbles: three bytes, no padding.
    assert_eq!(
        bytes("123.45", 5, 2, FixedDigits::Declared),
        vec![0x12, 0x34, 0x5c]
    );
    // `0xd` is the negative sign; `-1.5` under fixed<3,1> is 0, 1, 5.
    assert_eq!(bytes("-1.5", 3, 1, FixedDigits::Declared), vec![0x01, 0x5d]);
    // Zero is positive, and so is negative zero: there is one zero to write.
    assert_eq!(bytes("0", 3, 1, FixedDigits::Declared), vec![0x00, 0x0c]);
    assert_eq!(bytes("-0.0", 3, 1, FixedDigits::Declared), vec![0x00, 0x0c]);
    // The policy decides how many digits go out, and only that: the scale
    // is the type's and no value changes it, so `1` under fixed<5, 2> is
    // the digits 1, 0, 0 either way. `Declared` writes all five digits the
    // declaration names, `ValuePrecision` drops the leading zeros and keeps
    // what `BigDecimal.precision()` would count.
    assert_eq!(
        bytes("1", 5, 2, FixedDigits::Declared),
        vec![0x00, 0x10, 0x0c]
    );
    assert_eq!(
        bytes("1", 5, 2, FixedDigits::ValuePrecision),
        vec![0x10, 0x0c]
    );
}

/// A value that does not fit is refused, never truncated.
///
/// A silently shortened decimal is the wrong number with nothing to notice it
/// by — the failure `fixed` exists to prevent.
#[test]
fn a_fixed_that_does_not_fit_is_refused() {
    fn refuse(text: &str, digits: u8, scale: u8) -> bool {
        Writer::new(Endianness::Little)
            .write_fixed(text, digits, scale, FixedDigits::Declared)
            .is_err()
    }
    assert!(refuse("1234", 5, 2), "the integer half holds three digits");
    assert!(refuse("1.234", 5, 2), "the fraction holds two");
    assert!(refuse("", 5, 2), "no digits at all");
    assert!(refuse("12a.4", 5, 2), "not a decimal");
    assert!(refuse("1.0", 0, 0), "a fixed with no digits holds nothing");
    assert!(refuse("1.0", 32, 2), "IDL rule (42) caps the form at 31");
    assert!(refuse("1.0", 2, 3), "a scale wider than the type");
    // Both boundaries pass, so the refusals above are about the value.
    assert!(!refuse("999.99", 5, 2));
    assert!(!refuse("1.0", 31, 2));
}

/// Writing what was read gives the same text, under both digit counts.
///
/// The pair is what makes `fixed` usable at all: a writer alone can be
/// checked against §9.3.2.8 by hand, but a reader that took the point one
/// digit off would return a value a hundred times too large and every byte
/// on the wire would still be the one the clause asks for.
#[test]
fn a_fixed_reads_back_as_the_one_spelling_its_declaration_admits() {
    // Declaration, written text, the text that comes back.
    for (digits, scale, written, canonical) in [
        (5u8, 2u8, "123.45", "123.45"),
        (5, 2, "1", "1.00"),
        (5, 2, "0001.5", "1.50"),
        (3, 1, "-1.5", "-1.5"),
        (3, 1, "0", "0.0"),
        // §9.3.2.8 has one zero and gives it the positive sign nibble, so a
        // negative zero comes back without its sign.
        (3, 1, "-0.0", "0.0"),
        (5, 0, "12345", "12345"),
        (5, 5, ".12345", "0.12345"),
        (
            31,
            2,
            "12345678901234567890123456789.99",
            "12345678901234567890123456789.99",
        ),
        (1, 0, "9", "9"),
    ] {
        for policy in [FixedDigits::Declared, FixedDigits::ValuePrecision] {
            let mut w = Writer::new(Endianness::Little);
            w.write_fixed(written, digits, scale, policy)
                .unwrap_or_else(|error| panic!("{written} under fixed<{digits},{scale}>: {error}"));
            let bytes = w.finish();
            let mut r = Reader::new(&bytes, Endianness::Little);
            let read = r
                .read_fixed(digits, scale, policy)
                .unwrap_or_else(|error| panic!("{written} {policy:?}: {error}"));
            assert_eq!(
                read, canonical,
                "{written} under fixed<{digits},{scale}> {policy:?} came back as `{read}`"
            );
            assert_eq!(
                r.remaining(),
                0,
                "{written} {policy:?}: the reader left {} octets of the field behind",
                r.remaining()
            );
            // And writing the read-back gives the same octets.
            let mut again = Writer::new(Endianness::Little);
            again
                .write_fixed(&read, digits, scale, policy)
                .expect("the read-back writes");
            assert_eq!(
                again.finish(),
                bytes,
                "{written} {policy:?}: the read-back writes other octets"
            );
        }
    }
}

/// A stream the reader cannot make a value of is refused, not guessed at.
#[test]
fn a_fixed_the_reader_cannot_make_a_value_of_is_refused() {
    let read = |bytes: &[u8], digits, scale, policy| {
        Reader::new(bytes, Endianness::Little)
            .read_fixed(digits, scale, policy)
            .is_err()
    };
    // A nibble that is neither a digit nor the sign.
    assert!(read(&[0x1a, 0x2c], 3, 1, FixedDigits::Declared));
    // A stream that ends inside the field.
    assert!(read(&[0x12], 5, 2, FixedDigits::Declared));
    // Value-precision with no sign nibble inside the widest fixed IDL
    // admits: sixteen octets of digits and the reader stops.
    assert!(read(&[0x11; 20], 5, 2, FixedDigits::ValuePrecision));
    // A declaration outside rule (42), on this side as on the writer's.
    assert!(read(&[0x12, 0x3c], 0, 0, FixedDigits::Declared));
    assert!(read(&[0x12, 0x3c], 32, 2, FixedDigits::Declared));
    assert!(read(&[0x12, 0x3c], 2, 3, FixedDigits::Declared));
}

#[test]
fn a_fixed_reader_refuses_value_precision_beyond_the_declared_bound() {
    // Valid BCD is not necessarily a value this declaration can carry.
    for (bytes, digits, scale) in [(&[0x12, 0x3c][..], 1, 0), (&[0x12, 0x3d][..], 2, 1)] {
        let mut r = Reader::new(bytes, Endianness::Little);
        assert!(
            r.read_fixed(digits, scale, FixedDigits::ValuePrecision)
                .is_err()
        );
    }
}

#[test]
fn a_fixed_reader_stops_before_a_seventeenth_octet() {
    // A following field must not supply the missing sign for this one.
    let mut bytes = [0u8; 17];
    bytes[16] = 0x1c;
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert!(r.read_fixed(31, 0, FixedDigits::ValuePrecision).is_err());
    assert_eq!(r.position(), 16);
    assert_eq!(r.read_u8().unwrap(), 0x1c);
}

/// ACE's `ACE_CDR::Fixed` digit count — recorded values from
/// `unspecified_constructs.md` §3.
///
/// These are the exact byte strings measured from ACE/TAO 2.5.24's
/// `ACE_CDR::Fixed` over `ACE_OutputCDR`, which is what OpenDDS's
/// generated `write_fixed` calls.
#[test]
fn value_string_digits_matches_ace_recorded_values() {
    fn bytes(text: &str, digits: u8, scale: u8, policy: FixedDigits) -> Vec<u8> {
        let mut w = Writer::new(Endianness::Little);
        w.write_fixed(text, digits, scale, policy)
            .expect("a decimal that fits");
        w.finish()
    }

    // `fixed<5,2>` holding `123.45` → `12345c` (3 octets)
    // String digits: 5 (odd) → no padding
    assert_eq!(
        bytes("123.45", 5, 2, FixedDigits::ValueStringDigits),
        vec![0x12, 0x34, 0x5c],
        "fixed<5,2> \"123.45\" from ACE: 12345c"
    );

    // `fixed<5,2>` holding `-123.45` → `12345d` (3 octets)
    // String digits: 5 (odd) → no padding
    assert_eq!(
        bytes("-123.45", 5, 2, FixedDigits::ValueStringDigits),
        vec![0x12, 0x34, 0x5d],
        "fixed<5,2> \"-123.45\" from ACE: 12345d"
    );

    // `fixed<4,2>` holding `12.34` → `01234c` (3 octets)
    // String digits: 4 (even) → padded to 5 digits: 01234
    assert_eq!(
        bytes("12.34", 4, 2, FixedDigits::ValueStringDigits),
        vec![0x01, 0x23, 0x4c],
        "fixed<4,2> \"12.34\" from ACE: 01234c"
    );

    // `fixed<1,0>` holding `7` → `7c` (1 octet)
    // String digits: 1 (odd) → no padding
    assert_eq!(
        bytes("7", 1, 0, FixedDigits::ValueStringDigits),
        vec![0x7c],
        "fixed<1,0> \"7\" from ACE: 7c"
    );

    // `fixed<5,5>` holding `0.12345` → `0012345c` (4 octets)
    // String digits: 6 (even) → padded to 7 digits: 0012345
    assert_eq!(
        bytes("0.12345", 5, 5, FixedDigits::ValueStringDigits),
        vec![0x00, 0x12, 0x34, 0x5c],
        "fixed<5,5> \"0.12345\" from ACE: 0012345c"
    );

    // `fixed<5,2>` holding `0.00` → `000c` (2 octets)
    // String digits: 3 (odd) → no padding
    assert_eq!(
        bytes("0.00", 5, 2, FixedDigits::ValueStringDigits),
        vec![0x00, 0x0c],
        "fixed<5,2> \"0.00\" from ACE: 000c"
    );

    // `fixed<31,0>` holding 31 nines → 16 octets (all 9s + sign)
    // String digits: 31 (odd) → no padding
    let thirty_one_nines = "9".repeat(31);
    let mut expected = vec![0x99; 15];
    expected.push(0x9c); // last digit 9 + positive sign
    assert_eq!(
        bytes(&thirty_one_nines, 31, 0, FixedDigits::ValueStringDigits),
        expected,
        "fixed<31,0> 31 nines from ACE"
    );
}

/// ValueStringDigits reads back correctly (round-trip).
#[test]
fn value_string_digits_reads_back() {
    let mut w = Writer::new(Endianness::Little);
    w.write_fixed("0.12345", 5, 5, FixedDigits::ValueStringDigits)
        .expect("write");
    let bytes = w.finish();
    assert_eq!(bytes, vec![0x00, 0x12, 0x34, 0x5c]);

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = r
        .read_fixed(5, 5, FixedDigits::ValueStringDigits)
        .expect("read");
    // The reader canonicalizes: leading zeros removed, scale applied
    assert_eq!(read, "0.12345");
}

/// A count past 31 digits as written has no width a reader can be told,
/// and one past 255 used to wrap in the `u8` it was counted into.
#[test]
fn value_string_digits_past_31_digits_as_written_is_refused() {
    for text in [
        "00000000000000000000000000000001".to_string(),
        format!("{}1", "0".repeat(300)),
    ] {
        let mut w = Writer::new(Endianness::Little);
        assert!(
            w.write_fixed(&text, 31, 0, FixedDigits::ValueStringDigits)
                .is_err(),
            "{} digits as written",
            text.len()
        );
    }
    let mut w = Writer::new(Endianness::Little);
    w.write_fixed(&"9".repeat(31), 31, 0, FixedDigits::ValueStringDigits)
        .expect("thirty-one nines are the widest field");
    assert_eq!(w.finish().len(), 16);
}

/// The fraction is counted at `S`, not as written: `7` under `fixed<5,2>`
/// is the three digits `700`. Counting the one digit of the text kept
/// only the last nibble of the field, `0c`, which reads back as zero.
#[test]
fn value_string_digits_counts_the_scale_not_the_fraction_as_written() {
    let mut w = Writer::new(Endianness::Little);
    w.write_fixed("7", 5, 2, FixedDigits::ValueStringDigits)
        .expect("write");
    let bytes = w.finish();
    assert_eq!(bytes, vec![0x70, 0x0c]);
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(
        r.read_fixed(5, 2, FixedDigits::ValueStringDigits)
            .expect("read"),
        "7.00"
    );
}

/// A writer that continues a prefix counts alignment from the start of the
/// prefix (`Writer::continuing`), where one that appends counts from the end
/// of it (`Writer::appending`): an 8-octet value behind a 4-octet prefix is
/// padded to offset 8 in the first and to offset 4 + 8 in the second.
#[test]
fn a_continuing_writer_aligns_to_the_start_of_the_prefix() {
    let prefix = vec![1, 2, 3, 4];

    let mut continuing = Writer::continuing(Endianness::Little, 8, prefix.clone());
    continuing.write_u64(0x1122_3344_5566_7788);
    let bytes = continuing.finish();
    assert_eq!(bytes.len(), 16, "four octets of padding, then eight");
    assert_eq!(bytes[..4], [1, 2, 3, 4]);
    assert_eq!(bytes[4..8], [0, 0, 0, 0]);

    let mut appending = Writer::appending(Endianness::Little, 8, prefix);
    appending.write_u64(0x1122_3344_5566_7788);
    assert_eq!(
        appending.finish().len(),
        12,
        "counted from the end of the prefix"
    );
}

/// DDS-XTypes 1.3 §7.4.1.1.3.2: "the states of the remaining serialized
/// bits are not specified, and those bits are not considered to be part
/// of the bitmask"; Table 18: "Preserve bit values by index for all bits
/// identified in both T1 and T2". A bit the stream sets and the type does
/// not declare reads as not set, for every holder width.
#[test]
fn a_bitmask_read_keeps_only_the_declared_bits() {
    let mut r = Reader::new(&[0b0000_1111], Endianness::Little);
    assert_eq!(r.read_bitmask(8, 0b0000_0101).expect("read"), 0b0000_0101);

    let mut r = Reader::new(&[0xFF, 0xFF], Endianness::Little);
    assert_eq!(r.read_bitmask(16, 0x8001).expect("read"), 0x8001);

    let mut r = Reader::new(&[0xFF; 4], Endianness::Big);
    assert_eq!(r.read_bitmask(32, 0x0000_0010).expect("read"), 0x10);

    let mut r = Reader::new(&[0xFF; 8], Endianness::Little);
    assert_eq!(
        r.read_bitmask(64, 0x8000_0000_0000_0000).expect("read"),
        0x8000_0000_0000_0000
    );
}

/// XTypes 1.3 §7.4.3.4.1: "O.ssize is constrained to being smaller than 4
/// Giga Bytes". A DHEADER is a `UInt32`; a span of 4 GiB has none and is
/// refused, where `finish_dheader` would have written it modulo 2^32.
#[test]
fn a_dheader_length_of_4_gib_or_more_is_refused() {
    use zerodds_wire::{WireError, Writer, writer::checked_dheader_length};
    assert_eq!(checked_dheader_length(0, 0).unwrap(), 0);
    assert_eq!(
        checked_dheader_length(u32::MAX as usize, 0).unwrap(),
        u32::MAX
    );
    #[cfg(target_pointer_width = "64")]
    {
        let error = checked_dheader_length(u32::MAX as usize + 1, 12).unwrap_err();
        assert!(
            matches!(error, WireError::OutOfRange { at: 12, .. }),
            "{error:?}"
        );
    }
    // The writer's own refusal is the same function on the span it holds.
    let mut w = Writer::little_endian();
    let header = w.write_dheader();
    w.write_u32(7);
    w.try_finish_dheader(header).unwrap();
    assert_eq!(w.finish(), [4, 0, 0, 0, 7, 0, 0, 0]);
}

#[test]
fn the_ordered_map_reader_refuses_a_stream_that_repeats_a_key() {
    // XTypes 1.3 §7.2.2.4.3, Table 7: a map key "must be unique within a given map object". A
    // `BTreeMap` would keep the second entry of a key and drop the first, so the reader that
    // builds one refuses the stream instead of reporting success on fewer entries than counted.
    // (The entries form, `read_map_entries`, drops nothing and keeps both.)
    let mut w = Writer::little_endian();
    w.write_u32(2);
    for value in [10i32, 20] {
        w.write_i32(1);
        w.write_i32(value);
    }
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    let refused = r.read_map(|r| r.read_i32(), |r| r.read_i32());
    assert!(
        matches!(refused, Err(WireError::OutOfRange { .. })),
        "{refused:?}"
    );

    let mut r = Reader::new(&bytes, Endianness::Little);
    let kept = r
        .read_map_entries(|r| r.read_i32(), |r| r.read_i32())
        .expect("the entries form keeps both");
    assert_eq!(kept.len(), 2);
}
