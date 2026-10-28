// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **Counter-check.** An enumeration's holder width is answered by the
//! plan's [`EnumHolderWidth`] and by nothing else, in every place that
//! sizes one.
//!
//! # What is measured
//!
//! `tests/interop-results/sources/vendor-corpus.tsv`, the eight
//! `enum-bitbound-{8,16}-{plain,value}` cases and their unannotated
//! twins, three implementations, both encoding versions, both byte
//! orders. `enum-bitbound-8-plain` big endian, `BLUE, RED, 0xab`:
//!
//! ```text
//! cyclone   0200 ab
//! fastdds   00000002 00000000 ab
//! opendds   00000002 00000000 ab
//! rti       refused — `Only 32 bit bounds are accepted for enumerations.`
//! ```
//!
//! Nothing of that is hard-coded here. Each recorded row is swept
//! against `Dialect::ALL × EnumHolderWidth::ALL` and the coordinate is
//! read off the combinations that reproduce it:
//!
//! - every Cyclone row is reproduced, and **only** under
//!   [`EnumHolderWidth::Declared`];
//! - every Fast DDS row is reproduced, and **only** under
//!   [`EnumHolderWidth::BitBoundDroppedByFrontEnd`].
//!
//! Both directions, so the axis is pinned rather than merely reachable.
//! A build in which the width is taken from `Ast::effective_bit_bound`
//! everywhere reproduces the Cyclone rows and no Fast DDS row at all.
//!
//! # The rows this axis does not close, and why that is recorded here
//!
//! OpenDDS's four **`-value`** cases are reproduced by no combination,
//! and they are asserted to be so rather than left out. `opendds_idl`
//! numbers enumerators by ordinal and drops `@value`, so its
//! `enum-bitbound-8-value` is `00000002 00000001 ab` where `@value(200)`
//! and `@value(9)` were declared — the same four-octet holder as its
//! `-plain` rows, with different contents. That is a second question
//! (the inventory's E1, `warnings.rs::CLASSIFICATION` carries it as
//! `carried_into: "E1"`) and a holder width cannot answer it. Sixteen of
//! OpenDDS's rows therefore stay open; the sixteen `-plain` ones close.
//!
//! # Mutation-checked, at the crossing and at the position
//!
//! Five mutations were run against this file. What each one turns red is
//! recorded, including the one that only the source check catches — a
//! mutation whose colour is not written down is a mutation somebody has
//! to run again.
//!
//! | mutation | what goes red |
//! |---|---|
//! | **value** — `EnumHolderWidth::holder_bound` returning `declared` for `BitBoundDroppedByFrontEnd` | the three behavioural checks |
//! | **the axis's own arms swapped** | the three behavioural checks |
//! | **the writer** — `write_enumeration` reading `Ast::effective_bit_bound` | all four |
//! | **the element position** — `sequence_length_code`'s `Enumeration` arm reading `Ast::effective_bit_bound`, with `length_code`'s member arm left crossed | [`the_element_position_is_wired_too`] and [`the_sites_go_through_the_crossing`]. [`the_two_widths_are_told_apart`] stays **green**, which is the separation this file exists for: a member-position sweep cannot tell a wired element position from one that inherited the member's answer |
//! | **the planner** — `plan_at`'s `Enumeration` arm reading `Ast::effective_bit_bound` | [`the_sites_go_through_the_crossing`] **only** |
//!
//! That last row is a finding and not a gap in this file. The planner's
//! enumeration extent reaches an octet only where a `@mutable` member's
//! length code is derived from it, and **no corpus case declares a
//! `@mutable` member whose type is a narrow-bound enumeration**: every
//! `enum-bitbound-*` case is `@final`, and the two element-position
//! cases reach `sequence_length_code` rather than `plan_at`'s extent. So
//! the planner arm is unobservable in bytes today, and the source check
//! is the only thing standing between it and a silent bypass. Checked,
//! recorded, and the reason it is not left to a behavioural check that
//! could not fail.
//!
//! [`the_sites_go_through_the_crossing`] is the check that does not need
//! a corpus row: it names the sites and fails when one of them goes back
//! to reading the declared bound.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{Dialect, Encoding, Endianness, EnumHolderWidth, EnumeratorValue};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

