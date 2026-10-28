// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bytes the generated `map` codec puts on the wire.
//!
//! Everything here runs generated code, as `bytes.rs` does. The
//! difference is where the expected octets come from: the first test
//! holds them against a **recorded foreign measurement** rather than
//! against a second sequence this project wrote itself.
//!
//! # The foreign measurement
//!
//! `tests/interop-results/sources/vendor-corpus.tsv`, the row
//!
//! ```text
//! map-key-string  fastdds  xcdr1  be  wrote  000000030000000100000000ffed29790000000
//! 6616c7068610000007fffffff00000006c3bce188b400000000000001ffffffff
//! ```
//!
//! (one line in the file; wrapped here). Its IDL is
//! `tests/interop-corpus/map-key-string/type.idl` —
//! `@final struct Sample { map<string, long> entries; long tail; }` —
//! and its sample is that case's `values.json`: `"" -> -1234567`,
//! `"alpha" -> 2147483647`, `"üሴ" -> 1`, with `tail = -1`.
//!
//! Fast DDS is the only implementation in the corpus that produces a
//! `map`'s octets at all; the Cyclone row is `refused — syntax error`
//! and the RTI, OpenDDS and HDDS rows all say their C front end has no
//! `map`. `crates/wire-probe/tests/map_entry_order.rs` carries that
//! finding and its evidence.
//!
//! # What the row settles
//!
//! - **The length is the entry count.** `00000003` for three entries
//!   and six halves. XTypes 1.3 §7.4.1.1.4 defines `map<K, V>` by
//!   equivalence to a `sequence` whose element is the pair, and this is
//!   the equivalence as octets.
//! - **Key first, then value, per entry.** `00000006 616c7068 61000000`
//!   — `"alpha"` with its NUL and its padding — then `7fffffff`.
//! - **No delimiter here.** The row is XCDR1, which delimits nothing;
//!   the same case's `xcdr2 be` row is byte-identical, which is Fast
//!   DDS's `MapDelimiter::ValueOnly` reading of rule (14). This project
//!   takes the broader `EitherHalf` reading under
//!   `Dialect::Specification`, so the XCDR2 comparison belongs to
//!   `a_map_is_delimited_only_where_rule_14_does_not_exempt_it` and not
//!   here.
//! - **The order is not what this row separates.** `"" < "alpha" < "üሴ"`
//!   is ascending under an unsigned octet comparison *and* is the order
//!   `values.json` declares, so the row is the same either way. The
//!   order this column writes is the order the entries are held in
//!   (`zerodds_ast_layout::MapEntryOrder::Declaration`);
//!   the axis itself is measured in `map_entry_order.rs`.
//!
//! # The mutation
//!
//! Swap the two halves in `Writer::write_map` —
//! `value(self, v)?; key(self, k)?;` — and
//! [`the_bytes_are_the_recorded_fast_dds_row`] fails at the first
//! differing offset, byte 4: the entry count is followed by
//! `ffed2979` where the row has the empty key's `00000001`.
//! Swapping the two `element_writer` pushes in
//! `crates/lang-rules/src/encode.rs`'s `MemberWrite::Map` arm produces
//! the same red, through the generated code instead of through the
//! runtime.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire::MapEntry;

use zerodds_wire_conformance::generated::{
    MapBounded, MapDelimit, MapKeyString, TcMapDiscard, TcMapTrim, TcMapUseDefault,
};
use zerodds_wire_conformance::{Endianness, Reader, WireError, Writer};

/// The bytes one generated `encode` produces, big-endian.
///
/// Big-endian because the recorded row is: the byte order is the
/// writer's and not the plan's, so the same generated `encode` answers
/// both and the comparison needs no second generation.
fn encoded_be(f: impl FnOnce(&mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut w = Writer::big_endian();
    f(&mut w).expect("every value here fits its member");
    w.finish()
}

/// The bytes one generated `encode` produces, little-endian.
fn encoded_le(f: impl FnOnce(&mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut w = Writer::little_endian();
    f(&mut w).expect("every value here fits its member");
    w.finish()
}

/// A hex string as octets, so the recorded row can be quoted verbatim.
fn octets(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "an odd number of hex digits");
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("a hex pair"))
        .collect()
}

/// `tests/interop-corpus/map-key-string/values.json`, as this column's
/// type.
fn map_key_string_sample() -> MapKeyString {
    let entries = vec![
        MapEntry::new(String::new(), -1_234_567),
        MapEntry::new("alpha".to_string(), 2_147_483_647),
        MapEntry::new("üሴ".to_string(), 1),
    ];
    MapKeyString { entries, tail: -1 }
}

