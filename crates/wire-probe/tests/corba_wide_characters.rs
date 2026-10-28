// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The three ORB dialects, against the bytes the three ORBs write.
//!
//! CORBA 3.3 part 2 §9.3.1.6 has two wide-character encodings, one per
//! GIOP version, and XTypes 1.3 Table 31 replaced one half of the pair.
//! Neither `WstringLengthPolicy` nor any dialect reached the CORBA forms
//! before this file, so a topic with a `wchar` or a `wstring` in it could
//! not be exchanged with omniORB, TAO or JacORB in either direction —
//! `docs/corba-dialect-followup.md` items 1 and 2.
//!
//! Every byte string below was measured on the Linux test host on
//! 2026-08-10 through `corba-oracle/tools/wireprobe-{omniorb,tao,
//! jacorb}` — omniORB 4.3.3, TAO 2.5.24, JacORB 3.9 — with the two
//! types this file declares and the values it uses. They are
//! transcriptions of a run and not of a reading of the clause: three of
//! them contradict what the clause alone would predict.
//!
//! # What the run settled that the clause did not
//!
//! - **The `wchar` octets do not follow the stream.** omniORB writes
//!   `02 00e4` for U+00E4 in a *little-endian* stream, and so does
//!   JacORB. TAO writes `04 e4000000`, which is the stream's order. So
//!   the scalar's octet order is the same axis a `wstring`'s code units
//!   ride on, with the one difference CORBA makes: a scalar cannot carry
//!   a byte-order mark, so its absence means big-endian.
//! - **TAO's `wstring` is a form no DDS implementation writes.** An
//!   octet count over *four-octet* code units — GIOP 1.2's length rule
//!   over a four-octet transmission code set. It is the measured proof
//!   that `WstringLengthPolicy` was two axes glued together.
//! - **omniORB's mark is not decoration.** It writes the mark *and* the
//!   units in host order and swaps only the length word, so in a
//!   big-endian stream its mark reads as `FFFE` — the signal that the
//!   units behind it are the other way round. Its output is
//!   self-describing where Cyclone's identically-ordered output
//!   (`WstringLengthPolicy::UnswappedUnits`) is not.
//! - **An empty `wstring` carries no mark.** omniORB writes `00000000`,
//!   byte-identical to every other implementation.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_layout::{
    Dialect, Encoding, Endianness, LayoutKey, WcharFraming, WirePosition, WstringLengthPolicy,
    WstringUnitOrder,
};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

/// Two scalar `wchar`s, so that the second one's offset shows the
/// framing's effect on alignment as well as on width.
const WCHAR_IDL: &str = "@final struct Sample { wchar wide_letter_a; wchar wide_letter_b; };";

/// A `wstring` with a trailing `octet`, so that a form which consumed
/// the wrong number of octets moves a member the test can see.
const WSTRING_IDL: &str = "@final struct Sample { wstring w; octet tail; };";

fn request(type_name: &str, endianness: Endianness, dialect: Dialect) -> ProbeRequest {
    // XCDR1 and nothing else: CORBA CDR *is* XCDR1, CORBA 3.3 part 2
    // §9.3, and an ORB has no encoding version 2 to compare against.
    ProbeRequest::in_dialect(type_name, Encoding::PlainCdr1, endianness, dialect)
}

fn written(
    idl: &str,
    values: &serde_json::Value,
    endianness: Endianness,
    dialect: Dialect,
) -> String {
    let ast = model_from_idl(idl, "corba.idl").expect("the model");
    hex(&encode(&ast, &request("Sample", endianness, dialect), values).expect("the probe"))
}

fn unhex(text: &str) -> Vec<u8> {
    let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    (0..clean.len() / 2)
        .map(|index| u8::from_str_radix(&clean[index * 2..index * 2 + 2], 16).expect("hex"))
        .collect()
}

fn read(idl: &str, bytes: &str, endianness: Endianness, dialect: Dialect) -> serde_json::Value {
    let ast = model_from_idl(idl, "corba.idl").expect("the model");
    decode(&ast, &request("Sample", endianness, dialect), &unhex(bytes)).expect("the probe")
}

