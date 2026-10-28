// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The three `wstring` forms three implementations write, both
//! directions.
//!
//! Every byte string in this file is a measurement.
//! `crates/ast-layout/tests/fixtures/vendor_bytes.md` §4 records the
//! runs: `@final struct WStr { wstring w; octet tail; }` with
//! `w = "ab"` and with `w = "𝄞"` (`U+1D11E`), through Cyclone DDS, RTI
//! Connext 7.7 and Fast DDS, in both encodings and both byte orders.
//! Nothing here was worked out from the clause text alone.
//!
//! ## What the reader did before there was a policy
//!
//! One reader, the byte-count one, and the three forms went into it:
//!
//! - Fast DDS's character count returned **half the string with no
//!   error** — `"ab"` came back as `"a"` and the trailing octet came
//!   back as `0x62`, the second character's low byte. That is the
//!   silent-drop class arriving from the wire.
//! - RTI's XCDR1 count-with-NUL was refused, but as "an even byte
//!   count, two per UTF-16 code unit, found 3" — true, and no help at
//!   all in placing the cause.
//! - Fast DDS's truncated non-BMP character was refused for the same
//!   reason, by accident: its count of 1 is odd. Under the policy that
//!   reads its length right it is not refused at all — `1ed1` is
//!   `U+D11E`, a Hangul syllable, so the wrong character arrives in a
//!   faultless stream.
//!
//! The tests below pin all three, and pin the two cases that cannot be
//! caught at all.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, WstringLengthPolicy, WstringUnitOrder};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{ProbeError, encode, hex, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

/// `@final struct WStr { wstring w; octet tail; }`, the measured type.
const IDL: &str = "@final struct WStr { wstring w; octet tail; };";

/// The same with a four-byte trailing member, where a short `wstring`
/// read is absorbed by the next member's alignment.
const IDL_LONG: &str = "@final struct WLong { wstring w; long n; };";

/// A request that varies the **length** and holds every other
/// coordinate at the clause's answer.
///
/// The unit order is set explicitly and that is still not redundant.
/// `ProbeRequest::new` is `Dialect::default`, which is
/// `Dialect::Specification` and does answer `WstringUnitOrder::Stream` —
/// so the explicit call currently sets what the default would have
/// given. It stays because this function's contract is "C4 held at the
/// clause's answer" and not "C4 left wherever the default puts it": the
/// default was `Dialect::Cyclone` once, which leaves a `wstring`'s code
/// units in the host's order, and six of these tests went red at once
/// when the setting moved into the dialect it belongs in. A test of
/// C1 × C2 × C3 that takes its C4 from a default is a test that measures
/// two axes and reports one the next time the default moves.
/// `request_ordered` is the other half, and varies C4 with the length
/// held.
fn request(
    type_name: &str,
    encoding: Encoding,
    endianness: Endianness,
    policy: WstringLengthPolicy,
) -> ProbeRequest {
    let mut request = ProbeRequest::new(type_name, encoding, endianness);
    request.wstring_length = policy;
    request.wstring_unit_order = WstringUnitOrder::Stream;
    request
}

/// What the probe writes, as hex.
fn written(
    idl: &str,
    type_name: &str,
    values: &serde_json::Value,
    encoding: Encoding,
    endianness: Endianness,
    policy: WstringLengthPolicy,
) -> String {
    let ast = model_from_idl(idl, "wstring.idl").expect("the model");
    hex(&encode(
        &ast,
        &request(type_name, encoding, endianness, policy),
        values,
    )
    .expect("the probe"))
}

/// The same request with the code units' byte order set.
///
/// `HostUnswapped` used to be a value of the length policy, so the
/// tests below reached it through `request`. It is C4 now, and reaching
/// it needs the order rather than the length — which is the point of
/// the split and the reason these helpers are two.
fn request_ordered(
    type_name: &str,
    encoding: Encoding,
    endianness: Endianness,
    order: WstringUnitOrder,
) -> ProbeRequest {
    let mut request = ProbeRequest::new(type_name, encoding, endianness);
    request.wstring_unit_order = order;
    request
}

