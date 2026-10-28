// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The five constructs the probe used to refuse, against the rules that
//! define them.
//!
//! # These are not vendor bytes
//!
//! `vendor_bytes.rs` next door compares the probe against hex copied out
//! of Cyclone DDS, RTI Connext and Fast DDS runs. Most of this file is
//! not that: `crates/ast-layout/tests/fixtures/vendor_bytes.md` has no
//! `wstring`, no `bitmask`, no `bitset`, no `map` and no `fixed` in it,
//! so the byte strings below were worked out from the specification
//! text.
//!
//! The last three tests are the exception and they say so at each test.
//! `crates/ast-layout/tests/fixtures/unspecified_constructs.md`
//! measured a bitset in a collection and as a key member against Fast
//! DDS, and both readings of a delimited `map`'s length against Fast
//! DDS and OpenDDS, so those expectations are foreign bytes and not a
//! reading of the clause.
//!
//! A test that only compares the probe against itself proves nothing
//! about interop. What these do prove is narrower and still worth
//! having: that the probe writes what the clause quoted above each test
//! says, and that a later edit which changes those bytes is a
//! deliberate act rather than a side effect. Every expectation here is
//! **unverified against a vendor** until the harness on the Linux test
//! host produces the same case from Cyclone, RTI and Fast DDS.
//!
//! Two of the five have no clause at all. `bitset` and `fixed` are not
//! in DDS-XTypes 1.3; where their bytes come from is written at the
//! test, and those two are the weakest expectations in the file.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::encode;
use zerodds_wire_probe::encode::{ProbeError, hex, model_from_idl, probe};
use zerodds_wire_probe::value::{Emit, MapLengthPolicy, ProbeRequest};

/// The bytes the probe produces, as hex.
fn bytes(idl: &str, type_name: &str, values: &serde_json::Value, encoding: Encoding) -> String {
    hex(&probe(idl, type_name, values, encoding, Endianness::Little).expect("the probe"))
}

/// The sixteen-byte KeyHash of one sample.
fn key_hash(idl: &str, type_name: &str, values: &serde_json::Value) -> String {
    let ast = model_from_idl(idl, "probe.idl").expect("the model");
    let request = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Little)
        .emitting(Emit::KeyHash);
    hex(&encode(&ast, &request, values).expect("the probe"))
}

/// The error the probe refuses with.
fn refusal(idl: &str, type_name: &str, values: &serde_json::Value) -> ProbeError {
    probe(
        idl,
        type_name,
        values,
        Encoding::PlainCdr2,
        Endianness::Little,
    )
    .expect_err("a refusal")
}

// -- wstring -----------------------------------------------------------

/// §7.4.3.5.3 rule (4) and §7.4.1.1.2.
///
/// The length is bytes and not characters, and no terminator follows —
/// the two places a `wstring` differs from the `string` beside it.
#[test]
fn a_wstring_counts_bytes_and_writes_no_terminator() {
    let idl = "@final struct WStr { wstring w; long n; };";

    // Two BMP characters: four bytes of content, and the `long` lands
    // at offset 8 with no padding, which it would not if a terminator
    // had been written.
    assert_eq!(
        bytes(
            idl,
            "WStr",
            &json!({ "w": "ab", "n": 7 }),
            Encoding::PlainCdr2
        ),
        "040000006100620007000000"
    );

    // An empty `wstring` is a length of zero and nothing else. The
    // narrow string beside it would write `01000000 00`.
    assert_eq!(
        bytes(
            idl,
            "WStr",
            &json!({ "w": "", "n": 7 }),
            Encoding::PlainCdr2
        ),
        "0000000007000000"
    );
}

/// §7.4.1.1.2, the rationale paragraph.
///
/// "By setting the serialized length equal to the number of bytes the
/// representation could support sending UTF-16 encoded Unicode
/// characters outside the BMP (which map to two UTF-16 units)." One
/// character, two code units, a length of four.
#[test]
fn a_wstring_outside_the_bmp_is_four_bytes_for_one_character() {
    let idl = "@final struct WStr { wstring w; long n; };";
    assert_eq!(
        bytes(
            idl,
            "WStr",
            &json!({ "w": "\u{1d11e}", "n": 7 }),
            Encoding::PlainCdr2
        ),
        "0400000034d81edd07000000"
    );
}

