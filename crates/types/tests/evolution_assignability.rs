// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! §7.2.4's assignability rules against the corpus that measured the
//! bytes.
//!
//! Every corpus case with an `old.idl` is a writer and a reader over two
//! declarations of one type. Two questions can be asked of such a pair
//! and they are not the same question:
//!
//! - **Is `old` assignable to `type`?** XTypes 1.3 §7.2.4.4.8, Table 19.
//!   This is what a discovery match is supposed to decide, and it is
//!   decided from the two `TypeObject`s alone, with no bytes involved.
//! - **Does `type` read the bytes `old` wrote?** That is the
//!   `## Evolution` table in each case's README, produced by running the
//!   two through the probe, and held to by the Rust, C and Python
//!   columns in `crates/wire-probe/tests/evolution.rs`.
//!
//! The point of this file is the **correspondence** between them. A rule
//! that says two types are assignable is a promise that a reader can
//! read a writer's sample; where the recorded bytes say it cannot, one
//! of the two is wrong and the disagreement is the finding. This is what
//! keeps the rules from being checked only against types this crate
//! built itself — the corpus rows are the probe's output over the same
//! declarations, and four foreign implementations are held to those same
//! rows in `tests/interop-results/`.
//!
//! ## GitHub issues #41 and #42
//!
//! `evolution-final-to-appendable` is the pair those reports are. Every
//! member is the same, every type is the same, every name is the same,
//! and the two files differ in one annotation. Table 19 makes the pair
//! **not assignable**: two types are assignable only if their
//! extensibility kinds agree. So the reported failure is what the clause
//! requires and not a defect in either side — and a discovery that
//! consulted §7.2.4 would have refused the match instead of matching and
//! then failing to decode, which is what tracker 30 asks for.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use zerodds_ast::Ast;
use zerodds_types::assignability::{AssignabilityConfig, Assignable, is_assignable};
use zerodds_types::from_ast::{Form, minimal_type_object, register_all, type_identifier};
use zerodds_types::resolve::TypeRegistry;

/// How many corpus cases carry two IDL files.
///
/// Asserted rather than counted, for the reason `interop_corpus.rs`
/// gives: a walk that silently found nothing passes with an empty loop.
const EVOLUTION_CASES: usize = 15;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus")
}

fn model(source: &str, path: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(source);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "{path}: {errors:?}");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), path).ast;
    let report = ast
        .resolve_all_by_name()
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    assert!(report.is_clean(), "{path}: {:?}", report.broken);
    ast
}

fn top_level(ast: &Ast, name: &str, path: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{path}: no type named `{name}`"))
}

/// The `## Evolution` rows of one README, keyed by the flag string.
///
/// A row whose cell starts `**no**` is a refusal; anything else is the
/// value the reader handed back.
fn recorded_rows(readme: &str, name: &str) -> BTreeMap<String, bool> {
    let mut rows = BTreeMap::new();
    let Some((_, section)) = readme.split_once("## Evolution") else {
        panic!("{name}/README.md: a case with an `old.idl` and no `## Evolution` section");
    };
    for line in section.lines() {
        let Some(rest) = line.strip_prefix("| `--") else {
            continue;
        };
        let Some((flag, cell)) = rest.split_once("` | ") else {
            panic!("{name}/README.md: `{line}` is not an evolution row");
        };
        rows.insert(
            format!("--{flag}"),
            !cell.trim_start().starts_with("**no**"),
        );
    }
    assert!(
        !rows.is_empty(),
        "{name}/README.md: an `## Evolution` section with no rows"
    );
    rows
}

struct Case {
    name: String,
    /// Whether §7.2.4 says the writer's type is assignable to the
    /// reader's.
    assignable: Result<(), &'static str>,
    /// Whether the recorded XCDR2 rows say the reader read the bytes.
    reads_under_xcdr2: bool,
    /// Whether the recorded XCDR1 rows say so.
    reads_under_xcdr1: bool,
}

