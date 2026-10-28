// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **Counter-check.** How many digits a `fixed<P,S>` puts on the wire is
//! answered by the plan's [`FixedDigitCount`] and by nothing else, in
//! every place that writes or reads one.
//!
//! # What is measured
//!
//! `tests/interop-results/sources/corba-corpus.tsv`, every `fixed` case
//! in the corpus, under the two ORBs that produce octets for one.
//! CORBA 3.3 part 2 §9.3.2.8 gives the packing — two digits to the
//! octet, most significant first, the low nibble of the last octet the
//! sign — and says **nothing** about how many digits go out. The four
//! implementations that write a `fixed` split two and two:
//!
//! ```text
//! fixed-basic  xcdr1 be  omniorb  123456789c 00000000 1d ab
//!                        jacorb   123456789c       1d ab
//!                        ours     123456789c 00000000 1d ab
//! ```
//!
//! omniORB 4.3.3 and this project write the declared `P`; JacORB and
//! ACE/TAO 2.5.24's `ACE_CDR::Fixed` write the value's precision.
//! `tao_idl` refuses the corpus's `fixed` IDL so TAO has no recorded
//! row — its runtime is measured in `docs/unspecified_constructs.md` §3
//! — which leaves JacORB as the only recorded column for the second
//! value. **That does not make it one implementation:** judgement 3
//! asks for two implementations reading the clause the same way, and the
//! second one was measured, just not through this corpus.
//!
//! Nothing here is hard-coded. Each recorded row is swept against
//! `Dialect::ALL × FixedDigitCount::ALL` and the coordinate is read off
//! the combinations that reproduce it, in both directions:
//!
//! - every JacORB row that differs is reproduced, and **only** under
//!   [`FixedDigitCount::ValuePrecision`];
//! - every omniORB row is reproduced, and **only** under
//!   [`FixedDigitCount::Declared`] where the two forms differ at all.
//!
//! # The correction this closes
//!
//! `docs/open-rows-breakdown.md` cause K says "the declared count is the
//! only decodable half: a reader must know the octet count from the type
//! before it reads the value". **That is wrong**, and the clause it
//! cites says why: a digit nibble is `0` to `9` and a sign nibble is
//! `0xA` upwards, and the nibble count is even by construction, so the
//! field ends at the first octet whose **low** nibble is at least `0xA`.
//! The value-precision form is self-delimiting.
//! [`the_value_precision_form_reads_back`] puts JacORB's own octets
//! through this project's decoder and requires the case's own sample,
//! which is the same check group G rests on.
//!
//! It is also the fourth instance of the conflation
//! `docs/wire-axes-followup.md` swept for: a property of the form was
//! used as a reason not to build it.
//!
//! # It **is** a planner setting, unlike the last two
//!
//! `MapEntryOrder` and `UnionLabelMatch` are both out of
//! `LayoutKey::offset_key` and both say so as a test. This one is
//! **in** it: a `fixed<9,3>` is five octets under one value and as wide
//! as the value needs under the other, so the type stops being
//! fixed-size and every member behind it moves.
//! [`the_extent_is_unbounded_under_the_value`] is that claim, and it is
//! the separation — a cache that left the axis out would hand a
//! five-octet plan to a caller that asked for the value's digits.
//!
//! # Mutation-checked
//!
//! Six mutations, each applied to the file and reverted from a copy of
//! its bytes — never with `git checkout --`, and never with a restore
//! that carries the original mtime back, which is how the map-entry run
//! contaminated itself once. What each one turns red is measured and
//! recorded, the one that survives included.
//!
//! | mutation | what goes red |
//! |---|---|
//! | **value** — `FixedDigitCount::wire_digits` returning `declared` for `ValuePrecision` | [`the_six_rows_are_told_apart`], [`only_the_values_with_leading_zeros_differ`], [`a_zero_is_still_one_digit`]. **Not** [`the_value_precision_form_reads_back`]: the reader scans for the sign nibble and never asks the crossing, which is the separation between the two halves of this axis |
//! | **the floor** — the value's count allowed to reach zero instead of one | [`a_zero_is_still_one_digit`] **only**, which is the `BigDecimal("0.00").precision()` case and the one no corpus row reaches — every recorded `fixed` value is non-zero |
//! | **the extent** — the `Fixed` plan arm keeping `Extent::Sized` under the value's count | [`the_extent_is_unbounded_under_the_value`] only, which is what that check exists for |
//! | **the scan** — the reader taking `P` octets under the value's count | [`the_value_precision_form_reads_back`], [`the_round_trip_is_stable_under_both_values`], [`a_zero_is_still_one_digit`] |
//! | **the point** — the reader splitting at `P - S` from the left instead of `S` from the right | [`the_value_precision_form_reads_back`], [`the_round_trip_is_stable_under_both_values`] |
//! | **the sign threshold** — `FIRST_SIGN_NIBBLE` at `0x0b` instead of `0x0a` | **survives.** §9.3.2.8 admits `0xa` and `0xb` as signs and every recorded `fixed` carries `0xc` or `0xd`, so no corpus value separates the two thresholds. The read half's acceptance of `0xa` and `0xb` is a property of the clause and not of a measurement, and it is written down rather than left out |

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{
    DepCollector, Dialect, Encoding, Endianness, Extent, FixedDigitCount, LayoutKey, Purpose, plan,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

// The two ORBs that produce `fixed` octets, and the value each writes.
const PEERS: [(&str, FixedDigitCount); 2] = [
    ("omniorb", FixedDigitCount::Declared),
    ("jacorb", FixedDigitCount::ValuePrecision),
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

// Every corpus case whose IDL declares a `fixed`.
//
// Read from the directory and not from a list: a `fixed` case added
// beside the ones this file was written against has to be measured by
// it.
fn fixed_cases() -> Vec<String> {
    let directory = repository_root().join("tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("tests/interop-corpus") {
        let entry = entry.expect("a corpus entry");
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(text) = std::fs::read_to_string(entry.path().join("type.idl")) else {
            continue;
        };
        if text.contains("fixed<") {
            out.push(name);
        }
    }
    out.sort();
    assert!(
        out.len() >= 6,
        "only {} fixed cases were found — this check must not pass by finding nothing",
        out.len()
    );
    out
}

// Every recorded write row of `corba-corpus.tsv`, keyed by
// (case, system, order).
//
// Five fields and not six: a CORBA ORB has no encoding version 2, so
// the file carries no version column. A refusal is stored behind an
// `ERR1:` prefix in the payload column, which `sources.rs::read_corba`
// strips; here a cell that is not hex is simply not a row this file
// measures.
fn recorded() -> BTreeMap<(String, String, String), String> {
    let mut rows = BTreeMap::new();
    let path = repository_root().join("tests/interop-results/sources/corba-corpus.tsv");
    let text = std::fs::read_to_string(path).expect("tests/interop-results/sources");
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 || f[3] != "write" {
            continue;
        }
        if f[4].is_empty() || !f[4].bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        rows.insert(
            (f[0].to_string(), f[1].to_string(), f[2].to_string()),
            f[4].to_string(),
        );
    }
    rows
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

type Combination = (Dialect, FixedDigitCount);

fn combinations() -> Vec<Combination> {
    let mut out = Vec::new();
    for dialect in Dialect::ALL {
        for count in FixedDigitCount::ALL {
            out.push((dialect, count));
        }
    }
    out
}

fn request_for(
    case: &str,
    combination: Combination,
) -> (zerodds_ast::Ast, ProbeRequest, CaseFiles) {
    let files = case_files(case);
    let ast = model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), combination.0)
        .expect("the case lowers");
    let mut request = ProbeRequest::in_dialect(
        &files.type_name,
        Encoding::PlainCdr1,
        Endianness::Big,
        combination.0,
    );
    request.fixed_digits = combination.1;
    (ast, request, files)
}

