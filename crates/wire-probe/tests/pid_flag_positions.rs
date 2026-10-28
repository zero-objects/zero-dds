// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **Counter-check.** A PL_CDR1 parameter id's flags are answered at
//! the position the id sits in, and the sentinel is one of those
//! positions.
//!
//! `WirePosition::Sentinel` was listed as measured while nothing read
//! it: the XCDR1 writer took the sentinel's flags from the *member's*
//! framing policy, so the variant's status was an assertion in a doc
//! comment rather than a thing the code could be wrong about. This is
//! the check that it is read, and it is written so that it cannot pass
//! while the position is a literal.
//!
//! # What is measured
//!
//! From `tests/interop-results/sources/vendor-corpus.tsv`, four
//! implementations over `struct-mutable`
//! (`@mutable struct { long a; double b; short c; }`) and
//! `evolution-mutable-mustunderstand`
//! (`@id(1) long a; @id(2) long b; @id(3) @must_understand long c;`),
//! encoding version 1, both byte orders. `struct-mutable` big endian:
//!
//! ```text
//! Cyclone   7f0100080000000000000004 ffed2979 …  7f020000
//! RTI       00000004 ffed2979 …                  7f020000
//! Fast DDS  00000004 ffed2979 …                  3f020000
//! OpenDDS   c0000004 ffed2979 …                  3f020000
//! ```
//!
//! This test hard-codes none of that. It sweeps every combination of
//! the three settings a `@mutable` XCDR1 sample is framed by —
//! `PlCdr1HeaderPolicy` × `PidFlagPolicy` × `MustUnderstandPolicy`,
//! eighteen of them — against each recorded row, and requires the row
//! to be reproduced and to pin the PID-flag coordinate. Then it reads
//! the two positions' answers off the combinations it *derived* and
//! requires:
//!
//! - two implementations that agree at the member and differ at the
//!   sentinel — Fast DDS and RTI. So the sentinel's answer is not a
//!   function of the member's.
//! - two that differ at the member and agree at the sentinel — Fast DDS
//!   and OpenDDS. So the member's answer is not a function of the
//!   sentinel's.
//!
//! Both directions, from vendor bytes, which is what makes the sentinel
//! a position rather than a property of the list — judgement 3 of
//! `docs/wire-axes-followup.md`.
//!
//! # Mutation-checked
//!
//! Three mutations were run, and all three turn this file red —
//! `the_four_are_reproduced` and `the_sentinel_is_not_the_members_answer`
//! both fail in each case:
//!
//! - **at the value.** `PidFlagPolicy::flags_at` answering `0` instead
//!   of `PID_MUST_UNDERSTAND` at `WirePosition::Sentinel`.
//! - **at the position**, which is the mutation
//!   `the_two_positions_still_agree` survived once: `write_sentinel`
//!   asking `flags_at(WirePosition::Member)` instead of
//!   `WirePosition::Sentinel`. Cyclone's and RTI's `7f02` becomes
//!   `3f02` and OpenDDS's `3f02` becomes `ff02`. A gate that only
//!   mutates the value cannot tell a wired position from a literal one,
//!   which is why this one is here.
//! - **at the axis's own position arm.** `ImplExtensionOnMembers`
//!   answering at `WirePosition::Sentinel` instead of
//!   `WirePosition::Member` — the two positions swapped inside the
//!   axis.
//!
//! # The third position, and why it is here
//!
//! `WirePosition::ExtendedMarker` was
//! `PlCdr1HeaderPolicy::marker_must_understand`, on the argument that
//! the marker exists only under the always-extended form and its flag is
//! therefore a property of the form. `annotation-id-sparse` contradicts
//! that in one row: all four implementations write the marker, under
//! three different framing forms.
//!
//! ```text
//! implementation  framing form       the marker
//! Cyclone         AlwaysExtended     7f01
//! RTI             Specification      7f01
//! Fast DDS        ShortTrueLength    3f01
//! OpenDDS         ShortTrueLength    ff01
//! ```
//!
//! Two forms answer alike and two forms answer differently, which is the
//! same argument the sentinel needed and the same conclusion.
//! [`the_marker_is_not_the_forms_answer`] derives both the form and the
//! flags from recorded octets and asserts both directions;
//! [`the_marker_is_written_at_its_own_position`] is the behavioural half
//! — four implementations' `annotation-hashid-implicit` rows, reproduced
//! octet for octet.
//!
//! **Mutation-checked at the position as well as at the value.** Five
//! mutations, and which test each one turns red is recorded because two
//! of them are caught by one implementation only:
//!
//! - `flags_at` answering `0` at `WirePosition::ExtendedMarker` under
//!   `SentinelMustUnderstand` — Cyclone's and RTI's `7f01` becomes
//!   `3f01`. Red.
//! - `write_pid_extended` asking `flags_at(WirePosition::Member)` — the
//!   literal-position mutation. Cyclone's and RTI's `7f01` becomes
//!   `3f01` and OpenDDS's `ff01` stays. Red.
//! - `write_pid_extended` asking `flags_at(WirePosition::Sentinel)` —
//!   the mutation that tells the marker from the sentinel. Cyclone's and
//!   RTI's markers are unchanged, because `SentinelMustUnderstand`
//!   answers the same at both; **only OpenDDS's row goes red**, `ff01`
//!   to `3f01`. It is in the list because without the fourteenth
//!   configuration nothing in this tree would catch it.
//! - the marker's arm moved to the wrong policy — `flags_at` answering
//!   `PID_MUST_UNDERSTAND` at `WirePosition::ExtendedMarker` under
//!   `Bare` instead of under `SentinelMustUnderstand`. Red.
//! - `promote_pid`, the second site, asking `WirePosition::Member`. Not
//!   red here and recorded as such: no corpus case promotes a short
//!   header to the extended form, because promotion needs a value past
//!   sixteen bits and the corpus has none.
//!   `crates/wire/tests/wire.rs::a_promoted_header_moves_the_must_understand_bit_to_flag_two`
//!   covers that site with a 64 KiB value and goes red instead, which
//!   was run rather than assumed.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{
    Dialect, Encoding, Endianness, MustUnderstandPolicy, PidFlagPolicy, PlCdr1HeaderPolicy,
    WirePosition,
};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