/// §7.4.3.5.3 rule (4) is version-independent: it is written `XCDR <<`
/// and not `XCDR[1] <<` or `XCDR[2] <<`, so both encodings write the
/// same thing. Only the trailing `long`'s alignment could differ, and
/// at offset 8 it does not.
#[test]
fn a_wstring_is_the_same_under_both_encoding_versions() {
    let idl = "@final struct WStr { wstring w; long n; };";
    let values = json!({ "w": "ab", "n": 7 });
    assert_eq!(
        bytes(idl, "WStr", &values, Encoding::PlainCdr1),
        bytes(idl, "WStr", &values, Encoding::PlainCdr2)
    );
}

// -- bitmask -----------------------------------------------------------

/// §7.4.3.5.3 rule (6) with Table 33 of §7.4.1.1.3.2.
///
/// The holder is `UInt8` at a bound of 1 to 8 and `UInt32` at the
/// default bound of 32, and "Bit indexes are counted from zero starting
/// at the least-significant bit".
#[test]
fn a_bitmask_is_the_integer_its_bit_bound_calls_for() {
    let narrow = "\
@bit_bound(8) bitmask Perm { READ, WRITE, EXEC };
@final struct B8 { Perm p; };";
    assert_eq!(
        bytes(
            narrow,
            "B8",
            &json!({ "p": ["READ", "WRITE"] }),
            Encoding::PlainCdr2
        ),
        "03"
    );

    // No `@bit_bound` is 32 — §7.3.1.2.1.9 — so four bytes.
    let wide = "\
bitmask Flags { A, B, C };
@final struct B32 { Flags f; };";
    assert_eq!(
        bytes(
            wide,
            "B32",
            &json!({ "f": ["A", "C"] }),
            Encoding::PlainCdr2
        ),
        "05000000"
    );

    // No bits set is the holder at zero, not an absent member.
    assert_eq!(
        bytes(wide, "B32", &json!({ "f": [] }), Encoding::PlainCdr2),
        "00000000"
    );
}

/// The example in §7.5.1.2.2, taken as a wire case.
///
/// The clause prints the binding for `@bit_bound(19) bitmask MyFlags`
/// with `@position(14)` on the second flag, and states the values it
/// produces: `1 << 0`, `1 << 14`, `1 << 15`. A bound of 19 puts the
/// holder in Table 33's 17-to-32 row, so `FIRST_FLAG | THIRD_FLAG` is
/// `0x00008001` in four bytes.
#[test]
fn an_explicit_position_moves_the_bits_the_specification_example_moves() {
    let idl = "\
@bit_bound(19) bitmask MyFlags {
    FIRST_FLAG,
    @position(14) SECOND_FLAG,
    THIRD_FLAG
};
@final struct Holder { MyFlags f; };";
    assert_eq!(
        bytes(
            idl,
            "Holder",
            &json!({ "f": ["FIRST_FLAG", "THIRD_FLAG"] }),
            Encoding::PlainCdr2
        ),
        "01800000"
    );
}

/// §7.4.1.1.3.2: bits above the bound "are not considered to be part of
/// the bitmask", so writing one is refused rather than folded in.
///
/// **The refusal moved to the declaration.** It used to come from the
/// value contract, which saw `"C"` as the third bit of a two-bit mask
/// and reported `Holder.f[0]`. The front end now refuses the
/// declaration itself under IDL 4.2 §7.4.13.4.3.3 — "a bit mask holds
/// no more values than the `@bit_bound` it was given" — so no model of
/// the type is built and no sample of it is ever offered.
///
/// A bit above the bound is not reachable through valid IDL any more:
/// it takes either more values than the bound, which is this case, or
/// an `@position` past it, which the same clause refuses —
/// `tools/wire-oracle/cases/bitmask_past_bound` is that declaration and
/// `tools/idlc/tests/corpus.rs` carries it as an intended `IDL0014`.
/// So the declaration's refusal is what there is to measure, and this
/// asserts it names the clause rather than failing somewhere else.
#[test]
fn a_bit_above_the_bound_is_refused() {
    let idl = "\
@bit_bound(2) bitmask Narrow { A, B, C };
@final struct Holder { Narrow f; };";
    let error = refusal(idl, "Holder", &json!({ "f": ["C"] }));
    let said = format!("{error}");
    assert!(
        said.contains("§7.4.13.4.3.3") && said.contains("bit mask holds no more values"),
        "{said}"
    );
}