// What this project writes for one corpus case under one combination.
fn written(case: &str, combination: Combination) -> Option<String> {
    let files = case_files(case);
    let ast =
        model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), combination.0).ok()?;
    let mut request = ProbeRequest::in_dialect(
        &files.type_name,
        Encoding::PlainCdr1,
        Endianness::Big,
        combination.0,
    );
    request.fixed_digits = combination.1;
    encode(&ast, &request, &files.values).ok().map(|b| hex(&b))
}

// Whether two byte strings agree on every octet the peer determined.
//
// `tools/wireprobe/omniorb-template.cpp` and `tao-template.cpp` write
// `0xAA` at the alignment fill on purpose: measured over GIOP, both
// ORBs put the *previous request's* octets there
// (`tools/wire-capture`), so the fill is not a function of the sample
// and no buffer content records it. The marker names those positions
// instead of hiding them under a zero.
//
// So a comparison against a recorded ORB row is a comparison over the
// octets the ORB decided, and an `aa` where this project wrote `00` is
// the marker rather than a disagreement. It forgives nothing else: the
// lengths have to match, and an `aa` where this project wrote a value
// is a difference in that value.
fn agrees_where_the_peer_decided(mine: &str, theirs: &str) -> bool {
    if mine.len() != theirs.len() {
        return false;
    }
    mine.as_bytes()
        .chunks_exact(2)
        .zip(theirs.as_bytes().chunks_exact(2))
        .all(|(a, b)| a == b || (a == b"00" && b == b"aa"))
}

