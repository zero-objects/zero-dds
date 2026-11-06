// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every case in the corpus, out and back again.
//!
//! The vendor half of phase P2 needs three code generators and a Linux
//! host; this is the half that does not. It runs the whole corpus
//! through the reflective encoder and back through the reflective
//! decoder in every encoding and byte order, and asserts the value that
//! comes out is the value that went in.
//!
//! ## What it can and cannot catch
//!
//! It cannot catch a decoder that is wrong in the same way its own
//! encoder is wrong — that is exactly what a self round trip passes, and
//! it is why `tools/wireprobe/wireprobe-pairwise` and
//! `crates/ast-layout/tests/fixtures/vendor_roundtrip.md` exist.
//!
//! It catches everything else, on every machine, with no vendor
//! installed: a member dropped, a length misread, a union's discriminator
//! selecting the wrong case, an `@optional` that comes back present. Those
//! are the regressions a change to the planner or the reader introduces,
//! and they would otherwise wait for a nightly on the test host.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::canonical::text;
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

/// The cases whose negative case label a dialect can suppress, so that
/// the frame does not carry the member the sample names.
///
/// `union-discriminator-*` have no `default:`, so a suppressed label
/// writes no branch at all; `combined-everything` has one, so what goes
/// out is the `default:` branch read over the union's storage. Under a
/// dialect that suppresses, no reader of any implementation can hand the
/// named member back, and a round trip that did would be inventing it.
/// `Dialect::union_label` carries the measurement, and
/// `zerodds_wire_probe::expected::Verdict::nothing_to_read` is the same
/// exemption for the C and Python columns against the expected-bytes
/// table.
///
/// **Whether the exemption applies is asked of the dialect and not
/// assumed.** It was written as an unconditional list while
/// `Dialect::default` was `Dialect::Cyclone`; `Dialect::default` is
/// `Dialect::Specification` now, which matches the label, so these six
/// round-trip like every other case and [`exempt`] answers `false`. A
/// list that stayed unconditional would have gone on skipping six value
/// comparisons that pass — an exemption nobody needs is a hole nobody
/// sees.
///
/// The bytes were always checked: the case is encoded, decoded and
/// required to decode *at all* — only the value comparison is skipped,
/// and `union_label_match.rs` is where the frame itself is held against
/// Cyclone's own octets.
const NEGATIVE_LABEL_CASES: [&str; 6] = [
    "union-discriminator-int8",
    "union-discriminator-int8-unannotated",
    "union-discriminator-short",
    "union-discriminator-short-unannotated",
    "combined-everything",
    "combined-everything-unannotated",
];

/// Whether the dialect under test suppresses a negative narrow case
/// label, which is the one condition [`NEGATIVE_LABEL_CASES`] is exempt
/// under.
fn exempt() -> bool {
    Dialect::default().union_label()
        == zerodds_ast_layout::UnionLabelMatch::NegativeLabelUnmatchedBelow32
}

/// The encodings a case is exercised in.
///
/// All five, not the two shorthands: `@mutable` means a parameter list
/// under XCDR1 and EMHEADERs under XCDR2, and `@appendable` means a
/// delimiter, so a run that only used the plain forms never reads a
/// header at all. Under `xcdr2 le` alone the extended parameter header
/// and both byte-swap arms of the reader never execute.
const ENCODINGS: [Encoding; 5] = [
    Encoding::PlainCdr1,
    Encoding::PlCdr1,
    Encoding::PlainCdr2,
    Encoding::DelimitedCdr,
    Encoding::PlCdr2,
];

/// The six negative-label cases are compared like every other case
/// under the default dialect, and the exemption is off.
///
/// Stated as an assertion rather than left to be inferred from an empty
/// skip: [`exempt`] gates a `continue` that skips a *value* comparison,
/// and a skip that is never needed passes whatever the decoder returns.
/// This is the check that says the six are being held to their samples,
/// and it goes red the moment a default that suppresses the label comes
/// back — at which point the message names the six rather than leaving a
/// silent hole.
#[test]
fn the_negative_label_exemption_is_not_in_force_under_the_default() {
    assert!(
        !exempt(),
        "`Dialect::default` suppresses a negative narrow case label, so these six cases are \
         exempt from the value comparison: {}. That is correct under such a default and it is \
         a hole under any other, so check that the default is meant to be a dialect that \
         suppresses.",
        NEGATIVE_LABEL_CASES.join(", ")
    );
    // And the dialect that does suppress it still does, so this test is
    // measuring the default and not the disappearance of the setting.
    assert_eq!(
        Dialect::Cyclone.union_label(),
        zerodds_ast_layout::UnionLabelMatch::NegativeLabelUnmatchedBelow32,
        "`--dialect cyclone` stopped suppressing the label, which would make the assertion \
         above pass for the wrong reason"
    );
}

