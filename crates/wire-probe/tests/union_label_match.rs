// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **Counter-check.** Which branch a union writes is answered by the
//! plan's [`UnionLabelMatch`] and by nothing else, in every place that
//! selects one.
//!
//! # What is measured
//!
//! `tests/interop-results/sources/vendor-corpus.tsv`, the four
//! `union-discriminator-{int8,short}` cases and their unannotated twins,
//! four implementations, both encoding versions, both byte orders, plus
//! `combined-everything` and its twin for the half with a `default:`.
//!
//! ```text
//! union-discriminator-short  xcdr1 le  cyclone   c7cf0000 ffffffff
//!                                      rti       c7cf0000 7929edff ffffffff
//!                                      fastdds   c7cf0000 7929edff ffffffff
//!                                      opendds   c7cf0000 7929edff ffffffff
//! ```
//!
//! Nothing of that is hard-coded here. Each recorded row is swept
//! against `Dialect::ALL × UnionLabelMatch::ALL` and the coordinate is
//! read off the combinations that reproduce it:
//!
//! - every Cyclone row is reproduced, and **only** under
//!   [`UnionLabelMatch::NegativeLabelUnmatchedBelow32`];
//! - every RTI, Fast DDS and OpenDDS row is reproduced, and **only**
//!   under [`UnionLabelMatch::SignExtended`].
//!
//! Both directions, so the axis is pinned rather than merely reachable.
//!
//! # The two halves, and why they are one rule
//!
//! Where the union declares no `default:`, no label matches and no
//! branch is written — four octets shorter than the clause.
//! [`the_branchless_half_is_told_apart`] is the sixteen rows of that.
//!
//! Where it declares one, Cyclone falls into it and writes the union's
//! **storage** read through that branch.
//! [`the_storage_is_not_the_member_zero_padded`] is the eight rows of
//! that, and it is the check that separates the storage overlay from the
//! obvious wrong model: big endian is the same eight octets reversed
//! *whole*, `00000000 7fffffff`, where a member zero-padded to the
//! branch's width would give `7fffffff 00000000`.
//!
//! # Mutation-checked, at the crossing and at the coordinate
//!
//! Eight mutations were run against this file, each applied to a copy of
//! the tree and reverted from that copy. What each one turns red is
//! recorded, including the one that only the source check catches — a
//! mutation whose colour is not written down is a mutation somebody has
//! to run again.
//!
//! | mutation | what goes red |
//! |---|---|
//! | **value** — `UnionLabelMatch::label_matches` returning `true` for `NegativeLabelUnmatchedBelow32` | five: [`the_branchless_half_is_told_apart`], [`the_storage_is_not_the_member_zero_padded`], [`the_reader_takes_the_writers_setting`], [`the_pair_it_reaches_is_the_narrow_signed_one`], [`the_shape_with_no_storage_is_left_to_the_clause`] |
//! | **the axis's own arms swapped** | the same five |
//! | **the coordinate** — `discriminator_signedness` giving `Int32` a width of 16, so the rule reaches `long` | [`the_pair_it_reaches_is_the_narrow_signed_one`], [`the_wider_and_the_unsigned_discriminators_are_untouched`], [`the_shape_with_no_storage_is_left_to_the_clause`] |
//! | **the coordinate, the other way** — giving `UInt16` a signed answer | [`the_pair_it_reaches_is_the_narrow_signed_one`] and [`the_shape_with_no_storage_is_left_to_the_clause`]; no byte check can see it, because no union can declare a negative label on an unsigned discriminator |
//! | **the storage half** — `union_negative_matches_nothing` dropping its branch check | [`the_shape_with_no_storage_is_left_to_the_clause`] **only**, which is what that check exists for |
//! | **the overlay** — `write_selected_branch` writing the member zero-padded in the stream's order instead of the storage as one value | [`the_storage_is_not_the_member_zero_padded`] and [`the_round_trip_is_stable_under_the_rule`]. [`the_branchless_half_is_told_apart`] stays **green**, which is the separation this file exists for: the sixteen rows with no `default:` write no branch at all and say nothing about what a branch would have held |
//! | **the reader** — `select_case` taking `false` for its suppression | [`the_reader_takes_the_writers_setting`], [`the_round_trip_is_stable_under_the_rule`], [`the_sites_go_through_the_crossing`] |
//! | **the C column's dump** — `wireprobe-zerodds-c-gen`'s `dump.rs` reading the labels straight | [`the_sites_go_through_the_crossing`] **only** |
//!
//! The last row is a finding and not a gap in this file. That site's
//! bypass changes no octet in either direction — the C column's *codec*
//! still falls into `default:` — and only the JSON the read direction
//! prints names the wrong member. That comparison lives in
//! `three_column_gate.rs`, which no test here runs. Checked, recorded,
//! and the reason the source check is not redundant.
//!
//! A first attempt at the overlay mutation reversed the eight octets
//! whole and **survived**, which is the same identity the measurement
//! turns on: `from_be_bytes(reverse(le))` is `from_le_bytes(le)`. It is
//! written down because a mutation that survives for an arithmetic
//! reason is not evidence that the check is weak, and the next reader
//! would otherwise reach for it again.
//!
//! # The shape the rule does not reach, and how that was found
//!
//! A union branch with no fixed-width storage — a `string`, a
//! `sequence`, a nested aggregate — has nothing a probe can overlay: the
//! peer reads a *pointer* out of the union's storage. The corpus has
//! two such unions, `union-member-sequence` and its twin, and it says
//! what the peer does with them:
//! `tests/interop-results/sources/vendor-corpus.tsv` records Cyclone on
//! both, in both encoding versions and both byte orders, as
//! `refused — the probe was killed: Segmentation fault`. Their `-1` is
//! the union's only label, so under the rule every discriminator value
//! falls into `default:` and there is no frame at all to reproduce.
//!
//! So the crossing has a **storage half**: it answers false for a union
//! any of whose branches is not a primitive of one, two, four or eight
//! octets, and this project writes the clause's frame there — a
//! complete frame a conforming reader can use.
//! [`the_shape_with_no_storage_is_left_to_the_clause`] is that
//! restriction, and it reads the recorded refusals rather than quoting
//! them.
//!
//! **The first version of that check swept the six cases named above,
//! found what it was given, and passed.** It was a check that could not
//! fail: it never looked at the seventh and eighth narrow-signed union
//! in the corpus. The three-column gate found them, in the only place
//! they were visible — the three columns refused in three different
//! classes. The check reads the corpus directory now.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{
    DepCollector, Dialect, Encoding, Endianness, LayoutKey, Purpose, UnionLabelMatch, plan,
    union_negative_matches_nothing,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