/// The eight cases whose enumeration carries a bound narrower than 32.
///
/// `enum-bitbound-32-*` is deliberately not here: a 32-bit bound is the
/// width every implementation writes, so those rows cannot separate the
/// two values of this axis and a test that included them would pass on
/// rows that say nothing. That is the same prefix mistake
/// `starts_with("enum-bitbound-")` made when it swept
/// `enum-bitbound-32-value` into group G.
const MEMBER_CASES: [&str; 8] = [
    "enum-bitbound-8-plain",
    "enum-bitbound-8-plain-unannotated",
    "enum-bitbound-8-value",
    "enum-bitbound-8-value-unannotated",
    "enum-bitbound-16-plain",
    "enum-bitbound-16-plain-unannotated",
    "enum-bitbound-16-value",
    "enum-bitbound-16-value-unannotated",
];

/// The four cases OpenDDS cannot be reproduced on, and the reason: its
/// front end numbers enumerators by ordinal, so the *contents* differ as
/// well as the width.
const OPENDDS_ORDINAL_NUMBERED: [&str; 4] = [
    "enum-bitbound-8-value",
    "enum-bitbound-8-value-unannotated",
    "enum-bitbound-16-value",
    "enum-bitbound-16-value-unannotated",
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Every recorded `wrote` row of `vendor-corpus.tsv` for [`CASES`],
/// keyed by (case, system, version, order).
fn recorded() -> BTreeMap<(String, String, String, String), String> {
    let mut rows = BTreeMap::new();
    let path = repository_root().join("tests/interop-results/sources/vendor-corpus.tsv");
    let text = std::fs::read_to_string(path).expect("tests/interop-results/sources");
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        // case, system, version, endian, outcome, payload. A row that is
        // not `wrote` is a refusal — RTI's four, which are the same
        // measurement from the other side — and is left out rather than
        // parsed as octets.
        if f.len() < 6
            || f[4] != "wrote"
            || !MEMBER_CASES.contains(&f[0]) && !ELEMENT_CASES.iter().any(|(name, _)| *name == f[0])
        {
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

/// One point of the space a `enum-bitbound-*` sample sits in: which
/// peer's answers to everything else, and how wide the holder is.
type Combination = (Dialect, EnumHolderWidth);

fn combinations() -> Vec<Combination> {
    let mut out = Vec::new();
    for dialect in Dialect::ALL {
        for width in EnumHolderWidth::ALL {
            out.push((dialect, width));
        }
    }
    out
}

/// What this project writes for one corpus case under one combination.
fn written(
    case: &str,
    encoding: Encoding,
    endianness: Endianness,
    combination: Combination,
) -> Option<String> {
    let directory = repository_root().join("tests/interop-corpus").join(case);
    let idl = std::fs::read_to_string(directory.join("type.idl")).ok()?;
    let type_name = std::fs::read_to_string(directory.join("type")).ok()?;
    let values: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("values.json")).ok()?).ok()?;
    // The model is lowered in the dialect too: member numbering is a
    // front-end answer and not a plan's, so a model lowered once would
    // give every column Cyclone's member ids.
    let ast = model_from_idl_in_dialect(&idl, &format!("{case}/type.idl"), combination.0).ok()?;
    let mut request =
        ProbeRequest::in_dialect(type_name.trim(), encoding, endianness, combination.0);
    request.enum_holder = combination.1;
    // The enumerator's own value is held at the declared `@value(n)`,
    // and that is the point of this sweep rather than an omission: this
    // file measures the **holder's width** and `enumerator_value.rs`
    // measures the numbering. `Dialect::OpenDds` carries both — it is
    // the product that drops both annotations — so a sweep that let the
    // dialect bring the numbering along would report the holder alone
    // as reproducing a row that needs the two, which is the claim
    // `opendds_closes_on_the_holder_and_not_on_the_numbering` exists to
    // deny.
    request.enumerator_value = EnumeratorValue::Declared;
    encode(&ast, &request, &values).ok().map(|b| hex(&b))
}

/// Which combinations reproduce a recorded row exactly.
fn matching(case: &str, version: &str, endian: &str, theirs: &str) -> Vec<Combination> {
    let encoding = if version == "xcdr1" {
        Encoding::PlainCdr1
    } else {
        Encoding::PlainCdr2
    };
    let endianness = if endian == "be" {
        Endianness::Big
    } else {
        Endianness::Little
    };
    combinations()
        .into_iter()
        .filter(|combination| {
            written(case, encoding, endianness, *combination).as_deref() == Some(theirs)
        })
        .collect()
}

/// The two widths are told apart by the recorded octets, in both
/// directions.
///
/// Cyclone honours the bound and Fast DDS does not, so a row of one is
/// reproduced only under one value of the axis and a row of the other
/// only under the other. That is what makes this a measurement of the
/// axis rather than of the corpus: a build where the width never reaches
/// the writer reproduces one side and nothing of the other.
#[test]
fn the_two_widths_are_told_apart() {
    let rows = recorded();
    assert!(
        !rows.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check must not \
         pass by finding nothing"
    );
    let mut compared = 0usize;
    for case in MEMBER_CASES {
        for (system, expected) in [
            ("cyclone", EnumHolderWidth::Declared),
            ("fastdds", EnumHolderWidth::BitBoundDroppedByFrontEnd),
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
                    for (dialect, width) in &found {
                        assert_eq!(
                            *width,
                            expected,
                            "{case} {system} {version} {endian}: reproduced under \
                             {}/{} — the holder width is not pinned by these octets",
                            dialect.as_str(),
                            width.as_str()
                        );
                    }
                    compared += 1;
                }
            }
        }
    }
    // Eight cases, two implementations, two versions, two orders. It
    // fails below that rather than reporting whatever it found.
    assert_eq!(
        compared,
        MEMBER_CASES.len() * 2 * 2 * 2,
        "the sweep compared {compared} rows"
    );
}