/// A decoder that stopped early would return a shorter sample and say
/// nothing, which is the silent-drop shape this whole suite is built
/// around. Every prefix of a real payload has to be an error.
#[test]
fn xcdr2conformance_7_4_no_prefix_of_a_payload_reads_as_a_shorter_sample() {
    let idl = "@appendable struct Inner { long x; string text; };
               @mutable struct Sample {
                   long a; Inner nested; sequence<long> ids; double tail;
               };";
    let values = serde_json::json!({
        "a": -1234567,
        "nested": { "x": 7, "text": "über" },
        "ids": [1, -2, 3],
        "tail": -2.5
    });
    let ast = model_from_idl(idl, "prefix.idl").expect("the fixture parses");

    for encoding in ENCODINGS {
        let request = ProbeRequest::new("Sample", encoding, Endianness::Little);
        let bytes = encode(&ast, &request, &values).expect("the sample serialises");
        for cut in 0..bytes.len() {
            let read = decode(&ast, &request, &bytes[..cut]);
            assert!(
                read.is_err(),
                "{encoding:?}: {cut} of {} bytes read as a sample: {}",
                bytes.len(),
                read.map(|value| text(&value)).unwrap_or_default()
            );
        }
        // And the whole thing does read, so the loop above is not
        // passing because nothing reads at all.
        assert!(decode(&ast, &request, &bytes).is_ok(), "{encoding:?}");
    }
}

/// The three vendors' `@optional` marker, written and read.
///
/// An `@optional` member of a type that is not `@mutable` is an
/// `OPT_FMEMBER`. Rule (20) of §7.4.3.5.2 gives it a BOOLEAN
/// `<is_present>` under XCDR2 and §7.4.1.2 gives it a parameter header
/// with a declared length of zero under XCDR1, and every one of the
/// three implementations writes exactly that.
///
/// Run 2026-08-09 through `byte-gate/gate`, case
/// `opt_final_present` and `opt_final_absent`, `@final struct Sample
/// { long a; @optional octet b; long c; };` with `a = -1234567`,
/// `b = 0xAB`, `c = -2`, little endian. The bytes below are the
/// vendors', pasted; what is asserted is that this project produces
/// them and reads them.
///
/// This used to be `a_vendors_zero_filled_optional_slot_is_refused_and_
/// not_misread`, which asserted a **refusal**: the reader took the
/// vendors' `<is_present>` byte and its padding for a member header
/// that did not match, called the member absent, and read the four
/// bytes of `c` out of the wrong place. Making the guess falsifiable
/// turned a wrong value into a refusal; writing the marker makes it a
/// value again, and the same twelve bytes now go out of this project
/// as well as into it.
#[test]
fn xtypes_7_4_3_5_2_the_optional_marker_is_the_one_the_vendors_write_and_read() {
    let idl = "@final struct Sample { long a; @optional octet b; long c; };";
    let ast = model_from_idl(idl, "optional.idl").expect("the fixture parses");
    let present = serde_json::json!({ "a": -1234567, "b": 171, "c": -2 });
    let absent = serde_json::json!({ "a": -1234567, "c": -2 });

    // XCDR2, rule (20): `01` then the octet at offset 5, or `00` and
    // three bytes of padding. Cyclone zeroes the padding; RTI leaves
    // `0400` and Fast DDS `0100` behind the value, which §7.4.1.1 does
    // not fix — so the write side is compared against Cyclone and all
    // three are read.
    let request = ProbeRequest::new("Sample", Encoding::PlainCdr2, Endianness::Little);
    assert_eq!(
        hex(&encode(&ast, &request, &present).expect("it serialises")),
        "7929edff01ab0000feffffff"
    );
    assert_eq!(
        hex(&encode(&ast, &request, &absent).expect("it serialises")),
        "7929edff00000000feffffff"
    );
    for vendor in [
        "7929edff01ab0000feffffff", // cyclone
        "7929edff01ab0400feffffff", // rti
        "7929edff01ab0100feffffff", // fastdds
    ] {
        assert_eq!(
            text(&decode(&ast, &request, &bytes_of(vendor)).expect("the vendor bytes read")),
            r#"{"a":-1234567,"b":171,"c":-2}"#,
            "{vendor}"
        );
    }
    assert_eq!(
        text(
            &decode(&ast, &request, &bytes_of("7929edff00000000feffffff"))
                .expect("the vendor bytes read")
        ),
        r#"{"a":-1234567,"c":-2}"#
    );

    // XCDR1, §7.4.1.2: the parameter header either way. The three
    // differ in the header form and in what the length counts, and the
    // three dialects are those three answers — Cyclone the extended
    // form with the value's true length, RTI the short form with the
    // length rounded to four, Fast DDS the short form with the true
    // length.
    for (dialect, present_hex, absent_hex) in [
        (
            Dialect::Cyclone,
            "7929edff017f08000100000001000000ab000000feffffff",
            "7929edff017f08000100000000000000feffffff",
        ),
        (
            Dialect::Rti,
            "7929edff01000400ab000000feffffff",
            "7929edff01000000feffffff",
        ),
        (
            Dialect::FastDds,
            "7929edff01000100ab000000feffffff",
            "7929edff01000000feffffff",
        ),
    ] {
        let request =
            ProbeRequest::in_dialect("Sample", Encoding::PlainCdr1, Endianness::Little, dialect);
        assert_eq!(
            hex(&encode(&ast, &request, &present).expect("it serialises")),
            present_hex,
            "{dialect:?} present"
        );
        assert_eq!(
            hex(&encode(&ast, &request, &absent).expect("it serialises")),
            absent_hex,
            "{dialect:?} absent"
        );
        assert_eq!(
            text(&decode(&ast, &request, &bytes_of(present_hex)).expect("the vendor bytes read")),
            r#"{"a":-1234567,"b":171,"c":-2}"#,
            "{dialect:?} present"
        );
        assert_eq!(
            text(&decode(&ast, &request, &bytes_of(absent_hex)).expect("the vendor bytes read")),
            r#"{"a":-1234567,"c":-2}"#,
            "{dialect:?} absent"
        );
    }
}