/// The two corpus cases, and what each one is here to separate.
///
/// `struct-mutable` has no `@must_understand` member, so it separates
/// the sentinel's flag from the members'. `evolution-mutable-
/// mustunderstand` has one declared member out of three, so it
/// separates "as declared" from "always" and from "never".
const CASES: [&str; 2] = ["struct-mutable", "evolution-mutable-mustunderstand"];

/// The four implementations that write a PL_CDR1 parameter list in the
/// corpus, under the names `vendor-corpus.tsv` files them by.
const IMPLEMENTATIONS: [&str; 4] = ["cyclone", "rti", "fastdds", "opendds"];

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

/// One point of the framing space a `@mutable` XCDR1 sample sits in.
type Combination = (PlCdr1HeaderPolicy, PidFlagPolicy, MustUnderstandPolicy);

fn combinations() -> Vec<Combination> {
    let mut out = Vec::new();
    for headers in PlCdr1HeaderPolicy::ALL {
        for flags in PidFlagPolicy::ALL {
            for understand in MustUnderstandPolicy::ALL {
                out.push((headers, flags, understand));
            }
        }
    }
    out
}

/// What this project writes for one corpus case under one combination.
fn written(case: &str, endianness: Endianness, combination: Combination) -> Option<String> {
    let directory = repository_root().join("tests/interop-corpus").join(case);
    let idl = std::fs::read_to_string(directory.join("type.idl")).ok()?;
    let type_name = std::fs::read_to_string(directory.join("type")).ok()?;
    let values: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("values.json")).ok()?).ok()?;
    let ast = model_from_idl(&idl, &format!("{case}/type.idl")).ok()?;
    let mut request = ProbeRequest::new(type_name.trim(), Encoding::PlCdr1, endianness);
    request.pl_cdr1_headers = combination.0;
    request.pid_flags = combination.1;
    request.must_understand = combination.2;
    encode(&ast, &request, &values).ok().map(|b| hex(&b))
}

/// Which combinations reproduce a recorded row exactly.
fn matching(case: &str, endian: &str, theirs: &str) -> Vec<Combination> {
    let endianness = if endian == "be" {
        Endianness::Big
    } else {
        Endianness::Little
    };
    combinations()
        .into_iter()
        .filter(|combination| written(case, endianness, *combination).as_deref() == Some(theirs))
        .collect()
}

