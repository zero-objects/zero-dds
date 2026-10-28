// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wstring`: the eight cells of C1 × C2 × C3, the four points of C4,
//! and the BMP refusal.
//!
//! DDS-XTypes 1.3 §7.4.1.1.2 fixes one cell — an octet count, "twice
//! the number of characters in the string", no terminator, no
//! byte-order mark — and eight of the nine measured implementations
//! write something else in at least one axis. The axes are separate
//! because the implementations answer them separately, and this file is
//! the byte-level proof that all eight cells and all four orders are
//! reachable rather than five names and a default.
//!
//! # Where the vendor octets come from
//!
//! `tests/interop-results/sources/vendor-corpus.tsv`,
//! `corba-corpus.tsv` and `ros-corpus.tsv`, recorded from the vendors'
//! own generated encoders on the Linux test host. The constants below
//! are the `wstring` member's own octets out of those rows: the
//! recorded payload carries a trailing `ffffffff` tail marker and
//! whatever alignment padding precedes it, and both are the *sample's*
//! and not the member's.
//!
//! Two `wstring` cases carry a value:
//!
//! - `wstring-bounded-utf8`, `"ሴäZ"` — U+1234, U+00E4, U+005A.
//! - `wstring-unbounded-utf8`, `"äöü ሴ Z"` — seven characters, one of
//!   them outside Latin-1 and none outside the BMP.
//!
//! # What each mutation makes red
//!
//! | mutation in `Writer::write_wstring` | red |
//! |---|---|
//! | C1's two arms swapped — an octet count where a character count belongs | `an_octet_count_and_a_character_count_are_different_numbers`, and every table row of a `Characters` cell |
//! | C2's `units.push(0)` removed — the terminator dropped | `a_terminator_is_written_and_counted`, and `rti_xcdr1_is_a_character_count_over_four_octet_units_with_a_terminator` |
//! | C3 read as two octets where the cell says four | `the_unit_width_reaches_both_the_length_and_the_octets`, and `tao_is_an_octet_count_over_four_octet_units` |
//! | the BMP refusal removed, Fast CDR's truncation emitted instead | `a_character_count_over_two_octet_units_refuses_a_code_point_outside_the_bmp` |
//! | C4's mark not counted in the length | `omniorb_writes_a_counted_byte_order_mark` |
//! | C4's units written in the stream's order under `HostUnswapped` | `cyclone_big_endian_writes_little_endian_units` |
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire::{
    Endianness, Reader, WireError, WirePosition, Writer, WstringLengthCounts, WstringLengthPolicy,
    WstringTerminator, WstringUnitOrder, WstringUnitWidth,
};

/// `"ሴäZ"`, the value of the recorded `wstring-bounded-utf8` case.
const BOUNDED: &str = "\u{1234}\u{00e4}Z";

/// `"äöü ሴ Z"`, the value of the recorded `wstring-unbounded-utf8`
/// case: seven characters, all inside the BMP.
const UNBOUNDED: &str = "\u{00e4}\u{00f6}\u{00fc} \u{1234} Z";

/// `"a𝄞b"` — U+1D11E is outside the BMP and takes the surrogate pair
/// `D834 DD1E`.
const ASTRAL: &str = "a\u{1d11e}b";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The octets one cell puts on the wire for `text`, in `order` and in
/// `endianness`, at the member position.
fn wrote(
    text: &str,
    policy: WstringLengthPolicy,
    order: WstringUnitOrder,
    endianness: Endianness,
) -> String {
    let mut w = Writer::new(endianness);
    w.write_wstring(text, policy, order, WirePosition::Member)
        .expect("the cell writes this value");
    hex(w.as_bytes())
}

/// The same in the stream's own order, which is the default point of
/// C4 and the only one eight of the nine implementations write.
fn wrote_stream(text: &str, policy: WstringLengthPolicy) -> String {
    wrote(text, policy, WstringUnitOrder::Stream, Endianness::Little)
}

const OCTETS: WstringLengthCounts = WstringLengthCounts::Octets;
const CHARS: WstringLengthCounts = WstringLengthCounts::Characters;
const ABSENT: WstringTerminator = WstringTerminator::Absent;
const PRESENT: WstringTerminator = WstringTerminator::Present;
const TWO: WstringUnitWidth = WstringUnitWidth::Two;
const FOUR: WstringUnitWidth = WstringUnitWidth::Four;

// -- the eight cells -----------------------------------------------------

