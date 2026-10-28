// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! How much of the corpus a ROS 2 type support can be asked about.
//!
//! `rosidl`'s IDL is a subset of the corpus's. "How much of the corpus
//! does ROS reach" is therefore a result in its own right, and the cases
//! it cannot take have to be written down as such rather than skipped.
//! This walks every case through `ros_idl::ros_package` — the same
//! restriction `tools/wireprobe/wireprobe-ros` applies before it calls a
//! generator — and records the outcome per case in
//! `tests/fixtures/ros_reach.md`.
//!
//! The refusal reasons are grouped rather than listed one by one: a
//! reason is a property of `rosidl`, so a case moving between reasons is
//! interesting and a case moving between *reached* and *refused* is a
//! finding. Both are caught, because the counts are exact and the
//! fixture is written on every run.
//!
//! This test needs no ROS installation. It measures the restriction, and
//! the restriction is derived from measurements taken against `rosidl`
//! 3.1.6 on the Linux test host — those are in `docs/ros-column.md`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use zerodds_ast_core::ScopedName;
use zerodds_wire_probe::encode::{ProbeError, model_from_idl};
use zerodds_wire_probe::ros_idl::ros_package;

/// How many cases the corpus holds, counted from the directory rather
/// than written down.
///
/// It was a constant, and the constant went stale the moment cases were
/// added: `335` against 341 directories is a red gate that blocks every
/// run of the suite and says nothing about the thing under test. A
/// number that duplicates a countable set is wrong again at the next
/// addition, so the set is counted.
///
/// The guard the constant carried is kept and made explicit rather than
/// dropped with it: a walk that silently finds nothing must fail rather
/// than pass with an empty loop, so the count is asserted non-empty
/// here instead of being believed. What the constant could catch and
/// this cannot — a case *deleted* by accident — is caught by the
/// per-case fixtures, which name their cases.
fn cases() -> usize {
    let root = corpus_root();
    let count = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("the corpus at {}: {error}", root.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("type.idl").is_file())
        .count();
    assert!(count > 0, "the corpus at {} holds no cases", root.display());
    count
}

/// How many of them `rosidl` can express.
///
/// Exact, not a floor. A case gaining or losing a `rosidl` form is a
/// change in what this column measures and it has to be looked at.
/// 155 → 154 when the package generator learned to refuse a member
/// named by a C or C++ keyword (`identifier-keyword-member-ids-
/// unannotated`, member `class`): rosidl_generator_c spells the name
/// verbatim in a header the fastrtps type support compiles as C++.
/// 154 → 155 with the seven `@external`/recursion cases of 2026-08-24:
/// `annotation-external-typedef` is expressible (two files, the
/// typedef's target in its own), its six siblings are not (`@optional`,
/// `union`, recursion — the fixture names each).
/// 155 → 154 when the IDL front end took up IDL 4.2 §7.5.2:
/// `resolve-module-named-as-type` reached a model before and produced
/// two `rosidl` files; the file is not valid IDL and does not reach one
/// now, so the case is `Reach::NotValidIdl` and no longer a measurement
/// about `rosidl`.
const REACHED: usize = 154;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus")
}