fn matching(case: &str, theirs: &str) -> Vec<Combination> {
    combinations()
        .into_iter()
        .filter(|combination| {
            written(case, *combination)
                .is_some_and(|mine| agrees_where_the_peer_decided(&mine, theirs))
        })
        .collect()
}

// The six rows: the two values are told apart by the recorded octets,
// in both directions.
//
// Every JacORB row of the three constructs and their twins is
// reproduced, and **only** under [`FixedDigitCount::ValuePrecision`];
// the omniORB row of the same case is reproduced, and **only** under
// [`FixedDigitCount::Declared`]. That both halves hold is what makes
// this a measurement of the axis rather than of the corpus.
#[test]
fn the_six_rows_are_told_apart() {
    let rows = recorded();
    assert!(
        !rows.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check must not \
         pass by finding nothing"
    );
    const DIVERGENT: [&str; 6] = [
        "fixed-basic",
        "fixed-basic-unannotated",
        "sequence-fixed",
        "sequence-fixed-unannotated",
        "array-1d-fixed",
        "array-1d-fixed-unannotated",
    ];
    let mut seen = 0usize;
    for case in DIVERGENT {
        for (system, expected) in PEERS {
            let key = (case.to_string(), system.to_string(), "be".to_string());
            let Some(theirs) = rows.get(&key) else {
                panic!("{case} {system} be is not in corba-corpus.tsv");
            };
            let found = matching(case, theirs);
            assert!(
                !found.is_empty(),
                "{case} {system}: none of the {} combinations reproduce {theirs}",
                combinations().len()
            );
            // `ValueStringDigits` counts the integer digits as written
            // plus `S`; for these values it coincides with one of the
            // two the ORBs write, and the rows that pin it are ACE's
            // (`unspecified_constructs.md` §3, held in
            // `crates/wire/tests/wire.rs`).
            for (dialect, count) in found
                .iter()
                .filter(|(_, count)| *count != FixedDigitCount::ValueStringDigits)
            {
                assert_eq!(
                    *count, expected,
                    "{case} {system} is reproduced under {dialect:?} with the other digit \
                     count, so the recorded octets do not pin the axis"
                );
            }
            seen += 1;
        }
    }
    assert_eq!(
        seen, 12,
        "six constructs against two peers is twelve rows and {seen} were measured"
    );
}

// Only a value with leading zeros differs, and this derives that rather
// than quoting it.
//
// The completeness half of cause K. Every `fixed` case is read out of
// the corpus directory; a case whose every `fixed` value already uses
// its full declared width has to be reproduced under **both** values of
// the axis, and a case with a shorter value under one only.
//
// `fixed-scales` is the case that makes this exact rather than
// inferred: its `fixed<31,10>` holds thirty significant digits and pads
// to sixteen octets under both readings, so it agrees while carrying a
// leading zero. The test therefore compares octets and not digit
// counts.
#[test]
fn only_the_values_with_leading_zeros_differ() {
    let rows = recorded();
    let mut agreed = 0usize;
    let mut diverged = 0usize;
    for case in fixed_cases() {
        let mut both = 0usize;
        for (system, _) in PEERS {
            let key = (case.clone(), system.to_string(), "be".to_string());
            let Some(theirs) = rows.get(&key) else {
                continue;
            };
            both += 1;
            let found = matching(&case, theirs);
            assert!(
                !found.is_empty(),
                "{case} {system}: nothing reproduces {theirs}"
            );
            let under_both = FixedDigitCount::ALL
                .into_iter()
                .all(|count| found.iter().any(|(_, seen)| *seen == count));
            if under_both {
                agreed += 1;
            } else {
                diverged += 1;
            }
        }
        assert!(
            both == 0 || both == 2,
            "{case}: {both} of the two ORBs produced octets, so the two forms cannot be \
             compared on it"
        );
    }
    assert_eq!(
        diverged, 12,
        "the corpus separates the two values on twelve rows and {diverged} were found"
    );
    assert!(
        agreed >= 4,
        "only {agreed} rows agreed under both values — this check must not pass by finding \
         nothing"
    );
}