/// Every cell of C1 × C2 × C3 writes its own octets, and no two of the
/// eight agree on this value.
///
/// The table is the whole product and not the measured five: a cell
/// left out of a completeness check is a cell that can stop working
/// without anything going red, which is how the ROS form came to be
/// unreachable while the names were the values.
#[test]
fn every_cell_of_the_product_puts_its_own_octets_on_the_wire() {
    // `"ab"`, in the stream's order, little endian.
    let cells: [(WstringLengthPolicy, &str, &str); 8] = [
        // C1 octets, C2 absent, C3 two — the specification's own cell.
        (
            WstringLengthPolicy::new(OCTETS, ABSENT, TWO),
            "specification",
            "0400000061006200",
        ),
        (
            WstringLengthPolicy::new(OCTETS, ABSENT, FOUR),
            "specification-wide-units",
            "080000006100000062000000",
        ),
        (
            WstringLengthPolicy::new(OCTETS, PRESENT, TWO),
            "octets/terminated/2",
            "06000000610062000000",
        ),
        (
            WstringLengthPolicy::new(OCTETS, PRESENT, FOUR),
            "octets/terminated/4",
            "0c000000610000006200000000000000",
        ),
        (
            WstringLengthPolicy::new(CHARS, ABSENT, TWO),
            "character-count",
            "0200000061006200",
        ),
        (
            WstringLengthPolicy::new(CHARS, ABSENT, FOUR),
            "ros-type-support",
            "020000006100000062000000",
        ),
        (
            WstringLengthPolicy::new(CHARS, PRESENT, TWO),
            "characters/terminated/2",
            "03000000610062000000",
        ),
        (
            WstringLengthPolicy::new(CHARS, PRESENT, FOUR),
            "character-count-terminated",
            "03000000610000006200000000000000",
        ),
    ];
    assert_eq!(cells.len(), WstringLengthPolicy::ALL_CELLS.len());
    for (policy, name, expected) in cells {
        assert_eq!(policy.as_str(), name, "the cell's own name");
        assert_eq!(wrote_stream("ab", policy), expected, "cell {name}");
    }
    // Eight cells, eight distinct byte strings for one value. A cell
    // that collapsed onto another would be a setting with no effect.
    let mut octets: Vec<String> = cells
        .iter()
        .map(|(policy, _, _)| wrote_stream("ab", *policy))
        .collect();
    octets.sort_unstable();
    octets.dedup();
    assert_eq!(octets.len(), 8, "eight cells, eight byte strings");
}

/// Every cell round-trips, in both stream orders and under all four
/// points of C4.
///
/// Thirty-two cells × two orders, and the value is the recorded
/// `wstring-unbounded-utf8` one so that a cell losing a character shows
/// up as a shorter string rather than as a length nobody checks.
#[test]
fn every_cell_round_trips_under_every_unit_order() {
    for policy in WstringLengthPolicy::ALL_CELLS {
        for order in [
            WstringUnitOrder::Stream,
            WstringUnitOrder::BigEndianAlways,
            WstringUnitOrder::ByteOrderMark,
            WstringUnitOrder::HostUnswapped,
        ] {
            for endianness in [Endianness::Little, Endianness::Big] {
                let mut w = Writer::new(endianness);
                w.write_wstring(UNBOUNDED, policy, order, WirePosition::Member)
                    .expect("every cell writes BMP text");
                let bytes = w.finish();
                let mut r = Reader::new(&bytes, endianness);
                let back = r
                    .read_wstring(policy, order, WirePosition::Member)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} / {} / {endianness:?} reads back: {error:?}",
                            policy.as_str(),
                            order.spelling()
                        )
                    });
                assert_eq!(back, UNBOUNDED, "{} / {order:?}", policy.as_str());
                assert_eq!(r.remaining(), 0, "the whole run was consumed");
            }
        }
    }
}

/// The member and the collection element are measured to write the same
/// octets on all four axes, in all nine implementations.
///
/// A checked non-existence and not an assumption: the day an
/// implementation parts them, this is what goes red and says so.
#[test]
fn the_two_positions_still_agree() {
    for policy in WstringLengthPolicy::ALL_CELLS {
        for order in [
            WstringUnitOrder::Stream,
            WstringUnitOrder::BigEndianAlways,
            WstringUnitOrder::ByteOrderMark,
            WstringUnitOrder::HostUnswapped,
        ] {
            let mut member = Writer::new(Endianness::Little);
            member
                .write_wstring(UNBOUNDED, policy, order, WirePosition::Member)
                .expect("a member");
            let mut element = Writer::new(Endianness::Little);
            element
                .write_wstring(UNBOUNDED, policy, order, WirePosition::Element)
                .expect("an element");
            assert_eq!(
                hex(member.as_bytes()),
                hex(element.as_bytes()),
                "{} / {order:?}",
                policy.as_str()
            );
        }
    }
}

// -- the vendor anchors --------------------------------------------------

/// Cyclone, OpenDDS and RTI's XCDR2 write [`SPECIFICATION`], and the
/// octets are theirs.
///
/// `wstring-unbounded-utf8` little endian, cyclone/opendds/rti-xcdr2:
/// `0e000000 e400f600fc00200034122000 5a00`. Fourteen octets for seven
/// two-octet units.
///
/// [`SPECIFICATION`]: WstringLengthPolicy::SPECIFICATION
#[test]
fn cyclone_and_opendds_are_an_octet_count_over_two_octet_units() {
    assert_eq!(
        wrote_stream(UNBOUNDED, WstringLengthPolicy::SPECIFICATION),
        "0e000000e400f600fc002000341220005a00"
    );
    assert_eq!(
        wrote_stream(BOUNDED, WstringLengthPolicy::SPECIFICATION),
        "060000003412e4005a00"
    );
}