/// Every one of the four is reproduced, and by exactly one point of the
/// framing space.
///
/// The reproduction is what makes the rest of this file a measurement:
/// the flags each implementation writes at each position are read off
/// the combination that puts out its octets, not off a table here.
#[test]
fn the_four_are_reproduced() {
    let rows = recorded();
    assert!(
        !rows.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check must not \
         pass by finding nothing"
    );
    let mut compared = 0usize;
    for case in CASES {
        for system in IMPLEMENTATIONS {
            for endian in ["le", "be"] {
                let key = (
                    case.to_string(),
                    system.to_string(),
                    "xcdr1".to_string(),
                    endian.to_string(),
                );
                let Some(theirs) = rows.get(&key) else {
                    panic!("{case} {system} xcdr1 {endian} is not in vendor-corpus.tsv");
                };
                let found = matching(case, endian, theirs);
                assert!(
                    !found.is_empty(),
                    "{case} {system} xcdr1 {endian}: none of the {} framing combinations \
                     reproduce {theirs}",
                    combinations().len()
                );
                // The coordinate under test has to be pinned by the row.
                // The other two need not be: `struct-mutable` declares
                // no `@must_understand` member, so `Declared` and
                // `Omitted` write the same octets for it — a measured
                // degeneracy of the case, not a slack in the axis, and
                // `evolution-mutable-mustunderstand` is in this list to
                // remove it.
                let flags: Vec<PidFlagPolicy> = found.iter().map(|(_, flags, _)| *flags).collect();
                assert!(
                    flags.windows(2).all(|pair| pair[0] == pair[1]),
                    "{case} {system} xcdr1 {endian}: {flags:?} all reproduce {theirs}, so the \
                     row does not decide the PID-flag axis"
                );
                compared += 1;
            }
        }
    }
    // Four implementations, two cases, two byte orders. A run that
    // compared fewer has stopped measuring something.
    assert_eq!(compared, 16, "{compared} rows compared, not 16");
}

/// The PID-flag policy that reproduces an implementation, derived from
/// its recorded rows rather than named here.
///
/// The two cases are intersected because neither pins all three
/// coordinates on its own: `struct-mutable` has a `short` member, so it
/// separates the header forms by their declared length, and no
/// `@must_understand` member, so it cannot separate the must-understand
/// policies; `evolution-mutable-mustunderstand` is the other way round.
///
/// **A measured degeneracy, recorded so it is not read as slack.**
/// OpenDDS's flag is unconditional, so under encoding version 1 its
/// `MustUnderstandPolicy` cannot be measured at all: `Declared` and
/// `Omitted` put out the same octets behind it. Its XCDR2 EMHEADERs do
/// answer — `a0000003` for the declared member and `20000001` for the
/// others — but that is a different encoding and not this axis. So the
/// intersection is asserted unique on the PID-flag coordinate, which is
/// the one under test, and not on all three.
fn derived(
    rows: &BTreeMap<(String, String, String, String), String>,
    system: &str,
) -> PidFlagPolicy {
    let mut found: Option<Vec<Combination>> = None;
    for case in CASES {
        for endian in ["le", "be"] {
            let key = (
                case.to_string(),
                system.to_string(),
                "xcdr1".to_string(),
                endian.to_string(),
            );
            let theirs = rows.get(&key).expect("a recorded row");
            let here = matching(case, endian, theirs);
            found = Some(match found {
                None => here,
                Some(before) => before
                    .into_iter()
                    .filter(|combination| here.contains(combination))
                    .collect(),
            });
        }
    }
    let found = found.expect("at least one case");
    assert!(
        !found.is_empty(),
        "{system}: no one framing reproduces both cases in both orders"
    );
    let flags: Vec<PidFlagPolicy> = found.iter().map(|(_, flags, _)| *flags).collect();
    assert!(
        flags.windows(2).all(|pair| pair[0] == pair[1]),
        "{system}: {flags:?} all reproduce every recorded row, so the rows do not decide the \
         PID-flag axis"
    );
    flags[0]
}