/// A `@mutable` union's discriminator and selected member, each behind
/// the `MMEMBER` header rules (27) and (28) give it.
///
/// Run 2026-08-09 through `byte-gate/gate`, case `mut_union`:
///
/// ```text
/// @mutable union Choice switch (long) { case 1: long whole;
///                                       default: double frac; };
/// @final struct Sample { Choice picked; long tail; };
/// ```
///
/// `_d = 1`, `whole = 7`, `tail = -2`, little endian. Cyclone's `idlc`
/// refuses the type — "Mutable unions are not supported yet" — so RTI
/// and Fast DDS are the two witnesses, and they agree byte for byte
/// under XCDR2 and differ only in the sentinel's must-understand bit
/// under XCDR1.
///
/// This project wrote `08000000 01000000 07000000`: a DHEADER over an
/// undelimited discriminator and member, with no EMHEADERs, no
/// parameter headers and not even the sentinel its own planner
/// computed. A peer parsing those bytes as PL_CDR2 reads `01000000` as
/// an EMHEADER with LC0 and hands back a one-byte member, with no error
/// anywhere.
#[test]
fn xtypes_7_4_1_2_3_a_mutable_union_frames_its_discriminator_and_its_member() {
    let idl = "@mutable union Choice switch (long) {
                   case 1: long whole;
                   default: double frac;
               };
               @final struct Sample { Choice picked; long tail; };";
    let ast = model_from_idl(idl, "mut_union.idl").expect("the fixture parses");
    let value = serde_json::json!({ "picked": { "_d": 1, "whole": 7 }, "tail": -2 });

    // XCDR2, rule (27). The discriminator is member id 0 at LC2 with
    // `M_FLAG` clear, then the selected member at its own id.
    // §7.2.2.4.4.4.6 says the discriminator's `must_understand` is
    // always true and neither implementation sets it; the measurement
    // decides, and the inventory's J7 records the departure.
    let xcdr2 = "1000000000000020010000000100002007000000feffffff";
    // XCDR1, rule (28). RTI's sentinel carries the must-understand bit
    // and Fast DDS's does not, which is the only difference.
    let rti = "00000400010000000100040007000000027f0000feffffff";
    let fastdds = "00000400010000000100040007000000023f0000feffffff";

    for (dialect, encoding, expected) in [
        (Dialect::Rti, Encoding::PlainCdr2, xcdr2),
        (Dialect::FastDds, Encoding::PlainCdr2, xcdr2),
        (Dialect::Rti, Encoding::PlainCdr1, rti),
        (Dialect::FastDds, Encoding::PlainCdr1, fastdds),
    ] {
        let request = ProbeRequest::in_dialect("Sample", encoding, Endianness::Little, dialect);
        assert_eq!(
            hex(&encode(&ast, &request, &value).expect("it serialises")),
            expected,
            "{dialect:?} {encoding:?}"
        );
        assert_eq!(
            text(&decode(&ast, &request, &bytes_of(expected)).expect("the vendor bytes read")),
            r#"{"picked":{"_d":1,"whole":7},"tail":-2}"#,
            "{dialect:?} {encoding:?}"
        );
    }
}