// The four cases whose union has a negative label on a narrow signed
// discriminator and **no** `default:`.
//
// `union-discriminator-long` and `union-discriminator-longlong` are
// deliberately not here, and neither are the unsigned ones: every
// implementation agrees on those, so they cannot separate the two
// values of this axis. They are checked from the other side, in
// [`the_wider_and_the_unsigned_discriminators_are_untouched`].
const BRANCHLESS_CASES: [&str; 4] = [
    "union-discriminator-int8",
    "union-discriminator-int8-unannotated",
    "union-discriminator-short",
    "union-discriminator-short-unannotated",
];

// The two cases whose union has a negative label on a `short` **and** a
// `default:`.
const DEFAULTED_CASES: [&str; 2] = ["combined-everything", "combined-everything-unannotated"];

// The discriminators every implementation agrees on, with the case that
// carries each.
//
// Two are negative labels on a wider signed discriminator and three are
// labels on an unsigned one; between them they are the other three
// quadrants of (signed, narrow).
const AGREED_CASES: [&str; 5] = [
    "union-discriminator-long",
    "union-discriminator-longlong",
    "union-discriminator-octet",
    "union-discriminator-uint8",
    "union-discriminator-ushort",
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

// Every recorded `wrote` row of `vendor-corpus.tsv` for the cases this
// file measures, keyed by (case, system, version, order).
fn recorded() -> BTreeMap<(String, String, String, String), String> {
    let mut rows = BTreeMap::new();
    let path = repository_root().join("tests/interop-results/sources/vendor-corpus.tsv");
    let text = std::fs::read_to_string(path).expect("tests/interop-results/sources");
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        // case, system, version, endian, outcome, payload. A row that is
        // not `wrote` is a refusal — RTI's and OpenDDS's front ends on
        // `combined-everything` — and is left out rather than parsed as
        // octets.
        if f.len() < 6 || f[4] != "wrote" {
            continue;
        }
        let interesting = BRANCHLESS_CASES.contains(&f[0])
            || DEFAULTED_CASES.contains(&f[0])
            || AGREED_CASES.contains(&f[0]);
        if !interesting {
            continue;
        }
        rows.insert(
            (
                f[0].to_string(),
                f[1].to_string(),
                f[2].to_string(),
                f[3].to_string(),
            ),
            f[5].to_string(),
        );
    }
    rows
}

// One point of the space a union sample sits in: which peer's answers
// to everything else, and how a negative label matches.
type Combination = (Dialect, UnionLabelMatch);

fn combinations() -> Vec<Combination> {
    let mut out = Vec::new();
    for dialect in Dialect::ALL {
        for rule in UnionLabelMatch::ALL {
            out.push((dialect, rule));
        }
    }
    out
}

fn encoding_of(version: &str) -> Encoding {
    if version == "xcdr1" {
        Encoding::PlainCdr1
    } else {
        Encoding::PlainCdr2
    }
}