// -- the scalar `wchar` ------------------------------------------------

/// GIOP 1.2's `wchar`: an octet count, then that many octets.
///
/// omniORB and JacORB transmit two octets and TAO four, at the same
/// GIOP version — which is why the width is a setting of its own and not
/// part of the framing. XTypes Table 31 replaced this form with a
/// fixed-width `Char16`, so the default is unchanged and only these
/// three presets reach it.
#[test]
fn each_orbs_wchar_is_the_giop_form_that_orb_writes() {
    let values = json!({ "wide_letter_a": "ä", "wide_letter_b": "ሴ" });
    // (dialect, little-endian bytes, big-endian bytes)
    let measured = [
        // omniORB writes the same octets in both stream orders: the
        // code set is UTF-16 and a scalar carries no mark, so CORBA's
        // default — big-endian — applies to both.
        (Dialect::OmniOrb, "0200e4021234", "0200e4021234"),
        // TAO's four octets are the stream's order. Its big-endian half
        // is an extrapolation and says so: libACE here was built without
        // `ACE_ENABLE_SWAP_ON_WRITE`, so TAO writes host order only and
        // the two candidate orders coincide in what was measured.
        (Dialect::Tao, "04e40000000434120000", "04000000e40400001234"),
        // JacORB as omniORB. Measured big-endian only, for the mirror
        // reason: `org.jacorb.orb.CDROutputStream` has no
        // `setLittleEndian`.
        (Dialect::JacOrb, "0200e4021234", "0200e4021234"),
    ];
    for (dialect, little, big) in measured {
        assert_eq!(
            written(WCHAR_IDL, &values, Endianness::Little, dialect),
            little,
            "{dialect:?} in a little-endian stream"
        );
        assert_eq!(
            written(WCHAR_IDL, &values, Endianness::Big, dialect),
            big,
            "{dialect:?} in a big-endian stream"
        );
    }
}

/// The framing moves the member, and moves everything behind it.
///
/// This is why `WcharFraming` is in `LayoutKey::offset_key` and could
/// not be another value of the width: a `wchar` that carries its own
/// count is one octet longer *and* aligns to one, so an `octet` in front
/// of it no longer pads.
#[test]
fn the_giop_framing_aligns_a_wchar_to_one_and_moves_the_members_behind_it() {
    const IDL: &str = "@final struct Sample { octet head; wchar c; long n; };";
    let values = json!({ "head": 1, "c": "a", "n": 7 });
    let ast = model_from_idl(IDL, "corba.idl").expect("the model");
    let fixed = hex(&encode(
        &ast,
        &request("Sample", Endianness::Little, Dialect::Cyclone),
        &values,
    )
    .expect("the probe"));
    let prefixed = hex(&encode(
        &ast,
        &request("Sample", Endianness::Little, Dialect::OmniOrb),
        &values,
    )
    .expect("the probe"));
    // Table 31: `head` at 0, one byte of padding, `c` at 2, `n` at 4.
    assert_eq!(fixed, "0100610007000000");
    // §9.3.1.6: `head` at 0, `c` at 1 with no padding at all — the count
    // octet then the two code-unit octets — and `n` at 4.
    assert_eq!(prefixed, "01020061 07000000".replace(' ', ""));
}

/// The count is checked and never believed.
///
/// A stream whose first octet is `0xE4` — which is what a bare
/// two-octet `wchar` looks like read this way — would otherwise consume
/// 228 octets of whatever followed and return a character assembled out
/// of the next four members. The length is the one field a peer
/// controls, and the one a reader must never let decide how much it
/// reads.
#[test]
fn a_giop_wchar_whose_count_is_not_the_code_sets_width_is_refused() {
    let ast = model_from_idl(WCHAR_IDL, "corba.idl").expect("the model");
    // Cyclone's bare `e400 3412`, handed to an omniORB reader.
    let error = decode(
        &ast,
        &request("Sample", Endianness::Little, Dialect::OmniOrb),
        &unhex("e4003412"),
    )
    .expect_err("a refusal");
    let text = format!("{error}");
    assert!(
        text.contains("GIOP 1.2 wchar of 2 octets"),
        "the error has to name the width it expected: {text}"
    );
}