// JacORB's own octets read back as the case's own sample.
//
// The correction to cause K's "the declared count is the only decodable
// half". §9.3.2.8 makes a digit `0` to `9` and a sign `0xA` upwards,
// and the nibble count is even by construction, so the field ends at
// the first octet whose low nibble is a sign — the form is
// self-delimiting and this is the reader that shows it.
//
// The same check group G rests on: the peer's bytes through this
// project's decoder under the plan they were compared against,
// required to yield the case's own sample.
#[test]
fn the_value_precision_form_reads_back() {
    let rows = recorded();
    let mut checked = 0usize;
    for case in fixed_cases() {
        let key = (case.clone(), "jacorb".to_string(), "be".to_string());
        let Some(theirs) = rows.get(&key) else {
            continue;
        };
        let (ast, request, files) =
            request_for(&case, (Dialect::JacOrb, FixedDigitCount::ValuePrecision));
        let root = root_of(&ast, &files.type_name);
        let expected = text(&canonicalise(&ast, root, &files.values, "sample").expect("canonical"));
        let read = decode(&ast, &request, &unhex(theirs))
            .unwrap_or_else(|error| panic!("{case}: JacORB's octets do not decode: {error}"));
        assert_eq!(
            text(&read),
            expected,
            "{case}: JacORB's octets do not read back as the sample"
        );
        checked += 1;
    }
    assert!(
        checked >= 6,
        "only {checked} JacORB rows were read back — this check must not pass by finding nothing"
    );
}

// A zero is one digit and the sign, not none.
//
// `new BigDecimal("0.00").precision()` is 1 in Java, which is what
// `docs/corba-column.md` §3 traced JacORB's behaviour to, and it is
// the case **no corpus row reaches**: every recorded `fixed` value is
// non-zero. Written as a direct measurement of the crossing and of the
// three writers, so that the floor is not a claim.
#[test]
fn a_zero_is_still_one_digit() {
    assert_eq!(
        FixedDigitCount::ValuePrecision.wire_digits(9, 0, 0),
        1,
        "a value with no significant digit still needs one"
    );
    assert_eq!(FixedDigitCount::ValuePrecision.wire_digits(9, 4, 4), 4);
    assert_eq!(FixedDigitCount::Declared.wire_digits(9, 4, 4), 9);
    // ValueStringDigits: string "0.000" has 4 digits (even) -> 5 digits
    assert_eq!(FixedDigitCount::ValueStringDigits.wire_digits(9, 0, 4), 5);
    // ValueStringDigits: string "123.45" has 5 digits (odd) -> 5 digits
    assert_eq!(FixedDigitCount::ValueStringDigits.wire_digits(9, 5, 5), 5);
    // ValueStringDigits: string "0.12345" has 6 digits (even) -> 7 digits
    assert_eq!(FixedDigitCount::ValueStringDigits.wire_digits(9, 5, 6), 7);

    let idl = "typedef fixed<9,3> Money;\n@final struct Sample { Money zero; octet tail; };\n";
    let ast = model_from_idl_in_dialect(idl, "zero.idl", Dialect::JacOrb).expect("the IDL lowers");
    let values = serde_json::json!({ "zero": "0.000", "tail": 171 });
    let mut request = ProbeRequest::in_dialect(
        "Sample",
        Encoding::PlainCdr1,
        Endianness::Big,
        Dialect::JacOrb,
    );
    request.fixed_digits = FixedDigitCount::ValuePrecision;
    let bytes = encode(&ast, &request, &values).expect("a zero writes");
    assert_eq!(
        hex(&bytes),
        "0cab",
        "a zero under the value's count is the single nibble 0 and the sign"
    );
    let read = decode(&ast, &request, &bytes).expect("a zero reads back");
    assert_eq!(
        text(&read),
        text(&canonicalise(&ast, root_of(&ast, "Sample"), &values, "sample").expect("canonical"))
    );
}