/// The outcome for one case.
enum Reach {
    /// `rosidl` has a form for it; how many files the package needs.
    Expressible { files: usize },
    /// It has none: the construct, and the path it sits at.
    Refused {
        /// The construct with no `rosidl` form. Shared between cases, so
        /// it is what the summary groups on.
        kind: &'static str,
        /// Where in the type, so a refusal names something to act on.
        path: String,
    },
    /// `rosidl` **has** the form and its own generated code does not
    /// build.
    ///
    /// A different finding from [`Reach::Refused`] and the two were one
    /// word until `docs/specs/zerodds-verdicts-1.0.md` split them:
    /// `rosidl`'s IDL takes `wchar[3]` without a word, and what
    /// `rosidl_typesupport_fastrtps_c` 2.2.2 then emits names a
    /// `callbacks` that is declared nowhere in that function. Recording
    /// that as "ROS has no form for a collection of wchar" states the
    /// opposite of what was measured.
    ///
    /// This arm was missing while `ProbeError::GeneratorDefect` was
    /// added, so the walk panicked with "the rosidl restriction failed
    /// to decide" on a case it had decided perfectly well.
    GeneratorDefect {
        /// The construct whose generated code does not build.
        kind: &'static str,
        /// Where in the type.
        path: String,
    },
    /// The IDL front end refuses the file, so there is no model to ask
    /// `rosidl` about.
    ///
    /// A third answer and not a `Refused`: that one says "rosidl has no
    /// form for this construct", which would be a statement about
    /// `rosidl` that was never measured. The two cases here —
    /// `zerodds_wire_probe::REFUSED_BY_THE_FRONT_END` — are files no
    /// implementation is meant to accept, and their own READMEs say so.
    NotValidIdl {
        /// The front end's sentence, which names the clause.
        clause: String,
    },
}