/// A misspelled bit name would otherwise set nothing and pass.
#[test]
fn a_misspelled_bit_name_is_an_error_and_not_a_cleared_bit() {
    let idl = "\
bitmask Flags { A, B };
@final struct Holder { Flags f; };";
    let error = refusal(idl, "Holder", &json!({ "f": ["a"] }));
    assert!(
        matches!(&error, ProbeError::Unknown { path, declared }
            if path == "Holder.f[0]" && declared == &["A".to_string(), "B".to_string()]),
        "{error}"
    );
}

// -- bitset ------------------------------------------------------------

/// **No clause defines this.** DDS-XTypes 1.3 has no serialisation rule
/// for a bitset: §7.4.3.5.3 has no matching left-hand side and §7.4.1.1
/// does not cover the type. The bytes below follow from two things that
/// are fixed elsewhere and are written down at
/// `zerodds_wire_probe::encode::write_bitset`:
///
/// - positions run cumulatively from zero in declaration order, which
///   is how `crates/xml/src/typeobject_bridge.rs` fills
///   `CommonBitfield.position`. The clause number this line used to
///   carry, §7.3.4.6, is "Classification of TypeIdentifiers" and does
///   not support it; there is no clause that does, which is the whole
///   subject of `docs/refused-constructs.md`, "bitset";
/// - the anchor's width is the layout planner's, here two bytes for
///   fifteen declared bits.
///
/// `low = 5` at position 0 and `high = 4000` at position 3 is
/// `5 | (4000 << 3)` = `0x7d05`.
#[test]
fn a_bitset_packs_its_fields_from_the_least_significant_bit_up() {
    let idl = "\
bitset B { bitfield<3> low; bitfield<12> high; };
@final struct Holder { B packed; };";
    assert_eq!(
        bytes(
            idl,
            "Holder",
            &json!({ "packed": { "low": 5, "high": 4000 } }),
            Encoding::PlainCdr2
        ),
        "057d"
    );
}

/// An anonymous `bitfield<n>` is padding: it takes no value and moves
/// the field above it along. Sixteen bits in three fields, so `a` sits
/// at position 0 and `b` at position 8.
///
/// Unverified against a vendor, for the reason on the test above.
#[test]
fn an_anonymous_bitfield_shifts_the_field_above_it() {
    let idl = "\
bitset P { bitfield<3> a; bitfield<5>; bitfield<8> b; };
@final struct Holder { P padded; };";
    assert_eq!(
        bytes(
            idl,
            "Holder",
            &json!({ "padded": { "a": 7, "b": 171 } }),
            Encoding::PlainCdr2
        ),
        "07ab"
    );
}

/// A value wider than its bitfield would overwrite the field above it.
/// Masking it would put a number on the wire that the sample did not
/// ask for, which is the failure mode nobody notices.
#[test]
fn a_bitfield_value_that_does_not_fit_is_refused_and_not_masked() {
    let idl = "\
bitset B { bitfield<3> low; bitfield<12> high; };
@final struct Holder { B packed; };";
    let error = refusal(idl, "Holder", &json!({ "packed": { "low": 8, "high": 0 } }));
    assert!(
        matches!(&error, ProbeError::Mismatch { path, .. } if path == "Holder.packed.low"),
        "{error}"
    );
}