/// What the probe writes under an order, as hex.
fn written_ordered(
    idl: &str,
    type_name: &str,
    values: &serde_json::Value,
    encoding: Encoding,
    endianness: Endianness,
    order: WstringUnitOrder,
) -> String {
    let ast = model_from_idl(idl, "wstring.idl").expect("the model");
    hex(&encode(
        &ast,
        &request_ordered(type_name, encoding, endianness, order),
        values,
    )
    .expect("the probe"))
}

fn refusal(
    idl: &str,
    type_name: &str,
    values: &serde_json::Value,
    policy: WstringLengthPolicy,
) -> ProbeError {
    let ast = model_from_idl(idl, "wstring.idl").expect("the model");
    encode(
        &ast,
        &request(type_name, Encoding::PlainCdr2, Endianness::Little, policy),
        values,
    )
    .expect_err("a refusal")
}

fn unhex(text: &str) -> Vec<u8> {
    let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    (0..clean.len() / 2)
        .map(|index| u8::from_str_radix(&clean[index * 2..index * 2 + 2], 16).expect("hex"))
        .collect()
}

/// What the probe reads back, or the error it refused with.
fn read(
    idl: &str,
    type_name: &str,
    bytes: &str,
    encoding: Encoding,
    policy: WstringLengthPolicy,
) -> Result<serde_json::Value, ProbeError> {
    let ast = model_from_idl(idl, "wstring.idl").expect("the model");
    decode(
        &ast,
        &request(type_name, encoding, Endianness::Little, policy),
        &unhex(bytes),
    )
}

// -- the writer --------------------------------------------------------

/// The specification's form, which is Cyclone's in both versions and
/// RTI's under XCDR2. `vendor_bytes.md` §4.
#[test]
fn the_specification_form_is_the_byte_count_and_no_terminator() {
    let sample = json!({ "w": "ab", "tail": 171 });
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        assert_eq!(
            written(
                IDL,
                "WStr",
                &sample,
                encoding,
                Endianness::Little,
                WstringLengthPolicy::SPECIFICATION
            ),
            "0400000061006200ab"
        );
        assert_eq!(
            written(
                IDL,
                "WStr",
                &sample,
                encoding,
                Endianness::Big,
                WstringLengthPolicy::SPECIFICATION
            ),
            "0000000400610062ab"
        );
    }
}

/// A code point outside the BMP is a surrogate pair and counts four.
/// §7.4.1.1.2's rationale, and Cyclone's and RTI-XCDR2's bytes.
#[test]
fn the_specification_form_surrogate_pairs_a_non_bmp_code_point() {
    let sample = json!({ "w": "\u{1d11e}", "tail": 171 });
    assert_eq!(
        written(
            IDL,
            "WStr",
            &sample,
            Encoding::PlainCdr2,
            Endianness::Little,
            WstringLengthPolicy::SPECIFICATION
        ),
        "0400000034d81eddab"
    );
    // Big endian swaps the units, which is where ZeroDDS parts company
    // with Cyclone: `vendor_bytes.md` §4 records Cyclone writing
    // `34d81edd` in both orders. RTI and Fast DDS swap, and so does
    // this.
    assert_eq!(
        written(
            IDL,
            "WStr",
            &sample,
            Encoding::PlainCdr2,
            Endianness::Big,
            WstringLengthPolicy::SPECIFICATION
        ),
        "00000004d834dd1eab"
    );
}

/// Fast DDS's form: the character count, two bytes each, no terminator.
#[test]
fn the_character_count_form_is_what_fast_dds_writes() {
    let sample = json!({ "w": "ab", "tail": 171 });
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        assert_eq!(
            written(
                IDL,
                "WStr",
                &sample,
                encoding,
                Endianness::Little,
                WstringLengthPolicy::CHARACTER_COUNT
            ),
            "0200000061006200ab"
        );
        assert_eq!(
            written(
                IDL,
                "WStr",
                &sample,
                encoding,
                Endianness::Big,
                WstringLengthPolicy::CHARACTER_COUNT
            ),
            "0000000200610062ab"
        );
    }
}