/// The cases whose narrow-bound enumeration is reached **only through a
/// collection**, with whether Cyclone's own rows can be used as the
/// other side of the comparison.
///
/// The member position is not the whole axis. `array-3d-enum` puts the
/// enumeration in an array's element position and `combined-everything`
/// puts it in a `sequence<Colour, 4>` inside a `@mutable` member, which
/// is the only shape that reaches `sequence_length_code` — the element
/// twin of `length_code`, and the one arm a member-position case cannot
/// exercise. A holder width wired at the member position and nowhere
/// else reproduces every row of [`MEMBER_CASES`] and none of these.
///
/// Cyclone's `combined-everything` rows are **not** usable: that case
/// also carries `union Choice switch (short) { case -1: ... }`, and
/// Cyclone drops the branch on a negative label under a narrow
/// discriminator — `tests/interop-corpus/combined-everything/README.md`.
/// Its rows are reproduced by nothing at all, for a reason that has
/// nothing to do with an enumeration, so the `false` here is a recorded
/// exclusion rather than a case quietly left out.
const ELEMENT_CASES: [(&str, bool); 4] = [
    ("array-3d-enum", true),
    ("array-3d-enum-unannotated", true),
    ("combined-everything", false),
    ("combined-everything-unannotated", false),
];

/// The axis reaches an enumeration at a collection's element position,
/// not only at a member's.
///
/// The same two-directional pin as [`the_two_widths_are_told_apart`],
/// over cases where the enumeration is never a member of an aggregate.
/// Fast DDS's rows are reproduced only under
/// [`EnumHolderWidth::BitBoundDroppedByFrontEnd`]; Cyclone's, where the
/// case has usable ones, only under [`EnumHolderWidth::Declared`].
///
/// **Mutation-checked at the position.** With
/// `sequence_length_code`'s `TypeKind::Enumeration` arm reading
/// `Ast::effective_bit_bound` — the element arm, while the member arm in
/// `length_code` keeps the crossing — this test fails on
/// `combined-everything` and
/// [`the_two_widths_are_told_apart`] still passes. That is the
/// separation the check exists for: a gate that only mutates the value
/// cannot tell a wired position from one that inherited the member's
/// answer.
#[test]
fn the_element_position_is_wired_too() {
    let rows = recorded();
    let mut compared = 0usize;
    for (case, cyclone_usable) in ELEMENT_CASES {
        let mut systems: Vec<(&str, EnumHolderWidth)> =
            vec![("fastdds", EnumHolderWidth::BitBoundDroppedByFrontEnd)];
        if cyclone_usable {
            systems.push(("cyclone", EnumHolderWidth::Declared));
        }
        for (system, expected) in systems {
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
                         reproduce {theirs} — the holder width does not reach an enumeration \
                         at a collection's element position",
                        combinations().len()
                    );
                    for (dialect, width) in &found {
                        assert_eq!(
                            *width,
                            expected,
                            "{case} {system} {version} {endian}: reproduced under {}/{}",
                            dialect.as_str(),
                            width.as_str()
                        );
                    }
                    compared += 1;
                }
            }
        }
    }
    // Two cases with both sides and two with Fast DDS only, two
    // versions, two orders. It fails below that rather than reporting
    // whatever it found.
    assert_eq!(
        compared,
        (2 * 2 + 2) * 2 * 2,
        "the sweep compared {compared} rows"
    );
}

