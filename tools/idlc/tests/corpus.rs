// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every committed `.idl` goes through `check`.
//!
//! There was no such test. The tree has 748 `*.idl` files and nothing
//! ran the front end over all of them, so a file that stopped being
//! valid IDL only showed up if some other test happened to read it. That
//! is how `tools/wire-oracle/cases/bitmask_past_bound/type.idl` sat in
//! the tree: deliberately out of spec, handed only to other vendors'
//! generators, and indistinguishable from a file that had quietly gone
//! wrong. Adding the §7.4.13 bounds is what surfaced it — and the fact
//! that it took a new rule to surface it is the reason this test exists.
//!
//! The point is the list below. "Out of spec on purpose" and "out of
//! spec and nobody noticed" look identical in a tree with no gate; they
//! look nothing alike in a tree where the first has to be written down
//! with a reason.
//!
//! What this measures is `check` — parse, preprocess, lower, resolve —
//! the same pipeline `zerodds-idlc check` runs, over `git ls-files`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

/// A file that `check` refuses, and why that is intended.
///
/// Every entry names the codes it is allowed to report. A file that
/// reports a code not listed here fails the test even though it is on
/// the list — the exception is for the defect that was reasoned about,
/// not a blanket pass for the file.
struct Intended {
    /// Path from the repository root.
    path: &'static str,
    /// The diagnostic codes it is allowed to report, sorted, deduplicated.
    codes: &'static [&'static str],
    /// Why `check` refuses the file on purpose — because the file is
    /// out of spec, or because a dialect axis defaults to refusing what
    /// the clause permits. One line, and it has to say what the file is
    /// *for* — a reason that only restates the diagnostic explains
    /// nothing.
    reason: &'static str,
}