/// Fast CDR truncates a non-BMP code point to sixteen bits and sends a
/// different character. The length convention is reproduced; the data
/// loss is refused.
///
/// `vendor_bytes.md` §4 records what Fast DDS does instead:
/// `01000000 1ed1`, where `d11e` is `U+1D11E` with its high bits gone.
/// `U+D11E` is a Hangul syllable, so the peer that reads it has a
/// well-formed string with the wrong character in it and no way to
/// know.
#[test]
fn the_character_count_form_refuses_what_fast_cdr_would_truncate() {
    let error = refusal(
        IDL,
        "WStr",
        &json!({ "w": "\u{1d11e}", "tail": 171 }),
        WstringLengthPolicy::CHARACTER_COUNT,
    );
    assert!(
        matches!(&error, ProbeError::Unsupported { kind, .. } if kind.contains("outside the BMP")),
        "{error}"
    );
}

/// RTI's XCDR1 form: the character count including the NUL, four bytes
/// per character. Nothing is lost above the BMP.
#[test]
fn the_terminated_form_is_what_rti_writes_under_xcdr1() {
    assert_eq!(
        written(
            IDL,
            "WStr",
            &json!({ "w": "ab", "tail": 171 }),
            Encoding::PlainCdr1,
            Endianness::Little,
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED
        ),
        "03000000610000006200000000000000ab"
    );
    assert_eq!(
        written(
            IDL,
            "WStr",
            &json!({ "w": "\u{1d11e}", "tail": 171 }),
            Encoding::PlainCdr1,
            Endianness::Little,
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED
        ),
        "0300000034d800001edd000000000000ab"
    );
    assert_eq!(
        written(
            IDL,
            "WStr",
            &json!({ "w": "ab", "tail": 171 }),
            Encoding::PlainCdr1,
            Endianness::Big,
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED
        ),
        "00000003000000610000006200000000ab"
    );
}

/// An empty `wstring` is a length of zero under two of the forms and a
/// length of one under RTI's, which counts the terminator it writes.
#[test]
fn an_empty_wstring_differs_by_the_terminator_alone() {
    let sample = json!({ "w": "", "tail": 171 });
    assert_eq!(
        written(
            IDL,
            "WStr",
            &sample,
            Encoding::PlainCdr2,
            Endianness::Little,
            WstringLengthPolicy::SPECIFICATION
        ),
        "00000000ab"
    );
    assert_eq!(
        written(
            IDL,
            "WStr",
            &sample,
            Encoding::PlainCdr2,
            Endianness::Little,
            WstringLengthPolicy::CHARACTER_COUNT
        ),
        "00000000ab"
    );
    assert_eq!(
        written(
            IDL,
            "WStr",
            &sample,
            Encoding::PlainCdr1,
            Endianness::Little,
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED
        ),
        "0100000000000000ab"
    );
}

// -- the reader, each form under its own policy ------------------------

/// Each vendor's bytes read back as the sample that produced them, when
/// the reader is configured for that vendor's form.
#[test]
fn each_measured_form_reads_back_under_its_own_policy() {
    let expected = json!({ "tail": 171, "w": "ab" });

    // Cyclone, both versions; RTI under XCDR2.
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        assert_eq!(
            read(
                IDL,
                "WStr",
                "04000000 61006200 ab",
                encoding,
                WstringLengthPolicy::SPECIFICATION
            )
            .expect("the specification form"),
            expected
        );
        // Fast DDS, both versions.
        assert_eq!(
            read(
                IDL,
                "WStr",
                "02000000 61006200 ab",
                encoding,
                WstringLengthPolicy::CHARACTER_COUNT
            )
            .expect("the character-count form"),
            expected
        );
    }

    // RTI under XCDR1.
    assert_eq!(
        read(
            IDL,
            "WStr",
            "03000000 61000000 62000000 00000000 ab",
            Encoding::PlainCdr1,
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED
        )
        .expect("the terminated form"),
        expected
    );
}