/// OpenDDS's `-plain` rows close on the holder alone and its `-value`
/// rows do not, and the second half is asserted rather than omitted.
///
/// A row left out of a gate reads, next time, like a row that was never
/// a problem. `opendds_idl` drops `@value` and numbers enumerators by
/// ordinal, so its `-value` octets differ from every implementation's in
/// content as well as in width; no holder-width setting reaches them.
///
/// **What has changed, and it is the reason this comment is not the one
/// it was.** Those sixteen rows are reachable now — by
/// `EnumeratorValue::Ordinal` beside this axis, which is a second
/// setting and not this one. This sweep is over
/// `Dialect` × `EnumHolderWidth` and answers only for the holder; the
/// rows it calls open here are open *on the holder's axis*, which is
/// what it measured all along.
/// `crates/wire-probe/tests/enumerator_value.rs::the_sixteen_need_both_settings_and_neither_alone`
/// is the other half and holds both directions: the holder alone does
/// not reproduce them and the numbering alone does not either.
#[test]
fn opendds_closes_on_the_holder_and_not_on_the_numbering() {
    let rows = recorded();
    let mut closed = 0usize;
    let mut open = 0usize;
    for case in MEMBER_CASES {
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let key = (
                    case.to_string(),
                    "opendds".to_string(),
                    version.to_string(),
                    endian.to_string(),
                );
                let Some(theirs) = rows.get(&key) else {
                    panic!("{case} opendds {version} {endian} is not in vendor-corpus.tsv");
                };
                let found = matching(case, version, endian, theirs);
                if OPENDDS_ORDINAL_NUMBERED.contains(&case) {
                    assert!(
                        found.is_empty(),
                        "{case} opendds {version} {endian}: reproduced by {found:?} on the \
                         holder's axis alone — the numbering would then not be part of this \
                         cause, and `enumerator_value.rs` says it is"
                    );
                    open += 1;
                } else {
                    assert!(
                        !found.is_empty(),
                        "{case} opendds {version} {endian}: none of the {} combinations \
                         reproduce {theirs}",
                        combinations().len()
                    );
                    for (dialect, width) in &found {
                        assert_eq!(
                            *width,
                            EnumHolderWidth::BitBoundDroppedByFrontEnd,
                            "{case} opendds {version} {endian}: reproduced under {}/{}",
                            dialect.as_str(),
                            width.as_str()
                        );
                    }
                    closed += 1;
                }
            }
        }
    }
    assert_eq!(closed, 16, "OpenDDS `-plain` rows closed: {closed}");
    assert_eq!(open, 16, "OpenDDS `-value` rows still open: {open}");
}