/// An inherited bitset is one flat run, base-first, in one anchor sized
/// over the whole chain.
///
/// XTypes has no serialisation rule for a bitset at all — established by
/// exhaustion in `docs/refused-constructs.md` — so the only oracle is
/// eProsima Fast DDS, the one implementation whose IDL front end parses
/// `bitset Derived : Base`. Measured on 2026-08-09:
/// `bitset Base { bitfield<3> low; }; bitset Derived : Base
/// { bitfield<5> high; }; @final struct Sample { Derived value; octet
/// tail; };` with `low = 5, high = 17, tail = 0xAB` gives `8dab` under
/// XCDR1 and XCDR2, little and big endian — byte for byte what the same
/// probe gives for the two bitfields declared in one bitset.
#[test]
fn an_inheriting_bitset_packs_the_base_at_the_low_positions() {
    let idl = "\
bitset Base { bitfield<3> low; };
bitset Derived : Base { bitfield<5> high; };
@final struct Holder { Derived d; octet tail; };";
    let values = json!({ "d": { "low": 5, "high": 17 }, "tail": 171 });
    assert_eq!(bytes(idl, "Holder", &values, Encoding::PlainCdr2), "8dab");
    assert_eq!(bytes(idl, "Holder", &values, Encoding::PlainCdr1), "8dab");

    // A three-level chain is what fails if only the refusal is lifted
    // and the planner is not consulted over the whole chain: Fast DDS
    // emits `std::bitset<20>`, a four-byte anchor aligned to its own
    // width, and writes `11220300 ab`.
    let deep = "\
bitset B1 { bitfield<8> a; };
bitset B2 : B1 { bitfield<8> b; };
bitset B3 : B2 { bitfield<4> c; };
@final struct Wide { B3 value; octet tail; };";
    let deep_values = json!({ "value": { "a": 17, "b": 34, "c": 3 }, "tail": 171 });
    assert_eq!(
        bytes(deep, "Wide", &deep_values, Encoding::PlainCdr2),
        "11220300ab"
    );
}

// -- map ---------------------------------------------------------------

/// §7.4.3.5.3 rule (14): a map whose key and element are both primitive
/// is `O.length` and then the pairs, under both encoding versions and
/// with no delimiter.
#[test]
fn a_map_of_primitives_is_a_length_and_the_pairs() {
    let idl = "@final struct MapLL { map<long, long> m; };";
    let values = json!({ "m": [[1, 2], [3, 4]] });
    assert_eq!(
        bytes(idl, "MapLL", &values, Encoding::PlainCdr2),
        "0200000001000000020000000300000004000000"
    );
    assert_eq!(
        bytes(idl, "MapLL", &values, Encoding::PlainCdr1),
        "0200000001000000020000000300000004000000"
    );
}

/// §7.4.3.5.3 rule (15) delimits a map that rule (14) does not exempt,
/// and the planner is the one that decides which: `zerodds_ast_layout`
/// sets `needs_dheader` when either half is non-primitive. A `long` key
/// and a `@final struct` value is that case.
///
/// The four bytes of delimiter cover the length word and both pairs,
/// twelve in all.
#[test]
fn a_map_with_a_structured_value_is_delimited_under_xcdr2() {
    let idl = "\
@final struct Cell { long a; };
@final struct MapLS { map<long, Cell> m; };";
    let values = json!({ "m": [[1, { "a": 2 }]] });
    assert_eq!(
        bytes(idl, "MapLS", &values, Encoding::PlainCdr2),
        "0c000000010000000100000002000000"
    );

    // Rule (16) is the version-1 case: a length and the pairs, no
    // delimiter, whatever the halves are.
    assert_eq!(
        bytes(idl, "MapLS", &values, Encoding::PlainCdr1),
        "010000000100000002000000"
    );
}

/// The specification says the entry count twice and differently.
///
/// §7.4.1.1.4 makes a map equivalent to `sequence<MapEntry>`, and rule
/// (12) writes a sequence as `DHEADER`, `O.length`, elements. Rule (15),
/// written for a map, has the `DHEADER` and no `O.length`. Four bytes
/// apart, both decodable — so the probe carries both and names them.
/// This test is the two shapes against the clause; the one at the end of
/// the file is the same two against Fast DDS's and OpenDDS's bytes.
#[test]
fn the_two_readings_of_a_delimited_maps_length_differ_by_four_bytes() {
    let idl = "\
@final struct Cell { long a; };
@final struct MapLS { map<long, Cell> m; };";
    let values = json!({ "m": [[1, { "a": 2 }]] });
    let ast = model_from_idl(idl, "probe.idl").expect("the model");

    let equivalence = ProbeRequest::new("MapLS", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        hex(&encode(&ast, &equivalence, &values).expect("bytes")),
        "0c000000010000000100000002000000",
        "§7.4.1.1.4 read as an equivalence to a sequence"
    );

    let rule_15 = ProbeRequest {
        map_length: MapLengthPolicy::Rule15Omits,
        ..equivalence
    };
    assert_eq!(
        hex(&encode(&ast, &rule_15, &values).expect("bytes")),
        "080000000100000002000000",
        "rule (15) read literally"
    );
}