/// The non-BMP character survives two of the three forms and is gone
/// beyond recovery in the third before the reader ever sees it.
#[test]
fn the_non_bmp_character_reads_back_from_the_two_forms_that_carry_it() {
    let expected = json!({ "tail": 171, "w": "\u{1d11e}" });
    assert_eq!(
        read(
            IDL,
            "WStr",
            "04000000 34d81edd ab",
            Encoding::PlainCdr2,
            WstringLengthPolicy::SPECIFICATION
        )
        .expect("the specification form"),
        expected
    );
    assert_eq!(
        read(
            IDL,
            "WStr",
            "03000000 34d80000 1edd0000 00000000 ab",
            Encoding::PlainCdr1,
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED
        )
        .expect("the terminated form"),
        expected
    );

    // Fast DDS's own bytes, read with the policy that gets its length
    // right, are the second case no reader can catch. `1ed1` is
    // `U+D11E`, a Hangul syllable: a valid code point, well-formed
    // UTF-16, the declared length. The character is wrong and the
    // stream is faultless.
    //
    // Recorded rather than asserted away, because the reader cannot do
    // better and the next person to look must not conclude it can.
    assert_eq!(
        read(
            IDL,
            "WStr",
            "01000000 1ed1 ab",
            Encoding::PlainCdr2,
            WstringLengthPolicy::CHARACTER_COUNT
        )
        .expect("a stream nothing is wrong with"),
        json!({ "tail": 171, "w": "\u{d11e}" })
    );
}

/// The surrogate check catches the truncations that happen to land in
/// `D800..=DFFF`, and nothing else.
///
/// `U+1D834` truncates to `U+D834`, a high surrogate with nothing after
/// it. That is not UTF-16 and is refused; `U+1D11E` in the test above
/// truncates to a Hangul syllable and is not. The difference is where
/// the code point sits, not anything the reader does.
#[test]
fn an_unpaired_surrogate_is_refused_rather_than_replaced() {
    let error = read(
        IDL,
        "WStr",
        "01000000 34d8 ab",
        Encoding::PlainCdr2,
        WstringLengthPolicy::CHARACTER_COUNT,
    )
    .expect_err("a refusal");
    assert!(
        matches!(&error, ProbeError::Mismatch { expected, .. }
            if expected.contains("well-formed UTF-16")),
        "{error}"
    );
}

// -- the reader, a form it was not configured for ----------------------

/// A Fast DDS `wstring` read as the specification's is an error and not
/// a shorter string.
///
/// This is the case the policy exists for. The declared 2 is a
/// character count; read as a byte count it takes one code unit, the
/// trailing octet comes out of the second character's low byte, and two
/// bytes are left that no member claimed. The frame check refuses it.
///
/// It used to return `{"tail": 98, "w": "a"}` — a plausible sample, a
/// lost character and a wrong octet, with no diagnostic anywhere.
#[test]
fn a_fast_dds_wstring_read_as_the_specifications_is_refused() {
    let error = read(
        IDL,
        "WStr",
        "02000000 61006200 ab",
        Encoding::PlainCdr2,
        WstringLengthPolicy::SPECIFICATION,
    )
    .expect_err("a refusal");
    assert!(
        matches!(&error, ProbeError::Mismatch { found, .. } if found.contains("byte(s) left")),
        "{error}"
    );
}

/// RTI's XCDR1 form read as the specification's names the form the
/// length does fit.
///
/// The old error said only "an even byte count ... found 3". A count of
/// 3 is not a byte count and it is not nothing either: it is what a
/// count-with-terminator produces for two characters, and the error
/// says so.
#[test]
fn an_rti_xcdr1_wstring_read_as_the_specifications_names_the_other_form() {
    let error = read(
        IDL,
        "WStr",
        "03000000 61000000 62000000 00000000 ab",
        Encoding::PlainCdr1,
        WstringLengthPolicy::SPECIFICATION,
    )
    .expect_err("a refusal");
    let ProbeError::Mismatch {
        expected, found, ..
    } = &error
    else {
        panic!("{error}");
    };
    assert!(expected.contains("specification"), "{expected}");
    assert!(found.contains("character-count"), "{found}");
    assert!(found.contains("character-count-terminated"), "{found}");
}

/// The specification's form read as RTI's is refused on the terminator.
///
/// `04000000 61006200` under the terminated policy is a count of four
/// characters of four bytes each, which the stream does not hold.
#[test]
fn a_specification_wstring_read_as_rtis_is_refused() {
    let error = read(
        IDL,
        "WStr",
        "04000000 61006200 ab",
        Encoding::PlainCdr1,
        WstringLengthPolicy::CHARACTER_COUNT_TERMINATED,
    )
    .expect_err("a refusal");
    assert!(
        matches!(&error, ProbeError::Mismatch { expected, .. }
            if expected.contains("length the stream can hold")),
        "{error}"
    );
}