// The extent is unbounded under the value's count, so the axis belongs
// in `LayoutKey::offset_key`.
//
// The opposite claim from `MapEntryOrder` and `UnionLabelMatch`, and it
// is why this one is a planner setting: a `fixed<9,3>` is five octets
// under the declaration and as wide as the value needs under the other
// value, so the type is not fixed-size and every member behind it
// moves.
#[test]
fn the_extent_is_unbounded_under_the_value() {
    let idl = "typedef fixed<9,3> Money;\n@final struct Sample { Money m; octet tail; };\n";
    let ast = model_from_idl_in_dialect(idl, "extent.idl", Dialect::JacOrb).expect("lowers");
    let money = ast
        .all_members_of(root_of(&ast, "Sample"))
        .into_iter()
        .find_map(|id| ast.member(id).ok().and_then(|node| node.type_.resolved()))
        .expect("the member");
    // The **declared** side, so the dialect has to be one that takes the
    // declared count. `Dialect::JacOrb` selects `ValuePrecision` since
    // it became JacORB's own preset, which is what this test was
    // reaching for and is now the other half of the comparison below.
    let base = LayoutKey::in_dialect(
        Dialect::Specification,
        Encoding::PlainCdr1,
        Endianness::Big,
        Purpose::Data,
    );
    let mut deps = DepCollector::new();
    let declared = plan(&ast, money, base, &mut deps).expect("plans");
    assert_eq!(
        declared.extent,
        Extent::Sized(5),
        "fixed<9,3> is ceil((9 + 1) / 2) octets under the declaration"
    );
    let mut deps = DepCollector::new();
    let value = plan(
        &ast,
        money,
        base.with_fixed_digits(FixedDigitCount::ValuePrecision),
        &mut deps,
    )
    .expect("plans");
    assert_eq!(
        value.extent,
        Extent::Unbounded,
        "the value's count makes a fixed variable-length"
    );
    assert_ne!(
        base.offset_key(),
        base.with_fixed_digits(FixedDigitCount::ValuePrecision)
            .offset_key(),
        "the digit count must reach the plan cache's key, or a five-octet plan is served to a \
         caller that asked for the value's digits"
    );
    // ValueStringDigits also makes the extent unbounded.
    let mut deps = DepCollector::new();
    let string_digits = plan(
        &ast,
        money,
        base.with_fixed_digits(FixedDigitCount::ValueStringDigits),
        &mut deps,
    )
    .expect("plans");
    assert_eq!(
        string_digits.extent,
        Extent::Unbounded,
        "the value's string digit count makes a fixed variable-length"
    );
    assert_ne!(
        base.offset_key(),
        base.with_fixed_digits(FixedDigitCount::ValueStringDigits)
            .offset_key(),
        "ValueStringDigits must reach the plan cache's key"
    );
}

// Writing what was read gives the same octets under `Declared` and
// `ValuePrecision`, and the same value under `ValueStringDigits`.
//
// That one counts the integer digits *as written*, and the reader
// answers the canonical decimal: `-000000.001` under `fixed<9,3>` goes
// out with nine digits, reads back as `-0.001` and goes out again with
// five. The value is the same; the representation is what the count is
// about.
#[test]
fn the_round_trip_is_stable_under_every_value() {
    for case in fixed_cases() {
        for count in FixedDigitCount::ALL {
            let (ast, request, files) = request_for(&case, (Dialect::JacOrb, count));
            let bytes = encode(&ast, &request, &files.values)
                .unwrap_or_else(|error| panic!("{case} {count:?}: {error}"));
            let read = decode(&ast, &request, &bytes)
                .unwrap_or_else(|error| panic!("{case} {count:?}: {error}"));
            let again = encode(&ast, &request, &read).unwrap_or_else(|error| {
                panic!("{case} {count:?}: the read-back does not write: {error}")
            });
            if count == FixedDigitCount::ValueStringDigits {
                let read_again = decode(&ast, &request, &again)
                    .unwrap_or_else(|error| panic!("{case} {count:?}: {error}"));
                assert_eq!(
                    read, read_again,
                    "{case} {count:?}: the read-back sample does not read back as itself"
                );
            } else {
                assert_eq!(
                    hex(&bytes),
                    hex(&again),
                    "{case} {count:?}: writing the read-back sample gives other octets"
                );
            }
        }
    }
}