/// A `map<K, V>` whose key is not a string cannot be a JSON object, so
/// the value format is an array of pairs and an entry that is not a
/// pair is an error naming its index.
#[test]
fn a_map_entry_that_is_not_a_pair_is_an_error_naming_its_index() {
    let idl = "@final struct MapLL { map<long, long> m; };";
    let error = refusal(idl, "MapLL", &json!({ "m": [[1, 2], [3]] }));
    assert!(
        matches!(&error, ProbeError::Mismatch { path, .. } if path == "MapLL.m[1]"),
        "{error}"
    );
}

/// A bounded map is checked like a bounded sequence: the bound is a
/// promise about the source, and an overrun is refused rather than
/// truncated into bytes no peer expected.
#[test]
fn a_bounded_map_refuses_more_entries_than_its_bound() {
    let idl = "@final struct MapB { map<long, long, 2> m; };";
    let error = refusal(idl, "MapB", &json!({ "m": [[1, 2], [3, 4], [5, 6]] }));
    assert!(
        matches!(&error, ProbeError::Mismatch { path, .. } if path == "MapB.m"),
        "{error}"
    );
}

// -- fixed -------------------------------------------------------------

/// **No DDS clause defines this.** DDS-XTypes 1.3 has no fixed-point
/// type at all — §7.2.2 has no such kind and §7.4.3.5.3 no such rule —
/// so a DDS peer is not required to read these bytes. The form is
/// CORBA's, [CDR] §9.3.2.8: `P` digit nibbles most-significant first,
/// then `0xc` for positive and `0xd` for negative, with a leading zero
/// nibble when `P + 1` is odd.
///
/// `fixed<5, 2>` is `(5 + 2) / 2` = three bytes, which is the extent
/// `zerodds_ast_layout` plans, and `123.45` is `12 34 5c`. Unverified
/// against any DDS vendor; the two ORBs the pre-rebuild `zerodds-cdr`
/// crate checked this shape against were JacORB 3.9 and omniORB 4.3.
#[test]
fn a_fixed_is_packed_bcd_with_a_sign_nibble() {
    let idl = "@final struct Money { fixed<5,2> amount; };";
    assert_eq!(
        bytes(
            idl,
            "Money",
            &json!({ "amount": "123.45" }),
            Encoding::PlainCdr2
        ),
        "12345c"
    );

    // A negative value differs only in the sign nibble.
    assert_eq!(
        bytes(
            idl,
            "Money",
            &json!({ "amount": "-1.5" }),
            Encoding::PlainCdr2
        ),
        "00150d"
    );

    // Zero is positive: there is one zero, and `0xd` would make two.
    assert_eq!(
        bytes(
            idl,
            "Money",
            &json!({ "amount": "-0.00" }),
            Encoding::PlainCdr2
        ),
        "00000c"
    );
}

/// An odd `P + 1` takes a leading zero nibble so the nibbles pair up.
/// `fixed<4, 0>` with `1234` is `01 23 4c`.
///
/// Unverified against a DDS vendor, as above.
#[test]
fn an_odd_digit_count_takes_a_leading_pad_nibble() {
    let idl = "@final struct Count { fixed<4,0> n; };";
    assert_eq!(
        bytes(idl, "Count", &json!({ "n": "1234" }), Encoding::PlainCdr2),
        "01234c"
    );
}

/// Alignment one, which is what the planner gives the type: the digits
/// are bytes, nothing pads in front of them, and the `long` behind them
/// aligns to four as it would after any three bytes.
#[test]
fn a_fixed_takes_no_alignment_of_its_own() {
    let idl = "@final struct Row { fixed<5,2> amount; long n; };";
    assert_eq!(
        bytes(
            idl,
            "Row",
            &json!({ "amount": "123.45", "n": 7 }),
            Encoding::PlainCdr2
        ),
        "12345c0007000000"
    );
}

/// A value with more integer digits than `P - S` does not fit, and
/// writing its low digits would put a different number on the wire.
#[test]
fn a_fixed_that_overflows_its_digits_is_refused() {
    let idl = "@final struct Money { fixed<5,2> amount; };";
    for value in ["1234.5", "1.234", "12x.45"] {
        let error = refusal(idl, "Money", &json!({ "amount": value }));
        assert!(
            matches!(&error, ProbeError::Mismatch { path, .. } if path == "Money.amount"),
            "{value}: {error}"
        );
    }
}