/// The terminated form insists on the NUL its count includes.
#[test]
fn the_terminated_form_refuses_a_missing_or_interior_nul() {
    // Three characters declared, the last of them `62` and not a NUL.
    let missing = read(
        IDL,
        "WStr",
        "03000000 61000000 62000000 63000000 ab",
        Encoding::PlainCdr1,
        WstringLengthPolicy::CHARACTER_COUNT_TERMINATED,
    )
    .expect_err("a refusal");
    assert!(
        matches!(&missing, ProbeError::Mismatch { expected, .. }
            if expected.contains("NUL character ending the wstring")),
        "{missing}"
    );

    // A NUL in the middle: every C peer stops at it, so the value this
    // reader would return and the value that peer returns differ.
    let interior = read(
        IDL,
        "WStr",
        "04000000 61000000 00000000 62000000 00000000 ab",
        Encoding::PlainCdr1,
        WstringLengthPolicy::CHARACTER_COUNT_TERMINATED,
    )
    .expect_err("a refusal");
    assert!(
        matches!(&interior, ProbeError::Mismatch { expected, .. }
            if expected.contains("no NUL inside a wstring")),
        "{interior}"
    );
}

/// A four-byte character that is not a UTF-16 code unit is refused
/// rather than narrowed. Narrowing is the defect the whole policy is
/// about.
#[test]
fn the_terminated_form_refuses_a_character_wider_than_a_code_unit() {
    let error = read(
        IDL,
        "WStr",
        "02000000 1ed10100 00000000 ab",
        Encoding::PlainCdr1,
        WstringLengthPolicy::CHARACTER_COUNT_TERMINATED,
    )
    .expect_err("a refusal");
    assert!(
        matches!(&error, ProbeError::Mismatch { expected, .. }
            if expected.contains("one UTF-16 code unit")),
        "{error}"
    );
}

/// The one case no reader can catch, recorded so that it cannot change
/// without someone noticing.
///
/// `@final struct WLong { wstring w; long n; }`. Fast DDS's bytes for
/// `w = "ab", n = 7` are `02000000 61006200 07000000`. The
/// specification's bytes for `w = "a", n = 7` are `02000000 6100`, two
/// bytes of padding, `07000000` — the same twelve bytes, differing only
/// in what the padding holds, which no clause fixes. RTI's serialiser
/// writes nothing there; its RTPS layer writes zero
/// (`tools/wire-capture`), which is what this project writes, so on the
/// wire the two are the same twelve bytes and the case is undecidable
/// there too — for the padding's *content*. The undecidability is in the
/// length field either way.
///
/// So the specification's reader returns `"a"` and the `long` is
/// correct, and nothing is wrong with the stream. This is not a defect
/// to fix in the reader; it is why the policy is a setting rather than
/// a detection, and it is the whole argument on
/// `zerodds_ast_layout::WstringLengthPolicy`.
#[test]
fn the_undecidable_case_is_undecidable_and_stays_recorded() {
    let fast_dds = "02000000 61006200 07000000";
    assert_eq!(
        read(
            IDL_LONG,
            "WLong",
            fast_dds,
            Encoding::PlainCdr2,
            WstringLengthPolicy::CHARACTER_COUNT
        )
        .expect("Fast DDS's own form"),
        json!({ "n": 7, "w": "ab" })
    );
    assert_eq!(
        read(
            IDL_LONG,
            "WLong",
            fast_dds,
            Encoding::PlainCdr2,
            WstringLengthPolicy::SPECIFICATION
        )
        .expect("a stream the specification's reader cannot fault"),
        json!({ "n": 7, "w": "a" })
    );

    // And the two encodings really are the same bytes. If this ever
    // stops holding, the case above stopped being undecidable and the
    // reader can be made to catch it.
    assert_eq!(
        written(
            IDL_LONG,
            "WLong",
            &json!({ "w": "a", "n": 7 }),
            Encoding::PlainCdr2,
            Endianness::Little,
            WstringLengthPolicy::SPECIFICATION
        ),
        "020000006100000007000000"
    );
    assert_eq!(
        written(
            IDL_LONG,
            "WLong",
            &json!({ "w": "ab", "n": 7 }),
            Encoding::PlainCdr2,
            Endianness::Little,
            WstringLengthPolicy::CHARACTER_COUNT
        ),
        "020000006100620007000000"
    );
}

