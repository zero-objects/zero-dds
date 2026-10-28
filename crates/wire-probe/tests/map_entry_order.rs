// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **Counter-check.** In what order a `map`'s entries go on the wire is
//! answered by the plan's [`MapEntryOrder`] and by nothing else, in
//! every place that walks them.
//!
//! # What is measured
//!
//! `tests/interop-results/sources/vendor-corpus.tsv`, every `map` case
//! in the corpus, under the one implementation that produces `map`
//! octets at all. Fast DDS is that implementation and it is the only
//! one: the Cyclone, RTI and OpenDDS probes all record
//! `a map, which the IDL-to-C mapping §7.5.1 reuses does not have` or
//! `no CORBA IDL front end on the test host parses`, so there are no
//! octets from them to compare against. That is why no dialect selects
//! [`MapEntryOrder::AscendingKey`] — judgement 3 wants two
//! implementations and one has been asked.
//!
//! ```text
//! map-key-long  xcdr1 be  fastdds  00000003 ffed2979 00000001
//!                                  00000001 00000003 7fffffff fffffffe ffffffff
//!                         ours     00000003 ffed2979 00000001
//!                                  7fffffff fffffffe 00000001 00000003 ffffffff
//! ```
//!
//! A permutation: the same count, the same length, every entry present.
//!
//! Nothing of that is hard-coded here. Each recorded row is swept
//! against `Dialect::ALL × MapEntryOrder::ALL` and the coordinate is
//! read off the combinations that reproduce it.
//!
//! # The evidence is complete, and this file derives it rather than
//! quoting it
//!
//! [`only_the_unsorted_samples_diverge`] reads every `map` case out of
//! `tests/interop-corpus`, decides from the **sample** whether its keys
//! are already ascending, and requires the recorded Fast DDS row to
//! agree with this project exactly when they are. It is the claim
//! "exactly the four unsorted cases diverge" as a measurement over the
//! corpus directory, not as a list of four names — the shape
//! `union_label_match.rs` had to be repaired into after its first
//! version swept the cases it named and found what it was given.
//!
//! # Three corrections to `docs/open-rows-breakdown.md` cause D
//!
//! 1. **`map-key-enum` is not evidence.** The breakdown lists it among
//!    the cases that are "already ascending and all agree", with
//!    `BLUE`=0 and `RED`=1. Its IDL declares `enum Colour { RED, GREEN,
//!    BLUE }`, so the sample `["BLUE", "RED"]` is `[2, 0]` and
//!    **descending**; and `wireprobe-fastdds` records the case as
//!    `refused — fastddsgen failed` in all ten slots, because the key is
//!    a `@bit_bound(16)` enumeration. There are no octets from any
//!    implementation. [`the_enumeration_key_has_no_recorded_octets`] is
//!    that correction as a test, so the arm is not read as measured.
//! 2. **The order is the key's own value order and not one order.** The
//!    corpus separates signed from unsigned — `map-key-long` puts
//!    `-1234567` first and `map-key-ulonglong` puts `7` before
//!    `0xdeadbeefdeadc2be` — and separates an unsigned octet comparison
//!    of a `string` from a signed one, through `map-key-string`'s `üሴ`.
//!    [`the_order_is_the_keys_own_and_not_the_octets`] pins all three.
//! 3. **A key type with no order is a refusal.** The breakdown says
//!    "three writer sites, no plan, no reader" and stops there. A
//!    `double` key has no total order and an aggregate key has no
//!    `operator<` at all, so `AscendingKey` cannot be honoured for one;
//!    writing the declaration order there would be a flag that reported
//!    success and changed no octet.
//!    [`the_key_types_with_no_order_are_refused`] is the refusal.
//!
//! # Mutation-checked, at the crossing and at the coordinate
//!
//! Nine mutations were run, each applied to the file and reverted from a
//! copy of its bytes. What each one turns red is recorded, **including
//! the four that this file does not catch** — a mutation whose colour is
//! not written down is a mutation somebody has to run again.
//!
//! | mutation | what goes red |
//! |---|---|
//! | **value** — `MapEntryOrder::is_default` answering `true` for `AscendingKey`, so the crossing never fires | five: [`the_thirty_two_rows_are_told_apart`], [`only_the_unsorted_samples_diverge`], [`the_order_is_the_keys_own_and_not_the_octets`], [`the_key_types_with_no_order_are_refused`], [`the_round_trip_is_stable_under_the_rule`] |
//! | **the axis's own arms swapped** — `key_ordering` exchanging the signed and unsigned integer arms | [`the_order_is_the_keys_own_and_not_the_octets`], on `map-key-long` and `map-key-ulonglong` at once |
//! | **the coordinate** — a `string` key compared as **signed** octets | [`the_order_is_the_keys_own_and_not_the_octets`]: `map-key-string`'s `üሴ` moves to the front, and Fast DDS leaves it last |
//! | **the refusal** — `map_entry_order` falling back to declaration order for a key type with no value order | [`the_key_types_with_no_order_are_refused`] and [`the_sites_go_through_the_crossing`], which is what the second count in that check exists for |
//! | **the coordinate, the other way** — a `char` key compared **unsigned** | **survives.** No `map` key in the corpus is at or above `0x80` — `map-key-char` is `'a'`, `'Z'`, `'~'` — so no recorded octet separates the two readings. The answer is written from the C++ mapping and is stated in [`MapEntryOrder`] as unmeasured rather than left to be discovered |
//! | **the tie-break** — equal keys ordered by *descending* declared position | **survives.** No corpus sample declares two entries with the same key, which a `map` is not supposed to have. The tie-break is there so the three columns agree on a sample the corpus does not carry, and nothing measures it |
//! | **the C column bypassed** — `encode_c.rs` asking the crossing and walking the entries in declaration order anyway | **nothing here.** No test in this file links the C emitter. `three_column_gate.rs` catches it — see below |
//! | **the Python column bypassed** — `emit.rs` the same way | **nothing here.** `three_column_gate.rs` catches it, `RED in 379s` |
//! | **the C column bypassed, against the fast three-column gate** | **survived the first time it was run**, over all 18445 C slots. `representative` picks one case per construct family and the `map` family's first is `map-bounded`, whose two entries are already ascending — so the fast tier's C column wrote the same octets either way. `map-key-long` is in `EVER_DISAGREED` now and the same mutation is `RED: the_three_columns_agree`, on `map-key-long map-ascending-key/delimited-cdr/be` |
//!
//! The last row is the finding, and it is the one this file could not
//! have produced: a bypass in a column this file does not link is
//! invisible here **and** was invisible in the fast tier, for a reason
//! that is a property of the corpus rather than of the axis. The full
//! tier would have caught it a night later.
//!
//! **The first mutation run was discarded, and the reason is the
//! harness's.** The runner restored each file with `shutil.copy2`, which
//! carries the original mtime back with the contents, and cargo decides
//! freshness by mtime — so the restore left the previous mutation's
//! object file in place and mutations two to eight all ran with mutation
//! one still live. Every one of them read as red. The runner writes the
//! bytes and lets the mtime advance now. Recorded because a mutation
//! table is only worth what its harness is.
//!
//! # It is not a planner setting and not a reader setting
//!
//! [`the_extent_is_the_same_under_both_values`] is the first claim as a
//! test — a permutation writes the same entries, so nothing reaches an
//! offset and [`MapEntryOrder`] stays out of `LayoutKey::offset_key`.
//! [`the_reader_returns_the_sample_under_both_values`] is the second:
//! all 32 rows decode to the case's own sample under either value,
//! because every column sorts a map's entries into the canonical order
//! before handing them back. Cause D is the one cause in
//! `docs/open-rows-breakdown.md` where a reader sees no difference and
//! the wire decision is real all the same, and that is checked here
//! rather than asserted there.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{
    DepCollector, Dialect, Encoding, Endianness, LayoutKey, MapEntryOrder, MapKeyOrdering, Purpose,
    map_key_ordering, plan,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