fn endianness_of(endian: &str) -> Endianness {
    if endian == "be" {
        Endianness::Big
    } else {
        Endianness::Little
    }
}

struct CaseFiles {
    idl: String,
    type_name: String,
    values: serde_json::Value,
}

fn case_files(case: &str) -> CaseFiles {
    let directory = repository_root().join("tests/interop-corpus").join(case);
    let read = |file: &str| {
        std::fs::read_to_string(directory.join(file))
            .unwrap_or_else(|error| panic!("{case}/{file}: {error}"))
    };
    let values = read("values.json");
    CaseFiles {
        idl: read("type.idl"),
        type_name: read("type").trim().to_string(),
        values: serde_json::from_str(&values)
            .unwrap_or_else(|error| panic!("{case}/values.json: {error}")),
    }
}

// What this project writes for one corpus case under one combination.
fn written(
    case: &str,
    encoding: Encoding,
    endianness: Endianness,
    combination: Combination,
) -> Option<String> {
    let files = case_files(case);
    // The model is lowered in the dialect too: member numbering is a
    // front-end answer and not a plan's, so a model lowered once would
    // give every column Cyclone's member ids.
    let ast =
        model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), combination.0).ok()?;
    let mut request =
        ProbeRequest::in_dialect(&files.type_name, encoding, endianness, combination.0);
    request.union_label = combination.1;
    encode(&ast, &request, &files.values).ok().map(|b| hex(&b))
}

// Which combinations reproduce a recorded row exactly.
fn matching(case: &str, version: &str, endian: &str, theirs: &str) -> Vec<Combination> {
    let encoding = encoding_of(version);
    let endianness = endianness_of(endian);
    combinations()
        .into_iter()
        .filter(|combination| {
            written(case, encoding, endianness, *combination).as_deref() == Some(theirs)
        })
        .collect()
}