// -- round trips -------------------------------------------------------

/// Every policy reads back what it wrote, in both encodings and both
/// byte orders.
#[test]
fn every_policy_round_trips() {
    let ast = model_from_idl(IDL, "wstring.idl").expect("the model");
    let sample = json!({ "w": "aä\u{1d11e}", "tail": 171 });
    for policy in WstringLengthPolicy::MEASURED {
        // The character-count form cannot carry the non-BMP character
        // and says so; it round-trips the rest.
        let sample = if policy == WstringLengthPolicy::CHARACTER_COUNT {
            json!({ "w": "aä", "tail": 171 })
        } else {
            sample.clone()
        };
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            for endianness in Endianness::ALL {
                let request = request("WStr", encoding, endianness, policy);
                let bytes = encode(&ast, &request, &sample).expect("the probe");
                let back = decode(&ast, &request, &bytes).expect("the read back");
                assert_eq!(back, sample, "{policy:?} {encoding:?} {endianness:?}");
            }
        }
    }
}

/// The policy names round-trip and a typo is rejected rather than
/// falling back to the default.
#[test]
fn the_policy_names_round_trip_and_a_typo_is_rejected() {
    for policy in WstringLengthPolicy::MEASURED {
        assert_eq!(WstringLengthPolicy::parse(policy.as_str()), Some(policy));
    }
    assert_eq!(WstringLengthPolicy::parse("character_count"), None);
    assert_eq!(WstringLengthPolicy::parse("Specification"), None);
    assert_eq!(WstringLengthPolicy::parse(""), None);
    assert!(WstringLengthPolicy::default().is_default());
}

// -- the scalar `wchar` takes the same width ----------------------------

/// `@final struct WCh { wchar c; octet tail; }`, the measured type.
const IDL_WCHAR: &str = "@final struct WCh { wchar c; octet tail; };";

/// Every dialect's own `wchar`, in both encodings and both byte orders.
///
/// `vendor_bytes.md` §4, case `wchar_member`, `c = U+0061`,
/// `tail = 0xAB`. RTI writes four bytes under XCDR1 and two under
/// XCDR2; Cyclone and Fast DDS write two everywhere. Before this the
/// planner wrote two under every dialect, so `--dialect rti` produced a
/// `wchar` two bytes narrower than RTI's — which is not a wrong
/// character but a wrong offset for every member behind it, and no flag
/// reached the right answer.
#[test]
fn each_dialects_wchar_is_the_width_that_vendor_writes() {
    let sample = json!({ "c": "a", "tail": 171 });
    let ast = model_from_idl(IDL_WCHAR, "wstring.idl").expect("the model");
    let expected = [
        (Dialect::Cyclone, Encoding::PlainCdr1, "6100ab", "0061ab"),
        (Dialect::Cyclone, Encoding::PlainCdr2, "6100ab", "0061ab"),
        (
            Dialect::Rti,
            Encoding::PlainCdr1,
            "61000000ab",
            "00000061ab",
        ),
        (Dialect::Rti, Encoding::PlainCdr2, "6100ab", "0061ab"),
        (Dialect::FastDds, Encoding::PlainCdr1, "6100ab", "0061ab"),
        (Dialect::FastDds, Encoding::PlainCdr2, "6100ab", "0061ab"),
        // Table 31 says two, so the clause-by-clause dialect writes two.
        (
            Dialect::Specification,
            Encoding::PlainCdr1,
            "6100ab",
            "0061ab",
        ),
    ];
    for (dialect, encoding, little, big) in expected {
        for (endianness, want) in [(Endianness::Little, little), (Endianness::Big, big)] {
            let request = ProbeRequest::in_dialect("WCh", encoding, endianness, dialect);
            let bytes = encode(&ast, &request, &sample).expect("the probe");
            assert_eq!(
                hex(&bytes),
                want,
                "{dialect:?} / {encoding:?} / {endianness:?}"
            );
            let back = decode(&ast, &request, &bytes).expect("the read back");
            assert_eq!(back, sample, "{dialect:?} / {encoding:?} / {endianness:?}");
        }
    }
}

