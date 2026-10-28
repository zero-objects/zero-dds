// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **`EnumeratorValue`.** Which integer an enumerator serialises as.
//!
//! IDL 4.2 §7.4.13 gives it: the `@value(n)` the enumerator declares,
//! else the previous one's plus one, from zero. XTypes 1.3 §7.4.3.5.3
//! rule (5) puts that value into the holder. So the clause has an
//! answer, and this axis exists because two front ends do not carry the
//! annotation as far as a serialiser and write the enumerator's
//! **position** instead.
//!
//! # What is measured
//!
//! Forty recorded OpenDDS rows: `enum-negative-value`,
//! `enum-sparse-values` and `enum-bitbound-{8,16,32}-value` with their
//! unannotated twins, both encoding versions and both byte orders. Every
//! one is the **same length** as this project's and differs from octet
//! 0. `enum-bitbound-8-value` declares `@value(3) RED, @value(9) GREEN,
//! @value(200) BLUE` and holds `{BLUE, GREEN}`:
//!
//! ```text
//! ours    000000c8 00000009 ab
//! theirs  00000002 00000001 ab
//! ```
//!
//! `opendds_idl` answers `error in lookup of symbol` for `value` exactly
//! as it does for `bit_bound`, which is why sixteen of the forty need
//! **both** settings and why [`EnumHolderWidth`] alone closed 76 rows
//! and not the 92 an estimate gave.
//!
//! # The sixteen rows this file exists for
//!
//! `docs/open-rows-breakdown.md` attributed 24 rows to this cause and 16
//! more to the holder's width. The 16 —
//! `enum-bitbound-{8,16}-value` and their twins — are **both**: after
//! `bit-bound-dropped` the holder is four octets on each side and the
//! contents still differ. They were the whole of the difference between
//! that document's arithmetic and the measurement, which is why the
//! cause is 40 rows and not 24.
//! [`the_sixteen_need_both_settings_and_neither_alone`] is that as a
//! test.
//!
//! # Mutation-checked
//!
//! Recorded rather than summarised, because two of the mutations are
//! caught by one check each.
//!
//! Six mutations were run and each one's result is what was observed,
//! not what was expected.
//!
//! | mutation | what goes red |
//! |---|---|
//! | `enumerator_wire_value`'s `Ordinal` arm returning the declared value | five of the eight checks here |
//! | the probe's `write_enumeration` taking `contract::enumerator`'s declared value straight | [`the_forty_rows_are_reproduced`], [`the_sixteen_need_both_settings_and_neither_alone`], [`only_the_enumerations_that_declare_value_differ`] |
//! | the probe's `read_enumeration` reading `Ast::effective_enumerator_value` straight | [`the_peers_octets_read_back_only_under_the_setting`] and [`the_sites_go_through_the_crossing`] |
//! | the C emitter's chain built from the declared value | [`the_sites_go_through_the_crossing`] **only** here, because every check in this file measures the Rust column. Measured in the C column directly rather than left as a claim: `wireprobe-zerodds-c` on `enum-bitbound-8-value` then writes `000000c800000009ab` where the Rust and Python columns write `0000000200000001ab`, which is a column-to-column disagreement and what `three_column_gate.rs` compares |
//! | the Python emitter's `_WIRE_` table built from the declared value | the same, mirrored: the Python column writes `000000c8…` and the other two `00000002…` |
//! | `EnumeratorValue` added to `LayoutKey::offset_key` | **the compiler**, and that is the finding rather than a green: `OffsetKey` is a twelve-tuple and Rust's `Ord`, `Debug` and `Clone` impls stop at twelve, so a thirteenth element does not build. [`the_extent_is_the_same_under_both_values`] would have caught it and never got the chance. Recorded so the next axis's author knows the check is not what is holding that line today |
//!
//! The fast tier's representative for the `enum` family is
//! `enum-bitbound-16-plain`, which declares **no** `@value`, and
//! `EVER_DISAGREED` carries `enum-bitbound-8-value`, which does. So the
//! C column is exercised on the axis by the fast tier through the second
//! list and not through the first — the same shape as the
//! `map-ascending-key` gap, and named here so it is not rediscovered.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{
    Dialect, Encoding, Endianness, EnumHolderWidth, EnumeratorValue, LayoutKey, Purpose,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