// The sixteen rows with no `default:`: the two rules are told apart by
// the recorded octets, in both directions.
//
// Cyclone drops the branch and the other three write it, so a row of
// one is reproduced only under one value of the axis and a row of the
// others only under the other. That is what makes this a measurement of
// the axis rather than of the corpus: a build where the rule never
// reaches the writer reproduces one side and nothing of the other.
#[test]
fn the_branchless_half_is_told_apart() {
    let rows = recorded();
    assert!(
        !rows.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check must not \
         pass by finding nothing"
    );
    let mut cyclone = 0usize;
    let mut others = 0usize;
    for case in BRANCHLESS_CASES {
        for (system, expected) in [
            ("cyclone", UnionLabelMatch::NegativeLabelUnmatchedBelow32),
            ("rti", UnionLabelMatch::SignExtended),
            ("fastdds", UnionLabelMatch::SignExtended),
            ("opendds", UnionLabelMatch::SignExtended),
        ] {
            for version in ["xcdr1", "xcdr2"] {
                for endian in ["le", "be"] {
                    let key = (
                        case.to_string(),
                        system.to_string(),
                        version.to_string(),
                        endian.to_string(),
                    );
                    let Some(theirs) = rows.get(&key) else {
                        panic!("{case} {system} {version} {endian} is not in vendor-corpus.tsv");
                    };
                    let found = matching(case, version, endian, theirs);
                    assert!(
                        !found.is_empty(),
                        "{case} {system} {version} {endian}: none of the {} combinations \
                         reproduce {theirs}",
                        combinations().len()
                    );
                    for (dialect, rule) in &found {
                        assert_eq!(
                            *rule,
                            expected,
                            "{case} {system} {version} {endian}: reproduced under {}/{} — the \
                             label rule is not pinned by these octets",
                            dialect.as_str(),
                            rule.as_str()
                        );
                    }
                    if system == "cyclone" {
                        cyclone += 1;
                    } else {
                        others += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cyclone, 16, "Cyclone rows compared: {cyclone}");
    assert_eq!(
        others, 48,
        "RTI, Fast DDS and OpenDDS rows compared: {others}"
    );
}

// The eight rows with a `default:`: what Cyclone writes there is the
// union's storage and **not** the selected member zero-padded.
//
// The two models differ in exactly one place — big endian — because a
// little-endian stream cannot tell them apart. Cyclone's
// `combined-everything-unannotated` big-endian octets carry
// `00000000 7fffffff` where the padded member would be
// `7fffffff 00000000`, and this asserts both halves: that the recorded
// row is reproduced under the rule, and that the padded model is a
// different byte string.
#[test]
fn the_storage_is_not_the_member_zero_padded() {
    let rows = recorded();
    let mut compared = 0usize;
    for case in DEFAULTED_CASES {
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let key = (
                    case.to_string(),
                    String::from("cyclone"),
                    version.to_string(),
                    endian.to_string(),
                );
                let Some(theirs) = rows.get(&key) else {
                    panic!("{case} cyclone {version} {endian} is not in vendor-corpus.tsv");
                };
                let found = matching(case, version, endian, theirs);
                assert!(
                    !found.is_empty(),
                    "{case} cyclone {version} {endian}: none of the {} combinations reproduce \
                     {theirs}",
                    combinations().len()
                );
                for (dialect, rule) in &found {
                    assert_eq!(
                        *rule,
                        UnionLabelMatch::NegativeLabelUnmatchedBelow32,
                        "{case} cyclone {version} {endian}: reproduced under {}/{}",
                        dialect.as_str(),
                        rule.as_str()
                    );
                }
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 8, "rows with a `default:` compared: {compared}");

    // The separation, stated as octets rather than as a sentence. The
    // union is `case -1: long whole; default: double fraction;` with
    // `whole = 0x7fffffff`; the storage read through the `double` is
    // `000000007fffffff`, and the member zero-padded would be
    // `7fffffff00000000`.
    let key = (
        String::from("combined-everything-unannotated"),
        String::from("cyclone"),
        String::from("xcdr1"),
        String::from("be"),
    );
    let theirs = rows.get(&key).expect("the big-endian row");
    assert!(
        theirs.contains("000000007fffffff"),
        "the recorded row no longer carries the storage overlay: {theirs}"
    );
    assert!(
        !theirs.contains("7fffffff00000000"),
        "the recorded row carries the member zero-padded, which is a different measurement: \
         {theirs}"
    );
}

// The reader takes the writer's setting and finds the form, rather than
// matching a label the writer suppressed.
//
// Cyclone's own octets, through this project's decoder under the rule
// its bytes were compared against. The union with no `default:` hands
// back the discriminator alone; the one with a `default:` hands back
// the **default** branch, which is the branch the octets carry.
#[test]
fn the_reader_takes_the_writers_setting() {
    let rows = recorded();
    for (case, member) in [
        ("union-discriminator-short", None),
        ("combined-everything-unannotated", Some("fraction")),
    ] {
        for endian in ["le", "be"] {
            let key = (
                case.to_string(),
                String::from("cyclone"),
                String::from("xcdr1"),
                endian.to_string(),
            );
            let theirs = rows.get(&key).expect("a recorded Cyclone row");
            let bytes = unhex(theirs);
            let files = case_files(case);
            let ast = model_from_idl_in_dialect(
                &files.idl,
                &format!("{case}/type.idl"),
                Dialect::Cyclone,
            )
            .expect("the case lowers");
            let mut request = ProbeRequest::in_dialect(
                &files.type_name,
                Encoding::PlainCdr1,
                endianness_of(endian),
                Dialect::Cyclone,
            );
            request.union_label = UnionLabelMatch::NegativeLabelUnmatchedBelow32;
            let value = decode(&ast, &request, &bytes)
                .unwrap_or_else(|error| panic!("{case} {endian}: {error}"));
            let picked = pick(&value, case);
            assert_eq!(
                picked.get("_d").and_then(serde_json::Value::as_i64),
                Some(if case == "union-discriminator-short" {
                    -12345
                } else {
                    -1
                }),
                "{case} {endian}: the discriminator did not survive"
            );
            match member {
                None => assert_eq!(
                    picked.len(),
                    1,
                    "{case} {endian}: a branch came back out of a stream that carries none — {picked:?}"
                ),
                Some(name) => {
                    assert!(
                        picked.contains_key(name),
                        "{case} {endian}: the `default:` branch is not what came back — {picked:?}"
                    );
                    assert_eq!(
                        picked.len(),
                        2,
                        "{case} {endian}: more than the discriminator and one branch — {picked:?}"
                    );
                }
            }
        }
    }
}

// The union member of a decoded sample, whichever case it is.
fn pick<'a>(
    value: &'a serde_json::Value,
    case: &str,
) -> &'a serde_json::Map<String, serde_json::Value> {
    value
        .get("picked")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{case}: the sample has no `picked` member"))
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("a hex payload")
        })
        .collect()
}

