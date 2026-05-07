//! T7.4 — Grammar-Coverage-Report-Generator.
//!
//! Walkt CST jedes Fixtures, sammelt aktiv-getroffene Production-IDs,
//! vergleicht mit allen 108 Productions in IDL_42 und schreibt einen
//! Markdown-Report nach `tests/coverage_report.md`.
//!
//! Threshold-Assertion (Phase 0): mindestens 70% der Base-Productions
//! muessen durch die Fixture-Suite getroffen werden. Aspirational
//! Phase-1-Ziel: 90%.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use zerodds_idl::cst::{build_cst, walk};
use zerodds_idl::engine::Engine;
use zerodds_idl::grammar::ProductionId;
use zerodds_idl::grammar::idl42::IDL_42;
use zerodds_idl::lexer::Tokenizer;

const FIXTURES: &[(&str, &str)] = &[
    (
        "zerodds_dcps.idl",
        include_str!("fixtures/omg/zerodds_dcps.idl"),
    ),
    (
        "zerodds_security.idl",
        include_str!("fixtures/omg/zerodds_security.idl"),
    ),
    (
        "dds_xtypes.idl",
        include_str!("fixtures/omg/dds_xtypes.idl"),
    ),
    (
        "cyclonedds/throughput.idl",
        include_str!("fixtures/cyclonedds/throughput.idl"),
    ),
    (
        "cyclonedds/listener.idl",
        include_str!("fixtures/cyclonedds/listener.idl"),
    ),
    (
        "fastdds/hello_world.idl",
        include_str!("fixtures/fastdds/hello_world.idl"),
    ),
    (
        "fastdds/security_topic.idl",
        include_str!("fixtures/fastdds/security_topic.idl"),
    ),
    (
        "spec_features/attr_raises.idl",
        include_str!("fixtures/spec_features/attr_raises.idl"),
    ),
    (
        "spec_features/user_annotations.idl",
        include_str!("fixtures/spec_features/user_annotations.idl"),
    ),
    (
        "spec_features/corba_value_repository.idl",
        include_str!("fixtures/spec_features/corba_value_repository.idl"),
    ),
    (
        "spec_features/ccm_components.idl",
        include_str!("fixtures/spec_features/ccm_components.idl"),
    ),
    (
        "spec_features/template_modules.idl",
        include_str!("fixtures/spec_features/template_modules.idl"),
    ),
    (
        "spec_features/cyclonedds_pragmas.idl",
        include_str!("fixtures/spec_features/cyclonedds_pragmas.idl"),
    ),
    (
        "spec_features/fastdds_pragmas.idl",
        include_str!("fixtures/spec_features/fastdds_pragmas.idl"),
    ),
    (
        "spec_features/rti_connext_pragmas.idl",
        include_str!("fixtures/spec_features/rti_connext_pragmas.idl"),
    ),
];

const COVERAGE_THRESHOLD_PERCENT: usize = 70;

/// Grammar-Coverage-Report-Generator. `#[ignore]` weil Threshold von
/// Spec-/Fixture-Wachstum abhaengt — explizit auszufuehren via
/// `cargo test -p zerodds-idl --test coverage_report -- --ignored`. Der
/// generierte `coverage_report.md` wird ins Repo committet, sodass
/// Status auch ohne CI-Lauf sichtbar ist.
#[test]
#[ignore = "manueller Lauf — Grammar-Coverage-Report; CI faellt bei Spec-Wachstum sonst nicht-deterministisch"]
fn generate_grammar_coverage_report() {
    let mut per_fixture: Vec<(&str, BTreeSet<ProductionId>)> = Vec::new();
    let mut union_covered: BTreeSet<ProductionId> = BTreeSet::new();

    for (name, src) in FIXTURES {
        let covered =
            collect_covered(src).unwrap_or_else(|e| panic!("fixture {name} did not parse: {e}"));
        union_covered.extend(covered.iter().copied());
        per_fixture.push((*name, covered));
    }

    let total = IDL_42.production_count();
    let covered_count = union_covered.len();
    let pct = (covered_count * 100) / total;

    let mut md = String::new();
    md.push_str("# IDL_42 Grammar-Coverage-Report\n\n");
    md.push_str(
        "Auto-generiert durch `cargo test -p zerodds-idl --test coverage_report`. \
         Walkt jeden Fixture-CST, sammelt aktive Production-IDs.\n\n",
    );
    md.push_str(&format!(
        "**Total Productions**: {total}\n\n\
         **Union Coverage**: {covered_count} / {total} = **{pct}%** \
         (Threshold {COVERAGE_THRESHOLD_PERCENT}%)\n\n",
    ));

    md.push_str("## Per-Fixture-Coverage\n\n");
    md.push_str("| Fixture | Productions getroffen | %  |\n");
    md.push_str("|---|---:|---:|\n");
    for (name, covered) in &per_fixture {
        let p = (covered.len() * 100) / total;
        md.push_str(&format!("| {name} | {} | {p}% |\n", covered.len()));
    }

    md.push_str("\n## Nicht abgedeckte Productions\n\n");
    let mut uncovered: Vec<&zerodds_idl::grammar::Production> = IDL_42
        .productions_iter()
        .filter(|p| !union_covered.contains(&p.id))
        .collect();
    uncovered.sort_by_key(|p| p.id.0);
    if uncovered.is_empty() {
        md.push_str("_Keine_ — alle Productions getroffen.\n");
    } else {
        md.push_str("| ID | Name | Spec |\n");
        md.push_str("|---|---|---|\n");
        for p in &uncovered {
            md.push_str(&format!(
                "| {} | `{}` | {}/{} |\n",
                p.id.0, p.name, p.spec_ref.doc, p.spec_ref.section
            ));
        }
    }

    md.push_str(
        "\n---\n\n\
         *Report generated by `tests/coverage_report.rs`. Re-run with \
         `cargo test -p zerodds-idl --test coverage_report` to refresh.*\n",
    );

    let out_path = report_output_path();
    std::fs::write(&out_path, md)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", out_path.display()));

    assert!(
        pct >= COVERAGE_THRESHOLD_PERCENT,
        "Grammar-Coverage {pct}% unter Threshold {COVERAGE_THRESHOLD_PERCENT}%; \
         siehe {} fuer untouched Productions",
        out_path.display()
    );
}

fn collect_covered(src: &str) -> Result<BTreeSet<ProductionId>, String> {
    let tokenizer = Tokenizer::for_grammar(&IDL_42);
    let stream = tokenizer.tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let engine = Engine::new(&IDL_42);
    let result = engine
        .recognize(stream.tokens())
        .map_err(|e| format!("parse: {e:?}"))?;
    let cst = build_cst(engine.compiled_grammar(), stream.tokens(), &result)
        .ok_or_else(|| "cst build failed".to_string())?;
    let mut ids = BTreeSet::new();
    for node in walk::preorder(&cst) {
        if let Some(id) = node.production() {
            // Beschraenken auf Base-Grammar-IDs (synth-IDs aus
            // Compile-Pass uebergehen wir; wir wollen Coverage der
            // OMG-IDL-4.2-Grammar messen).
            if (id.0 as usize) < IDL_42.production_count() {
                ids.insert(id);
            }
        }
    }
    Ok(ids)
}

fn report_output_path() -> PathBuf {
    // Workspace-Root → crates/idl/tests/coverage_report.md
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set during cargo test");
    PathBuf::from(manifest_dir)
        .join("tests")
        .join("coverage_report.md")
}