/// The files `check` refuses on purpose.
///
/// Three groups: probe inputs whose whole point is a construct the front
/// end must refuse; grammar-coverage fixtures curated for syntax rather
/// than for meaning; and one wire-measurement case that asks other
/// vendors what they do with input the specification forbids.
///
/// Each is invalid IDL or an input a front end is probed with.
const INTENDED: &[Intended] = &[
    Intended {
        path: "crates/spatial-ros2/idl/ros_msgs.idl",
        codes: &["IDL0008"],
        reason: "ROS 2's own member form, `Pose pose;`, which OMG IDL 4.2 §7.2.3.1 \
                 refuses and the ROS 2 dialect admits: `zerodds-idlc --ros2` and \
                 crates/spatial-ros2/build.rs read it under that dialect",
    },
    // -- probe inputs: the refusal is the measurement ---------------
    Intended {
        path: "tools/hashid-gate/cases/collision/type.idl",
        codes: &["IDL0016"],
        reason: "the hashid gate's collision case: two members whose @hashid names hash to \
                 one id, which is what the case is there to make the gate report",
    },
    Intended {
        path: "tools/hashid-gate/cases/idandhashid/type.idl",
        codes: &["IDL0007"],
        reason: "the hashid gate's case for @id and @hashid on one member, which \
                 DDS-XTypes 1.3 §7.3.1.2.1.1 forbids and the gate exists to prove is caught",
    },
    Intended {
        path: "tools/inventory-probes/key-map/types.idl",
        codes: &["IDL0007"],
        reason: "an inventory probe for @key on a map member, which the model has no slot \
                 for; the probe records the refusal, not an emission",
    },
    // `tools/inventory-probes/key-union/types.idl` holds §7.2.2.4.4.4.8's
    // own example, `union MyCommand switch (@key CommandKind)`, which
    // Table 21 permits ("Structure Members, Union discriminator").
    // `KeyOnUnionDiscriminator` is `Carried` by default, so `check`
    // accepts it and it is not on this list; the refusal that measured
    // front ends make is the `Refused` setting.
    Intended {
        path: "tools/unspecified-reach/cases/collection_appendable_member.idl",
        codes: &["IDL0007"],
        reason: "a reach probe for @appendable written on a member, which DDS-XTypes 1.3 \
                 Table 21 lists for type declarations only; the probe records what a \
                 front end does with the form, and `check` refuses it by that table",
    },
    Intended {
        path: "tools/try-construct-gate/fe/f_map.idl",
        codes: &["IDL0007"],
        reason: "the try-construct gate's front-end case for @try_construct on a map key, \
                 which can only be DISCARD; the gate asserts the diagnostic",
    },
    Intended {
        path: "tools/try-construct-gate/fe/g_trim_long.idl",
        codes: &["IDL0007"],
        reason: "the try-construct gate's front-end case for TRIM on a primitive, which \
                 has nothing to trim; the gate asserts the diagnostic",
    },
    // -- grammar-coverage fixtures: legal to write, wrong to mean ---
    //
    // The same three files `crates/idl-lower/tests/conformance.rs`
    // curates in its own EXPECTED list, for the same reason. That test
    // measures lowering; this one measures the whole of `check`, so
    // `ccm_components.idl` earns resolve diagnostics here that it does
    // not earn there.
    Intended {
        path: "crates/idl-parse/tests/fixtures/spec_features/ccm_components.idl",
        codes: &["IDL0010", "IDL0015"],
        reason: "a CCM grammar-coverage fixture: forward declarations it never defines and \
                 component clauses naming scopes it never declares, written for syntax \
                 coverage rather than for a model that closes",
    },
    Intended {
        path: "crates/idl-parse/tests/fixtures/spec_features/corba_value_repository.idl",
        codes: &["IDL0010"],
        reason: "a CORBA repository-id fixture: `typeprefix Bar` and `typeid Bar` \
                 demonstrate the directives, and the file declares no `Bar` for them to \
                 apply to (CORBA 3.3 §10.7)",
    },
    Intended {
        path: "crates/idl-parse/tests/fixtures/spec_features/fastdds_pragmas.idl",
        codes: &["IDL0016"],
        reason: "a Fast DDS annotation fixture where two members share @hashid(\"position\") \
                 and so hash to one id; kept as written because the fixture is about the \
                 annotation form and the collision is what the model is meant to report",
    },
    Intended {
        path: "crates/idl-parse/tests/fixtures/omg/zerodds_security.idl",
        codes: &["IDL0010"],
        reason: "the DDS-Security IDL as the specification prints it, which names \
                 `DomainId_t` from DDS-DCPS without including it; kept verbatim so the \
                 fixture stays comparable to the printed clause",
    },
    Intended {
        path: "crates/corba-codegen/tests/orb-corpus/rows/05-typecode.idl",
        codes: &["IDL0010"],
        reason: "an ORB-corpus row for `CORBA::TypeCode`, a name every ORB predeclares and \
                 no `.idl` in this corpus declares; the row is fed to real ORBs, which \
                 supply it",
    },
    // -- wire measurements of what the specification forbids --------
    Intended {
        path: "tests/interop-corpus/resolve-module-named-as-type/type.idl",
        codes: &["IDL0008"],
        reason: "the interop corpus's case for `module common { struct common { … }; }`,                  which IDL 4.2 §7.5.2 refuses — the clause's own example is                  `module M { typedef short M; }` marked `// Error`; the case measures which                  vendors refuse it and which resolve it anyway",
    },
    Intended {
        path: "tests/interop-corpus/recursive-array/type.idl",
        codes: &["IDL0014"],
        reason: "the interop corpus's recursion-through-an-array case: an array has no                  length on the wire, so the cycle never terminates and IDL 4.2                  §7.4.1.4.4.4.4 does not admit it — its README says every implementation                  must refuse the file and the refusal is the measurement",
    },
    Intended {
        path: "tools/wire-oracle/cases/bitmask_past_bound/type.idl",
        codes: &["IDL0014"],
        reason: "a wire-oracle case asking Cyclone, Fast DDS, OpenDDS and RTI what they do \
                 with @position(7) under @bit_bound(4), which IDL 4.2 §7.4.13.4.3.3 \
                 forbids; byte-gate hands it to those generators and never to this one",
    },
];