/// Neither position's answer can be read off the other's.
///
/// Two implementations agree at the member and differ at the sentinel;
/// two differ at the member and agree at the sentinel. Both pairs are
/// derived from recorded octets by [`derived`], so this fails if the
/// vendors' bytes stop saying it as well as if the axis stops carrying
/// it.
#[test]
fn the_sentinel_is_not_the_members_answer() {
    let rows = recorded();
    let rti = derived(&rows, "rti");
    let fastdds = derived(&rows, "fastdds");
    let opendds = derived(&rows, "opendds");

    // RTI and Fast DDS: the same bare member id, `7f02` against `3f02`.
    assert_eq!(
        rti.must_understand_at(WirePosition::Member),
        fastdds.must_understand_at(WirePosition::Member),
        "RTI and Fast DDS were measured writing the same member id"
    );
    assert_eq!(
        rti.impl_extension_at(WirePosition::Member),
        fastdds.impl_extension_at(WirePosition::Member)
    );
    assert_ne!(
        rti.must_understand_at(WirePosition::Sentinel),
        fastdds.must_understand_at(WirePosition::Sentinel),
        "RTI writes 7f02 and Fast DDS 3f02: the sentinel is not the member's answer"
    );

    // Fast DDS and OpenDDS: the same bare sentinel, `0001` against
    // `c001`.
    assert_eq!(
        fastdds.must_understand_at(WirePosition::Sentinel),
        opendds.must_understand_at(WirePosition::Sentinel),
        "Fast DDS and OpenDDS were measured writing the same sentinel"
    );
    assert_ne!(
        fastdds.impl_extension_at(WirePosition::Member),
        opendds.impl_extension_at(WirePosition::Member),
        "OpenDDS sets FLAG_IMPL_EXTENSION on every member id and Fast DDS on none: the \
         member is not the sentinel's answer either"
    );
}

/// The sentinel's flags do not depend on what any member declared.
///
/// `struct-mutable` declares no `@must_understand` member and
/// `evolution-mutable-mustunderstand` declares one of three. Every
/// implementation writes the same sentinel in both, which is why
/// `PID_LIST_END` takes no member argument.
#[test]
fn the_sentinel_does_not_move_with_a_declaration() {
    let rows = recorded();
    for system in IMPLEMENTATIONS {
        let mut sentinels = Vec::new();
        for case in CASES {
            let key = (
                case.to_string(),
                system.to_string(),
                "xcdr1".to_string(),
                "be".to_string(),
            );
            let theirs = rows.get(&key).expect("a recorded row");
            // The last four octets of a PL_CDR1 sample are the sentinel
            // and its zero length.
            sentinels.push(theirs[theirs.len() - 8..].to_string());
        }
        assert_eq!(
            sentinels[0], sentinels[1],
            "{system} writes {} where no member declares must-understand and {} where one \
             does — the sentinel would then be the member's answer after all",
            sentinels[0], sentinels[1]
        );
    }
}

// -- the extended marker ------------------------------------------------

/// The corpus case every framing form writes a `PID_EXTENDED` marker
/// for, and the only one that does.
///
/// `@hashid long alpha; @hashid long beta; long plain;` — the two hashed
/// ids are `0x0343172c` and `0x00ca7b98`, both past `PID_SHORT_MAX_ID`,
/// so `needs_extended_pid` escalates them under **every** form and not
/// only under `AlwaysExtended`. That is the whole of the correction:
/// the marker is not a property of the form.
const MARKER_CASE: &str = "annotation-hashid-implicit";

/// Which named configuration reproduces each implementation on
/// [`MARKER_CASE`].
///
/// `impl-extension-pids-rti` is the fourteenth configuration and is here
/// for OpenDDS, whose row is RTI's octets to the octet apart from three
/// flag positions — the marker twice, the member id once and the
/// sentinel once.
const MARKER_COLUMNS: [(&str, Dialect, Option<PidFlagPolicy>); 4] = [
    ("cyclone", Dialect::Cyclone, None),
    ("rti", Dialect::Rti, None),
    ("fastdds", Dialect::FastDds, None),
    (
        "opendds",
        Dialect::Rti,
        Some(PidFlagPolicy::ImplExtensionOnMembers),
    ),
];