// -- the `wstring` -----------------------------------------------------

/// Each ORB's `wstring`, including a code point outside the BMP.
///
/// `U+1D11E` is a surrogate pair in all three, one pair of code units —
/// TAO zero-extends each unit to four octets rather than writing one
/// UCS-4 code point, so nothing is lost and the surrogate rule is the
/// same rule.
#[test]
fn each_orbs_wstring_is_the_form_that_orb_writes() {
    let values = json!({ "w": "a\u{1D11E}b", "tail": 171 });
    let measured = [
        (
            Dialect::OmniOrb,
            "0a000000fffe610034d81edd6200ab",
            "0000000afffe610034d81edd6200ab",
        ),
        (
            Dialect::Tao,
            "100000006100000034d800001edd000062000000ab",
            "00000010000000610000d8340000dd1e00000062ab",
        ),
        (
            Dialect::JacOrb,
            "080000000061d834dd1e0062ab",
            "000000080061d834dd1e0062ab",
        ),
    ];
    for (dialect, little, big) in measured {
        assert_eq!(
            written(WSTRING_IDL, &values, Endianness::Little, dialect),
            little,
            "{dialect:?} in a little-endian stream"
        );
        assert_eq!(
            written(WSTRING_IDL, &values, Endianness::Big, dialect),
            big,
            "{dialect:?} in a big-endian stream"
        );
    }
}

/// An empty `wstring` carries no byte-order mark.
///
/// Measured: omniORB writes `00000000` for one, which is what every
/// other implementation writes. A mark on an empty string would be a
/// two-octet string that decodes to nothing, and no peer writes it.
#[test]
fn an_empty_wstring_carries_no_byte_order_mark() {
    let values = json!({ "w": "", "tail": 171 });
    for endianness in [Endianness::Little, Endianness::Big] {
        assert_eq!(
            written(WSTRING_IDL, &values, endianness, Dialect::OmniOrb),
            "00000000ab",
            "an empty wstring under omniORB, {endianness:?}"
        );
    }
}

/// Every ORB form reads back under its own dialect, in both orders.
///
/// The bytes are the ones the ORBs themselves produced. The read
/// direction was confirmed the other way round on the test host as well:
/// all three ORBs read the bytes these presets write, twelve of twelve
/// over both types and both byte orders.
#[test]
fn each_orb_form_reads_back_under_its_own_dialect() {
    let expected_wstring = json!({ "w": "a\u{1D11E}b", "tail": 171 });
    let expected_wchar = json!({ "wide_letter_a": "ä", "wide_letter_b": "ሴ" });
    let cases = [
        (
            Dialect::OmniOrb,
            Endianness::Little,
            "0a000000fffe610034d81edd6200ab",
            "0200e4021234",
        ),
        (
            Dialect::OmniOrb,
            Endianness::Big,
            "0000000afffe610034d81edd6200ab",
            "0200e4021234",
        ),
        (
            Dialect::Tao,
            Endianness::Little,
            "100000006100000034d800001edd000062000000ab",
            "04e40000000434120000",
        ),
        (
            Dialect::JacOrb,
            Endianness::Little,
            "080000000061d834dd1e0062ab",
            "0200e4021234",
        ),
        (
            Dialect::JacOrb,
            Endianness::Big,
            "000000080061d834dd1e0062ab",
            "0200e4021234",
        ),
    ];
    for (dialect, endianness, wstring, wchar) in cases {
        assert_eq!(
            read(WSTRING_IDL, wstring, endianness, dialect),
            expected_wstring,
            "{dialect:?} reading its own wstring, {endianness:?}"
        );
        assert_eq!(
            read(WCHAR_IDL, wchar, endianness, dialect),
            expected_wchar,
            "{dialect:?} reading its own wchar, {endianness:?}"
        );
    }
}