/// Fast DDS writes a character count over two-octet units.
///
/// `wstring-unbounded-utf8` little endian, fastdds:
/// `07000000 e400f600fc00200034122000 5a00` — seven, for seven
/// characters, over the same two-octet units Cyclone counted fourteen
/// octets of.
#[test]
fn fastdds_is_a_character_count_over_two_octet_units() {
    assert_eq!(
        wrote_stream(UNBOUNDED, WstringLengthPolicy::CHARACTER_COUNT),
        "07000000e400f600fc002000341220005a00"
    );
    assert_eq!(
        wrote_stream(BOUNDED, WstringLengthPolicy::CHARACTER_COUNT),
        "030000003412e4005a00"
    );
}

/// RTI Connext 7.7 under XCDR1 writes a character count that includes a
/// terminating NUL, over four-octet units.
///
/// `wstring-unbounded-utf8` little endian, rti xcdr1:
/// `08000000 e4000000 f6000000 fc000000 20000000 34120000 20000000
/// 5a000000 00000000`. Eight, for seven characters and the NUL.
#[test]
fn rti_xcdr1_is_a_character_count_over_four_octet_units_with_a_terminator() {
    assert_eq!(
        wrote_stream(UNBOUNDED, WstringLengthPolicy::CHARACTER_COUNT_TERMINATED),
        "08000000e4000000f6000000fc0000002000000034120000200000005a00000000000000"
    );
    assert_eq!(
        wrote_stream(BOUNDED, WstringLengthPolicy::CHARACTER_COUNT_TERMINATED),
        "0400000034120000e40000005a00000000000000"
    );
}

/// TAO 2.5.24 writes CORBA's GIOP 1.2 `wstring` over a four-octet
/// transmission code set: an octet count, no terminator, four-octet
/// units.
///
/// `wstring-bounded-utf8` little endian, tao:
/// `0c000000 34120000 e4000000 5a000000`. Twelve octets for three
/// four-octet units — the same three characters Fast DDS counted as
/// three and OpenDDS as six.
#[test]
fn tao_is_an_octet_count_over_four_octet_units() {
    assert_eq!(
        wrote_stream(BOUNDED, WstringLengthPolicy::SPECIFICATION_WIDE_UNITS),
        "0c00000034120000e40000005a000000"
    );
}

/// Both ROS 2 type supports write a character count over four-octet
/// units — the cell the old enumeration could not reach.
///
/// `wstring-unbounded-utf8` little endian, ros-cyclone and
/// ros-fastrtps, byte-identical:
/// `07000000 e4000000 f6000000 fc000000 20000000 34120000 20000000
/// 5a000000`.
#[test]
fn both_ros_type_supports_are_a_character_count_over_four_octet_units() {
    assert_eq!(
        wrote_stream(UNBOUNDED, WstringLengthPolicy::ROS_TYPE_SUPPORT),
        "07000000e4000000f6000000fc0000002000000034120000200000005a000000"
    );
    assert_eq!(
        wrote_stream(BOUNDED, WstringLengthPolicy::ROS_TYPE_SUPPORT),
        "0300000034120000e40000005a000000"
    );
}

// -- the fourth axis ----------------------------------------------------

/// omniORB 4.3.3 writes a byte-order mark, counted in the length, then
/// the units, all little-endian.
///
/// `wstring-bounded-utf8`, omniorb, **both** stream orders:
/// `08000000 fffe 3412 e400 5a00` little endian and
/// `00000008 fffe 3412 e400 5a00` big endian. Eight octets over four
/// units — the mark is one of them — and the mark's own octets are
/// `fffe`, which is `U+FEFF` written little-endian.
#[test]
fn omniorb_writes_a_counted_byte_order_mark() {
    assert_eq!(
        wrote(
            BOUNDED,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::ByteOrderMark,
            Endianness::Little,
        ),
        "08000000fffe3412e4005a00"
    );
    assert_eq!(
        wrote(
            BOUNDED,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::ByteOrderMark,
            Endianness::Big,
        ),
        "00000008fffe3412e4005a00"
    );
}

/// An **empty** `wstring` carries no mark.
///
/// `wstring-unbounded-empty`, every implementation including omniORB:
/// `00000000`. A mark on an empty string would be a two-octet string
/// that decodes to nothing, and no peer writes it.
#[test]
fn an_empty_wstring_carries_no_byte_order_mark() {
    for order in [
        WstringUnitOrder::Stream,
        WstringUnitOrder::BigEndianAlways,
        WstringUnitOrder::ByteOrderMark,
        WstringUnitOrder::HostUnswapped,
    ] {
        assert_eq!(
            wrote(
                "",
                WstringLengthPolicy::SPECIFICATION,
                order,
                Endianness::Little,
            ),
            "00000000",
            "{order:?}"
        );
    }
    // RTI's XCDR1 empty string is its own terminator and nothing else:
    // `wstring-unbounded-empty`, rti xcdr1 le, `01000000 00000000`.
    assert_eq!(
        wrote_stream("", WstringLengthPolicy::CHARACTER_COUNT_TERMINATED),
        "0100000000000000"
    );
}