/// The value of the entry under `key`: a map is held as its entries (XTypes 1.3 §7.5.1.3.6).
fn value_of(entries: &[MapEntry<String, i32>], key: &str) -> i32 {
    entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.value)
        .expect("an entry under the key")
}

// -- against the recorded foreign row -------------------------------------

#[test]
fn the_bytes_are_the_recorded_fast_dds_row() {
    // Quoted from `tests/interop-results/sources/vendor-corpus.tsv`,
    // `map-key-string fastdds xcdr1 be`.
    const RECORDED: &str = "000000030000000100000000ffed297900000006616c7068610000007fffffff\
                            00000006c3bce188b400000000000001ffffffff";
    let expected = octets(RECORDED);
    let bytes = encoded_be(|w| map_key_string_sample().encode(w));

    // The first differing offset, not `assert_eq!` on the whole vector:
    // a swapped pair or a missing pad is one offset, and a message that
    // prints two fifty-two-byte arrays does not say which.
    if let Some(at) = (0..bytes.len().min(expected.len())).find(|at| bytes[*at] != expected[*at]) {
        panic!(
            "byte {at}: this column writes {:#04x} where Fast DDS wrote {:#04x}\n  ours: \
             {bytes:02x?}\n  theirs: {expected:02x?}",
            bytes[at], expected[at]
        );
    }
    assert_eq!(
        bytes.len(),
        expected.len(),
        "the two agree on every shared byte and not on how many there are\n  ours: \
         {bytes:02x?}\n  theirs: {expected:02x?}"
    );
}

#[test]
fn the_length_word_is_the_entry_count_and_not_the_number_of_halves() {
    // The claim on its own, so a failure says which of the two numbers
    // is on the wire rather than only that byte 0 differs. Three
    // entries, six halves; §7.4.1.1.4 counts pairs.
    let bytes = encoded_be(|w| map_key_string_sample().encode(w));
    assert_eq!(
        &bytes[0..4],
        &[0, 0, 0, 3],
        "the count is not 3 — a 6 here is the halves, which no clause asks for"
    );
}

#[test]
fn the_recorded_row_round_trips_back_into_the_sample() {
    // The reader against foreign octets, not against this column's own:
    // an encoder and a decoder that share a mistake agree with each
    // other and with nobody else.
    const RECORDED: &str = "000000030000000100000000ffed297900000006616c7068610000007fffffff\
                            00000006c3bce188b400000000000001ffffffff";
    let bytes = octets(RECORDED);
    let mut r = Reader::new(&bytes, Endianness::Big);
    let read = MapKeyString::decode(&mut r).expect("the recorded row decodes");

    assert_eq!(read.entries.len(), 3);
    assert_eq!(value_of(&read.entries, ""), -1_234_567);
    assert_eq!(value_of(&read.entries, "alpha"), 2_147_483_647);
    assert_eq!(value_of(&read.entries, "üሴ"), 1);
    assert_eq!(read.tail, -1);
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");
}

// -- the delimiter, rule (14) --------------------------------------------

#[test]
fn a_map_is_delimited_only_where_rule_14_does_not_exempt_it() {
    // §7.4.3.5 rule (14) exempts a map from the DHEADER only when
    // **both** halves are primitive, and rule (15) delimits the rest.
    // `MapDelimit` is `map<long, long> plain; map<string, long> keyed;`
    // under XCDR2, so the two members answer the two sides of the rule
    // in one sample and the offsets say which.
    let plain = vec![MapEntry::new(1i32, 10i32)];
    let keyed = vec![MapEntry::new("a".to_string(), 20i32)];
    let bytes = encoded_le(|w| {
        MapDelimit {
            plain,
            keyed,
            tail: 7,
        }
        .encode(w)
    });

    // `plain`: no DHEADER. Count 1, then the pair.
    assert_eq!(&bytes[0..4], &[1, 0, 0, 0], "the exempt map has a DHEADER");
    assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
    assert_eq!(&bytes[8..12], &[10, 0, 0, 0]);
    // `keyed`: a DHEADER, then the count, the key `"a"` with its NUL and
    // its padding, and the value. Sixteen bytes inside the delimiter.
    assert_eq!(
        &bytes[12..16],
        &[16, 0, 0, 0],
        "the string-keyed map is not delimited, or its DHEADER is wrong"
    );
    assert_eq!(&bytes[16..20], &[1, 0, 0, 0], "the entry count");
    assert_eq!(&bytes[20..24], &[2, 0, 0, 0], "the key's length, with NUL");
    assert_eq!(&bytes[24..28], &[b'a', 0, 0, 0]);
    assert_eq!(&bytes[28..32], &[20, 0, 0, 0]);
    assert_eq!(&bytes[32..36], &[7, 0, 0, 0], "the tail");
    assert_eq!(bytes.len(), 36, "{bytes:02x?}");
}