/// omniORB's mark makes its own output self-describing.
///
/// The mark and the units are host order in both stream orders, so in a
/// big-endian stream the mark reads as `FFFE` and says so. A reader that
/// obeys the mark gets the right characters from either stream; one that
/// ignored it would get different ones from one of the two.
#[test]
fn the_byte_order_mark_and_not_the_stream_decides_the_units() {
    let expected = json!({ "w": "ab", "tail": 171 });
    // The mark the right way up, units in the same order.
    assert_eq!(
        read(
            WSTRING_IDL,
            "06000000fffe61006200ab",
            Endianness::Little,
            Dialect::OmniOrb
        ),
        expected
    );
    // The same octets in a big-endian stream: the length word swaps,
    // the mark and the units do not, and the mark now says `FFFE`.
    assert_eq!(
        read(
            WSTRING_IDL,
            "00000006fffe61006200ab",
            Endianness::Big,
            Dialect::OmniOrb
        ),
        expected
    );
    // A mark that is the right way up in a big-endian stream means the
    // units are big-endian too — what a big-endian host's omniORB would
    // write. Unmeasured, and the rule that covers it is CORBA's own.
    assert_eq!(
        read(
            WSTRING_IDL,
            "00000006feff00610062ab",
            Endianness::Big,
            Dialect::OmniOrb
        ),
        expected
    );
    // No mark at all: CORBA's default for a GIOP 1.2 `wstring` is
    // UTF-16 big-endian, which is what makes this variant read a JacORB
    // peer as well as an omniORB one.
    assert_eq!(
        read(
            WSTRING_IDL,
            "0400000000610062ab",
            Endianness::Little,
            Dialect::OmniOrb
        ),
        expected
    );
}

/// The silent case, in both directions, named rather than assumed.
///
/// A `wstring`'s code-unit byte order is not in the stream. XTypes
/// §7.4.1.1.2 makes the units the stream's, the OMG code-set rule for a
/// GIOP 1.2 `wstring` makes them UTF-16's own, and `04000000 61006200`
/// is a legal encoding of `"ab"` under the first and of `"愀戀"` under
/// the second. Each side is self-consistent and they cannot talk; that
/// is the divergence the three presets exist for, and it is the reason
/// this is a setting and not a detection.
#[test]
fn an_orb_wstring_read_as_the_specification_returns_different_characters() {
    // JacORB's form, read as §7.4.1.1.2.
    let ast = model_from_idl(WSTRING_IDL, "corba.idl").expect("the model");
    let mut plain = ProbeRequest::new("Sample", Encoding::PlainCdr1, Endianness::Little);
    plain.wstring_length = WstringLengthPolicy::SPECIFICATION;
    plain.wstring_unit_order = WstringUnitOrder::Stream;
    let read = decode(&ast, &plain, &unhex("0400000000610062ab")).expect("well-formed UTF-16");
    assert_eq!(
        read,
        json!({ "w": "愀戀", "tail": 171 }),
        "the right length, well formed, and the wrong characters"
    );
    // And omniORB's, where the mark arrives as a character of the
    // string — which §7.4.1.1.2 forbids an encoder from writing and says
    // nothing about a decoder receiving.
    let read = decode(&ast, &plain, &unhex("06000000fffe61006200ab")).expect("well-formed UTF-16");
    assert_eq!(read, json!({ "w": "\u{FEFF}ab", "tail": 171 }));
}

// -- the settings themselves -------------------------------------------

/// The byte order is one axis, so there is no pair to refuse.
///
/// `HostUnswapped` used to be `WstringLengthPolicy::UnswappedUnits` — a
/// byte order living in the length enumeration under a length's name —
/// and every caller that could see both settings had to refuse the
/// combination, because two settings deciding one byte order with a
/// precedence rule between them is the shape that produces a wrong
/// character and no error. Judgement 2 of `docs/wire-axes-followup.md`
/// moved it to C4, and one axis cannot conflict with itself: all four
/// orders combine with all eight cells of C1 x C2 x C3.
///
/// The check is kept, inverted, so that a setting reintroducing the
/// overlap fails here rather than in a peer's `wstring`.
#[test]
fn no_length_cell_and_no_unit_order_both_decide_the_byte_order() {
    for policy in WstringLengthPolicy::ALL_CELLS {
        for order in WstringUnitOrder::ALL {
            let key = LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
                .with_wstring_length(policy)
                .with_wstring_unit_order(order);
            assert!(
                !key.wstring_order_conflicts(),
                "{} and {order:?} were reported as both deciding the code units' byte order, \
                 which means a second setting has started claiming it",
                policy.as_str()
            );
        }
    }
}