/// `FLAG_MUST_UNDERSTAND` under XCDR1, written and read.
///
/// Run 2026-08-09 through `byte-gate/gate`, case
/// `mut_mustunderstand`:
///
/// ```text
/// @mutable struct M { @id(1) @must_understand long a; @id(2) long b; };
/// ```
///
/// `a = 7`, `b = -2`, little endian. §7.4.1.2.2: the flag "shall be set
/// if and only if the must_understand property of the member is set to
/// true". Cyclone writes `FLAG_2` of the extended `eMemberHeader` —
/// `01000040` for `a` against `02000000` for `b` — and RTI writes bit 14
/// of the short id, `01400400`. Fast DDS writes the bare id.
///
/// This project wrote no flag at all, in either form.
#[test]
fn the_xcdr1_must_understand_bit_is_the_one_the_vendors_write() {
    let idl = "@mutable struct M { @id(1) @must_understand long a; @id(2) long b; };";
    let ast = model_from_idl(idl, "mustunderstand.idl").expect("the fixture parses");
    let value = serde_json::json!({ "a": 7, "b": -2 });

    for (dialect, expected) in [
        (
            Dialect::Cyclone,
            "017f0800010000400400000007000000017f08000200000004000000feffffff027f0000",
        ),
        (Dialect::Rti, "014004000700000002000400feffffff027f0000"),
        // Fast DDS does not write the member's bit. The clause and two
        // implementations say it should; this dialect exists to
        // reproduce the third, and `PlCdr1HeaderPolicy` carries the
        // argument.
        (Dialect::FastDds, "010004000700000002000400feffffff023f0000"),
    ] {
        let request = ProbeRequest::in_dialect("M", Encoding::PlCdr1, Endianness::Little, dialect);
        assert_eq!(
            hex(&encode(&ast, &request, &value).expect("it serialises")),
            expected,
            "{dialect:?}"
        );
        assert_eq!(
            text(&decode(&ast, &request, &bytes_of(expected)).expect("the vendor bytes read")),
            r#"{"a":7,"b":-2}"#,
            "{dialect:?}"
        );
    }
}

/// A member whose `eMemberHeader` carries `FLAG_2` is the member, not a
/// member with an id four hundred million higher.
///
/// §7.4.1.2.1 splits the extended header into `FLAG_1` to `FLAG_4` and a
/// 28-bit id. Reading the whole word as the id gave Cyclone's
/// `@must_understand` member an id of `0x40000001`, which matches
/// nothing the model declares — and under XCDR1 an unmatched parameter
/// was skipped, so the member was **silently dropped** and the sample
/// came back without it.
///
/// The bytes below are Cyclone's own, and the assertion is that `a`
/// arrives.
#[test]
fn a_cyclone_must_understand_member_is_not_dropped_over_its_flag_bit() {
    let idl = "@mutable struct M { @id(1) @must_understand long a; @id(2) long b; };";
    let ast = model_from_idl(idl, "mustunderstand.idl").expect("the fixture parses");
    let cyclone =
        bytes_of("017f0800010000400400000007000000017f08000200000004000000feffffff027f0000");
    let request =
        ProbeRequest::in_dialect("M", Encoding::PlCdr1, Endianness::Little, Dialect::Cyclone);
    assert_eq!(
        text(&decode(&ast, &request, &cyclone).expect("the vendor bytes read")),
        r#"{"a":7,"b":-2}"#
    );
}