#[test]
fn a_delimited_map_round_trips() {
    let plain = vec![MapEntry::new(-3i32, 4i32), MapEntry::new(9i32, -8i32)];
    let keyed = vec![
        MapEntry::new("beta".to_string(), 1i32),
        MapEntry::new("alpha".to_string(), 2i32),
    ];
    let sample = MapDelimit {
        plain,
        keyed,
        tail: 5,
    };
    let bytes = encoded_le(|w| sample.encode(w));

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = MapDelimit::decode(&mut r).expect("decode");
    assert_eq!(read.plain, sample.plain);
    assert_eq!(read.keyed, sample.keyed);
    assert_eq!(read.tail, 5);
    assert_eq!(r.remaining(), 0, "the decoder left bytes unread");
}

// -- the bound ------------------------------------------------------------

#[test]
fn a_bounded_map_writes_the_same_bytes_as_an_unbounded_one() {
    // Table 17's `O2.length <= T1.length` is a property of the value.
    // The wire carries no bound to compare with, so `map<long, long, 2>`
    // with two entries is the same octets a `map<long, long>` would be.
    let pairs = vec![MapEntry::new(1i32, 2i32), MapEntry::new(3i32, 4i32)];
    let bytes = encoded_le(|w| MapBounded { pairs, tail: 6 }.encode(w));

    assert_eq!(&bytes[0..4], &[2, 0, 0, 0], "the entry count");
    assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
    assert_eq!(&bytes[8..12], &[2, 0, 0, 0]);
    assert_eq!(&bytes[12..16], &[3, 0, 0, 0]);
    assert_eq!(&bytes[16..20], &[4, 0, 0, 0]);
    assert_eq!(&bytes[20..24], &[6, 0, 0, 0], "the tail");
    assert_eq!(bytes.len(), 24, "{bytes:02x?}");
}

#[test]
fn a_bounded_map_refuses_a_value_that_overruns_its_bound() {
    // Refused and not written short, for the reason
    // `write_string_bounded` gives: the receiver's own `map<K, V, N>`
    // has room for `N`, and a sender that wrote more has written a value
    // of a type nobody declared. Nothing partial reaches the caller,
    // because `encoded_*` would have unwrapped the refusal.
    let mut pairs = Vec::new();
    for k in 0..3i32 {
        pairs.push(MapEntry::new(k, k));
    }
    let mut w = Writer::little_endian();
    let error = MapBounded { pairs, tail: 0 }
        .encode(&mut w)
        .expect_err("three entries in a map bounded at two");
    match error {
        WireError::OutOfRange { expected, .. } => {
            assert!(expected.contains("entries"), "{expected}");
        }
        other => panic!("the bound is not what refused: {other:?}"),
    }
}

// -- `@try_construct`, Table 11 ------------------------------------------

/// A `@mutable` payload whose `m` is a `map<long, long>` of `entries`
/// pairs, written by hand so the reading types below are narrower than
/// what produced it — which is the only shape the question has, since
/// the bound is not on the wire.
fn wide_map_payload(entries: i32) -> Vec<u8> {
    let mut w = Writer::little_endian();
    let dheader = w.write_dheader();
    let h = w.write_emheader(0, 4, false).expect("a");
    w.write_i32(7);
    w.finish_emheader(h).expect("a");
    let h = w.write_emheader(1, 4, false).expect("m");
    #[allow(clippy::cast_sign_loss)]
    w.write_u32(entries as u32);
    for k in 0..entries {
        w.write_i32(k);
        w.write_i32(k * 10);
    }
    w.finish_emheader(h).expect("m");
    let h = w.write_emheader(2, 4, false).expect("b");
    w.write_i32(9);
    w.finish_emheader(h).expect("b");
    w.finish_dheader(dheader);
    w.finish()
}

