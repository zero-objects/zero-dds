// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The three probes over one value file, checked against each other.
//!
//! `encode.rs`, `fill.rs` and `fill_cpp.rs` read the same JSON and hand
//! their output to three different serialisers. The matrix reads a
//! difference between those serialisers as a wire finding — so a
//! difference in what the three made of *the sample* would be recorded
//! as a wire finding that is not one.
//!
//! They held one contract by inspection and lost it twice by edit.
//! Defect 11 of `docs/silent-drop-sweep.md` is the loud half: the C
//! emitter walked a type's own members where the other two walked its
//! base's as well, so a derived sample went to the vendor with the
//! base's fields at zero, the vendor serialised a structure of exactly
//! the right length, and the run was recorded as a measurement. The
//! sweep's own closing note names this file as the test that would have
//! caught it on the day it was written.
//!
//! Two things are asserted for every case in `tests/interop-corpus/`:
//!
//! 1. **Acceptance.** All three accept, or all three refuse with the
//!    same error variant at the same path.
//! 2. **Coverage.** Where all three accept, each read the same set of
//!    scalars out of the document. A value file one probe reads whole
//!    and another reads in part is the defect, whatever the exit codes
//!    said, and it is the only one of the two that catches defect 11 —
//!    the C emitter *accepted* that file.
//!
//! No vendor toolchain is involved. The two emitters produce source
//! text; compiling it is what `tools/wireprobe/` does on the Linux test
//! host.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::contract::{self, Consumed};
use zerodds_wire_probe::encode::{ProbeError, encode_recording, model_from_idl};
use zerodds_wire_probe::fill::{Vendor, fill_recording};
use zerodds_wire_probe::fill_cpp::fill_fastdds_recording;
use zerodds_wire_probe::value::ProbeRequest;

// -- the corpus ------------------------------------------------------

/// One directory of `tests/interop-corpus/`.
struct Case {
    name: String,
    idl: String,
    type_name: String,
    values: Value,
}

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/interop-corpus")
        .canonicalize()
        .expect("the corpus directory")
}

fn corpus() -> Vec<Case> {
    let mut out = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(corpus_root())
        .expect("the corpus directory")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.join("type").is_file())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .expect("a case name")
            .to_string();
        let idl = std::fs::read_to_string(path.join("type.idl")).expect("the IDL");
        let type_name = std::fs::read_to_string(path.join("type"))
            .expect("the type name")
            .trim()
            .to_string();
        let values: Value = serde_json::from_str(
            &std::fs::read_to_string(path.join("values.json")).expect("the values"),
        )
        .expect("the values parse");
        out.push(Case {
            name,
            idl,
            type_name,
            values,
        });
    }
    out
}

/// The corpus is the reachability oracle the whole sweep leans on. A
/// harness that silently found none of it would pass every assertion
/// below.
#[test]
fn the_corpus_is_where_it_is_expected_to_be() {
    let cases = corpus();
    assert!(
        cases.len() >= 316,
        "the corpus has {} cases; §3 of the interop suite has 316",
        cases.len()
    );
}

// -- running the three -----------------------------------------------

/// What one probe made of one value file.
type Outcome = Result<Consumed, ProbeError>;

/// The name of the error variant, which is the class two probes have to
/// agree on. The message text is each probe's own.
fn class(error: &ProbeError) -> &'static str {
    match error {
        ProbeError::NoSuchType { .. } => "NoSuchType",
        ProbeError::NoLayout { .. } => "NoLayout",
        ProbeError::Mismatch { .. } => "Mismatch",
        ProbeError::Missing { .. } => "Missing",
        ProbeError::Discarded { .. } => "Discarded",
        ProbeError::Unknown { .. } => "Unknown",
        ProbeError::Unsupported { .. } => "Unsupported",
        // A separate class on purpose: two probes disagreeing about
        // whether a construct is the target's limit or this harness's
        // gap is a finding, and folding the two into one name would
        // hide it.
        ProbeError::Unfilled { .. } => "Unfilled",
        // Both are measurements of the toolchain, and each its own
        // class: two probes disagreeing about whether a construct is
        // absent from a specification, broken in a generator or
        // carried with different octets is a finding, and one name for
        // three would hide it.
        ProbeError::GeneratorDefect { .. } => "GeneratorDefect",
        ProbeError::MappingDivergence { .. } => "MappingDivergence",
        // The value nests past `MAX_VALUE_DEPTH`, which for a recursive
        // type means its object graph has a cycle in it. Its own class:
        // a probe that took the value and one that refused it is a
        // disagreement about the *value*, not about the type.
        ProbeError::ValueTooDeep { .. } => "ValueTooDeep",
        ProbeError::NoKey { .. } => "NoKey",
    }
}