/// Cyclone swaps the length word for a big-endian stream and does not
/// swap the content.
///
/// `wstring-unbounded-utf8` **big endian**, cyclone:
/// `0000000e e400f600fc00200034122000 5a00` — a big-endian length over
/// little-endian units. Read as the big-endian stream the encapsulation
/// identifier says it is, that is seven different characters, well
/// formed and the right length, with nothing in the frame saying so.
/// The point exists because refusing to model it means nobody can
/// exchange a `wstring` with Cyclone over big endian at all.
#[test]
fn cyclone_big_endian_writes_little_endian_units() {
    assert_eq!(
        wrote(
            UNBOUNDED,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::HostUnswapped,
            Endianness::Big,
        ),
        "0000000ee400f600fc002000341220005a00"
    );
    // `rosidl_typesupport_fastrtps` does the same at four-octet units:
    // `wstring-unbounded-utf8` big endian, ros-fastrtps,
    // `00000007 e4000000 …`. Two implementations, which is what makes
    // this a point on the axis rather than one product's defect.
    assert_eq!(
        wrote(
            UNBOUNDED,
            WstringLengthPolicy::ROS_TYPE_SUPPORT,
            WstringUnitOrder::HostUnswapped,
            Endianness::Big,
        ),
        "00000007e4000000f6000000fc0000002000000034120000200000005a000000"
    );
    // In little endian it is byte for byte `Stream`, which is why the
    // divergence took a big-endian run to find.
    assert_eq!(
        wrote(
            UNBOUNDED,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::HostUnswapped,
            Endianness::Little,
        ),
        wrote_stream(UNBOUNDED, WstringLengthPolicy::SPECIFICATION)
    );
}

/// JacORB decodes the units big-endian whatever the stream says.
///
/// `wstring-bounded-utf8`, jacorb, big endian:
/// `00000006 1234 00e4 005a`. JacORB's write direction cannot tell this
/// apart from [`WstringUnitOrder::Stream`] —
/// `org.jacorb.orb.CDROutputStream` has no `setLittleEndian`, so every
/// JacORB byte string recorded here is big-endian and the two points
/// coincide in it. What separates them is a **little-endian** stream,
/// which JacORB never writes and this axis therefore has to state
/// rather than measure on the write side.
#[test]
fn jacorb_reads_big_endian_units_out_of_either_stream() {
    assert_eq!(
        wrote(
            BOUNDED,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::BigEndianAlways,
            Endianness::Big,
        ),
        "00000006123400e4005a"
    );
    // The half no vendor run separates: in a little-endian stream the
    // length is little-endian and the units stay big-endian.
    assert_eq!(
        wrote(
            BOUNDED,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::BigEndianAlways,
            Endianness::Little,
        ),
        "06000000123400e4005a"
    );
}

// -- the axes, one mutation each ----------------------------------------

/// C1: the length counts octets or code units, and for this value the
/// two are different numbers.
///
/// Swapping the two arms of C1 in the writer makes this red. Fourteen
/// against seven for the same seven characters is Cyclone against Fast
/// DDS, byte for byte.
#[test]
fn an_octet_count_and_a_character_count_are_different_numbers() {
    let octets = wrote_stream(UNBOUNDED, WstringLengthPolicy::SPECIFICATION);
    let characters = wrote_stream(UNBOUNDED, WstringLengthPolicy::CHARACTER_COUNT);
    assert_eq!(&octets[..8], "0e000000", "fourteen octets");
    assert_eq!(&characters[..8], "07000000", "seven characters");
    // The content is the same; only the count parts.
    assert_eq!(octets[8..], characters[8..]);
}

/// C2: the terminator is written **and** counted, or neither.
///
/// Dropping `units.push(0)` makes this red twice: the count falls by
/// one and the trailing NUL unit goes missing. The two cells here
/// differ in C2 alone.
#[test]
fn a_terminator_is_written_and_counted() {
    let without = wrote_stream(UNBOUNDED, WstringLengthPolicy::ROS_TYPE_SUPPORT);
    let with = wrote_stream(UNBOUNDED, WstringLengthPolicy::CHARACTER_COUNT_TERMINATED);
    assert_eq!(&without[..8], "07000000", "seven characters");
    assert_eq!(&with[..8], "08000000", "seven characters and the NUL");
    // The content is the same run with one NUL unit behind it, and the
    // count went up by exactly that unit.
    assert_eq!(with[8..], format!("{}00000000", &without[8..]));
    assert_eq!(
        with.len(),
        without.len() + 8,
        "one four-octet unit longer on the wire"
    );
}

/// C3: the unit width reaches both the length field and the octets.
///
/// Reading C3 as two where the cell says four makes this red in the
/// length (eight against fourteen) and in the content (the units are
/// half as wide). The two cells here differ in C3 alone.
#[test]
fn the_unit_width_reaches_both_the_length_and_the_octets() {
    let narrow = wrote_stream("ab", WstringLengthPolicy::SPECIFICATION);
    let wide = wrote_stream("ab", WstringLengthPolicy::SPECIFICATION_WIDE_UNITS);
    assert_eq!(narrow, "0400000061006200");
    assert_eq!(wide, "080000006100000062000000");
    assert_eq!(
        WstringLengthPolicy::SPECIFICATION.character_width(WirePosition::Member),
        2
    );
    assert_eq!(
        WstringLengthPolicy::SPECIFICATION_WIDE_UNITS.character_width(WirePosition::Member),
        4
    );
}

// -- the BMP refusal ----------------------------------------------------