// Every place that writes or reads a `fixed` goes through the crossing
// or names the policy.
//
// The Rust column reaches the crossing directly; the C and Python
// columns name the policy on the call into their own runtimes, which
// implement the count from §9.3.2.8 themselves. That is the same
// arrangement the packing has, and it is what makes the three columns
// three readings rather than one — the three-column gate is where they
// are held against each other.
#[test]
fn the_sites_go_through_the_crossing() {
    const SITES: [(&str, usize); 8] = [
        // The definition, once in its signature.
        ("crates/ast-layout/src/xcdr.rs", 1),
        // The writer reaches the count through the byte layer's own copy
        // of the axis rather than the planner's: `zerodds-wire` does not
        // depend on `zerodds-ast-layout` and must not. The two halves are
        // held against each other in
        // `crates/wire-codec/tests/fixed_digits_agree.rs`.
        ("crates/wire/src/writer.rs", 0),
        ("crates/wire-codec/src/encode.rs", 0),
        // The reader reaches the count the same way the writer does, and
        // for the same reason.
        ("crates/wire/src/reader.rs", 0),
        // The reader takes the policy and scans; it has no digit count
        // to cross until it has read one.
        ("crates/wire-codec/src/decode.rs", 0),
        // Python is on this walk now, like every other
        // `zerodds-lang-rules`-based column: `crates/lang-python/src/emit.rs`'s
        // separate copy — which also crossed zero times — is gone with
        // the crate, and `wireprobe-zerodds-python-gen` only passes the
        // policy through the `LayoutKey` these two already cover.
        ("crates/lang-rules/src/emit/encode.rs", 0),
        ("crates/lang-rules/src/emit/decode.rs", 0),
        ("crates/ast-layout/src/key.rs", 0),
    ];
    for (path, crossings) in SITES {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        let found = text.matches("fixed_wire_digits(").count();
        assert_eq!(
            found, crossings,
            "{path} names the crossing {found} times, not {crossings}"
        );
    }
    // And every site that writes or reads a `fixed` says which count it
    // is asking for, rather than taking `P` straight.
    for (path, needle, count) in [
        // The writer, its crossing into the byte layer's copy, and that
        // crossing's three arms.
        ("crates/wire-codec/src/encode.rs", "FixedDigitCount", 6usize),
        // The count itself, asked once by the writer and once by the
        // byte layer's copy of the axis.
        ("crates/wire/src/writer.rs", "wire_digits(", 2),
        // The reader and its crossing into the byte layer's copy.
        ("crates/wire-codec/src/decode.rs", "FixedDigitCount", 2),
        // The import, the signature, the three values the match on it
        // names, and the two mentions in the doc comment that say which
        // reads up to the sign nibble.
        ("crates/wire/src/reader.rs", "FixedDigits", 7),
        (
            "crates/lang-rules/src/emit/encode.rs",
            "fixed_digit_count(",
            2,
        ),
        // The decoder emitter's match on the count, one arm per value.
        (
            "crates/lang-rules/src/emit/decode.rs",
            "FixedDigitCount::",
            3,
        ),
        ("runtime/c/src/writer.c", "ZDW_FIXED_VALUE_PRECISION", 1),
        ("runtime/c/src/reader.c", "ZDW_FIXED_VALUE_PRECISION", 1),
        (
            "runtime/python/zerodds_wire/writer.py",
            "value-precision",
            3,
        ),
        (
            "runtime/python/zerodds_wire/reader.py",
            "value-precision",
            3,
        ),
    ] {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(
            text.matches(needle).count(),
            count,
            "{path} names `{needle}` a different number of times than {count} — a `fixed` site \
             that takes P straight bypasses the axis and nothing at run time says so"
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

// The setting is in `Dialect::JacOrb`, and all four lists drive that
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
        zerodds_ast_layout::Dialect::JacOrb.fixed_digits(),
        FixedDigitCount::ValuePrecision,
        "`Dialect::JacOrb` no longer selects the setting this file measures"
    );
    // OpenDDS writes the declared count (`Fixed_T<P,S>`), measured
    // through the FACE mapping; `ValueStringDigits` is ACE's own operator
    // and no dialect's.
    assert_eq!(
        zerodds_ast_layout::Dialect::OpenDds.fixed_digits(),
        FixedDigitCount::Declared,
        "`Dialect::OpenDds` no longer writes the count OpenDDS writes"
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
            text.contains("jacorb"),
            "{path} does not drive `--dialect jacorb`"
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
