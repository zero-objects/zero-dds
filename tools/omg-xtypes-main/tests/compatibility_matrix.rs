// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the `dds-xtypes` suite actually measures, measured.
//!
//! The suite pairs a publisher of one of its 25 types with a subscriber
//! of another and asks whether the two match — 625 combinations, whose
//! expected answer it carries in `compatibility_matrix.xlsx`. That
//! spreadsheet is the suite's only statement of intent; its 625 cells
//! hold no text, the answer is the cell's fill colour, and
//! [`SUITE_EXPECTED`] below is that colouring transcribed (green =
//! compatible, red = not), 83 of 625.
//!
//! Running all 625 pairs as processes is not what this file does. It
//! measures the decision those processes would make, at the place our
//! runtime makes it: `zerodds_types::type_matcher::TypeMatcher`, over
//! the `TypeObject`s the generated types announce, with the registry
//! populated the way `DcpsRuntime::register_type_object` populates it.
//! That is one function call per pair instead of two processes, and it
//! is the same function the live SEDP match path calls
//! (`crates/dcps/src/runtime.rs`, `matcher.match_types(...)`).
//!
//! [`MEASURED`] is what that returns today, 87 of 625, agreeing with
//! the spreadsheet in 621 of the 625 cells. The four cells they still
//! disagree about are pinned by name below with the clause they belong
//! to, and all four are ours-yes/suite-no. Neither matrix is edited to
//! make the other true: a change in either direction has to come here
//! and say which cell moved and why.
//!
//! It was 59 of 625 and 32 disagreements until `check_minimal_types`
//! was made to call `flatten_inheritance` (XTypes 1.3 §7.2.2.4.5)
//! before comparing two structs. That one wiring moved 32 cells: the 28
//! the suite called compatible and we did not, the two `Shape5Mutable`
//! /`Shape5MutableExplicitID` cells that were compatible only because
//! their ids were unflattened, and the two `Shape2Final`/`Shape5Final`
//! cells named below.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
// The loops below walk one 25 x 25 grid across three arrays at once —
// the two matrices and the name list — and the index is what names the
// cell in every failure message. Zipping three iterators would hide the
// coordinate the assertions report.
#![allow(clippy::needless_range_loop)]

use zerodds_omg_xtypes_main::shape_type::{VARIANT_NAMES, variant_type_objects};
use zerodds_types::TypeIdentifier;
use zerodds_types::assignability::{AssignabilityConfig, flatten_inheritance};
use zerodds_types::qos::TypeConsistencyEnforcement;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_matcher::TypeMatcher;

/// `compatibility_matrix.xlsx`, transcribed.
///
/// Row = the published type, column = the subscribed type, both in the
/// order of `VARIANT_NAMES` (version outermost, extensibility kind
/// innermost — `Shape1Default, Shape1Final, Shape1Extensible,
/// Shape1Mutable, Shape1MutableExplicitID, Shape2Default, …`). The
/// spreadsheet fills only the upper triangle and is therefore symmetric
/// by construction; the transcription mirrors it, so both directions of
/// a pair carry the spreadsheet's one answer.
const SUITE_EXPECTED: [&[u8; 25]; 25] = [
    b"1010010100000000000010100",
    b"0100000000000000000000000",
    b"1010010100000000000010100",
    b"0001000000000000000000010",
    b"0000100001000010000100001",
    b"1010010100000000000010100",
    b"0000001000000000000000000",
    b"1010010100000000000010100",
    b"0000000010000000000000010",
    b"0000100001000010000100001",
    b"0000000000101000000000000",
    b"0000000000010000000000000",
    b"0000000000101000000000000",
    b"0000000000000100000000000",
    b"0000100001000010000100001",
    b"0000000000000001010000000",
    b"0000000000000000100000000",
    b"0000000000000001010000000",
    b"0000000000000000001000000",
    b"0000100001000010000100001",
    b"1010010100000000000010100",
    b"0000000000000000000001000",
    b"1010010100000000000010100",
    b"0001000010000000000000010",
    b"0000100001000010000100001",
];