/// A code point outside the BMP under a character count over two-octet
/// units is refused, not truncated.
///
/// Fast CDR 2.3.5 `src/cpp/Cdr.cpp:884` counts `wcslen` characters and
/// then writes each four-octet `wchar_t` as two octets, so `U+1D11E`
/// leaves as `d11e` — `U+D11E`, a Hangul syllable and a perfectly
/// well-formed code point. Nothing downstream can see that it is the
/// wrong one. Reproducing a vendor's length convention is a policy;
/// reproducing a data loss no reader can see is not.
#[test]
fn a_character_count_over_two_octet_units_refuses_a_code_point_outside_the_bmp() {
    let mut w = Writer::new(Endianness::Little);
    let error = w
        .write_wstring(
            ASTRAL,
            WstringLengthPolicy::CHARACTER_COUNT,
            WstringUnitOrder::Stream,
            WirePosition::Member,
        )
        .expect_err("the cell has no way to carry U+1D11E");
    match error {
        WireError::OutOfRange { expected, .. } => {
            assert!(expected.contains("U+1D11E"), "{expected}");
            assert!(expected.contains("BMP"), "{expected}");
        }
        other => panic!("expected an OutOfRange, got {other:?}"),
    }
    assert!(w.as_bytes().is_empty(), "nothing went on the wire");
    // The other cell of C1 = characters at the narrow width: the
    // refusal is on the pair (characters, two octets) and not on a
    // named form.
    let mut w = Writer::new(Endianness::Little);
    assert!(
        w.write_wstring(
            ASTRAL,
            WstringLengthPolicy::new(CHARS, PRESENT, TWO),
            WstringUnitOrder::Stream,
            WirePosition::Member,
        )
        .is_err(),
        "characters/terminated/2 has the same narrow unit"
    );
}

/// A character count over **four**-octet units carries the surrogate
/// pair intact and is not refused.
///
/// The refusal is on the narrow unit and not on the count. RTI's XCDR1
/// `"𝄞"` is `03000000 34d80000 1edd0000 00000000` — a count of three
/// over two surrogate units and a NUL — measured 2026-08-09.
#[test]
fn a_character_count_over_four_octet_units_carries_the_surrogate_pair() {
    assert_eq!(
        wrote_stream("\u{1d11e}", WstringLengthPolicy::CHARACTER_COUNT_TERMINATED),
        "0300000034d800001edd000000000000"
    );
    assert_eq!(
        wrote_stream("\u{1d11e}", WstringLengthPolicy::ROS_TYPE_SUPPORT),
        "0200000034d800001edd0000"
    );
    // And Cyclone's own octet count, measured the same day:
    // `04000000 34d81edd`.
    assert_eq!(
        wrote_stream("\u{1d11e}", WstringLengthPolicy::SPECIFICATION),
        "0400000034d81edd"
    );
}

// -- the read side ------------------------------------------------------

/// A length the configured cell cannot have produced is refused, and
/// the error names the cell it does fit.
///
/// Fast DDS's odd character counts are odd octet counts under
/// [`WstringLengthPolicy::SPECIFICATION`]. Rounding one down would
/// leave the remainder in the stream and put every later member at the
/// wrong offset.
#[test]
fn an_octet_count_the_unit_width_does_not_divide_is_refused() {
    // Fast DDS's `"ሴäZ"`: a count of three, which is not an octet count
    // over two-octet units.
    let bytes = [0x03, 0x00, 0x00, 0x00, 0x12, 0x34, 0xe4, 0x00, 0x5a, 0x00];
    let mut r = Reader::new(&bytes, Endianness::Little);
    let error = r
        .read_wstring(
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::Stream,
            WirePosition::Member,
        )
        .expect_err("three is not an even octet count");
    match error {
        WireError::WrongFraming {
            expected,
            found,
            fits,
            ..
        } => {
            assert!(expected.contains("specification"), "{expected}");
            assert!(found.contains("character-count"), "{found}");
            assert!(
                fits.is_some_and(|flag| flag.starts_with("--wstring-length ")),
                "the flag that fixes it, not just a complaint"
            );
        }
        other => panic!("expected a WrongFraming, got {other:?}"),
    }
}