/// A JSON number would round-trip a `fixed<31, 2>` through a double,
/// which is the case the type exists for. Refused rather than accepted
/// with the digits a double happens to keep.
#[test]
fn a_fixed_given_a_json_number_is_refused() {
    let idl = "@final struct Money { fixed<5,2> amount; };";
    let error = refusal(idl, "Money", &json!({ "amount": 123.45 }));
    assert!(
        matches!(&error, ProbeError::Mismatch { path, .. } if path == "Money.amount"),
        "{error}"
    );
}

// -- the constructs inside the collections that carry them -------------

/// A `sequence<wstring>` exercises the delimiter decision as well: a
/// `wstring` is not a primitive under §7.2.2.2, so §7.4.3.5.3 rule (12)
/// delimits the sequence under XCDR2.
#[test]
fn a_sequence_of_wstrings_is_delimited_and_each_element_counts_bytes() {
    let idl = "@final struct WSeq { sequence<wstring> ws; };";
    let values = json!({ "ws": ["ab", "c"] });
    // The delimiter covers the element count and both elements:
    // 4 + (4 + 4) + (4 + 2) = 18 bytes.
    assert_eq!(
        bytes(idl, "WSeq", &values, Encoding::PlainCdr2),
        "12000000020000000400000061006200020000006300"
    );
}

/// A typedef is transparent, so a `fixed` behind one writes the same
/// three bytes.
#[test]
fn a_typedef_of_a_fixed_writes_what_the_target_writes() {
    let direct = "@final struct Money { fixed<5,2> sum; };";
    let aliased = "\
typedef fixed<5,2> Amount;
@final struct Money { Amount sum; };";
    let values = json!({ "sum": "123.45" });
    assert_eq!(
        bytes(aliased, "Money", &values, Encoding::PlainCdr2),
        bytes(direct, "Money", &values, Encoding::PlainCdr2)
    );
}

// -- the two rows that had no foreign measurement at all ---------------

/// P5's collection half. A bitset element is **not** primitive for
/// rules (10) and (12), so a collection of them is delimited under
/// XCDR2.
///
/// The argument comes before the bytes. §7.4.3.5.1's match criteria
/// have no bitset entry, so a bitset satisfies neither
/// `PRIMITIVE_TYPE` nor the primitive-element criterion `PARRAY_TYPE`
/// and `PSEQUENCE_TYPE` require, and a criterion that cannot be
/// satisfied leaves only the delimited branch. Treating a bitset like a
/// bitmask instead would not follow a rule — rule (6) makes a bitmask
/// an integer outright, and Cyclone and Fast DDS disagree about a
/// collection of them (G13).
///
/// Measured 2026-08-10 against Fast DDS 3.6.1, the one implementation
/// whose front end parses the keyword:
/// `crates/ast-layout/tests/fixtures/unspecified_constructs.md` §1.
#[test]
fn a_bitset_in_a_collection_is_delimited_the_way_fast_dds_delimits_it() {
    let seq = "\
bitset Packed { bitfield<3> low; bitfield<5> high; };
@final struct Sample { sequence<Packed> many; long tail; };";
    let seq_values = json!({
        "many": [{ "low": 5, "high": 17 }, { "low": 0, "high": 31 }],
        "tail": -1
    });

    // DHEADER of 6 — the four-byte count and the two one-byte anchors —
    // then two padding bytes before the 4-aligned `long`. Fast DDS
    // leaves those two uninitialised (A4); everything else is
    // byte-identical.
    assert_eq!(
        bytes(seq, "Sample", &seq_values, Encoding::PlainCdr2),
        "06000000020000008df80000ffffffff"
    );
    // Version 1 has no delimiter for any collection, so the two agree
    // exactly there.
    assert_eq!(
        bytes(seq, "Sample", &seq_values, Encoding::PlainCdr1),
        "020000008df80000ffffffff"
    );

    let array = "\
bitset Packed { bitfield<3> low; bitfield<5> high; };
@final struct Sample { Packed items[2]; octet tail; };";
    let array_values = json!({
        "items": [{ "low": 5, "high": 17 }, { "low": 7, "high": 0 }],
        "tail": 171
    });

    // `02000000` is the array's DHEADER: two one-byte anchors, no count.
    // Byte for byte with Fast DDS in both versions.
    assert_eq!(
        bytes(array, "Sample", &array_values, Encoding::PlainCdr2),
        "020000008d07ab"
    );
    assert_eq!(
        bytes(array, "Sample", &array_values, Encoding::PlainCdr1),
        "8d07ab"
    );
}