// The rule reaches a signed integer narrower than 32 bits and nothing
// else.
//
// Every discriminator the corpus declares, asked of the crossing
// directly. `octet`, `char` and `boolean` cannot carry a negative label
// at all; `long` and `long long` were measured to match one; an
// enumeration discriminator answers `false` whatever the policy,
// because `union-discriminator-enum` declares no negative `@value` and
// nothing measured says what Cyclone does with one.
#[test]
fn the_pair_it_reaches_is_the_narrow_signed_one() {
    const DISCRIMINATORS: [(&str, bool); 12] = [
        ("union-discriminator-int8", true),
        ("union-discriminator-short", true),
        ("union-discriminator-long", false),
        ("union-discriminator-longlong", false),
        ("union-discriminator-octet", false),
        ("union-discriminator-uint8", false),
        ("union-discriminator-ushort", false),
        ("union-discriminator-ulong", false),
        ("union-discriminator-ulonglong", false),
        ("union-discriminator-boolean", false),
        ("union-discriminator-char", false),
        ("union-discriminator-enum", false),
    ];
    for (case, expected) in DISCRIMINATORS {
        let files = case_files(case);
        let ast =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::Cyclone)
                .expect("the case lowers");
        let union_ = union_of(&ast, case);
        assert_eq!(
            union_negative_matches_nothing(
                &ast,
                union_,
                UnionLabelMatch::NegativeLabelUnmatchedBelow32
            ),
            expected,
            "{case}: the crossing answers the wrong way for this discriminator"
        );
        // And under the default it is false for every one of them, which
        // is what makes the default a no-op rather than a second rule.
        assert!(
            !union_negative_matches_nothing(&ast, union_, UnionLabelMatch::SignExtended),
            "{case}: the default suppresses a label"
        );
    }
}

// The union inside a corpus case.
fn union_of(ast: &zerodds_ast::Ast, case: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    for (id, node) in ast.types() {
        if matches!(node.kind, zerodds_ast::TypeKind::Union { .. }) {
            return id;
        }
    }
    panic!("{case}: no union in the model")
}

// The cases every implementation agrees on write the same octets under
// both values of the axis.
//
// The other side of [`the_branchless_half_is_told_apart`]: a setting
// that reached a wider or an unsigned discriminator would change rows
// four implementations agree on, and that is the shape of a silent
// wrong value rather than of a reproduction.
#[test]
fn the_wider_and_the_unsigned_discriminators_are_untouched() {
    let mut compared = 0usize;
    for case in AGREED_CASES {
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let encoding = encoding_of(version);
                let endianness = endianness_of(endian);
                let clause = written(
                    case,
                    encoding,
                    endianness,
                    (Dialect::Cyclone, UnionLabelMatch::SignExtended),
                );
                let cyclone = written(
                    case,
                    encoding,
                    endianness,
                    (
                        Dialect::Cyclone,
                        UnionLabelMatch::NegativeLabelUnmatchedBelow32,
                    ),
                );
                assert!(clause.is_some(), "{case} {version} {endian}: no bytes");
                assert_eq!(
                    clause, cyclone,
                    "{case} {version} {endian}: the rule reached a discriminator it must not"
                );
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 20, "agreed rows compared: {compared}");
}

// `Dialect::Cyclone` answers `NegativeLabelUnmatchedBelow32` and every
// other dialect answers the clause.
//
// It used to be all ten, on the reading that a default has to be the
// form a conforming reader gets right — and the rule does drop the
// selected branch off the wire. That is what Cyclone puts there, and a
// preset that named Cyclone and wrote a branch could not exchange
// `union-discriminator-short` with it. The reading was right about
// defaults and wrong about where to apply it: `Dialect::Cyclone` was
// `Dialect::default` at the time, so moving the rule into the preset
// moved it into the default with it. `Dialect::default` is
// `Dialect::Specification` now and answers the clause, so a caller who
// names no dialect gets the branch.
//
// Exact in both directions: a second dialect taking the rule fails
// here, and Cyclone dropping it fails here.
#[test]
fn the_default_is_the_clause() {
    let taken: Vec<Dialect> = Dialect::ALL
        .into_iter()
        .filter(|dialect| dialect.union_label() == UnionLabelMatch::NegativeLabelUnmatchedBelow32)
        .collect();
    assert_eq!(
        taken,
        vec![Dialect::Cyclone],
        "the suppressed narrow negative label is Cyclone's and nobody else's"
    );
    assert_eq!(
        Dialect::Specification.union_label(),
        UnionLabelMatch::SignExtended,
        "the clause-by-clause dialect may not carry a vendor's departure"
    );
    assert!(UnionLabelMatch::default().is_default());
    for rule in UnionLabelMatch::ALL {
        assert_eq!(
            UnionLabelMatch::parse(rule.as_str()),
            Some(rule),
            "{} does not parse back to itself",
            rule.as_str()
        );
    }
    assert_eq!(UnionLabelMatch::parse("cyclone"), None);
}