/// The ten cases whose enumeration declares an `@value`.
///
/// Every one of them is a case OpenDDS wrote octets for and every one
/// differs; the `-plain` cases beside them declare no `@value` and agree
/// under both policy values, which
/// [`only_the_enumerations_that_declare_value_differ`] derives from the
/// corpus rather than from this list.
const VALUE_CASES: [&str; 10] = [
    "enum-negative-value",
    "enum-negative-value-unannotated",
    "enum-sparse-values",
    "enum-sparse-values-unannotated",
    "enum-bitbound-8-value",
    "enum-bitbound-8-value-unannotated",
    "enum-bitbound-16-value",
    "enum-bitbound-16-value-unannotated",
    "enum-bitbound-32-value",
    "enum-bitbound-32-value-unannotated",
];

/// The four of the ten whose holder is also narrower than 32 bits, so
/// that reproducing them needs [`EnumHolderWidth`] as well.
const NARROW_HOLDER: [&str; 4] = [
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

/// Every recorded `wrote` row of `vendor-corpus.tsv`, keyed by
/// (case, system, version, order).
fn recorded() -> BTreeMap<(String, String, String, String), String> {
    let mut rows = BTreeMap::new();
    let path = repository_root().join("tests/interop-results/sources/vendor-corpus.tsv");
    let text = std::fs::read_to_string(path).expect("tests/interop-results/sources");
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        // case, system, version, endian, outcome, payload. A row that is
        // not `wrote` is a refusal or a version alias and is left out
        // rather than parsed as octets.
        if f.len() < 6 || f[4] != "wrote" {
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

/// One point of the space an `enum-*-value` sample sits in: whose
/// answers to everything else, how wide the holder is, and which integer
/// goes into it.
type Combination = (Dialect, EnumHolderWidth, EnumeratorValue);

fn combinations() -> Vec<Combination> {
    let mut out = Vec::new();
    for dialect in Dialect::ALL {
        for width in EnumHolderWidth::ALL {
            for value in EnumeratorValue::ALL {
                out.push((dialect, width, value));
            }
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
    let ast = model_from_idl_in_dialect(&idl, &format!("{case}/type.idl"), combination.0).ok()?;
    let mut request =
        ProbeRequest::in_dialect(type_name.trim(), encoding, endianness, combination.0);
    request.enum_holder = combination.1;
    request.enumerator_value = combination.2;
    encode(&ast, &request, &values).ok().map(|b| hex(&b))
}

/// Which combinations reproduce a recorded row exactly.
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

/// All forty rows are reproduced, and every combination that reproduces
/// one is [`EnumeratorValue::Ordinal`].
///
/// The second half is what makes this a measurement of the axis rather
/// than of a preset: if some dialect's other answers happened to produce
/// the same octets, the row would not decide the coordinate and the test
/// says so instead of counting it.
#[test]
fn the_forty_rows_are_reproduced() {
    let rows = recorded();
    assert!(
        !rows.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check must not \
         pass by finding nothing"
    );
    let mut compared = 0usize;
    for case in VALUE_CASES {
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
                assert!(
                    !found.is_empty(),
                    "{case} opendds {version} {endian}: none of the {} combinations reproduce \
                     {theirs}",
                    combinations().len()
                );
                for (dialect, width, value) in &found {
                    assert_eq!(
                        *value,
                        EnumeratorValue::Ordinal,
                        "{case} opendds {version} {endian}: reproduced under {}/{}/{}, so the \
                         row does not decide the enumerator-value axis",
                        dialect.as_str(),
                        width.as_str(),
                        value.as_str()
                    );
                }
                compared += 1;
            }
        }
    }
    // Ten cases, two encoding versions, two byte orders.
    assert_eq!(compared, 40, "{compared} rows compared, not 40");
}

/// The sixteen rows the breakdown attributed to the holder's width need
/// **both** settings, and neither of them alone.
///
/// This is the sixteen-row discrepancy between
/// `docs/open-rows-breakdown.md`'s arithmetic and the measurement, as a
/// test. `enum-bitbound-{8,16}-value` and their twins differ from
/// OpenDDS in the holder *and* in the contents; the six rows beside them
/// — `enum-negative-value`, `enum-sparse-values`,
/// `enum-bitbound-32-value` and twins — are a four-octet holder already
/// and need the numbering only.
#[test]
fn the_sixteen_need_both_settings_and_neither_alone() {
    let rows = recorded();
    let mut both = 0usize;
    let mut numbering_only = 0usize;
    for case in VALUE_CASES {
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let key = (
                    case.to_string(),
                    "opendds".to_string(),
                    version.to_string(),
                    endian.to_string(),
                );
                let theirs = rows.get(&key).expect("a recorded row");
                let encoding = encoding_of(version);
                let endianness = endianness_of(endian);
                let under = |width, value| {
                    written(
                        case,
                        encoding,
                        endianness,
                        (Dialect::Specification, width, value),
                    )
                };
                let holder_only = under(
                    EnumHolderWidth::BitBoundDroppedByFrontEnd,
                    EnumeratorValue::Declared,
                );
                let numbering = under(EnumHolderWidth::Declared, EnumeratorValue::Ordinal);
                let together = under(
                    EnumHolderWidth::BitBoundDroppedByFrontEnd,
                    EnumeratorValue::Ordinal,
                );
                assert_eq!(
                    together.as_deref(),
                    Some(theirs.as_str()),
                    "{case} opendds {version} {endian}: the two settings together do not \
                     reproduce the row"
                );
                assert_ne!(
                    holder_only.as_deref(),
                    Some(theirs.as_str()),
                    "{case}: the holder alone reproduces the row, so the numbering is not \
                     part of this cause after all"
                );
                if NARROW_HOLDER.contains(&case) {
                    assert_ne!(
                        numbering.as_deref(),
                        Some(theirs.as_str()),
                        "{case}: the numbering alone reproduces a narrow-bound row, so the \
                         holder is not part of it after all"
                    );
                    both += 1;
                } else {
                    assert_eq!(
                        numbering.as_deref(),
                        Some(theirs.as_str()),
                        "{case}: a 32-bit holder is what every implementation writes, so the \
                         numbering alone has to reproduce the row"
                    );
                    numbering_only += 1;
                }
            }
        }
    }
    assert_eq!(both, 16, "rows needing both settings: {both}, not 16");
    assert_eq!(
        numbering_only, 24,
        "rows needing the numbering only: {numbering_only}, not 24"
    );
}

/// Only an enumeration that declares a `@value` differing from the
/// position writes different octets, and that is derived from the corpus
/// rather than from a list of names.
///
/// A list would say what somebody believed. This walks every case
/// directory, encodes it under both values of the axis and requires the
/// two to differ **exactly** where the IDL carries an `@value` — which
/// is the same shape `only_the_values_with_leading_zeros_differ` takes
/// for `FixedDigitCount`.
#[test]
fn only_the_enumerations_that_declare_value_differ() {
    let corpus = repository_root().join("tests/interop-corpus");
    let mut differing: Vec<String> = Vec::new();
    let mut agreeing = 0usize;
    let mut walked = 0usize;
    let entries = std::fs::read_dir(&corpus).expect("tests/interop-corpus");
    for entry in entries {
        let entry = entry.expect("a corpus entry");
        if !entry.path().is_dir() {
            continue;
        }
        let case = entry.file_name().to_string_lossy().to_string();
        let declared = written(
            &case,
            Encoding::PlainCdr1,
            Endianness::Big,
            (
                Dialect::Specification,
                EnumHolderWidth::Declared,
                EnumeratorValue::Declared,
            ),
        );
        let ordinal = written(
            &case,
            Encoding::PlainCdr1,
            Endianness::Big,
            (
                Dialect::Specification,
                EnumHolderWidth::Declared,
                EnumeratorValue::Ordinal,
            ),
        );
        let (Some(declared), Some(ordinal)) = (declared, ordinal) else {
            continue;
        };
        walked += 1;
        if declared == ordinal {
            agreeing += 1;
        } else {
            differing.push(case);
        }
    }
    assert!(
        walked > 300,
        "only {walked} cases encoded — this check must not pass by finding nothing"
    );
    differing.sort();
    // Every case whose octets move is one whose IDL declares an
    // `@value`, read out of the IDL rather than named here.
    for case in &differing {
        let idl = std::fs::read_to_string(corpus.join(case).join("type.idl")).expect("the IDL");
        assert!(
            idl.contains("@value("),
            "{case} writes different octets under the two values and declares no @value"
        );
    }
    // The ten cause rows are all here, and so are two more:
    // `combined-everything` and its twin declare `@value(9) GREEN` on a
    // `@bit_bound(16)` enumeration. They are **not** part of the forty,
    // and the reason is recorded rather than assumed — `opendds_idl`
    // refuses the case outright (`Error - tao_idl: Illegal component in
    // scoped name`), so there are no octets from the one implementation
    // whose front end drops the annotation. The axis reaches the
    // construct; the corpus has no peer row for it.
    for case in VALUE_CASES {
        assert!(
            differing.iter().any(|found| found == case),
            "{case} declares @value and writes the same octets under both values"
        );
    }
    let extra: Vec<&String> = differing
        .iter()
        .filter(|case| !VALUE_CASES.contains(&case.as_str()))
        .collect();
    assert_eq!(
        extra,
        vec![
            "combined-everything",
            "combined-everything-unannotated",
            // Declares enumerators with gaps rather than @value, and the
            // gap is what the two settings disagree about.
            "enum-value-gaps",
        ],
        "a case outside the forty writes different octets under the two values"
    );
    assert!(agreeing > 300, "cases that agree: {agreeing}");
}

/// OpenDDS's own octets read back as the case's sample **only** under
/// the setting, and name a different enumerator or none without it.
///
/// The reader half, and the contrast with `MapEntryOrder`: a permutation
/// of a map reaches no reader, and this reaches every one of the forty.
#[test]
fn the_peers_octets_read_back_only_under_the_setting() {
    let rows = recorded();
    let mut read_back = 0usize;
    let mut refused_or_wrong = 0usize;
    for case in VALUE_CASES {
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let key = (
                    case.to_string(),
                    "opendds".to_string(),
                    version.to_string(),
                    endian.to_string(),
                );
                let theirs = rows.get(&key).expect("a recorded row");
                let directory = repository_root().join("tests/interop-corpus").join(case);
                let idl = std::fs::read_to_string(directory.join("type.idl")).expect("the IDL");
                let type_name = std::fs::read_to_string(directory.join("type")).expect("the type");
                let values: serde_json::Value = serde_json::from_str(
                    &std::fs::read_to_string(directory.join("values.json")).expect("the sample"),
                )
                .expect("the sample as JSON");
                let ast = model_from_idl_in_dialect(
                    &idl,
                    &format!("{case}/type.idl"),
                    Dialect::Specification,
                )
                .expect("the model");
                let root = root_of(&ast, type_name.trim());
                let sample =
                    text(&canonicalise(&ast, root, &values, type_name.trim()).expect("canon"));
                let bytes = unhex(theirs);

                let mut request = ProbeRequest::in_dialect(
                    type_name.trim(),
                    encoding_of(version),
                    endianness_of(endian),
                    Dialect::Specification,
                );
                request.enum_holder = EnumHolderWidth::BitBoundDroppedByFrontEnd;
                request.enumerator_value = EnumeratorValue::Ordinal;
                let under_setting = decode(&ast, &request, &bytes).map(|value| text(&value));
                assert_eq!(
                    under_setting.as_deref().ok(),
                    Some(sample.as_str()),
                    "{case} opendds {version} {endian}: the peer's octets do not read back as \
                     the sample under the setting"
                );
                read_back += 1;

                request.enumerator_value = EnumeratorValue::Declared;
                let without = decode(&ast, &request, &bytes).map(|value| text(&value));
                assert_ne!(
                    without.as_deref().ok(),
                    Some(sample.as_str()),
                    "{case} opendds {version} {endian}: the peer's octets read back as the \
                     sample without the setting too, so the axis reaches no reader here"
                );
                refused_or_wrong += 1;
            }
        }
    }
    assert_eq!(read_back, 40);
    assert_eq!(refused_or_wrong, 40);
}