// The system whose `map` octets exist. One, and the file says why.
const PEER: &str = "fastdds";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

// Every corpus case whose root type reaches a `map`.
//
// Read from the directory and not from a list: a `map` case added
// beside the ones this file was written against has to be measured by
// it, and a list would let it through.
fn map_cases() -> Vec<String> {
    let directory = repository_root().join("tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("tests/interop-corpus") {
        let entry = entry.expect("a corpus entry");
        let name = entry.file_name().to_string_lossy().to_string();
        let idl = entry.path().join("type.idl");
        let Ok(text) = std::fs::read_to_string(&idl) else {
            continue;
        };
        if text.contains("map<") {
            out.push(name);
        }
    }
    out.sort();
    assert!(
        out.len() >= 20,
        "only {} map cases were found — this check must not pass by finding nothing",
        out.len()
    );
    out
}

// Every recorded row of `vendor-corpus.tsv`, keyed by
// (case, system, version, order), with the outcome and the payload.
fn recorded() -> BTreeMap<(String, String, String, String), (String, String)> {
    let mut rows = BTreeMap::new();
    let path = repository_root().join("tests/interop-results/sources/vendor-corpus.tsv");
    let text = std::fs::read_to_string(path).expect("tests/interop-results/sources");
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        rows.insert(
            (
                f[0].to_string(),
                f[1].to_string(),
                f[2].to_string(),
                f[3].to_string(),
            ),
            (
                f[4].to_string(),
                f.get(5).map_or(String::new(), |p| (*p).to_string()),
            ),
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

// One point of the space a map sample sits in: which peer's answers to
// everything else, and in what order the entries go out.
type Combination = (Dialect, MapEntryOrder);

fn combinations() -> Vec<Combination> {
    let mut out = Vec::new();
    for dialect in Dialect::ALL {
        for order in MapEntryOrder::ALL {
            out.push((dialect, order));
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

// What this project writes for one corpus case under one combination.
fn written(
    case: &str,
    encoding: Encoding,
    endianness: Endianness,
    combination: Combination,
) -> Option<String> {
    let files = case_files(case);
    let ast =
        model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), combination.0).ok()?;
    let mut request =
        ProbeRequest::in_dialect(&files.type_name, encoding, endianness, combination.0);
    request.map_entries = combination.1;
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

// The four cases the corpus separates the two values on, and their
// unannotated twins. Derived rather than declared — see
// [`only_the_unsorted_samples_diverge`], which is what checks that this
// list is the whole of it.
const UNSORTED: [&str; 8] = [
    "map-key-boolean",
    "map-key-boolean-unannotated",
    "map-key-char",
    "map-key-char-unannotated",
    "map-key-long",
    "map-key-long-unannotated",
    "map-key-ulonglong",
    "map-key-ulonglong-unannotated",
];

// The 32 rows: the two values are told apart by the recorded octets, in
// both directions.
//
// Every recorded Fast DDS row of the eight constructs is reproduced,
// and **only** under [`MapEntryOrder::AscendingKey`]. That the other
// value does not reproduce it is the half that makes this a measurement
// of the axis rather than of the corpus: a build where the setting
// never reaches the writer reproduces the declaration order and nothing
// of the peer's.
#[test]
fn the_thirty_two_rows_are_told_apart() {
    let rows = recorded();
    assert!(
        !rows.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check must not \
         pass by finding nothing"
    );
    let mut seen = 0usize;
    for case in UNSORTED {
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let key = (
                    case.to_string(),
                    PEER.to_string(),
                    version.to_string(),
                    endian.to_string(),
                );
                let Some((outcome, theirs)) = rows.get(&key) else {
                    panic!("{case} {PEER} {version} {endian} is not in vendor-corpus.tsv");
                };
                assert_eq!(
                    outcome, "wrote",
                    "{case} {PEER} {version} {endian} carries no octets"
                );
                let found = matching(case, version, endian, theirs);
                assert!(
                    !found.is_empty(),
                    "{case} {PEER} {version} {endian}: none of the {} combinations reproduce \
                     {theirs}",
                    combinations().len()
                );
                for (dialect, order) in &found {
                    assert_eq!(
                        *order,
                        MapEntryOrder::AscendingKey,
                        "{case} {PEER} {version} {endian} is reproduced under {dialect:?} with \
                         the declaration order, so the recorded octets do not pin the axis"
                    );
                }
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 32, "the cause is 32 rows and {seen} were measured");
}

// Exactly the cases whose sample is not already in ascending key order
// diverge, and every other `map` case agrees under both values.
//
// The completeness half of cause D, derived from the corpus rather than
// from a list of four names. It reads every `map` case, asks the
// **sample** whether its top-level map is already sorted, and requires:
//
// - an already-sorted case's recorded Fast DDS row to be reproduced
//   under both values of the axis — the setting changes nothing there;
// - an unsorted case's row to need [`MapEntryOrder::AscendingKey`].
//
// A case with no recorded octets is skipped **loudly**: it is counted
// and the count is asserted, so a corpus where Fast DDS stopped
// answering cannot make this pass by finding nothing.
#[test]
fn only_the_unsorted_samples_diverge() {
    let rows = recorded();
    let mut agreed = 0usize;
    let mut diverged = 0usize;
    let mut without_octets = 0usize;
    for case in map_cases() {
        let sorted = sample_is_sorted(&case);
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let key = (
                    case.clone(),
                    PEER.to_string(),
                    version.to_string(),
                    endian.to_string(),
                );
                let Some((outcome, theirs)) = rows.get(&key) else {
                    without_octets += 1;
                    continue;
                };
                if outcome != "wrote" {
                    without_octets += 1;
                    continue;
                }
                let found = matching(&case, version, endian, theirs);
                let by_declaration = found
                    .iter()
                    .any(|(_, order)| *order == MapEntryOrder::Declaration);
                match sorted {
                    // Already ascending: this project's declaration
                    // order is the ascending order, so the row is
                    // reproduced whatever the axis says. A row nothing
                    // reproduces is a different question — the string
                    // key's DHEADER, which `--dialect rti` reaches — and
                    // is not counted either way.
                    Some(true) => {
                        if !found.is_empty() {
                            assert!(
                                by_declaration,
                                "{case} {version} {endian}: the sample is already ascending, so \
                                 the recorded row must be reproduced without the setting"
                            );
                            agreed += 1;
                        }
                    }
                    Some(false) => {
                        assert!(
                            !found.is_empty() && !by_declaration,
                            "{case} {version} {endian}: the sample is not ascending, so the \
                             recorded row must need --map-entries ascending-key"
                        );
                        diverged += 1;
                    }
                    // A shape whose top-level member is not a map — a
                    // sequence of maps, an array of maps, a map inside a
                    // `@mutable` struct. Their inner maps are single or
                    // empty, so they cannot separate the values either;
                    // they are counted as agreeing when they reproduce.
                    None => {
                        if !found.is_empty() {
                            assert!(
                                by_declaration,
                                "{case} {version} {endian}: no top-level map to sort, so the \
                                 recorded row must be reproduced without the setting"
                            );
                            agreed += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(
        diverged, 32,
        "the corpus separates the two values on 32 rows and {diverged} were found"
    );
    assert!(
        agreed >= 40,
        "only {agreed} already-sorted rows agreed — this check must not pass by finding nothing"
    );
    assert!(
        without_octets > 0,
        "no map case is recorded without octets, which contradicts map-key-enum's refusal"
    );
}

// Whether the case's sample has a top-level `entries` map whose keys are
// already in ascending order, or `None` where there is no top-level map
// or the key type is one this helper will not rank.
//
// The order is decided from the sample's JSON and from the key type's
// **kind**, and deliberately **not** from [`map_key_ordering`]'s
// answer: a helper that asked the crossing would agree with the writer
// by construction and could not separate a wrong order from a right
// one. The kind is a structural fact of the IDL and carries no setting.
//
// The key type is what tells a one-character `string` from a `char`.
// `typedef-of-map`'s keys are `"alpha"` and `"ü"`, and a helper that
// guessed from the JSON read the second as a code point and the first
// as text, which made an ascending sample look descending. Measured:
// that guess put this file's first run red on a case that agrees.
//
// An **enumeration** key answers `None`. Ranking one needs the declared
// `@value`s, and the only case that has one — `map-key-enum` — carries
// no recorded octets from any implementation, so there is nothing for a
// rank to be checked against. [`the_enumeration_key_has_no_recorded_octets`]
// is where that is recorded.
fn sample_is_sorted(case: &str) -> Option<bool> {
    let files = case_files(case);
    let entries = files.values.get("entries")?.as_array()?;
    let keys: Vec<&serde_json::Value> = entries
        .iter()
        .filter_map(|entry| entry.as_array()?.first())
        .collect();
    if keys.len() != entries.len() {
        return None;
    }
    let kind = map_key_kind(case)?;
    if keys.len() < 2 {
        return Some(true);
    }
    let rank = |value: &serde_json::Value| -> Option<(i128, Vec<u8>)> {
        match kind {
            KeyKind::Boolean => Some((i128::from(value.as_bool()?), Vec::new())),
            // Signed on the recorded host; the corpus carries no `char`
            // at or above `0x80` and `--map-entries` says so out loud.
            KeyKind::Character => {
                let point = i128::from(u32::from(value.as_str()?.chars().next()?));
                Some((
                    if point >= 0x80 { point - 0x100 } else { point },
                    Vec::new(),
                ))
            }
            KeyKind::Integer => match value.as_i64() {
                Some(number) => Some((i128::from(number), Vec::new())),
                None => value
                    .as_str()?
                    .parse::<i128>()
                    .ok()
                    .map(|n| (n, Vec::new())),
            },
            KeyKind::Text => Some((0, value.as_str()?.as_bytes().to_vec())),
        }
    };
    let mut ranked = Vec::new();
    for key in keys {
        ranked.push(rank(key)?);
    }
    Some(ranked.windows(2).all(|pair| pair[0] <= pair[1]))
}

// The shape of a map key, as far as this file's own ranking needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyKind {
    Boolean,
    Character,
    Integer,
    Text,
}

// The kind of a case's top-level map key, read from the IDL.
fn map_key_kind(case: &str) -> Option<KeyKind> {
    let files = case_files(case);
    let ast = model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::FastDds)
        .ok()?;
    let root = root_of(&ast, &files.type_name);
    let member = ast
        .all_members_of(root)
        .into_iter()
        .find_map(|id| ast.member(id).ok().map(|node| node.type_.resolved()))??;
    let member = zerodds_wire_probe::contract::resolve_alias(&ast, member);
    let zerodds_ast::TypeKind::Map { key, .. } = &ast.type_(member).ok()?.kind else {
        return None;
    };
    let key = zerodds_wire_probe::contract::resolve_alias(&ast, key.resolved()?);
    match &ast.type_(key).ok()?.kind {
        zerodds_ast::TypeKind::Primitive(kind) => Some(match kind {
            zerodds_ast::PrimitiveKind::Boolean => KeyKind::Boolean,
            zerodds_ast::PrimitiveKind::Char | zerodds_ast::PrimitiveKind::WChar => {
                KeyKind::Character
            }
            zerodds_ast::PrimitiveKind::Float32
            | zerodds_ast::PrimitiveKind::Float64
            | zerodds_ast::PrimitiveKind::Float128 => return None,
            _ => KeyKind::Integer,
        }),
        zerodds_ast::TypeKind::String { .. } => Some(KeyKind::Text),
        _ => None,
    }
}

// The order is the key's own value order, and the corpus separates
// three readings of it.
//
// Not asserted from the enum: each claim is a recorded octet string
// this project has to reproduce, and the wrong reading is shown not to.
//
// - a **signed** integer key, from `map-key-long` — `ffed2979` first;
// - an **unsigned** integer key, from `map-key-ulonglong` — `7` before
//   `0xdeadbeefdeadc2be`, which a signed reading would put first;
// - an **unsigned octet** comparison of a `string`, from
//   `map-key-string` — `üሴ` last, which a signed octet comparison would
//   put first because its leading octet is `c3`.
//
// The third is the reason [`MapKeyOrdering::CodeUnits`] is named for
// unsigned octets rather than left to each column's `char`.
#[test]
fn the_order_is_the_keys_own_and_not_the_octets() {
    let rows = recorded();
    for (case, version, endian) in [
        ("map-key-long", "xcdr1", "be"),
        ("map-key-ulonglong", "xcdr1", "be"),
        ("map-key-string", "xcdr1", "be"),
    ] {
        let key = (
            case.to_string(),
            PEER.to_string(),
            version.to_string(),
            endian.to_string(),
        );
        let (outcome, theirs) = rows
            .get(&key)
            .unwrap_or_else(|| panic!("{case} {PEER} {version} {endian} is not recorded"));
        assert_eq!(outcome, "wrote", "{case} carries no octets");
        let ours = written(
            case,
            encoding_of(version),
            endianness_of(endian),
            (Dialect::FastDds, MapEntryOrder::AscendingKey),
        )
        .unwrap_or_else(|| panic!("{case} does not encode"));
        assert_eq!(
            &ours, theirs,
            "{case}: the ascending order does not reproduce Fast DDS"
        );
    }

    // And the crossing gives each key type the reading the octets above
    // pin, so a mutation of the coordinate is red here as well as in the
    // bytes.
    for (case, expected) in [
        ("map-key-long", MapKeyOrdering::SignedInteger),
        ("map-key-ulonglong", MapKeyOrdering::UnsignedInteger),
        ("map-key-boolean", MapKeyOrdering::UnsignedInteger),
        ("map-key-char", MapKeyOrdering::SignedInteger),
        ("map-key-string", MapKeyOrdering::CodeUnits),
        ("map-key-enum", MapKeyOrdering::EnumeratorValue),
    ] {
        assert_eq!(
            ordering_of(case),
            Some(expected),
            "{case}: the crossing gives the map key another order"
        );
    }
}

// The crossing's answer for a case's top-level `entries` map.
fn ordering_of(case: &str) -> Option<MapKeyOrdering> {
    let files = case_files(case);
    let ast = model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::FastDds)
        .expect("the case lowers");
    let root = root_of(&ast, &files.type_name);
    let member = ast
        .all_members_of(root)
        .into_iter()
        .find_map(|id| ast.member(id).ok().map(|node| node.type_.resolved()))??;
    let member = zerodds_wire_probe::contract::resolve_alias(&ast, member);
    map_key_ordering(&ast, member, MapEntryOrder::AscendingKey)
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

// A key type with no value order is refused under `ascending-key`, and
// written in declaration order under the default.
//
// Three shapes: a `double`, whose IEEE 754 values have no total order
// and for which `std::map`'s `std::less` is undefined behaviour on a
// NaN; an aggregate, for which C++ has no `operator<` at all and for
// which `fastddsgen`'s `std::map` would not compile; and a `fixed`,
// which is held as text in every column here and has no `operator<`
// either.
//
// The refusal is the point. A writer that fell back to the declaration
// order for these would be a setting that reported success and changed
// no octet, which is the failure class this tree keeps paying for.
#[test]
fn the_key_types_with_no_order_are_refused() {
    const UNORDERED: [(&str, &str); 3] = [
        ("a double key", "map<double, long> entries;"),
        ("an aggregate key", "map<Cell, long> entries;"),
        ("a fixed key", "map<fixed<5,2>, long> entries;"),
    ];
    for (what, member) in UNORDERED {
        let idl = format!(
            "@final struct Cell {{ long a; }};\n@final struct Sample {{ {member} long tail; }};\n"
        );
        let ast = model_from_idl_in_dialect(&idl, "unordered.idl", Dialect::FastDds)
            .unwrap_or_else(|error| panic!("{what}: the IDL does not lower: {error}"));
        let values = serde_json::json!({ "entries": [], "tail": -1 });
        let mut request = ProbeRequest::in_dialect(
            "Sample",
            Encoding::PlainCdr1,
            Endianness::Big,
            Dialect::FastDds,
        );
        // The declaration order is what has to keep writing, and it is
        // not Fast DDS's answer any more: `Dialect::FastDds` selects
        // `AscendingKey`, because that is what Fast DDS puts on the
        // wire. The framing still has to be Fast DDS's — it is the one
        // implementation that produces a map's octets — so the order is
        // set here rather than the dialect being changed.
        request.map_entries = MapEntryOrder::Declaration;
        assert!(
            encode(&ast, &request, &values).is_ok(),
            "{what}: the declaration order must still write"
        );
        request.map_entries = MapEntryOrder::AscendingKey;
        let error = encode(&ast, &request, &values)
            .err()
            .unwrap_or_else(|| panic!("{what}: ascending-key wrote where it has no order"));
        assert!(
            format!("{error}").contains("no value order"),
            "{what}: the refusal does not say why: {error}"
        );
    }
}

// The extent is the same under both values, so the axis stays out of
// `LayoutKey::offset_key`.
//
// A permutation writes the same entries, so the count, the byte extent
// and every offset behind the map are identical. This is that claim as
// a test rather than as a sentence in `key.rs`.
#[test]
fn the_extent_is_the_same_under_both_values() {
    let mut checked = 0usize;
    for case in map_cases() {
        let files = case_files(&case);
        let Ok(ast) =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::FastDds)
        else {
            continue;
        };
        let root = root_of(&ast, &files.type_name);
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            let base = LayoutKey::in_dialect(
                Dialect::FastDds,
                encoding,
                Endianness::Little,
                Purpose::Data,
            );
            let mut deps = DepCollector::new();
            let declared = plan(&ast, root, base, &mut deps).expect("the case plans");
            let mut deps = DepCollector::new();
            let ascending = plan(
                &ast,
                root,
                base.with_map_entries(MapEntryOrder::AscendingKey),
                &mut deps,
            )
            .expect("the case plans");
            assert_eq!(
                extent_of(&declared),
                extent_of(&ascending),
                "{case} {encoding:?}: the extent moved under the entry order"
            );
            assert_eq!(
                base.offset_key(),
                base.with_map_entries(MapEntryOrder::AscendingKey)
                    .offset_key(),
                "the entry order reached the plan cache's key"
            );
            checked += 1;
        }
    }
    assert!(checked >= 40, "only {checked} plans were compared");
}

fn extent_of(layout: &zerodds_ast_layout::TypeLayout) -> (u8, Option<u32>) {
    (layout.align, layout.extent.sized())
}

// The peer's own octets decode to the case's own sample under both
// values of the axis.
//
// Cause D's "reaches no reader": a map is a mapping, so every column
// sorts its entries into the canonical order before handing them back
// and neither order can be told from the other by a reader. The 32 rows
// stay in the open count because octets differ, and this is what says
// the reader is not the reason.
#[test]
fn the_reader_returns_the_sample_under_both_values() {
    let rows = recorded();
    let mut checked = 0usize;
    for case in UNSORTED {
        let files = case_files(case);
        let ast =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::FastDds)
                .expect("the case lowers");
        let root = root_of(&ast, &files.type_name);
        let expected = text(&canonicalise(&ast, root, &files.values, "sample").expect("canonical"));
        for version in ["xcdr1", "xcdr2"] {
            for endian in ["le", "be"] {
                let key = (
                    case.to_string(),
                    PEER.to_string(),
                    version.to_string(),
                    endian.to_string(),
                );
                let (_, theirs) = rows.get(&key).expect("a recorded row");
                let bytes = unhex(theirs);
                for order in MapEntryOrder::ALL {
                    let mut request = ProbeRequest::in_dialect(
                        &files.type_name,
                        encoding_of(version),
                        endianness_of(endian),
                        Dialect::FastDds,
                    );
                    request.map_entries = order;
                    let read = decode(&ast, &request, &bytes)
                        .unwrap_or_else(|error| panic!("{case} {version} {endian}: {error}"));
                    assert_eq!(
                        text(&read),
                        expected,
                        "{case} {version} {endian} under {order:?}: the peer's octets do not \
                         read back as the sample"
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 32);
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("a hex payload")
        })
        .collect()
}

// `map-key-enum` has no recorded octets from any implementation, and
// its sample is descending.
//
// The correction to `docs/open-rows-breakdown.md`, which lists the case
// among the ones that are "already ascending and all agree" with
// `BLUE`=0 and `RED`=1. Its IDL declares `enum Colour { RED, GREEN,
// BLUE }` and its sample is `["BLUE", "RED"]`, which is `[2, 0]`; and
// `wireprobe-fastdds` refused every slot because the key is a
// `@bit_bound(16)` enumeration. So the enumeration arm of the crossing
// is written from the C++ mapping — `enum class E : int32_t` under
// `std::less` is the underlying integer — and no octets confirm it.
//
// Recorded as a test rather than as a comment, so that the arm is not
// read as measured and so that the day Fast DDS answers for the case
// this goes red and somebody looks.
#[test]
fn the_enumeration_key_has_no_recorded_octets() {
    let rows = recorded();
    let mut refusals = 0usize;
    for (key, (outcome, _)) in &rows {
        if key.0 != "map-key-enum" {
            continue;
        }
        assert_ne!(
            outcome, "wrote",
            "{} now carries octets for map-key-enum — the enumeration arm of the crossing has \
             evidence and this file's claim that it has none is stale",
            key.1
        );
        refusals += 1;
    }
    assert!(
        refusals >= 10,
        "only {refusals} map-key-enum rows were read — this check must not pass by finding \
         nothing"
    );

    // And the sample is descending, which is the other half of the
    // correction: the case would have separated the two values if the
    // peer had produced octets.
    assert_eq!(
        sample_is_sorted("map-key-enum"),
        // `BLUE` and `RED` are enumerator names, which `sample_is_sorted`
        // cannot rank — it answers `None` rather than guessing, and the
        // declared values below are what say the sample is descending.
        None,
        "map-key-enum's keys stopped being enumerator names"
    );
    let files = case_files("map-key-enum");
    assert!(
        files.idl.contains("enum Colour { RED, GREEN, BLUE }"),
        "map-key-enum no longer declares RED before BLUE"
    );
    let text = serde_json::to_string(&files.values).expect("the sample serialises");
    let blue = text.find("BLUE").expect("BLUE in the sample");
    let red = text.find("RED").expect("RED in the sample");
    assert!(
        blue < red,
        "map-key-enum's sample is no longer BLUE (2) before RED (0)"
    );
}

// Every place that walks a map's entries goes through the crossing.
//
// A site that iterates the sample straight answers [`MapEntryOrder`] by
// omission, and nothing at run time would say so: the octets would be
// the declaration order under a setting that asked for ascending keys.
// The counts are exact, so a walk added beside a crossed one fails the
// count rather than passing a search.
//
// The second number is how many times the file names the refusal a key
// type with no order gets. A writer site that asked the crossing and
// then fell back to declaration order would raise the first count and
// not the second.
#[test]
fn the_sites_go_through_the_crossing() {
    const SITES: [(&str, usize, usize); 4] = [
        // The definition, which names itself once in its signature.
        ("crates/ast-layout/src/xcdr.rs", 1, 0),
        ("crates/wire-codec/src/encode.rs", 1, 1),
        // The plan, which resolves the ordering and refuses a key type
        // with no order — the same refusal in every column.
        // Two namings: the function's contract and the error type that
        // carries it. One crossing, which is the number that matters.
        ("crates/ast-layout/src/plan.rs", 1, 2),
        // The encoder walk: zero crossings and one refusal. The plan
        // decides that a key type has no value order; the generator is
        // what says so to a caller, and the split is the point — a
        // writer that asked the crossing itself would raise the first
        // number. Python is on this walk now, like every other
        // `zerodds-lang-rules`-based column: `crates/lang-python/src/emit.rs`'s
        // separate copy is gone with the crate, and
        // `wireprobe-zerodds-python-gen` never called the crossing
        // itself either.
        ("crates/lang-rules/src/emit/encode.rs", 0, 1),
    ];
    for (path, crossings, refusals) in SITES {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        let found = text.matches("map_key_ordering(").count();
        assert_eq!(
            found, crossings,
            "{path} names the crossing {found} times, not {crossings}"
        );
        let refused = text.matches("no value order").count();
        assert_eq!(
            refused, refusals,
            "{path} names the refusal {refused} times, not {refusals} — a writer that asked the \
             crossing and then wrote the declaration order anyway would raise the first count \
             and not this one"
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

// The setting is in `Dialect::FastDds`, and all four lists drive that
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
        zerodds_ast_layout::Dialect::FastDds.map_entries(),
        MapEntryOrder::AscendingKey,
        "`Dialect::FastDds` no longer selects the setting this file measures"
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
            text.contains("fastdds"),
            "{path} does not drive `--dialect fastdds`"
        );
    }
}

// Writing what was read gives the same octets, under the rule.
//
// A round trip of this project's own bytes: what is written is read
// back and written again unchanged, which is the property that makes
// the setting usable rather than merely reproducible. It is the one
// place the ascending order has to survive the reader's canonical
// sort — the two are different orders for a signed key, and a writer
// that took the reader's would not reproduce Fast DDS.
#[test]
fn the_round_trip_is_stable_under_the_rule() {
    for case in UNSORTED {
        let files = case_files(case);
        let ast =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::FastDds)
                .expect("the case lowers");
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            for endianness in [Endianness::Little, Endianness::Big] {
                let mut request = ProbeRequest::in_dialect(
                    &files.type_name,
                    encoding,
                    endianness,
                    Dialect::FastDds,
                );
                request.map_entries = MapEntryOrder::AscendingKey;
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
    }
}