/// The four-byte `wchar` moves every member after it.
///
/// The width is the whole point: `@final struct { wchar c; long n; }`
/// puts `n` at offset 4 in three of the four dialects and at offset 4
/// in RTI's XCDR1 too — but with the `wchar` filling all four bytes
/// rather than two of padding, which is a different stream and not a
/// different spelling of one.
#[test]
fn the_wide_wchar_fills_the_padding_the_narrow_one_leaves() {
    let idl = "@final struct WN { wchar c; long n; };";
    let ast = model_from_idl(idl, "wstring.idl").expect("the model");
    let sample = json!({ "c": "a", "n": 7 });
    let narrow = ProbeRequest::in_dialect(
        "WN",
        Encoding::PlainCdr1,
        Endianness::Little,
        Dialect::Cyclone,
    );
    let wide =
        ProbeRequest::in_dialect("WN", Encoding::PlainCdr1, Endianness::Little, Dialect::Rti);
    assert_eq!(
        hex(&encode(&ast, &narrow, &sample).expect("the probe")),
        "6100000007000000"
    );
    assert_eq!(
        hex(&encode(&ast, &wide, &sample).expect("the probe")),
        "6100000007000000"
    );
    // Identical bytes, and that is the danger: the two padding bytes
    // Cyclone leaves are zero and RTI's high half is zero too. It shows
    // the moment the member before it is odd-sized.
    let idl = "@final struct WO { octet head; wchar c; long n; };";
    let ast = model_from_idl(idl, "wstring.idl").expect("the model");
    let sample = json!({ "head": 1, "c": "a", "n": 7 });
    let narrow = ProbeRequest::in_dialect(
        "WO",
        Encoding::PlainCdr1,
        Endianness::Little,
        Dialect::Cyclone,
    );
    let wide =
        ProbeRequest::in_dialect("WO", Encoding::PlainCdr1, Endianness::Little, Dialect::Rti);
    assert_eq!(
        hex(&encode(&ast, &narrow, &sample).expect("the probe")),
        "0100610007000000"
    );
    assert_eq!(
        hex(&encode(&ast, &wide, &sample).expect("the probe")),
        "010000006100000007000000"
    );
}

/// A `wchar` written wide and read narrow does not return a sample.
#[test]
fn a_wide_wchar_read_narrow_is_refused_and_not_a_wrong_sample() {
    let idl = "@final struct WO { octet head; wchar c; long n; };";
    let ast = model_from_idl(idl, "wstring.idl").expect("the model");
    let sample = json!({ "head": 1, "c": "a", "n": 7 });
    let wide =
        ProbeRequest::in_dialect("WO", Encoding::PlainCdr1, Endianness::Little, Dialect::Rti);
    let narrow = ProbeRequest::in_dialect(
        "WO",
        Encoding::PlainCdr1,
        Endianness::Little,
        Dialect::Cyclone,
    );
    let bytes = encode(&ast, &wide, &sample).expect("the probe");
    let error = decode(&ast, &narrow, &bytes).expect_err("a refusal");
    assert!(
        !matches!(decode(&ast, &narrow, &bytes), Ok(value) if value == sample),
        "the narrow read returned the wide sample: {error}"
    );
}

// -- Cyclone's unswapped code units in big endian -----------------------

/// The fourth form, which is Cyclone's big-endian output.
///
/// `vendor_bytes.md` §4, big-endian table: Cyclone writes
/// `00000004 61006200 ab` for `"ab"` where RTI and Fast DDS write
/// `00000004 00610062 ab`. The length word swaps and the content does
/// not. In little endian it is the specification's form byte for byte,
/// which is why the divergence needed a big-endian run to find.
#[test]
fn the_unswapped_form_is_what_cyclone_writes_in_big_endian() {
    let sample = json!({ "w": "ab", "tail": 171 });
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        assert_eq!(
            written_ordered(
                IDL,
                "WStr",
                &sample,
                encoding,
                Endianness::Big,
                WstringUnitOrder::HostUnswapped
            ),
            "0000000461006200ab",
            "{encoding:?} big endian"
        );
        // Little endian: nothing to tell apart.
        assert_eq!(
            written_ordered(
                IDL,
                "WStr",
                &sample,
                encoding,
                Endianness::Little,
                WstringUnitOrder::HostUnswapped
            ),
            written(
                IDL,
                "WStr",
                &sample,
                encoding,
                Endianness::Little,
                WstringLengthPolicy::SPECIFICATION
            ),
            "{encoding:?} little endian"
        );
    }
}