/// What this project writes for a case in a dialect, with the PID-flag
/// axis optionally overridden.
fn written_in_dialect(
    case: &str,
    endianness: Endianness,
    dialect: Dialect,
    flags: Option<PidFlagPolicy>,
) -> Option<String> {
    let directory = repository_root().join("tests/interop-corpus").join(case);
    let idl = std::fs::read_to_string(directory.join("type.idl")).ok()?;
    let type_name = std::fs::read_to_string(directory.join("type")).ok()?;
    let values: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("values.json")).ok()?).ok()?;
    // The dialect reaches the *lowering* as well as the writer: whether
    // a `@hashid` member advances the sequential counter is a member-id
    // question and is decided before any octet is written.
    let ast = model_from_idl_in_dialect(&idl, &format!("{case}/type.idl"), dialect).ok()?;
    let mut request =
        ProbeRequest::in_dialect(type_name.trim(), Encoding::PlCdr1, endianness, dialect);
    if let Some(policy) = flags {
        request.pid_flags = policy;
    }
    encode(&ast, &request, &values).ok().map(|b| hex(&b))
}

/// The marker word a recorded row opens with, as a `u16`.
///
/// Every implementation puts [`MARKER_CASE`]'s first member behind an
/// extended header, so the marker is the first two octets. The function
/// asserts that rather than assuming it: a row whose low fourteen bits
/// are not `PID_EXTENDED` is a row this test is reading wrong, and it
/// says so instead of comparing flags off a payload octet.
fn marker_word(octets: &str, endian: &str, what: &str) -> u16 {
    let byte = |at: usize| {
        u8::from_str_radix(&octets[at * 2..at * 2 + 2], 16).expect("hex in the recorded row")
    };
    let word = if endian == "be" {
        (u16::from(byte(0)) << 8) | u16::from(byte(1))
    } else {
        (u16::from(byte(1)) << 8) | u16::from(byte(0))
    };
    assert_eq!(
        word & 0x3fff,
        0x3f01,
        "{what}: {octets} does not open with a PID_EXTENDED marker"
    );
    word
}

/// The framing forms that reproduce an implementation, derived from its
/// recorded rows exactly as [`derived`] derives the PID-flag policy.
///
/// A set and not one form: on these two cases every member is a
/// primitive, and RTI's form — the short header until the sample's type
/// can exceed what one states, then the extended one on every constructed
/// member — writes what `Specification` and `ExtendedForConstructed`
/// write. Two implementations frame alike where their sets share a form.
fn derived_forms(
    rows: &BTreeMap<(String, String, String, String), String>,
    system: &str,
) -> std::collections::BTreeSet<PlCdr1HeaderPolicy> {
    let mut found: Option<Vec<Combination>> = None;
    for case in CASES {
        for endian in ["le", "be"] {
            let key = (
                case.to_string(),
                system.to_string(),
                "xcdr1".to_string(),
                endian.to_string(),
            );
            let theirs = rows.get(&key).expect("a recorded row");
            let here = matching(case, endian, theirs);
            found = Some(match found {
                None => here,
                Some(before) => before
                    .into_iter()
                    .filter(|combination| here.contains(combination))
                    .collect(),
            });
        }
    }
    let found = found.expect("at least one case");
    let forms: std::collections::BTreeSet<PlCdr1HeaderPolicy> =
        found.iter().map(|(form, _, _)| *form).collect();
    assert!(
        !forms.is_empty(),
        "{system}: no framing form reproduces both cases in both orders"
    );
    forms
}