/// A must-understand parameter this type does not declare discards the
/// sample; one without the bit is skipped and the sample kept.
///
/// §7.2.2.4.4.4.6. The XCDR2 half of this already refused; the XCDR1
/// half stepped over the parameter and said nothing, which is the same
/// stream read two ways depending only on the encoding version.
///
/// The two payloads are this project's own `@mutable struct Wide`,
/// written with a member the reader's `Narrow` does not declare — the
/// evolution shape §7.4 exists for, and the one no vendor row carries.
#[test]
fn xtypes_7_2_2_4_4_4_6_an_unknown_must_understand_parameter_discards_the_sample_under_xcdr1() {
    let wide = model_from_idl(
        "@mutable struct S { @id(1) long a; @id(9) @must_understand long extra; };",
        "wide.idl",
    )
    .expect("the fixture parses");
    let skippable = model_from_idl(
        "@mutable struct S { @id(1) long a; @id(9) long extra; };",
        "skippable.idl",
    )
    .expect("the fixture parses");
    let narrow =
        model_from_idl("@mutable struct S { @id(1) long a; };", "narrow.idl").expect("it parses");
    let value = serde_json::json!({ "a": 7, "extra": -2 });

    for encoding in [Encoding::PlCdr1, Encoding::PlCdr2] {
        let request = ProbeRequest::new("S", encoding, Endianness::Little);

        let refused = encode(&wide, &request, &value).expect("it serialises");
        assert!(
            decode(&narrow, &request, &refused).is_err(),
            "{encoding:?}: a must-understand member this type does not declare was skipped"
        );

        let kept = encode(&skippable, &request, &value).expect("it serialises");
        assert_eq!(
            text(&decode(&narrow, &request, &kept).expect("the sample reads")),
            r#"{"a":7}"#,
            "{encoding:?}"
        );
    }
}

/// Three parameters no user member owns: vendor-private, `PID_IGNORE`
/// and one from the reserved range.
///
/// XTypes 1.3 §7.4.1.2.2 gives bit 15 of a short parameter id to
/// `FLAG_IMPL_EXTENSION` and says user-defined members "will never set"
/// it, reserves 0x3F01 to 0x3FFF, and says of 0x3F03 that "all consumers
/// of this Data Representation shall ignore parameters with this ID".
///
/// The stream is hand-built because no vendor writes one: it is the
/// shape a *peer's* private extension takes on a topic this type also
/// subscribes to. All three parameters used to reach the id chain with
/// their flag bits masked off, so `0x8002` was read as member id 2 and a
/// stranger's four octets became `b`'s value — with no error, and with
/// the right length, so nothing downstream could tell.
#[test]
fn xtypes_7_4_1_2_1_a_vendor_private_or_reserved_parameter_is_not_a_member_of_this_type() {
    let idl = "@mutable struct M { @id(1) long a; @id(2) long b; };";
    let ast = model_from_idl(idl, "reserved.idl").expect("the fixture parses");
    let request = ProbeRequest::in_dialect("M", Encoding::PlCdr1, Endianness::Little, Dialect::Rti);

    // `a` = 7, then a parameter carrying FLAG_IMPL_EXTENSION over the
    // low bits of member id 2 with `0xdeadbeef` behind it, then the
    // sentinel. `b` must take Table 9's zero and not the intruder.
    let vendor_private = bytes_of("010004000700000002800400efbeadde027f0000");
    assert_eq!(
        text(&decode(&ast, &request, &vendor_private).expect("the sample reads")),
        r#"{"a":7,"b":0}"#
    );

    // `PID_IGNORE` with the must-understand bit set. The clause states
    // no exception, so it is ignored rather than discarding the sample —
    // which is what the unknown-parameter path would have done.
    let with_ignore = bytes_of("0100040007000000037f0400112233440200040009000000027f0000");
    assert_eq!(
        text(&decode(&ast, &request, &with_ignore).expect("PID_IGNORE is ignored")),
        r#"{"a":7,"b":9}"#
    );

    // A reserved id above `PID_IGNORE`, without the flag: skipped like
    // any parameter that is not a member, and the sample kept.
    let reserved = bytes_of("01000400070000000c3f0400112233440200040009000000027f0000");
    assert_eq!(
        text(&decode(&ast, &request, &reserved).expect("a reserved id is skipped")),
        r#"{"a":7,"b":9}"#
    );

    // The same reserved id *with* must-understand: not a member of this
    // type, so §7.2.2.4.4.4.6 applies and the sample goes.
    let reserved_mu = bytes_of("01000400070000000c7f0400112233440200040009000000027f0000");
    assert!(
        decode(&ast, &request, &reserved_mu).is_err(),
        "a must-understand parameter that is no member of this type was kept"
    );
}

/// The bytes of a hex string, which is how the vendor runs are recorded.
fn bytes_of(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "an odd hex string: {hex}");
    (0..hex.len() / 2)
        .map(|index| {
            u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                .unwrap_or_else(|error| panic!("{hex}: {error}"))
        })
        .collect()
}