#[test]
fn a_map_that_fits_its_bound_is_taken_under_every_behaviour() {
    // The control. Two entries into a `map<long, long, 2>` never reaches
    // a narrowing at all, so all three types agree — and a test that
    // only showed the over-long cases could not tell a narrowing from a
    // member that never decodes.
    let bytes = wide_map_payload(2);
    for (label, m) in [
        (
            "DISCARD",
            TcMapDiscard::decode(&mut Reader::new(&bytes, Endianness::Little))
                .expect("two entries fit")
                .m,
        ),
        (
            "USE_DEFAULT",
            TcMapUseDefault::decode(&mut Reader::new(&bytes, Endianness::Little))
                .expect("two entries fit")
                .m,
        ),
        (
            "TRIM",
            TcMapTrim::decode(&mut Reader::new(&bytes, Endianness::Little))
                .expect("two entries fit")
                .m,
        ),
    ] {
        assert_eq!(m.len(), 2, "{label}");
        assert_eq!(m[0].key, 0, "{label}");
        assert_eq!(m[0].value, 0, "{label}");
        assert_eq!(m[1].key, 1, "{label}");
        assert_eq!(m[1].value, 10, "{label}");
    }
}

#[test]
fn an_over_long_map_is_discarded_where_the_position_says_discard() {
    // §7.2.2.7: an absent annotation is DISCARD, and a discarded sample
    // is an absence rather than a malformed stream.
    let bytes = wide_map_payload(4);
    let error = TcMapDiscard::decode(&mut Reader::new(&bytes, Endianness::Little))
        .expect_err("four entries into a map bounded at two");
    assert!(error.is_discarded(), "{error:?}");
}

#[test]
fn an_over_long_map_takes_the_default_where_the_position_says_use_default() {
    // Table 9's default for a map is "an empty map of the same key and
    // value types", and the members after it still come back correct —
    // the whole value was read off the wire before the bound was
    // applied.
    let bytes = wide_map_payload(4);
    let read = TcMapUseDefault::decode(&mut Reader::new(&bytes, Endianness::Little))
        .expect("the sample survives");
    assert!(read.m.is_empty(), "{:?}", read.m);
    assert_eq!(read.a, 7);
    assert_eq!(
        read.b, 9,
        "the member after the narrowed one is at the wrong offset"
    );
}

#[test]
fn an_over_long_map_is_trimmed_where_the_position_says_trim() {
    // §7.3.1.2.1.12.3 takes the first elements: the first two entries, in the
    // order the stream carried them.
    let bytes = wide_map_payload(4);
    let read =
        TcMapTrim::decode(&mut Reader::new(&bytes, Endianness::Little)).expect("the sample is cut");
    assert_eq!(read.m.len(), 2, "{:?}", read.m);
    assert_eq!(read.m[0].value, 0);
    assert_eq!(read.m[1].value, 10);
    assert_eq!(read.m[1].key, 1);
    assert_eq!(read.a, 7);
    assert_eq!(
        read.b, 9,
        "the member after the narrowed one is at the wrong offset"
    );
}

// -- the silent half ------------------------------------------------------

#[test]
fn a_stream_that_repeats_a_key_keeps_every_entry() {
    // The wire carries a sequence of entries (XTypes 1.3 §7.4.1.1.4), and a map held as its
    // entries (§7.5.1.3.6) keeps all of them. A `BTreeMap` would keep the last entry of a key
    // and drop the others, which is why the decode of that container refused the stream; here
    // nothing is dropped, and what a repeated key means is the application's to decide.
    let mut w = Writer::little_endian();
    w.write_u32(2);
    for value in [10i32, 20] {
        w.write_i32(1);
        w.write_i32(value);
    }
    w.write_i32(-1);
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    let read = MapBounded::decode(&mut r).expect("both entries are kept");
    let values: Vec<i32> = read.pairs.iter().map(|entry| entry.value).collect();
    assert_eq!(values, [10, 20]);
}

#[test]
fn a_count_larger_than_the_stream_is_refused_before_anything_is_allocated() {
    // An entry is two values and neither is empty, so a count larger
    // than the bytes left cannot be true. `read_sequence` makes the same
    // check for the same reason: a length that cannot be true is how a
    // reader turns a bad message into an out-of-memory.
    let mut w = Writer::little_endian();
    w.write_u32(u32::MAX);
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    let error = MapBounded::decode(&mut r).expect_err("a count nothing could fill");
    assert!(
        matches!(error, WireError::ImpossibleLength { .. }),
        "{error:?}"
    );
}