/// What `TypeMatcher::match_types` answers today, same layout.
///
/// Recorded rather than derived so that a change in the assignability
/// implementation shows up here as a named cell instead of quietly
/// moving the agreement count.
const MEASURED: [&[u8; 25]; 25] = [
    b"1010010100000000000010100",
    b"0100000000000000000000000",
    b"1010010100000000000010100",
    b"0001000010000000000000010",
    b"0000100001000010000100001",
    b"1010010100000000000010100",
    b"0000001000000000000001000",
    b"1010010100000000000010100",
    b"0001000010000000000000010",
    b"0000100001000010000100001",
    b"0000000000101000000000000",
    b"0000000000010000000000000",
    b"0000000000101000000000000",
    b"0000000000000100000000000",
    b"0000100001000010000100001",
    b"0000000000000001010000000",
    b"0000000000000000100000000",
    b"0000000000000001010000000",
    b"0000000000000000001000000",
    b"0000100001000010000100001",
    b"1010010100000000000010100",
    b"0000001000000000000001000",
    b"1010010100000000000010100",
    b"0001000010000000000000010",
    b"0000100001000010000100001",
];

/// The registry as `DcpsRuntime::register_type_object` builds it, and
/// each type's minimal `TypeIdentifier`.
///
/// The complete objects go in first because the minimal projection of
/// one of them needs the complete objects of its dependencies — which,
/// for the five `Shape5…` types, is their base type.
fn registry_and_ids() -> (TypeRegistry, Vec<(&'static str, TypeIdentifier)>) {
    let mut registry = TypeRegistry::new();
    let mut complete_hashes = Vec::new();
    for (name, obj) in VARIANT_NAMES.iter().zip(variant_type_objects()) {
        let obj = obj.unwrap_or_else(|| panic!("{name} announces no TypeObject"));
        let hash = zerodds_types::compute_hash(&obj).expect("complete hash");
        match obj {
            zerodds_types::TypeObject::Complete(c) => registry.insert_complete(hash, c),
            other => panic!("{name} announced {other:?}, not a CompleteTypeObject"),
        }
        complete_hashes.push((*name, hash));
    }
    let mut ids = Vec::new();
    for (name, hash) in &complete_hashes {
        let complete = registry.get_complete(hash).expect("complete").clone();
        let minimal = complete
            .to_minimal(&registry)
            .unwrap_or_else(|e| panic!("{name}: minimal projection failed: {e:?}"));
        let minimal_hash =
            zerodds_types::hash::compute_minimal_hash(&minimal).expect("minimal hash");
        registry.insert_minimal(minimal_hash, minimal);
        ids.push((*name, TypeIdentifier::EquivalenceHashMinimal(minimal_hash)));
    }
    (registry, ids)
}

/// The 625 answers, in the layout of the two matrices above.
fn measure() -> Vec<Vec<bool>> {
    let (registry, ids) = registry_and_ids();
    // The suite creates its endpoints with `DATAWRITER_QOS_DEFAULT` /
    // `DATAREADER_QOS_DEFAULT`, so the enforcement is the default one:
    // ALLOW_TYPE_COERCION, ignore_sequence_bounds and
    // ignore_string_bounds true, ignore_member_names false,
    // prevent_type_widening and force_type_validation false
    // (XTypes 1.3 §7.6.3.4).
    let tce = TypeConsistencyEnforcement::default();
    let matcher = TypeMatcher::new(&tce);
    ids.iter()
        .map(|(_, w)| {
            ids.iter()
                .map(|(_, r)| matcher.match_types(w, r, &registry).is_match())
                .collect()
        })
        .collect()
}

/// The two matrices are 25 x 25 and say what this file says they say.
#[test]
fn the_two_matrices_are_well_formed() {
    assert_eq!(VARIANT_NAMES.len(), 25);
    let suite_ok: usize = SUITE_EXPECTED
        .iter()
        .map(|row| row.iter().filter(|c| **c == b'1').count())
        .sum();
    assert_eq!(suite_ok, 83, "the spreadsheet marks 83 of 625 compatible");
    let measured_ok: usize = MEASURED
        .iter()
        .map(|row| row.iter().filter(|c| **c == b'1').count())
        .sum();
    assert_eq!(measured_ok, 87, "we answer 87 of 625 compatible");

    // The agreement count itself, because it is the number the finding
    // is reported with and a number nobody asserts is a number that
    // drifts.
    let agree: usize = (0..25)
        .map(|w| {
            (0..25)
                .filter(|r| SUITE_EXPECTED[w][*r] == MEASURED[w][*r])
                .count()
        })
        .sum();
    assert_eq!(agree, 621, "621 of the 625 cells carry the same answer");

    // The spreadsheet fills only the upper triangle, so its statement is
    // symmetric. Ours is not required to be — assignability is a
    // directed relation (XTypes 1.3 §7.2.4.1 is "T1 is-assignable-from
    // T2") — but the suite's expectation is, and a transcription that
    // lost that would compare against a matrix nobody wrote.
    for w in 0..25 {
        for r in 0..25 {
            assert_eq!(
                SUITE_EXPECTED[w][r], SUITE_EXPECTED[r][w],
                "the transcribed expectation is not symmetric at ({w},{r})"
            );
        }
    }
}

/// Our assignability implementation answers what [`MEASURED`] records.
///
/// This is the whole measurement. It fails when the answer for any of
/// the 625 pairs changes — in either direction, including toward the
/// suite's expectation. That is deliberate: a cell that moves has to be
/// named here, so that a fix is recorded as a fix and a regression
/// cannot pass as one.
#[test]
fn the_measured_matrix_is_what_the_implementation_answers() {
    let measured = measure();
    let mut drift = Vec::new();
    for (w, row) in measured.iter().enumerate() {
        for (r, got) in row.iter().enumerate() {
            let recorded = MEASURED[w][r] == b'1';
            if *got != recorded {
                drift.push(format!(
                    "{} -> {}: recorded {recorded}, measured {got}",
                    VARIANT_NAMES[w], VARIANT_NAMES[r]
                ));
            }
        }
    }
    assert!(
        drift.is_empty(),
        "the assignability answer moved in {} of 625 pairs:\n{}",
        drift.len(),
        drift.join("\n")
    );
}

/// Where we and the suite disagree, and in which direction.
///
/// Four cells, all of them ours-yes/suite-no; not one cell is left that
/// the suite calls compatible and we do not. They are named in full
/// rather than counted, because a count that stays the same while its
/// members change says nothing.
#[test]
fn the_divergence_from_the_suite_is_the_documented_one() {
    let mut suite_yes_ours_no = Vec::new();
    let mut ours_yes_suite_no = Vec::new();
    for w in 0..25 {
        for r in 0..25 {
            let suite = SUITE_EXPECTED[w][r] == b'1';
            let ours = MEASURED[w][r] == b'1';
            let pair = format!("{} -> {}", VARIANT_NAMES[w], VARIANT_NAMES[r]);
            match (suite, ours) {
                (true, false) => suite_yes_ours_no.push(pair),
                (false, true) => ours_yes_suite_no.push(pair),
                _ => {}
            }
        }
    }

    // This list was 28 pairs, every one of them with a `Shape5…` type on
    // one side, and that was the fingerprint of one cause rather than of
    // 28: `Shape5…` is the only family declared by inheritance
    // (`struct Shape5Default : Shape1Default`), and `check_minimal_types`
    // compared a struct's own `member_seq` without ever forming the
    // hypothetical flat type of XTypes 1.3 §7.2.2.4.5. It calls
    // `flatten_inheritance` now and the list is empty — see
    // `the_inherited_types_are_compared_with_their_base_members` below.
    assert!(
        suite_yes_ours_no.is_empty(),
        "the suite calls these compatible and we do not: {suite_yes_ours_no:#?}"
    );

    // The other direction, named in full.
    assert_eq!(
        ours_yes_suite_no,
        [
            // XTypes 1.3 §7.2.4.4.4.4 for MUTABLE matches members by id
            // and lets a member that only one side declares pass, unless
            // it is must-understand. `Shape2Mutable` is `Shape1Mutable`
            // plus `angle` at the next automatic id, and neither side
            // marks anything must-understand, so the clause makes the
            // pair assignable — which is what we answer. The
            // spreadsheet marks it red, and marks
            // `Shape1Mutable -> Shape5Mutable` green in the very next
            // column although `Shape5Mutable` has, after §7.2.2.4.5
            // flattening, exactly `Shape2Mutable`'s five members with
            // exactly its five ids. The two cells cannot both be right.
            "Shape1Mutable -> Shape2Mutable",
            // Flattened, `Shape5Final` IS `Shape2Final`: the IDL declares
            // `@final struct Shape5Final : Shape1Final { float angle; }`
            // and `@final struct Shape2Final` as `Shape1Final`'s four
            // members plus the same `float angle`, so §7.2.2.4.5 gives
            // the two the same five members with the same five ids and
            // the same five names. §7.2.4.4.4.4 for FINAL asks whether
            // the two member sequences are the same one, and here they
            // are; FINAL carries no DHEADER either, so the two are the
            // same octets on the wire. The spreadsheet's Final block is
            // "compatible with itself and nothing else" throughout, which
            // is right for the other 23 Final cells and cannot be right
            // for this one.
            "Shape2Final -> Shape5Final",
            "Shape2Mutable -> Shape1Mutable",
            "Shape5Final -> Shape2Final",
        ],
        "the divergence in our favour changed"
    );
}

/// An inherited type is compared with the members it inherits.
///
/// `zerodds_types::assignability::flatten_inheritance` builds the
/// hypothetical flat type of XTypes 1.3 §7.2.2.4.5 — base members
/// first, then the derived ones — and `check_minimal_types` calls it
/// before dispatching to the three per-extensibility checks. It did not
/// used to: the checks read `w.member_seq` and `r.member_seq` directly,
/// so `Shape5Default`, which the IDL declares as `struct Shape5Default :
/// Shape1Default { float angle; }`, entered §7.2.4.4.4.4 as a
/// one-member `@appendable` struct whose only member sat at id 4 — and
/// against `Shape1Default`, whose first member sits at id 0, the shared
/// prefix disagreed at position 0. That was the whole difference
/// between the suite's 83 and our 59, and it was not confined to this
/// suite: any inherited topic type was matched on its own members
/// alone.
///
/// This test keeps both halves — that the registered object really is
/// the unflattened one, and that the flat form is what the match path
/// answers on — so a regression that stopped flattening would fail here
/// with the cause named and not only as 30 moved cells.
#[test]
fn the_inherited_types_are_compared_with_their_base_members() {
    let (registry, ids) = registry_and_ids();
    let index = |name: &str| VARIANT_NAMES.iter().position(|n| *n == name).expect(name);

    let TypeIdentifier::EquivalenceHashMinimal(h5) = ids[index("Shape5Default")].1 else {
        panic!("expected a minimal hash");
    };
    let Some(zerodds_types::MinimalTypeObject::Struct(s5)) = registry.get_minimal(&h5) else {
        panic!("Shape5Default is not a struct in the registry");
    };

    // As registered: one own member, and a base type that is present.
    // The flattening happens in the comparison, not in the registry —
    // the announced TypeObject stays the one the peer sent.
    assert_eq!(
        s5.member_seq.len(),
        1,
        "Shape5Default carries only `angle` in its own member list"
    );
    assert_ne!(
        s5.header.base_type,
        TypeIdentifier::None,
        "Shape5Default lost its base type in the TypeObject"
    );

    let flat = flatten_inheritance(s5, &registry, AssignabilityConfig::default().max_depth)
        .expect("the base of Shape5Default resolves in the registry");
    assert_eq!(
        flat.member_seq.len(),
        5,
        "§7.2.2.4.5 gives Shape5Default five members: Shape1Default's four, then angle"
    );

    // And the assignability path uses it. `Shape2Default` has the same
    // five members as flattened `Shape5Default`, and the suite marks the
    // pair compatible.
    let tce = TypeConsistencyEnforcement::default();
    let matcher = TypeMatcher::new(&tce);
    let s2 = &ids[index("Shape2Default")].1;
    let s5id = &ids[index("Shape5Default")].1;
    assert!(
        matcher.match_types(s5id, s2, &registry).is_match(),
        "an inherited type must be compared as its §7.2.2.4.5 flat type"
    );

    // The other direction of the same wiring: `Shape5Mutable` and
    // `Shape5MutableExplicitID` differ in every member id ONCE their
    // bases are folded in — before that they were two one-member
    // mutable structs with disjoint ids, which §7.2.4.4.4.4 calls
    // assignable. Flattening turns that accidental yes into a no, which
    // is the suite's answer.
    let m = &ids[index("Shape5Mutable")].1;
    let mx = &ids[index("Shape5MutableExplicitID")].1;
    assert!(
        !matcher.match_types(m, mx, &registry).is_match(),
        "flattened, the two mutable Shape5 types share no member id"
    );
}

/// The `@final` block, where we and the suite differ in exactly two
/// cells and agree in the other 23.
///
/// XTypes 1.3 §7.2.4.4.4.4 for FINAL: the two member sequences are the
/// same one. It is the strictest of the three rules and the one with no
/// room for interpretation, so agreement here is the floor — a
/// divergence beyond the named pair would mean the extensibility kinds
/// themselves did not survive generation.
///
/// The named pair is `Shape2Final` / `Shape5Final`, which §7.2.2.4.5
/// flattening makes the same five members; see
/// `the_divergence_from_the_suite_is_the_documented_one`.
#[test]
fn the_final_block_agrees_with_the_suite_apart_from_the_flattened_pair() {
    let named = ["Shape2Final", "Shape5Final"];
    for w in 0..25 {
        for r in 0..25 {
            if !VARIANT_NAMES[w].ends_with("Final") && !VARIANT_NAMES[r].ends_with("Final") {
                continue;
            }
            if named.contains(&VARIANT_NAMES[w]) && named.contains(&VARIANT_NAMES[r]) && w != r {
                assert_ne!(
                    SUITE_EXPECTED[w][r], MEASURED[w][r],
                    "{} -> {} is the named divergence and stopped being one",
                    VARIANT_NAMES[w], VARIANT_NAMES[r]
                );
                continue;
            }
            assert_eq!(
                SUITE_EXPECTED[w][r], MEASURED[w][r],
                "{} -> {}",
                VARIANT_NAMES[w], VARIANT_NAMES[r]
            );
        }
    }
}

/// A type is assignable from itself, all 25 times.
///
/// XTypes 1.3 §7.2.4.1 identity. This is the one cell of every row that
/// a suite run has to produce samples for; a diagonal hole would mean a
/// type that cannot talk to a second process running the same type,
/// which is the first thing a reviewer tries.
#[test]
fn every_type_is_assignable_from_itself() {
    let measured = measure();
    for (i, name) in VARIANT_NAMES.iter().enumerate() {
        assert!(measured[i][i], "{name} is not assignable from itself");
        assert_eq!(
            SUITE_EXPECTED[i][i], b'1',
            "{name}: the suite expects it too"
        );
    }
}

/// Extensibility kinds do not mix, and both matrices say so.
///
/// XTypes 1.3 §7.2.4.4.4.4 opens with the extensibility check: a
/// `@final` writer and a `@mutable` reader are not assignable whatever
/// their members are, because the three kinds are three different wire
/// framings (Tab. 59). `…Default` and `…Extensible` are the same kind
/// here — APPENDABLE — which is why the Default/Extensible blocks agree
/// with each other throughout both matrices.
#[test]
fn no_pair_across_two_extensibility_kinds_is_compatible() {
    let kind = |name: &str| -> &'static str {
        if name.ends_with("Final") {
            "final"
        } else if name.ends_with("Mutable") || name.ends_with("MutableExplicitID") {
            "mutable"
        } else {
            "appendable"
        }
    };
    for w in 0..25 {
        for r in 0..25 {
            if kind(VARIANT_NAMES[w]) == kind(VARIANT_NAMES[r]) {
                continue;
            }
            assert_eq!(
                SUITE_EXPECTED[w][r], b'0',
                "the suite expects {} -> {} to match across kinds",
                VARIANT_NAMES[w], VARIANT_NAMES[r]
            );
            assert_eq!(
                MEASURED[w][r], b'0',
                "we match {} -> {} across kinds",
                VARIANT_NAMES[w], VARIANT_NAMES[r]
            );
        }
    }
}