// The union's extent is the same under both values, which is the claim
// [`LayoutKey::offset_key`] makes by leaving the axis out.
//
// Without a `default:` the suppressed match writes *less* than the
// extent, which the extent already bounds; with one it writes the
// default branch, which the extent already covers. If either were
// false, a plan cache keyed without this axis would hand one caller
// another's offsets.
#[test]
fn the_extent_is_the_same_under_both_values() {
    let mut compared = 0usize;
    for case in BRANCHLESS_CASES.iter().chain(DEFAULTED_CASES.iter()) {
        let files = case_files(case);
        let ast =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::Cyclone)
                .expect("the case lowers");
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            let base = LayoutKey::in_dialect(
                Dialect::Cyclone,
                encoding,
                Endianness::Little,
                Purpose::Data,
            );
            let cyclone = base.with_union_label(UnionLabelMatch::NegativeLabelUnmatchedBelow32);
            assert_eq!(
                base.offset_key(),
                cyclone.offset_key(),
                "{case}: the axis reached the cache key"
            );
            for (id, node) in ast.types() {
                if !matches!(node.kind, zerodds_ast::TypeKind::Union { .. }) {
                    continue;
                }
                let mut deps = DepCollector::new();
                let one = plan(&ast, id, base, &mut deps).expect("a plan under the clause");
                let mut deps = DepCollector::new();
                let two = plan(&ast, id, cyclone, &mut deps).expect("a plan under the rule");
                assert_eq!(
                    one.extent, two.extent,
                    "{case}: the union's extent moved under the rule"
                );
                compared += 1;
            }
        }
    }
    assert!(compared >= 12, "unions planned: {compared}");
}

// **Over the whole corpus**: every union the rule reaches has
// overlayable branches, and the two that do not are left to the clause.
//
// The first version of this check swept the six cases named at the head
// of this file, found what it was given, and passed. It was a check
// that could not fail: `union-member-sequence` is
// `switch (short) { case -1: sequence<long> ids; default: string text; }`
// — a seventh and eighth narrow-signed union it never looked at. This
// one reads the corpus directory.
//
// The two cases it finds are the reason the crossing has a storage
// half. Their `-1` is the union's only label, so under the rule every
// discriminator value falls into `default:` and the peer reads a
// **pointer** out of the union's storage.
// `tests/interop-results/sources/vendor-corpus.tsv` records what
// happens: Cyclone, both encoding versions, both byte orders,
// `refused — the probe was killed: Segmentation fault`. There are no
// octets, so the setting does not reach the shape and this project
// writes the clause's frame there.
#[test]
fn the_shape_with_no_storage_is_left_to_the_clause() {
    const NO_STORAGE: [&str; 2] = ["union-member-sequence", "union-member-sequence-unannotated"];
    let directory = repository_root().join("tests/interop-corpus");
    let mut narrow_signed: Vec<String> = Vec::new();
    let mut reached: Vec<String> = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.expect("a corpus entry").path())
        .filter(|path| path.join("type").is_file())
        .collect();
    entries.sort();
    for path in entries {
        let case = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a case name")
            .to_string();
        let files = case_files(&case);
        let Ok(ast) =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::Cyclone)
        else {
            continue;
        };
        for (id, node) in ast.types() {
            let zerodds_ast::TypeKind::Union { discriminator, .. } = &node.kind else {
                continue;
            };
            let Some(discriminator) = discriminator.resolved() else {
                continue;
            };
            // The discriminator half on its own, so that a union the
            // storage half excludes is still counted as one the rule
            // would otherwise have reached.
            let narrow = matches!(
                ast.type_(discriminator).map(|node| node.kind.clone()),
                Ok(zerodds_ast::TypeKind::Primitive(
                    zerodds_ast::PrimitiveKind::Int8 | zerodds_ast::PrimitiveKind::Int16
                ))
            );
            let negative = ast.cases_of(id).iter().any(|case_id| {
                ast.case(*case_id).is_ok_and(|node| {
                    node.labels.iter().any(|label| {
                        label
                            .expr()
                            .and_then(|expr| ast.eval_integral(expr).ok())
                            .is_some_and(|value| value < 0)
                    })
                })
            });
            if narrow && negative {
                narrow_signed.push(case.clone());
            }
            if union_negative_matches_nothing(
                &ast,
                id,
                UnionLabelMatch::NegativeLabelUnmatchedBelow32,
            ) {
                reached.push(case.clone());
            }
        }
    }
    let mut expected_reached: Vec<String> = BRANCHLESS_CASES
        .iter()
        .chain(DEFAULTED_CASES.iter())
        .map(|case| (*case).to_string())
        .collect();
    expected_reached.sort();
    reached.sort();
    assert_eq!(
        reached, expected_reached,
        "the rule reaches a different set of unions than this file measures"
    );
    let mut expected_narrow = expected_reached.clone();
    expected_narrow.extend(NO_STORAGE.iter().map(|case| (*case).to_string()));
    expected_narrow.sort();
    narrow_signed.sort();
    assert_eq!(
        narrow_signed, expected_narrow,
        "the corpus has a narrow signed union with a negative label that this file does not \
         account for"
    );

    // And the recorded evidence for the exclusion, read rather than
    // quoted: the matrix's cyclone cells for those two cases are marked
    // failed in all four option sets, and each cell's why names the
    // mechanism — `find_union_case` compares the zero-extended narrow
    // discriminator against the sign-extended label, so a negative
    // label matches nothing and the union has no branch to serialise.
    // The cells live in the `zerodds-matrix` repository, split out on
    // 2026-08-26 and cloned into `matrix/`. Without the clone every marker
    // below is missing, and the assertion reads like the recording changed
    // when in fact nothing was compared. Refuse instead, naming the reason:
    // a run that could not look is not a run that found nothing.
    let cells = repository_root().join("matrix/cyclone");
    assert!(
        cells.is_dir(),
        "matrix/cyclone is not here: this test reads recorded cells, and \
         they live in the zerodds-matrix repository, cloned into matrix/. \
         See docs/matrix-repo-split.md."
    );
    let mut failed_cells = 0usize;
    for case in NO_STORAGE {
        for optset in ["xcdr1-be", "xcdr1-le", "xcdr2-be", "xcdr2-le"] {
            let cell = repository_root()
                .join("matrix/cyclone")
                .join(optset)
                .join(case);
            let marker = cell.join(format!("cyclone-{optset}-{case}-failed.md"));
            assert!(
                marker.is_file(),
                "{}: the cell is no longer marked failed, so the exclusion needs re-measuring",
                marker.display()
            );
            let why = std::fs::read_to_string(cell.join("why.md"))
                .unwrap_or_else(|error| panic!("{}/why.md: {error}", cell.display()));
            assert!(
                why.contains("find_union_case"),
                "{}: the why no longer carries the mechanism behind the exclusion",
                cell.display()
            );
            failed_cells += 1;
        }
    }
    assert_eq!(
        failed_cells, 8,
        "the recorded failed cells behind the exclusion: {failed_cells}"
    );

    // The clause's frame is what the three columns write there, and it
    // is the same frame the default setting writes.
    for case in NO_STORAGE {
        let files = case_files(case);
        let ast =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::Cyclone)
                .expect("the case lowers");
        for endianness in [Endianness::Little, Endianness::Big] {
            let clause = ProbeRequest::in_dialect(
                &files.type_name,
                Encoding::PlainCdr1,
                endianness,
                Dialect::Cyclone,
            );
            let mut ruled = clause.clone();
            ruled.union_label = UnionLabelMatch::NegativeLabelUnmatchedBelow32;
            let one = encode(&ast, &clause, &files.values).expect("the clause's bytes");
            let two = encode(&ast, &ruled, &files.values).expect("the same bytes under the rule");
            assert_eq!(
                hex(&one),
                hex(&two),
                "{case}: the rule reached a union with no storage to overlay"
            );
        }
    }
}