/// Under C2 the count includes the NUL, so a count of zero is a frame
/// that does not hold its own terminator, and a last unit that is not a
/// NUL is a frame that disagrees with itself.
#[test]
fn a_terminated_cell_refuses_a_frame_without_its_terminator() {
    let policy = WstringLengthPolicy::CHARACTER_COUNT_TERMINATED;
    // A count of zero.
    let zero = [0x00, 0x00, 0x00, 0x00];
    let mut r = Reader::new(&zero, Endianness::Little);
    assert!(matches!(
        r.read_wstring(policy, WstringUnitOrder::Stream, WirePosition::Member),
        Err(WireError::WrongFraming { .. })
    ));
    // A last unit that is not a NUL: `"ab"` counted as two, no
    // terminator behind it.
    let unterminated = [
        0x02, 0x00, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00, 0x62, 0x00, 0x00, 0x00,
    ];
    let mut r = Reader::new(&unterminated, Endianness::Little);
    match r.read_wstring(policy, WstringUnitOrder::Stream, WirePosition::Member) {
        Err(WireError::OutOfRange { expected, .. }) => {
            assert!(expected.contains("NUL"), "{expected}");
        }
        other => panic!("expected an OutOfRange about the NUL, got {other:?}"),
    }
    // A NUL inside the text, which every C peer stops at.
    let inner = [
        0x03, 0x00, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    let mut r = Reader::new(&inner, Endianness::Little);
    match r.read_wstring(policy, WstringUnitOrder::Stream, WirePosition::Member) {
        Err(WireError::OutOfRange { expected, .. }) => {
            assert!(expected.contains("C peer"), "{expected}");
        }
        other => panic!("expected an OutOfRange about the inner NUL, got {other:?}"),
    }
    // A cell **without** a terminator is entitled to carry U+0000 as a
    // character: the same three units under the ROS cell read back as a
    // one-character string.
    let mut r = Reader::new(&inner, Endianness::Little);
    let back = r
        .read_wstring(
            WstringLengthPolicy::ROS_TYPE_SUPPORT,
            WstringUnitOrder::Stream,
            WirePosition::Member,
        )
        .expect("an unterminated cell may carry U+0000");
    assert_eq!(back, "a\u{0}\u{0}");
}

/// A four-octet character above `0xFFFF` is refused rather than
/// truncated: the wide form zero-extends a code *unit* and does not
/// carry a code point.
#[test]
fn a_wide_unit_that_is_not_a_code_unit_is_refused() {
    let bytes = [0x01, 0x00, 0x00, 0x00, 0x1e, 0xd1, 0x01, 0x00];
    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_wstring(
        WstringLengthPolicy::ROS_TYPE_SUPPORT,
        WstringUnitOrder::Stream,
        WirePosition::Member,
    ) {
        Err(WireError::OutOfRange { expected, .. }) => {
            assert!(expected.contains("one UTF-16 code unit"), "{expected}");
        }
        other => panic!("expected an OutOfRange, got {other:?}"),
    }
}

/// A length past the end of the stream is an `ImpossibleLength` and not
/// an allocation.
#[test]
fn a_length_the_stream_cannot_hold_is_refused_before_allocating() {
    let bytes = [0xff, 0xff, 0xff, 0x7f, 0x61, 0x00];
    let mut r = Reader::new(&bytes, Endianness::Little);
    assert!(matches!(
        r.read_wstring(
            WstringLengthPolicy::CHARACTER_COUNT,
            WstringUnitOrder::Stream,
            WirePosition::Member,
        ),
        Err(WireError::ImpossibleLength { .. })
    ));
}

/// An unpaired surrogate is refused rather than replaced: U+FFFD in its
/// place would be a character the sender never had.
#[test]
fn an_unpaired_surrogate_is_refused_and_not_replaced() {
    let bytes = [0x02, 0x00, 0x00, 0x00, 0x34, 0xd8, 0x61, 0x00];
    let mut r = Reader::new(&bytes, Endianness::Little);
    match r.read_wstring(
        WstringLengthPolicy::SPECIFICATION,
        WstringUnitOrder::Stream,
        WirePosition::Member,
    ) {
        Err(WireError::OutOfRange { expected, .. }) => {
            assert!(expected.contains("UTF-16"), "{expected}");
        }
        other => panic!("expected an OutOfRange, got {other:?}"),
    }
}

/// Under [`WstringUnitOrder::ByteOrderMark`] the mark decides, and a
/// frame with no mark is CORBA's big-endian default.
///
/// The third case is the one that matters: `FFFE` means every unit
/// behind it is byte-swapped, which is what a conforming big-endian
/// omniORB peer produces and what a reader that ignored the mark would
/// return as different characters, well formed and the right length.
#[test]
fn the_byte_order_mark_decides_on_read() {
    let policy = WstringLengthPolicy::SPECIFICATION;
    let order = WstringUnitOrder::ByteOrderMark;
    // `FEFF` the right way up: the units are as read, little-endian.
    let upright = [0x06, 0x00, 0x00, 0x00, 0xff, 0xfe, 0x61, 0x00, 0x62, 0x00];
    let mut r = Reader::new(&upright, Endianness::Little);
    assert_eq!(
        r.read_wstring(policy, order, WirePosition::Member)
            .expect("an upright mark"),
        "ab"
    );
    // `FFFE`: the mark is byte-swapped, so every unit behind it is too.
    let swapped = [0x06, 0x00, 0x00, 0x00, 0xfe, 0xff, 0x00, 0x61, 0x00, 0x62];
    let mut r = Reader::new(&swapped, Endianness::Little);
    assert_eq!(
        r.read_wstring(policy, order, WirePosition::Member)
            .expect("a swapped mark"),
        "ab"
    );
    // No mark at all: CORBA's rule for a GIOP 1.2 `wstring` without one
    // is UTF-16 big-endian, which is what makes this point read a
    // JacORB peer as well as an omniORB one.
    let bare = [0x04, 0x00, 0x00, 0x00, 0x00, 0x61, 0x00, 0x62];
    let mut r = Reader::new(&bare, Endianness::Little);
    assert_eq!(
        r.read_wstring(policy, order, WirePosition::Member)
            .expect("no mark, so big-endian"),
        "ab"
    );
}

// -- the bound ----------------------------------------------------------

/// A `wstring<N>` bound counts UTF-16 code units, and a value that
/// overruns it is refused rather than truncated.
#[test]
fn a_bounded_wstring_counts_code_units_and_refuses_an_overrun() {
    let policy = WstringLengthPolicy::SPECIFICATION;
    let order = WstringUnitOrder::Stream;
    // `"ሴäZ"` is three code units and three characters, and seven
    // octets of UTF-8. A bound of three admits it; an octet bound would
    // have refused it.
    let mut w = Writer::new(Endianness::Little);
    w.write_wstring_bounded(BOUNDED, 3, policy, order, WirePosition::Member)
        .expect("three code units fit a bound of three");
    assert_eq!(hex(w.as_bytes()), "060000003412e4005a00");
    // `"a𝄞b"` is four code units for three characters, so a bound of
    // three refuses it. Counting Unicode scalar values would have
    // accepted a value no peer's buffer holds.
    let mut w = Writer::new(Endianness::Little);
    let error = w
        .write_wstring_bounded(ASTRAL, 3, policy, order, WirePosition::Member)
        .expect_err("four code units do not fit a bound of three");
    match error {
        WireError::OutOfRange { expected, .. } => {
            assert!(expected.contains("code units"), "{expected}");
        }
        other => panic!("expected an OutOfRange, got {other:?}"),
    }
    assert!(w.as_bytes().is_empty(), "nothing went on the wire");
}

/// The terminator C2 adds is not counted against the bound.
///
/// `wstring<3>` names a capacity of three characters, and RTI's XCDR1
/// form writes four units for three characters — the recorded
/// `wstring-bounded-utf8` row, whose declaration is `wstring<5>`.
/// Counting the terminator would refuse a value the declaration admits.
#[test]
fn the_terminator_is_not_counted_against_the_bound() {
    let mut w = Writer::new(Endianness::Little);
    w.write_wstring_bounded(
        BOUNDED,
        3,
        WstringLengthPolicy::CHARACTER_COUNT_TERMINATED,
        WstringUnitOrder::Stream,
        WirePosition::Member,
    )
    .expect("three characters fit wstring<3> even with a NUL behind them");
    assert_eq!(
        hex(w.as_bytes()),
        "0400000034120000e40000005a00000000000000"
    );
}

/// `Reader::fit_wstring` counts code units, and a TRIM never splits a
/// surrogate pair.
#[test]
fn fit_wstring_counts_code_units_and_trims_whole_characters() {
    use zerodds_wire::{Narrowed, TryConstruct, fit_wstring};

    // Four code units for three characters: the bound of three does not
    // hold it.
    assert!(matches!(
        fit_wstring(0, ASTRAL.to_string(), 3, TryConstruct::Discard),
        Err(WireError::Discarded { .. })
    ));
    assert!(matches!(
        fit_wstring(0, ASTRAL.to_string(), 4, TryConstruct::Discard),
        Ok(Narrowed::Value(_))
    ));
    assert!(matches!(
        fit_wstring(0, ASTRAL.to_string(), 3, TryConstruct::UseDefault),
        Ok(Narrowed::Default)
    ));
    // A bound of two falls between the two halves of the surrogate
    // pair, so the trim takes one unit and not half a character: a lone
    // surrogate is not text, and a peer holding a two-unit buffer
    // cannot store half a pair either.
    match fit_wstring(0, ASTRAL.to_string(), 2, TryConstruct::Trim) {
        Ok(Narrowed::Value(trimmed)) => assert_eq!(trimmed, "a"),
        other => panic!("expected a trim to \"a\", got {other:?}"),
    }
    // A bound that falls on a whole character takes it.
    match fit_wstring(0, ASTRAL.to_string(), 3, TryConstruct::Trim) {
        Ok(Narrowed::Value(trimmed)) => assert_eq!(trimmed, "a\u{1d11e}"),
        other => panic!("expected a trim to \"a𝄞\", got {other:?}"),
    }
}

// -- the diagnostic ----------------------------------------------------

/// Every one of the eight cells has a name and a flag, and no two share
/// either.
///
/// A cell that cannot be printed cannot be reported in a diagnostic,
/// and a cell that cannot be named on a command line cannot be reached
/// to be measured — which is what the enumeration cost.
#[test]
fn every_cell_has_a_name_and_a_flag_of_its_own() {
    let mut names: Vec<&str> = WstringLengthPolicy::ALL_CELLS
        .iter()
        .map(|cell| cell.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 8);
    let mut flags: Vec<&str> = WstringLengthPolicy::ALL_CELLS
        .iter()
        .map(|cell| cell.flag())
        .collect();
    flags.sort_unstable();
    flags.dedup();
    assert_eq!(flags.len(), 8);
    for cell in WstringLengthPolicy::ALL_CELLS {
        assert_eq!(cell.flag(), format!("--wstring-length {}", cell.as_str()));
    }
    assert!(WstringLengthPolicy::SPECIFICATION.is_default());
    // C4's two questions, asked at a position for the reason C1's are:
    // the mark is one point of the axis and the little-endian content is
    // two, and the nine implementations answer both alike at the member
    // and the element.
    for position in [WirePosition::Member, WirePosition::Element] {
        assert!(WstringUnitOrder::ByteOrderMark.has_mark());
        assert!(!WstringUnitOrder::Stream.has_mark());
        assert!(!WstringUnitOrder::BigEndianAlways.has_mark());
        assert!(!WstringUnitOrder::HostUnswapped.has_mark());
        assert!(
            WstringUnitOrder::HostUnswapped.writes_little_endian_units(position),
            "{position:?}"
        );
        assert!(
            WstringUnitOrder::ByteOrderMark.writes_little_endian_units(position),
            "{position:?}"
        );
        assert!(!WstringUnitOrder::Stream.writes_little_endian_units(position));
        assert!(!WstringUnitOrder::BigEndianAlways.writes_little_endian_units(position));
    }
    assert_eq!(
        WstringLengthPolicy::default(),
        WstringLengthPolicy::SPECIFICATION
    );
}

/// `admits_length` is the shape of the number and nothing else.
#[test]
fn a_cell_admits_only_the_lengths_it_could_have_written() {
    // An octet count over two-octet units is even.
    assert!(WstringLengthPolicy::SPECIFICATION.admits_length(4));
    assert!(!WstringLengthPolicy::SPECIFICATION.admits_length(3));
    // Over four-octet units it divides by four.
    assert!(WstringLengthPolicy::SPECIFICATION_WIDE_UNITS.admits_length(8));
    assert!(!WstringLengthPolicy::SPECIFICATION_WIDE_UNITS.admits_length(6));
    // A character count is any number …
    assert!(WstringLengthPolicy::CHARACTER_COUNT.admits_length(3));
    assert!(WstringLengthPolicy::CHARACTER_COUNT.admits_length(0));
    // … unless it includes a terminator, which cannot be zero.
    assert!(WstringLengthPolicy::CHARACTER_COUNT_TERMINATED.admits_length(1));
    assert!(!WstringLengthPolicy::CHARACTER_COUNT_TERMINATED.admits_length(0));
}

/// A local spelling for a diagnostic, because
/// `zerodds_wire::WstringUnitOrder` carries no `as_str` of its own —
/// the ast-layout half does, and this crate's copy is the byte layer.
trait OrderName {
    fn spelling(self) -> &'static str;
}

impl OrderName for WstringUnitOrder {
    fn spelling(self) -> &'static str {
        match self {
            WstringUnitOrder::Stream => "stream",
            WstringUnitOrder::BigEndianAlways => "big-endian-always",
            WstringUnitOrder::ByteOrderMark => "byte-order-mark",
            WstringUnitOrder::HostUnswapped => "host-unswapped",
        }
    }
}

