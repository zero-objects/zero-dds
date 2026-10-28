// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One specification, one opinion about what is in it.
//!
//! Two things in this tree decide whether a corpus case belongs to a
//! toolchain's specification, and they used to disagree:
//!
//! - `zerodds_ast::profile::Profile`, which answers "is this model
//!   inside CORBA 3.3 / ROS 2" and is what a *report* uses to pick a
//!   denominator.
//! - `wireprobe-corba-idl` and `wireprobe-ros-package`, which rewrite a
//!   type into that specification's IDL and refuse by name when it has
//!   no form for a construct.
//!
//! The profile was the laxer of the two. It admitted a structure with a
//! base, an enumeration with a `@bit_bound`, an enumerator with an
//! explicit `@value` and — for ROS — an enumeration at all, all of
//! which the rewriters refuse with a clause. Since the profile is what
//! a denominator comes from, the disagreement did not show up as a
//! failing test; it showed up as a table saying the CORBA ORBs refused
//! 127 cases, when CORBA 3.3 does not define most of them.
//!
//! # What this asserts
//!
//! That every case the profile calls *inside* is one the rewriter does
//! not refuse **for the absence of a form**. Exit 3 is the one answer
//! compared here; a rewriter that refuses for a different reason —
//! exit 7 for a generator whose own output does not build, exit 8 for
//! a mapping whose octets differ — is answering a different question
//! and is not a disagreement. `docs/specs/zerodds-verdicts-1.0.md` has
//! the vocabulary.
//!
//! The other direction is not asserted. A case the profile calls
//! outside and the rewriter would carry is the profile being strict,
//! which costs a column nothing.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_ast::profile::Profile;

/// Refusals the profile is right to disagree with — none.
///
/// It held two while `no-form` was one word for three answers: CORBA's
/// `long double`, which `omniidl` and `tao_idl` both take and whose
/// C++ mapping is x87 extended rather than §9.3.1.3's binary128, and
/// `rosidl`'s collection of `wchar`, which its IDL accepts and whose
/// generated C++ does not build. Both are measurements of a toolchain
/// and neither is an absence in a specification; with
/// `mapping-divergence` and `generator-defect` to say so — exit 8 and
/// exit 7 — they left through their own door and the exceptions went
/// with them.
///
/// The array stays because the next one will need it, and because an
/// empty exception list is a claim worth making explicitly.
const KNOWN: [(&str, &str, &str); 0] = [];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether the rewriter for a column carries a case.
///
/// Exit 3 is "this specification has no form for it" — the one answer
/// this test compares against the profile. Any other non-zero status is
/// a different statement and is reported as such.
fn rewriter_refuses_for_no_form(program: &Path, case: &Path, type_name: &str) -> Option<String> {
    let output = Command::new(program)
        .args(["--idl", &case.join("type.idl").to_string_lossy()])
        .args(["--type", type_name])
        .args(["--out", "/dev/null"])
        .output()
        .ok()?;
    if output.status.success() {
        return None;
    }
    if output.status.code() != Some(3) {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .next()
            .unwrap_or("refused")
            .to_string(),
    )
}

#[test]
fn what_a_profile_admits_its_rewriter_can_carry() {
    let root = repository_root();
    let corpus = root.join("tests/interop-corpus");
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&corpus)
        .expect("the corpus")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.join("type.idl").is_file())
        .collect();
    cases.sort();

    let columns = [
        ("tao", Profile::Corba33, "wireprobe-corba-idl"),
        ("ros", Profile::Ros2, "wireprobe-ros-package"),
    ];
    let mut disagreements = Vec::new();
    let mut checked = 0;

    for (column, profile, program) in columns {
        let program = root.join("target/debug").join(program);
        if !program.is_file() {
            println!(
                "SKIPPED: {} is not built; nothing measured",
                program.display()
            );
            return;
        }
        for case in &cases {
            let name = case
                .file_name()
                .expect("a directory name")
                .to_string_lossy()
                .into_owned();
            let source = std::fs::read_to_string(case.join("type.idl")).expect("the IDL");
            let parsed = zerodds_idl_parse::parse(&source);
            let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "t.idl");
            if lowered.ast.validate(profile).is_err() {
                continue;
            }
            checked += 1;
            let type_name = std::fs::read_to_string(case.join("type"))
                .expect("the corpus names its root")
                .trim()
                .to_string();
            let Some(reason) = rewriter_refuses_for_no_form(&program, case, &type_name) else {
                continue;
            };
            if KNOWN.iter().any(|(known_column, fragment, _)| {
                *known_column == column && reason.contains(fragment)
            }) {
                continue;
            }
            disagreements.push(format!("{column} {name}: {reason}"));
        }
    }

    assert!(
        checked > 200,
        "the profiles admitted {checked} cases across both columns, which is not a measurement"
    );
    assert!(
        disagreements.is_empty(),
        "the profile calls these inside its specification and the rewriter says the \
         specification has no form for them — one of the two is wrong:\n  {}",
        disagreements.join("\n  ")
    );
}