/// The marker's flags are not the framing form's, in both directions.
///
/// Both coordinates are **derived** — the form and the PID-flag policy
/// from `struct-mutable` and `evolution-mutable-mustunderstand`, the
/// marker's own octets from [`MARKER_CASE`] — so this fails if the
/// vendors' bytes stop saying it as well as if the axis stops carrying
/// it.
#[test]
fn the_marker_is_not_the_forms_answer() {
    let rows = recorded();
    let mut markers: BTreeMap<&str, u16> = BTreeMap::new();
    let mut forms: BTreeMap<&str, std::collections::BTreeSet<PlCdr1HeaderPolicy>> = BTreeMap::new();
    let mut policies: BTreeMap<&str, PidFlagPolicy> = BTreeMap::new();
    for system in IMPLEMENTATIONS {
        forms.insert(system, derived_forms(&rows, system));
        policies.insert(system, derived(&rows, system));
        let mut seen: Option<u16> = None;
        for endian in ["le", "be"] {
            let key = (
                MARKER_CASE.to_string(),
                system.to_string(),
                "xcdr1".to_string(),
                endian.to_string(),
            );
            let theirs = rows
                .get(&key)
                .unwrap_or_else(|| panic!("{MARKER_CASE} {system} xcdr1 {endian} is not recorded"));
            let word = marker_word(theirs, endian, &format!("{system} {endian}"));
            // The same marker in both byte orders, which is what makes
            // it a flag rather than an artefact of one order's octets.
            if let Some(before) = seen {
                assert_eq!(before, word, "{system} writes two different markers");
            }
            seen = Some(word);
        }
        markers.insert(system, seen.expect("both orders"));
    }

    // Direction one: two different forms, the same marker.
    assert!(
        forms["cyclone"].is_disjoint(&forms["rti"]),
        "Cyclone and RTI were measured writing different framing forms: {:?} and {:?}",
        forms["cyclone"],
        forms["rti"]
    );
    assert_eq!(
        markers["cyclone"], markers["rti"],
        "Cyclone writes {:04x} and RTI {:04x}: the argument for the marker being a property \
         of the form rested on these differing",
        markers["cyclone"], markers["rti"]
    );

    // Direction two: the same form, two different markers.
    assert!(
        !forms["fastdds"].is_disjoint(&forms["opendds"]),
        "Fast DDS and OpenDDS were measured writing the same framing form: {:?} and {:?}",
        forms["fastdds"],
        forms["opendds"]
    );
    assert_ne!(
        markers["fastdds"], markers["opendds"],
        "Fast DDS and OpenDDS write the same form and the same marker, which would make the \
         marker a property of the form after all"
    );

    // And the axis answers each of the four, at the marker's own
    // position. Read off the policy `derived` gave, not off a table
    // here: this is what makes the marker `PidFlagPolicy`'s.
    for system in IMPLEMENTATIONS {
        let policy = policies[system];
        let mut expected = 0x3f01_u16;
        if policy.must_understand_at(WirePosition::ExtendedMarker) {
            expected |= 0x4000;
        }
        if policy.impl_extension_at(WirePosition::ExtendedMarker) {
            expected |= 0x8000;
        }
        assert_eq!(
            markers[system], expected,
            "{system} writes {:04x} and its PID-flag policy answers {expected:04x} at the \
             marker",
            markers[system]
        );
    }
}

/// The marker is written where the axis says, in whole rows.
///
/// The behavioural half. Four implementations' [`MARKER_CASE`] rows,
/// both byte orders, reproduced octet for octet by a named
/// configuration — so a writer that took the marker from the framing
/// form, or asked at the wrong position, fails here rather than in a
/// comparison of two flag words.
#[test]
fn the_marker_is_written_at_its_own_position() {
    let rows = recorded();
    let mut compared = 0usize;
    for (system, dialect, flags) in MARKER_COLUMNS {
        for (endian, endianness) in [("be", Endianness::Big), ("le", Endianness::Little)] {
            let key = (
                MARKER_CASE.to_string(),
                system.to_string(),
                "xcdr1".to_string(),
                endian.to_string(),
            );
            let theirs = rows
                .get(&key)
                .unwrap_or_else(|| panic!("{MARKER_CASE} {system} xcdr1 {endian} is not recorded"));
            let ours = written_in_dialect(MARKER_CASE, endianness, dialect, flags)
                .unwrap_or_else(|| panic!("{MARKER_CASE} in {dialect:?} produced nothing"));
            assert_eq!(
                &ours,
                theirs,
                "{MARKER_CASE} {system} xcdr1 {endian}: {dialect:?}{} does not reproduce the \
                 recorded row",
                match flags {
                    Some(policy) => format!(" with --pid-flags {}", policy.as_str()),
                    None => String::new(),
                }
            );
            compared += 1;
        }
    }
    // Four implementations, both orders. A run that compared fewer has
    // stopped measuring something.
    assert_eq!(compared, 8, "{compared} rows compared, not 8");
}