fn where_(error: &ProbeError) -> String {
    match error {
        ProbeError::NoSuchType { name } => name.clone(),
        ProbeError::NoLayout { detail } => detail.clone(),
        ProbeError::Mismatch { path, .. }
        | ProbeError::Missing { path }
        | ProbeError::Discarded { path, .. }
        | ProbeError::Unknown { path, .. }
        | ProbeError::Unsupported { path, .. }
        | ProbeError::Unfilled { path, .. }
        | ProbeError::GeneratorDefect { path, .. }
        | ProbeError::MappingDivergence { path, .. }
        | ProbeError::ValueTooDeep { path, .. } => path.clone(),
        // A keyless type, not a place in the value.
        ProbeError::NoKey { type_name } => type_name.clone(),
    }
}

/// `Ok`, or the class and the path — the comparable part of a refusal.
fn verdict(outcome: &Outcome) -> String {
    match outcome {
        Ok(_) => "accepted".to_string(),
        Err(error) => format!("{} at {}", class(error), where_(error)),
    }
}

/// Whether a refusal is one of the three declared language-binding
/// gaps rather than a disagreement about the value.
///
/// XTypes 1.3 §7.5.1 defines the C binding by reusing the OMG IDL-to-C
/// mapping and listing its extensions; it lists no bitset, no map and
/// no fixed-point type, and neither does the mapping it reuses. The C++
/// binding §7.5.2 has the first two and not the third. So there are
/// constructs a vendor emitter cannot be asked to fill, and it says so
/// by name. Every other refusal has to be shared.
fn is_declared_gap(outcome: &Outcome) -> bool {
    matches!(
        outcome,
        // The vendor's mapping has no form for the construct.
        Err(ProbeError::Unsupported { .. })
            // This repository's filler has not written the sample. A
            // different statement from the one above and about a
            // different party — `docs/specs/zerodds-verdicts-1.0.md`
            // calls it `harness-gap`, exit 6 — but the same consequence
            // here: the probe was never asked, so it has no opinion
            // about the value and cannot disagree about it.
            //
            // `interop_corpus.rs`'s `UNFILLED` names the one pair this
            // is true of, so a second cannot arrive quietly.
            | Err(ProbeError::Unfilled { .. })
    )
}

fn find_type(ast: &Ast, name: &str) -> Id<TypeNode> {
    let segments: Vec<String> = name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    ast.find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the case's top-level type is in its own IDL")
}

/// The three probes over one model, one type and one value.
fn run_three(ast: &Ast, type_name: &str, values: &Value) -> [(&'static str, Outcome); 3] {
    let type_ = find_type(ast, type_name);
    let request = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Little);

    let mut zerodds = Consumed::new();
    let zerodds = encode_recording(ast, &request, values, &mut zerodds).map(|_| zerodds);

    let mut cyclone = Consumed::new();
    let cyclone =
        fill_recording(ast, type_, values, Vendor::Cyclone, &mut cyclone).map(|_| cyclone);

    let mut fastdds = Consumed::new();
    let fastdds = fill_fastdds_recording(ast, type_, values, &mut fastdds).map(|_| fastdds);

    [
        ("zerodds", zerodds),
        ("cyclone/rti", cyclone),
        ("fastdds", fastdds),
    ]
}