// The sites that select a union's branch go through
// [`union_negative_matches_nothing`], and none of them compares a label
// against the discriminator without the guard.
//
// The behavioural checks above measure the Rust column. This one is the
// only check that reaches the C and Python emitters without compiling a
// generated program, and it is here because the last time a setting
// landed it was missing from `zerodds-idl-python` while every runtime
// test was green: the runtime was never the gap. It is also the only
// check that reaches `wireprobe-zerodds-c-gen`'s `dump.rs`, whose
// bypass changes no octet in either direction and only the JSON the
// read direction prints.
//
// It counts as well as searches. A second selection site added beside a
// crossed one would pass a search and fails the count.
#[test]
fn the_sites_go_through_the_crossing() {
    // (path, how many times the crossing is named, how many times the
    // guard it produces is named). The guard is spelled
    // `suppress_negative` everywhere, so a site that compares a label
    // without one does not raise the second count.
    const SITES: [(&str, usize, usize); 9] = [
        // The definition, which names itself once in its signature and
        // once in `label_matches`'s doc-free body — counted as one.
        ("crates/ast-layout/src/xcdr.rs", 1, 0),
        // The probe's writer reaches selection through
        // `contract::union_selection`, whose argument is the crossing's
        // answer.
        ("crates/wire-codec/src/encode.rs", 1, 5),
        ("crates/wire-codec/src/contract.rs", 0, 3),
        ("crates/wire-codec/src/decode.rs", 1, 4),
        // The plan, which resolves whether a negative label selects
        // nothing.
        ("crates/ast-layout/src/plan.rs", 1, 0),
        // The encoder walk, which asks the plan.
        ("crates/lang-rules/src/emit/encode.rs", 0, 2),
        // The decoder walk, which asks the plan. Python is on this walk
        // now, like every other `zerodds-lang-rules`-based column, so
        // `crates/lang-python/src/emit.rs`'s separate copy — and the
        // separate entry that once caught it missing the guard — are
        // both gone with the crate.
        ("crates/lang-rules/src/emit/decode.rs", 0, 2),
        ("tools/wireprobe/wireprobe-zerodds-c-gen/src/dump.rs", 1, 2),
        // The Python column's own read-direction JSON dumper, the same
        // reason the C column's is above: neither goes through
        // `crates/lang-rules/src/emit/decode.rs`, so neither is covered
        // by that entry.
        (
            "tools/wireprobe/wireprobe-zerodds-python-gen/src/dump.rs",
            1,
            2,
        ),
    ];
    for (path, crossings, guards) in SITES {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        let found = text.matches("union_negative_matches_nothing(").count();
        assert_eq!(
            found, crossings,
            "{path} names the crossing {found} times, not {crossings}"
        );
        let guarded = text.matches("suppress_negative").count();
        assert_eq!(
            guarded, guards,
            "{path} names the guard {guarded} times, not {guards} — a selection site that \
             compares a label without it bypasses the axis and nothing at run time says so"
        );
    }
}