fn walk() -> BTreeMap<String, Reach> {
    let root = corpus_root();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();

    let mut out = BTreeMap::new();
    for dir in dirs {
        let name = dir
            .file_name()
            .expect("a case directory has a name")
            .to_string_lossy()
            .into_owned();
        let read = |file: &str| {
            std::fs::read_to_string(dir.join(file))
                .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
        };
        let idl = read("type.idl");
        let type_name = read("type").trim().to_owned();
        // The two cases the IDL front end refuses; see
        // `zerodds_wire_probe::REFUSED_BY_THE_FRONT_END`. A file with no
        // model has no ROS form either, and the reach this walk records
        // is over the cases that reach a model. Asserted rather than
        // assumed, so a case that starts lowering comes back here.
        if zerodds_wire_probe::REFUSED_BY_THE_FRONT_END.contains(&name.as_str()) {
            let clause = model_from_idl(&idl, &format!("{name}/type.idl"))
                .err()
                .unwrap_or_else(|| {
                    panic!(
                        "{name}: the front end no longer refuses this file, so it belongs \
                         back in the walk"
                    )
                });
            out.insert(
                name,
                Reach::NotValidIdl {
                    clause: format!("{clause}"),
                },
            );
            continue;
        }
        let ast = model_from_idl(&idl, &format!("{name}/type.idl"))
            .unwrap_or_else(|error| panic!("{name}: the IDL does not lower: {error}"));
        let segments: Vec<String> = type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect();
        let type_ = ast
            .find_by_scoped_name(&ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .unwrap_or_else(|| panic!("{name}: `{type_name}` is not in the model"));

        let reach = match ros_package(&ast, type_) {
            Ok(package) => Reach::Expressible {
                files: package.files.len(),
            },
            // Two of the variants are statements about `rosidl` and the
            // rest mean the restriction hit a model it could not read,
            // which is this file's defect and not a measurement.
            Err(ProbeError::Unsupported { path, kind }) => Reach::Refused { kind, path },
            Err(ProbeError::GeneratorDefect { path, kind }) => {
                Reach::GeneratorDefect { kind, path }
            }
            Err(other) => panic!("{name}: the rosidl restriction failed to decide: {other}"),
        };
        out.insert(name, reach);
    }
    out
}

#[test]
fn the_corpus_reach_of_a_ros_type_support_is_measured_and_recorded() {
    let reach = walk();
    let total = cases();
    assert_eq!(reach.len(), total, "the corpus walk found the wrong number");

    let expressible: Vec<&String> = reach
        .iter()
        .filter(|(_, r)| matches!(r, Reach::Expressible { .. }))
        .map(|(name, _)| name)
        .collect();

    // Grouped by the refusal sentence, so the fixture says how much of
    // the gap each missing construct accounts for.
    let mut by_reason: BTreeMap<&'static str, Vec<&String>> = BTreeMap::new();
    for (name, r) in &reach {
        if let Reach::Refused { kind, .. } = r {
            by_reason.entry(kind).or_default().push(name);
        }
    }

    let mut out = String::new();
    out.push_str("<!-- generated by crates/wire-probe/tests/ros_corpus.rs -->\n");
    out.push_str("# What a ROS 2 type support can be asked about\n\n");
    let _ = writeln!(
        out,
        "`rosidl` 3.1.6 expresses **{} of the corpus's {total} cases**. \
         The {} it cannot are grouped below by the construct that stops \
         them.\n",
        expressible.len(),
        total - expressible.len()
    );
    let not_valid: Vec<&String> = reach
        .iter()
        .filter(|(_, r)| matches!(r, Reach::NotValidIdl { .. }))
        .map(|(name, _)| name)
        .collect();
    if !not_valid.is_empty() {
        let _ = writeln!(
            out,
            "{} of those never reach a model at all: the IDL front end refuses the file, \
             and the case exists to record that every implementation must. {}.\n",
            not_valid.len(),
            not_valid
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    out.push_str("## Why the rest cannot be expressed\n\n");
    out.push_str("| cases | the construct with no `rosidl` form |\n|---|---|\n");
    let mut reasons: Vec<(&&'static str, &Vec<&String>)> = by_reason.iter().collect();
    reasons.sort_by_key(|(reason, names)| (std::cmp::Reverse(names.len()), **reason));
    for (reason, names) in &reasons {
        let _ = writeln!(out, "| {} | {reason} |", names.len());
    }

    out.push_str("\n## Case by case\n\n");
    out.push_str("| case | `rosidl` | note |\n|---|---|---|\n");
    for (name, r) in &reach {
        match r {
            Reach::Expressible { files: 1 } => {
                let _ = writeln!(out, "| `{name}` | yes | one file |");
            }
            Reach::Expressible { files } => {
                let _ = writeln!(out, "| `{name}` | yes | {files} files |");
            }
            Reach::Refused { kind, path } => {
                let _ = writeln!(out, "| `{name}` | no | `{path}`: {kind} |");
            }
            Reach::NotValidIdl { clause } => {
                let _ = writeln!(
                    out,
                    "| `{name}` | not asked | the IDL front end refuses the file: {clause} |"
                );
            }
            // "yes" in the `rosidl` column, because it has the form; the
            // note carries what its generator then does with it.
            Reach::GeneratorDefect { kind, path } => {
                let _ = writeln!(
                    out,
                    "| `{name}` | yes | generator defect at `{path}`: {kind} |"
                );
            }
        }
    }

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ros_reach.md");
    std::fs::create_dir_all(fixture.parent().expect("the fixture has a parent"))
        .unwrap_or_else(|error| panic!("{}: {error}", fixture.display()));
    std::fs::write(&fixture, &out).unwrap_or_else(|error| panic!("{}: {error}", fixture.display()));

    assert_eq!(
        expressible.len(),
        REACHED,
        "the number of corpus cases a ROS type support can be asked about changed; \
         see {}",
        fixture.display()
    );
}

#[test]
fn every_family_of_the_corpus_is_either_reached_or_refused_for_a_stated_reason() {
    // A refusal with an empty or pathless message would leave a case
    // recorded as unreachable with nothing to act on.
    for (name, r) in walk() {
        let (kind, path) = match r {
            Reach::Expressible { .. } => continue,
            // The front end's own sentence names the clause; there is
            // no path in the type because there is no type.
            Reach::NotValidIdl { clause } => {
                assert!(
                    clause.len() > 20,
                    "{name}: the refusal `{clause}` says too little to act on"
                );
                continue;
            }
            // Both answers have to name something to act on. A generator
            // defect is the one a reader is most likely to mistake for a
            // missing form, so its sentence carries at least as much.
            Reach::Refused { kind, path } | Reach::GeneratorDefect { kind, path } => (kind, path),
        };
        assert!(
            !path.is_empty(),
            "{name}: the answer `{kind}` names no path"
        );
        assert!(
            kind.len() > 20,
            "{name}: the answer `{kind}` says too little to act on"
        );
    }
}