/// The axis moves no offset, which is why it is out of
/// [`LayoutKey::offset_key`].
///
/// The sharpest contrast with [`EnumHolderWidth`], which is in it: the
/// two reach the same construct and only one of them is a length.
#[test]
fn the_extent_is_the_same_under_both_values() {
    for case in VALUE_CASES {
        let directory = repository_root().join("tests/interop-corpus").join(case);
        let idl = std::fs::read_to_string(directory.join("type.idl")).expect("the IDL");
        let type_name = std::fs::read_to_string(directory.join("type")).expect("the type");
        let ast = model_from_idl(&idl, &format!("{case}/type.idl")).expect("the model");
        let root = root_of(&ast, type_name.trim());
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            let base = LayoutKey::in_dialect(
                Dialect::Specification,
                encoding,
                Endianness::Big,
                Purpose::Data,
            );
            let declared = base.with_enumerator_value(EnumeratorValue::Declared);
            let ordinal = base.with_enumerator_value(EnumeratorValue::Ordinal);
            assert_eq!(
                declared.offset_key(),
                ordinal.offset_key(),
                "{case}: the two values give different offset keys, so the axis is a planner \
                 setting after all"
            );
            let mut left = zerodds_ast_layout::DepCollector::new();
            let mut right = zerodds_ast_layout::DepCollector::new();
            let a = zerodds_ast_layout::plan(&ast, root, declared, &mut left).expect("a plan");
            let b = zerodds_ast_layout::plan(&ast, root, ordinal, &mut right).expect("a plan");
            assert_eq!(a.extent, b.extent, "{case}: the extent moved");
        }
    }
}