fn cases() -> Vec<Case> {
    let root = corpus_root();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.join("old.idl").is_file())
        .collect();
    dirs.sort();
    assert_eq!(
        dirs.len(),
        EVOLUTION_CASES,
        "the corpus has {} cases with an `old.idl` and this file expects {EVOLUTION_CASES}",
        dirs.len()
    );

    dirs.iter()
        .map(|dir| {
            let name = dir
                .file_name()
                .expect("a case directory has a name")
                .to_string_lossy()
                .into_owned();
            let read = |file: &str| {
                std::fs::read_to_string(dir.join(file))
                    .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
            };
            let type_name = read("type").trim().to_owned();
            let old = model(&read("old.idl"), &format!("{name}/old.idl"));
            let new = model(&read("type.idl"), &format!("{name}/type.idl"));

            // One registry over both models: a member's identifier is a
            // hash, and `is_assignable` follows it back to the object
            // through the registry.
            let mut registry = TypeRegistry::new();
            for (ast, which) in [(&old, "old.idl"), (&new, "type.idl")] {
                let skipped = register_all(ast, &mut registry)
                    .unwrap_or_else(|error| panic!("{name}/{which}: {error}"));
                assert!(
                    skipped.is_empty(),
                    "{name}/{which}: {skipped:?} — an evolution case declares only \
                     constructs XTypes can represent"
                );
            }

            let writer = type_identifier(
                &old,
                top_level(&old, &type_name, &format!("{name}/old.idl")),
                Form::Minimal,
            )
            .unwrap_or_else(|error| panic!("{name}/old.idl: {error}"));
            let reader = type_identifier(
                &new,
                top_level(&new, &type_name, &format!("{name}/type.idl")),
                Form::Minimal,
            )
            .unwrap_or_else(|error| panic!("{name}/type.idl: {error}"));

            let assignable =
                match is_assignable(&writer, &reader, &registry, &AssignabilityConfig::default()) {
                    Assignable::Yes => Ok(()),
                    Assignable::No(why) => Err(why),
                };

            let rows = recorded_rows(&read("README.md"), &name);
            let reads = |flag: &str| {
                *rows
                    .get(flag)
                    .unwrap_or_else(|| panic!("{name}/README.md: no `{flag}` row"))
            };
            let reads_under_xcdr2 =
                reads("--encoding xcdr2 --endian le") && reads("--encoding xcdr2 --endian be");
            let reads_under_xcdr1 =
                reads("--encoding xcdr1 --endian le") && reads("--encoding xcdr1 --endian be");
            Case {
                name,
                assignable,
                reads_under_xcdr2,
                reads_under_xcdr1,
            }
        })
        .collect()
}

#[test]
fn the_pair_github_issues_41_and_42_report_is_not_assignable() {
    let cases = cases();
    let case = cases
        .iter()
        .find(|c| c.name == "evolution-final-to-appendable")
        .expect("the corpus holds `evolution-final-to-appendable`");
    assert_eq!(
        case.assignable,
        Err("extensibility mismatch"),
        "§7.2.4.4.8, Table 19: two types are assignable only if their extensibility kinds agree, \
         and this pair differs in exactly that one annotation"
    );
    // And what the bytes did, which is the report: readable under
    // encoding version 1 and not under version 2. The rule refuses the
    // pair in both, which is stricter than the bytes in one of them —
    // §7.2.4 is about the types and not about whether a particular
    // encoding happens to survive the difference.
    assert!(
        case.reads_under_xcdr1,
        "under XCDR1 the two readings are octet for octet identical, which is why the report \
         says the pair worked until XCDR2 was turned on"
    );
    assert!(
        !case.reads_under_xcdr2,
        "under XCDR2 the writer emits no delimiter and the reader expects one"
    );
}

#[test]
fn assignability_never_promises_an_exchange_the_bytes_refuse() {
    // The direction that matters for a discovery match: where §7.2.4
    // says two types are assignable, the reader has to be able to read
    // what the writer wrote. The other direction does not hold and is
    // not asserted — a pair the rules refuse may still decode by
    // accident, because a rule about types is not a rule about bytes.
    let mut trouble = Vec::new();
    for case in cases() {
        if case.assignable.is_ok() && !case.reads_under_xcdr2 {
            trouble.push(format!(
                "{}: §7.2.4 says assignable and the recorded XCDR2 rows say the reader refused",
                case.name
            ));
        }
    }
    assert!(trouble.is_empty(), "{}", trouble.join("\n"));
}

/// The cases where §7.2.4 refuses a pair whose bytes decode.
///
/// Two, and both for the same clause. §7.2.4.4.4.4 makes a member with
/// `must_understand` that the other side does not declare fatal to the
/// match — which is the point of the flag: the reader would otherwise
/// receive a sample, find the member absent, take its default and never
/// know. The stream decodes precisely because nothing in it is wrong;
/// what is wrong is that a member the reader was told it must understand
/// is not one the writer will ever send.
///
/// `evolution-mutable-key` is the second because §7.2.2.4.4.4.8 gives a
/// key member must-understand always, and a reader that added a key
/// member is a reader with a different notion of instance identity.
const REFUSED_THOUGH_THE_BYTES_DECODE: [&str; 2] =
    ["evolution-mutable-key", "evolution-mutable-mustunderstand"];

#[test]
fn the_rules_and_the_recorded_bytes_disagree_only_where_must_understand_says_so() {
    let mut unexpected = Vec::new();
    for case in cases() {
        let refuses = case.assignable.is_err();
        let expected = REFUSED_THOUGH_THE_BYTES_DECODE.contains(&case.name.as_str());
        if refuses && case.reads_under_xcdr2 && !expected {
            unexpected.push(format!(
                "{}: §7.2.4 refuses the pair and the recorded XCDR2 rows read it — {:?}",
                case.name, case.assignable
            ));
        }
        if expected && !(refuses && case.reads_under_xcdr2) {
            unexpected.push(format!(
                "{}: listed as a must-understand refusal over decodable bytes and is not one \
                 — assignable {:?}, reads {}",
                case.name, case.assignable, case.reads_under_xcdr2
            ));
        }
    }
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n"));
}