/// The `UInt32` length field is refused rather than wrapped.
///
/// A length that names a prefix of its own content is the silent class:
/// the peer reads a well-formed shorter `wstring` and then every
/// following member from inside this one, and nothing in the frame says
/// so. `(units.len() * width) as u32` stood in the writer and would
/// have done exactly that at 2 GiB of text for the narrow width and
/// 1 GiB for the wide.
///
/// Tested on [`WstringLengthPolicy::declared_length`] and not through
/// [`Writer::write_wstring`], because reaching it through the writer
/// means holding two gigabytes of text — which is why the computation
/// is a function of its own.
#[test]
fn a_length_that_does_not_fit_the_uint32_field_is_refused_and_not_wrapped() {
    let member = WirePosition::Member;
    // The boundary, exactly: `u32::MAX` code units under a character
    // count fits, one more does not.
    assert_eq!(
        WstringLengthPolicy::CHARACTER_COUNT.declared_length(u32::MAX as usize, member),
        Some(u32::MAX)
    );
    assert_eq!(
        WstringLengthPolicy::CHARACTER_COUNT.declared_length(u32::MAX as usize + 1, member),
        None
    );
    // An octet count divides the room by C3, so the boundary moves with
    // the width. `as u32` would have answered 0xFFFFFFFE here.
    let half = (u32::MAX as usize).div_ceil(2);
    assert_eq!(
        WstringLengthPolicy::SPECIFICATION.declared_length(half - 1, member),
        Some(0xFFFF_FFFE)
    );
    assert_eq!(
        WstringLengthPolicy::SPECIFICATION.declared_length(half, member),
        None
    );
    let quarter = (u32::MAX as usize).div_ceil(4);
    assert_eq!(
        WstringLengthPolicy::SPECIFICATION_WIDE_UNITS.declared_length(quarter, member),
        None
    );
    // And the multiplication itself cannot overflow into a small
    // number: `usize::MAX * 4` is not `usize::MAX - 3`.
    for cell in WstringLengthPolicy::ALL_CELLS {
        assert_eq!(
            cell.declared_length(usize::MAX, member),
            None,
            "{}",
            cell.as_str()
        );
    }
    // The ordinary case still answers what the recorded rows show:
    // seven characters is fourteen octets or seven code units.
    assert_eq!(
        WstringLengthPolicy::SPECIFICATION.declared_length(7, member),
        Some(14)
    );
    assert_eq!(
        WstringLengthPolicy::CHARACTER_COUNT.declared_length(7, member),
        Some(7)
    );
}