/// The sites that put an enumerator on the wire or take one off it go
/// through [`zerodds_ast_layout::enumerator_wire_value`].
///
/// The only check that reaches the C and Python emitters without
/// compiling a generated program, and the reason it counts as well as
/// searches: a site put back to `Ast::effective_enumerator_value` next
/// to a surviving crossing would pass a search and fails the count.
///
/// # The callers that are deliberately **not** sites
///
/// `Ast::effective_enumerator_value` is also the **language mapping**: a
/// generated C constant, a Rust discriminant and a Python `IntEnum`
/// member take the value the IDL declares whatever a peer's front end
/// did with the annotation. Those calls stay, and the counts below say
/// how many of them each file has.
///
/// `crates/lang-rules/src/encode.rs` **was** a checked non-site: its
/// enumeration arm emitted `w.write_u32(*v)` and `r.read_u32()`, which
/// move the integer the generated type already holds and consult no
/// enumerator table in either direction. It is a site now. The arm is a
/// `match` — the cast form did not compile, `adapter::cst` derives no
/// `Copy` — and a `match` has to name each enumerator's integer, which
/// is the integer that leaves the process. It named
/// `Ast::effective_enumerator_value`'s, so the Rust column wrote
/// `@value(200)` under a setting that asked for the ordinal, in both
/// directions, and no corpus row reaches that code: this count is what
/// caught it.
///
/// The Rust **discriminant** is still the language mapping and still
/// declared. `adapter/cst.rs` writes it.
#[test]
fn the_sites_go_through_the_crossing() {
    // (path, crossings, remaining `effective_enumerator_value` calls —
    // every one of them a language mapping, a sort key or a union case
    // label, none of which is the integer that leaves the process.)
    const SITES: [(&str, usize, usize); 8] = [
        // The definition, which calls `effective_enumerator_value` once
        // in its `Declared` arm.
        ("crates/ast-layout/src/xcdr.rs", 0, 1),
        ("crates/wire-codec/src/encode.rs", 1, 0),
        ("crates/wire-codec/src/decode.rs", 1, 1),
        // The plan, which resolves the mapping once so a generator
        // cannot pass the wrong policy to the crossing.
        ("crates/ast-layout/src/plan.rs", 1, 0),
        // The encoder walk, which asks the plan. Zero is the assertion:
        // a generator that started calling the crossing itself would
        // fail here, and one that read the declared value straight
        // would fail on the second number.
        ("crates/lang-rules/src/emit/encode.rs", 0, 0),
        // The decoder walk, which asks the plan for the same mapping
        // the encoder asks for. Zero on both counts. Python is on this
        // walk now, the same as every other `zerodds-lang-rules`-based
        // column, so it has no site of its own here any more —
        // `crates/lang-python/src/emit.rs`'s separate copy, and the
        // separate entry that once caught it missing an axis, are both
        // gone with the crate.
        ("crates/lang-rules/src/emit/decode.rs", 0, 0),
        // The Rust code generator: one crossing, in the helper both
        // `enumeration_encode_fn` and `enumeration_decode_fn` take the
        // enumerator's integer from.
        ("crates/lang-rules/src/encode.rs", 1, 0),
        // The language mapping, which keeps the declared value.
        ("crates/lang-rules/src/adapter/c.rs", 0, 1),
    ];
    for (path, crossings, straight) in SITES {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        let is_definition = path.ends_with("ast-layout/src/xcdr.rs");
        // The definition names itself once in `pub fn`; that is not a
        // call site.
        let found = text.matches("enumerator_wire_value(").count() - usize::from(is_definition);
        assert_eq!(
            found, crossings,
            "{path} calls the crossing {found} times, not {crossings}"
        );
        let raw = text.matches("effective_enumerator_value(").count();
        assert_eq!(
            raw, straight,
            "{path} reads the declared value straight {raw} times, not {straight} — a site \
             that does so for a value reaching the wire bypasses the axis and nothing at run \
             time says so"
        );
    }
}