#[test]
fn every_evolution_case_gets_a_verdict_and_the_verdicts_are_recorded() {
    // The table, printed, so a change in it is visible in the run and
    // not only in an assertion that flipped. Every case is named, so a
    // case added to the corpus without a verdict fails the count above
    // rather than passing unnoticed.
    let cases = cases();
    let mut refused = 0;
    for case in &cases {
        match case.assignable {
            Ok(()) => println!("{:<44} assignable", case.name),
            Err(why) => {
                refused += 1;
                println!("{:<44} not assignable — {why}", case.name);
            }
        }
    }
    assert_eq!(
        cases.len(),
        EVOLUTION_CASES,
        "every case with an `old.idl` gets a verdict"
    );
    assert!(
        refused > 0 && refused < cases.len(),
        "a rule that said the same thing about all {} pairs would not be a rule: {refused} refused",
        cases.len()
    );
}

#[test]
fn a_type_is_assignable_to_itself() {
    // The identity §7.2.4.1 gives, checked through the same path the
    // corpus cases take rather than through a hand-built TypeObject.
    let ast = model(
        "@appendable struct Same { long a; double b; };",
        "identity.idl",
    );
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("a registrable model");
    let id = type_identifier(&ast, top_level(&ast, "Same", "identity.idl"), Form::Minimal)
        .expect("a TypeIdentifier");
    assert_eq!(
        is_assignable(&id, &id, &registry, &AssignabilityConfig::default()),
        Assignable::Yes
    );
}

#[test]
fn the_extensibility_of_the_declaration_is_what_reaches_the_type_object() {
    // The mechanism behind the #41/#42 verdict: the flag bit in the
    // TypeObject is the frontend's `effective_extensibility` and nothing
    // this crate decided again. An unannotated declaration is
    // `@appendable` — §7.3.1.2.1.8's value — which is why the pair in
    // the corpus has to write `@final` explicitly to differ.
    use zerodds_types::type_object::flags::StructTypeFlag;
    use zerodds_types::type_object::minimal::MinimalTypeObject;

    for (source, expected, label) in [
        (
            "@final struct T { long a; };",
            StructTypeFlag::IS_FINAL,
            "@final",
        ),
        (
            "@appendable struct T { long a; };",
            StructTypeFlag::IS_APPENDABLE,
            "@appendable",
        ),
        (
            "@mutable struct T { long a; };",
            StructTypeFlag::IS_MUTABLE,
            "@mutable",
        ),
        (
            "struct T { long a; };",
            StructTypeFlag::IS_APPENDABLE,
            "no annotation at all",
        ),
    ] {
        let ast = model(source, "extensibility.idl");
        let object = minimal_type_object(&ast, top_level(&ast, "T", "extensibility.idl"))
            .expect("a TypeObject");
        let MinimalTypeObject::Struct(structure) = object else {
            panic!("{label}: a struct lowers to a struct");
        };
        assert_eq!(
            structure.struct_flags.0 & 0x7,
            expected,
            "{label}: the TypeObject carries the extensibility the frontend decided"
        );
    }
}

/// The OMG `dds-xtypes` suite's `ext_autoid_1`: `@autoid` on both
/// structures, and the reader's member names its writer counterpart
/// through `@hashid("x1")`. Both members carry the hash id of `x1`
/// (§7.3.1.2.1.1, §7.3.1.2.1.3), so the two `@final` structures differ in
/// a member name and nothing else; §7.2.4.4.4.5 lets that through when
/// `ignore_member_names` is set and refuses it otherwise ("members with
/// the same ID also have the same name").
#[test]
fn hashid_of_the_writer_member_name_makes_the_suite_pair_assignable() {
    let ast = model(
        "@autoid @final struct struct_hashid_1 { int32 x1; };
         @autoid @final struct struct_hashid_2 { @hashid(\"x1\") int32 x2; };",
        "ext_autoid_1.idl",
    );
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("a registrable model");
    let writer = type_identifier(
        &ast,
        top_level(&ast, "struct_hashid_1", "ext_autoid_1.idl"),
        Form::Minimal,
    )
    .expect("a TypeIdentifier");
    let reader = type_identifier(
        &ast,
        top_level(&ast, "struct_hashid_2", "ext_autoid_1.idl"),
        Form::Minimal,
    )
    .expect("a TypeIdentifier");
    let ignoring = AssignabilityConfig {
        ignore_member_names: true,
        ..AssignabilityConfig::default()
    };
    assert_eq!(
        is_assignable(&writer, &reader, &registry, &ignoring),
        Assignable::Yes,
        "same member id, member names ignored"
    );
    assert!(
        !matches!(
            is_assignable(&writer, &reader, &registry, &AssignabilityConfig::default()),
            Assignable::Yes
        ),
        "with member names compared, x1 and x2 under one id is a mismatch"
    );
}