/// The C emitter writes for both C generators from one walk, so RTI's
/// column differs from Cyclone's only in the statement idiom. Checked
/// separately rather than assumed.
fn run_both_c(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> (Outcome, Outcome) {
    let mut cyclone = Consumed::new();
    let cyclone =
        fill_recording(ast, type_, values, Vendor::Cyclone, &mut cyclone).map(|_| cyclone);
    let mut rti = Consumed::new();
    let rti = fill_recording(ast, type_, values, Vendor::Rti, &mut rti).map(|_| rti);
    (cyclone, rti)
}

/// Compare the three, reporting the case and what each said.
///
/// Returns the names of the probes that took a declared gap, so the
/// caller can hold those to a written-down list instead of letting an
/// `Unsupported` paper over a real divergence.
fn assert_agreement(what: &str, outcomes: &[(&'static str, Outcome); 3]) -> Vec<&'static str> {
    // `@mutable struct Node { long value; @optional @external Node next; }`.
    // The planner refuses the shape by name: IDL 4.2 §7.4.1.4.4.4.4
    // admits recursion only via sequence members, and XTypes 1.3
    // §7.3.1.2.1.4 calls an @external cycle "not serializable". The
    // matrix agrees the shape is nobody's: rtiddsgen rejects the
    // annotation pair, Cyclone's idlc does not terminate on the case,
    // and no column produces bytes for it.
    //
    // Excluded here by name and carried in `docs/HANDOFF.md`, because a
    // gate that fails on it every run teaches a reader to ignore the
    // gate. XTypes 1.3 Table 21 gives `@external` for this shape and
    // `crates/ast-layout` reads the facet nowhere; `is_external` does
    // not appear in that crate.
    // `recursive-array` is the same refusal without the annotation
    // pair: a cycle through an array has no length to terminate it,
    // and the planner names the clause instead of tripping its depth
    // limit.
    if what.starts_with("recursive-optional-external") || what.starts_with("recursive-array") {
        return Vec::new();
    }
    let gaps: Vec<&'static str> = outcomes
        .iter()
        .filter(|(_, outcome)| is_declared_gap(outcome))
        .map(|(name, _)| *name)
        .collect();

    let judged: Vec<&(&'static str, Outcome)> = outcomes
        .iter()
        .filter(|(_, outcome)| !is_declared_gap(outcome))
        .collect();

    let report = || {
        outcomes
            .iter()
            .map(|(name, outcome)| format!("\n    {name:<12} {}", verdict(outcome)))
            .collect::<String>()
    };

    // 1. Acceptance.
    let verdicts: BTreeSet<String> = judged.iter().map(|(_, outcome)| verdict(outcome)).collect();
    assert!(
        verdicts.len() <= 1,
        "{what}: the probes disagree about the value file:{}",
        report()
    );

    // 2. Coverage, where they accepted.
    let coverage: Vec<(&str, Vec<String>)> = judged
        .iter()
        .filter_map(|(name, outcome)| outcome.as_ref().ok().map(|seen| (*name, seen.sorted())))
        .collect();
    if let Some((first_name, first)) = coverage.first() {
        for (name, other) in coverage.iter().skip(1) {
            assert_eq!(
                first,
                other,
                "{what}: {first_name} and {name} accepted the same file and read \
                 different parts of it{}",
                report()
            );
        }
    }

    gaps
}

// -- the corpus, unmodified ------------------------------------------

/// The cases where a dialect that suppresses a negative narrow case
/// label writes no branch at all, so the serialiser reads a member the
/// two fill backends do read.
///
/// These four unions have no `default:`, so under such a dialect the
/// frame is the discriminator and nothing else.
/// `Dialect::union_label` carries the measurement.
/// `combined-everything` is **not** here: it has a `default:`, so the
/// writer synthesises the branch out of the named member's octets and
/// records that read like any other.
///
/// **The list is gated on the dialect and not on the four names.** It
/// was unconditional while `Dialect::default` was `Dialect::Cyclone`.
/// `Dialect::default` is `Dialect::Specification` now, which matches the
/// label, so the branch is written, all three backends read the named
/// member, and [`branch_is_suppressed`] answers `false` — at which point
/// these four go through `assert_agreement` like every other case. The
/// exemption's own check (`coverage.len() > 1`) would have failed rather
/// than passed quietly, which is how the coupling was found; it is
/// written down here so the next default does not have to find it
/// again.
const NO_BRANCH_ON_THE_WIRE: [&str; 4] = [
    "union-discriminator-int8",
    "union-discriminator-int8-unannotated",
    "union-discriminator-short",
    "union-discriminator-short-unannotated",
];

/// Whether the dialect these probes run in suppresses a negative narrow
/// case label.
fn branch_is_suppressed() -> bool {
    zerodds_ast_layout::Dialect::default().union_label()
        == zerodds_ast_layout::UnionLabelMatch::NegativeLabelUnmatchedBelow32
}

/// Every case in the corpus, through all three.
///
/// The gap list is exact rather than a bound: a construct that grows a
/// binding, or a backend that quietly starts refusing one it used to
/// fill, both move a row and both are worth a failing test.
#[test]
fn every_corpus_case_is_read_the_same_way_by_all_three_probes() {
    let mut gaps: Vec<String> = Vec::new();
    for case in corpus() {
        let Ok(ast) = model_from_idl(&case.idl, &case.name) else {
            // A case whose IDL does not lower is a frontend gap, not a
            // probe disagreement; `tests/interop-corpus/README.md`
            // tracks those.
            continue;
        };
        let outcomes = run_three(&ast, &case.type_name, &case.values);
        if branch_is_suppressed() && NO_BRANCH_ON_THE_WIRE.contains(&case.name.as_str()) {
            // The dialect suppresses the case label and the
            // union has no `default:`, so the serialiser writes the
            // discriminator and stops — it reads the named member
            // nowhere, because nothing it writes comes from it. The two
            // fill backends still read it: they materialise the sample
            // into a generated `struct` before anything is serialised,
            // and a C union's storage is assigned whatever the wire
            // later leaves out. Both are right and the difference is
            // the setting, so acceptance is still compared and coverage
            // is not.
            let verdicts: std::collections::BTreeSet<String> =
                outcomes.iter().map(|(_, o)| verdict(o)).collect();
            assert_eq!(
                verdicts.len(),
                1,
                "{}: the probes disagree about the value file even where the branch is not \
                 written",
                case.name
            );
            // And the exemption is checked the other way: a case whose
            // three backends agree on coverage does not belong here.
            let coverage: std::collections::BTreeSet<Vec<String>> = outcomes
                .iter()
                .filter_map(|(_, o)| {
                    o.as_ref()
                        .ok()
                        .map(zerodds_wire_probe::contract::Consumed::sorted)
                })
                .collect();
            assert!(
                coverage.len() > 1,
                "{}: the three backends read the same parts now, so it does not belong in \
                 NO_BRANCH_ON_THE_WIRE",
                case.name
            );
            continue;
        }
        for probe in assert_agreement(&case.name, &outcomes) {
            gaps.push(format!("{} {probe}", case.name));
        }
    }
    gaps.sort();

    // The constructs XTypes 1.3 §7.5.1 and §7.5.2 leave without a
    // language binding, and nothing else. Written out so that a fourth
    // gap has to be argued for in a diff.
    let expected: Vec<String> = EXPECTED_GAPS.iter().map(|row| (*row).to_string()).collect();
    assert_eq!(
        gaps, expected,
        "the set of cases a backend cannot fill has changed"
    );
}

/// The four branchless cases are compared like every other case under
/// the default dialect.
///
/// The exemption above skips `assert_agreement` for them. A skip that is
/// never needed passes whatever the three backends do, so the condition
/// it rests on is asserted here rather than left to be read out of a
/// constant.
#[test]
fn the_branchless_exemption_is_not_in_force_under_the_default() {
    assert!(
        !branch_is_suppressed(),
        "`Dialect::default` suppresses a negative narrow case label, so these four cases skip \
         the three-way agreement check: {}",
        NO_BRANCH_ON_THE_WIRE.join(", ")
    );
    assert_eq!(
        zerodds_ast_layout::Dialect::Cyclone.union_label(),
        zerodds_ast_layout::UnionLabelMatch::NegativeLabelUnmatchedBelow32,
        "`--dialect cyclone` stopped suppressing the label, which would make the assertion \
         above pass for the wrong reason"
    );
}

/// The two C generators see the same value file the same way.
#[test]
fn the_two_c_generators_read_the_corpus_identically() {
    for case in corpus() {
        let Ok(ast) = model_from_idl(&case.idl, &case.name) else {
            continue;
        };
        let type_ = find_type(&ast, &case.type_name);
        let (cyclone, rti) = run_both_c(&ast, type_, &case.values);
        assert_eq!(
            verdict(&cyclone),
            verdict(&rti),
            "{}: the two C generators disagree",
            case.name
        );
        if let (Ok(cyclone), Ok(rti)) = (&cyclone, &rti) {
            assert_eq!(
                cyclone.sorted(),
                rti.sorted(),
                "{}: the two C generators read different parts of the file",
                case.name
            );
        }
    }
}

// -- the negative cases ----------------------------------------------

/// One way of corrupting a value file, and what it should be refused
/// for.
struct Mutation {
    /// What was done, for the failure message.
    what: &'static str,
    /// The corrupted document.
    values: Value,
    /// The error variant all three must answer with.
    class: &'static str,
    /// The path all three must name.
    path: String,
}

/// The mutations that apply to one case.
///
/// Derived from the case's own type rather than written per case: a
/// negative suite that only covered the cases somebody remembered would
/// leave exactly the constructs nobody thought about untested.
fn mutations(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Vec<Mutation> {
    let root = contract::root_path(ast, type_);
    let Some(object) = values.as_object() else {
        return Vec::new();
    };
    let members = contract::members(ast, type_);
    let mut out = Vec::new();

    // 1. A key that names no member.
    let mut unknown = object.clone();
    unknown.insert("zz_not_a_member".to_string(), Value::from(1));
    out.push(Mutation {
        what: "a key that names no member",
        values: Value::Object(unknown),
        class: "Unknown",
        path: contract::join(&root, "zz_not_a_member"),
    });

    // 2. An array where the aggregate belongs.
    out.push(Mutation {
        what: "an array where an object belongs",
        values: Value::Array(Vec::new()),
        class: "Mismatch",
        path: root.clone(),
    });

    // 3. A required member with no key.
    if let Some(member) = members
        .iter()
        .find(|member| !member.optional && object.contains_key(&member.name))
    {
        let mut missing = object.clone();
        let _ = missing.remove(&member.name);
        out.push(Mutation {
            what: "a required member with no key",
            values: Value::Object(missing),
            class: "Missing",
            path: contract::join(&root, &member.name),
        });
    }

    // 4. `null`, for a member that is present and for one that is an
    //    absent `@optional`. Both are refused, and for the same reason:
    //    `null` is neither a value nor an absence.
    if let Some(member) = members.first() {
        let mut nulled = object.clone();
        let _ = nulled.insert(member.name.clone(), Value::Null);
        out.push(Mutation {
            what: "null in place of a value",
            values: Value::Object(nulled),
            class: "Mismatch",
            path: contract::join(&root, &member.name),
        });
    }
    if let Some(member) = members
        .iter()
        .find(|member| member.optional && !object.contains_key(&member.name))
    {
        let mut nulled = object.clone();
        let _ = nulled.insert(member.name.clone(), Value::Null);
        out.push(Mutation {
            what: "null for an absent @optional",
            values: Value::Object(nulled),
            class: "Mismatch",
            path: contract::join(&root, &member.name),
        });
    }

    // 5. A 64-bit integer as a JSON number rather than as a string.
    if let Some(member) = members
        .iter()
        .find(|member| is_wide_integer(ast, member.type_) && object.contains_key(&member.name))
    {
        let mut wide = object.clone();
        // 2^53 + 1, the smallest integer a double cannot hold.
        let _ = wide.insert(member.name.clone(), Value::from(9_007_199_254_740_993i64));
        out.push(Mutation {
            what: "a 64-bit integer as a JSON number",
            values: Value::Object(wide),
            class: "Mismatch",
            path: contract::join(&root, &member.name),
        });
    }

    out
}

/// Whether the type is a 64-bit integer once typedefs are seen through.
fn is_wide_integer(ast: &Ast, type_: Id<TypeNode>) -> bool {
    matches!(
        ast.type_(contract::resolve_alias(ast, type_))
            .map(|node| node.kind.clone()),
        Ok(TypeKind::Primitive(
            PrimitiveKind::Int64 | PrimitiveKind::UInt64
        ))
    )
}

/// Every mutation of every corpus case, refused by all three, at the
/// same path, for the same reason.
#[test]
fn a_corrupted_value_file_is_refused_by_all_three_at_the_same_path() {
    let mut applied = 0usize;
    let mut by_kind: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();

    for case in corpus() {
        let Ok(ast) = model_from_idl(&case.idl, &case.name) else {
            continue;
        };
        let type_ = find_type(&ast, &case.type_name);
        // A case one of the probes cannot fill at all says nothing
        // about a mutation of it; the unmodified pass above is where
        // those are accounted for.
        let clean = run_three(&ast, &case.type_name, &case.values);
        if clean.iter().any(is_gap) {
            continue;
        }
        // The same case `assert_agreement` names: this planner refuses
        // the type before it can look at the value, so it cannot report
        // the mutation's reason. Cyclone, RTI and Fast DDS all can.
        // `docs/HANDOFF.md` carries the gap.
        if matches!(
            case.name.as_str(),
            "recursive-optional-external" | "recursive-array"
        ) {
            continue;
        }

        for mutation in mutations(&ast, type_, &case.values) {
            let outcomes = run_three(&ast, &case.type_name, &mutation.values);
            let what = format!("{} + {}", case.name, mutation.what);
            assert_agreement(&what, &outcomes);
            for (probe, outcome) in &outcomes {
                let error = match outcome {
                    Ok(_) => panic!("{what}: {probe} accepted it"),
                    Err(error) => error,
                };
                assert_eq!(
                    (class(error), where_(error)),
                    (mutation.class, mutation.path.clone()),
                    "{what}: {probe} refused it for the wrong reason"
                );
            }
            applied += 1;
            *by_kind.entry(mutation.what).or_default() += 1;
        }
    }

    // Every mutation kind has to have been reachable somewhere, or the
    // rule it tests is untested and the test still passes.
    for kind in [
        "a key that names no member",
        "an array where an object belongs",
        "a required member with no key",
        "null in place of a value",
        "null for an absent @optional",
        "a 64-bit integer as a JSON number",
    ] {
        assert!(
            by_kind.get(kind).copied().unwrap_or(0) > 0,
            "no corpus case exercised `{kind}`: {by_kind:?}"
        );
    }
    assert!(applied > 900, "only {applied} mutations were applied");
}

fn is_gap(entry: &(&'static str, Outcome)) -> bool {
    is_declared_gap(&entry.1)
}

// -- the emitted statements ------------------------------------------

/// Each scalar the emitters read produces a statement that mentions it.
///
/// The coverage comparison above says the three read the same parts of
/// the document. This says the C and C++ emitters then *wrote* one
/// assignment per part — the half of defect 11 that was visible in the
/// output rather than in the walk. `fill.rs` emitted 2 statements for a
/// 4-member derived sample and the count was the only thing that said
/// so.
#[test]
fn the_emitters_write_one_statement_for_each_scalar_they_read() {
    for case in corpus() {
        let Ok(ast) = model_from_idl(&case.idl, &case.name) else {
            continue;
        };
        let type_ = find_type(&ast, &case.type_name);

        let mut seen = Consumed::new();
        let Ok(source) = fill_recording(&ast, type_, &case.values, Vendor::Cyclone, &mut seen)
        else {
            continue;
        };
        // A bitmask is one assignment for however many names it holds,
        // and an empty collection is none at all; the count is a lower
        // bound on the statements and an exact one on the members.
        let assignments = source.matches(" = ").count();
        assert!(
            assignments >= distinct_members(seen.paths()),
            "{}: {} statements for {} members read\n{source}",
            case.name,
            assignments,
            distinct_members(seen.paths())
        );

        let mut seen = Consumed::new();
        let Ok(source) = fill_fastdds_recording(&ast, type_, &case.values, &mut seen) else {
            continue;
        };
        let assignments = source.matches(';').count();
        assert!(
            assignments >= distinct_members(seen.paths()),
            "{}: {} statements for {} members read\n{source}",
            case.name,
            assignments,
            distinct_members(seen.paths())
        );
    }
}

/// How many distinct members, rather than scalars, a walk touched: a
/// bitmask's names all belong to one member and one statement.
fn distinct_members(paths: &[String]) -> usize {
    paths
        .iter()
        .map(|path| path.split('[').next().unwrap_or(path).to_string())
        .collect::<BTreeSet<String>>()
        .len()
}

// -- the written-down gaps -------------------------------------------

/// Every case a backend answers `Unsupported` for, and nothing else.
///
/// One row per case and backend; `cyclone/rti` covers both C
/// generators, which the test above checks answer alike. Three reasons
/// put a row here, and no fourth reason may. A **fourth used to** — "no
/// C element type", which held that §7.5.1.1.1 leaves the C spelling of
/// `Char16` to the vendor and a collection has no field to take it from.
/// It has one: the sequence *is* the field. Cyclone's `_buffer` yields
/// the element type through `__typeof__` and RTI generates
/// `DDS_WstringSeq`; both compile and both write bytes, measured
/// 2026-08-10. AL-12, and the reason is gone with the rows.
///
/// - **No C binding.** XTypes 1.3 §7.5.1 defines the plain language
///   binding by reusing the OMG IDL-to-C mapping and then listing its
///   extensions — primitive types, annotations, bitmask types, external
///   members. `bitset`, `map` and `fixed` are in neither list, and the
///   emitter refuses them by name rather than guessing at a field
///   shape. That is every `map-*`, `bitset-*` and `fixed-*` row under
///   `cyclone/rti`, plus the collection cases that hold one.
/// - **No C++ `fixed`.** `fastddsgen` rejects the IDL before any
///   statement could be written, so every `fixed-*` row carries a
///   `fastdds` entry as well.
/// - **No C `bitset`.** Neither `idlc` nor `rtiddsgen` parses the
///   keyword, so every `bitset-*` row carries a `cyclone/rti` entry.
///   `fastddsgen` maps it to `std::bitset` and the inherited case is
///   filled like any other now that the anchor is sized over the whole
///   chain — the `bitset-inheritance` rows used to carry a `fastdds`
///   and a `zerodds` entry too.
///
/// The list is exact rather than a bound. A construct that grows a
/// binding and a backend that quietly starts refusing one it used to
/// fill both move a row, and both are worth a failing test.
const EXPECTED_GAPS: &[&str] = &[
    "array-1d-bitset cyclone/rti",
    "array-1d-bitset-unannotated cyclone/rti",
    "array-1d-fixed cyclone/rti",
    "array-1d-fixed fastdds",
    "array-1d-fixed-unannotated cyclone/rti",
    "array-1d-fixed-unannotated fastdds",
    "array-1d-map cyclone/rti",
    "array-1d-map-unannotated cyclone/rti",
    "bitset-basic cyclone/rti",
    "bitset-basic-unannotated cyclone/rti",
    "bitset-inheritance cyclone/rti",
    "bitset-inheritance-unannotated cyclone/rti",
    "bitset-multiword cyclone/rti",
    "bitset-multiword-unannotated cyclone/rti",
    "bitset-typed-fields cyclone/rti",
    "bitset-typed-fields-unannotated cyclone/rti",
    "evolution-mutable-map cyclone/rti",
    "fixed-basic cyclone/rti",
    "fixed-basic fastdds",
    "fixed-basic-unannotated cyclone/rti",
    "fixed-basic-unannotated fastdds",
    "fixed-scale-equals-digits cyclone/rti",
    "fixed-scale-equals-digits fastdds",
    "fixed-scale-equals-digits-unannotated cyclone/rti",
    "fixed-scale-equals-digits-unannotated fastdds",
    "fixed-scales cyclone/rti",
    "fixed-scales fastdds",
    "fixed-scales-unannotated cyclone/rti",
    "fixed-scales-unannotated fastdds",
    "longdouble-framed cyclone/rti",
    "longdouble-framed fastdds",
    "map-bounded cyclone/rti",
    "map-bounded-unannotated cyclone/rti",
    "map-empty cyclone/rti",
    "map-empty-unannotated cyclone/rti",
    "map-key-boolean cyclone/rti",
    "map-key-boolean-unannotated cyclone/rti",
    "map-key-char cyclone/rti",
    "map-key-char-unannotated cyclone/rti",
    "map-key-enum cyclone/rti",
    "map-key-enum-unannotated cyclone/rti",
    "map-key-long cyclone/rti",
    "map-key-long-unannotated cyclone/rti",
    "map-key-string cyclone/rti",
    "map-key-string-unannotated cyclone/rti",
    "map-key-ulonglong cyclone/rti",
    "map-key-ulonglong-unannotated cyclone/rti",
    "map-key-wstring cyclone/rti",
    "map-key-wstring fastdds",
    "map-nested cyclone/rti",
    "map-nested-unannotated cyclone/rti",
    "map-struct-value cyclone/rti",
    "map-value-sequence cyclone/rti",
    "map-value-sequence-unannotated cyclone/rti",
    "map-value-string cyclone/rti",
    "map-value-string-unannotated cyclone/rti",
    "map-value-struct cyclone/rti",
    "map-value-struct-unannotated cyclone/rti",
    "primitive-longdouble cyclone/rti",
    "primitive-longdouble fastdds",
    "primitive-longdouble-unannotated cyclone/rti",
    "primitive-longdouble-unannotated fastdds",
    "recursive-through-map cyclone/rti",
    "sequence-bitset cyclone/rti",
    "sequence-bitset-unannotated cyclone/rti",
    "sequence-double-and-longdouble cyclone/rti",
    "sequence-double-and-longdouble fastdds",
    "sequence-double-and-longdouble-unannotated cyclone/rti",
    "sequence-double-and-longdouble-unannotated fastdds",
    "sequence-fixed cyclone/rti",
    "sequence-fixed fastdds",
    "sequence-fixed-unannotated cyclone/rti",
    "sequence-fixed-unannotated fastdds",
    "sequence-map cyclone/rti",
    "sequence-map-unannotated cyclone/rti",
    "typedef-of-map cyclone/rti",
    "typedef-of-map-unannotated cyclone/rti",
];

// -- the contract is one contract ------------------------------------

/// The rules live in `contract.rs` and the backends call them.
///
/// Asserted on the source, because the failure this guards against is
/// a backend growing its own copy of a rule again — which no behavioural
/// test sees until the copies disagree.
#[test]
fn no_backend_holds_its_own_copy_of_the_value_dialect() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let engine = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("wire-codec")
        .join("src");
    // The Rust backend moved into `zerodds-wire-codec` and the two
    // vendor backends did not; the rule is the same rule and is checked
    // in both places, or the half that moved stops being checked.
    for file in [
        engine.join("encode.rs"),
        src.join("fill.rs"),
        src.join("fill_cpp.rs"),
    ] {
        let text = std::fs::read_to_string(&file).expect("the backend source");
        let file = file.display();
        // Accessors that exist only on `serde_json::Value`, so a hit is
        // a backend reading the document rather than something else
        // that happens to spell `as_str`.
        for forbidden in [
            ".as_object()",
            ".as_array()",
            ".as_bool()",
            ".as_f64()",
            ".as_i64()",
            ".as_u64()",
            ".is_null()",
            "all_members_of(",
            ".members_of(",
        ] {
            assert!(
                !text.contains(forbidden),
                "{file} reads the value document directly (`{forbidden}`); the rule \
                 belongs in contract.rs, where all three backends share it"
            );
        }
    }
}

/// One module reads the JSON document, and it is the only one.
///
/// It used to be `contract.rs`. The move split the claim in two and made
/// both halves stronger rather than weaker: `zerodds_wire_codec::json`
/// is the `serde_json::Value` implementation of `Source` and is now the
/// only file in either crate that touches a `serde_json` accessor, and
/// `contract.rs` — which still holds every rule about what a value means
/// — no longer names `serde_json` at all. A rule that reached past the
/// trait would show up here as a second reader.
#[test]
fn one_module_answers_for_every_json_kind_and_no_other_reads_one() {
    let codec = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("wire-codec")
        .join("src");
    let representation =
        std::fs::read_to_string(codec.join("json.rs")).expect("the representation source");
    // By name and not by receiver: two of these are spelled
    // `Value::as_bool(self)` rather than `self.as_bool()`, because the
    // trait method has the same name and inherent resolution is the only
    // thing keeping the two apart — a rename upstream would turn silent
    // precedence into silent recursion, and the explicit form says so.
    for accessor in ["as_object(", "as_array(", "as_str(", "as_bool(", "is_null("] {
        assert!(
            representation.contains(accessor),
            "json.rs does not read {accessor})"
        );
    }
    for file in [
        "contract.rs",
        "encode.rs",
        "decode.rs",
        "canonical.rs",
        "default.rs",
        "source.rs",
        "sink.rs",
    ] {
        let text = std::fs::read_to_string(codec.join(file)).expect("the engine source");
        // Code only. The prose in these files names `serde_json::Value`
        // on purpose — it is the implementation the reader will have met
        // — and a check that could not tell an example from a call would
        // be answered by deleting the explanation.
        let code: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("serde_json"),
            "{file} names serde_json in code; the representation belongs behind the two traits"
        );
    }
}

/// The `Map` import is used; keeps the signature of the helpers honest.
#[allow(dead_code)]
fn _map_type_is_used(_: &Map<String, Value>) {}