// The axis-reaches-every-wrapper check lives once, in
// `enumerator_value.rs::every_axis_reaches_every_wrapper`.
//
// It stood here too, and in three sibling files, as five copies of one
// assertion with five hard-coded surface paths each. When the five
// generators became renderers on `tools/wireprobe/wireprobe-zerodds-gen`
// the flag left the C column's `lib.rs` for the stem, and every copy
// then named a file that no longer held it. Reading the wrappers to fix
// the first copy showed the larger gap the copies had all missed:
// `-cpp`, `-csharp`, `-java` and `-typescript` forwarded **none** of the
// sixteen axes. One check over all sixteen axes and all nine surfaces
// replaced the five.

// The three configuration lists carry the new column, and the sample
// each one reproduces is the same.
//
// `three_column_gate.rs::the_three_columns_cover_the_same_configurations`
// compares the lists whole; this asserts the one thing that test cannot
// — that the name means the same setting in all four places.
// The setting is in `Dialect::Cyclone`, and all four lists drive that
// dialect.
//
// It used to be a configuration of its own — a dialect plus this
// one override — and it is not one any more, because it describes
// what a measured product puts on the wire and belongs in that
// product's dialect. So the claim this test makes is two claims:
// the dialect selects the setting, and every one of the four
// column lists names the dialect.
#[test]
fn the_configuration_is_named_the_same_everywhere() {
    assert_eq!(
        zerodds_ast_layout::Dialect::Cyclone.union_label(),
        UnionLabelMatch::NegativeLabelUnmatchedBelow32,
        "`Dialect::Cyclone` no longer selects the setting this file measures"
    );
    const LISTS: [&str; 4] = [
        "crates/wire-probe/tests/three_column_gate.rs",
        "tools/interop-results/src/rust_column.rs",
        "crates/wire-probe/src/matrix.rs",
        "tools/wireprobe/wireprobe-zerodds-c-gen/src/bin/wireprobe-c-matrix.rs",
    ];
    for path in LISTS {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert!(
            text.contains("cyclone"),
            "{path} does not drive `--dialect cyclone`"
        );
    }
}

// The canonical text of a read-back sample is the same in shape as the
// sample that went in, minus the branch the stream does not carry.
//
// A round trip of this project's own bytes under the rule: what is
// written is read back, which is the property that makes the setting
// usable rather than merely reproducible.
#[test]
fn the_round_trip_is_stable_under_the_rule() {
    for case in BRANCHLESS_CASES.iter().chain(DEFAULTED_CASES.iter()) {
        let files = case_files(case);
        let ast =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::Cyclone)
                .expect("the case lowers");
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            for endianness in [Endianness::Little, Endianness::Big] {
                let mut request = ProbeRequest::in_dialect(
                    &files.type_name,
                    encoding,
                    endianness,
                    Dialect::Cyclone,
                );
                request.union_label = UnionLabelMatch::NegativeLabelUnmatchedBelow32;
                let bytes = encode(&ast, &request, &files.values)
                    .unwrap_or_else(|error| panic!("{case}: {error}"));
                let read = decode(&ast, &request, &bytes)
                    .unwrap_or_else(|error| panic!("{case}: {error}"));
                let again = encode(&ast, &request, &read).unwrap_or_else(|error| {
                    panic!("{case}: the read-back does not write: {error}")
                });
                assert_eq!(
                    hex(&bytes),
                    hex(&again),
                    "{case}: writing the read-back sample gives other octets"
                );
            }
        }
        // And the sample the corpus declares still canonicalises, so the
        // comparison the gate makes has something to compare against.
        let type_ = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
                files
                    .type_name
                    .split("::")
                    .filter(|segment| !segment.is_empty())
                    .map(std::string::ToString::to_string)
                    .collect(),
            ))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the case's type");
        let canonical =
            canonicalise(&ast, type_, &files.values, "sample").expect("the sample canonicalises");
        assert!(!text(&canonical).is_empty());
    }
}
