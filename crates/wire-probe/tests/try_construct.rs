// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `try_construct`, in the reflective column. DDS-XTypes 1.3 §7.2.2.7.
//!
//! Two IDL files per case: a writer's type and a reader's narrower one.
//! The bound is not on the wire — a `string<32>` and a `string<5>`
//! serialise identically — so the pair produces one payload and the
//! whole question is what the *reader* does with it.
//!
//! Every case asserts the member *after* the narrowed one as well. That
//! is the property the four behaviours stand or fall on: a reader that
//! stopped at its own bound would leave the rest of the value in the
//! stream and read the next member out of the middle of this one, which
//! is a decode that succeeds and is wrong.
//!
//! `crates/ast-layout/tests/fixtures/try_construct_behaviour.md` is the
//! same question put to Cyclone, RTI, Fast DDS, OpenDDS and omniORB.
//! OpenDDS is the only one of the five that implements USE_DEFAULT and
//! TRIM, and every expectation below is what it produced.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::canonical::text;
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{ProbeError, encode, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

/// Both encodings and both byte orders. A narrowing that worked only
/// under XCDR2 would be a narrowing nobody could rely on.
const MODES: [(Encoding, Endianness); 4] = [
    (Encoding::PlainCdr2, Endianness::Little),
    (Encoding::PlainCdr2, Endianness::Big),
    (Encoding::PlainCdr1, Endianness::Little),
    (Encoding::PlainCdr1, Endianness::Big),
];

/// Write `values` as `type_name` of `writer_idl`, read it back as the
/// same name in `reader_idl`, in every mode.
///
/// The outcome is whatever the reader made of it, rendered as canonical
/// text, or the refusal. Every mode has to agree: the behaviour is a
/// property of the two types and not of the framing.
fn narrowed(writer_idl: &str, reader_idl: &str, type_name: &str, values: &str) -> String {
    let writer = model_from_idl(writer_idl, "writer.idl").expect("the writer's model");
    let reader = model_from_idl(reader_idl, "reader.idl").expect("the reader's model");
    let sample: serde_json::Value = serde_json::from_str(values).expect("the sample");

    let mut answers: Vec<String> = Vec::new();
    for (encoding, endianness) in MODES {
        let request = ProbeRequest::new(type_name, encoding, endianness);
        let bytes = encode(&writer, &request, &sample).expect("the writer's bytes");
        answers.push(match decode(&reader, &request, &bytes) {
            Ok(value) => text(&value),
            Err(error @ ProbeError::Discarded { .. }) => format!("discarded: {error}"),
            Err(other) => format!("refused: {other}"),
        });
    }
    let first = answers[0].clone();
    for (index, answer) in answers.iter().enumerate() {
        assert_eq!(
            *answer, first,
            "mode {index} disagreed with the first: {answer}"
        );
    }
    first
}

/// A `@mutable` writer with a wide member and a reader with a narrow
/// one. Mutable because that is the extensibility OpenDDS implements
/// member-level TryConstruct for, so the expectations below are
/// comparable against the one implementation there is.
fn string_pair(annotation: &str) -> String {
    narrowed(
        "@mutable struct S { long a; string<32> m; long b; };",
        &format!("@mutable struct S {{ long a; {annotation} string<5> m; long b; }};"),
        "S",
        r#"{"a": 7, "m": "Hello World!", "b": 9}"#,
    )
}

/// §7.2.2.7, Table 11: "The default behavior unless otherwise specified
/// using the TryConstruct annotation is DISCARD." All five
/// implementations agree, and four of them have no other behaviour at
/// all.
#[test]
fn an_unannotated_member_discards_the_sample() {
    let answer = string_pair("");
    assert!(answer.starts_with("discarded:"), "{answer}");
    assert!(answer.contains("at most 5 octets"), "{answer}");
}

/// §7.3.1.2.1.12.1: "The constructed object would have O1.member set to
/// the empty string." Table 9's `STRING_TYPE` row.
#[test]
fn use_default_substitutes_table_nines_value_and_keeps_the_sample() {
    assert_eq!(
        string_pair("@try_construct(USE_DEFAULT)"),
        r#"{"a":7,"b":9,"m":""}"#
    );
}

/// §7.3.1.2.1.12.2: "The constructed object would have O1.member contain
/// the characters of O2.member that can fit on its string<5> type, that
/// is, the string 'Hello'." Measured against OpenDDS, which produces the
/// same five characters.
#[test]
fn trim_cuts_the_string_to_the_bound_and_keeps_the_sample() {
    assert_eq!(
        string_pair("@try_construct(TRIM)"),
        r#"{"a":7,"b":9,"m":"Hello"}"#
    );
}

/// §7.3.1.2.1.12: "If the annotation is specified without a value ...
/// the behavior is set to DEFAULT." Not DISCARD, which is what an
/// *absent* annotation means — the two defaults belong to different
/// things.
#[test]
fn a_bare_annotation_is_use_default_and_not_discard() {
    assert_eq!(string_pair("@try_construct"), r#"{"a":7,"b":9,"m":""}"#);
    assert!(string_pair("").starts_with("discarded:"));
}

fn sequence_pair(annotation: &str) -> String {
    narrowed(
        "@mutable struct S { long a; sequence<long, 32> m; long b; };",
        &format!("@mutable struct S {{ long a; {annotation} sequence<long, 4> m; long b; }};"),
        "S",
        r#"{"a": 7, "m": [1, 2, 3, 4, 5, 6, 7, 8], "b": 9}"#,
    )
}

/// §7.3.1.2.1.12.3: "The constructed object would have O1.member as a
/// sequence of 4 longs containing the first four elements of
/// O2.member" — the first four and not any four, so the trim is a
/// prefix.
#[test]
fn trim_cuts_a_sequence_to_its_bound_keeping_the_first_elements() {
    assert_eq!(
        sequence_pair("@try_construct(TRIM)"),
        r#"{"a":7,"b":9,"m":[1,2,3,4]}"#
    );
    assert_eq!(
        sequence_pair("@try_construct(USE_DEFAULT)"),
        r#"{"a":7,"b":9,"m":[]}"#
    );
    assert!(sequence_pair("").starts_with("discarded:"));
}

/// §7.3.1.2.1.12.4, written out as the clause writes it:
/// `sequence<@try_construct(TRIM) String5, 10>` against a writer's
/// `sequence<String16, 10>` whose first element is "Hello World".
///
/// The element's behaviour and the member's are different questions.
/// Here the sequence itself fits — one element against a bound of ten —
/// and it is the element that cannot be constructed.
#[test]
fn an_element_annotation_trims_the_element_and_not_the_collection() {
    assert_eq!(
        narrowed(
            "typedef string<16> String16;
             @mutable struct S { long a; sequence<String16, 10> m; long b; };",
            "typedef string<5> String5;
             @mutable struct S { long a; sequence<@try_construct(TRIM) String5, 10> m; long b; };",
            "S",
            r#"{"a": 7, "m": ["Hello World"], "b": 9}"#,
        ),
        r#"{"a":7,"b":9,"m":["Hello"]}"#
    );
}

/// Table 18: "If the name or value of the T2 object does not exist in
/// T1, the object cannot construct any object of type T1." The
/// member's behaviour then decides, and Table 9's `ENUM_TYPE` row is
/// what USE_DEFAULT substitutes.
#[test]
fn an_enumerator_the_reader_does_not_declare_is_a_construction_failure() {
    let pair = |annotation: &str| {
        narrowed(
            "@appendable enum K { K1, @default_literal K2, K3 };
             @mutable struct S { long a; K m; long b; };",
            &format!(
                "@appendable enum K {{ K1, @default_literal K2 }};
                 @mutable struct S {{ long a; {annotation} K m; long b; }};"
            ),
            "S",
            r#"{"a": 7, "m": "K3", "b": 9}"#,
        )
    };
    assert!(pair("").starts_with("discarded:"));
    assert_eq!(
        pair("@try_construct(USE_DEFAULT)"),
        r#"{"a":7,"b":9,"m":"K2"}"#
    );
}

/// §7.3.1.2.1.12.5's own example: a discriminator value the reader's
/// enumeration does not have, with `switch (@try_construct K)`.
///
/// The discriminator takes its default and the member that default
/// selects takes *its* default — not the writer's value. The clause's
/// prose says "O1.e1_value would be constructed from O2.e3_value", which
/// no receiver can do: §7.2.4.1 rules out "any inspection of a data
/// producer's type definition", and without one there is no way to know
/// what type the writer's member had. OpenDDS hands back `d=0 v=0` for
/// exactly this input.
#[test]
fn a_defaulted_discriminator_defaults_the_member_it_now_selects() {
    let pair = |annotation: &str| {
        narrowed(
            "@appendable enum K { K1, @default_literal K2, K3 };
             @mutable union U switch (K) { case K1: long v1; case K2: long v2; case K3: long v3; };",
            &format!(
                "@appendable enum K {{ K1, @default_literal K2 }};
                 @mutable union U switch ({annotation} K) {{ case K1: long v1; case K2: long v2; }};"
            ),
            "U",
            r#"{"_d": "K3", "v3": 42}"#,
        )
    };
    assert!(pair("").starts_with("discarded:"));
    assert_eq!(pair("@try_construct"), r#"{"_d":"K2","v2":0}"#);
}

/// A map's value slot narrows; its key slot cannot, and the frontend is
/// what refuses a key that tries. Table 17.
#[test]
fn a_map_value_trims_and_the_entry_survives() {
    assert_eq!(
        narrowed(
            "@mutable struct S { long a; map<long, string<32>, 10> m; long b; };",
            "@mutable struct S { long a; map<long, @try_construct(TRIM) string<5>, 10> m; \
             long b; };",
            "S",
            r#"{"a": 7, "m": [[1, "Hello World!"]], "b": 9}"#,
        ),
        r#"{"a":7,"b":9,"m":[[1,"Hello"]]}"#
    );
}

/// An alias states the behaviour its uses take by default.
///
/// The specification does not address it and two of the five front ends
/// refuse to parse it; the reading here is the broader one, and it costs
/// nothing because a use site that writes its own wins. See
/// `Ast::effective_try_construct`.
#[test]
fn an_alias_borne_behaviour_reaches_a_member_that_wrote_none() {
    assert_eq!(
        narrowed(
            "@mutable struct S { long a; string<32> m; long b; };",
            "@try_construct(TRIM) typedef string<5> String5;
             @mutable struct S { long a; String5 m; long b; };",
            "S",
            r#"{"a": 7, "m": "Hello World!", "b": 9}"#,
        ),
        r#"{"a":7,"b":9,"m":"Hello"}"#
    );
}

/// A trim never lands inside a character.
///
/// A `string<5>` bound counts octets, so a value of five two-octet
/// characters overruns it. Cutting at the fifth octet would leave half
/// of the third character, and half a UTF-8 sequence is not text — a
/// peer holding `char[6]` cannot store it either.
#[test]
fn a_trim_stops_on_a_character_boundary() {
    let answer = narrowed(
        "@mutable struct S { long a; string<32> m; long b; };",
        "@mutable struct S { long a; @try_construct(TRIM) string<5> m; long b; };",
        "S",
        r#"{"a": 7, "m": "äöüäö", "b": 9}"#,
    );
    // Four octets of the five the bound allows: the third character
    // would have needed a sixth.
    assert_eq!(answer, "{\"a\":7,\"b\":9,\"m\":\"\u{e4}\u{f6}\"}");
}

/// A discarded sample and a malformed stream are different outcomes.
///
/// The whole reason `ProbeError::Discarded` exists. A subscriber that
/// logged every narrowed sample as a protocol fault would drown, and one
/// that read a truncated stream as a narrowing would never notice a
/// broken peer.
#[test]
fn a_discard_and_a_malformed_stream_are_not_the_same_error() {
    let reader = model_from_idl(
        "@mutable struct S { long a; string<5> m; long b; };",
        "r.idl",
    )
    .unwrap();
    let writer = model_from_idl(
        "@mutable struct S { long a; string<32> m; long b; };",
        "w.idl",
    )
    .unwrap();
    let request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);
    let sample: serde_json::Value =
        serde_json::from_str(r#"{"a": 7, "m": "Hello World!", "b": 9}"#).unwrap();
    let bytes = encode(&writer, &request, &sample).unwrap();

    assert!(matches!(
        decode(&reader, &request, &bytes),
        Err(ProbeError::Discarded { .. })
    ));
    // The same payload with its last four bytes gone is a stream this
    // reader cannot parse at all, and it is not a `Discarded`.
    let short = &bytes[..bytes.len() - 4];
    assert!(!matches!(
        decode(&reader, &request, short),
        Err(ProbeError::Discarded { .. }) | Ok(_)
    ));
}