/// Every axis the stem parses reaches every column's wrapper.
///
/// The shells are the gap and always have been. The last time a setting
/// landed it was missing from `zerodds-idl-python`, and the time before
/// that the shell in front of the C column filtered it out of the flags
/// it forwards — neither was a runtime gap and neither was visible to a
/// runtime test.
///
/// This asserted one flag on five paths and so missed the larger one.
/// When the five generators became renderers on
/// `tools/wireprobe/wireprobe-zerodds-gen`, the flag left the C column's
/// `lib.rs` for the stem and the test failed for the right reason on the
/// wrong file. Reading the wrappers then showed that `wireprobe-zerodds-c`
/// and `-python` forwarded all sixteen `LayoutKey` axes and
/// `-cpp`, `-csharp`, `-java` and `-typescript` forwarded **none** — they
/// named seven flags and answered `` `--length-codes` is not an option ``
/// with exit 1 for the rest. Four of the six columns had never been
/// measured on any axis. Measured after: `--enum-holder declared` gives
/// `01` and `bit-bound-dropped` gives `01000000` in all four, agreeing
/// with C.
///
/// So the list is every axis and the surfaces are every wrapper. A flag
/// a wrapper does not name is a column that cannot be asked, which is
/// invisible to any test that only runs the default layout.
#[test]
fn every_axis_reaches_every_wrapper() {
    const AXES: [&str; 16] = [
        "--collection-delimiters",
        "--enum-holder",
        "--enumerator-value",
        "--fixed-digits",
        "--length-codes",
        "--map-delimiter",
        "--map-entries",
        "--map-length",
        "--pid-flags",
        "--short-pid-range",
        "--union-framing",
        "--union-labels",
        "--wchar-framing",
        "--wchar-width",
        "--wstring-length",
        "--wstring-units",
    ];
    const SURFACES: [&str; 8] = [
        "tools/wireprobe/wireprobe-zerodds-python-gen/src/lib.rs",
        "tools/wireprobe/wireprobe-zerodds-gen/src/lib.rs",
        "tools/wireprobe/wireprobe-zerodds-c",
        "tools/wireprobe/wireprobe-zerodds-cpp",
        "tools/wireprobe/wireprobe-zerodds-csharp",
        "tools/wireprobe/wireprobe-zerodds-java",
        "tools/wireprobe/wireprobe-zerodds-typescript",
        "tools/wireprobe/wireprobe-zerodds-python-walk",
    ];
    let mut missing = Vec::new();
    for path in SURFACES {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        for axis in AXES {
            if !text.contains(axis) {
                missing.push(format!("{path}: {axis}"));
            }
        }
    }
    // `crates/wire-probe/src/main.rs` is not in `SURFACES`: since
    // `crates/ast-layout/src/axes.rs` gave every flag one table, it no
    // longer spells any axis name out itself — it dispatches every
    // `--<axis>` flag it reads through `zerodds_ast_layout::AXES` and
    // `find_axis`, so a name-substring search of its own text would
    // fail for the right list and the wrong reason. What replaces the
    // substring check for this one surface is the guarantee the shared
    // table makes structural: `find_axis` walks the whole of `AXES`
    // unconditionally, so a flag `AXES` carries is a flag this file
    // accepts with no per-flag code path to have dropped it from. The
    // two assertions below are that guarantee, checked rather than
    // assumed — that `AXES` really does carry every name this test
    // measures, and that `crates/wire-probe/src/main.rs` really does
    // dispatch through it and not through some reintroduced per-flag
    // match arm that this test could not see either.
    for axis in AXES {
        assert!(
            zerodds_ast_layout::AXES
                .iter()
                .any(|flag| flag.flag_name() == axis),
            "zerodds_ast_layout::AXES does not carry {axis}, so wire-probe's \
             generic dispatch cannot reach it either"
        );
    }
    let wire_probe_main =
        std::fs::read_to_string(repository_root().join("crates/wire-probe/src/main.rs"))
            .unwrap_or_else(|error| panic!("crates/wire-probe/src/main.rs: {error}"));
    for marker in ["zerodds_ast_layout", "AXES", "find_axis(name)"] {
        assert!(
            wire_probe_main.contains(marker),
            "crates/wire-probe/src/main.rs no longer names `{marker}` — it may have \
             gone back to a per-flag match arm, which this test cannot see the axes of"
        );
    }
    assert!(
        missing.is_empty(),
        "these surfaces do not name these axes, so that column cannot be asked \
         for the setting and a sweep over the axis measures the others only:\n  {}",
        missing.join("\n  ")
    );
}

/// The setting is in `Dialect::OpenDds`, and all four lists drive that
/// dialect.
///
/// It used to be a configuration of its own — a dialect plus this
/// one override — and it is not one any more, because it describes
/// what a measured product puts on the wire and belongs in that
/// product's dialect. So the claim this test makes is two claims:
/// the dialect selects the setting, and every one of the four
/// column lists names the dialect.
#[test]
fn the_configuration_is_named_the_same_everywhere() {
    assert_eq!(
        zerodds_ast_layout::Dialect::OpenDds.enumerator_value(),
        EnumeratorValue::Ordinal,
        "`Dialect::OpenDds` no longer selects the setting this file measures"
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
            text.contains("opendds"),
            "{path} does not drive `--dialect opendds`"
        );
    }
}

fn root_of(ast: &zerodds_ast::Ast, type_name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
        type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the case's type")
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("a hex payload")
        })
        .collect()
}