/// The ten sites that size an enumeration's holder go through
/// [`zerodds_ast_layout::enumeration_bit_bound`], and none of them reads
/// the declared bound straight.
///
/// The behavioural checks above measure the Rust column. This one is the
/// only check that reaches the C and Python emitters without compiling a
/// generated program, and it is here because the last time a setting
/// landed it was missing from `zerodds-idl-python` while every runtime
/// test was green: the runtime was never the gap.
///
/// It counts as well as searches. A site put back to
/// `Ast::effective_bit_bound` next to a surviving
/// `enumeration_bit_bound` would pass a search and fails the count.
#[test]
fn the_sites_go_through_the_crossing() {
    // (path, how many times the crossing is called, how many
    // `effective_bit_bound` calls remain — every one of them a bitmask,
    // which is Table 33 and a different clause, or CORBA's refusal of
    // the annotation, which is about the declaration and not the wire.)
    const SITES: [(&str, usize, usize); 8] = [
        // The planner: the `Enumeration` arm, `length_code`'s and
        // `sequence_length_code`'s, each split from the `Bitmask` arm it
        // used to share.
        ("crates/ast-layout/src/xcdr.rs", 3, 0),
        ("crates/wire-codec/src/encode.rs", 1, 1),
        ("crates/wire-codec/src/decode.rs", 1, 1),
        // Three, and each is a separate answer: the enumeration's own
        // `encode` and `decode` size the holder they write and read, and
        // `write_of`'s `Enumeration` arm sizes the one an aggregate's
        // member or a collection's element sits in. The count is three
        // rather than one because the codec was split in two; all three
        // go through the crossing, and the `0` beside them is what says
        // none of them reads the declared bound instead.
        ("crates/lang-rules/src/encode.rs", 3, 0),
        // The plan, which now answers instead of the generators asking.
        // `ScalarWrite::Enum` carries the resolved width, so a generator
        // has no axis to read and no argument to get wrong — which is
        // the failure this counting check cannot see. One site, and the
        // `1` beside it is the raw read the crossing itself makes.
        // Two: `ScalarWrite::Enum` carries the width for a generator
        // that has an instruction, and `enumeration_holder_bits` answers
        // the reader that needs the width before it has one.
        ("crates/ast-layout/src/plan.rs", 2, 1),
        // The encoder walk, which asks the plan and reads nothing.
        // Zero is the assertion: a generator that started reading the
        // axis again would fail here.
        ("crates/lang-rules/src/emit/encode.rs", 0, 0),
        // The decoder walk, which asks the plan for the width. Python is
        // on this walk now, like every other `zerodds-lang-rules`-based
        // column, so `crates/lang-python/src/emit.rs`'s separate copy —
        // and the separate entry that once caught it missing an axis —
        // are both gone with the crate.
        ("crates/lang-rules/src/emit/decode.rs", 0, 1),
        // CORBA has no `@bit_bound`: the one call refuses a type that
        // carries one before any ORB sees it, which is a question about
        // the declaration and not about a holder's width.
        ("crates/wire-probe/src/corba_idl.rs", 0, 1),
    ];
    for (path, crossings, straight) in SITES {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        // The definition itself calls `effective_bit_bound` once and
        // names itself once; neither is a site.
        let is_definition = path.ends_with("ast-layout/src/xcdr.rs");
        let found = text.matches("enumeration_bit_bound(").count() - usize::from(is_definition);
        assert_eq!(
            found, crossings,
            "{path} calls the crossing {found} times, not {crossings}"
        );
        let raw = text.matches("effective_bit_bound(").count() - usize::from(is_definition);
        assert_eq!(
            raw, straight,
            "{path} reads the declared bound straight {raw} times, not {straight} — a site that \
             does so for an enumeration bypasses the axis and nothing at run time says so"
        );
    }
}