/// P5's key half. In a key holder a bitset is its anchor and nothing
/// else — no delimiter, no per-bitfield framing — because §7.6.8 Step 4
/// asks for the PLAIN_CDR2 big-endian serialization of the holder and
/// the holder's serialization of a bitset is the integer P1 settles.
///
/// Both hashes are Fast DDS 3.6.1's, measured 2026-08-10. The second is
/// the first foreign check this project's MD5 branch has ever had: a
/// `string<40>` puts the holder's maximum serialized size over sixteen
/// bytes, so §9.6.4.8 hashes rather than zero-extends.
#[test]
fn a_bitset_key_member_is_its_anchor_and_the_hash_fast_dds_computes() {
    let short = "\
bitset Packed { bitfield<3> low; bitfield<5> high; };
@final struct KeyS { @key Packed k; octet tail; };";
    let short_values = json!({ "k": { "low": 5, "high": 17 }, "tail": 171 });
    assert_eq!(
        bytes(short, "KeyS", &short_values, Encoding::PlainCdr2),
        "8dab"
    );
    assert_eq!(
        key_hash(short, "KeyS", &short_values),
        "8d000000000000000000000000000000"
    );

    let long = "\
bitset Wide { bitfield<12> a; bitfield<12> b; };
@final struct KeyW { @key Wide k; @key string<40> s; octet tail; };";
    let long_values = json!({ "k": { "a": 273, "b": 546 }, "s": "abc", "tail": 171 });
    // A 24-bit bitset takes a four-byte anchor — the 1/2/4/8 band rule
    // at a width that is not itself a power of two.
    assert_eq!(
        bytes(long, "KeyW", &long_values, Encoding::PlainCdr2),
        "112122000400000061626300ab"
    );
    assert_eq!(
        key_hash(long, "KeyW", &long_values),
        "0168f0070ddb81e794da3da67abebd41"
    );
}

/// The `map` length contradiction, with an implementation on each side.
///
/// `the_two_readings_of_a_delimited_maps_length_differ_by_four_bytes`
/// above pins the two shapes against the clause. This pins them against
/// bytes: `MapLengthPolicy::SequenceEquivalent` is Fast DDS 3.6.1 and
/// `Rule15Omits` is OpenDDS 3.34.0, for one IDL and one sample,
/// measured 2026-08-10. Neither stream is malformed and neither reader
/// can detect the other's shape, which is why this is OMG issue
/// **DDSXTY14-72** and not a vendor defect —
/// `docs/omg-xtypes-issues.md` §A1.
#[test]
fn each_reading_of_a_delimited_maps_length_is_the_bytes_of_one_implementation() {
    let idl = "\
@final struct Cell { long a; long b; };
@final struct Sample { map<long, Cell> m; octet tail; };";
    let values = json!({ "m": [[7, { "a": 1, "b": 2 }]], "tail": 171 });
    let ast = model_from_idl(idl, "probe.idl").expect("the model");

    let fast_dds = ProbeRequest::new("Sample", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        hex(&encode(&ast, &fast_dds, &values).expect("bytes")),
        "1000000001000000070000000100000002000000ab",
        "Fast DDS 3.6.1 writes the DHEADER and the entry count"
    );

    let open_dds = ProbeRequest {
        map_length: MapLengthPolicy::Rule15Omits,
        ..fast_dds.clone()
    };
    assert_eq!(
        hex(&encode(&ast, &open_dds, &values).expect("bytes")),
        "0c000000070000000100000002000000ab",
        "OpenDDS 3.34.0 writes the DHEADER and no count"
    );

    // Rule (16) has no DHEADER, so the two agree exactly under version 1
    // and the divergence is confined to XCDR2.
    let version_one = ProbeRequest::new("Sample", Encoding::PlainCdr1, Endianness::Little);
    assert_eq!(
        hex(&encode(&ast, &version_one, &values).expect("bytes")),
        "01000000070000000100000002000000ab"
    );
}