/// A code point above the BMP keeps both surrogates, unswapped.
///
/// `vendor_bytes.md` §4: Cyclone's big-endian `"𝄞"` is
/// `00000004 34d81edd ab`, the same content bytes as its little-endian
/// output.
#[test]
fn the_unswapped_form_carries_a_surrogate_pair_the_same_way() {
    let sample = json!({ "w": "\u{1d11e}", "tail": 171 });
    assert_eq!(
        written_ordered(
            IDL,
            "WStr",
            &sample,
            Encoding::PlainCdr2,
            Endianness::Big,
            WstringUnitOrder::HostUnswapped
        ),
        "0000000434d81eddab"
    );
}

/// What a conforming peer gets from Cyclone's big-endian bytes.
///
/// This is the whole reason the policy's documentation says it
/// reproduces a peer misreading its own output: the specification's
/// reader returns two well-formed characters of the right count that
/// are not the ones that were sent, and there is nothing in the frame
/// to object to.
#[test]
fn cyclones_big_endian_wstring_read_as_the_specification_is_a_different_string() {
    let cyclone = unhex("0000000461006200ab");
    let ast = model_from_idl(IDL, "wstring.idl").expect("the model");

    let spec = request(
        "WStr",
        Encoding::PlainCdr2,
        Endianness::Big,
        WstringLengthPolicy::SPECIFICATION,
    );
    assert_eq!(
        decode(&ast, &spec, &cyclone).expect("a well-formed read"),
        json!({ "w": "\u{6100}\u{6200}", "tail": 171 }),
        "the conforming read of Cyclone's bytes is no longer the wrong \
         string, which would mean the policy had stopped being needed"
    );

    let unswapped = request_ordered(
        "WStr",
        Encoding::PlainCdr2,
        Endianness::Big,
        WstringUnitOrder::HostUnswapped,
    );
    assert_eq!(
        decode(&ast, &unswapped, &cyclone).expect("the read back"),
        json!({ "w": "ab", "tail": 171 })
    );
}

/// Exactly two dialects select it, and they are the two products that
/// write it.
///
/// It used to be none, `Dialect::Cyclone` included, on the reading that
/// a default has to be the form a conforming reader gets right. Cyclone
/// leaves a `wstring`'s code units in the host's order and
/// `rosidl_typesupport_fastrtps_c` does the same, so a preset that
/// named either and wrote the stream's order could not exchange a
/// big-endian `wstring` with it at all. That reading was right about
/// defaults and wrong about where to apply it: the preset takes the
/// product's order and the **default** takes the clause's.
/// `Dialect::default` is `Dialect::Specification`, which answers
/// `WstringUnitOrder::Stream`, and
/// `zerodds_ast_layout::key::the_default_takes_no_product_departure_on_the_two_axes`
/// holds it there.
///
/// The list is exact in both directions: a third dialect taking it
/// fails here, and either of these two dropping it fails here.
#[test]
fn exactly_the_two_products_that_write_it_select_the_unswapped_form() {
    let taken: Vec<Dialect> = Dialect::ALL
        .into_iter()
        .filter(|dialect| dialect.wstring_unit_order() == WstringUnitOrder::HostUnswapped)
        .collect();
    assert_eq!(
        taken,
        vec![Dialect::Cyclone, Dialect::RosFastRtps],
        "the unswapped order is Cyclone's and rosidl_typesupport_fastrtps_c's, and nobody          else's"
    );
    // And it is reachable by name, for a caller who wants the point
    // without the rest of either preset.
    assert_eq!(
        WstringUnitOrder::parse("host-unswapped"),
        Some(WstringUnitOrder::HostUnswapped)
    );
}