/// Every committed `.idl`, from `git ls-files`.
///
/// `git ls-files` rather than a directory walk, for the reason the gap
/// scan uses it: a file that is in the tree but not committed is not
/// part of what a release ships, and a build artefact under `target/`
/// is not IDL anyone wrote.
fn committed_idl(repo: &std::path::Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "--", "*.idl"])
        .current_dir(repo)
        .output()
        .expect("git ls-files runs");
    assert!(output.status.success(), "git ls-files failed");
    String::from_utf8(output.stdout)
        .expect("git ls-files writes utf-8")
        .lines()
        // The release mirror is a copy of the tree and would double
        // every file; the gap scan's own corpus is probes, not sources.
        .filter(|path| !path.starts_with("github/"))
        .filter(|path| !path.starts_with("tools/idl-4.2-gap-scan/"))
        .map(ToString::to_string)
        .collect()
}

/// The distinct diagnostic codes `check` reports for one file, sorted.
fn codes_of(repo: &std::path::Path, path: &str) -> Vec<String> {
    let full = repo.join(path);
    let source = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path}: {e}"));
    let composed = zerodds_idl_compose::compose_source(
        &full.to_string_lossy(),
        &source,
        // The loader `check` uses, so an `#include` resolves here the
        // way it resolves there.
        &zerodds_idl_compose::PathLoader::bare(),
    );
    let mut codes: Vec<String> = composed
        .diagnostics
        .iter()
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .map(|d| d.code.as_str().to_string())
        .collect();
    codes.sort_unstable();
    codes.dedup();
    codes
}

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

#[test]
fn every_committed_idl_passes_check_or_is_on_the_list_with_a_reason() {
    let repo = repo_root();
    let files = committed_idl(&repo);
    assert!(
        files.len() >= 700,
        "the corpus is present ({} files found)",
        files.len()
    );

    let mut unexplained = Vec::new();
    for path in &files {
        let codes = codes_of(&repo, path);
        if codes.is_empty() {
            continue;
        }
        let Some(intended) = INTENDED.iter().find(|i| i.path == *path) else {
            unexplained.push(format!("  {path}: {codes:?} — not on the list"));
            continue;
        };
        if codes != intended.codes {
            unexplained.push(format!(
                "  {path}: reports {codes:?}, the list allows {:?}",
                intended.codes
            ));
        }
    }
    assert!(
        unexplained.is_empty(),
        "{} committed .idl file(s) are refused by `check` without a written reason.\n{}\n\
         Either the file is wrong and the fix belongs in the file, or it is out of spec on \
         purpose and belongs in INTENDED in this test with a line saying what it is for.",
        unexplained.len(),
        unexplained.join("\n")
    );
}

#[test]
fn the_list_names_only_files_that_still_earn_their_place() {
    // The other half. An entry that stopped reporting would otherwise
    // sit here forever, and the next person would read it as a
    // description of the tree. Same rule the coverage pages hold to: a
    // stale exception is worse than none.
    let repo = repo_root();
    let mut stale = Vec::new();
    for intended in INTENDED {
        let full = repo.join(intended.path);
        if !full.exists() {
            stale.push(format!("  {}: no such file", intended.path));
            continue;
        }
        let codes = codes_of(&repo, intended.path);
        if codes.is_empty() {
            stale.push(format!(
                "  {}: passes `check` now — take it off the list",
                intended.path
            ));
        }
        assert!(
            !intended.reason.is_empty(),
            "{}: an entry without a reason is not an exception, it is a silence",
            intended.path
        );
    }
    assert!(stale.is_empty(), "{}", stale.join("\n"));
}

#[test]
fn the_list_is_sorted_within_its_groups_and_free_of_duplicates() {
    // A list nobody can scan stops being read. Duplicates are the
    // failure that matters: two entries for one file mean the second is
    // dead and its reason is never checked against anything.
    let mut seen = std::collections::BTreeSet::new();
    for intended in INTENDED {
        assert!(
            seen.insert(intended.path),
            "{} appears twice in INTENDED",
            intended.path
        );
        let mut sorted = intended.codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.as_slice(),
            intended.codes,
            "{}: codes must be sorted and deduplicated",
            intended.path
        );
    }
}