/// Each ORB preset selects the framing, the width and the order its ORB
/// was measured writing.
///
/// A preset that answered one of the four wrongly would still produce
/// bytes; this is the assertion that says which bytes. The doc comment
/// on [`Dialect`] carries the same table in prose.
///
/// The framing is in the table and not shared, because the three ORBs
/// do not share it. Measured from the recorded corpus: omniORB and
/// JacORB write GIOP 1.2's octet count wherever a `wchar` appears —
/// `02 00e4 02 1234 02 005a` for the three elements of
/// `array-1d-primitives` — and TAO writes it for a member
/// (`04 e4000000` in `primitive-wchar`) and not for an element
/// (`00 e4000000 34120000 5a000000`, one pad octet to the four-octet
/// alignment and then three bare units). `WcharFraming::
/// GiopLengthPrefixedMemberOnly` is that third answer.
#[test]
fn each_orb_preset_names_the_three_settings_its_orb_was_measured_with() {
    let expected = [
        (
            Dialect::OmniOrb,
            WcharFraming::GiopLengthPrefixed,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::ByteOrderMark,
            2,
        ),
        (
            Dialect::Tao,
            WcharFraming::GiopLengthPrefixedMemberOnly,
            WstringLengthPolicy::SPECIFICATION_WIDE_UNITS,
            WstringUnitOrder::Stream,
            4,
        ),
        (
            Dialect::JacOrb,
            WcharFraming::GiopLengthPrefixed,
            WstringLengthPolicy::SPECIFICATION,
            WstringUnitOrder::BigEndianAlways,
            2,
        ),
    ];
    for (dialect, framing, length, order, width) in expected {
        assert!(dialect.is_corba(), "{dialect:?} is a CORBA ORB");
        assert_eq!(dialect.wchar_framing(), framing, "{dialect:?} framing");
        assert_eq!(dialect.wstring_length(Encoding::PlainCdr1), length);
        assert_eq!(dialect.wstring_unit_order(), order);
        assert_eq!(
            dialect
                .wstring_length(Encoding::PlainCdr1)
                .character_width(WirePosition::Member),
            width
        );
    }
    // And no DDS preset reaches the GIOP framing: every one of them
    // writes Table 31's fixed-width `Char16`, and that stays.
    for dialect in [
        Dialect::Cyclone,
        Dialect::Rti,
        Dialect::FastDds,
        Dialect::OpenDds,
        Dialect::RosFastRtps,
        Dialect::RosCyclone,
        Dialect::Specification,
    ] {
        assert!(!dialect.is_corba());
        assert_eq!(dialect.wchar_framing(), WcharFraming::Fixed);
    }
    // The **unit order** is a different axis and two of them do depart
    // from the stream's: Cyclone leaves a `wstring`'s code units in the
    // host's order and `rosidl_typesupport_fastrtps_c` does the same.
    // Both are measured and both are in the dialect named for the
    // product — `WstringUnitOrder` carries the runs — so the claim here
    // is the exact split rather than "no DDS preset departs", which is
    // what it used to say and is no longer true.
    for dialect in [
        Dialect::Rti,
        Dialect::FastDds,
        Dialect::OpenDds,
        Dialect::RosCyclone,
        Dialect::Specification,
    ] {
        assert_eq!(dialect.wstring_unit_order(), WstringUnitOrder::Stream);
    }
    for dialect in [Dialect::Cyclone, Dialect::RosFastRtps] {
        assert_eq!(
            dialect.wstring_unit_order(),
            WstringUnitOrder::HostUnswapped
        );
    }
}
